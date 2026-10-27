//! The admin's rules: which models the org offers, who may use which, and
//! how much.
//!
//! - The **catalog** is every model the org has turned on: hosted (Claude,
//!   ChatGPT) or sovereign (self-hosted on the org's own machines). Each says
//!   where its data goes and what a token of it costs in credits.
//! - **Grants** say which models and how many credits a period. They are set
//!   on the org (the default), a role, a team or one person; the most
//!   specific one set wins, and the org's ceiling caps them all.
//! - **Credits, not raw tokens.** A million tokens of a frontier model and a
//!   million of a small local one are not the same spend, so every model has
//!   a weight and allowances are in weighted tokens. Weight 1.0 makes credits
//!   tokens, if that is what the admin wants.
//! - **Agents bill someone.** An agent working for a person spends that
//!   person's allowance and may use only what they may. A shared agent spends
//!   its own pool, which the admin sets like any grant.
//!
//! Checks happen before a turn. A turn that starts under the limit is
//! allowed to finish, so a period can end slightly over; the next turn is
//! refused.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Where a model runs, and so where a prompt goes.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Residency {
    /// A provider's API outside the org (Anthropic, OpenAI…).
    External,
    /// The org's own servers (vLLM, Ollama on a GPU box): sovereign.
    OrgHosted,
    /// The person's own device.
    OnDevice,
}

/// How a room is protected (Anarchy's trust states).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Trust {
    /// End-to-end encrypted, nothing leaves the devices: on-device models only.
    Sealed,
    /// The org's servers may read it: org-hosted models, and external ones
    /// where the policy allows.
    #[default]
    Company,
}

/// How to reach a model. Keys are never stored here: `key_env` names the
/// environment variable (or secret) that holds one.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Endpoint {
    OpenAi { base: String, key_env: String },
    Ollama { url: String },
    A2a { url: String, #[serde(default)] token_env: Option<String> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Model {
    /// The catalog id ("claude-opus", "gpt", "mistral-onprem").
    pub id: String,
    pub label: String,
    /// The provider's own model name, sent with each request.
    pub model: String,
    pub endpoint: Endpoint,
    pub residency: Residency,
    /// Credits per token. Defaults to 1.
    #[serde(default = "one")]
    pub weight: f64,
}

fn one() -> f64 {
    1.0
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    Day,
    Week,
    Month,
}

impl Period {
    /// The start of the period `now` is in (UTC). Weeks start on Monday;
    /// months are 30 days counted from the epoch, which is simple and close
    /// enough for an allowance (a calendar month would need a date library).
    pub fn start(self, now_ms: u64) -> u64 {
        const DAY: u64 = 86_400_000;
        match self {
            Period::Day => now_ms - now_ms % DAY,
            // 1 Jan 1970 was a Thursday: Monday is 4 days on.
            Period::Week => {
                let shifted = now_ms + 3 * DAY;
                (shifted - shifted % (7 * DAY)).saturating_sub(3 * DAY)
            }
            Period::Month => now_ms - now_ms % (30 * DAY),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Allowance {
    pub credits: u64,
    pub per: Period,
}

/// What one grant gives. A field left `None` falls through to the next,
/// less specific grant.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Grant {
    /// Catalog ids. An empty list grants no model.
    #[serde(default)]
    pub models: Option<Vec<String>>,
    #[serde(default)]
    pub allowance: Option<Allowance>,
    /// May prompts go to models outside the org.
    #[serde(default)]
    pub external: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub teams: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Policy {
    /// Everyone, unless something more specific says otherwise.
    pub default: Grant,
    #[serde(default)]
    pub roles: BTreeMap<String, Grant>,
    #[serde(default)]
    pub teams: BTreeMap<String, Grant>,
    #[serde(default)]
    pub people: BTreeMap<String, Grant>,
    /// Shared agents' own grants, by agent id.
    #[serde(default)]
    pub agents: BTreeMap<String, Grant>,
    /// No allowance goes above this, whatever grants it.
    #[serde(default)]
    pub ceiling: Option<Allowance>,
}

/// What a person (or shared agent) ends up with once grants are merged.
#[derive(Clone, Debug, PartialEq)]
pub struct Effective {
    pub models: Vec<String>,
    pub allowance: Option<Allowance>,
    pub external: bool,
}

impl Policy {
    /// Most specific first: the person, their teams, their role, the default.
    /// Someone in several teams gets the most generous of them: an admin who
    /// puts a developer in Finance too did not mean to cut their budget.
    pub fn effective(&self, p: &Person) -> Effective {
        let teams: Vec<&Grant> = p.teams.iter().filter_map(|t| self.teams.get(t)).collect();
        let team = merge_teams(&teams);
        let chain = [self.people.get(&p.id), team.as_ref(), self.roles.get(&p.role), Some(&self.default)];
        self.resolve(&chain)
    }

    pub fn effective_agent(&self, agent: &str) -> Effective {
        // A shared agent with no grant of its own gets nothing: it has no
        // person to fall back on, and the default is for people.
        let none = Grant { models: Some(vec![]), allowance: None, external: Some(false) };
        self.resolve(&[self.agents.get(agent).or(Some(&none))])
    }

    fn resolve(&self, chain: &[Option<&Grant>]) -> Effective {
        let models = chain.iter().flatten().find_map(|g| g.models.clone()).unwrap_or_default();
        let external = chain.iter().flatten().find_map(|g| g.external).unwrap_or(false);
        let allowance = chain.iter().flatten().find_map(|g| g.allowance);
        let allowance = match (allowance, self.ceiling) {
            (Some(a), Some(c)) => Some(Allowance { credits: a.credits.min(c.credits), per: a.per }),
            (None, c) => c,
            (a, None) => a,
        };
        Effective { models, allowance, external }
    }
}

fn merge_teams(gs: &[&Grant]) -> Option<Grant> {
    if gs.is_empty() {
        return None;
    }
    let models = gs.iter().filter_map(|g| g.models.as_ref()).fold(None::<Vec<String>>, |acc, m| {
        let mut v = acc.unwrap_or_default();
        for id in m {
            if !v.contains(id) {
                v.push(id.clone());
            }
        }
        Some(v)
    });
    let allowance = gs.iter().filter_map(|g| g.allowance).max_by_key(|a| a.credits);
    let external = gs.iter().filter_map(|g| g.external).reduce(|a, b| a || b);
    Some(Grant { models, allowance, external })
}

/// Who a turn is billed to.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "lowercase")]
pub enum Payer {
    Person(String),
    /// A shared agent, out of its own pool.
    Agent(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Spend {
    pub payer: Payer,
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub credits: u64,
    pub at: u64,
    /// The backend did not report usage, so it was estimated from text.
    #[serde(default)]
    pub estimated: bool,
}

#[derive(Debug, PartialEq)]
pub enum Refusal {
    UnknownPerson,
    UnknownModel,
    /// The model is in the catalog but not granted to this payer.
    NotGranted,
    /// The model sends prompts outside the org and this payer may not.
    External,
    /// The room is sealed and the model is not on the device.
    Sealed,
    OverAllowance { spent: u64, credits: u64, per: Period },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::UnknownPerson => write!(f, "this person is not in the workspace"),
            Refusal::UnknownModel => write!(f, "this model is not in the org's catalog"),
            Refusal::NotGranted => write!(f, "your admin has not given you this model"),
            Refusal::External => write!(f, "this model is outside the org and you may not send it company data"),
            Refusal::Sealed => write!(f, "this room is sealed: only models on your device can read it"),
            Refusal::OverAllowance { spent, credits, per } => {
                write!(f, "allowance used: {spent} of {credits} credits this {}", format!("{per:?}").to_lowercase())
            }
        }
    }
}

/// An invite to a workspace, with what the new member gets on joining.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Invite {
    /// What identifies it. Store a hash, as Anarchy does, never the code itself.
    pub code: String,
    pub role: String,
    #[serde(default)]
    pub teams: Vec<String>,
    pub expires_at: u64,
    pub max_uses: u32,
    #[serde(default)]
    pub uses: u32,
    #[serde(default)]
    pub revoked: bool,
}

impl Invite {
    pub fn redeem(&mut self, id: &str, name: &str, now: u64) -> Result<Person, &'static str> {
        if self.revoked {
            return Err("this invite was revoked");
        }
        if now >= self.expires_at {
            return Err("this invite has expired");
        }
        if self.uses >= self.max_uses {
            return Err("this invite has been used up");
        }
        self.uses += 1;
        Ok(Person { id: id.into(), name: name.into(), role: self.role.clone(), teams: self.teams.clone() })
    }
}

/// The org's rules and what has been spent against them.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Governor {
    pub catalog: Vec<Model>,
    pub policy: Policy,
    pub people: BTreeMap<String, Person>,
    #[serde(default)]
    pub ledger: Vec<Spend>,
}

impl Governor {
    pub fn model(&self, id: &str) -> Option<&Model> {
        self.catalog.iter().find(|m| m.id == id)
    }

    pub fn effective(&self, payer: &Payer) -> Option<Effective> {
        match payer {
            Payer::Person(id) => self.people.get(id).map(|p| self.policy.effective(p)),
            Payer::Agent(id) => Some(self.policy.effective_agent(id)),
        }
    }

    /// Credits `payer` has spent in the period that contains `now`.
    pub fn spent(&self, payer: &Payer, per: Period, now: u64) -> u64 {
        let from = per.start(now);
        self.ledger.iter().filter(|s| &s.payer == payer && s.at >= from).map(|s| s.credits).sum()
    }

    /// The models `payer` may pick in a room, for the composer's picker.
    pub fn choices(&self, payer: &Payer, trust: Trust) -> Vec<&Model> {
        let Some(e) = self.effective(payer) else { return vec![] };
        self.catalog.iter().filter(|m| e.models.contains(&m.id) && allowed_in(m, &e, trust)).collect()
    }

    /// Whether `payer` may run a turn on `model` in a room with `trust`.
    /// `Ok` carries the credits left this period (`None`: no limit).
    pub fn check(&self, payer: &Payer, model: &str, trust: Trust, now: u64) -> Result<Option<u64>, Refusal> {
        let e = self.effective(payer).ok_or(Refusal::UnknownPerson)?;
        let m = self.model(model).ok_or(Refusal::UnknownModel)?;
        if !e.models.contains(&m.id) {
            return Err(Refusal::NotGranted);
        }
        if trust == Trust::Sealed && m.residency != Residency::OnDevice {
            return Err(Refusal::Sealed);
        }
        if m.residency == Residency::External && !e.external {
            return Err(Refusal::External);
        }
        let Some(a) = e.allowance else { return Ok(None) };
        let spent = self.spent(payer, a.per, now);
        if spent >= a.credits {
            return Err(Refusal::OverAllowance { spent, credits: a.credits, per: a.per });
        }
        Ok(Some(a.credits - spent))
    }

    /// Book a turn's tokens. Returns the credits it cost.
    pub fn record(&mut self, payer: Payer, model: &str, input: u64, output: u64, estimated: bool, at: u64) -> u64 {
        let w = self.model(model).map(|m| m.weight).unwrap_or(1.0);
        let credits = ((input + output) as f64 * w).ceil() as u64;
        self.ledger.push(Spend { payer, model: model.into(), input, output, credits, at, estimated });
        credits
    }
}

fn allowed_in(m: &Model, e: &Effective, trust: Trust) -> bool {
    (trust == Trust::Company || m.residency == Residency::OnDevice) && (m.residency != Residency::External || e.external)
}

/// Who pays for `agent`'s turn: its owner, or its own pool when shared.
pub fn payer_for(agent: &crate::room::Member) -> Option<Payer> {
    match &agent.who {
        crate::room::Who::Agent { owner: Some(o), .. } => Some(Payer::Person(o.clone())),
        crate::room::Who::Agent { owner: None, .. } => Some(Payer::Agent(agent.id.clone())),
        crate::room::Who::Human { .. } => None,
    }
}
