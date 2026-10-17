//! Where a project stands in specify → plan → execute → test → ship, and the
//! record of every gate it has passed or been refused at.
//!
//! The phase lives on the project state, not in a prompt, so the harness and
//! the UI read the same answer. A gate is passed by a rule, a model or a
//! person, and says which: a model may refuse a gate, but only a rule or a
//! person opens one (docs/PLAN-PHASES v2).

use std::path::Path;

use serde::{Deserialize, Serialize};

const FILE: &str = ".backspace/phase.json";

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Specify,
    Plan,
    Execute,
    Test,
    Ship,
    Done,
}

impl Phase {
    pub const ALL: [Phase; 6] = [Phase::Specify, Phase::Plan, Phase::Execute, Phase::Test, Phase::Ship, Phase::Done];

    /// The phase a passed gate leads to.
    pub fn next(self) -> Phase {
        match self {
            Phase::Specify => Phase::Plan,
            Phase::Plan => Phase::Execute,
            Phase::Execute => Phase::Test,
            Phase::Test => Phase::Ship,
            Phase::Ship | Phase::Done => Phase::Done,
        }
    }
}

/// Who answered a gate.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum GateBy {
    Rules,
    Model,
    Human,
}

/// One answer at one gate, kept whether it passed or not: a refused gate is
/// how a reader later sees what was missing.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Gate {
    pub phase: Phase,
    pub passed: bool,
    pub by: GateBy,
    /// Unix milliseconds.
    pub at: u64,
    /// The file the gate judged (`SPEC.md`, `PLAN.md`, …).
    pub artifact: String,
    /// What was missing, or why it passed.
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct PhaseState {
    pub phase: Phase,
    /// Every gate answer, oldest first.
    pub gates: Vec<Gate>,
}

impl PhaseState {
    /// Record a gate answer for the current phase. A pass moves the project on
    /// unless only a model said so: models may block, never open.
    pub fn record(&mut self, passed: bool, by: GateBy, at: u64, artifact: &str, note: &str) -> &Gate {
        let passed = passed && by != GateBy::Model;
        self.gates.push(Gate { phase: self.phase, passed, by, at, artifact: artifact.to_string(), note: note.to_string() });
        if passed {
            self.phase = self.phase.next();
        }
        self.gates.last().unwrap()
    }

    /// A spec gate answer, whatever phase the project is in. The spec is the
    /// root of everything after it: a refused rewrite sends the project back
    /// to Specify, and a passing one moves it on only from Specify.
    pub fn record_spec(&mut self, passed: bool, at: u64, note: &str) -> &Gate {
        if self.phase == Phase::Specify {
            return self.record(passed, GateBy::Rules, at, crate::spec::FILE, note);
        }
        if !passed {
            self.phase = Phase::Specify;
        }
        self.gates.push(Gate { phase: Phase::Specify, passed, by: GateBy::Rules, at, artifact: crate::spec::FILE.into(), note: note.to_string() });
        self.gates.last().unwrap()
    }

    /// The project's phase, from `<project>/.backspace/phase.json`. Its own
    /// file, not the harness's state.json: the spec gate runs from Code chat
    /// while a harness may be rewriting that one.
    pub fn load(project: &Path) -> PhaseState {
        std::fs::read_to_string(project.join(FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, project: &Path) -> anyhow::Result<()> {
        let path = project.join(FILE);
        std::fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    /// May the main agent create tickets? Only once the spec gate is behind us.
    pub fn may_plan(&self) -> bool {
        self.phase != Phase::Specify
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_is_kept_and_a_pass_moves_on() {
        let mut p = PhaseState::default();
        assert_eq!(p.phase, Phase::Specify);
        assert!(!p.may_plan());
        p.record(false, GateBy::Rules, 1, "SPEC.md", "Open questions: 2");
        assert_eq!(p.phase, Phase::Specify);
        p.record(true, GateBy::Rules, 2, "SPEC.md", "no open questions");
        assert_eq!(p.phase, Phase::Plan);
        assert!(p.may_plan());
        assert_eq!(p.gates.len(), 2);
        assert!(!p.gates[0].passed && p.gates[1].passed);
    }

    #[test]
    fn a_model_can_block_but_never_open() {
        let mut p = PhaseState::default();
        assert!(!p.record(true, GateBy::Model, 1, "SPEC.md", "looks complete").passed);
        assert_eq!(p.phase, Phase::Specify);
        p.record(true, GateBy::Human, 2, "SPEC.md", "approved by hand");
        assert_eq!(p.phase, Phase::Plan);
    }

    #[test]
    fn a_refused_respec_reopens_specify_and_a_pass_does_not_skip_ahead() {
        let mut p = PhaseState::default();
        p.record_spec(true, 1, "ok");
        assert_eq!(p.phase, Phase::Plan);
        p.record_spec(true, 2, "ok again");
        assert_eq!(p.phase, Phase::Plan, "a second pass must not jump to Execute");
        p.record_spec(false, 3, "Open questions: 1");
        assert_eq!(p.phase, Phase::Specify);
        assert!(p.gates.iter().all(|g| g.phase == Phase::Specify));
    }

    #[test]
    fn loads_and_saves_beside_the_project() {
        let dir = std::env::temp_dir().join(format!("phase-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(PhaseState::load(&dir), PhaseState::default());
        let mut p = PhaseState::default();
        p.record_spec(true, 1, "ok");
        p.save(&dir).unwrap();
        assert_eq!(PhaseState::load(&dir), p);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trips_and_old_state_loads_without_it() {
        let mut p = PhaseState::default();
        p.record(true, GateBy::Rules, 7, "SPEC.md", "ok");
        let back: PhaseState = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
        assert_eq!(serde_json::to_value(Phase::Execute).unwrap(), "execute");
        assert_eq!(Phase::ALL.len(), 6);
        assert_eq!(Phase::Done.next(), Phase::Done);
    }
}
