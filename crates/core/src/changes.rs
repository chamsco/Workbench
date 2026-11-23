//! A durable journal of how the project's state changed, so a client that was
//! cut off (network, restart of this machine's app) can catch up with only
//! what it missed instead of reloading everything.
//!
//! The state stays the source of truth. After a burst of changes the caller
//! hands the journal the new state as JSON; the journal stores the difference
//! as numbered changes in SQLite. A client remembers `(epoch, seq)` and asks
//! [`Journal::changes_since`]: it gets the changes after `seq`, or [`Resume::Reset`]
//! when it cannot be caught up (other epoch, or it fell behind what is kept)
//! and must take [`Journal::current`] whole.
//!
//! The state is saved to `state.json` on its own schedule, so after a crash it
//! can differ from where the journal ended. [`Journal::open`] rebuilds the
//! journal's last state (snapshot plus later changes) and records the
//! difference with the state the project restarted from as an ordinary
//! change: a client that was ahead or behind sees exactly what the restart
//! changed, and its `seq` stays valid.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

/// Changes since the last snapshot after which a new one is written.
const SNAPSHOT_EVERY: u64 = 500;
/// How many changes before the snapshot are kept for clients that lag.
const KEEP_BEFORE_SNAPSHOT: u64 = 1000;

pub struct Journal {
    conn: Mutex<Connection>,
    epoch: String,
    live: Mutex<Live>,
}

struct Live {
    state: Value,
    seq: u64,
    since_snapshot: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub seq: u64,
    pub patch: Value,
}

#[derive(Debug, PartialEq)]
pub enum Resume {
    /// Apply these, in order, to the state the client holds (may be empty).
    Changes(Vec<Change>),
    /// Cannot be caught up: take the whole current state.
    Reset,
}

impl Journal {
    /// Opens (or creates) the journal at `path`. `initial` is the state the
    /// project starts from now.
    pub fn open(path: &Path, initial: &Value) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (epoch TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS snapshot (seq INTEGER NOT NULL, state TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS changes (seq INTEGER PRIMARY KEY, patch TEXT NOT NULL);",
        )?;
        let epoch: Option<String> = conn.query_row("SELECT epoch FROM meta", [], |r| r.get(0)).optional()?;
        let epoch = match epoch {
            Some(e) => e,
            None => {
                let e = crate::remote::new_token();
                conn.execute("INSERT INTO meta (epoch) VALUES (?1)", params![e])?;
                e
            }
        };
        let snap: Option<(u64, String)> =
            conn.query_row("SELECT seq, state FROM snapshot", [], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        let (mut state, mut seq) = match snap {
            Some((seq, text)) => (serde_json::from_str::<Value>(&text)?, seq),
            None => {
                conn.execute("INSERT INTO snapshot (seq, state) VALUES (0, ?1)", params![initial.to_string()])?;
                (initial.clone(), 0)
            }
        };
        {
            let mut q = conn.prepare("SELECT seq, patch FROM changes WHERE seq > ?1 ORDER BY seq")?;
            let rows = q.query_map(params![seq], |r| Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?)))?;
            for row in rows {
                let (s, text) = row?;
                apply(&mut state, &serde_json::from_str(&text)?)?;
                seq = s;
            }
        }
        let j = Self {
            conn: Mutex::new(conn),
            epoch,
            live: Mutex::new(Live { state, seq, since_snapshot: 0 }),
        };
        // What the restart changed relative to where the journal ended.
        j.record(initial)?;
        Ok(j)
    }

    /// Records `next` as the new state. Returns the change's `seq`, or None
    /// when nothing differs.
    pub fn record(&self, next: &Value) -> Result<Option<u64>> {
        let mut live = self.live.lock().unwrap();
        let mut ops = Vec::new();
        diff(&live.state, next, &mut Vec::new(), &mut ops);
        if ops.is_empty() {
            return Ok(None);
        }
        let seq = live.seq + 1;
        let patch = Value::Array(ops);
        {
            let mut c = self.conn.lock().unwrap();
            let tx = c.transaction()?;
            tx.execute("INSERT INTO changes (seq, patch) VALUES (?1, ?2)", params![seq, patch.to_string()])?;
            tx.commit()?;
        }
        live.state = next.clone();
        live.seq = seq;
        live.since_snapshot += 1;
        if live.since_snapshot >= SNAPSHOT_EVERY {
            let mut c = self.conn.lock().unwrap();
            let tx = c.transaction()?;
            tx.execute("DELETE FROM snapshot", [])?;
            tx.execute("INSERT INTO snapshot (seq, state) VALUES (?1, ?2)", params![seq, live.state.to_string()])?;
            tx.execute("DELETE FROM changes WHERE seq <= ?1", params![seq.saturating_sub(KEEP_BEFORE_SNAPSHOT)])?;
            tx.commit()?;
            live.since_snapshot = 0;
        }
        Ok(Some(seq))
    }

    /// The epoch, the last `seq` and the state at it, as one consistent read.
    pub fn current(&self) -> (String, u64, Value) {
        let live = self.live.lock().unwrap();
        (self.epoch.clone(), live.seq, live.state.clone())
    }

    /// What a client that holds the state at `(epoch, after)` is missing.
    pub fn changes_since(&self, epoch: &str, after: u64) -> Result<Resume> {
        let live = self.live.lock().unwrap();
        if epoch != self.epoch || after > live.seq {
            return Ok(Resume::Reset);
        }
        let c = self.conn.lock().unwrap();
        let oldest: Option<u64> = c.query_row("SELECT MIN(seq) FROM changes", [], |r| r.get(0))?;
        // The client needs change after+1 onward; if the first one kept is
        // later than that, it fell behind what is kept.
        if after < live.seq && oldest.map_or(true, |o| o > after + 1) {
            return Ok(Resume::Reset);
        }
        let mut q = c.prepare("SELECT seq, patch FROM changes WHERE seq > ?1 ORDER BY seq")?;
        let rows = q.query_map(params![after], |r| Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, text) = row?;
            out.push(Change { seq, patch: serde_json::from_str(&text)? });
        }
        Ok(Resume::Changes(out))
    }
}

/// The operations that turn `prev` into `next`: `{"path":[..],"set":v}` or
/// `{"path":[..],"del":true}`. Objects are compared key by key, everything
/// else (arrays, scalars) is replaced whole.
fn diff(prev: &Value, next: &Value, path: &mut Vec<String>, out: &mut Vec<Value>) {
    match (prev, next) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, nv) in b {
                path.push(k.clone());
                match a.get(k) {
                    Some(pv) => diff(pv, nv, path, out),
                    None => out.push(json!({"path": path, "set": nv})),
                }
                path.pop();
            }
            for k in a.keys().filter(|k| !b.contains_key(*k)) {
                path.push(k.clone());
                out.push(json!({"path": path, "del": true}));
                path.pop();
            }
        }
        _ if prev != next => out.push(json!({"path": path, "set": next})),
        _ => {}
    }
}

/// Applies a patch made by [`diff`] to `state`.
pub fn apply(state: &mut Value, patch: &Value) -> Result<()> {
    let Some(ops) = patch.as_array() else { bail!("a patch is a list of operations") };
    for op in ops {
        let path: Vec<&str> = op["path"].as_array().map(|p| p.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let Some((last, parents)) = path.split_last() else {
            *state = op["set"].clone();
            continue;
        };
        let mut at = &mut *state;
        for k in parents {
            at = match at.as_object_mut().and_then(|o| o.get_mut(*k)) {
                Some(v) => v,
                None => bail!("the patch does not fit this state: no `{k}`"),
            };
        }
        let Some(obj) = at.as_object_mut() else { bail!("the patch does not fit this state: not an object") };
        if op.get("del").is_some() {
            obj.remove(*last);
        } else {
            obj.insert((*last).to_string(), op["set"].clone());
        }
    }
    Ok(())
}

