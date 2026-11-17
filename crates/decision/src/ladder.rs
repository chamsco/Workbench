//! The cost ladder (PLAN §3.1): where a step starts, and when it may move up.
//! A step starts at the cheapest rung that can do the job and climbs only on
//! a measured failure, never by default.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::decision::{TaskType, Triage};

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
        match self {
            ModelClass::Local => "local",
            ModelClass::Mid => "mid",
            ModelClass::Frontier => "frontier",
        }
    }
}

/// What the ladder needs to know about a model. The host implements it for
/// its own model description (Cauldron: `config::ModelSpec`).
pub trait Rung {
    fn id(&self) -> &str;
    /// 0 local, 1–3 mid, 4 frontier, unless [`Rung::class`] says otherwise.
    fn tier(&self) -> u8;
    /// An explicit class from the host's config, which beats the tier.
    fn class(&self) -> Option<ModelClass>;
    fn input_usd_per_mtok(&self) -> f64;
    fn output_usd_per_mtok(&self) -> f64;
}

impl<T: Rung + ?Sized> Rung for &T {
    fn id(&self) -> &str {
        (**self).id()
    }
    fn tier(&self) -> u8 {
        (**self).tier()
    }
    fn class(&self) -> Option<ModelClass> {
        (**self).class()
    }
    fn input_usd_per_mtok(&self) -> f64 {
        (**self).input_usd_per_mtok()
    }
    fn output_usd_per_mtok(&self) -> f64 {
        (**self).output_usd_per_mtok()
    }
}

/// The model's `model_class` (or older `class`) when the config sets one,
/// otherwise by tier: 0 local, 1–3 mid, 4 frontier. Only tier 4 is frontier by
/// default. Class is separate from tier and from effort: a tier-3 model that
/// takes high effort is still mid unless its config says `model_class = "frontier"`.
pub fn class_of<M: Rung + ?Sized>(spec: &M) -> ModelClass {
    spec.class().unwrap_or(match spec.tier() {
        0 => ModelClass::Local,
        1..=3 => ModelClass::Mid,
        _ => ModelClass::Frontier,
    })
}

/// The highest class the planner may start on for this task. PLAN §3.1 gives
/// the frontier two jobs: orchestrating long tasks, and escalation after a
/// failure. So only a long task, and only when the classifier is sure, starts
/// there; everything else starts mid and climbs on evidence. Unsure means
/// ask the user, not spend more.
pub fn planner_start(triage: Option<&Triage>) -> ModelClass {
    match triage {
        Some(Triage::Confident { task: TaskType::Long, .. }) => ModelClass::Frontier,
        _ => ModelClass::Mid,
    }
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
    pub fn new(kind: FailureKind, evidence: impl Into<String>) -> Result<Failure> {
        let evidence = evidence.into();
        if evidence.trim().is_empty() {
            bail!("a failure needs evidence (failing test names, an error, the rejection) before a step moves up");
        }
        Ok(Failure { kind, evidence })
    }
}

/// The next rung up after a measured failure; nothing is above the frontier.
pub fn escalate(from: ModelClass, _why: &Failure) -> Option<ModelClass> {
    match from {
        ModelClass::Local => Some(ModelClass::Mid),
        ModelClass::Mid => Some(ModelClass::Frontier),
        ModelClass::Frontier => None,
    }
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
        (self.successes as f64 + 1.0) / (self.attempts as f64 + 2.0)
    }

    /// Expected spend to get one success by retrying this rung.
    pub fn expected_cost_to_green(&self) -> f64 {
        self.mean_cost_usd / self.p_success()
    }
}

/// True when retrying the cheap rung is expected to cost more than going to
/// the frontier now (PLAN §3.4: "if the expected cost of another cheap retry
/// exceeds a frontier call, escalate sooner"). Both sides are the expected
/// spend to get one success, so a cheap rung that keeps failing loses.
pub fn escalate_sooner(cheap: &RungStats, frontier: &RungStats) -> bool {
    cheap.expected_cost_to_green() > frontier.expected_cost_to_green()
}

/// Keep the candidates at or below `ceiling`; when there are none, keep them
/// all, so a project with only strong models still routes.
pub fn within<'a, M: Rung>(candidates: &[&'a M], ceiling: ModelClass) -> Vec<&'a M> {
    let kept: Vec<_> = candidates.iter().copied().filter(|m| class_of(*m) <= ceiling).collect();
    if kept.is_empty() {
        candidates.to_vec()
    } else {
        kept
    }
}

/// The cheapest candidate on exactly `class`; failing that the nearest class
/// below, then above. Used to price a policy, not to route.
pub fn cheapest_on<'a, M: Rung>(candidates: &[&'a M], class: ModelClass) -> Option<&'a M> {
    let cheapest = |c: ModelClass| {
        candidates.iter().copied().filter(|m| class_of(*m) == c).min_by(|a, b| {
            a.output_usd_per_mtok()
                .total_cmp(&b.output_usd_per_mtok())
                .then(a.input_usd_per_mtok().total_cmp(&b.input_usd_per_mtok()))
                .then_with(|| a.id().cmp(b.id()))
        })
    };
    let all = [ModelClass::Local, ModelClass::Mid, ModelClass::Frontier];
    cheapest(class)
        .or_else(|| all.iter().rev().filter(|c| **c < class).find_map(|c| cheapest(*c)))
        .or_else(|| all.iter().filter(|c| **c > class).find_map(|c| cheapest(*c)))
}
