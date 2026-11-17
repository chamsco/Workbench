//! M6: the sandbox agents' commands run in. (stub: red)
use std::path::{Path, PathBuf};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum SandboxMode { #[default] Auto, Required, Off }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SandboxConfig { pub mode: SandboxMode, pub network: bool, pub pass_env: Vec<String>, pub hide: Vec<String> }
impl Default for SandboxConfig { fn default() -> Self { SandboxConfig { mode: SandboxMode::Auto, network: true, pass_env: vec![], hide: vec![] } } }

#[derive(Clone, Debug, PartialEq)]
pub enum Backend { Bwrap(PathBuf), Seatbelt(PathBuf), Plain }
impl Backend {
    pub fn probe() -> Backend { Backend::Plain }
    pub fn isolates(&self) -> bool { todo!() }
    pub fn label(&self) -> &'static str { todo!() }
}

#[derive(Clone, Debug)]
pub struct Profile { pub project: PathBuf, pub work: PathBuf, pub writable: Vec<PathBuf>, pub read_only: Vec<PathBuf>, pub hidden: Vec<PathBuf>, pub network: bool, pub scrub_env: bool, pub pass_env: Vec<String> }
impl Profile {
    pub fn agent(_project: &Path, _work: &Path, _cfg: &SandboxConfig) -> Profile { todo!() }
    pub fn rampart(_project: &Path, _work: &Path, _cfg: &SandboxConfig) -> Profile { todo!() }
    pub fn cli(_project: &Path, _work: &Path, _bin: &Path, _cfg: &SandboxConfig) -> Profile { todo!() }
}

#[derive(Clone, Debug)]
pub struct Sandbox { pub mode: SandboxMode, pub backend: Backend }
pub struct Wrapped { pub argv: Vec<String>, pub env: Option<Vec<(String, String)>> }
impl Sandbox {
    pub fn new(_cfg: &SandboxConfig) -> Sandbox { todo!() }
    pub fn with_backend(mode: SandboxMode, backend: Backend) -> Sandbox { Sandbox { mode, backend } }
    pub fn wrap(&self, _p: &Profile, _argv: &[String]) -> Result<Wrapped> { todo!() }
}

pub struct Jail { pub sandbox: Sandbox, pub profile: Profile }

pub fn is_secret_name(_name: &str) -> bool { todo!() }
