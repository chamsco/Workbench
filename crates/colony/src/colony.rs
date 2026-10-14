//! The registry: one durable object per agent, persisted as `colony.json`.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::backend::{Backend, VisorCli};
use crate::model::{now_ms, Capabilities, CreateSpec, ExecOut, ProviderKind, Workspace};

/// The state file inside the colony directory.
pub const STATE_FILE: &str = "colony.json";

/// What `colony.json` holds. The provider is remembered so reopening a colony
/// created for Hyper-V does not silently default back to WSL.
#[derive(Debug, Serialize, Deserialize)]
struct Disk {
    #[serde(default)]
    provider: ProviderKind,
    #[serde(default)]
    workspaces: Vec<Workspace>,
}

/// A durable colony of workspaces.
///
/// `dir` is the durable object's home: `colony.json` lives in it and survives
/// restarts. Operations are recorded only after the backend agrees, so a
/// failed create does not leave a workspace in the registry that is not there.
pub struct Colony {
    /// Where `colony.json` lives.
    pub dir: PathBuf,
    /// Default provider for new workspaces (an existing `colony.json` wins).
    pub provider: ProviderKind,
    /// The hypervisor. [`VisorCli`] in production, `FakeBackend` in tests.
    pub backend: Box<dyn Backend>,
    workspaces: Vec<Workspace>,
}

impl Colony {
    /// Open the colony in `dir` and drive the real `visor-core` CLI.
    ///
    /// Fails when `visor-core` cannot be found — use [`Colony::with_backend`]
    /// when there is no hypervisor (tests, or another host).
    pub fn open(dir: impl Into<PathBuf>) -> Result<Colony> {
        Self::with_backend(dir, ProviderKind::default(), VisorCli::discover()?)
    }

    /// Open the colony in `dir` over a caller-supplied backend. `provider` is
    /// the default for new workspaces; a provider already in `colony.json`
    /// stays.
    pub fn with_backend(
        dir: impl Into<PathBuf>,
        provider: ProviderKind,
        backend: impl Backend + 'static,
    ) -> Result<Colony> {
        let mut colony = Colony {
            dir: dir.into(),
            provider,
            backend: Box::new(backend),
            workspaces: Vec::new(),
        };
        colony.load()?;
        Ok(colony)
    }

    /// The state file's path.
    pub fn path(&self) -> PathBuf {
        self.dir.join(STATE_FILE)
    }

    /// What the registry knows without asking the backend.
    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    /// One workspace, or `None`. Never panics; see [`Colony::get_or_err`].
    pub fn get(&self, name: &str) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.name == name)
    }

    /// The host's capabilities, straight from the backend.
    pub fn capabilities(&self) -> Result<Capabilities> {
        self.backend.capabilities()
    }

    /// Ask the backend what exists and adopt it: the backend is the truth, and
    /// `created`/`task` (colony's own fields) are carried over by name.
    pub fn list(&mut self) -> Result<Vec<Workspace>> {
        let live = self.backend.list()?;
        let mut out = Vec::with_capacity(live.len());
        for mut ws in live {
            if let Some(old) = self.workspaces.iter().find(|o| o.name == ws.name) {
                ws.created = old.created;
                if ws.task.is_none() {
                    ws.task = old.task.clone();
                }
            }
            out.push(ws);
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        self.workspaces = out;
        self.save()?;
        Ok(self.workspaces.clone())
    }

    /// Create a workspace and register it. A name already in the registry is
    /// refused before the backend is touched.
    pub fn create(&mut self, spec: &CreateSpec) -> Result<Workspace> {
        if spec.name.trim().is_empty() {
            bail!("a workspace needs a name");
        }
        if self.get(&spec.name).is_some() {
            bail!("workspace '{}' already exists in this colony", spec.name);
        }
        let mut ws = self.backend.create(spec)?;
        ws.created = now_ms();
        if ws.task.is_none() {
            ws.task = spec.task.clone();
        }
        self.workspaces.push(ws.clone());
        self.save()?;
        Ok(ws)
    }

    pub fn start(&mut self, name: &str) -> Result<Workspace> {
        self.get_or_err(name)?;
        let ws = self.backend.start(name)?;
        self.put(ws)
    }

    pub fn stop(&mut self, name: &str, force: bool) -> Result<Workspace> {
        self.get_or_err(name)?;
        let ws = self.backend.stop(name, force)?;
        self.put(ws)
    }

    /// Ask the backend for one workspace's current state and record it.
    pub fn status(&mut self, name: &str) -> Result<Workspace> {
        self.get_or_err(name)?;
        let ws = self.backend.status(name)?;
        self.put(ws)
    }

    /// Run a command in a workspace. `visor-core` supports WSL workspaces
    /// only; its refusal for anything else comes back verbatim.
    pub fn exec(&mut self, name: &str, cmd: &[String]) -> Result<ExecOut> {
        self.get_or_err(name)?;
        self.backend.exec(name, cmd)
    }

    /// Delete a workspace and forget it. The entry is only removed when the
    /// backend succeeded.
    pub fn delete(&mut self, name: &str) -> Result<()> {
        self.get_or_err(name)?;
        self.backend.delete(name)?;
        self.workspaces.retain(|w| w.name != name);
        self.save()?;
        Ok(())
    }

    /// Read `colony.json`. A missing file is an empty colony, not an error.
    pub fn load(&mut self) -> Result<()> {
        let path = self.path();
        if !path.exists() {
            self.workspaces.clear();
            return Ok(());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let disk: Disk =
            serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        self.provider = disk.provider;
        self.workspaces = disk.workspaces;
        Ok(())
    }

    /// Write `colony.json` (pretty JSON; it is meant to be read by humans and
    /// diffed in git).
    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("creating {}", self.dir.display()))?;
        let disk = Disk {
            provider: self.provider,
            workspaces: self.workspaces.clone(),
        };
        let text = serde_json::to_string_pretty(&disk)?;
        std::fs::write(self.path(), text)
            .with_context(|| format!("writing {}", self.path().display()))?;
        Ok(())
    }

    /// A workspace that is in this colony, or a plain error. Every operation
    /// goes through this, so an unknown name is an error rather than a panic.
    fn get_or_err(&self, name: &str) -> Result<&Workspace> {
        self.get(name).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown workspace '{name}': it is not in {} (list() adopts what the backend reports)",
                self.path().display()
            )
        })
    }

    /// Record what a backend returned, keeping colony's own `created`/`task`,
    /// and persist.
    fn put(&mut self, mut ws: Workspace) -> Result<Workspace> {
        match self.workspaces.iter_mut().find(|o| o.name == ws.name) {
            Some(old) => {
                if ws.created == 0 {
                    ws.created = old.created;
                }
                if ws.task.is_none() {
                    ws.task = old.task.clone();
                }
                *old = ws.clone();
            }
            None => self.workspaces.push(ws.clone()),
        }
        self.save()?;
        Ok(ws)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeBackend;
    use crate::model::State;

    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("colony-test-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn lifecycle_create_list_status_stop_start_delete() {
        let dir = tmp("lifecycle");
        let fake = FakeBackend::new();
        let mut colony = Colony::with_backend(&dir, ProviderKind::Wsl, fake.clone()).unwrap();

        let ws = colony.create(&CreateSpec::new("dev")).unwrap();
        assert_eq!(ws.state, State::Running);
        assert!(ws.created > 0, "colony stamps created");
        assert_eq!(fake.calls(), vec!["create dev"]);

        let listed = colony.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "dev");
        assert_eq!(listed[0].created, ws.created, "created survives a list");

        assert_eq!(colony.status("dev").unwrap().state, State::Running);
        assert_eq!(colony.stop("dev", false).unwrap().state, State::Stopped);
        assert_eq!(colony.start("dev").unwrap().state, State::Running);

        colony.delete("dev").unwrap();
        assert!(colony.get("dev").is_none(), "delete forgets the workspace");
        assert!(colony.list().unwrap().is_empty());
        assert_eq!(
            fake.calls(),
            vec![
                "create dev",
                "list",
                "status dev",
                "stop dev",
                "start dev",
                "delete dev",
                "list",
            ]
        );
        assert!(colony.path().exists(), "colony.json was written");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn duplicate_name_is_refused_before_the_backend() {
        let dir = tmp("duplicate");
        let fake = FakeBackend::new();
        let mut colony = Colony::with_backend(&dir, ProviderKind::Wsl, fake.clone()).unwrap();
        colony.create(&CreateSpec::new("dev")).unwrap();
        let err = colony
            .create(&CreateSpec::new("dev"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(fake.calls(), vec!["create dev"], "no second backend create");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn exec_records_argv_and_returns_the_output() {
        let dir = tmp("exec");
        let fake = FakeBackend::new();
        let mut colony = Colony::with_backend(&dir, ProviderKind::Wsl, fake.clone()).unwrap();
        colony.create(&CreateSpec::new("dev")).unwrap();

        let argv: Vec<String> = ["bash", "-lc", "uname -a"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let out = colony.exec("dev", &argv).unwrap();
        assert_eq!(out.code, 0);
        assert!(out.ok());
        assert!(out.stdout.contains("bash -lc uname -a"), "{}", out.stdout);
        assert_eq!(fake.execs(), vec![("dev".to_string(), argv)]);
        assert!(colony.exec("dev", &[]).is_err(), "empty argv is refused");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stop_force_reaches_the_backend() {
        let dir = tmp("force");
        let fake = FakeBackend::new();
        let mut colony = Colony::with_backend(&dir, ProviderKind::Wsl, fake.clone()).unwrap();
        colony.create(&CreateSpec::new("dev")).unwrap();
        colony.stop("dev", true).unwrap();
        assert!(fake.calls().contains(&"stop dev --force".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_workspace_is_an_error_not_a_panic() {
        let dir = tmp("unknown");
        let fake = FakeBackend::new();
        let mut colony = Colony::with_backend(&dir, ProviderKind::Wsl, fake.clone()).unwrap();

        assert!(colony.get("nope").is_none());
        for err in [
            colony.start("nope").unwrap_err(),
            colony.stop("nope", false).unwrap_err(),
            colony.status("nope").unwrap_err(),
            colony.delete("nope").unwrap_err(),
            colony.exec("nope", &["true".to_string()]).unwrap_err(),
        ] {
            assert!(
                err.to_string().contains("unknown workspace 'nope'"),
                "{err}"
            );
        }
        assert!(fake.calls().is_empty(), "the backend was never asked");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn task_and_created_survive_a_backend_round_trip() {
        let dir = tmp("task");
        let fake = FakeBackend::new();
        let mut colony = Colony::with_backend(&dir, ProviderKind::Wsl, fake.clone()).unwrap();
        let mut spec = CreateSpec::new("api");
        spec.task = Some("run the API tests".into());
        let created = colony.create(&spec).unwrap().created;

        // The CLI never reports task; colony keeps its own.
        let status = colony.status("api").unwrap();
        assert_eq!(status.task.as_deref(), Some("run the API tests"));
        assert_eq!(status.created, created);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn colony_json_round_trips() {
        let dir = tmp("persist");
        let fake = FakeBackend::new();
        let mut first = Colony::with_backend(&dir, ProviderKind::Hyperv, fake.clone()).unwrap();
        let mut spec = CreateSpec::new("vm1");
        spec.provider = ProviderKind::Hyperv;
        spec.task = Some("windows build".into());
        let ws = first.create(&spec).unwrap();
        drop(first);

        // A fresh colony over the same directory, with an empty backend.
        let second = Colony::with_backend(&dir, ProviderKind::Wsl, FakeBackend::new()).unwrap();
        assert_eq!(
            second.provider,
            ProviderKind::Hyperv,
            "provider is remembered"
        );
        let got = second.get("vm1").expect("reloaded from colony.json");
        assert_eq!(got, &ws);
        assert_eq!(got.state, State::Running);
        assert_eq!(got.task.as_deref(), Some("windows build"));

        let text = std::fs::read_to_string(second.path()).unwrap();
        assert!(text.contains("\"vm1\""), "{text}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_adopts_what_the_backend_reports() {
        let dir = tmp("adopt");
        let fake = FakeBackend::new();
        fake.seed(Workspace {
            name: "pre-existing".into(),
            provider: "wsl".into(),
            os: "clone:Ubuntu".into(),
            state: State::Stopped,
            ip: None,
            created: 7,
            task: None,
        });
        let mut colony = Colony::with_backend(&dir, ProviderKind::Wsl, fake.clone()).unwrap();
        assert!(colony.get("pre-existing").is_none(), "not known yet");
        let listed = colony.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(colony.get("pre-existing").unwrap().state, State::Stopped);
        assert_eq!(
            colony.get("pre-existing").unwrap().created,
            7,
            "seed's own stamp is kept when it is not adopted"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_state_file_is_an_empty_colony() {
        let dir = tmp("empty");
        let colony = Colony::with_backend(&dir, ProviderKind::Wsl, FakeBackend::new()).unwrap();
        assert!(colony.workspaces().is_empty());
        assert!(!colony.path().exists());
        assert!(colony.get("anything").is_none());
        assert!(!dir.exists(), "open does not create the directory");
    }

    #[test]
    fn a_corrupt_state_file_is_an_error() {
        let dir = tmp("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(STATE_FILE), "{ not json").unwrap();
        let err = Colony::with_backend(&dir, ProviderKind::Wsl, FakeBackend::new())
            .err()
            .expect("corrupt colony.json must fail")
            .to_string();
        assert!(err.contains("parsing"), "{err}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
