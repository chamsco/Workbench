//! The cost ladder (PLAN §3.1): where a step starts, and when it may move up.
//! A step starts at the cheapest rung that can do the job and climbs only on
//! a measured failure, never by default.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::ModelSpec;
use crate::decision::Triage;

/// Which rung of the ladder a model sits on. `Frontier` is a property of the
/// model, not of its price: a model billed to a subscription at $0 is still a
/// frontier call.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelClass {
    Local,
    Mid,
    Frontier,
}

impl ModelClass {
    pub fn as_str(self) -> &'static str {
        unimplemented!("M2")
    }
}

/// The model's `class` when the config sets one, otherwise by tier:
/// 0 local, 1–2 mid, 3–4 frontier.
pub fn class_of(_spec: &ModelSpec) -> ModelClass {
    unimplemented!("M2")
}

/// The highest class the planner may start on for this task.
pub fn planner_start(_triage: Option<&Triage>) -> ModelClass {
    unimplemented!("M2")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    TestFailed,
    CheckFailed,
    Rejected,
    TurnBudget,
    Loop,
}

/// Evidence that a rung did not work. Moving up requires one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub kind: FailureKind,
    pub evidence: String,
}

impl Failure {
    pub fn new(_kind: FailureKind, _evidence: impl Into<String>) -> Result<Failure> {
        unimplemented!("M2")
    }
}

/// The next rung up after a measured failure; nothing is above the frontier.
pub fn escalate(_from: ModelClass, _why: &Failure) -> Option<ModelClass> {
    unimplemented!("M2")
}

/// What has been measured about one rung on this kind of work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RungStats {
    pub attempts: u32,
    pub successes: u32,
    pub mean_cost_usd: f64,
}

impl RungStats {
    /// Laplace-smoothed, so an unmeasured rung is 0.5 and no rung is ever 0 or 1.
    pub fn p_success(&self) -> f64 {
        unimplemented!("M2")
    }

    /// Expected spend to get one success by retrying this rung.
    pub fn expected_cost_to_green(&self) -> f64 {
        unimplemented!("M2")
    }
}

/// True when retrying the cheap rung is expected to cost more than going to
/// the frontier now.
pub fn escalate_sooner(_cheap: &RungStats, _frontier: &RungStats) -> bool {
    unimplemented!("M2")
}

/// Keep the candidates at or below `ceiling`; when there are none, keep them
/// all, so a project with only strong models still routes.
pub fn within<'a>(_candidates: &[&'a ModelSpec], _ceiling: ModelClass) -> Vec<&'a ModelSpec> {
    unimplemented!("M2")
}

/// The cheapest candidate on exactly `class`; failing that the nearest class
/// below, then above. Used to price a policy, not to route.
pub fn cheapest_on<'a>(_candidates: &[&'a ModelSpec], _class: ModelClass) -> Option<&'a ModelSpec> {
    unimplemented!("M2")
}
