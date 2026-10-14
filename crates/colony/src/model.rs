//! The shapes Colony keeps: a workspace, and the vocabulary its backends
//! speak. Nothing here talks to a VM.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Where a workspace is in its life.
///
/// visor-core knows more words than this (`off`, `paused`, `saved`,
/// `missing`); [`State::from_cli`] folds them in and says what is lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Creating,
    Running,
    Stopped,
    Failed,
    Deleted,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Creating => "creating",
            State::Running => "running",
            State::Stopped => "stopped",
            State::Failed => "failed",
            State::Deleted => "deleted",
        }
    }

    /// The CLI's own word for a state → ours.
    ///
    /// Deliberately lossy: `paused`/`saved` are not running, so they read as
    /// stopped; `missing` (metadata exists, the VM or distro does not) and any
    /// word a newer CLI invents read as failed, because the workspace is not
    /// usable and we will not guess further.
    pub fn from_cli(s: &str) -> State {
        match s.trim().to_ascii_lowercase().as_str() {
            "creating" => State::Creating,
            "running" => State::Running,
            "stopped" | "off" | "paused" | "saved" => State::Stopped,
            "deleted" => State::Deleted,
            _ => State::Failed,
        }
    }
}

/// One disposable computer, as the registry remembers it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    /// Unique within a colony; the name chat says out loud.
    pub name: String,
    /// "wsl" or "hyperv" — the CLI's word, kept as-is.
    pub provider: String,
    /// The image it was built from ("ubuntu-24.04", "clone:Ubuntu").
    pub os: String,
    pub state: State,
    /// Guest IP when the backend knows one (Hyper-V running guests).
    #[serde(default)]
    pub ip: Option<String>,
    /// Unix milliseconds when the colony created it.
    #[serde(default)]
    pub created: u64,
    /// One line on what this workspace is for. Colony's own field; the CLI
    /// never sees it.
    #[serde(default)]
    pub task: Option<String>,
}

/// Which provider a workspace uses. visor-core also defaults to WSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Hyperv,
    Wsl,
}

impl Default for ProviderKind {
    fn default() -> Self {
        ProviderKind::Wsl
    }
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderKind::Hyperv => "hyperv",
            ProviderKind::Wsl => "wsl",
        }
    }
}

/// What to create. Defaults match `visor-core create`'s own defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateSpec {
    pub name: String,
    #[serde(default)]
    pub provider: ProviderKind,
    #[serde(default = "default_os")]
    pub os: String,
    #[serde(default = "default_cpus")]
    pub cpus: u32,
    #[serde(default = "default_memory_mib")]
    pub memory_mib: u32,
    #[serde(default = "default_disk_gib")]
    pub disk_gib: u32,
    /// Host path to an SSH public key. The CLI reads the file itself, so this
    /// is a path, not the key.
    #[serde(default)]
    pub ssh_key_path: Option<String>,
    /// apt packages to install on the guest's first boot.
    #[serde(default)]
    pub packages: Vec<String>,
    /// What this workspace is for. Stored in `colony.json`, not sent to the
    /// CLI.
    #[serde(default)]
    pub task: Option<String>,
}

fn default_os() -> String {
    "ubuntu-24.04".into()
}
fn default_cpus() -> u32 {
    2
}
fn default_memory_mib() -> u32 {
    4096
}
fn default_disk_gib() -> u32 {
    20
}

impl CreateSpec {
    pub fn new(name: impl Into<String>) -> Self {
        CreateSpec {
            name: name.into(),
            provider: ProviderKind::default(),
            os: default_os(),
            cpus: default_cpus(),
            memory_mib: default_memory_mib(),
            disk_gib: default_disk_gib(),
            ssh_key_path: None,
            packages: Vec::new(),
            task: None,
        }
    }
}

/// What the host can do, from `visor-core capabilities`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Capabilities {
    pub hyperv: bool,
    pub wsl: bool,
    pub elevated: bool,
    /// Which providers are actually present, in this order: hyperv, wsl.
    #[serde(default)]
    pub providers: Vec<String>,
}

impl Capabilities {
    /// visor-core prints `{hyperv, wsl, elevated}`; `providers` is ours and is
    /// derived from those three when the CLI did not supply it.
    pub fn from_cli(v: &Value) -> Result<Capabilities> {
        let mut caps: Capabilities = serde_json::from_value(v.clone())
            .context("visor-core capabilities was not the expected shape")?;
        if caps.providers.is_empty() {
            if caps.hyperv {
                caps.providers.push("hyperv".into());
            }
            if caps.wsl {
                caps.providers.push("wsl".into());
            }
        }
        Ok(caps)
    }
}

/// What a command in a workspace returned. `visor-core exec` calls the exit
/// code `exit_code`; we call it `code` and accept both on the way in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecOut {
    #[serde(alias = "exit_code")]
    pub code: i32,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
}

impl ExecOut {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
}

/// Unix milliseconds. The registry stamps `created` with it.
pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
