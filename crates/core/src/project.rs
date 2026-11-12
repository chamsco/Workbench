use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::router::{AgentRole, Decision};
use crate::ticket::Ticket;

pub type AgentId = usize;
pub const MAIN: AgentId = 0;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Queued,
    Running,
    AwaitingApproval,
    Approved,
    Failed,
    /// The process went away mid-turn (Backspace closed, the machine slept).
    /// Distinct from `Failed`, which means the work itself did not come out:
    /// an interrupted agent's branch, worktree and CLI session are all still
    /// there, so re-dispatching the ticket carries on rather than starting
    /// over. Absent from anything that reports failure to the human.
    Interrupted,
}

impl AgentStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, AgentStatus::Approved | AgentStatus::Failed)
    }

    /// Is the agent still doing something?
    pub fn is_active(self) -> bool {
        matches!(
            self,
            AgentStatus::Queued | AgentStatus::Running | AgentStatus::AwaitingApproval
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    User,
    Assistant,
    ToolCall,
    ToolResult,
    System,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LogEntry {
    pub kind: LogKind,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Main,
    /// Works one ticket in its own worktree.
    Worker,
    /// Classifies a filed ticket; read-only.
    Triage,
    /// Looks something up for another agent on a small fast model; read-only.
    Scout,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Deliverable {
    pub summary: String,
    pub files: Vec<String>,
    /// `git diff --stat` against the parent branch, when isolated.
    #[serde(default)]
    pub diff_stat: Option<String>,
    /// Exact per-file line counts against the parent branch.
    #[serde(default)]
    pub changes: Vec<FileChange>,
    /// The ticket check that passed before this reached review.
    #[serde(default)]
    pub check_passed: Option<String>,
    /// What the worker reports it ran to verify the work. Its own claim: the
    /// ticket check above is the one Backspace ran itself.
    #[serde(default)]
    pub checks_run: Vec<CheckRun>,
    /// What the worker assumed where the request or spec did not say.
    #[serde(default)]
    pub assumptions: Vec<String>,
    /// What the worker could not settle and someone should decide.
    #[serde(default)]
    pub open_questions: Vec<String>,
    /// False when the worker gave no structured record (a coding CLI that
    /// did not end with one): the lists above are then unknown, not empty.
    #[serde(default)]
    pub reported: bool,
}

/// One check a worker ran, as it reported it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CheckRun {
    pub command: String,
    /// What it showed, in the worker's words ("12 passed", "lint clean").
    pub result: String,
    pub exit_status: Option<i32>,
}

impl Deliverable {
    /// The files this delivery changed: git's count when there is one, else
    /// what the agent listed.
    pub fn files_changed(&self) -> Vec<String> {
        if self.changes.is_empty() {
            self.files.clone()
        } else {
            self.changes.iter().map(|c| c.path.clone()).collect()
        }
    }

    /// The delivery record a dependent ticket (and a reviewer) reads in place
    /// of the worker's prose alone.
    pub fn record(&self) -> String {
        let list = |items: &[String]| -> String {
            if items.is_empty() {
                "- none\n".into()
            } else {
                items.iter().map(|i| format!("- {i}\n")).collect()
            }
        };
        let mut s = format!("**Summary:** {}\n", self.summary.trim());
        s.push_str("\n**files_changed:**\n");
        if self.changes.is_empty() {
            s.push_str(&list(&self.files));
        } else {
            for c in &self.changes {
                s.push_str(&format!("- {} (+{} -{})\n", c.path, c.added, c.removed));
            }
        }
        if let Some(d) = &self.diff_stat {
            s.push_str(&format!("\n**diff_stat:**\n```\n{}\n```\n", d.trim_end()));
        }
        if let Some(c) = &self.check_passed {
            s.push_str(&format!("\n**Ticket check (run by Backspace):** `{c}` exited 0\n"));
        }
        if !self.reported {
            s.push_str("\n**checks_run, assumptions, open_questions:** not reported by the worker; treat them as unknown, not as none.\n");
            return s;
        }
        s.push_str("\n**checks_run (as the worker reported them):**\n");
        if self.checks_run.is_empty() {
            s.push_str("- none\n");
        }
        for c in &self.checks_run {
            let code = c.exit_status.map(|x| x.to_string()).unwrap_or_else(|| "not given".into());
            s.push_str(&format!("- `{}` exit {code}: {}\n", c.command, c.result));
        }
        s.push_str("\n**assumptions:**\n");
        s.push_str(&list(&self.assumptions));
        s.push_str("\n**open_questions:**\n");
        s.push_str(&list(&self.open_questions));
        s
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FileChange {
    pub path: String,
    pub added: u32,
    pub removed: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentRecord {
    pub id: AgentId,
    pub role: AgentRole,
    pub kind: AgentKind,
    pub parent: Option<AgentId>,
    pub depth: usize,
    pub key: String,
    pub title: String,
    pub brief: String,
    pub depends_on: Vec<AgentId>,
    pub status: AgentStatus,
    pub decision: Option<Decision>,
    pub log: Vec<LogEntry>,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub deliverable: Option<Deliverable>,
    /// Spend cap for this agent and everything it spawns.
    pub budget_usd: Option<f64>,
    /// Ticket this agent works or triages.
    pub ticket: Option<String>,
    pub branch: Option<String>,
    pub worktree: Option<PathBuf>,
    /// One line per step up the ladder, with the reason.
    pub escalations: Vec<String>,
    /// Trace ids of this agent's runs, oldest first (`<data>/traces`).
    #[serde(default)]
    pub traces: Vec<String>,
    /// Times this agent consulted the advisor.
    #[serde(default)]
    pub advisor_calls: u32,
    /// The coding CLI's own session for this agent, when it keeps one (Claude
    /// Code does). Saved with the state so a worker that was cut off by a
    /// restart answers again *in its own session* instead of starting cold —
    /// the second half of the audit's P0-2.
    #[serde(default)]
    pub ses: Option<String>,
    /// What this agent learned that may be worth remembering, and what its
    /// own agents handed up when they finished (`leave_note`). The main
    /// agent's list is the project's compiled notes, which the Librarian
    /// files at Commit (stages.rs).
    #[serde(default)]
    pub notes: Vec<FieldNote>,
}

/// One note on its way up to the Librarian.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct FieldNote {
    pub text: String,
    /// The agent that first wrote it (its key).
    pub from: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected { feedback: String },
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    /// A batch of tickets from the main agent, before any work starts.
    Plan,
    Deliverable,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Approval {
    pub id: usize,
    pub kind: ApprovalKind,
    pub agent: AgentId,
    pub deliverable: Deliverable,
    /// Ticket keys: the plan's tickets, or the one ticket delivered.
    pub tickets: Vec<String>,
    pub state: ApprovalState,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectState {
    pub name: String,
    pub workspace: PathBuf,
    pub agents: Vec<AgentRecord>,
    pub approvals: Vec<Approval>,
    pub tickets: Vec<Ticket>,
    pub total_cost_usd: f64,
    pub router_cost_usd: f64,
    /// Part of `router_cost_usd` and `total_cost_usd`: routing calls that were
    /// sent but whose cost was never reported, counted at the amount held for
    /// them. Kept apart so the person can see how much of the spend is a
    /// conservative count and not a reported cost.
    #[serde(default)]
    pub router_unknown_cost_usd: f64,
    pub config_source: Option<PathBuf>,
    /// Messages agents sent each other (see `board`).
    #[serde(default)]
    pub board: Vec<crate::board::BoardMsg>,
    /// A CLI planner's session, resumed on the next message (and after a
    /// restart).
    #[serde(default)]
    pub planner_session: Option<String>,
    /// Canvases agents asked to open or close, in order; the shell applies
    /// the ones it hasn't yet (by `id`).
    #[serde(default)]
    pub canvas_ops: Vec<CanvasOp>,
    /// Sets of visual options an agent showed the user (see `options`).
    #[serde(default)]
    pub options: Vec<crate::options::OptionSet>,
    /// What the agents produced: diffs, images, documents, tables, and the
    /// things they refer to (see `artifact`). Bounded by `MAX_ARTIFACTS`.
    #[serde(default)]
    pub artifacts: Vec<crate::artifact::Artifact>,
    /// Specify → plan → execute → test → ship, and every gate answer so far.
    #[serde(default)]
    pub phase: crate::phase::PhaseState,
    /// Intake (intake.rs): the path the current task was given, and a task
    /// waiting on the user's answer to "which kind is this?".
    #[serde(default)]
    pub intake: crate::intake::IntakeState,
}

/// An agent opening or closing a canvas: a browser on its dev server, its
/// worktree's files, an agent's session, the diagram, PLAN.md, team chat.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CanvasOp {
    pub id: u64,
    /// "open" or "close".
    pub op: String,
    /// "browser", "files", "agent", "diagram", "docs" or "board".
    pub kind: String,
    /// For "agent" and "files": whose (an agent key; empty: the asker).
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    /// For "options": which set to show (`OptionSet::id`).
    #[serde(default)]
    pub option: Option<String>,
    /// The agent that asked.
    pub by: String,
}

impl ProjectState {
    /// A project with nothing in it yet (a remote machine before it answers).
    pub fn empty(name: &str) -> ProjectState {
        ProjectState {
            name: name.to_string(),
            workspace: PathBuf::new(),
            agents: vec![],
            approvals: vec![],
            tickets: vec![],
            total_cost_usd: 0.0,
            router_cost_usd: 0.0,
            router_unknown_cost_usd: 0.0,
            config_source: None,
            board: vec![],
            planner_session: None,
            canvas_ops: vec![],
            options: vec![],
            artifacts: vec![],
            phase: Default::default(),
            intake: Default::default(),
        }
    }

    pub fn pending_approvals(&self) -> impl Iterator<Item = &Approval> {
        self.approvals
            .iter()
            .filter(|a| a.state == ApprovalState::Pending)
    }

    /// This agent's spend plus every descendant's.
    pub fn subtree_cost(&self, id: AgentId) -> f64 {
        self.agents
            .iter()
            .filter(|a| self.ancestry(a.id).contains(&id))
            .map(|a| a.cost_usd)
            .sum()
    }

    /// `id` followed by its parent, grandparent, ... up to main.
    pub fn ancestry(&self, id: AgentId) -> Vec<AgentId> {
        let mut chain = vec![id];
        let mut cur = id;
        while let Some(p) = self.agents[cur].parent {
            chain.push(p);
            cur = p;
        }
        chain
    }

    /// Workers only: triage agents are cheap and must never be blocked by the cap.
    pub fn subagent_count(&self) -> usize {
        self.agents
            .iter()
            .filter(|a| a.kind == AgentKind::Worker)
            .count()
    }

    pub fn ticket(&self, key: &str) -> Option<&Ticket> {
        self.tickets.iter().find(|t| t.key == key)
    }

    pub fn ticket_mut(&mut self, key: &str) -> Option<&mut Ticket> {
        self.tickets.iter_mut().find(|t| t.key == key)
    }
}
