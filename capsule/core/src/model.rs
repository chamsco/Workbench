use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSpec {
    pub name: String,
    pub os: String,
    pub cpus: u32,
    pub memory_mib: u32,
    pub disk_gib: u32,
    pub ssh_public_key: Option<String>,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub provider: String,
    pub os: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_port: Option<u16>,
    #[serde(default)]
    pub cpus: u32,
    #[serde(default)]
    pub memory_mib: u32,
    #[serde(default)]
    pub disk_gib: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Capabilities {
    pub hyperv: bool,
    pub wsl: bool,
    pub elevated: bool,
}

pub fn data_dir() -> Result<PathBuf> {
    let base = std::env::var_os("VISOR_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("visor")))
        .context("no LOCALAPPDATA; set VISOR_HOME")?;
    std::fs::create_dir_all(&base)?;
    Ok(base)
}

fn meta_dir() -> Result<PathBuf> {
    let d = data_dir()?.join("workspaces");
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

pub fn save_meta(ws: &Workspace) -> Result<()> {
    let path = meta_dir()?.join(format!("{}.json", ws.name));
    std::fs::write(path, serde_json::to_string_pretty(ws)?)?;
    Ok(())
}

pub fn load_meta(name: &str) -> Result<Workspace> {
    let path = meta_dir()?.join(format!("{name}.json"));
    let s = std::fs::read_to_string(&path)
        .with_context(|| format!("workspace '{name}' not found (no metadata)") )?;
    Ok(serde_json::from_str(&s)?)
}

pub fn delete_meta(name: &str) -> Result<()> {
    let path = meta_dir()?.join(format!("{name}.json"));
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
