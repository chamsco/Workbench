//! Where a project stands in specify → plan → execute → test → ship →
//! commit, and the record of every gate it has passed or been refused at.
//!
//! The app shows four stages over those phases (see [`Stage`]): Plan
//! (specify, plan), Execute (execute, test), Ship, and Commit, where the
//! Librarian files what the agents learned before the project is done.
//!
//! The phase lives on the project state, not in a prompt, so the harness and
//! the UI read the same answer. A gate is passed by a rule, a model or a
//! person, and says which: a model may refuse a gate, but only a rule or a
//! person opens one (docs/PLAN-PHASES v2).

use std::path::Path;

use serde::{Deserialize, Serialize};

const FILE: &str = ".backspace/phase.json";

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Specify,
    Plan,
    Execute,
    Test,
    Ship,
    /// Shipped; the Librarian is filing the notes (stages.rs).
    Commit,
    Done,
}

impl Phase {
    pub const ALL: [Phase; 7] = [Phase::Specify, Phase::Plan, Phase::Execute, Phase::Test, Phase::Ship, Phase::Commit, Phase::Done];

    /// The phase a passed gate leads to.
    pub fn next(self) -> Phase {
        match self {
            Phase::Specify => Phase::Plan,
            Phase::Plan => Phase::Execute,
            Phase::Execute => Phase::Test,
            Phase::Test => Phase::Ship,
            Phase::Ship => Phase::Commit,
            Phase::Commit | Phase::Done => Phase::Done,
        }
    }

    /// The stage the app shows for this phase.
    pub fn stage(self) -> Stage {
        match self {
            Phase::Specify | Phase::Plan => Stage::Plan,
            Phase::Execute | Phase::Test => Stage::Execute,
            Phase::Ship => Stage::Ship,
            Phase::Commit | Phase::Done => Stage::Commit,
        }
    }
}

/// What the app shows: four stages, each with its own decision model and spend.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Plan,
    Execute,
    Ship,
    Commit,
}

impl Stage {
    pub const ALL: [Stage; 4] = [Stage::Plan, Stage::Execute, Stage::Ship, Stage::Commit];

    /// The key prefs and the UI use ("plan", "execute", "ship", "commit").
    pub fn key(self) -> &'static str {
        match self {
            Stage::Plan => "plan",
            Stage::Execute => "execute",
            Stage::Ship => "ship",
            Stage::Commit => "commit",
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
    /// Dollars spent per stage before the current one was entered: the agents'
    /// work, and the decisions (routing and gate reviews) on top.
    #[serde(default)]
    pub spent: std::collections::BTreeMap<Stage, f64>,
    #[serde(default)]
    pub decided: std::collections::BTreeMap<Stage, f64>,
    /// The project's total spend, and its decision spend, when the current
    /// stage was entered; the current stage's own spend is what has gone on
    /// top of these since.
    #[serde(default)]
    pub mark_usd: f64,
    #[serde(default)]
    pub mark_decided_usd: f64,
    /// The stage the marks belong to.
    #[serde(default)]
    pub mark_stage: Option<Stage>,
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

    /// A decision model refused a gate the rules had already opened: the
    /// project goes back to the phase that gate closes. Models may block,
    /// never open.
    pub fn veto(&mut self, phase: Phase, at: u64, artifact: &str, note: &str) {
        self.gates.push(Gate { phase, passed: false, by: GateBy::Model, at, artifact: artifact.to_string(), note: note.to_string() });
        if self.phase > phase {
            self.phase = phase;
        }
    }

    /// Put spend on the stage it happened in. `total` and `decided` are the
    /// project's running totals now; whatever went on since the last call
    /// belongs to the stage the project was in then. True when anything moved.
    pub fn charge(&mut self, total: f64, decided: f64) -> bool {
        let now = self.phase.stage();
        let Some(was) = self.mark_stage else {
            self.mark_stage = Some(now);
            self.mark_usd = total;
            self.mark_decided_usd = decided;
            return true;
        };
        if was == now && total >= self.mark_usd && decided >= self.mark_decided_usd {
            return false;
        }
        // A total that went down is a fresh project state: start counting again.
        *self.spent.entry(was).or_default() += (total - self.mark_usd).max(0.0);
        *self.decided.entry(was).or_default() += (decided - self.mark_decided_usd).max(0.0);
        self.mark_stage = Some(now);
        self.mark_usd = total;
        self.mark_decided_usd = decided;
        true
    }

    /// A decision a stage paid for outside the project's totals (a gate review).
    pub fn add_decision(&mut self, stage: Stage, usd: f64) {
        *self.decided.entry(stage).or_default() += usd.max(0.0);
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
        assert_eq!(Phase::ALL.len(), 7);
        assert_eq!(Phase::Ship.next(), Phase::Commit);
        assert_eq!(Phase::Done.next(), Phase::Done);
    }

    #[test]
    fn four_stages_over_seven_phases() {
        let stages: Vec<Stage> = Phase::ALL.iter().map(|p| p.stage()).collect();
        assert_eq!(stages, [Stage::Plan, Stage::Plan, Stage::Execute, Stage::Execute, Stage::Ship, Stage::Commit, Stage::Commit]);
        assert_eq!(serde_json::to_value(Stage::Commit).unwrap(), "commit");
    }

    #[test]
    fn spend_lands_on_the_stage_it_happened_in() {
        let mut p = PhaseState::default();
        assert!(p.charge(1.0, 0.1), "the first call only sets the marks");
        assert!(!p.charge(1.0, 0.1));
        p.record(true, GateBy::Rules, 1, "SPEC.md", "ok");
        p.record(true, GateBy::Rules, 2, "tickets", "ok");
        assert!(p.charge(3.5, 0.3));
        assert_eq!(p.spent[&Stage::Plan], 2.5);
        assert!((p.decided[&Stage::Plan] - 0.2).abs() < 1e-9);
        assert_eq!(p.mark_stage, Some(Stage::Execute));
        p.add_decision(Stage::Execute, 0.05);
        assert_eq!(p.decided[&Stage::Execute], 0.05);
    }

    #[test]
    fn a_veto_closes_the_gate_again() {
        let mut p = PhaseState::default();
        p.record_spec(true, 1, "rules passed");
        assert_eq!(p.phase, Phase::Plan);
        p.veto(Phase::Specify, 2, "SPEC.md", "acceptance A2 can't be checked");
        assert_eq!(p.phase, Phase::Specify);
        let g = p.gates.last().unwrap();
        assert!(!g.passed && g.by == GateBy::Model);
    }
}
