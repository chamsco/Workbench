//! Folders: a way to file streams. A person names them (or, later, accepts a
//! name the Librarian proposes); a stream is in at most one, and a stream in
//! none is fine.
//!
//! A folder is filing and nothing else. Where a stream runs is its
//! `Thread.project`, and filing never reads or writes it: moving a stream to a
//! folder must not change the directory its CLI works in. The folder a stream
//! is in is `Thread.folder` (a folder id), written into the thread's own file
//! by [`Chats`]; the folders themselves, and the proposals waiting for an
//! answer, are one registry in `<data>/folders.json`, the way
//! [`crate::projects`] keeps `agent-projects.json`.
//!
//! Counts and lists are always read off the live threads, never stored, so
//! deleting a stream or a restart cannot leave them wrong. A stream whose
//! folder id is in no registry (the file was lost) counts as unfiled.
//!
//! [`FolderProposal`] is only data: this module can keep, accept and reject
//! one, and has no way to make one up.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

use crate::chat::{new_id, now_ms, Chats, ThreadInfo};

/// Folder colours, handed out in creation order and wrapping.
pub const PALETTE: [&str; 6] = ["#8b5cf6", "#3fb27f", "#e0a32a", "#5b8cf0", "#d9669a", "#38c0e8"];

/// The longest folder name, in characters; a longer one is cut here.
pub const NAME_MAX: usize = 60;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Folder {
    pub id: String,
    pub name: String,
    /// A `#rrggbb` from [`PALETTE`].
    pub color: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub updated: u64,
}

/// Where a proposal would put a stream.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ProposalTarget {
    /// A folder that exists, by id.
    Existing(String),
    /// A folder that does not exist yet, by the name it would get.
    New(String),
}

/// A suggestion to file one stream, waiting for a yes or a no. Nothing changes
/// until [`Folders::accept`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FolderProposal {
    pub thread_id: String,
    pub target: ProposalTarget,
    /// Why, in a line, for whoever is asked.
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub created: u64,
}

/// One folder's size, for the sidebar.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FolderCount {
    pub folder_id: String,
    pub streams: usize,
    /// The newest `updated` among its streams; 0 for an empty folder.
    pub last_activity: u64,
}

/// What is on disk.
#[derive(Serialize, Deserialize, Default)]
struct Registry {
    /// In creation order, which is the order they are listed in.
    #[serde(default)]
    folders: Vec<Folder>,
    /// How many folders have ever been made, so the next colour carries on
    /// the palette even after a folder is deleted.
    #[serde(default)]
    made: usize,
    /// At most one per stream, oldest first.
    #[serde(default)]
    proposals: Vec<FolderProposal>,
}

pub struct Folders {
    path: PathBuf,
    chats: Arc<Chats>,
    lock: Mutex<()>,
}

impl Folders {
    /// The registry lives in the data folder, next to the threads `chats` holds.
    pub fn open(data_dir: PathBuf, chats: Arc<Chats>) -> Self {
        Self { path: data_dir.join("folders.json"), chats, lock: Mutex::new(()) }
    }

    fn guard(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn load(&self) -> Registry {
        let Ok(b) = std::fs::read(&self.path) else { return Registry::default() };
        serde_json::from_slice(&b).unwrap_or_default()
    }

    fn write(&self, r: &Registry) -> Result<()> {
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(r)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    // ---------------------------------------------------------------- folders

    /// Every folder, in the order they were made.
    pub fn list(&self) -> Vec<Folder> {
        let _g = self.guard();
        self.load().folders
    }

    pub fn get(&self, id: &str) -> Option<Folder> {
        let _g = self.guard();
        self.load().folders.into_iter().find(|f| f.id == id)
    }

    /// A new folder. The name is trimmed, cut to [`NAME_MAX`], and has to be
    /// non-empty and unlike every other folder's apart from case.
    pub fn create(&self, name: &str) -> Result<Folder> {
        let _g = self.guard();
        let mut r = self.load();
        let f = make(&mut r, name)?;
        self.write(&r)?;
        Ok(f)
    }

    /// Rename under the same rules (a change of case alone is fine). The
    /// folder keeps its id, colour, creation time and place in the list.
    pub fn rename(&self, id: &str, name: &str) -> Result<Folder> {
        let _g = self.guard();
        let mut r = self.load();
        let name = clean(name)?;
        if !r.folders.iter().any(|f| f.id == id) {
            bail!("no such folder");
        }
        if r.folders.iter().any(|f| f.id != id && same_name(&f.name, &name)) {
            bail!("there is already a folder called {name}");
        }
        let stamp = next_stamp(&r.folders);
        let slot = r.folders.iter_mut().find(|f| f.id == id).ok_or_else(|| anyhow!("no such folder"))?;
        slot.name = name;
        slot.updated = stamp;
        let f = slot.clone();
        self.write(&r)?;
        Ok(f)
    }

    /// Remove a folder. Its streams become unfiled and are never deleted, and
    /// proposals to file something in it are dropped.
    pub fn delete(&self, id: &str) -> Result<()> {
        let _g = self.guard();
        let mut r = self.load();
        let before = r.folders.len();
        r.folders.retain(|f| f.id != id);
        if r.folders.len() == before {
            bail!("no such folder");
        }
        r.proposals.retain(|p| p.target != ProposalTarget::Existing(id.to_string()));
        // Streams first: stopping between the two leaves an empty folder, not
        // streams filed in nothing.
        self.chats.unfile_all(id);
        self.write(&r)
    }

    // ---------------------------------------------------------------- filing

    /// File a stream in a folder, or with `None` take it out of its folder.
    /// The folder has to exist. Only the stream's `folder` changes: its
    /// `project` (where it runs) and everything else stay as they were.
    pub fn set_thread_folder(&self, thread_id: &str, folder: Option<&str>) -> Result<()> {
        let _g = self.guard();
        if let Some(id) = folder {
            if !self.load().folders.iter().any(|f| f.id == id) {
                bail!("no such folder");
            }
        }
        self.chats.set_folder(thread_id, folder.map(str::to_string))
    }

    /// The streams filed in a folder, as the sidebar lists them (pinned, then
    /// newest first). A folder that does not exist holds nothing.
    pub fn threads_in(&self, folder_id: &str) -> Vec<ThreadInfo> {
        let _g = self.guard();
        if !self.load().folders.iter().any(|f| f.id == folder_id) {
            return vec![];
        }
        self.chats
            .list_with_folder()
            .into_iter()
            .filter(|(_, f)| f.as_deref() == Some(folder_id))
            .map(|(t, _)| t)
            .collect()
    }

    /// The streams in no folder (including any filed in one that is gone).
    pub fn unfiled(&self) -> Vec<ThreadInfo> {
        let _g = self.guard();
        let known: HashSet<String> = self.load().folders.into_iter().map(|f| f.id).collect();
        self.chats
            .list_with_folder()
            .into_iter()
            .filter(|(_, f)| f.as_ref().map_or(true, |f| !known.contains(f)))
            .map(|(t, _)| t)
            .collect()
    }

    /// One entry per folder, in creation order, empty folders included.
    pub fn counts(&self) -> Vec<FolderCount> {
        let _g = self.guard();
        let folders = self.load().folders;
        let mut by: HashMap<String, (usize, u64)> = HashMap::new();
        for (t, f) in self.chats.list_with_folder() {
            if let Some(f) = f {
                let e = by.entry(f).or_default();
                e.0 += 1;
                e.1 = e.1.max(t.updated);
            }
        }
        folders
            .into_iter()
            .map(|f| {
                let (streams, last_activity) = by.get(&f.id).copied().unwrap_or_default();
                FolderCount { folder_id: f.id, streams, last_activity }
            })
            .collect()
    }

    // ---------------------------------------------------------------- proposals

    /// Keep a proposal until it is accepted or rejected. A stream has one at a
    /// time: a newer one replaces the older. The stream and the folder it
    /// names have to exist now, and a new folder's name has to be a usable one.
    pub fn propose(&self, mut p: FolderProposal) -> Result<FolderProposal> {
        let _g = self.guard();
        let mut r = self.load();
        if !self.chats.can_file(&p.thread_id) {
            bail!("no such chat");
        }
        p.target = vet(&r, &p.target)?;
        if p.created == 0 {
            p.created = now_ms();
        }
        r.proposals.retain(|q| q.thread_id != p.thread_id);
        r.proposals.push(p.clone());
        self.write(&r)?;
        Ok(p)
    }

    /// The proposals waiting for an answer, oldest first, for streams that
    /// still exist.
    pub fn proposals(&self) -> Vec<FolderProposal> {
        let _g = self.guard();
        self.load().proposals.into_iter().filter(|p| self.chats.can_file(&p.thread_id)).collect()
    }

    /// Say yes: file the stream where the proposal says, making the folder
    /// first for a new one (or using the folder of that name if there is one by
    /// now). Returns the folder it was filed in. If it cannot be done, nothing
    /// is created and nothing is filed.
    pub fn accept(&self, p: &FolderProposal) -> Result<Folder> {
        let _g = self.guard();
        let mut r = self.load();
        if !self.chats.can_file(&p.thread_id) {
            bail!("no such chat");
        }
        let (folder, made) = match vet(&r, &p.target)? {
            ProposalTarget::Existing(id) => (r.folders.iter().find(|f| f.id == id).cloned().ok_or_else(|| anyhow!("no such folder"))?, false),
            ProposalTarget::New(name) => match r.folders.iter().find(|f| same_name(&f.name, &name)) {
                Some(f) => (f.clone(), false),
                None => (make(&mut r, &name)?, true),
            },
        };
        // A new folder is on disk before any stream points at it.
        if made {
            self.write(&r)?;
        }
        self.chats.set_folder(&p.thread_id, Some(folder.id.clone()))?;
        r.proposals.retain(|q| q.thread_id != p.thread_id);
        self.write(&r)?;
        Ok(folder)
    }

    /// Say no: forget the proposal and change nothing else. Harmless if it is
    /// not there.
    pub fn reject(&self, p: &FolderProposal) -> Result<()> {
        let _g = self.guard();
        let mut r = self.load();
        let target = vet(&r, &p.target).unwrap_or_else(|_| p.target.clone());
        let before = r.proposals.len();
        r.proposals.retain(|q| !(q.thread_id == p.thread_id && q.target == target));
        if r.proposals.len() != before {
            self.write(&r)?;
        }
        Ok(())
    }
}

/// A folder name as it is kept: trimmed and cut to [`NAME_MAX`] characters.
/// Empty is refused.
fn clean(name: &str) -> Result<String> {
    let n: String = name.trim().chars().take(NAME_MAX).collect();
    let n = n.trim_end().to_string();
    if n.is_empty() {
        bail!("a folder needs a name");
    }
    Ok(n)
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// A stamp strictly newer than every folder's, so "the just-renamed one is
/// newest" holds even when two edits land in the same millisecond.
fn next_stamp(all: &[Folder]) -> u64 {
    let latest = all.iter().map(|f| f.updated).max().unwrap_or(0);
    now_ms().max(latest.saturating_add(1))
}

/// Add a folder to the registry in memory: the caller writes it.
fn make(r: &mut Registry, name: &str) -> Result<Folder> {
    let name = clean(name)?;
    if r.folders.iter().any(|f| same_name(&f.name, &name)) {
        bail!("there is already a folder called {name}");
    }
    let nth = r.made.max(r.folders.len());
    let stamp = next_stamp(&r.folders);
    let f = Folder { id: new_id(), name, color: PALETTE[nth % PALETTE.len()].to_string(), created: stamp, updated: stamp };
    r.made = nth + 1;
    r.folders.push(f.clone());
    Ok(f)
}

/// A proposal's target as it will be kept: a folder it names has to exist; a
/// new folder's name is cleaned.
fn vet(r: &Registry, target: &ProposalTarget) -> Result<ProposalTarget> {
    match target {
        ProposalTarget::Existing(id) => {
            if !r.folders.iter().any(|f| &f.id == id) {
                bail!("no such folder");
            }
            Ok(target.clone())
        }
        ProposalTarget::New(name) => Ok(ProposalTarget::New(clean(name)?)),
    }
}
