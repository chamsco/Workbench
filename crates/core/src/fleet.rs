//! The machines a shell can show: this one (the in-process harness) plus
//! any remote harnesses from prefs. The shell draws whichever is selected
//! through [`Backend`], and one change channel wakes it for all of them.

use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use serde_json::json;

use crate::files::FileEntry;
use crate::harness::Harness;
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
    local: Arc<Harness>,
    remotes: Mutex<Vec<Arc<Remote>>>,
    current: Mutex<usize>,
    prefs: Mutex<Prefs>,
    notify: async_channel::Sender<()>,
    changes: async_channel::Receiver<()>,
    serving: Mutex<Option<tokio::task::AbortHandle>>,
    share_error: Mutex<Option<String>>,
    rt: tokio::runtime::Runtime,
}

impl Fleet {
    pub fn new(local: Harness, prefs: Prefs) -> Result<Arc<Fleet>> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("fleet")
            .enable_all()
            .build()?;
        let (notify, changes) = async_channel::bounded(1);
        let local = Arc::new(local);
        // Local changes wake the shell through the shared channel too.
        {
            let rx = local.changes();
            let tx = notify.clone();
            rt.spawn(async move {
                while rx.recv().await.is_ok() {
                    let _ = tx.try_send(());
                }
            });
        }
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
            local,
            remotes: Mutex::new(remotes),
            current: Mutex::new(0),
            prefs: Mutex::new(prefs.clone()),
            notify,
            changes,
            serving: Mutex::new(None),
            share_error: Mutex::new(None),
            rt,
        });
        if prefs.share.enabled {
            if let Err(e) = fleet.start_sharing() {
                *fleet.share_error.lock().unwrap() = Some(e.to_string());
            }
        }
        Ok(fleet)
    }

    pub fn local(&self) -> &Arc<Harness> {
        &self.local
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
            0 => self.local.clone(),
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
            &self.local.snapshot(),
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
        let handle = self.local.serve(&share.addr, &share.token)?;
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
