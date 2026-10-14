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
    /// The same canvas layout, per project. `tabs` above stays the fallback for
    /// when no project is open, so an existing prefs file keeps working and a
    /// layout made in one project is never written into another's namespace.
    /// Keys are `Prefs::tab_key()`.
    pub tabs_by_project: BTreeMap<String, TabsFor>,
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
    /// Whether messages you send are scored for notes to keep on their own.
    pub memory_auto: bool,
    /// Whether memory dreams (tidies itself with a model) about once a day.
    /// Off by default: it sends recent messages from every chat to one model.
    pub memory_dream: bool,
    /// Inside Chat: "chat" (one model) or "agents" (your named agents and groups).
    pub chat_view: String,
    /// The footer's "Motion": "full" or "reduced".
    pub motion: String,
    /// The Workbench body: "canvases" (tabs of split canvases), "wall"
    /// (every agent's session tiled) or "inbox" (what needs you, then what runs).
    pub wb_view: String,
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
    /// Model id the Workbench planner runs on; a CLI one ("claude-code",
    /// "codex") runs it on your subscription. None: an API model if a key
    /// is set, else the workers' CLI.
    pub planner: Option<String>,
    /// What coding CLIs may do without asking: "edits", "auto" or "full".
    pub cli_permission: String,
    /// Effort for coding CLIs: low, medium, high, xhigh, max. None: theirs.
    pub cli_effort: Option<String>,
    /// The window's backdrop: "harbour", "dawn", "night", "open-sea",
    /// "fog", "aurora" or "plain".
    pub backdrop: String,
    /// How Backspace tells you something needs you: "system" (the OS's
    /// notifications), "app" (inside the app only) or "off".
    pub notify: String,
    /// Where a new chat goes unless the user picks otherwise.
    pub default_route: Option<crate::chat::Route>,
    /// Where traces go besides this machine (Settings → Tracing).
    pub tracing: crate::trace::Export,
    /// Agents that live elsewhere and speak A2A, brought into Backspace.
    pub remote_agents: Vec<RemoteAgent>,
    /// Outside tools (MCP servers) bots can be given (a bot's Connected apps).
    pub connections: Vec<Connection>,
    /// Colours, fonts and contrast, layered over the theme (Settings → Appearance).
    pub appearance: Appearance,
    /// Small local jobs (reading, classifying, summarising) go here when a
    /// runner is up: the `model_server` id of a tiny model, e.g.
    /// "ollama:lfm2.5:latest". None: only the big agent's own model.
    pub subagent_model: Option<String>,
    /// Which small delegated jobs that model runs, from "scouts", "triage"
    /// and "workers". Empty: scouts, the moment a model is chosen (they are
    /// the read-only lookups that are safe to hand to a small model).
    pub local_tasks: Vec<String>,
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
            tabs_by_project: BTreeMap::new(),
            onboarded: false,
            mode: "chat".into(),
            code_view: "agents".into(),
            split: None,
            pinned_apps: vec![],
            memory_on: true,
            memory_auto: true,
            memory_dream: false,
            chat_view: "chat".into(),
            motion: "full".into(),
            wb_view: "canvases".into(),
            planner: None,
            cli_permission: "edits".into(),
            cli_effort: None,
            backdrop: "harbour".into(),
            notify: "system".into(),
            companion: CompanionCfg::default(),
            uses: vec!["chat".into(), "code".into()],
            harnesses: BTreeMap::new(),
            ollama_url: "http://localhost:11434".into(),
            routers: vec![],
            cloud: CloudCfg::default(),
            projects: vec![],
            default_route: None,
            tracing: Default::default(),
            remote_agents: vec![],
            connections: vec![],
            worker: None,
            api_keys: BTreeMap::new(),
            appearance: Appearance::default(),
            subagent_model: None,
            local_tasks: vec![],
        }
    }
}

/// The Appearance panel: an accent, a background and a foreground on top of
/// the light/dark theme, the two font stacks, translucency and contrast.
/// Every field empty means "whatever the theme already says".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Appearance {
    /// Accent for the light theme, a hex colour ("#cf5a24").
    pub accent: String,
    /// Accent for the dark theme; empty falls back to `accent`.
    pub accent_dark: String,
    pub bg: String,
    pub fg: String,
    /// Empty: the bundled Inter. A name is used verbatim, then a system one.
    pub ui_font: String,
    /// Empty: the platform monospace stack.
    pub code_font: String,
    /// Off makes the window opaque (a solid `--pane`) instead of frosted.
    pub translucent: bool,
    /// 0.6 (softest) .. 1.4 (sharpest). 1.0 is the theme as designed.
    pub contrast: f32,
}

impl Appearance {
    /// Contrast rounded into the range the panel offers, 1.0 when unset.
    pub fn contrast_or_default(&self) -> f32 {
        if self.contrast <= 0.0 {
            1.0
        } else {
            self.contrast.clamp(0.6, 1.4)
        }
    }

    /// The CSS custom properties this panel owns, as `(name, value)`.
    /// Returned rather than written here so the shells can apply them to
    /// whichever document holds the window.
    pub fn css(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let var = |n: &str, v: &str| Some((n.to_string(), v.to_string()));
        if !self.accent.is_empty() {
            out.extend(var("--bs-accent", &self.accent));
        }
        if !self.bg.is_empty() {
            out.extend(var("--page-bg", &self.bg));
        }
        if !self.fg.is_empty() {
            out.extend(var("--fg", &self.fg).into_iter().chain(var("--page-fg", &self.fg)));
        }
        if !self.ui_font.is_empty() {
            out.extend(var("--font-ui", &self.ui_font));
        }
        if !self.code_font.is_empty() {
            out.extend(var("--font-mono", &self.code_font));
        }
        out
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct CompanionCfg {
    pub enabled: bool,
    /// Listens on every interface so a phone on the same network can reach it.
    pub addr: String,
    pub token: String,
    /// Off (the default): the listener binds loopback, where only this machine
    /// can reach it. Turn it on when a phone has to connect over the network —
    /// the address then has to be a LAN one too. Two switches because the
    /// audit's point stands: plain HTTP with a token, chats that drive agents,
    /// and a bot webhook is not something to expose by accident.
    pub allow_lan: bool,
}

impl Default for CompanionCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            addr: "127.0.0.1:7421".into(),
            token: crate::remote::new_token(),
            allow_lan: false,
        }
    }
}

impl CompanionCfg {
    /// The address to bind: loopback while `allow_lan` is off, whatever the
    /// user typed once it is on. An address the user set explicitly to a
    /// non-loopback one is honoured only with `allow_lan`, so upgrading from a
    /// prefs file that carried `0.0.0.0` cannot open the port by itself.
    pub fn bind_addr(&self) -> String {
        let addr = self.addr.trim();
        if self.allow_lan {
            return if addr.is_empty() { "0.0.0.0:7421".into() } else { addr.to_string() };
        }
        let port = addr.rsplit(':').next().filter(|p| !p.is_empty()).unwrap_or("7421");
        format!("127.0.0.1:{port}")
    }

    /// Is this address on this machine only?
    pub fn is_loopback(&self) -> bool {
        let a = self.bind_addr();
        a.starts_with("127.") || a.starts_with("localhost") || a.starts_with("[::1]")
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

/// A remote agent over A2A: its card's name and description, and where
/// to send messages.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct RemoteAgent {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The JSON-RPC endpoint from its card.
    pub url: String,
    /// Bearer token, if it wants one.
    pub token: String,
}

/// An MCP server bots can use: a command that runs one here, or its URL.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Connection {
    /// Its MCP server name: lowercase letters, digits and underscores.
    pub id: String,
    pub name: String,
    /// `npx -y @modelcontextprotocol/server-github`, or `https://mcp.example.com/mcp`.
    pub target: String,
}

impl Connection {
    /// Its entry in an `mcpServers` config.
    /// ponytail: the command splits on spaces; quoted arguments and env come when a server needs them.
    pub fn server(&self) -> serde_json::Value {
        let t = self.target.trim();
        if t.starts_with("http://") || t.starts_with("https://") {
            return serde_json::json!({"type": "http", "url": t});
        }
        let mut parts = t.split_whitespace();
        serde_json::json!({"command": parts.next().unwrap_or(""), "args": parts.collect::<Vec<_>>()})
    }
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

/// One project's canvas layout: the tabs it had open and which was showing.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct TabsFor {
    pub tabs: Vec<TabSpec>,
    pub active: usize,
}

/// A title-bar tab: up to four canvases side by side (four = 2x2).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TabSpec {    /// None: the tab shows its layout icon instead of a name.
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
        // One canvas to start; you (or the agents) add the rest as the work grows.
        vec![TabSpec::new(None, vec![PaneSpec::agent(0)])]
    }
}

/// What a canvas shows. `kind`: "empty" (the picker), "agent", "files",
/// "browser", "diagram", "docs" or "options".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct PaneSpec {
    pub kind: String,
    pub agent: usize,
    pub url: Option<String>,
    /// For "options": which set to show (an `options::OptionSet` id).
    pub option: Option<String>,
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
            option: None,
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
        // A project's layout must never be empty either: an entry with no tabs
        // would render a tab bar with nothing in it.
        for t in p.tabs_by_project.values_mut() {
            if t.tabs.is_empty() {
                t.tabs = TabSpec::defaults();
            }
            t.active = t.active.min(t.tabs.len() - 1);
        }
        p
    }

    /// Where a project's canvas layout is filed. `workspace` is the open
    /// project's absolute path, or "" when none is open and the global `tabs`
    /// apply. Case is folded so a path typed with different case is one entry.
    pub fn tab_key(workspace: &str) -> String {
        let w = workspace.trim();
        if w.is_empty() {
            return String::new();
        }
        w.replace('\\', "/").trim_end_matches('/').to_lowercase()
    }

    /// The tabs to show for a workspace: its own, made from the global default
    /// the first time that project is opened.
    pub fn tabs_for(&self, workspace: &str) -> TabsFor {
        let key = Self::tab_key(workspace);
        if key.is_empty() {
            return TabsFor { tabs: self.tabs.clone(), active: self.active_tab };
        }
        self.tabs_by_project.get(&key).cloned().unwrap_or_else(|| TabsFor {
            tabs: TabSpec::defaults(),
            active: 0,
        })
    }

    /// File a workspace's tabs. An empty workspace writes the global layout, so
    /// the app still remembers where you were with no project open.
    pub fn set_tabs_for(&mut self, workspace: &str, tabs: Vec<TabSpec>, active: usize) {
        let key = Self::tab_key(workspace);
        if key.is_empty() {
            self.set_global_tabs(tabs, active);
            return;
        }
        let tabs = if tabs.is_empty() { TabSpec::defaults() } else { tabs };
        let active = active.min(tabs.len() - 1);
        self.tabs_by_project.insert(key, TabsFor { tabs, active });
    }

    fn set_global_tabs(&mut self, tabs: Vec<TabSpec>, active: usize) {
        if !tabs.is_empty() {
            self.tabs = tabs;
            self.active_tab = active.min(self.tabs.len() - 1);
        }
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
