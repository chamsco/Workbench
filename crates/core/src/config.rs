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
    #[serde(default)]
    pub advisor: AdvisorConfig,
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

/// Orchestrator, advisor, scouts: the lead builds, a stronger model stays on
/// call for the decisions that matter, and small fast models do the looking.
/// "Plan on high. Delegate on medium. Keep Opus on call."
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AdvisorConfig {
    /// The advisor for agents on the Anthropic API (the server-side
    /// `advisor` tool). None: no advisor.
    pub model: Option<String>,
    /// The advisor for agents in Claude Code (`claude --advisor <this>`).
    pub cli: Option<String>,
    /// Model id scouts run on; None: the cheapest usable model.
    pub scout_model: Option<String>,
    /// The model Claude Code's own scout subagent runs on.
    pub cli_scouts: Option<String>,
    /// Scouts one call may send out at once.
    pub scouts: usize,
}

impl Default for AdvisorConfig {
    fn default() -> Self {
        Self {
            model: Some("claude-opus-5-5".into()),
            cli: Some("opus".into()),
            scout_model: None,
            cli_scouts: Some("haiku".into()),
            scouts: 3,
        }
    }
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
    /// A coding agent CLI on this machine (Claude Code, Codex, Cursor,
    /// Grok, OpenCode). A worker routed here runs the CLI headless in its
    /// ticket's worktree instead of Backspace's own tool loop.
    Cli,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key_env: String,
    /// For `kind = "cli"`: which one ("claude", "codex", "cursor", "grok",
    /// "opencode").
    #[serde(default)]
    pub cli: Option<String>,
}

impl ProviderConfig {
    pub fn api_key(&self) -> Option<String> {
        if self.api_key_env.is_empty() {
            return None;
        }
        std::env::var(&self.api_key_env)
            .ok()
            .filter(|k| !k.is_empty())
    }

    pub fn is_cli(&self) -> bool {
        self.kind == ProviderKind::Cli
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

    /// API models whose provider has credentials (or is keyless-local)
    /// right now. CLI harnesses are never in this pool: they run only when
    /// pinned, see [`Config::cli_usable`].
    pub fn usable_models(&self) -> Vec<&ModelSpec> {
        self.models
            .iter()
            .filter(|m| {
                self.providers.get(&m.provider).is_some_and(|p| {
                    !p.is_cli()
                        && (p.api_key().is_some()
                            || p.base_url.contains("localhost")
                            || p.base_url.contains("127.0.0.1"))
                })
            })
            .collect()
    }

    /// The provider behind a model, when it is a CLI.
    pub fn cli_of(&self, model: &str) -> Option<&ProviderConfig> {
        let m = self.model(model)?;
        self.providers.get(&m.provider).filter(|p| p.is_cli())
    }

    /// A CLI model whose binary is installed.
    pub fn cli_usable(&self, model: &str) -> bool {
        self.cli_of(model)
            .and_then(|p| p.cli.as_deref())
            .is_some_and(|c| crate::harnesses::which(crate::harnesses::bin_for(c)).is_some())
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
        assert!(cfg.model("claude-sonnet-5-5").is_some());
        assert_eq!(cfg.router.main_min_effort, Effort::High);
    }
}
