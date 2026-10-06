//! Memory: short notes you want every chat and agent to know ("I write
//! Rust 2024, prefer tabs", "the staging DB is read-only"). A note is either
//! global or tied to one project folder. Chats get the global notes plus
//! their project's; a project's planner gets the same when it opens.
//!
//! Stored as one JSON file in the data dir (`memory.json`).
//!
//! It evolves on its own: messages you send are scored on this machine
//! (memory_learn.rs) and the ones that read like preferences, facts about
//! you or standing instructions become notes; your "Don't remember that"
//! teaches the scorer. A new note close to an old one (same scope, mostly
//! the same words) replaces it instead of piling up, and notes that chats
//! actually get are ranked ahead of ones that never come up.

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
    /// "preference", "identity", "instruction", "fact" or "" (written by hand).
    #[serde(default)]
    pub kind: String,
    /// 0..1: how sure the capture was (1 for notes you wrote).
    #[serde(default = "one")]
    pub importance: f32,
    /// How many prompts it has been part of.
    #[serde(default)]
    pub uses: u32,
    /// Features the capture saw, kept so your answer can train on them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}

/// The most text notes may add to a prompt. Past this the oldest are left out.
const BUDGET: usize = 6000;

pub struct Memory {
    path: PathBuf,
    notes: Mutex<Vec<Note>>,
    learner: crate::memory_learn::Learner,
}

/// Words of a note, for telling near-duplicates apart.
fn words(s: &str) -> std::collections::BTreeSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2 && !["the", "user", "and", "for", "with", "are", "was", "his", "her", "its"].contains(w))
        .map(String::from)
        .collect()
}

fn similar(a: &str, b: &str) -> bool {
    let (x, y) = (words(a), words(b));
    if x.is_empty() || y.is_empty() {
        return false;
    }
    let inter = x.intersection(&y).count() as f32;
    inter / (x.len().min(y.len()) as f32) >= 0.7
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
            learner: crate::memory_learn::Learner::open(dir),
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
            kind: String::new(),
            importance: 1.0,
            uses: 0,
            features: vec![],
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

    /// Score a message you sent and, if it reads like something to keep,
    /// save it (or refresh the note it repeats). Returns the note so the app
    /// can say "… will remember that".
    pub fn capture(&self, message: &str, project: Option<String>, source: &str) -> Option<Note> {
        let p = self.learner.propose(message)?;
        let project = project.filter(|p| !p.is_empty());
        let mut notes = self.notes.lock().unwrap();
        let now = now_ms();
        if let Some(n) = notes.iter_mut().find(|n| n.project == project && similar(&n.text, &p.text)) {
            // The newer wording wins; it is how you say it now.
            n.text = p.text.clone();
            n.updated = now;
            n.importance = n.importance.max(p.score);
            n.features = p.features.clone();
            n.on = true;
            let out = n.clone();
            let _ = self.save(&notes);
            return Some(out);
        }
        let n = Note {
            id: new_id(),
            text: p.text,
            project,
            source: if source.is_empty() { "chat".into() } else { source.into() },
            created: now,
            updated: now,
            on: true,
            kind: p.kind,
            importance: p.score,
            uses: 0,
            features: p.features,
        };
        notes.push(n.clone());
        let _ = self.save(&notes);
        Some(n)
    }

    /// "Don't remember that": delete it and learn not to keep its like.
    pub fn forget(&self, id: &str) -> Result<()> {
        let n = self.list().into_iter().find(|n| n.id == id).ok_or_else(|| anyhow!("no such note"))?;
        if !n.features.is_empty() {
            self.learner.feedback(&n.features, false);
        }
        self.delete(id)
    }

    /// The toast was dismissed: keep it, and learn that this was right.
    pub fn confirm(&self, id: &str) -> Result<()> {
        let mut notes = self.notes.lock().unwrap();
        let n = notes.iter_mut().find(|n| n.id == id).ok_or_else(|| anyhow!("no such note"))?;
        if !n.features.is_empty() {
            self.learner.feedback(&n.features, true);
        }
        n.importance = (n.importance + 0.2).min(1.0);
        self.save(&notes)
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
        // What you wrote or confirmed, and what keeps coming up, first.
        let now = now_ms();
        let rank = |n: &Note| {
            let age_days = (now.saturating_sub(n.updated) as f32) / 864e5;
            n.importance + (n.uses as f32).ln_1p() * 0.1 - (age_days / 365.0).min(0.5)
        };
        notes.sort_by(|a, b| rank(b).partial_cmp(&rank(a)).unwrap_or(std::cmp::Ordering::Equal));
        let mut out = String::new();
        let mut used: Vec<String> = vec![];
        for n in notes {
            let line = format!("- {}\n", n.text.replace('\n', " "));
            if out.len() + line.len() > BUDGET {
                break;
            }
            out.push_str(&line);
            used.push(n.id.clone());
        }
        if !used.is_empty() {
            let mut all = self.notes.lock().unwrap();
            for n in all.iter_mut().filter(|n| used.contains(&n.id)) {
                n.uses = n.uses.saturating_add(1);
            }
            let _ = self.save(&all);
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
