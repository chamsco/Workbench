//! Picks a (model, effort) pair per agent. Jev does the deciding when a key is
//! present; a keyword heuristic keeps the harness usable without one.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::config::{Config, ModelSpec, RouterBackend};
use crate::effort::Effort;

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
    pub note: Option<String>,
}

pub struct Router {
    http: reqwest::Client,
    cfg: Config,
}

impl Router {
    pub fn new(http: reqwest::Client, cfg: Config) -> Self {
        Self { http, cfg }
    }

    pub async fn route(&self, role: AgentRole, task: &str, context: &str) -> Result<Decision> {
        let candidates = self.cfg.usable_models();
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

        let mut decision = match self.cfg.router.backend {
            RouterBackend::Jev => match self.jev(role, task, context, &candidates).await {
                Ok(d) => d,
                Err(e) => {
                    let mut d = heuristic(role, task, &candidates);
                    d.note = Some(format!("jev unavailable, heuristic used: {e:#}"));
                    d
                }
            },
            RouterBackend::Heuristic => heuristic(role, task, &candidates),
        };

        if let Some(pin) = pin {
            decision.model = pin.clone();
            decision.source = format!("{}+pinned", decision.source);
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

        let resp = self
            .http
            .post(&rc.jev_endpoint)
            .bearer_auth(key)
            .json(&body)
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await
            .context("jev request failed")?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            bail!("jev returned {status}: {}", truncate(&text, 300));
        }
        let v: Value = serde_json::from_str(&text).context("jev returned non-JSON")?;

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
        let tokens = v["usage"]["input_tokens"].as_u64().unwrap_or(0)
            + v["usage"]["output_tokens"].as_u64().unwrap_or(0);

        Ok(Decision {
            model: choice.to_string(),
            // Expected score can land between levels; round half up.
            effort: Effort::from_index((score + 0.5).floor().max(0.0) as usize),
            source: "jev".into(),
            confidence,
            router_cost_usd: tokens as f64 * rc.jev_usd_per_mtok / 1_000_000.0,
            note: None,
        })
    }
}

/// No-network fallback. Deliberately crude: it exists so the harness runs
/// without Jev, not to compete with it.
pub fn heuristic(role: AgentRole, task: &str, candidates: &[&ModelSpec]) -> Decision {
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
    if role == AgentRole::Main {
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
        note: None,
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
    fn heuristic_scales_with_task() {
        let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
        let all: Vec<&ModelSpec> = cfg.models.iter().collect();
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
        let main = heuristic(AgentRole::Main, "make a todo app", &all);
        assert!(main.effort >= Effort::Xhigh);
    }
}
