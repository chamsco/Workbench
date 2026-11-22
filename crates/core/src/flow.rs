//! A coordinator's flow (Charles, 22 Nov): the steps every coordinator stream works through by
//! default, Ask, Plan, Build, Test, Verify, Ship, Learn, each with what it does and the skill
//! it loads (Test loads `sharp-testing`). The steps are the user's: they edit, reorder, turn off
//! or add steps in Settings > Flow, and the Learn step may propose a change to a step, which
//! the user applies from a card. So the flow grows with them and with the codebase.
//!
//! Where it lives: `<project>/.backspace/flow.json` when a project has one (a team's flow,
//! versioned with its code), else `<data>/flow.json`, else the defaults.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Step {
    /// Stable id, lowercase ("test"); a change proposal names a step by it.
    pub key: String,
    pub name: String,
    /// What the agent does in this step, in the user's words.
    pub does: String,
    /// The skill the agent loads at this step, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    #[serde(default = "yes")]
    pub on: bool,
}

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Flow {
    pub steps: Vec<Step>,
}

fn step(key: &str, name: &str, does: &str, skill: Option<&str>) -> Step {
    Step { key: key.into(), name: name.into(), does: does.into(), skill: skill.map(str::to_string), on: true }
}

impl Default for Flow {
    fn default() -> Self {
        Flow {
            steps: vec![
                step("ask", "Ask", "Say in a sentence or two what is being asked. If something that changes the outcome is unclear, ask before going further; otherwise say what you assumed.", None),
                step("plan", "Plan", "List the steps and the files you will touch, briefly. For a large change, write the plan as a todo block first.", None),
                step("build", "Build", "Make the change in small steps, keeping the project building.", None),
                step("test", "Test", "Decide which tests the change needs by the sharp-testing rules, write only those, and run the suites that cover the change.", Some("sharp-testing")),
                step("verify", "Verify", "Check the result against what was asked: run it and look at it (screenshots for anything visual), and say what you checked and what you could not.", None),
                step("ship", "Ship", "Commit with a clear message and push the branch. Never merge, and never open a pull request without the user's go-ahead.", None),
                step("learn", "Learn", "Say in a line what you would do differently next time. If a step of this flow should change for this user or codebase, propose it in a `flow` block.", None),
            ],
        }
    }
}

/// A change to one step, proposed by an agent in a fenced `flow` block and applied by the user.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Change {
    pub step: String,
    #[serde(default)]
    pub does: Option<String>,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub why: String,
}

impl Change {
    pub fn parse(body: &str) -> Option<Change> {
        serde_json::from_str::<Change>(body.trim()).ok().filter(|c| !c.step.trim().is_empty() && (c.does.is_some() || c.skill.is_some()))
    }
}

impl Flow {
    pub fn user_file(data: &Path) -> PathBuf {
        data.join("flow.json")
    }

    pub fn project_file(project: &Path) -> PathBuf {
        project.join(".backspace").join("flow.json")
    }

    /// The flow for work in `project`: the project's own, else the user's, else the defaults.
    /// A file that does not parse counts as absent, so a typo never empties the flow.
    pub fn load(data: &Path, project: Option<&Path>) -> Flow {
        let read = |p: PathBuf| std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str::<Flow>(&s).ok()).filter(|f| !f.steps.is_empty());
        project.and_then(|p| read(Self::project_file(p))).or_else(|| read(Self::user_file(data))).unwrap_or_default()
    }

    pub fn save(&self, file: &Path) -> anyhow::Result<()> {
        if let Some(d) = file.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(file, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Applies `c` to its step. False when no step has that key.
    pub fn apply(&mut self, c: &Change) -> bool {
        let Some(s) = self.steps.iter_mut().find(|s| s.key.eq_ignore_ascii_case(c.step.trim())) else { return false };
        if let Some(d) = c.does.as_ref().filter(|d| !d.trim().is_empty()) {
            s.does = d.trim().to_string();
        }
        if let Some(k) = &c.skill {
            s.skill = Some(k.trim().to_string()).filter(|k| !k.is_empty());
        }
        true
    }

    /// The system-prompt section a coordinator works by. Empty when every step is off.
    pub fn brief(&self) -> String {
        let on: Vec<&Step> = self.steps.iter().filter(|s| s.on).collect();
        if on.is_empty() {
            return String::new();
        }
        let mut s = String::from("# How you work\n\nThis user's flow for a piece of work, in order. Follow it for anything beyond a quick question, without announcing the steps; skip a step that has nothing to do and say so in a few words.\n\n");
        for (i, st) in on.iter().enumerate() {
            s.push_str(&format!("{}. {} ({}): {}", i + 1, st.name, st.key, st.does));
            if let Some(k) = &st.skill {
                s.push_str(&format!(" Load the `{k}` skill for this step."));
            }
            s.push('\n');
        }
        s.push_str("\nTo propose a change to a step, put a fenced `flow` block with one JSON object: {\"step\": \"test\", \"does\": \"the new instructions\", \"why\": \"one sentence\"} (or \"skill\" instead of \"does\"). The user decides whether to apply it.");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The project's flow wins over the user's; a broken file falls back instead of emptying it.
    #[test]
    fn a_projects_flow_wins_and_a_broken_one_falls_back() {
        let root = std::env::temp_dir().join(format!("cauldron-flow-{}", std::process::id()));
        let (data, project) = (root.join("data"), root.join("proj"));
        let _ = std::fs::remove_dir_all(&root);
        let mut mine = Flow::default();
        mine.steps.truncate(2);
        mine.save(&Flow::user_file(&data)).unwrap();
        let mut team = Flow::default();
        team.steps.truncate(1);
        team.save(&Flow::project_file(&project)).unwrap();
        assert_eq!(Flow::load(&data, Some(&project)).steps.len(), 1, "the project's own");
        std::fs::write(Flow::project_file(&project), "{ not json").unwrap();
        assert_eq!(Flow::load(&data, Some(&project)).steps.len(), 2, "a broken project file: the user's");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A proposal changes only its step, and only when that step exists.
    #[test]
    fn a_proposed_change_lands_on_its_step() {
        let mut f = Flow::default();
        let c = Change::parse(r#"{"step": "Test", "does": "Run only the crate you touched.", "why": "The full suite takes 20 minutes."}"#).unwrap();
        assert!(f.apply(&c));
        let t = f.steps.iter().find(|s| s.key == "test").unwrap();
        assert_eq!(t.does, "Run only the crate you touched.");
        assert_eq!(t.skill.as_deref(), Some("sharp-testing"), "a change to what it does keeps its skill");
        assert!(!f.apply(&Change { step: "deploy".into(), does: Some("x".into()), skill: None, why: String::new() }));
        assert!(Change::parse(r#"{"step": "test", "why": "nothing to change"}"#).is_none(), "a proposal must change something");
    }

    /// A step turned off is not in the brief.
    #[test]
    fn steps_turned_off_leave_the_brief() {
        let mut f = Flow::default();
        f.steps.iter_mut().filter(|s| s.key != "test").for_each(|s| s.on = false);
        let b = f.brief();
        assert!(b.contains("(test)") && !b.contains("(ship)"));
        f.steps.iter_mut().for_each(|s| s.on = false);
        assert!(f.brief().is_empty());
    }
}
