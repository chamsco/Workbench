//! UI preferences shared by both desktop shells: theme, the machines you
//! follow, whether this machine shares its harness, update checks, and the
//! tab/canvas layout. One JSON file, `~/.config/backspace/prefs.json`.

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
    /// Split positions as fractions: columns, then the 2x2 row split.
    pub cols: Vec<f32>,
    pub rows: f32,
}

impl Default for TabSpec {
    fn default() -> Self {
        Self {
            name: None,
            panes: vec![PaneSpec::default()],
            cols: vec![],
            rows: 0.56,
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

    pub fn save(&self) {
        if let Some(path) = Self::path() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(s) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, s);
            }
        }
    }
}
