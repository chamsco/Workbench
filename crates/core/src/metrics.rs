//! The PLAN §3.4 metrics, from a ledger the harness fills in as it runs.
//!
//! Until PR creation exists (M7) a "merged" unit is a ticket whose work was
//! accepted, so every per-merged figure is per merged ticket and says so in
//! its `unit`. Human tokens are estimated from characters (`ceil(chars / 3)`)
//! and cover what the user typed to the main agent and what the main agent
//! replied; they are a lower bound on what the user read.

use serde::{Deserialize, Serialize};

use crate::ladder::ModelClass;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CallRecord {
    pub model: String,
    pub class: ModelClass,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub usd: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RunMetrics {
    pub unit: String,
    pub calls: u64,
    pub frontier_calls: u64,
    pub total_tokens: u64,
    pub total_usd: f64,
    /// Everything spent, retries and escalations included, up to the last
    /// green result; None until something is green.
    pub cost_to_green_usd: Option<f64>,
    pub tasks: u64,
    pub escalated_tasks: u64,
    pub escalated_green: u64,
    pub escalation_rate: Option<f64>,
    pub escalation_success_rate: Option<f64>,
    pub human_tokens_typed: u64,
    pub human_tokens_read: u64,
    pub merged: u64,
    pub frontier_calls_per_merged: Option<f64>,
    pub tokens_per_merged: Option<f64>,
    pub usd_per_merged: Option<f64>,
    pub human_tokens_per_merged: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct Ledger {}

impl Ledger {
    pub fn record_call(&mut self, _call: CallRecord) {
        unimplemented!("M2")
    }

    pub fn task_started(&mut self, _key: &str) {
        unimplemented!("M2")
    }

    pub fn task_escalated(&mut self, _key: &str) {
        unimplemented!("M2")
    }

    /// The task finished; `green` when its result was accepted.
    pub fn task_finished(&mut self, _key: &str, _green: bool) {
        unimplemented!("M2")
    }

    pub fn human_typed(&mut self, _chars: usize) {
        unimplemented!("M2")
    }

    pub fn human_read(&mut self, _chars: usize) {
        unimplemented!("M2")
    }

    pub fn metrics(&self) -> RunMetrics {
        unimplemented!("M2")
    }
}
