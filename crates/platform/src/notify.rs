//! The notification inbox (PRD §3.4): the events a person should see, kept in
//! `<data>/notifications.json` until read. The same note is not repeated while
//! it is unread. Quiet hours keep the note but drop its alert (no desktop
//! notification, no sound); the app decides how to alert.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::events::Event;

/// Notes kept: the newest ones.
const KEEP: usize = 200;

/// Hours in local time, `from_hour` inclusive to `to_hour` exclusive; wraps
/// past midnight when `from_hour > to_hour`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct QuietHours {
    pub from_hour: u8,
    pub to_hour: u8,
}

impl QuietHours {
    pub fn contains(&self, hour: u8) -> bool {
        if self.from_hour <= self.to_hour {
            hour >= self.from_hour && hour < self.to_hour
        } else {
            hour >= self.from_hour || hour < self.to_hour
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Note {
    pub id: u64,
    pub event: Event,
    pub read: bool,
    /// False when it came in quiet hours: shown in the inbox, never alerted.
    pub alert: bool,
}

pub struct Inbox {
    path: PathBuf,
}

impl Inbox {
    pub fn open(data: &Path) -> Inbox {
        Inbox { path: data.join("notifications.json") }
    }

    fn load(&self) -> Vec<Note> {
        std::fs::read_to_string(&self.path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    fn save(&self, notes: &[Note]) -> Result<()> {
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(notes)?)?;
        std::fs::rename(tmp, &self.path)?;
        Ok(())
    }

    /// Files `e` as a note, unless the same one (app, source, kind, title) is
    /// still unread. `local_hour` is the person's hour now.
    pub fn push(&self, e: Event, quiet: Option<QuietHours>, local_hour: u8) -> Result<Option<Note>> {
        let mut notes = self.load();
        if notes.iter().any(|n| !n.read && n.event.app == e.app && n.event.source == e.source && n.event.kind == e.kind && n.event.title == e.title) {
            return Ok(None);
        }
        let id = notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let note = Note { id, event: e, read: false, alert: !quiet.is_some_and(|q| q.contains(local_hour)) };
        notes.push(note.clone());
        if notes.len() > KEEP {
            notes.drain(..notes.len() - KEEP);
        }
        self.save(&notes)?;
        Ok(Some(note))
    }

    /// Every note, oldest first.
    pub fn list(&self) -> Vec<Note> {
        self.load()
    }

    pub fn unread(&self) -> usize {
        self.load().iter().filter(|n| !n.read).count()
    }

    pub fn mark_read(&self, id: u64) -> Result<()> {
        let mut notes = self.load();
        for n in notes.iter_mut().filter(|n| n.id == id) {
            n.read = true;
        }
        self.save(&notes)
    }

    pub fn mark_all_read(&self) -> Result<()> {
        let mut notes = self.load();
        for n in &mut notes {
            n.read = true;
        }
        self.save(&notes)
    }
}
