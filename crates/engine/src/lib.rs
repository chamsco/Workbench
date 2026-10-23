//! Brainpowr Engine: the part of an inference app that sits between a folder
//! of GGUF files and an OpenAI-compatible URL, after Atomic Chat's core.
//!
//! - [`backend`]: installs one pinned llama.cpp build for this machine
//!   (Metal, Vulkan or CPU), sha256-checked, or uses a `llama-server` on PATH.
//! - [`gguf`]: reads what a model file says about itself.
//! - [`session`]: loads a model on the first request for it, unloads it when
//!   it has been idle, and makes room when another needs it.
//!
//! It does not serve HTTP and does not download models: the host app has a
//! server (Backspace's model_gateway.rs) and its own downloads, and calls
//! [`Engine::ensure`] before it forwards a request. Nothing here knows the
//! host, so another app can use the same crate.

pub mod backend;
pub mod gguf;
pub mod session;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use serde::Serialize;

pub use session::{SessionInfo, Settings};

/// A model file the engine can load.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Model {
    /// What clients ask for: the file name without `.gguf`.
    pub id: String,
    pub path: String,
    pub size: u64,
    pub meta: gguf::Meta,
}

/// What a settings page shows.
#[derive(Serialize, Clone, Debug)]
pub struct Status {
    pub tag: &'static str,
    /// The `llama-server` that would run, and whether it is ours or from PATH.
    pub binary: Option<String>,
    pub bundled: bool,
    pub installed: Vec<backend::Installed>,
    /// The builds for this machine, best first.
    pub variants: Vec<&'static backend::Asset>,
    pub settings: Settings,
    pub sessions: Vec<SessionInfo>,
}

pub struct Engine {
    home: PathBuf,
    models_dir: PathBuf,
    prefer: Mutex<Option<String>>,
    sup: session::Supervisor,
    meta: Mutex<HashMap<PathBuf, (SystemTime, gguf::Meta)>>,
}

impl Engine {
    /// `home` keeps the builds and the logs; `models_dir` is scanned for
    /// `*.gguf` (one level of folders deep). Starts the idle reaper.
    pub fn new(home: PathBuf, models_dir: PathBuf, settings: Settings) -> Arc<Engine> {
        let e = Arc::new(Engine {
            sup: session::Supervisor::new(home.join("logs"), settings),
            home,
            models_dir,
            prefer: Mutex::new(None),
            meta: Mutex::new(HashMap::new()),
        });
        let weak: Weak<Engine> = Arc::downgrade(&e);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(30));
            let Some(e) = weak.upgrade() else { return };
            e.sup.reap();
        });
        e
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// Which installed build to run when several are.
    pub fn prefer(&self, variant: Option<String>) {
        *self.prefer.lock().unwrap() = variant;
    }

    pub fn binary(&self) -> Option<PathBuf> {
        backend::server_binary(&self.home, self.prefer.lock().unwrap().as_deref())
    }

    pub fn status(&self) -> Status {
        let bin = self.binary();
        Status {
            tag: backend::PINNED_TAG,
            bundled: bin.as_ref().is_some_and(|b| b.starts_with(&self.home)),
            binary: bin.map(|b| b.display().to_string()),
            installed: backend::installed(&self.home),
            variants: backend::variants(),
            settings: self.sup.settings(),
            sessions: self.sup.sessions(),
        }
    }

    pub fn set_settings(&self, s: Settings) {
        self.sup.set_settings(s);
    }

    /// Models kept loaded whatever the idle timer says.
    pub fn set_pinned(&self, ids: Vec<String>) {
        self.sup.set_pinned(ids);
    }

    /// Every GGUF under the models folder, without vision projectors and the
    /// later parts of a split file; metadata read once per file version.
    pub fn models(&self) -> Vec<Model> {
        let mut files = Vec::new();
        let Ok(top) = std::fs::read_dir(&self.models_dir) else {
            return vec![];
        };
        for e in top.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Ok(inner) = std::fs::read_dir(&p) {
                    files.extend(inner.flatten().map(|f| f.path()));
                }
            } else {
                files.push(p);
            }
        }
        let mut out: Vec<Model> = files
            .into_iter()
            .filter(|p| is_model_file(p))
            .filter_map(|p| {
                let md = std::fs::metadata(&p).ok()?;
                let meta = self.meta_of(&p, md.modified().unwrap_or(SystemTime::UNIX_EPOCH));
                Some(Model {
                    id: p.file_stem()?.to_string_lossy().into_owned(),
                    path: p.display().to_string(),
                    size: md.len(),
                    meta,
                })
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out.dedup_by(|a, b| a.id == b.id);
        out
    }

    fn meta_of(&self, p: &Path, mtime: SystemTime) -> gguf::Meta {
        if let Some((t, m)) = self.meta.lock().unwrap().get(p) {
            if *t == mtime {
                return m.clone();
            }
        }
        let m = gguf::read(p).unwrap_or_default();
        self.meta
            .lock()
            .unwrap()
            .insert(p.to_path_buf(), (mtime, m.clone()));
        m
    }

    /// The model loaded and answering; started now if it was not. Blocks
    /// while it loads.
    pub fn ensure(&self, id: &str) -> Result<SessionInfo> {
        if let Some(s) = self.sup.touch(id) {
            return Ok(s);
        }
        let m = self
            .models()
            .into_iter()
            .find(|m| m.id == id)
            .ok_or_else(|| anyhow!("no downloaded model `{id}`"))?;
        let bin = self.binary().ok_or_else(|| {
            anyhow!("no llama.cpp build is installed; install one in Settings → Local models")
        })?;
        let spec = session::Spec {
            id: m.id.clone(),
            path: PathBuf::from(&m.path),
            ctx_train: m.meta.context_length,
            embedding: m.meta.embedding,
        };
        self.sup.ensure(&bin, &spec)
    }

    /// Mark a session used, so the idle timer starts over.
    pub fn touch(&self, id: &str) {
        self.sup.touch(id);
    }

    pub fn unload(&self, id: &str) {
        self.sup.unload(id);
    }

    pub fn unload_all(&self) {
        self.sup.unload_all();
    }

    pub fn sessions(&self) -> Vec<SessionInfo> {
        self.sup.sessions()
    }
}

fn is_model_file(p: &Path) -> bool {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let later_part = name.contains("-of-0") && !name.contains("-00001-of-");
    name.ends_with(".gguf") && !name.contains("mmproj") && !later_part
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gguf::{fixture, FixV};

    #[test]
    fn models_are_listed_with_their_metadata() {
        let root = std::env::temp_dir().join(format!("brainpowr-models-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let models = root.join("models");
        std::fs::create_dir_all(models.join("org__repo")).unwrap();
        std::fs::write(
            models.join("org__repo/Tiny-Q4_K_M.gguf"),
            fixture(&[
                ("general.architecture", FixV::S("llama")),
                ("llama.context_length", FixV::U32(4096)),
            ]),
        )
        .unwrap();
        std::fs::write(models.join("org__repo/mmproj-F16.gguf"), b"x").unwrap();
        std::fs::write(models.join("loose.gguf"), b"not gguf").unwrap();
        std::fs::write(models.join("notes.txt"), b"x").unwrap();
        let e = Engine::new(root.join("home"), models, Settings::default());
        let ms = e.models();
        let ids: Vec<&str> = ms.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["Tiny-Q4_K_M", "loose"]);
        assert_eq!(ms[0].meta.context_length, Some(4096));
        // An unreadable header is listed anyway, with nothing known.
        assert_eq!(ms[1].meta, gguf::Meta::default());
        assert!(e.ensure("nope").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
