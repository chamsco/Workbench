//! The PLAN §3.4 metrics, from a ledger the harness fills in as it runs.
//!
//! Until PR creation exists (M7) a "merged" unit is a ticket whose work was
//! accepted, so every per-merged figure is per merged ticket and says so in
//! its `unit`. Human tokens are estimated from characters (`ceil(chars / 3)`)
//! and cover what the user typed (messages to the main agent, rejection
//! feedback) and what they were shown (the main agent's replies, and the
//! summaries and diff stats on approval cards). They are a lower bound: files
//! and diffs the user opens themselves are not counted.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::gateway::estimate_tokens;
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
struct TaskLine {
    escalated: bool,
    green: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Ledger {
    calls: u64,
    frontier_calls: u64,
    tokens: u64,
    usd: f64,
    /// Total spend at the moment of the most recent green result.
    usd_at_green: Option<f64>,
    tasks: BTreeMap<String, TaskLine>,
    typed: u64,
    read: u64,
}

impl Ledger {
    pub fn record_call(&mut self, call: CallRecord) {
        self.calls += 1;
        if call.class == ModelClass::Frontier {
            self.frontier_calls += 1;
        }
        self.tokens += call.input_tokens + call.output_tokens;
        self.usd += call.usd;
    }

    /// Spend on a decision model (the router's), which is cost but not a call.
    pub fn record_decision(&mut self, usd: f64) {
        self.usd += usd;
    }

    pub fn task_started(&mut self, key: &str) {
        self.tasks.entry(key.to_string()).or_default();
    }

    pub fn task_escalated(&mut self, key: &str) {
        self.tasks.entry(key.to_string()).or_default().escalated = true;
    }

    /// The task finished; `green` when its result was accepted.
    pub fn task_finished(&mut self, key: &str, green: bool) {
        self.tasks.entry(key.to_string()).or_default().green = green;
        if green {
            self.usd_at_green = Some(self.usd);
        }
    }

    pub fn human_typed(&mut self, chars: usize) {
        self.typed += estimate_tokens(chars);
    }

    pub fn human_read(&mut self, chars: usize) {
        self.read += estimate_tokens(chars);
    }

    pub fn metrics(&self) -> RunMetrics {
        let tasks = self.tasks.len() as u64;
        let escalated = self.tasks.values().filter(|t| t.escalated).count() as u64;
        let escalated_green = self.tasks.values().filter(|t| t.escalated && t.green).count() as u64;
        let merged = self.tasks.values().filter(|t| t.green).count() as u64;
        let per = |x: f64| (merged > 0).then(|| x / merged as f64);
        RunMetrics {
            unit: "ticket".into(),
            calls: self.calls,
            frontier_calls: self.frontier_calls,
            total_tokens: self.tokens,
            total_usd: self.usd,
            cost_to_green_usd: self.usd_at_green,
            tasks,
            escalated_tasks: escalated,
            escalated_green,
            escalation_rate: (tasks > 0).then(|| escalated as f64 / tasks as f64),
            escalation_success_rate: (escalated > 0).then(|| escalated_green as f64 / escalated as f64),
            human_tokens_typed: self.typed,
            human_tokens_read: self.read,
            merged,
            frontier_calls_per_merged: per(self.frontier_calls as f64),
            tokens_per_merged: per(self.tokens as f64),
            usd_per_merged: per(self.usd),
            human_tokens_per_merged: per((self.typed + self.read) as f64),
        }
    }
}
