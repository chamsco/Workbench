//! UI preferences shared by both desktop shells: theme, the machines you
//! follow, whether this machine shares its harness, update checks, and the
//! tab/canvas layout. One JSON file, `~/.config/backspace/prefs.json`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Prefs {
    /// "system", "light" or "dark".
    pub theme: String,
    pub machines: Vec<MachineCfg>,
    pub share: Share,
    pub check_updates: bool,
    pub tabs: Vec<TabSpec>,
    pub active_tab: usize,
    /// False until the first-run setup is finished or skipped.
    pub onboarded: bool,
    /// Which zone of the rail was showing: "chat", "code", "memory",
    /// "apps" or "app:<id>".
    pub mode: String,
    /// Inside Code: "pair" (you and one CLI, turn by turn, in the project)
    /// or "agents" (the planner, tickets and workers).
    pub code_view: String,
    /// A second pane beside the zone: what it shows and its share of the width.
    pub split: Option<SplitCfg>,
    /// Installed apps shown in the rail, in order.
    pub pinned_apps: Vec<String>,
    /// Whether chats and agents get the memory notes.
    pub memory_on: bool,
    /// The phone companion's link (docs/companion.md).
    pub companion: CompanionCfg,
    /// What the user picked in setup: "chat", "code" or both. The switch
    /// between them only shows when both are on.
    pub uses: Vec<String>,
    /// Harness id -> on/off, only for ones the user has toggled; the rest
    /// follow the scan (on when installed and signed in).
    pub harnesses: BTreeMap<String, bool>,
    pub ollama_url: String,
    pub routers: Vec<RouterCfg>,
    pub cloud: CloudCfg,
    /// Project folders, most recent first.
    pub projects: Vec<String>,
    /// API keys entered in the app, by environment variable name
    /// (ANTHROPIC_API_KEY, OPENROUTER_API_KEY...). Apps opened from the Dock
    /// don't see your shell's variables; these are applied at startup.
    pub api_keys: BTreeMap<String, String>,
    /// Model id every coding worker runs on ("claude-code", "codex"...).
    /// None: the router decides per ticket.
    pub worker: Option<String>,
    /// Where a new chat goes unless the user picks otherwise.
    pub default_route: Option<crate::chat::Route>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            machines: vec![],
            share: Share::default(),
            check_updates: true,
            tabs: TabSpec::defaults(),
            active_tab: 0,
            onboarded: false,
            mode: "chat".into(),
            code_view: "agents".into(),
            split: None,
            pinned_apps: vec![],
            memory_on: true,
            companion: CompanionCfg::default(),
            uses: vec!["chat".into(), "code".into()],
            harnesses: BTreeMap::new(),
            ollama_url: "http://localhost:11434".into(),
            routers: vec![],
            cloud: CloudCfg::default(),
            projects: vec![],
            default_route: None,
            worker: None,
            api_keys: BTreeMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct CompanionCfg {
    pub enabled: bool,
    /// Listens on every interface so a phone on the same network can reach it.
    pub addr: String,
    pub token: String,
}

impl Default for CompanionCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            addr: "0.0.0.0:7421".into(),
            token: crate::remote::new_token(),
        }
    }
}

/// The second pane of the split view.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SplitCfg {
    /// "chat", "code", "memory" or "app:<id>".
    pub what: String,
    /// The main pane's share of the width, 0.2..0.8.
    pub ratio: f32,
}

/// Any OpenAI-compatible endpoint: OpenRouter, LM Studio, vLLM, LiteLLM,
/// a company gateway...
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct RouterCfg {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    /// Models the user picked to show; empty means all the endpoint lists.
    pub models: Vec<String>,
}

/// The Backspace Cloud account on this machine.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct CloudCfg {
    pub url: String,
    /// Account token from sign-up; empty = not signed up.
    pub token: String,
}

impl Default for CloudCfg {
    fn default() -> Self {
        Self {
            url: std::env::var("BACKSPACE_CLOUD_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:7430".into()),
            token: String::new(),
        }
    }
}

/// A remote machine running the harness (`backspace-cli serve`, or a
/// desktop shell with sharing on).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MachineCfg {
    pub name: String,
    pub url: String,
    pub token: String,
}

/// Serving this machine's harness to others.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Share {
    pub enabled: bool,
    pub addr: String,
    pub token: String,
}

impl Default for Share {
    fn default() -> Self {
        Self {
            enabled: false,
            addr: "127.0.0.1:7420".into(),
            token: crate::remote::new_token(),
        }
    }
}

/// A title-bar tab: up to four canvases side by side (four = 2x2).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TabSpec {
    /// None: the tab shows its layout icon instead of a name.
    pub name: Option<String>,
    pub panes: Vec<PaneSpec>,
    /// How the panes are arranged; None (older prefs) means the default
    /// for their count.
    pub layout: Option<crate::layout::Node>,
}

impl Default for TabSpec {
    fn default() -> Self {
        Self {
            name: None,
            panes: vec![PaneSpec::default()],
            layout: None,
        }
    }
}

impl TabSpec {
    pub fn new(name: Option<String>, panes: Vec<PaneSpec>) -> Self {
        Self {
            name,
            panes,
            ..Self::default()
        }
    }

    /// The layout to draw: the saved one if it still matches the panes.
    pub fn tree(&self) -> crate::layout::Node {
        match &self.layout {
            Some(t) if t.valid_for(self.panes.len()) => t.clone(),
            _ => crate::layout::Node::default_for(self.panes.len()),
        }
    }

    pub fn defaults() -> Vec<TabSpec> {
        vec![
            TabSpec::new(
                Some("Terminals".into()),
                vec![PaneSpec::agent(0), PaneSpec::files(0)],
            ),
            TabSpec::new(Some("Browser".into()), vec![PaneSpec::of("browser")]),
            TabSpec::new(Some("Diagram".into()), vec![PaneSpec::of("diagram")]),
            TabSpec::new(Some("PLAN.md".into()), vec![PaneSpec::of("docs")]),
        ]
    }
}

/// What a canvas shows. `kind`: "empty" (the picker), "agent", "files",
/// "browser", "diagram" or "docs".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct PaneSpec {
    pub kind: String,
    pub agent: usize,
    pub url: Option<String>,
}

impl Default for PaneSpec {
    fn default() -> Self {
        Self::of("empty")
    }
}

impl PaneSpec {
    pub fn of(kind: &str) -> Self {
        Self {
            kind: kind.into(),
            agent: 0,
            url: None,
        }
    }
    pub fn agent(id: usize) -> Self {
        Self {
            agent: id,
            ..Self::of("agent")
        }
    }
    pub fn files(id: usize) -> Self {
        Self {
            agent: id,
            ..Self::of("files")
        }
    }
}

impl Prefs {
    /// `BACKSPACE_PREFS` overrides the location (tests, benches, profiles).
    pub fn path() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os("BACKSPACE_PREFS") {
            return Some(PathBuf::from(p));
        }
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
        Some(PathBuf::from(home).join(".config/backspace/prefs.json"))
    }

    pub fn load() -> Prefs {
        let mut p: Prefs = Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if p.tabs.is_empty() {
            p.tabs = TabSpec::defaults();
        }
        p.active_tab = p.active_tab.min(p.tabs.len() - 1);
        p
    }

    /// Holds API keys and tokens, so it is written readable by you only.
    pub fn save(&self) {
        if let Some(path) = Self::path() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(s) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(&path, s);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
                }
            }
        }
    }

    /// Put the saved API keys into this process's environment, where the
    /// harness config looks for them. A variable already set wins.
    pub fn apply_keys(&self) {
        for (k, v) in &self.api_keys {
            if !v.is_empty() && std::env::var_os(k).is_none_or(|x| x.is_empty()) {
                std::env::set_var(k, v);
            }
        }
    }

    /// Remember a project folder at the top of the recent list.
    pub fn touch_project(&mut self, path: &str) {
        self.projects.retain(|p| p != path);
        self.projects.insert(0, path.to_string());
        self.projects.truncate(12);
    }

    /// Where chats, attachments and the CLI scratch folder live.
    /// `BACKSPACE_DATA` overrides it.
    pub fn data_dir() -> PathBuf {
        if let Some(p) = std::env::var_os("BACKSPACE_DATA") {
            return PathBuf::from(p);
        }
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        if cfg!(target_os = "macos") {
            home.join("Library/Application Support/Backspace")
        } else {
            home.join(".local/share/backspace")
        }
    }
}
