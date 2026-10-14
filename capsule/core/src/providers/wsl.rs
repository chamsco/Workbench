use crate::model::{data_dir, CreateSpec, Workspace};
use crate::providers::Provider;
use anyhow::{bail, Context, Result};
use std::process::Command;

const PREFIX: &str = "visor-";

pub struct WslProvider;

impl WslProvider {
    fn distro(name: &str) -> String {
        format!("{PREFIX}{name}")
    }

    fn wsl(args: &[&str]) -> Result<String> {
        let out = Command::new("wsl.exe")
            .args(args)
            .output()
            .context("failed to launch wsl.exe")?;
        // wsl.exe emits UTF-16LE on some invocations
        let stdout = decode_wsl_output(&out.stdout);
        if !out.status.success() {
            let stderr = decode_wsl_output(&out.stderr);
            bail!("wsl {:?} failed: {} {}", args, stdout.trim(), stderr.trim());
        }
        Ok(stdout)
    }

    fn distros() -> Result<Vec<String>> {
        let out = Self::wsl(&["--list", "--quiet"])?;
        Ok(out
            .lines()
            .map(|l| l.trim().trim_end_matches(" (Default)").to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    fn running_distro(name: &str) -> Result<bool> {
        let out = Self::wsl(&["-d", name, "-e", "true"]);
        Ok(out.is_ok())
    }
}

fn decode_wsl_output(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && (bytes[0] == 0xFF && bytes[1] == 0xFE || bytes[1] == 0 && bytes[0] != 0)
    {
        let u16s: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u16s)
    } else {
        String::from_utf8_lossy(bytes).to_string()
    }
}

impl Provider for WslProvider {
    fn name(&self) -> &'static str {
        "wsl"
    }

    fn available(&self) -> bool {
        which::which("wsl.exe").is_ok()
            && Self::wsl(&["--status"]).map(|_| true).unwrap_or(false)
    }

    fn create(&self, spec: &CreateSpec) -> Result<Workspace> {
        let distro = Self::distro(&spec.name);
        if Self::distros()?.contains(&distro) {
            bail!("workspace '{}' already exists", spec.name);
        }
        let rootfs = if let Some(src) = spec.os.strip_prefix("clone:") {
            crate::images::export_wsl_distro(src)?
        } else {
            crate::images::ensure_wsl_rootfs(&spec.os)?
        };
        let dir = data_dir()?.join("wsl").join(&spec.name);
        std::fs::create_dir_all(&dir)?;
        Self::wsl(&[
            "--import",
            &distro,
            dir.to_str().context("bad path")?,
            rootfs.to_str().context("bad path")?,
        ])?;
        // sanity: boot it once so first-boot init happens now
        Self::wsl(&["-d", &distro, "-e", "true"])?;
        Ok(Workspace {
            name: spec.name.clone(),
            provider: "wsl".into(),
            os: spec.os.clone(),
            state: "stopped".into(),
            ip: None,
            ssh_user: Some("root".into()),
            ssh_port: None,
            cpus: spec.cpus,
            memory_mib: spec.memory_mib,
            disk_gib: spec.disk_gib,
        })
    }

    fn start(&self, name: &str) -> Result<Workspace> {
        let distro = Self::distro(name);
        Self::wsl(&["-d", &distro, "-e", "true"])?;
        self.status(name)
    }

    fn stop(&self, name: &str, _force: bool) -> Result<Workspace> {
        let distro = Self::distro(name);
        // --terminate is per-distro and graceful from WSL's perspective
        Self::wsl(&["--terminate", &distro])?;
        self.status(name)
    }

    fn delete(&self, name: &str) -> Result<()> {
        let distro = Self::distro(name);
        Self::wsl(&["--unregister", &distro])?;
        let dir = data_dir()?.join("wsl").join(name);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    fn list(&self) -> Result<Vec<Workspace>> {
        let mut out = Vec::new();
        for d in Self::distros()? {
            if let Some(name) = d.strip_prefix(PREFIX) {
                out.push(self.status(name).unwrap_or_else(|_| Workspace {
                    name: name.into(),
                    provider: "wsl".into(),
                    os: "unknown".into(),
                    state: "unknown".into(),
                    ip: None,
                    ssh_user: Some("root".into()),
                    ssh_port: None,
                    cpus: 0,
                    memory_mib: 0,
                    disk_gib: 0,
                }));
            }
        }
        Ok(out)
    }

    fn status(&self, name: &str) -> Result<Workspace> {
        let mut ws = crate::model::load_meta(name)?;
        let distro = Self::distro(name);
        if !Self::distros()?.contains(&distro) {
            ws.state = "missing".into();
            return Ok(ws);
        }
        ws.state = if Self::running_distro(&distro)? {
            "running".into()
        } else {
            "stopped".into()
        };
        Ok(ws)
    }
}

impl WslProvider {
    /// Run a command inside the workspace; returns JSON {exit_code, stdout, stderr}.
    pub fn exec(&self, name: &str, cmd: &[String]) -> Result<serde_json::Value> {
        let distro = Self::distro(name);
        let out = Command::new("wsl.exe")
            .arg("-d")
            .arg(&distro)
            .arg("-e")
            .args(cmd)
            .output()
            .context("wsl exec failed to launch")?;
        Ok(serde_json::json!({
            "exit_code": out.status.code().unwrap_or(-1),
            "stdout": decode_wsl_output(&out.stdout),
            "stderr": decode_wsl_output(&out.stderr),
        }))
    }
}
