//! The typed event bus (PRD §3.4): an app publishes what happened, as a typed
//! event; every subscriber gets its own copy. A subscriber that is gone is
//! forgotten on the next publish. In-process only: what must outlive the
//! process goes to the inbox ([`crate::notify`]).

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// What kind of thing happened, as a person would sort it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Work is waiting on a person: an approval, an answer.
    NeedsYou,
    /// Work was delivered: a branch, a pull request.
    Delivered,
    /// Work stopped and will not go on by itself.
    Failed,
    Info,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Event {
    /// The app that published it, e.g. "code".
    pub app: String,
    /// What in that app it is about, e.g. a familiar's id.
    pub source: String,
    pub kind: Kind,
    pub title: String,
    pub body: String,
    /// Milliseconds since the epoch.
    pub at: u64,
}

#[derive(Default)]
pub struct Bus {
    subs: Mutex<Vec<Sender<Event>>>,
}

impl Bus {
    pub fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = channel();
        self.subs.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
        rx
    }

    /// Sends `e` to every live subscriber; returns how many got it.
    pub fn publish(&self, e: Event) -> usize {
        let mut subs = self.subs.lock().unwrap_or_else(|e| e.into_inner());
        subs.retain(|s| s.send(e.clone()).is_ok());
        subs.len()
    }
}
