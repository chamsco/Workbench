use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::effort::Effort;

pub const DEFAULT_CONFIG: &str = include_str!("../default-config.toml");

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub router: RouterConfig,
    pub orchestrator: OrchestratorConfig,
    #[serde(default)]
    pub escalation: EscalationConfig,
    #[serde(default)]
    pub workflow: WorkflowConfig,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub models: Vec<ModelSpec>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouterConfig {
    pub backend: RouterBackend,
    pub jev_endpoint: String,
    pub jev_model: String,
    pub jev_api_key_env: String,
    pub jev_usd_per_mtok: f64,
    pub main_min_effort: Effort,
    pub pin_main: Option<String>,
    pub pin_sub: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum RouterBackend {
    Jev,
    Heuristic,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OrchestratorConfig {
    pub max_parallel_calls: usize,
    pub max_subagents: usize,
    pub auto_approve: bool,
    pub bash_timeout_secs: u64,
    /// Delegation depth. Main is 0; `2` lets main's reports hire their own.
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    /// Deliverables from agents at depth <= this go to the human; deeper ones
    /// are reviewed by the agent that spawned them.
    #[serde(default = "default_human_review_depth")]
    pub human_review_depth: usize,
    /// Hard stop for the whole project, router calls included.
    #[serde(default)]
    pub max_project_usd: Option<f64>,
    #[serde(default)]
    pub isolation: Isolation,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Isolation {
    /// Each ticket agent gets its own git worktree and branch.
    #[default]
    Worktree,
    /// Everyone edits the workspace directly. Only for throwaway experiments.
    Shared,
}

/// Start cheap, climb on evidence. Workers begin at most at these caps; a
/// failed check, a rejection, a detected loop or an exhausted turn budget
/// moves them one rung up (effort first, then a stronger model).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EscalationConfig {
    pub start_max_effort: Effort,
    pub start_max_tier: u8,
    pub max_steps: usize,
    /// Identical consecutive tool calls that count as a loop.
    pub loop_repeats: usize,
}

impl Default for EscalationConfig {
    fn default() -> Self {
        Self {
            start_max_effort: Effort::Medium,
            start_max_tier: 2,
            max_steps: 3,
            loop_repeats: 3,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkflowConfig {
    /// Main agent grills you in rounds before planning (mattpocock `grilling`).
    pub grill: bool,
    /// Every ticket batch from the main agent needs your approval before work.
    pub plan_approval: bool,
    /// Skills injected into each role's system prompt. Others stay loadable.
    pub main_skills: Vec<String>,
    pub worker_skills: Vec<String>,
    pub triage_skills: Vec<String>,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            grill: true,
            plan_approval: true,
            main_skills: vec!["grilling".into(), "to-tickets".into()],
            worker_skills: vec![],
            triage_skills: vec!["triage".into()],
        }
    }
}

fn default_max_depth() -> usize {
    2
}

fn default_human_review_depth() -> usize {
    1
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Anthropic,
    /// OpenAI-compatible Chat Completions.
    Openai,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key_env: String,
}

impl ProviderConfig {
    pub fn api_key(&self) -> Option<String> {
        std::env::var(&self.api_key_env)
            .ok()
            .filter(|k| !k.is_empty())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModelSpec {
    /// Name Backspace and Jev use for this entry.
    pub id: String,
    pub provider: String,
    /// Wire model name; defaults to `id`.
    pub model: Option<String>,
    /// 1 = cheapest/weakest .. 4 = strongest. Used by the heuristic router.
    pub tier: u8,
    pub description: String,
    pub input_usd_per_mtok: f64,
    pub output_usd_per_mtok: f64,
    pub max_output_tokens: u32,
    /// Effort levels the provider accepts for this model. Empty = no knob.
    #[serde(default)]
    pub efforts: Vec<Effort>,
}

impl ModelSpec {
    pub fn wire_model(&self) -> &str {
        self.model.as_deref().unwrap_or(&self.id)
    }

    pub fn cost(&self, input_tokens: u64, output_tokens: u64) -> f64 {
        (input_tokens as f64 * self.input_usd_per_mtok
            + output_tokens as f64 * self.output_usd_per_mtok)
            / 1_000_000.0
    }
}

impl Config {
    pub fn parse(src: &str) -> Result<Config> {
        let cfg: Config = toml::from_str(src).context("invalid backspace config")?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// `$BACKSPACE_CONFIG`, then `<workspace>/backspace.toml`, then
    /// `~/.config/backspace/config.toml`, then the built-in default.
    pub fn load(workspace: &Path) -> Result<(Config, Option<PathBuf>)> {
        for path in Self::search_paths(workspace) {
            if path.is_file() {
                let src = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                let cfg = Self::parse(&src).with_context(|| format!("in {}", path.display()))?;
                return Ok((cfg, Some(path)));
            }
        }
        Ok((Self::parse(DEFAULT_CONFIG)?, None))
    }

    pub fn search_paths(workspace: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Ok(p) = std::env::var("BACKSPACE_CONFIG") {
            paths.push(PathBuf::from(p));
        }
        paths.push(workspace.join("backspace.toml"));
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(PathBuf::from(home).join(".config/backspace/config.toml"));
        }
        paths
    }

    pub fn model(&self, id: &str) -> Option<&ModelSpec> {
        self.models.iter().find(|m| m.id == id)
    }

    /// Models whose provider has credentials (or is keyless-local) right now.
    pub fn usable_models(&self) -> Vec<&ModelSpec> {
        self.models
            .iter()
            .filter(|m| {
                self.providers.get(&m.provider).is_some_and(|p| {
                    p.api_key().is_some()
                        || p.base_url.contains("localhost")
                        || p.base_url.contains("127.0.0.1")
                })
            })
            .collect()
    }

    fn validate(&self) -> Result<()> {
        if self.models.is_empty() {
            bail!("config has no [[models]]");
        }
        for m in &self.models {
            if !self.providers.contains_key(&m.provider) {
                bail!("model {} references unknown provider {}", m.id, m.provider);
            }
        }
        for pin in [&self.router.pin_main, &self.router.pin_sub]
            .into_iter()
            .flatten()
        {
            if self.model(pin).is_none() {
                bail!("router pin {pin} is not a configured model id");
            }
        }
        if self.orchestrator.max_parallel_calls == 0 {
            bail!("orchestrator.max_parallel_calls must be >= 1");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
        assert!(cfg.model("claude-sonnet-5").is_some());
        assert_eq!(cfg.router.main_min_effort, Effort::High);
    }
}
