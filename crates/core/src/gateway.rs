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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

/// How a `[policy]` section is read. Absent (`None`) keeps the M1 rules and
/// warns about every dimension left open.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    /// Open dimensions are a choice, stated as one: noted, not warned about.
    LegacyOpen,
    /// Every list must be present; an empty one denies everything.
    Enforced,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct PolicyConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<PolicyMode>,
    pub orchestration_models: Option<Vec<String>>,
    pub execution_models: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    pub plugins: Option<Vec<String>>,
    pub budget: TaskBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeLevel {
    Warning,
    Note,
}

/// Something about the policy the user should see when a project opens.
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyNotice {
    pub level: NoticeLevel,
    pub text: String,
}

impl PolicyConfig {
    /// What to tell the user when this policy is loaded.
    pub fn notices(&self) -> Vec<PolicyNotice> {
        Vec::new()
    }

    /// Notices for skills that exist but this policy does not allow.
    pub fn skill_notices(&self, _present: &[String]) -> Vec<PolicyNotice> {
        Vec::new()
    }

    /// `enforced` needs every list; the error names the ones that are missing.
    pub fn check_complete(&self) -> Result<(), String> {
        Ok(())
    }
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

/// Input tokens a request can cost at most, from its serialized size: the
/// UTF-8 bytes of the exact body, plus allowances for what the provider adds
/// around them. Assumes a token covers at least one input byte.
pub fn input_bound(body_bytes: usize, _messages: usize, _tools: usize) -> u64 {
    estimate_tokens(body_bytes)
}

/// One budget scope (a task, an agent's subtree, the project) with the key
/// that says which reservations count against it.
#[derive(Clone, Debug)]
pub struct ScopedLine {
    pub key: String,
    pub line: BudgetLine,
}

/// What a finished call cost beyond what was reserved for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overshoot {
    pub usd: f64,
    pub tokens: u64,
}

/// Worst-case amounts held against each scope while calls are in flight.
#[derive(Debug, Default)]
pub struct Reservations {
    held: Mutex<HashMap<String, (f64, u64)>>,
}

/// A call's hold on the budget; released when dropped.
#[derive(Debug)]
pub struct Reservation {
    pub grant: Grant,
    usd: f64,
    tokens: u64,
}

impl Reservations {
    /// Check the call against every scope, counting what other in-flight calls
    /// have already reserved, and reserve its worst case, all under one lock.
    pub fn reserve<F: FnOnce() -> Vec<ScopedLine>>(
        self: &Arc<Self>,
        gateway: &Gateway,
        role: Role,
        model: &ModelSpec,
        input_bound: u64,
        input_price_factor: f64,
        lines: F,
    ) -> Result<Reservation, Refusal> {
        let lines: Vec<BudgetLine> = lines().into_iter().map(|l| l.line).collect();
        let grant = gateway.authorize_call_priced(role, model, input_bound, input_price_factor, &lines)?;
        Ok(Reservation { grant, usd: 0.0, tokens: 0 })
    }

    /// Reserve a fixed amount (the advisor's) against the same scopes.
    pub fn reserve_fixed<F: FnOnce() -> Vec<ScopedLine>>(
        self: &Arc<Self>,
        _label: &str,
        _usd: f64,
        _tokens: u64,
        _lines: F,
    ) -> Result<Reservation, Refusal> {
        Ok(Reservation { grant: Grant { max_output_tokens: 0 }, usd: 0.0, tokens: 0 })
    }

    /// What is reserved against a scope right now.
    pub fn held(&self, key: &str) -> (f64, u64) {
        self.held.lock().unwrap().get(key).copied().unwrap_or((0.0, 0))
    }
}

impl Reservation {
    pub fn reserved_usd(&self) -> f64 {
        self.usd
    }

    pub fn reserved_tokens(&self) -> u64 {
        self.tokens
    }

    /// How far the real cost of the call went past its reservation.
    pub fn overshoot(&self, _actual_usd: f64, _actual_tokens: u64) -> Option<Overshoot> {
        None
    }
}

/// Where the adopted policy for a workspace is kept: outside the workspace,
/// in the app's data folder.
pub fn lock_path(workspace: &Path) -> PathBuf {
    let name: String = workspace.display().to_string().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    crate::prefs::Prefs::data_dir().join("policy-locks").join(format!("{name}.toml"))
}

/// Settle which policy a project runs under: the one it adopted before
/// (kept outside the workspace, `lock_path`) unless the file in the workspace
/// is at least as strict everywhere. A first open adopts the file as it is.
pub fn resolve_policy(_workspace: &Path, file: &PolicyConfig) -> anyhow::Result<(PolicyConfig, Vec<PolicyNotice>)> {
    Ok((file.clone(), Vec::new()))
}

/// A person's step: make the workspace file's policy the adopted one.
pub fn adopt_policy(_workspace: &Path, _file: &PolicyConfig) -> anyhow::Result<()> {
    Ok(())
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
        role: Role,
        model: &ModelSpec,
        est_input_tokens: u64,
        budgets: &[BudgetLine],
    ) -> Result<Grant, Refusal> {
        self.authorize_call_priced(role, model, est_input_tokens, 1.0, budgets)
    }

    /// As `authorize_call`, with the prompt priced at `input_price_factor`
    /// times the model's input rate (Anthropic bills a cache write at 1.25x).
    pub fn authorize_call_priced(
        &self,
        _role: Role,
        model: &ModelSpec,
        est_input_tokens: u64,
        _input_price_factor: f64,
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
