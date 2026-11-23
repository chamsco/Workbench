//! Command receipts: a small SQLite journal of side effects that must not be
//! done twice, or must be known to have been cut off.
//!
//! A command takes a key that names exactly what it does (for a merge: the
//! branch, its tip commit and the branch it lands on). [`Receipts::begin`]
//! records that it started; [`Receipts::done`] records the result. After a
//! crash, a key that is `done` is not repeated, and a key that only `started`
//! is reported as cut off, so the caller can check what the effect left behind
//! before retrying. SQLite's own journal makes each write atomic.

use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub struct Receipts {
    conn: Mutex<Connection>,
}

/// What `begin` found for a key.
#[derive(Debug, PartialEq, Eq)]
pub enum Begin {
    /// Never seen: do it, then call `done`.
    New,
    /// Done before, with this result: do not do it again.
    Done(String),
    /// Started before and never finished (a crash): `attempts` includes this one.
    CutOff { attempts: u32 },
}

/// A command that started and has no result.
#[derive(Debug, PartialEq, Eq)]
pub struct InFlight {
    pub key: String,
    pub kind: String,
    pub attempts: u32,
}

impl Receipts {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        // WAL keeps a committed receipt across a kill -9; FULL syncs each commit.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS receipts (
                key      TEXT PRIMARY KEY,
                kind     TEXT NOT NULL,
                attempts INTEGER NOT NULL,
                result   TEXT
            )",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn begin(&self, key: &str, kind: &str) -> Result<Begin> {
        let c = self.conn.lock().unwrap();
        let row: Option<(u32, Option<String>)> = c
            .query_row("SELECT attempts, result FROM receipts WHERE key = ?1", params![key], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        match row {
            None => {
                c.execute("INSERT INTO receipts (key, kind, attempts) VALUES (?1, ?2, 1)", params![key, kind])?;
                Ok(Begin::New)
            }
            Some((_, Some(result))) => Ok(Begin::Done(result)),
            Some((attempts, None)) => {
                c.execute("UPDATE receipts SET attempts = attempts + 1 WHERE key = ?1", params![key])?;
                Ok(Begin::CutOff { attempts: attempts + 1 })
            }
        }
    }

    pub fn done(&self, key: &str, result: &str) -> Result<()> {
        self.conn.lock().unwrap().execute("UPDATE receipts SET result = ?2 WHERE key = ?1", params![key, result])?;
        Ok(())
    }

    /// The command did nothing that lasts (a merge that conflicted and was
    /// aborted): forget it, so the same key can run again.
    pub fn abandon(&self, key: &str) -> Result<()> {
        self.conn.lock().unwrap().execute("DELETE FROM receipts WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn in_flight(&self) -> Result<Vec<InFlight>> {
        let c = self.conn.lock().unwrap();
        let mut q = c.prepare("SELECT key, kind, attempts FROM receipts WHERE result IS NULL ORDER BY rowid")?;
        let rows = q.query_map([], |r| Ok(InFlight { key: r.get(0)?, kind: r.get(1)?, attempts: r.get(2)? }))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}
