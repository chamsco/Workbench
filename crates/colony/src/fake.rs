//! An in-memory [`Backend`]: the same eight methods, no VM.
//!
//! Tests use it so the registry's whole lifecycle runs on any machine. It also
//! records every call, so a test can assert the argv that *would* have reached
//! `visor-core`. It deliberately answers like the CLI, not like a nicer API:
//! `create` reports `state: running`, and `created`/`task` come back empty
//! because the CLI does not report them.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{bail, Result};

use crate::backend::Backend;
use crate::model::{Capabilities, CreateSpec, ExecOut, State, Workspace};

/// A backend that only exists in memory.
#[derive(Clone, Default)]
pub struct FakeBackend {
    inner: Arc<Mutex<Fake>>,
}

#[derive(Default)]
struct Fake {
    workspaces: BTreeMap<String, Workspace>,
    calls: Vec<String>,
    execs: Vec<(String, Vec<String>)>,
    caps: Capabilities,
}

impl FakeBackend {
    /// A machine with WSL and no Hyper-V — the WSL-only case visor-core's
    /// `exec` supports.
    pub fn new() -> Self {
        FakeBackend::with_capabilities(Capabilities {
            hyperv: false,
            wsl: true,
            elevated: false,
            providers: vec!["wsl".into()],
        })
    }

    pub fn with_capabilities(caps: Capabilities) -> Self {
        FakeBackend {
            inner: Arc::new(Mutex::new(Fake {
                caps,
                ..Fake::default()
            })),
        }
    }

    /// Pretend a workspace already exists (as if another tool made it).
    pub fn seed(&self, ws: Workspace) {
        self.lock().workspaces.insert(ws.name.clone(), ws);
    }

    /// Every call, in order, as a readable line: `create dev`, `stop dev
    /// --force`, `list`.
    pub fn calls(&self) -> Vec<String> {
        self.lock().calls.clone()
    }

    /// Every `exec`, as (workspace, argv).
    pub fn execs(&self) -> Vec<(String, Vec<String>)> {
        self.lock().execs.clone()
    }

    fn lock(&self) -> MutexGuard<'_, Fake> {
        // A poisoned mutex means a previous test panicked; in a test backend
        // the recorded state is still what the assertions want.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Backend for FakeBackend {
    fn capabilities(&self) -> Result<Capabilities> {
        Ok(self.lock().caps.clone())
    }

    fn create(&self, spec: &CreateSpec) -> Result<Workspace> {
        let mut fake = self.lock();
        fake.calls.push(format!("create {}", spec.name));
        if fake.workspaces.contains_key(&spec.name) {
            bail!("workspace '{}' already exists", spec.name);
        }
        let ws = Workspace {
            name: spec.name.clone(),
            provider: spec.provider.as_str().into(),
            os: spec.os.clone(),
            state: State::Running,
            ip: None,
            created: 0,
            task: None,
        };
        fake.workspaces.insert(ws.name.clone(), ws.clone());
        Ok(ws)
    }

    fn start(&self, name: &str) -> Result<Workspace> {
        let mut fake = self.lock();
        fake.calls.push(format!("start {name}"));
        match fake.workspaces.get_mut(name) {
            Some(ws) => {
                ws.state = State::Running;
                Ok(ws.clone())
            }
            None => bail!("workspace '{name}' not found"),
        }
    }

    fn stop(&self, name: &str, force: bool) -> Result<Workspace> {
        let mut fake = self.lock();
        fake.calls.push(if force {
            format!("stop {name} --force")
        } else {
            format!("stop {name}")
        });
        match fake.workspaces.get_mut(name) {
            Some(ws) => {
                ws.state = State::Stopped;
                Ok(ws.clone())
            }
            None => bail!("workspace '{name}' not found"),
        }
    }

    fn delete(&self, name: &str) -> Result<()> {
        let mut fake = self.lock();
        fake.calls.push(format!("delete {name}"));
        if fake.workspaces.remove(name).is_none() {
            bail!("workspace '{name}' not found");
        }
        Ok(())
    }

    fn list(&self) -> Result<Vec<Workspace>> {
        let mut fake = self.lock();
        fake.calls.push("list".into());
        Ok(fake.workspaces.values().cloned().collect())
    }

    fn status(&self, name: &str) -> Result<Workspace> {
        let mut fake = self.lock();
        fake.calls.push(format!("status {name}"));
        match fake.workspaces.get(name) {
            Some(ws) => Ok(ws.clone()),
            None => bail!("workspace '{name}' not found"),
        }
    }

    fn exec(&self, name: &str, cmd: &[String]) -> Result<ExecOut> {
        let mut fake = self.lock();
        fake.calls.push(format!("exec {name} {}", cmd.join(" ")));
        if !fake.workspaces.contains_key(name) {
            bail!("workspace '{name}' not found");
        }
        if cmd.is_empty() {
            bail!("exec needs a command");
        }
        fake.execs.push((name.to_string(), cmd.to_vec()));
        Ok(ExecOut {
            code: 0,
            stdout: format!("{name}: {}", cmd.join(" ")),
            stderr: String::new(),
        })
    }
}

/// A WSL spec, as tests would write it.
#[cfg(test)]
fn sample_spec(name: &str) -> CreateSpec {
    CreateSpec::new(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_refuses_a_duplicate_like_the_cli() {
        let fake = FakeBackend::new();
        fake.create(&sample_spec("dev")).unwrap();
        assert!(fake.create(&sample_spec("dev")).is_err());
    }

    #[test]
    fn the_fake_reports_running_then_stopped() {
        let fake = FakeBackend::new();
        fake.create(&sample_spec("dev")).unwrap();
        assert_eq!(fake.status("dev").unwrap().state, State::Running);
        fake.stop("dev", false).unwrap();
        assert_eq!(fake.status("dev").unwrap().state, State::Stopped);
        assert_eq!(fake.list().unwrap().len(), 1);
        fake.delete("dev").unwrap();
        assert!(fake.list().unwrap().is_empty());
        assert!(fake.status("dev").is_err());
    }
}
