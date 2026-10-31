//! Picks a (model, effort) pair per agent. Jev does the deciding when a key is
//! present; a keyword heuristic keeps the harness usable without one.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::config::{Config, ModelSpec, RouterBackend};
use crate::effort::Effort;

/// Is this base URL on this machine? A local runtime is the only thing whose
/// absence should quietly remove a model from the routing table.
fn is_loopback(base_url: &str) -> bool {
    let rest = base_url.split("://").nth(1).unwrap_or(base_url);
    let host = rest.split('/').next().unwrap_or(rest);
    let host = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
    let host = host.trim_matches(|c| c == '[' || c == ']');
    host.eq_ignore_ascii_case("localhost")
        || host == "::1"
        || host.parse::<std::net::Ipv4Addr>().map(|ip| ip.is_loopback()).unwrap_or(false)
}

/// Is a local runtime listening at this base URL?
///
/// A TCP connect is the cheapest honest answer: it catches the common case (the
/// runtime is not running) without an HTTP round trip or a model call, and it
/// assumes nothing about which runtime it is. The answer is cached for 15
/// seconds, because this runs on the routing path for every delegated job and a
/// laptop's Ollama does not start and stop between tickets.
fn local_runtime_is_up(base_url: &str) -> bool {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    static CACHE: std::sync::OnceLock<Mutex<HashMap<String, (bool, Instant)>>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let now = Instant::now();
    if let Some((up, at)) = cache.lock().unwrap().get(base_url).copied() {
        if now.duration_since(at) < Duration::from_secs(15) {
            return up;
        }
    }
    // "http://host:port/path" -> ("host", port). Loopback names default to 80,
    // which no local runtime uses, so a malformed URL reads as "not up".
    let rest = base_url.split("://").nth(1).unwrap_or(base_url);
    let authority = rest.split('/').next().unwrap_or(rest);
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.trim_matches(|c| c == '[' || c == ']'), p.parse::<u16>().unwrap_or(0)),
        None => (authority, 0),
    };
    let up = if host.is_empty() || port == 0 {
        false
    } else {
        use std::net::ToSocketAddrs;
        // Every address the name resolves to, not just the first: `localhost`
        // usually answers with ::1 before 127.0.0.1, and a runtime listening on
        // only one of them must still read as up.
        let addrs: Vec<std::net::SocketAddr> = match host.parse::<std::net::IpAddr>() {
            Ok(ip) => vec![std::net::SocketAddr::new(ip, port)],
            Err(_) => (host, port).to_socket_addrs().map(|a| a.collect()).unwrap_or_default(),
        };
        addrs
            .iter()
            .any(|addr| std::net::TcpStream::connect_timeout(addr, Duration::from_millis(400)).is_ok())
    };
    cache.lock().unwrap().insert(base_url.to_string(), (up, now));
    up
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum AgentRole {
    Main,
    Sub,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Decision {
    pub model: String,
    /// Ladder position chosen; the wire value may be clamped per model.
    pub effort: Effort,
    pub source: String,
    pub confidence: f32,
    pub router_cost_usd: f64,
    /// The routing call's cost was already added to the project's spend when
    /// it was settled (see `Router::with_recorder`), so the caller must not add it again.
    #[serde(default)]
    pub cost_settled: bool,
    pub note: Option<String>,
    /// For the planner: what the task was classified as, and whether the
    /// classifier was sure enough to act on. None for everything else.
    #[serde(default)]
    pub triage: Option<crate::decision::Triage>,
}

pub struct Router {
    http: reqwest::Client,
    cfg: Config,
    /// Small delegated jobs the user pinned to a model on this machine:
    /// "scouts" -> "ollama:lfm2.5:latest", "triage" -> ..., "workers" -> ...
    /// (Settings → Local models; model_server.rs). Empty: the big model does
    /// everything, which is the default.
    local: std::collections::BTreeMap<String, String>,
    /// Answers the typed question that says where the planner starts.
    decider: std::sync::Arc<dyn crate::decision::DecisionModel>,
    /// Holds a routing call's worst case against the project's budget. None:
    /// no budget is around this router, and its calls are declared unmetered.
    meter: Option<Meter>,
    /// Records a Jev call's actual cost on the project's books. Run by
    /// `Reservation::settle`, under the reservation table's lock and before the
    /// call's hold is given back, so no other reservation sees the hold gone
    /// and the cost not yet recorded.
    recorder: Option<std::sync::Arc<dyn Fn(f64) + Send + Sync>>,
    /// Records the cost of a call that was sent but whose cost was never
    /// reported, at the amount held for it, and marks it as unknown. Run like
    /// `recorder`, in the step that gives the hold back.
    unknown_recorder: Option<std::sync::Arc<dyn Fn(f64) + Send + Sync>>,
    /// How long a Jev request may take. A test seam; production keeps the default.
    jev_timeout: std::time::Duration,
}

/// Reserves `(usd, tokens)` for a routing call, or says why it may not go out.
pub type Meter = std::sync::Arc<dyn Fn(f64, u64) -> std::result::Result<crate::gateway::Reservation, crate::gateway::Refusal> + Send + Sync>;

/// A local pick: the decision, plus which job it was for and which runtime
/// holds it, so `route_for` can check the runtime is actually listening.
struct LocalPick {
    key: String,
    runtime: String,
    decision: Decision,
}

impl Router {
    pub fn new(http: reqwest::Client, cfg: Config) -> Self {
        Self { http, cfg, local: Default::default(), decider: std::sync::Arc::new(crate::decision::KeywordDecider), meter: None, recorder: None, unknown_recorder: None, jev_timeout: std::time::Duration::from_secs(20) }
    }

    /// Swap the decision model without touching the router.
    pub fn with_decider(mut self, decider: std::sync::Arc<dyn crate::decision::DecisionModel>) -> Self {
        self.decider = decider;
        self
    }

    /// Hold every Jev call's worst case against a budget before it is sent.
    pub fn with_meter(mut self, meter: impl Fn(f64, u64) -> std::result::Result<crate::gateway::Reservation, crate::gateway::Refusal> + Send + Sync + 'static) -> Self {
        self.meter = Some(std::sync::Arc::new(meter));
        self
    }

    /// Record a Jev call's actual cost in the same step that releases its hold.
    /// A decision whose cost was recorded this way says so (`Decision::cost_settled`)
    /// and its `router_cost_usd` is not to be added again. Without a recorder the
    /// caller adds `router_cost_usd` after the decision returns, as before.
    pub fn with_recorder(mut self, recorder: impl Fn(f64) + Send + Sync + 'static) -> Self {
        self.recorder = Some(std::sync::Arc::new(recorder));
        self
    }

    /// Record the cost of a call that was sent and never reported one, at the
    /// amount held for it, in the same step that releases the hold. Without it
    /// the ordinary recorder is used; without either the hold is released and
    /// the call's cost is lost to the books.
    pub fn with_unknown_cost_recorder(mut self, recorder: impl Fn(f64) + Send + Sync + 'static) -> Self {
        self.unknown_recorder = Some(std::sync::Arc::new(recorder));
        self
    }

    /// Change how long a Jev request may take (tests; the default is 20 seconds).
    pub fn with_jev_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.jev_timeout = timeout;
        self
    }

    /// Which small jobs run locally, and on what. Builder style so the
    /// harness can set it from prefs without a second constructor.
    pub fn with_local(mut self, local: std::collections::BTreeMap<String, String>) -> Self {
        self.local = local;
        self
    }

    /// The local model for a small job, when the user picked one for it.
    ///
    /// `role` is what the job is (`AgentRole::Sub` for a worker or a triage
    /// agent); `job` is the finer kind those roles share ("triage", "worker"),
    /// so a user can send triage to a 1.5B model and leave workers alone.
    ///
    /// A local model is on the leash only while it is usable. The config was
    /// already given a model spec for it in `harness::local_subagent`, and a
    /// keyless loopback provider counts as usable, so `route_for` additionally
    /// asks whether anything is listening before it commits to it.
    /// Which small jobs run locally, and on what: "scouts", "triage",
    /// "workers". Every caller that may be delegated to a tiny model goes
    /// through `route_for`, so the tick in Settings actually decides.
    fn local_pick(&self, role: AgentRole, job: &str) -> Option<LocalPick> {
        // "workers" and "triage" name the job; a bare Sub with no job means
        // "whatever workers are set to".
        let key = if job.trim().is_empty() {
            match role {
                AgentRole::Sub => "workers".to_string(),
                AgentRole::Main => return None,
            }
        } else {
            job.trim().to_string()
        };
        // Tolerate the plural and the singular, since the UI and the prefs file
        // have both spelled these two ways.
        let (model, provider) = self
            .local
            .iter()
            .find(|(k, _)| {
                let k = k.as_str();
                k == key || k == key.trim_end_matches('s') || format!("{k}s") == key
            })
            .map(|(k, m)| (m.clone(), k.clone()))?;
        // Only if the model really is in this config and usable right now.
        if !self.cfg.usable_models().iter().any(|m| m.id == model) {
            return None;
        }
        // `usable_models` gives no runtime id, and a model id's first segment is
        // its provider ("ollama:lfm2.5:latest" -> "ollama"), so ask the config
        // which provider actually holds this model rather than parsing the id.
        let runtime = self
            .cfg
            .models
            .iter()
            .find(|m| m.id == model)
            .map(|m| m.provider.clone())
            .unwrap_or(provider);
        Some(LocalPick {
            key,
            runtime,
            decision: Decision {
                model,
                effort: Effort::Low,
                source: format!("local-{job}"),
                confidence: 1.0,
                router_cost_usd: 0.0,
                cost_settled: false,
                note: None,
                triage: None,
            },
        })
    }

    /// The model for an agent, with the local pick first when the user asked
    /// for it. `job` is "" for "just this role".
    ///
    /// The local runtime is checked to be listening first. Without that, a
    /// stopped Ollama was still "usable" (a keyless loopback provider), so every
    /// delegated job died on a connection error instead of quietly using the
    /// model the router would otherwise have picked.
    pub async fn route_for(&self, role: AgentRole, job: &str, task: &str, context: &str) -> Result<Decision> {
        if let Some(pick) = self.local_pick(role, job) {
            // The provider has to exist and be listening. An unknown provider is
            // not a reason to hand out a model nobody can call.
            let base = self
                .cfg
                .providers
                .get(&pick.runtime)
                .map(|p| p.base_url.clone())
                .filter(|b| !b.trim().is_empty());
            if base.clone().is_some_and(|b| local_runtime_is_up(&b)) {
                let mut d = pick.decision;
                d.note = Some(format!(
                    "you are a small model on this machine, doing the {} job for a bigger agent",
                    pick.key
                ));
                return Ok(d);
            }
            return self.route(role, task, context).await.map_err(|e| {
                anyhow::anyhow!(
                    "`{}` is set for local {} jobs but nothing is listening {}; {e:#}",
                    pick.decision.model,
                    pick.key,
                    base.map(|b| format!("on {b}")).unwrap_or_else(|| "on any known endpoint".into()),
                )
            });
        }
        self.route(role, task, context).await
    }

    pub async fn route(&self, role: AgentRole, task: &str, context: &str) -> Result<Decision> {
        let mut candidates = self.cfg.usable_models();
        // A local provider that is not listening is not a candidate. The injected
        // tiny model sits at tier 0, so without this the "fall back" in
        // `route_for` would pick the very model that just failed the liveness
        // check, and the job would die on a connection error anyway.
        candidates.retain(|m| {
            self.cfg
                .providers
                .get(&m.provider)
                .map(|p| !p.base_url.trim().is_empty())
                .unwrap_or(true)
                && self
                    .cfg
                    .providers
                    .get(&m.provider)
                    .map(|p| !is_loopback(&p.base_url) || local_runtime_is_up(&p.base_url))
                    .unwrap_or(true)
        });
        // The policy's lists steer the pick; the gateway still checks the call.
        let gate = crate::gateway::Gateway::new(self.cfg.policy.clone());
        let gate_role = match role {
            AgentRole::Main => crate::gateway::Role::Orchestration,
            AgentRole::Sub => crate::gateway::Role::Execution,
        };
        candidates.retain(|m| gate.check_model(gate_role, &m.id).is_ok());
        let candidates = candidates;
        if role == AgentRole::Sub {
            if let Some(pin) = self
                .cfg
                .router
                .pin_sub
                .as_ref()
                .filter(|p| self.cfg.cli_usable(p))
            {
                // A worker pinned to a CLI needs no API model at all.
                if candidates.is_empty() {
                    return Ok(Decision {
                        model: pin.clone(),
                        effort: self.cfg.escalation.start_max_effort,
                        source: "pinned".into(),
                        confidence: 1.0,
                        router_cost_usd: 0.0,
                        cost_settled: false,
                        note: None,
                        triage: None,
                    });
                }
            }
        }
        if candidates.is_empty() {
            bail!(
                "no usable models: set an API key for one of the providers ({})",
                self.cfg
                    .providers
                    .values()
                    .map(|p| p.api_key_env.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        let pin = match role {
            AgentRole::Main => self.cfg.router.pin_main.as_ref(),
            AgentRole::Sub => self.cfg.router.pin_sub.as_ref(),
        };

        // The planner starts on the cheapest class the task allows (PLAN §3.1):
        // the frontier only for a long task the classifier is sure about. A
        // decider that fails leaves the task unclassified, which starts mid
        // rather than failing the turn or reaching for the dearest model.
        let (triage, triage_note) = if role == AgentRole::Main {
            match crate::decision::classify(self.decider.as_ref(), task, self.cfg.router.triage_threshold) {
                Ok(t) => (Some(t), None),
                Err(e) => (None, Some(format!("task not classified, starting mid: {e:#}"))),
            }
        } else {
            (None, None)
        };
        let candidates: Vec<&ModelSpec> = if role == AgentRole::Main {
            crate::ladder::within(&candidates, crate::ladder::planner_start(triage.as_ref()))
        } else {
            candidates
        };

        let mut decision = match self.cfg.router.backend {
            RouterBackend::Jev => match self.jev(role, task, context, &candidates).await {
                Ok(d) => d,
                Err(e) => {
                    let mut d = heuristic_with(role, task, &candidates, triage.as_ref());
                    d.note = Some(format!("jev unavailable, heuristic used: {e:#}"));
                    d
                }
            },
            RouterBackend::Heuristic => heuristic_with(role, task, &candidates, triage.as_ref()),
        };
        decision.triage = triage;
        if decision.note.is_none() {
            decision.note = triage_note;
        }

        if let Some(pin) = pin {
            // The planner needs Backspace's own tools, so it never runs on a
            // CLI; a CLI that isn't installed is skipped with a note.
            match self.cfg.cli_of(pin) {
                Some(_) if role == AgentRole::Main => {}
                Some(_) if !self.cfg.cli_usable(pin) => {
                    decision.note = Some(format!("{pin} is pinned but its CLI isn't installed"));
                }
                _ => {
                    decision.model = pin.clone();
                    decision.source = format!("{}+pinned", decision.source);
                }
            }
        }
        if role == AgentRole::Main && decision.effort < self.cfg.router.main_min_effort {
            decision.effort = self.cfg.router.main_min_effort;
        }
        Ok(decision)
    }

    async fn jev(
        &self,
        role: AgentRole,
        task: &str,
        context: &str,
        candidates: &[&ModelSpec],
    ) -> Result<Decision> {
        let rc = &self.cfg.router;
        // The router's own model is a model call like any other, with a role of its own: the project's
        // `routing_models` names it, or Jev is not asked and the heuristic decides. This comes before
        // the key and the reservation, so a refused call costs and holds nothing.
        crate::gateway::Gateway::new(self.cfg.policy.clone()).check_routing_model(&rc.jev_model)?;
        let key = std::env::var(&rc.jev_api_key_env)
            .ok()
            .filter(|k| !k.is_empty())
            .ok_or_else(|| anyhow!("{} not set", rc.jev_api_key_env))?;

        let mut model_criteria = Map::new();
        for m in candidates {
            model_criteria.insert(
                m.id.clone(),
                Value::String(format!(
                    "{}; ${}/${} per million input/output tokens",
                    m.description, m.input_usd_per_mtok, m.output_usd_per_mtok
                )),
            );
        }

        let body = json!({
            "model": rc.jev_model,
            "state": {
                "role": match role { AgentRole::Main => "lead agent planning and integrating a whole project", AgentRole::Sub => "sub-agent executing one scoped task" },
                "task": truncate(task, 6000),
                "context": truncate(context, 4000),
            },
            "questions": {
                "model": {
                    "type": "choice",
                    "instructions": "Which model should do this task? Pick the cheapest one that will reliably get it right; paying for a stronger model is only worth it when a weaker one would likely fail.",
                    "criteria": model_criteria,
                },
                "effort": {
                    "type": "score",
                    "instructions": "How much reasoning effort does this task need?",
                    "criteria": Effort::rubric(),
                },
            },
        });

        // Where a budget is around the router, the call's worst case (the body
        // as it goes, in bytes, and an assumed answer, at Jev's own rate) is
        // held until the response is in; if it does not fit, Jev is not called
        // and the heuristic decides. Without one the call is declared unmetered.
        // Docs: docs/cauldron/M1-BUDGET-BOUNDARIES.md.
        let hold = match &self.meter {
            Some(meter) => {
                let bytes = serde_json::to_string(&body)?.len();
                let tokens = crate::gateway::input_bound(bytes, 1, 0) + crate::gateway::ROUTER_OUTPUT_TOKENS;
                Some(meter(tokens as f64 * rc.jev_usd_per_mtok / 1_000_000.0, tokens)?)
            }
            None => {
                crate::provider::note_unmetered(crate::gateway::Unmetered::JevRouter);
                None
            }
        };
        // From here on the request may have left. Until it is known not to have, it may have been
        // billed: only a failure to connect, or to build the request at all, gives the hold back
        // as nothing spent. Every other failure keeps the amount held on the books as spent,
        // marked as an unknown cost, in the step that gives the hold back.
        let resp = match self.http.post(&rc.jev_endpoint).bearer_auth(key).json(&body).timeout(self.jev_timeout).send().await {
            Ok(r) => r,
            Err(e) if e.is_connect() || e.is_builder() => return Err(anyhow::Error::new(e).context("jev request failed")),
            Err(e) => return Err(self.unknown_cost(hold, anyhow::Error::new(e).context("jev request failed"))),
        };
        let status = resp.status();
        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => return Err(self.unknown_cost(hold, anyhow::Error::new(e).context("jev response could not be read"))),
        };
        let parsed: Option<Value> = serde_json::from_str(&text).ok();
        // What Jev says it used, from any body: a failure can still be billed.
        let reported = parsed.as_ref().and_then(|v| {
            let (i, o) = (v["usage"]["input_tokens"].as_u64(), v["usage"]["output_tokens"].as_u64());
            (i.is_some() || o.is_some()).then(|| i.unwrap_or(0) + o.unwrap_or(0))
        });
        let failure = if !status.is_success() {
            Some(anyhow!("jev returned {status}: {}", truncate(&text, 300)))
        } else if parsed.is_none() {
            Some(anyhow!("jev returned non-JSON"))
        } else {
            None
        };
        if let Some(why) = failure {
            return Err(match reported {
                Some(tokens) => {
                    self.settle(hold, tokens as f64 * rc.jev_usd_per_mtok / 1_000_000.0, tokens);
                    why
                }
                None => self.unknown_cost(hold, why),
            });
        }
        let v = parsed.expect("a 2xx body that parsed");
        // The call is billed whether or not its answer is usable, so its cost goes on the books now, in
        // the step that gives the hold back. An answer with no usage in it is used, and counted at the
        // amount held for it.
        let (cost, cost_settled, over) = match reported {
            Some(tokens) => {
                let cost = tokens as f64 * rc.jev_usd_per_mtok / 1_000_000.0;
                let (settled, over) = self.settle(hold, cost, tokens);
                (cost, settled, over)
            }
            None => {
                let (cost, settled) = self.settle_unknown(hold);
                (cost, settled, Some(unknown_note(cost, settled)))
            }
        };

        let choice = v["answers"]["model"]["choice"]
            .as_str()
            .ok_or_else(|| anyhow!("jev response missing answers.model.choice"))?;
        if !candidates.iter().any(|m| m.id == choice) {
            bail!("jev chose unknown model {choice}");
        }
        let score = v["answers"]["effort"]["score"]
            .as_f64()
            .ok_or_else(|| anyhow!("jev response missing answers.effort.score"))?;
        let confidence = v["answers"]["model"]["confidence"].as_f64().unwrap_or(0.0) as f32;

        Ok(Decision {
            model: choice.to_string(),
            // Expected score can land between levels; round half up.
            effort: Effort::from_index((score + 0.5).floor().max(0.0) as usize),
            source: "jev".into(),
            confidence,
            router_cost_usd: cost,
            cost_settled,
            note: over,
            triage: None,
        })
    }

    /// A request that was sent and then failed with no report of what it cost. Its hold is not given
    /// back as if nothing had been spent: the amount held is counted as spent, as an unknown cost, and the
    /// error says so.
    fn unknown_cost(&self, hold: Option<crate::gateway::Reservation>, why: anyhow::Error) -> anyhow::Error {
        let (usd, counted) = self.settle_unknown(hold);
        why.context(unknown_note(usd, counted))
    }

    /// Count what a call was held for as its cost, recorded under the table's lock in the step that
    /// gives the hold back (see `Reservation::settle`). Through the unknown-cost recorder when there is
    /// one, so the books can mark it; otherwise through the ordinary one. With neither, or with no
    /// budget around the router, there is nothing to count it on and the hold is simply released.
    /// Returns the amount counted and whether it was.
    fn settle_unknown(&self, hold: Option<crate::gateway::Reservation>) -> (f64, bool) {
        let Some(h) = hold else { return (0.0, false) };
        let usd = h.reserved_usd();
        match self.unknown_recorder.as_ref().or(self.recorder.as_ref()) {
            Some(record) => {
                h.settle(|| record(usd));
                (usd, true)
            }
            None => (0.0, false),
        }
    }

    /// End a Jev call's hold. With a recorder the cost is recorded under the
    /// table's lock in the same step (see `Reservation::settle`); a cost above
    /// what was held is recorded in full and named in the returned note.
    /// Without one the hold is simply given back and the caller records the
    /// cost afterwards, which leaves a gap of one call's cost.
    fn settle(&self, hold: Option<crate::gateway::Reservation>, cost: f64, tokens: u64) -> (bool, Option<String>) {
        let over = hold.as_ref().and_then(|h| h.overshoot(cost, tokens)).map(|o| {
            format!("jev overshoot: the call cost ${cost:.6}, ${:.6} more than the ${:.6} held for it; counted in full", o.usd, cost - o.usd)
        });
        match (&self.recorder, hold) {
            (Some(record), Some(h)) => h.settle(|| record(cost)),
            (Some(record), None) => record(cost),
            (None, _) => return (false, over),
        }
        (true, over)
    }
}

/// What a decision or error says about a call whose cost was never reported.
fn unknown_note(usd: f64, counted: bool) -> String {
    if counted {
        format!("jev cost is unknown: the ${usd:.6} held for the call is counted as spent")
    } else {
        "jev cost is unknown, and no budget is around the router to count it on".to_string()
    }
}

/// No-network fallback. Deliberately crude: it exists so the harness runs
/// without Jev, not to compete with it. The planner is classified with the
/// keyword baseline at the default threshold; the router passes its own.
pub fn heuristic(role: AgentRole, task: &str, candidates: &[&ModelSpec]) -> Decision {
    let triage = (role == AgentRole::Main)
        .then(|| {
            crate::decision::classify(&crate::decision::KeywordDecider, task, crate::decision::DEFAULT_TRIAGE_THRESHOLD).ok()
        })
        .flatten();
    heuristic_with(role, task, candidates, triage.as_ref())
}

/// The heuristic with the planner's classification given. Only a confident
/// "long" raises the planner to Xhigh; the old rule raised it on every task.
pub fn heuristic_with(
    role: AgentRole,
    task: &str,
    candidates: &[&ModelSpec],
    triage: Option<&crate::decision::Triage>,
) -> Decision {
    let t = task.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| t.contains(w));

    let mut level: usize = 1; // medium
    if has(&[
        "rename",
        "format",
        "typo",
        "boilerplate",
        "readme",
        "docs",
        "scaffold",
        "list",
    ]) {
        level = 0;
    }
    if has(&[
        "debug",
        "integrat",
        "refactor",
        "migrat",
        "multiple files",
        "api",
        "database",
        "auth",
    ]) {
        level = level.max(2);
    }
    if has(&[
        "architect",
        "design the",
        "plan",
        "concurren",
        "distributed",
        "performance",
    ]) {
        level = level.max(3);
    }
    if has(&["security", "cryptograph", "proof", "race condition"]) {
        level = level.max(4);
    }
    if task.len() > 4000 {
        level = level.max(2);
    }
    if role == AgentRole::Main && crate::ladder::planner_start(triage) == crate::ladder::ModelClass::Frontier {
        level = level.max(3);
    }
    let effort = Effort::from_index(level);

    let needed_tier: u8 = match effort {
        Effort::Low => 1,
        Effort::Medium | Effort::High => 2,
        Effort::Xhigh => 3,
        Effort::Max | Effort::Ultra => 4,
    };
    let model = candidates
        .iter()
        .filter(|m| m.tier >= needed_tier)
        .min_by(|a, b| a.output_usd_per_mtok.total_cmp(&b.output_usd_per_mtok))
        .or_else(|| candidates.iter().max_by_key(|m| m.tier))
        .map(|m| m.id.clone())
        .unwrap_or_default();

    Decision {
        model,
        effort,
        source: "heuristic".into(),
        confidence: 0.0,
        router_cost_usd: 0.0,
        cost_settled: false,
        note: None,
        triage: None,
    }
}

pub fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, DEFAULT_CONFIG};

    #[test]
    fn cli_pin_routes_workers_not_the_planner() {
        let mut cfg = Config::parse(DEFAULT_CONFIG).unwrap();
        // Claude Code is installed in this sandbox's CI; skip where it isn't.
        if !cfg.cli_usable("claude-code") {
            return;
        }
        cfg.router.backend = RouterBackend::Heuristic;
        cfg.router.pin_sub = Some("claude-code".into());
        cfg.router.pin_main = Some("claude-code".into());
        let r = Router::new(reqwest::Client::new(), cfg.clone());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let sub = rt
            .block_on(r.route(AgentRole::Sub, "add a button", ""))
            .unwrap();
        assert_eq!(sub.model, "claude-code");
        // No API key: the planner cannot run on a CLI, so routing fails.
        if cfg.usable_models().is_empty() {
            assert!(rt.block_on(r.route(AgentRole::Main, "plan", "")).is_err());
        } else {
            let main = rt.block_on(r.route(AgentRole::Main, "plan", "")).unwrap();
            assert_ne!(main.model, "claude-code");
        }
    }

    #[test]
    fn heuristic_scales_with_task() {
        let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
        // The router only ever offers API models to the heuristic.
        let all: Vec<&ModelSpec> = cfg
            .models
            .iter()
            .filter(|m| cfg.cli_of(&m.id).is_none())
            .collect();
        let easy = heuristic(AgentRole::Sub, "fix a typo in the readme", &all);
        assert_eq!(easy.effort, Effort::Low);
        assert_eq!(easy.model, "claude-haiku-4-5");
        let hard = heuristic(
            AgentRole::Sub,
            "audit the auth flow for security holes",
            &all,
        );
        assert_eq!(hard.effort, Effort::Max);
        assert_eq!(hard.model, "claude-fable-5-1");
        // M2: the planner no longer starts at Xhigh on every task. A small task
        // keeps its own effort (the user's `main_min_effort` floor is applied by
        // the router, not here); a long one is still planned at Xhigh or above.
        // Routing end to end is in tests/router_policy.rs.
        let main_small = heuristic(AgentRole::Main, "fix a typo in the readme", &all);
        assert!(main_small.effort < Effort::Xhigh, "{:?}", main_small.effort);
        let main_long = heuristic(
            AgentRole::Main,
            "design and build a multi-tenant billing system across several services with database migrations, auth and an admin dashboard",
            &all,
        );
        assert!(main_long.effort >= Effort::Xhigh, "{:?}", main_long.effort);
    }
}

#[cfg(test)]
mod liveness_tests {
    use super::local_runtime_is_up;
    use std::net::TcpListener;

    #[test]
    fn a_listening_port_is_up_and_a_closed_one_is_not() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        assert!(local_runtime_is_up(&format!("http://127.0.0.1:{port}/v1")), "a listener is up");
        assert!(local_runtime_is_up(&format!("http://localhost:{port}/v1")), "localhost resolves too");
        drop(l);
        // A different port that nothing can be listening on, since the answer is
        // cached per URL for 15 seconds.
        assert!(!local_runtime_is_up("http://127.0.0.1:1/v1"), "port 1 is closed");
        assert!(!local_runtime_is_up("http://127.0.0.1/v1"), "no port given");
        assert!(!local_runtime_is_up(""), "an empty base url is not up");
        assert!(!local_runtime_is_up("nonsense"), "a malformed url is not up");
    }
}
