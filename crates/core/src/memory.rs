//! Memory: short notes you want every chat and agent to know ("I write
//! Rust 2024, prefer tabs", "the staging DB is read-only"). A note is either
//! global or tied to one project folder. Chats get the global notes plus
//! their project's; a project's planner gets the same when it opens.
//!
//! Stored as one JSON file in the data dir (`memory.json`).

use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::chat::{new_id, now_ms};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Note {
    pub id: String,
    pub text: String,
    /// A project folder, or None for every chat and project.
    #[serde(default)]
    pub project: Option<String>,
    /// Who wrote it: "you", or the chat it was saved from.
    #[serde(default)]
    pub source: String,
    pub created: u64,
    pub updated: u64,
    /// Off: kept, but not given to chats or agents.
    #[serde(default = "yes")]
    pub on: bool,
}

fn yes() -> bool {
    true
}

/// The most text notes may add to a prompt. Past this the oldest are left out.
const BUDGET: usize = 6000;

pub struct Memory {
    path: PathBuf,
    notes: Mutex<Vec<Note>>,
}

impl Memory {
    pub fn open(dir: PathBuf) -> Self {
        let path = dir.join("memory.json");
        let notes = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            path,
            notes: Mutex::new(notes),
        }
    }

    fn save(&self, notes: &[Note]) -> Result<()> {
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(notes)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// Newest first.
    pub fn list(&self) -> Vec<Note> {
        let mut v = self.notes.lock().unwrap().clone();
        v.sort_by(|a, b| b.updated.cmp(&a.updated));
        v
    }

    pub fn add(&self, text: &str, project: Option<String>, source: &str) -> Result<Note> {
        let text = text.trim();
        if text.is_empty() {
            return Err(anyhow!("a note needs some text"));
        }
        let now = now_ms();
        let n = Note {
            id: new_id(),
            text: text.chars().take(2000).collect(),
            project: project.filter(|p| !p.is_empty()),
            source: if source.is_empty() { "you".into() } else { source.into() },
            created: now,
            updated: now,
            on: true,
        };
        let mut notes = self.notes.lock().unwrap();
        notes.push(n.clone());
        self.save(&notes)?;
        Ok(n)
    }

    pub fn update(&self, id: &str, text: Option<String>, project: Option<Option<String>>, on: Option<bool>) -> Result<Note> {
        let mut notes = self.notes.lock().unwrap();
        let n = notes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or_else(|| anyhow!("no such note"))?;
        if let Some(t) = text {
            let t = t.trim();
            if t.is_empty() {
                return Err(anyhow!("a note needs some text"));
            }
            n.text = t.chars().take(2000).collect();
        }
        if let Some(p) = project {
            n.project = p.filter(|p| !p.is_empty());
        }
        if let Some(o) = on {
            n.on = o;
        }
        n.updated = now_ms();
        let out = n.clone();
        self.save(&notes)?;
        Ok(out)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let mut notes = self.notes.lock().unwrap();
        notes.retain(|n| n.id != id);
        self.save(&notes)
    }

    /// The notes a chat or agent in `project` gets, as a block for its
    /// system prompt; None when there are none.
    pub fn context(&self, project: Option<&str>) -> Option<String> {
        let mut notes: Vec<Note> = self
            .notes
            .lock()
            .unwrap()
            .iter()
            .filter(|n| n.on && (n.project.is_none() || n.project.as_deref() == project))
            .cloned()
            .collect();
        notes.sort_by(|a, b| b.updated.cmp(&a.updated));
        let mut out = String::new();
        for n in notes {
            let line = format!("- {}\n", n.text.replace('\n', " "));
            if out.len() + line.len() > BUDGET {
                break;
            }
            out.push_str(&line);
        }
        (!out.is_empty()).then(|| {
            format!("Notes the user asked you to keep in mind (from Backspace's memory):\n{out}")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_and_persistence() {
        let dir = std::env::temp_dir().join(format!("bs-mem-{}", now_ms()));
        let m = Memory::open(dir.clone());
        m.add("prefers tabs", None, "").unwrap();
        let p = m.add("staging DB is read-only", Some("/w/a".into()), "").unwrap();
        m.add("uses pnpm", Some("/w/b".into()), "").unwrap();
        let a = m.context(Some("/w/a")).unwrap();
        assert!(a.contains("prefers tabs") && a.contains("read-only") && !a.contains("pnpm"));
        assert!(!m.context(None).unwrap().contains("read-only"));
        m.update(&p.id, None, None, Some(false)).unwrap();
        assert!(!m.context(Some("/w/a")).unwrap().contains("read-only"));
        // Reopened from disk.
        let m2 = Memory::open(dir.clone());
        assert_eq!(m2.list().len(), 3);
        m2.delete(&p.id).unwrap();
        assert_eq!(Memory::open(dir.clone()).list().len(), 2);
        assert!(m.add("  ", None, "").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
