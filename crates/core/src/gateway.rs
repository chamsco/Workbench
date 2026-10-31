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
//! exactly, with no trimming, case folding, prefix or glob. `mode` says how
//! the section is read: `enforced` needs every list, `legacy_open` says that
//! open lists are intended, and no mode warns about each open list. A project
//! with no config file starts `enforced`, with every list written out.
//!
//! The policy a project runs under is the one it adopted, kept outside the
//! workspace (see `resolve_policy`); a `backspace.toml` that loosens it is
//! ignored with a warning until a person adopts it.
//!
//! Budgets: a call is authorized only if its worst case fits under every
//! budget that applies, counting the worst case of every call already in
//! flight (`Reservations`). The prompt is priced from an upper bound on its
//! tokens (`input_bound`) and the output limit sent to the provider is clamped
//! to what is left. Actual usage replaces the reservation when the call ends;
//! if it was higher than the reservation, that is recorded (`Overshoot`).
//!
//! What this does not do is in docs/cauldron/M1-BUDGET-BOUNDARIES.md: the
//! input bound rests on one assumption (a token covers at least one input
//! byte), the advisor's spend is reserved at an assumed cap the provider does
//! not enforce, and some paths are `Unmetered` on purpose and listed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context};
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

/// How a `[policy]` section is read. Unset keeps the M1 rules and warns about
/// every dimension left open.
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

fn warn(text: String) -> PolicyNotice {
    PolicyNotice { level: NoticeLevel::Warning, text }
}

fn note(text: String) -> PolicyNotice {
    PolicyNotice { level: NoticeLevel::Note, text }
}

fn quoted(names: &[&str]) -> String {
    names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")
}

/// Names in `new` that are not in `old`, as a list for a message.
fn gained(old: &[String], new: &[String]) -> Vec<String> {
    new.iter().filter(|n| !old.contains(n)).map(|n| format!("`{n}`")).collect()
}

impl PolicyConfig {
    fn lists(&self) -> [(&'static str, &Option<Vec<String>>); 4] {
        [
            ("orchestration_models", &self.orchestration_models),
            ("execution_models", &self.execution_models),
            ("skills", &self.skills),
            ("plugins", &self.plugins),
        ]
    }

    /// The dimensions this policy leaves open (no list).
    pub fn open_lists(&self) -> Vec<&'static str> {
        self.lists().into_iter().filter(|(_, l)| l.is_none()).map(|(n, _)| n).collect()
    }

    /// What to tell the user when this policy is loaded.
    pub fn notices(&self) -> Vec<PolicyNotice> {
        let open = self.open_lists();
        match self.mode {
            Some(PolicyMode::Enforced) => vec![],
            Some(PolicyMode::LegacyOpen) if open.is_empty() => {
                vec![note("policy mode is legacy_open, and every list is written out, so nothing is open".into())]
            }
            Some(PolicyMode::LegacyOpen) => vec![note(format!(
                "policy mode is legacy_open: {} {} left open on purpose",
                quoted(&open),
                if open.len() == 1 { "is" } else { "are" }
            ))],
            None if *self == PolicyConfig::default() => vec![warn(
                "no [policy] section in backspace.toml: every model, skill and plugin is allowed for every agent (orchestration_models, execution_models, skills and plugins are all open). Write the lists and set `mode = \"enforced\"` (`backspace-cli policy init` prints them), or set `mode = \"legacy_open\"` under [policy] to say this is intended.".into(),
            )],
            None if !open.is_empty() => vec![warn(format!(
                "[policy] leaves {} open: anything is allowed there. List what should be allowed and set `mode = \"enforced\"` (`backspace-cli policy init` prints the lists), or set `mode = \"legacy_open\"` to say this is intended.",
                quoted(&open)
            ))],
            None => vec![],
        }
    }

    /// Notices for skills that exist but this policy does not allow.
    pub fn skill_notices(&self, present: &[String]) -> Vec<PolicyNotice> {
        let Some(allowed) = &self.skills else { return vec![] };
        let denied: Vec<&str> = present.iter().filter(|s| !allowed.contains(s)).map(String::as_str).collect();
        if denied.is_empty() {
            return vec![];
        }
        vec![warn(format!(
            "skills {} exist but are not allowed by [policy].skills, so agents are not offered them. Add a name to the list to allow it.",
            quoted(&denied)
        ))]
    }

    /// `enforced` needs every list; the error names the ones that are missing.
    pub fn check_complete(&self) -> Result<(), String> {
        if self.mode != Some(PolicyMode::Enforced) {
            return Ok(());
        }
        let missing = self.open_lists();
        if missing.is_empty() {
            return Ok(());
        }
        Err(format!(
            "[policy] mode = \"enforced\" needs every list; missing {}. An empty list `[]` allows nothing.",
            quoted(&missing)
        ))
    }

    /// What `new` lets through that this policy (the one adopted) does not,
    /// one line per difference. Empty when `new` is at least as strict
    /// everywhere. A list is judged by its members, so a reordered list is
    /// not a loosening.
    pub fn loosened_by(&self, new: &PolicyConfig) -> Vec<String> {
        let mut out = vec![];
        for ((name, old), (_, new)) in self.lists().into_iter().zip(new.lists()) {
            match (old, new) {
                (Some(_), None) => out.push(format!("{name} is no longer a list: everything would be allowed")),
                (Some(old), Some(new)) => {
                    let more = gained(old, new);
                    if !more.is_empty() {
                        out.push(format!("{name} gains {}", more.join(", ")));
                    }
                }
                _ => {}
            }
        }
        let (a, b) = (&self.budget, &new.budget);
        match (a.max_usd, b.max_usd) {
            (Some(x), None) => out.push(format!("max_usd (${x}) is removed")),
            (Some(x), Some(y)) if y > x => out.push(format!("max_usd goes from ${x} to ${y}")),
            _ => {}
        }
        match (a.max_tokens, b.max_tokens) {
            (Some(x), None) => out.push(format!("max_tokens ({x}) is removed")),
            (Some(x), Some(y)) if y > x => out.push(format!("max_tokens goes from {x} to {y}")),
            _ => {}
        }
        if self.mode == Some(PolicyMode::Enforced) && new.mode != Some(PolicyMode::Enforced) {
            out.push(format!(
                "mode `enforced` is {}",
                if new.mode.is_some() { "changed to `legacy_open`" } else { "removed" }
            ));
        }
        out
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

/// The words that mark a refusal as one that goes away when a running call ends.
const HELD: &str = "held by a running turn";

impl Refusal {
    /// Refused only because a call still in flight holds the room: asking
    /// again once it has finished may be a different answer.
    pub fn is_held(&self) -> bool {
        matches!(self, Refusal::Budget { detail, .. } if detail.contains(HELD))
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
/// English runs near 4 characters a token, code and JSON nearer 3. This is
/// for the metrics' estimate of what a person typed or read; a model call is
/// bounded by `input_bound`, which reads bytes, not characters.
pub fn estimate_tokens(chars: usize) -> u64 {
    (chars as u64).div_ceil(3)
}

/// What a reservation assumes the advisor may write in one consultation (the
/// default of `advisor.max_output_tokens`). The advisor tool takes no output
/// cap, so this is an assumption about the provider's behaviour (its replies
/// are documented as a few hundred tokens, more with thinking), not something
/// a request enforces.
pub const ADVISOR_OUTPUT_TOKENS: u64 = 4_096;

/// What a reservation for a Jev routing call assumes its answer may take (two
/// short answers and a usage block). Jev takes no output cap either, so like
/// the advisor's this is an assumption about the provider, not a limit.
pub const ROUTER_OUTPUT_TOKENS: u64 = 512;

/// What the provider adds around a request's own text that the body does not
/// show: the tool-use system prompt Anthropic documents at up to 804 tokens
/// (Opus 4.7, `any`/`tool`), plus request framing. Rounded up to 1024.
pub const FIXED_INPUT_TOKENS: u64 = 1024;
/// Role and separator tokens per message.
pub const PER_MESSAGE_TOKENS: u64 = 8;
/// The wrapper a provider renders each tool definition into.
pub const PER_TOOL_TOKENS: u64 = 16;

/// An upper bound on the input tokens of a request, from its serialized size.
///
/// One assumption carries it: a token covers at least one byte of the UTF-8
/// body. Byte-level tokenizers satisfy it by construction, and no text
/// tokenizer is known to emit more than one token per byte of plain text. It
/// is not checked against a real tokenizer here (none is available); text
/// blocks only, since a request with images is not built. Cache reads and
/// writes are inputs too, priced separately (`provider::input_price_factor`).
pub fn input_bound(body_bytes: usize, messages: usize, tools: usize) -> u64 {
    body_bytes as u64 + FIXED_INPUT_TOKENS + messages as u64 * PER_MESSAGE_TOKENS + tools as u64 * PER_TOOL_TOKENS
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

type Holds = HashMap<String, Vec<(u64, f64, u64)>>;

/// Worst-case amounts held against each scope while calls are in flight.
/// Checking a call against what is left and holding its worst case happen
/// under one lock, so two agents cannot both be told the same dollar is free.
#[derive(Debug, Default)]
pub struct Reservations {
    held: Mutex<Holds>,
    next: Mutex<u64>,
    /// Woken whenever a reservation is dropped, for a turn waiting on a hold.
    released: tokio::sync::Notify,
}

fn held_in(h: &Holds, key: &str) -> (f64, u64) {
    h.get(key).map_or((0.0, 0), |v| v.iter().fold((0.0, 0), |(u, t), (_, du, dt)| (u + du, t + dt)))
}

/// A call's hold on the budget; released when dropped. Reserved for the
/// worst case: the whole prompt bound plus the whole granted output.
#[derive(Debug)]
pub struct Reservation {
    table: Option<Arc<Reservations>>,
    id: u64,
    keys: Vec<String>,
    pub grant: Grant,
    usd: f64,
    tokens: u64,
    input_bound: u64,
    /// For a CLI turn: tokens it may report before it must be stopped.
    token_ceiling: Option<u64>,
}

impl Reservation {
    /// Take this call's hold out of the table. `held` is the table's own lock,
    /// so a caller that is also recording the call's cost does both under it.
    fn release_in(&mut self, held: &mut Holds) {
        for k in &self.keys {
            if let Some(v) = held.get_mut(k) {
                v.retain(|(id, _, _)| *id != self.id);
                if v.is_empty() {
                    held.remove(k);
                }
            }
        }
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let Some(t) = self.table.take() else { return };
        let mut held = t.held.lock().unwrap();
        self.release_in(&mut held);
        drop(held);
        t.released.notify_waiters();
    }
}

/// `line` with what other calls hold against its scope counted as spent.
fn with_holds(h: &Holds, s: &ScopedLine) -> BudgetLine {
    let (u, t) = held_in(h, &s.key);
    BudgetLine { spent_usd: s.line.spent_usd + u, spent_tokens: s.line.spent_tokens + t, ..s.line.clone() }
}

impl Reservations {
    fn id(&self) -> u64 {
        let mut n = self.next.lock().unwrap();
        *n += 1;
        *n
    }

    #[allow(clippy::too_many_arguments)]
    fn hold(self: &Arc<Self>, held: &mut Holds, lines: &[ScopedLine], usd: f64, tokens: u64, grant: Grant, input_bound: u64, token_ceiling: Option<u64>) -> Reservation {
        let id = self.id();
        let keys: Vec<String> = lines.iter().map(|l| l.key.clone()).collect();
        for k in &keys {
            held.entry(k.clone()).or_default().push((id, usd, tokens));
        }
        Reservation { table: Some(self.clone()), id, keys, grant, usd, tokens, input_bound, token_ceiling }
    }

    /// Check the call against every scope, counting what other in-flight
    /// calls have already reserved, and reserve its worst case, all under one
    /// lock. `lines` reads the spent figures; it runs under that lock.
    pub fn reserve<F: FnOnce() -> Vec<ScopedLine>>(
        self: &Arc<Self>,
        gateway: &Gateway,
        role: Role,
        model: &ModelSpec,
        input_bound: u64,
        input_price_factor: f64,
        lines: F,
    ) -> Result<Reservation, Refusal> {
        let mut held = self.held.lock().unwrap();
        let lines = lines();
        let adjusted: Vec<BudgetLine> = lines.iter().map(|l| with_holds(&held, l)).collect();
        let grant = gateway.authorize_call_priced(role, model, input_bound, input_price_factor, &adjusted)?;
        let out = grant.max_output_tokens as f64;
        let usd = (input_bound as f64 * model.input_usd_per_mtok * input_price_factor + out * model.output_usd_per_mtok) / 1e6;
        Ok(self.hold(&mut held, &lines, usd, input_bound + grant.max_output_tokens as u64, grant, input_bound, None))
    }

    /// Reserve a fixed amount (the advisor's) against the same scopes. It
    /// fits only if every scope has that much left after what is held.
    pub fn reserve_fixed<F: FnOnce() -> Vec<ScopedLine>>(
        self: &Arc<Self>,
        what: &str,
        usd: f64,
        tokens: u64,
        lines: F,
    ) -> Result<Reservation, Refusal> {
        let mut held = self.held.lock().unwrap();
        let lines = lines();
        for l in lines.iter().map(|l| with_holds(&held, l)) {
            let refuse = |detail: String| Refusal::Budget { label: l.label.clone(), detail };
            if let Some(limit) = l.limit_usd {
                if l.spent_usd + usd > limit {
                    return Err(refuse(format!("{what} could cost up to ${usd:.4}, and only ${:.4} of ${limit:.4} is left", (limit - l.spent_usd).max(0.0))));
                }
            }
            if let Some(limit) = l.limit_tokens {
                if l.spent_tokens + tokens > limit {
                    return Err(refuse(format!("{what} could use up to {tokens} tokens, and only {} of {limit} are left", limit.saturating_sub(l.spent_tokens))));
                }
            }
        }
        Ok(self.hold(&mut held, &lines, usd, tokens, Grant { max_output_tokens: 0 }, 0, None))
    }

    /// Admit a turn in a coding CLI. A CLI is billed to a plan, so a turn adds
    /// no dollars to any scope whatever price is configured for its model, and
    /// it reports its usage only as it goes (Codex) or once at the end (Claude
    /// Code), so what it will use is not known when it starts. It is therefore:
    ///
    /// - refused when a scope it counts toward is already at its limit, or
    ///   when its tokens are held by a turn that is still running (`is_held`:
    ///   wait for `released` and ask again);
    /// - refused outright when a dollar limit has no token limit beside it,
    ///   because nothing would bind it;
    /// - otherwise given everything the tightest token limit has left, **and
    ///   that is held against every scope it counts toward until it ends**,
    ///   so two turns are never given the same tokens. Turns on one shared
    ///   token budget therefore run one at a time. The only overshoot is the
    ///   one turn's own, between its reports.
    pub fn admit_cli<F: FnOnce() -> Vec<ScopedLine>>(self: &Arc<Self>, model: &ModelSpec, lines: F) -> Result<Reservation, Refusal> {
        let mut held = self.held.lock().unwrap();
        let lines = lines();
        let mut ceiling: Option<u64> = None;
        for l in &lines {
            let with = with_holds(&held, l);
            let refuse = |detail: String| Refusal::Budget { label: l.line.label.clone(), detail };
            if l.line.limit_usd.is_some() && l.line.limit_tokens.is_none() {
                return Err(refuse(format!(
                    "a dollar limit cannot bind `{}`: a coding CLI is billed to its plan and adds no dollars to this budget, whatever price is configured for it. Set a token limit beside it, or remove the dollar limit for this project",
                    model.id
                )));
            }
            if let Some(limit) = l.line.limit_usd {
                if with.spent_usd >= limit {
                    return Err(refuse(format!("${:.4} of ${limit:.4} already spent", with.spent_usd)));
                }
            }
            if let Some(limit) = l.line.limit_tokens {
                let left = limit.saturating_sub(with.spent_tokens);
                if left == 0 {
                    let by_others = held_in(&held, &l.key).1;
                    return Err(refuse(if l.line.spent_tokens < limit && by_others > 0 {
                        format!("{by_others} of {limit} tokens are {HELD}: {} used, the rest reserved for it", l.line.spent_tokens)
                    } else {
                        format!("{} of {limit} tokens already used", l.line.spent_tokens)
                    }));
                }
                ceiling = Some(ceiling.map_or(left, |c| c.min(left)));
            }
        }
        Ok(self.hold(&mut held, &lines, 0.0, ceiling.unwrap_or(0), Grant { max_output_tokens: 0 }, 0, ceiling))
    }

    /// Woken each time a reservation is released.
    pub fn released(&self) -> &tokio::sync::Notify {
        &self.released
    }

    /// What is reserved against a scope right now.
    pub fn held(&self, key: &str) -> (f64, u64) {
        held_in(&self.held.lock().unwrap(), key)
    }
}

impl Reservation {
    /// Record what the call really cost and give its hold back in one step.
    ///
    /// `record` runs while the table's lock is held and the hold is still
    /// there; the hold is removed under the same lock hold. Every other
    /// reservation takes that lock to read what is held and (through its
    /// `lines`) what is spent, so none of them can see the hold gone and the
    /// spend not yet recorded, or a cost above the estimate not yet on the
    /// books. `record` must not ask this table for anything (it would wait for
    /// the lock it is under); it may take the locks `lines` takes, which are
    /// taken in that order everywhere.
    pub fn settle<R>(mut self, record: impl FnOnce() -> R) -> R {
        let Some(t) = self.table.take() else { return record() };
        let mut held = t.held.lock().unwrap();
        let out = record();
        self.release_in(&mut held);
        drop(held);
        t.released.notify_waiters();
        out
    }

    pub fn reserved_usd(&self) -> f64 {
        self.usd
    }

    pub fn reserved_tokens(&self) -> u64 {
        self.tokens
    }

    /// The input bound this reservation was made for; a request that bounds
    /// higher than this was not reserved for.
    pub fn input_bound(&self) -> u64 {
        self.input_bound
    }

    /// For a CLI turn: how many tokens it may report before it is stopped.
    pub fn token_ceiling(&self) -> Option<u64> {
        self.token_ceiling
    }

    /// How far the real cost of the call went past its reservation.
    pub fn overshoot(&self, actual_usd: f64, actual_tokens: u64) -> Option<Overshoot> {
        let usd = actual_usd - self.usd;
        let tokens = actual_tokens.saturating_sub(self.tokens);
        (usd > 1e-9 || tokens > 0).then(|| Overshoot { usd: usd.max(0.0), tokens })
    }
}

/// Why a model call is not metered by a reservation. A closed list: a new
/// reason is a code change, a line in `tests/unmetered.rs` and a paragraph in
/// docs/cauldron/M1-BUDGET-BOUNDARIES.md.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unmetered {
    /// The development Backspace Cloud server answers chat with the models in
    /// its own config; it has no task and no project of its own.
    CloudChatServer,
    /// Chat with an agent, a bot or a routine: not a task, so no task budget.
    AgentChat,
    /// The Jev router's own classification call, counted as router cost afterwards.
    JevRouter,
}

impl Unmetered {
    pub fn why(self) -> &'static str {
        match self {
            Unmetered::CloudChatServer => "cloud chat server",
            Unmetered::AgentChat => "agent chat",
            Unmetered::JevRouter => "Jev router",
        }
    }
}

/// How a model call is accounted for. Every call that leaves this process
/// carries one.
#[derive(Clone, Copy, Debug)]
pub enum Authorization<'a> {
    Reserved(&'a Reservation),
    Unmetered(Unmetered),
}

impl Authorization<'_> {
    /// Refuse a request that outgrew what was reserved for it.
    pub fn check_request(&self, bound: u64) -> anyhow::Result<()> {
        match self {
            Authorization::Reserved(r) if r.input_bound > 0 && bound > r.input_bound => Err(anyhow!(
                "the request grew after it was reserved ({bound} tokens at most, reserved for {})",
                r.input_bound
            )),
            _ => Ok(()),
        }
    }
}

// ------------------------------------------------------------- the lock

/// Where the adopted policy for a workspace is kept: outside the workspace,
/// in the app's data folder.
pub fn lock_path(workspace: &Path) -> PathBuf {
    let name: String = workspace.display().to_string().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    // A long path is cut, with a hash of the whole so two long paths differ.
    let name = if name.len() > 120 {
        let h = name.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
        format!("{}_{h:016x}", &name[name.len() - 100..])
    } else {
        name
    };
    crate::prefs::Prefs::data_dir().join("policy-locks").join(format!("{name}.toml"))
}

#[derive(Serialize, Deserialize)]
struct LockFile {
    workspace: String,
    #[serde(default)]
    policy: PolicyConfig,
}

fn write_lock(workspace: &Path, policy: &PolicyConfig) -> anyhow::Result<()> {
    let path = lock_path(workspace);
    std::fs::create_dir_all(path.parent().expect("a lock path has a folder"))?;
    let body = toml::to_string(&LockFile { workspace: workspace.display().to_string(), policy: policy.clone() })?;
    let text = format!(
        "# The policy this project runs under. A backspace.toml that loosens it is ignored\n# until a person runs `backspace-cli policy adopt`. Not for agents or hand editing.\n{body}"
    );
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path).with_context(|| format!("writing {}", path.display()))
}

/// Settle which policy a project runs under: the one it adopted before
/// (kept outside the workspace, `lock_path`) unless the file in the workspace
/// is at least as strict everywhere. A first open adopts the file as it is; a
/// stricter file is adopted; a file that loosens anything is ignored whole,
/// with a warning that says what and how to accept it. A lock that cannot be
/// read stops the open, because falling back to the file would be a way
/// around it.
pub fn resolve_policy(workspace: &Path, file: &PolicyConfig) -> anyhow::Result<(PolicyConfig, Vec<PolicyNotice>)> {
    let path = lock_path(workspace);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            write_lock(workspace, file)?;
            return Ok((file.clone(), vec![]));
        }
        Err(e) => return Err(anyhow!("cannot read the policy lock {}: {e}", path.display())),
    };
    let locked = toml::from_str::<LockFile>(&text)
        .map_err(|e| anyhow!("the policy lock {} cannot be read ({e}). Remove it to adopt backspace.toml as it stands, or restore it", path.display()))?
        .policy;
    let loosened = locked.loosened_by(file);
    if loosened.is_empty() {
        if *file != locked {
            write_lock(workspace, file)?;
        }
        return Ok((file.clone(), vec![]));
    }
    Ok((
        locked,
        vec![warn(format!(
            "backspace.toml loosens the policy this project adopted, so it is ignored: {}. Run `backspace-cli policy adopt` to accept it.",
            loosened.join("; ")
        ))],
    ))
}

/// A person's step: make this policy the adopted one. Returns what it loosens
/// compared with the one adopted before, for them to read.
pub fn adopt_policy(workspace: &Path, file: &PolicyConfig) -> anyhow::Result<Vec<String>> {
    let path = lock_path(workspace);
    let before = std::fs::read_to_string(&path).ok().and_then(|t| toml::from_str::<LockFile>(&t).ok()).map(|l| l.policy);
    write_lock(workspace, file)?;
    Ok(before.map(|b| b.loosened_by(file)).unwrap_or_default())
}

// ----------------------------------------------------------- the gateway

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
    /// This looks at the figures it is given; `Reservations::reserve` is what
    /// makes the answer hold while other calls are in flight.
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
        input_price_factor: f64,
        budgets: &[BudgetLine],
    ) -> Result<Grant, Refusal> {
        let refuse = |b: &BudgetLine, detail: String| Refusal::Budget { label: b.label.clone(), detail };
        let mut out = model.max_output_tokens as u64;
        for b in budgets {
            if let Some(limit) = b.limit_usd {
                let (pin, pout) = (model.input_usd_per_mtok * input_price_factor, model.output_usd_per_mtok);
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
