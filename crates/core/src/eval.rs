//! The router eval PLAN §10 requires before a decision model ships: accuracy
//! per task type, calibration, cost against always-frontier, and the
//! escalation rate below the confidence threshold, over labeled traces.

use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::decision::{DecisionModel, TaskType};

/// One labeled task. Token counts are optional; without them cost is not
/// computed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Trace {
    pub id: String,
    pub text: String,
    pub label: TaskType,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
}

/// One JSON object per line. A line without a label, or with a label that is
/// not a task type, is an error naming the line.
pub fn parse_traces(_jsonl: &str) -> Result<Vec<Trace>> {
    unimplemented!("M2")
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TypeStats {
    pub n: usize,
    pub correct: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bin {
    pub lo: f64,
    pub hi: f64,
    pub n: usize,
    pub mean_confidence: f64,
    pub accuracy: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CostCompare {
    pub priced: usize,
    pub routed_usd: f64,
    pub always_frontier_usd: f64,
}

#[derive(Clone, Debug)]
pub struct EvalReport {
    pub decider: String,
    pub threshold: f64,
    pub n: usize,
    pub correct: usize,
    pub per_type: BTreeMap<TaskType, TypeStats>,
    pub bins: Vec<Bin>,
    pub ece: f64,
    pub brier: f64,
    pub below_threshold: usize,
    pub wrong_below_threshold: usize,
    pub confident_wrong: usize,
    pub cost: Option<CostCompare>,
}

pub fn evaluate(_model: &dyn DecisionModel, _traces: &[Trace], _threshold: f64, _cfg: &Config) -> Result<EvalReport> {
    unimplemented!("M2")
}

impl EvalReport {
    pub fn to_markdown(&self) -> String {
        unimplemented!("M2")
    }
}
