//! The policy gateway: the one place that decides which model, skill or
//! plugin an agent may use, and whether a model call fits the budget.
//!
//! Everything here is decided from typed values the project's policy file
//! produced when the project opened: a model id, a skill name, a plugin name,
//! a role, a price and a count. No prompt text, tool output, memory note or
//! file an agent can write is ever read, and the gateway has no way to change
//! its own policy once built. A disallowed thing is refused in code, before
//! anything is sent.
//!
//! Lists: a list that is not in the policy leaves that dimension open; a list
//! that is there (even empty) is the whole of what is allowed. Names match
//! exactly, with no trimming, case folding, prefix or glob.
//!
//! Budgets: a call is authorized only if its worst case fits under every
//! budget that applies. The prompt is priced from a high token estimate and
//! the output limit sent to the provider is clamped to what is left, so the
//! worst case cannot overshoot by more than the estimate's error. Actual
//! usage is recorded by the caller and counts against the next call.

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
    /// The main agent, and any agent that plans tickets for others, decide;
    /// workers, triage and scouts carry out bounded work.
    pub fn of(kind: AgentKind, plans: bool) -> Role {
        if kind == AgentKind::Main || plans {
            Role::Orchestration
        } else {
            Role::Execution
        }
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
        match self {
            Refusal::Model { role, model } => write!(
                f,
                "refused by project policy: model `{model}` is not allowed for {} agents",
                role.label()
            ),
            Refusal::Skill { role, skill } => write!(
                f,
                "refused by project policy: skill `{skill}` is not allowed for {} agents",
                role.label()
            ),
            Refusal::Plugin { role, plugin } => write!(
                f,
                "refused by project policy: plugin `{plugin}` is not allowed for {} agents",
                role.label()
            ),
            Refusal::Budget { label, detail } => {
                write!(f, "refused by project policy: {label} budget: {detail}")
            }
        }
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

/// Input tokens for `chars` characters of prompt, rounded up on purpose:
/// English runs near 4 characters a token, code and JSON nearer 3, so 3 keeps
/// the estimate above the real count for the text agents send.
pub fn estimate_tokens(chars: usize) -> u64 {
    (chars as u64).div_ceil(3)
}

impl Gateway {
    pub fn new(policy: PolicyConfig) -> Self {
        Gateway { policy }
    }

    pub fn policy(&self) -> &PolicyConfig {
        &self.policy
    }

    pub fn check_model(&self, role: Role, model: &str) -> Result<(), Refusal> {
        let list = match role {
            Role::Orchestration => &self.policy.orchestration_models,
            Role::Execution => &self.policy.execution_models,
        };
        match list {
            Some(l) if !l.iter().any(|m| m == model) => {
                Err(Refusal::Model { role, model: model.to_string() })
            }
            _ => Ok(()),
        }
    }

    /// For filtering what is offered (the skills index, injected skills).
    pub fn skill_allowed(&self, skill: &str) -> bool {
        self.policy.skills.as_ref().is_none_or(|l| l.iter().any(|s| s == skill))
    }

    pub fn check_skill(&self, role: Role, skill: &str) -> Result<(), Refusal> {
        if self.skill_allowed(skill) {
            Ok(())
        } else {
            Err(Refusal::Skill { role, skill: skill.to_string() })
        }
    }

    pub fn check_plugin(&self, role: Role, plugin: &str) -> Result<(), Refusal> {
        match &self.policy.plugins {
            Some(l) if !l.iter().any(|p| p == plugin) => {
                Err(Refusal::Plugin { role, plugin: plugin.to_string() })
            }
            _ => Ok(()),
        }
    }

    /// The per-task line from the policy, with this task's own spend; None
    /// when the policy sets no per-task limit.
    pub fn task_budget(&self, spent_usd: f64, spent_tokens: u64) -> Option<BudgetLine> {
        let b = &self.policy.budget;
        (b.max_usd.is_some() || b.max_tokens.is_some()).then(|| BudgetLine {
            label: "task".into(),
            limit_usd: b.max_usd,
            limit_tokens: b.max_tokens,
            spent_usd,
            spent_tokens,
        })
    }

    /// Decide whether a model call may be made. It is refused unless its worst
    /// case (the estimated prompt plus the largest output it may produce)
    /// fits under every budget; the grant says how many output tokens it may
    /// ask for, so the provider is never allowed to go past what is left.
    pub fn authorize_call(
        &self,
        _role: Role,
        model: &ModelSpec,
        est_input_tokens: u64,
        budgets: &[BudgetLine],
    ) -> Result<Grant, Refusal> {
        let refuse = |b: &BudgetLine, detail: String| Refusal::Budget { label: b.label.clone(), detail };
        let mut out = model.max_output_tokens as u64;
        for b in budgets {
            if let Some(limit) = b.limit_usd {
                let (pin, pout) = (model.input_usd_per_mtok, model.output_usd_per_mtok);
                // A model that costs nothing has nothing to spend against a dollar limit.
                if pin > 0.0 || pout > 0.0 {
                    let remaining = limit - b.spent_usd;
                    if remaining <= 0.0 {
                        return Err(refuse(b, format!("${:.4} of ${limit:.4} already spent", b.spent_usd)));
                    }
                    let input_cost = est_input_tokens as f64 * pin / 1e6;
                    let left = remaining - input_cost;
                    if left < 0.0 {
                        return Err(refuse(
                            b,
                            format!("the prompt alone (about ${input_cost:.4}) is more than the ${remaining:.4} left"),
                        ));
                    }
                    if pout > 0.0 {
                        let affordable = (left * 1e6 / pout).floor();
                        if affordable < 1.0 {
                            return Err(refuse(
                                b,
                                format!("${left:.4} is left after the prompt, not enough for any output"),
                            ));
                        }
                        out = out.min(affordable as u64);
                    }
                }
            }
            if let Some(limit) = b.limit_tokens {
                let remaining = limit.saturating_sub(b.spent_tokens);
                if remaining <= est_input_tokens {
                    return Err(refuse(
                        b,
                        format!("{remaining} of {limit} tokens left, and the prompt alone is about {est_input_tokens}"),
                    ));
                }
                out = out.min(remaining - est_input_tokens);
            }
        }
        if out == 0 {
            return Err(Refusal::Budget { label: "model".into(), detail: "the model allows no output".into() });
        }
        Ok(Grant { max_output_tokens: out.min(u32::MAX as u64) as u32 })
    }
}
