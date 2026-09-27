//! Tickets are the unit of work, shaped after mattpocock/skills' to-tickets
//! and triage: a vertical slice with acceptance criteria, blocking edges and a
//! triage state. Each is mirrored to `.backspace/tickets/NN-key.md`.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::project::AgentId;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Bug,
    Enhancement,
    Task,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TicketState {
    /// Proposed in a plan the human has not approved yet.
    Proposed,
    NeedsTriage,
    NeedsInfo,
    ReadyForAgent,
    ReadyForHuman,
    /// Dispatched, waiting on its blockers.
    Queued,
    InProgress,
    InReview,
    Done,
    Failed,
    Wontfix,
}

impl TicketState {
    pub fn label(self) -> &'static str {
        match self {
            TicketState::Proposed => "proposed",
            TicketState::NeedsTriage => "needs-triage",
            TicketState::NeedsInfo => "needs-info",
            TicketState::ReadyForAgent => "ready-for-agent",
            TicketState::ReadyForHuman => "ready-for-human",
            TicketState::Queued => "queued",
            TicketState::InProgress => "in-progress",
            TicketState::InReview => "in-review",
            TicketState::Done => "done",
            TicketState::Failed => "failed",
            TicketState::Wontfix => "wontfix",
        }
    }

    pub fn parse(s: &str) -> Option<TicketState> {
        Some(match s.replace('-', "_").as_str() {
            "needs_triage" => TicketState::NeedsTriage,
            "needs_info" => TicketState::NeedsInfo,
            "ready_for_agent" => TicketState::ReadyForAgent,
            "ready_for_human" => TicketState::ReadyForHuman,
            "wontfix" => TicketState::Wontfix,
            _ => return None,
        })
    }

    pub fn is_settled(self) -> bool {
        matches!(
            self,
            TicketState::Done | TicketState::Failed | TicketState::Wontfix
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ticket {
    pub num: usize,
    pub key: String,
    pub title: String,
    pub category: Category,
    pub state: TicketState,
    pub what_to_build: String,
    pub acceptance: Vec<String>,
    /// Shell command that must exit 0 before the work reaches a reviewer.
    pub check: Option<String>,
    pub out_of_scope: Vec<String>,
    /// Keys of sibling tickets that must be done first.
    pub blocked_by: Vec<String>,
    /// Agent that planned it and reviews it (or hands it to the human).
    pub owner: AgentId,
    pub assignee: Option<AgentId>,
    pub budget_usd: Option<f64>,
    pub notes: Vec<String>,
}

impl Ticket {
    /// Parse one ticket from a tool call. `state` and `owner` come from context.
    pub fn from_json(v: &Value, num: usize, owner: AgentId, state: TicketState) -> Result<Ticket> {
        let s = |k: &str| v[k].as_str().map(str::to_owned);
        let list = |k: &str| -> Vec<String> {
            v[k].as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        let key = s("key").context("each ticket needs a `key`")?;
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            bail!("ticket key `{key}` must be letters, digits, - or _ (it names a git branch)");
        }
        let what = s("what_to_build")
            .or_else(|| s("description"))
            .context("each ticket needs `what_to_build`")?;
        Ok(Ticket {
            num,
            title: s("title").unwrap_or_else(|| key.clone()),
            key,
            category: match v["category"].as_str() {
                Some("bug") => Category::Bug,
                Some("enhancement") => Category::Enhancement,
                _ => Category::Task,
            },
            state,
            what_to_build: what,
            acceptance: list("acceptance"),
            check: s("check").filter(|c| !c.trim().is_empty()),
            out_of_scope: list("out_of_scope"),
            blocked_by: list("blocked_by"),
            owner,
            assignee: None,
            budget_usd: v["budget_usd"].as_f64(),
            notes: vec![],
        })
    }

    /// The agent brief: mattpocock's AGENT-BRIEF shape, minus file paths.
    pub fn brief(&self) -> String {
        let mut b = format!(
            "# {}: {}\n\n**Category:** {:?}\n\n**What to build:**\n{}\n",
            self.key, self.title, self.category, self.what_to_build
        );
        if !self.acceptance.is_empty() {
            b.push_str("\n**Acceptance criteria:**\n");
            for a in &self.acceptance {
                b.push_str(&format!("- [ ] {a}\n"));
            }
        }
        if let Some(c) = &self.check {
            b.push_str(&format!(
                "\n**Check:** `{c}` must exit 0 in your worktree before your work reaches review. It runs automatically when you submit.\n"
            ));
        }
        if !self.out_of_scope.is_empty() {
            b.push_str("\n**Out of scope:**\n");
            for o in &self.out_of_scope {
                b.push_str(&format!("- {o}\n"));
            }
        }
        if !self.notes.is_empty() {
            b.push_str("\n**Notes:**\n");
            for n in &self.notes {
                b.push_str(&format!("- {n}\n"));
            }
        }
        b
    }

    pub fn markdown(&self) -> String {
        let blocked = if self.blocked_by.is_empty() {
            "None (can start immediately)".to_string()
        } else {
            self.blocked_by.join(", ")
        };
        format!(
            "{}\n**Blocked by:** {blocked}\n\n**Status:** {}\n",
            self.brief(),
            self.state.label()
        )
    }

    pub fn write(&self, root: &Path) {
        let dir = root.join(".backspace/tickets");
        if std::fs::create_dir_all(&dir).is_ok() {
            let _ = std::fs::write(
                dir.join(format!("{:02}-{}.md", self.num, self.key)),
                self.markdown(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_and_renders() {
        let t = Ticket::from_json(
            &json!({"key": "api", "title": "REST API", "category": "enhancement",
                    "what_to_build": "CRUD for todos", "acceptance": ["GET /todos lists"],
                    "check": "cargo test", "blocked_by": ["schema"]}),
            3,
            0,
            TicketState::Proposed,
        )
        .unwrap();
        let md = t.markdown();
        assert!(md.contains("- [ ] GET /todos lists"));
        assert!(md.contains("**Blocked by:** schema"));
        assert!(md.contains("`cargo test`"));
        assert!(Ticket::from_json(
            &json!({"key": "bad key", "what_to_build": "x"}),
            1,
            0,
            TicketState::Proposed
        )
        .is_err());
    }
}
