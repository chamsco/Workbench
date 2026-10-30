//! The policy gateway: the one place that decides which model, skill or
//! plugin an agent may use, and whether a model call fits the budget.
//!
//! SKELETON for the M1 acceptance tests: every check below allows
//! everything, so the tests in `tests/gateway.rs` and `tests/gateway_e2e.rs`
//! fail for the right reason. The implementation replaces this body.

use serde::{Deserialize, Serialize};

use crate::config::ModelSpec;
use crate::project::AgentKind;

/// What an agent is doing: planning and deciding, or carrying out bounded work.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Orchestration,
    Execution,
}

impl Role {
    pub fn of(_kind: AgentKind, _plans: bool) -> Role {
        Role::Execution
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::Orchestration => "orchestration",
            Role::Execution => "execution",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct TaskBudget {
    pub max_usd: Option<f64>,
    pub max_tokens: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct PolicyConfig {
    pub orchestration_models: Option<Vec<String>>,
    pub execution_models: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    pub plugins: Option<Vec<String>>,
    pub budget: TaskBudget,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    Model { role: Role, model: String },
    Skill { role: Role, skill: String },
    Plugin { role: Role, plugin: String },
    Budget { label: String, detail: String },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Refusal {}

/// One limit a call must fit under, with what has been spent against it.
#[derive(Clone, Debug)]
pub struct BudgetLine {
    pub label: String,
    pub limit_usd: Option<f64>,
    pub limit_tokens: Option<u64>,
    pub spent_usd: f64,
    pub spent_tokens: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grant {
    pub max_output_tokens: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Gateway {
    policy: PolicyConfig,
}

/// Input tokens for `chars` characters of prompt, rounded up on purpose.
pub fn estimate_tokens(chars: usize) -> u64 {
    chars as u64
}

impl Gateway {
    pub fn new(policy: PolicyConfig) -> Self {
        Gateway { policy }
    }

    pub fn policy(&self) -> &PolicyConfig {
        &self.policy
    }

    pub fn check_model(&self, _role: Role, _model: &str) -> Result<(), Refusal> {
        Ok(())
    }

    pub fn skill_allowed(&self, _skill: &str) -> bool {
        true
    }

    pub fn check_skill(&self, _role: Role, _skill: &str) -> Result<(), Refusal> {
        Ok(())
    }

    pub fn check_plugin(&self, _role: Role, _plugin: &str) -> Result<(), Refusal> {
        Ok(())
    }

    /// The per-task line from the policy, with this task's own spend.
    pub fn task_budget(&self, _spent_usd: f64, _spent_tokens: u64) -> Option<BudgetLine> {
        None
    }

    pub fn authorize_call(
        &self,
        _role: Role,
        model: &ModelSpec,
        _est_input_tokens: u64,
        _budgets: &[BudgetLine],
    ) -> Result<Grant, Refusal> {
        Ok(Grant { max_output_tokens: model.max_output_tokens })
    }
}
