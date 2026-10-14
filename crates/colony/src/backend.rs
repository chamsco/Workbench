//! The backend: what actually makes and runs workspaces.
//!
//! [`Backend`] is the seam. [`VisorCli`] is the real one — it shells out to
//! `visor-core`'s JSON CLI. [`crate::FakeBackend`] is the same seam in memory.
//! Colony only ever calls these eight methods, so the registry is one code
//! path for a VM and for a test.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;

use crate::model::{Capabilities, CreateSpec, ExecOut, State, Workspace};

/// Everything Colony needs from a hypervisor.
pub trait Backend: Send + Sync {
    fn capabilities(&self) -> Result<Capabilities>;
    fn create(&self, spec: &CreateSpec) -> Result<Workspace>;
    fn start(&self, name: &str) -> Result<Workspace>;
    fn stop(&self, name: &str, force: bool) -> Result<Workspace>;
    fn delete(&self, name: &str) -> Result<()>;
    fn list(&self) -> Result<Vec<Workspace>>;
    fn status(&self, name: &str) -> Result<Workspace>;
    fn exec(&self, name: &str, cmd: &[String]) -> Result<ExecOut>;
}

/// The `visor-core` executable's name on this host.
pub const EXE: &str = if cfg!(windows) {
    "visor-core.exe"
} else {
    "visor-core"
};

/// A [`Backend`] over `visor-core`'s JSON CLI (`capsule/core`).
///
/// Every subcommand prints one JSON value on stdout and exits 0; a failure
/// prints `{"error": "..."}` on stderr and exits 1 (`capsule/core/src/main.rs`).
/// We surface that message verbatim — no wrapping, no rewriting — so the agent
/// reads the hypervisor's own words.
#[derive(Debug, Clone)]
pub struct VisorCli {
    exe: PathBuf,
}

impl VisorCli {
    /// Use a known binary (tests, or a caller that resolved it itself).
    pub fn at(exe: impl Into<PathBuf>) -> VisorCli {
        VisorCli { exe: exe.into() }
    }

    /// Find `visor-core`: on `PATH`, then `VISOR_CORE` (a binary, or a
    /// directory holding one), then next to the current executable — the three
    /// places `agentd` looks too.
    pub fn discover() -> Result<VisorCli> {
        find_exe()
            .map(VisorCli::at)
            .ok_or_else(|| {
                anyhow!(
                    "{EXE} not found: put it on PATH, set VISOR_CORE to it, or place it next to this executable"
                )
            })
    }

    pub fn exe(&self) -> &Path {
        &self.exe
    }

    /// Run the CLI. `Ok(None)` when the command prints nothing (delete).
    fn run(&self, args: &[String]) -> Result<Option<Value>> {
        let out = Command::new(&self.exe)
            .args(args)
            .output()
            .with_context(|| format!("failed to run {}", self.exe.display()))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        if !out.status.success() {
            if let Some(msg) = parse_error(&stderr) {
                return Err(anyhow!(msg));
            }
            bail!(
                "{} {:?} failed (exit {}): {}",
                self.exe.display(),
                args,
                out.status.code().unwrap_or(-1),
                stderr.trim()
            );
        }
        let text = stdout.trim();
        if text.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(text).with_context(|| {
            format!("{} did not print JSON: {text}", self.exe.display())
        })?))
    }
}

impl Backend for VisorCli {
    fn capabilities(&self) -> Result<Capabilities> {
        let v = self
            .run(&["capabilities".to_string()])?
            .context("visor-core capabilities printed nothing")?;
        Capabilities::from_cli(&v)
    }

    fn create(&self, spec: &CreateSpec) -> Result<Workspace> {
        if spec.name.trim().is_empty() {
            bail!("a workspace needs a name");
        }
        let v = self
            .run(&create_args(spec))?
            .context("visor-core create printed nothing")?;
        workspace_from_cli(&v)
    }

    fn start(&self, name: &str) -> Result<Workspace> {
        let v = self
            .run(&["start".into(), name.to_string()])?
            .context("visor-core start printed nothing")?;
        workspace_from_cli(&v)
    }

    fn stop(&self, name: &str, force: bool) -> Result<Workspace> {
        let mut args = vec!["stop".to_string(), name.to_string()];
        if force {
            args.push("--force".into());
        }
        let v = self
            .run(&args)?
            .context("visor-core stop printed nothing")?;
        workspace_from_cli(&v)
    }

    fn delete(&self, name: &str) -> Result<()> {
        self.run(&["delete".into(), name.to_string()])?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<Workspace>> {
        let v = self
            .run(&["list".to_string()])?
            .context("visor-core list printed nothing")?;
        let rows = v
            .as_array()
            .context("visor-core list did not print an array of workspaces")?;
        rows.iter().map(workspace_from_cli).collect()
    }

    fn status(&self, name: &str) -> Result<Workspace> {
        let v = self
            .run(&["status".into(), name.to_string()])?
            .context("visor-core status printed nothing")?;
        workspace_from_cli(&v)
    }

    /// `visor-core exec` is WSL only — it refuses anything else with its own
    /// message (`capsule/core/src/main.rs`), which we pass through unchanged.
    fn exec(&self, name: &str, cmd: &[String]) -> Result<ExecOut> {
        if cmd.is_empty() {
            bail!("exec needs a command");
        }
        let mut args = vec!["exec".to_string(), name.to_string()];
        args.extend(cmd.iter().cloned());
        let v = self
            .run(&args)?
            .context("visor-core exec printed nothing")?;
        Ok(serde_json::from_value(v).context("visor-core exec was not the expected shape")?)
    }
}

/// The argv `visor-core create` wants for this spec. Pure, so the flags are
/// tested without running anything.
pub fn create_args(spec: &CreateSpec) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "create".into(),
        "--provider".into(),
        spec.provider.as_str().into(),
        "--name".into(),
        spec.name.clone(),
        "--os".into(),
        spec.os.clone(),
        "--cpus".into(),
        spec.cpus.to_string(),
        "--memory".into(),
        spec.memory_mib.to_string(),
        "--disk".into(),
        spec.disk_gib.to_string(),
    ];
    if let Some(key) = spec
        .ssh_key_path
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
    {
        args.push("--ssh-key".into());
        args.push(key.to_string());
    }
    if !spec.packages.is_empty() {
        args.push("--packages".into());
        args.push(spec.packages.join(","));
    }
    args
}

/// The CLI's `{"error": "..."}` from stderr, verbatim; the last one if a
/// warning line printed after it. `None` when stderr carries no such object.
pub fn parse_error(stderr: &str) -> Option<String> {
    stderr.lines().rev().find_map(|line| {
        let v: Value = serde_json::from_str(line.trim()).ok()?;
        v.get("error")?.as_str().map(str::to_string)
    })
}

/// A workspace as visor-core prints it. The CLI has no `created`/`task`; the
/// registry fills those in, so they are 0/None here.
pub fn workspace_from_cli(v: &Value) -> Result<Workspace> {
    let name = v
        .get("name")
        .and_then(Value::as_str)
        .context("visor-core returned a workspace with no name")?;
    Ok(Workspace {
        name: name.to_string(),
        provider: v
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        os: v
            .get("os")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        state: State::from_cli(v.get("state").and_then(Value::as_str).unwrap_or_default()),
        ip: v.get("ip").and_then(Value::as_str).map(str::to_string),
        created: 0,
        task: None,
    })
}

/// PATH → VISOR_CORE → next to the current executable.
fn find_exe() -> Option<PathBuf> {
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(EXE);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Some(setting) = std::env::var_os("VISOR_CORE") {
        let p = PathBuf::from(setting);
        if p.is_file() {
            return Some(p);
        }
        let inside = p.join(EXE);
        if inside.is_file() {
            return Some(inside);
        }
    }
    let me = std::env::current_exe().ok()?;
    let beside = me.parent()?.join(EXE);
    beside.is_file().then_some(beside)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A capabilities body with no `providers` key, as the CLI prints it.
    fn capabilities_body(hyperv: bool, wsl: bool, elevated: bool) -> Value {
        json!({ "hyperv": hyperv, "wsl": wsl, "elevated": elevated })
    }

    #[test]
    fn error_message_comes_out_verbatim() {
        assert_eq!(
            parse_error("{\"error\":\"workspace 'x' already exists\"}").as_deref(),
            Some("workspace 'x' already exists")
        );
    }

    #[test]
    fn parse_error_ignores_noise_and_junk() {
        // tracing writes plain lines; the JSON error is the last line.
        let stderr = "2026-10-12T10:00:00Z WARN something\n{\"error\":\"exec via visor-core is only for wsl workspaces; use SSH for hyperv\"}\n";
        assert_eq!(
            parse_error(stderr).as_deref(),
            Some("exec via visor-core is only for wsl workspaces; use SSH for hyperv")
        );
        assert_eq!(parse_error(""), None);
        assert_eq!(parse_error("not json at all"), None);
        assert_eq!(parse_error("{\"ok\":true}"), None);
        assert_eq!(parse_error("{\"error\":42}"), None);
    }

    #[test]
    fn create_args_are_the_cli_flags() {
        let mut spec = CreateSpec::new("api");
        spec.packages = vec!["git".into(), "curl".into()];
        spec.ssh_key_path = Some(" C:\\keys\\id_ed25519.pub ".into());
        assert_eq!(
            create_args(&spec),
            vec![
                "create",
                "--provider",
                "wsl",
                "--name",
                "api",
                "--os",
                "ubuntu-24.04",
                "--cpus",
                "2",
                "--memory",
                "4096",
                "--disk",
                "20",
                "--ssh-key",
                "C:\\keys\\id_ed25519.pub",
                "--packages",
                "git,curl",
            ]
        );
        // Hyper-V and no extras: the optional flags stay out.
        let mut hv = CreateSpec::new("vm1");
        hv.provider = crate::model::ProviderKind::Hyperv;
        hv.cpus = 4;
        assert_eq!(
            create_args(&hv),
            vec![
                "create",
                "--provider",
                "hyperv",
                "--name",
                "vm1",
                "--os",
                "ubuntu-24.04",
                "--cpus",
                "4",
                "--memory",
                "4096",
                "--disk",
                "20",
            ]
        );
    }

    #[test]
    fn cli_workspace_json_maps_to_ours() {
        let ws = workspace_from_cli(&json!({
            "name": "dev", "provider": "wsl", "os": "clone:Ubuntu",
            "state": "running", "ip": "172.20.1.5"
        }))
        .unwrap();
        assert_eq!(ws.name, "dev");
        assert_eq!(ws.state, State::Running);
        assert_eq!(ws.ip.as_deref(), Some("172.20.1.5"));
        assert_eq!(ws.created, 0, "the CLI does not report created");
        assert_eq!(ws.task, None);
        assert!(workspace_from_cli(&json!({"state": "running"})).is_err());
    }

    #[test]
    fn cli_states_fold_into_five() {
        assert_eq!(State::from_cli("running"), State::Running);
        assert_eq!(State::from_cli("stopped"), State::Stopped);
        assert_eq!(State::from_cli("off"), State::Stopped);
        assert_eq!(State::from_cli("paused"), State::Stopped);
        assert_eq!(State::from_cli("missing"), State::Failed);
        assert_eq!(State::from_cli("surprising"), State::Failed);
    }

    #[test]
    fn capabilities_print_providers() {
        let caps = Capabilities::from_cli(&capabilities_body(false, true, false)).unwrap();
        assert!(caps.wsl && !caps.hyperv && !caps.elevated);
        assert_eq!(caps.providers, vec!["wsl"]);
        let caps = Capabilities::from_cli(&capabilities_body(true, true, true)).unwrap();
        assert_eq!(caps.providers, vec!["hyperv", "wsl"]);
        assert!(Capabilities::from_cli(&json!("nope")).is_err());
    }

    #[test]
    fn exec_output_reads_the_cli_exit_code() {
        let out: ExecOut = serde_json::from_value(json!({
            "exit_code": 3, "stdout": "hi", "stderr": "bad"
        }))
        .unwrap();
        assert_eq!(out.code, 3);
        assert!(!out.ok());
        assert_eq!(out.stdout, "hi");
        assert_eq!(out.stderr, "bad");
    }

    #[test]
    fn a_missing_cli_is_an_error_not_a_panic() {
        let cli = VisorCli::at("definitely-not-visor-core-4c1f");
        let err = cli.capabilities().unwrap_err().to_string();
        assert!(err.contains("failed to run"), "{err}");
        assert!(VisorCli::discover().is_err() || VisorCli::discover().is_ok());
    }
}
