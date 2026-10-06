//! The machines a shell can show: this one (the in-process harness) plus
//! any remote harnesses from prefs. The shell draws whichever is selected
//! through [`Backend`], and one change channel wakes it for all of them.

use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use serde_json::json;

use crate::chat::{Chats, Ctx};
use crate::cloud::{Account, Overage, Plan};
use crate::files::FileEntry;
use crate::harness::Harness;
use crate::harnesses::HarnessInfo;
use crate::prefs::{MachineCfg, Prefs};
use crate::project::{AgentStatus, ProjectState};
use crate::remote::{Link, Remote};
use crate::update::UpdateInfo;

/// Everything a shell does with a machine.
pub trait Backend: Send + Sync {
    fn snapshot(&self) -> ProjectState;
    fn send(&self, text: String);
    fn approve(&self, id: usize);
    fn reject(&self, id: usize, feedback: String);
    fn file_ticket(&self, title: &str, body: &str) -> Result<String>;
    fn list_files(&self, agent: usize) -> Vec<FileEntry>;
    fn read_file(&self, path: &str) -> Result<String>;
    /// Post on the agents' message board as the human.
    fn post_message(&self, to: &str, text: &str) -> Result<()>;
}

impl Backend for Harness {
    fn snapshot(&self) -> ProjectState {
        Harness::snapshot(self)
    }
    fn send(&self, text: String) {
        Harness::send(self, text)
    }
    fn approve(&self, id: usize) {
        Harness::approve(self, id)
    }
    fn reject(&self, id: usize, feedback: String) {
        Harness::reject(self, id, feedback)
    }
    fn file_ticket(&self, title: &str, body: &str) -> Result<String> {
        Harness::file_ticket(self, title, body)
    }
    fn list_files(&self, agent: usize) -> Vec<FileEntry> {
        Harness::list_files(self, agent)
    }
    fn read_file(&self, path: &str) -> Result<String> {
        Harness::read_file(self, path)
    }
    fn post_message(&self, to: &str, text: &str) -> Result<()> {
        Harness::post_message(self, to, text)
    }
}

/// This machine with no project open: everything empty, nothing to send to.
struct NoProject;

impl Backend for NoProject {
    fn snapshot(&self) -> ProjectState {
        ProjectState::empty("")
    }
    fn send(&self, _: String) {}
    fn approve(&self, _: usize) {}
    fn reject(&self, _: usize, _: String) {}
    fn file_ticket(&self, _: &str, _: &str) -> Result<String> {
        bail!("open a project first")
    }
    fn list_files(&self, _: usize) -> Vec<FileEntry> {
        vec![]
    }
    fn read_file(&self, _: &str) -> Result<String> {
        bail!("open a project first")
    }
    fn post_message(&self, _: &str, _: &str) -> Result<()> {
        bail!("open a project first")
    }
}

impl Backend for Remote {
    /// Until the first answer arrives: an empty project named after the
    /// machine, so shells can render "connecting" without special cases.
    fn snapshot(&self) -> ProjectState {
        self.snapshot()
            .unwrap_or_else(|| ProjectState::empty(&self.name))
    }
    fn send(&self, text: String) {
        self.post_bg("/v1/send", json!({ "text": text }));
    }
    fn approve(&self, id: usize) {
        self.post_bg("/v1/approve", json!({ "id": id }));
    }
    fn reject(&self, id: usize, feedback: String) {
        self.post_bg("/v1/reject", json!({ "id": id, "feedback": feedback }));
    }
    fn file_ticket(&self, title: &str, body: &str) -> Result<String> {
        let v = self.post_wait("/v1/ticket", json!({ "title": title, "body": body }))?;
        Ok(v["key"].as_str().unwrap_or("").to_string())
    }
    fn list_files(&self, agent: usize) -> Vec<FileEntry> {
        self.post_wait("/v1/files", json!({ "agent": agent }))
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }
    fn read_file(&self, path: &str) -> Result<String> {
        let v = self.post_wait("/v1/file", json!({ "path": path }))?;
        Ok(v["text"].as_str().unwrap_or("").to_string())
    }
    fn post_message(&self, to: &str, text: &str) -> Result<()> {
        self.post_wait("/v1/message", json!({ "to": to, "text": text }))?;
        Ok(())
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct MachineInfo {
    pub index: usize,
    pub name: String,
    /// None for this machine.
    pub url: Option<String>,
    pub local: bool,
    pub selected: bool,
    pub link: Link,
    pub project: String,
    pub running: usize,
    pub pending: usize,
}

pub struct Fleet {
    /// The project open on this machine, if any.
    local: Mutex<Option<Arc<Harness>>>,
    chats: Arc<Chats>,
    memory: Arc<crate::memory::Memory>,
    apps: Arc<crate::apps::Apps>,
    harnesses: Mutex<Vec<HarnessInfo>>,
    http: reqwest::Client,
    remotes: Mutex<Vec<Arc<Remote>>>,
    current: Mutex<usize>,
    prefs: Mutex<Prefs>,
    notify: async_channel::Sender<()>,
    changes: async_channel::Receiver<()>,
    serving: Mutex<Option<tokio::task::AbortHandle>>,
    companion: Mutex<Option<tokio::task::AbortHandle>>,
    companion_error: Mutex<Option<String>>,
    share_error: Mutex<Option<String>>,
    rt: tokio::runtime::Runtime,
}

impl Fleet {
    pub fn new(local: Option<Harness>, prefs: Prefs) -> Result<Arc<Fleet>> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("fleet")
            .enable_all()
            .build()?;
        let (notify, changes) = async_channel::bounded(1);
        let local = local.map(Arc::new);
        if let Some(h) = &local {
            forward(&rt, h, &notify);
        }
        let chats = Chats::open(Prefs::data_dir(), rt.handle().clone(), notify.clone());
        let remotes = prefs
            .machines
            .iter()
            .map(|m| {
                Remote::connect(
                    m.name.clone(),
                    m.url.clone(),
                    m.token.clone(),
                    rt.handle().clone(),
                    notify.clone(),
                )
            })
            .collect();
        let fleet = Arc::new(Fleet {
            local: Mutex::new(local),
            chats,
            memory: Arc::new(crate::memory::Memory::open(Prefs::data_dir())),
            apps: Arc::new(crate::apps::Apps::open(Prefs::data_dir().join("apps"))),
            harnesses: Mutex::new(Vec::new()),
            http: reqwest::Client::new(),
            remotes: Mutex::new(remotes),
            current: Mutex::new(0),
            prefs: Mutex::new(prefs.clone()),
            notify,
            changes,
            serving: Mutex::new(None),
            companion: Mutex::new(None),
            companion_error: Mutex::new(None),
            share_error: Mutex::new(None),
            rt,
        });
        if prefs.companion.enabled {
            fleet.start_companion();
        }
        if prefs.share.enabled {
            if let Err(e) = fleet.start_sharing() {
                *fleet.share_error.lock().unwrap() = Some(e.to_string());
            }
        }
        Ok(fleet)
    }

    pub fn local(&self) -> Option<Arc<Harness>> {
        self.local.lock().unwrap().clone()
    }

    pub fn chats(&self) -> &Arc<Chats> {
        &self.chats
    }

    pub fn memory(&self) -> &Arc<crate::memory::Memory> {
        &self.memory
    }

    pub fn apps(&self) -> &Arc<crate::apps::Apps> {
        &self.apps
    }

    // ------------------------------------------------------------ companion

    fn start_companion(self: &Arc<Self>) {
        if let Some(h) = self.companion.lock().unwrap().take() {
            h.abort();
        }
        let c = self.prefs().companion;
        let weak = Arc::downgrade(self);
        let me = Arc::downgrade(self);
        *self.companion_error.lock().unwrap() = None;
        let task = self.rt.spawn(async move {
            if let Err(e) = crate::companion::serve(weak, c.addr, c.token).await {
                if let Some(f) = me.upgrade() {
                    *f.companion_error.lock().unwrap() = Some(e.to_string());
                    let _ = f.notify.try_send(());
                }
            }
        });
        *self.companion.lock().unwrap() = Some(task.abort_handle());
    }

    /// Turn the phone link on or off; `new_token` unpairs every phone.
    pub fn set_companion(self: &Arc<Self>, enabled: bool, new_token: bool) {
        self.update_prefs(|p| {
            p.companion.enabled = enabled;
            if new_token {
                p.companion.token = crate::remote::new_token();
            }
        });
        if enabled {
            self.start_companion();
        } else if let Some(h) = self.companion.lock().unwrap().take() {
            h.abort();
        }
    }

    /// What Settings shows: on or off, the address a phone uses, the
    /// pairing link for the QR code, and any error binding the port.
    pub fn companion_status(&self) -> serde_json::Value {
        let c = self.prefs().companion;
        let port = c.addr.rsplit(':').next().unwrap_or("7421").to_string();
        let ip = crate::companion::lan_ip();
        let url = ip.map(|ip| format!("http://{ip}:{port}"));
        let name = std::env::var("HOSTNAME")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Backspace".into());
        serde_json::json!({
            "enabled": c.enabled,
            "running": c.enabled && self.companion.lock().unwrap().is_some() && self.companion_error.lock().unwrap().is_none(),
            "addr": c.addr,
            "url": url,
            "pair": url.as_deref().map(|u| crate::companion::pair_link(u, &c.token, &name)),
            "error": self.companion_error.lock().unwrap().clone(),
            "protocol": crate::companion::PROTOCOL,
        })
    }

    /// Download and install an app from its manifest URL.
    pub fn install_app_url(&self, url: &str) -> Result<crate::apps::Installed> {
        self.rt.block_on(self.apps.install_url(&self.http, url))
    }

    /// What a chat send needs from prefs.
    pub fn chat_ctx(&self) -> Ctx {
        Ctx {
            prefs: self.prefs(),
            memory: Some(self.memory.clone()),
        }
    }

    /// Open a project folder on this machine (closing the one open, which
    /// stops its agents), remember it, and show it.
    pub fn open_project(&self, path: &str) -> Result<()> {
        let p = std::path::PathBuf::from(path.trim());
        if !p.is_dir() {
            bail!("{} is not a folder", p.display());
        }
        let prefs = self.prefs();
        let canon = p.canonicalize().unwrap_or_else(|_| p.clone());
        let memory = prefs
            .memory_on
            .then(|| self.memory.context(Some(&canon.display().to_string())))
            .flatten();
        let h = Harness::open_with(
            p,
            crate::harness::Overrides {
                worker: prefs.worker,
                memory,
            },
        )?;
        let root = h.root().display().to_string();
        forward(&self.rt, &h, &self.notify);
        let old = self.local.lock().unwrap().replace(Arc::new(h));
        // A harness owns a runtime, which must not be dropped on one.
        if let Some(old) = old {
            std::thread::spawn(move || drop(old));
        }
        self.update_prefs(|pr| pr.touch_project(&root));
        let sharing = self.sharing();
        if sharing {
            let _ = self.start_sharing();
        }
        self.select(0);
        Ok(())
    }

    pub fn close_project(&self) {
        if let Some(h) = self.serving.lock().unwrap().take() {
            h.abort();
        }
        if let Some(old) = self.local.lock().unwrap().take() {
            std::thread::spawn(move || drop(old));
        }
        self.poke();
    }

    pub fn forget_project(&self, path: &str) {
        self.update_prefs(|p| p.projects.retain(|x| x != path));
        self.poke();
    }

    // ------------------------------------------------------------ harnesses

    /// The last scan, with the current switches applied.
    pub fn harnesses(&self) -> Vec<HarnessInfo> {
        let mut v = self.harnesses.lock().unwrap().clone();
        crate::harnesses::apply_toggles(&mut v, &self.prefs().harnesses);
        v
    }

    /// Probe every CLI, Ollama and router again. Blocking, a few seconds.
    pub fn rescan(&self) -> Vec<HarnessInfo> {
        let v = crate::harnesses::scan(&self.prefs());
        *self.harnesses.lock().unwrap() = v.clone();
        self.poke();
        v
    }

    pub fn set_harness(&self, id: &str, on: bool) -> Vec<HarnessInfo> {
        self.update_prefs(|p| {
            p.harnesses.insert(id.to_string(), on);
        });
        self.poke();
        self.harnesses()
    }

    // ------------------------------------------------------------ cloud

    fn block<T: Send>(&self, f: impl std::future::Future<Output = T> + Send) -> T {
        std::thread::scope(|s| s.spawn(|| self.rt.block_on(f)).join().unwrap())
    }

    /// Create a Free account on this machine (no email in the dev server).
    pub fn cloud_signup(&self) -> Result<Account> {
        let url = self.prefs().cloud.url;
        let (token, acct) = self.block(crate::cloud::signup(&self.http, &url))?;
        self.update_prefs(|p| p.cloud.token = token);
        self.chats.set_account(Some(acct.clone()));
        Ok(acct)
    }

    pub fn cloud_account(&self) -> Result<Option<Account>> {
        let c = self.prefs().cloud;
        if c.token.is_empty() {
            return Ok(None);
        }
        let a = self.block(crate::cloud::account(&self.http, &c.url, &c.token))?;
        self.chats.set_account(Some(a.clone()));
        Ok(Some(a))
    }

    pub fn cloud_set_plan(&self, plan: Plan, overage: Overage) -> Result<Account> {
        let c = self.prefs().cloud;
        if c.token.is_empty() {
            bail!("sign up first");
        }
        let a = self.block(crate::cloud::set_plan(
            &self.http, &c.url, &c.token, plan, overage,
        ))?;
        self.chats.set_account(Some(a.clone()));
        Ok(a)
    }

    pub fn cloud_sign_out(&self) {
        self.update_prefs(|p| p.cloud.token.clear());
        self.chats.set_account(None);
    }

    pub fn link_preview(&self, url: &str) -> Result<crate::chat::LinkPreview> {
        self.block(crate::chat::link_preview(&self.http, url))
    }

    /// Fires (coalesced) when any machine's state or link changes.
    pub fn changes(&self) -> async_channel::Receiver<()> {
        self.changes.clone()
    }

    fn poke(&self) {
        let _ = self.notify.try_send(());
    }

    pub fn current(&self) -> usize {
        *self.current.lock().unwrap()
    }

    pub fn select(&self, index: usize) {
        let n = self.remotes.lock().unwrap().len() + 1;
        *self.current.lock().unwrap() = index.min(n - 1);
        self.poke();
    }

    pub fn backend(&self) -> Arc<dyn Backend> {
        match self.current() {
            0 => match self.local() {
                Some(h) => h,
                None => Arc::new(NoProject),
            },
            i => self.remotes.lock().unwrap()[i - 1].clone(),
        }
    }

    pub fn snapshot(&self) -> ProjectState {
        self.backend().snapshot()
    }

    pub fn machines(&self) -> Vec<MachineInfo> {
        let cur = self.current();
        let info =
            |index: usize, name: String, url: Option<String>, link: Link, s: &ProjectState| {
                MachineInfo {
                    index,
                    local: url.is_none(),
                    name,
                    url,
                    selected: index == cur,
                    link,
                    project: s.name.clone(),
                    running: s
                        .agents
                        .iter()
                        .filter(|a| a.status == AgentStatus::Running)
                        .count(),
                    pending: s.pending_approvals().count(),
                }
            };
        let mut out = vec![info(
            0,
            "Local".into(),
            None,
            Link::Online,
            &self
                .local()
                .map_or_else(|| ProjectState::empty(""), |h| h.snapshot()),
        )];
        for (i, r) in self.remotes.lock().unwrap().iter().enumerate() {
            out.push(info(
                i + 1,
                r.name.clone(),
                Some(r.url.clone()),
                r.link(),
                &Backend::snapshot(&**r),
            ));
        }
        out
    }

    pub fn prefs(&self) -> Prefs {
        self.prefs.lock().unwrap().clone()
    }

    /// Change and save prefs (tabs, theme, update checks).
    pub fn update_prefs(&self, f: impl FnOnce(&mut Prefs)) {
        let mut p = self.prefs.lock().unwrap();
        f(&mut p);
        p.save();
    }

    /// Check the machine answers with this token, then add and select it.
    pub fn add_machine(&self, name: &str, url: &str, token: &str) -> Result<usize> {
        let name = name.trim();
        let mut url = url.trim().trim_end_matches('/').to_string();
        if name.is_empty() || url.is_empty() || token.trim().is_empty() {
            bail!("name, address and token are all needed");
        }
        if !url.contains("://") {
            url = format!("http://{url}");
        }
        let probe = crate::remote::probe(&url, token.trim());
        std::thread::scope(|s| s.spawn(|| self.rt.block_on(probe)).join())
            .map_err(|_| anyhow!("probe panicked"))??;
        let cfg = MachineCfg {
            name: name.into(),
            url: url.clone(),
            token: token.trim().into(),
        };
        self.update_prefs(|p| p.machines.push(cfg));
        let r = Remote::connect(
            name.into(),
            url,
            token.trim().into(),
            self.rt.handle().clone(),
            self.notify.clone(),
        );
        let index = {
            let mut rs = self.remotes.lock().unwrap();
            rs.push(r);
            rs.len()
        };
        self.select(index);
        Ok(index)
    }

    pub fn remove_machine(&self, index: usize) {
        if index == 0 {
            return;
        }
        {
            let mut rs = self.remotes.lock().unwrap();
            if index > rs.len() {
                return;
            }
            rs.remove(index - 1);
        }
        self.update_prefs(|p| {
            if index - 1 < p.machines.len() {
                p.machines.remove(index - 1);
            }
        });
        let cur = self.current();
        if cur == index {
            self.select(0);
        } else if cur > index {
            self.select(cur - 1);
        } else {
            self.poke();
        }
    }

    fn start_sharing(&self) -> Result<()> {
        let share = self.prefs().share;
        if let Some(h) = self.serving.lock().unwrap().take() {
            h.abort();
        }
        let local = self
            .local()
            .ok_or_else(|| anyhow!("open a project to share it"))?;
        let handle = local.serve(&share.addr, &share.token)?;
        *self.serving.lock().unwrap() = Some(handle);
        Ok(())
    }

    /// Turn sharing on or off, optionally with a new address or token.
    pub fn set_share(&self, enabled: bool, addr: Option<&str>, new_token: bool) -> Result<()> {
        self.update_prefs(|p| {
            p.share.enabled = enabled;
            if let Some(a) = addr.filter(|a| !a.trim().is_empty()) {
                p.share.addr = a.trim().to_string();
            }
            if new_token {
                p.share.token = crate::remote::new_token();
            }
        });
        *self.share_error.lock().unwrap() = None;
        let res = if enabled {
            self.start_sharing()
        } else {
            if let Some(h) = self.serving.lock().unwrap().take() {
                h.abort();
            }
            Ok(())
        };
        if let Err(e) = &res {
            *self.share_error.lock().unwrap() = Some(e.to_string());
            self.update_prefs(|p| p.share.enabled = false);
        }
        self.poke();
        res
    }

    pub fn sharing(&self) -> bool {
        self.serving.lock().unwrap().is_some()
    }

    pub fn share_error(&self) -> Option<String> {
        self.share_error.lock().unwrap().clone()
    }

    /// Blocking; call off the UI thread or accept a few seconds' wait.
    pub fn check_update(&self) -> Result<UpdateInfo> {
        let http = reqwest::Client::new();
        let fut = async move { crate::update::check(&http).await };
        std::thread::scope(|s| s.spawn(|| self.rt.block_on(fut)).join())
            .map_err(|_| anyhow!("update check panicked"))?
    }
}

/// A harness's changes wake the shell through the shared channel. Ends when
/// the harness is dropped.
fn forward(rt: &tokio::runtime::Runtime, h: &Harness, notify: &async_channel::Sender<()>) {
    let rx = h.changes();
    let tx = notify.clone();
    rt.spawn(async move {
        while rx.recv().await.is_ok() {
            let _ = tx.try_send(());
        }
    });
}
