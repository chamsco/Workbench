//! M6 (PLAN §9.2): herdr behind a swappable adapter, version pinned.
//!
//! herdr (herdrdev/herdr) is a terminal agent multiplexer: it owns agents'
//! terminals in panes and reports each agent as idle, working, blocked or
//! done, over a local socket (newline-delimited JSON) that its CLI wraps.
//! Backspace reaches it only through [`Supervisor`], which also has a native
//! implementation, so herdr can be switched off or replaced.
//!
//! * Pinned: only [`PINNED`] (or the version the project's config pins) is
//!   used. Another version, a missing binary or a socket that does not answer
//!   `ping` falls back to native, with a notice saying why.
//! * Never a gate: what herdr says about an agent is shown as a signal. No
//!   ticket, gate or merge waits on it or changes because of it.
//!
//! Only the documented methods are used (`ping`, `agent.list`). This was
//! written against herdr's published socket API for 0.9.5; the real binary
//! could not be downloaded from this environment, so it is tested against a
//! stand-in that speaks that protocol (docs/cauldron/M6-NOTES.md).

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The herdr release this adapter is written and pinned against.
pub const PINNED: &str = "0.9.5";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct HerdrConfig {
    pub enabled: bool,
    /// The release to accept; any other is refused.
    pub version: String,
    /// The herdr binary (a name on PATH, or a path).
    pub bin: String,
    /// The socket; unset, herdr's default (~/.config/herdr/herdr.sock).
    pub socket: Option<String>,
}

impl Default for HerdrConfig {
    fn default() -> Self {
        HerdrConfig { enabled: false, version: PINNED.into(), bin: "herdr".into(), socket: None }
    }
}

/// An agent's state as herdr reports it: a signal, never a gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Signal {
    Idle,
    Working,
    Blocked,
    Done,
    Unknown,
}

impl Signal {
    pub fn parse(s: &str) -> Signal {
        match s {
            "idle" => Signal::Idle,
            "working" => Signal::Working,
            "blocked" => Signal::Blocked,
            "done" => Signal::Done,
            _ => Signal::Unknown,
        }
    }
}

pub type Signals<'a> = Pin<Box<dyn Future<Output = Result<Vec<(String, Signal)>>> + Send + 'a>>;

/// Who supervises the agents' terminals.
pub trait Supervisor: Send + Sync {
    /// "native", or "herdr <version>".
    fn name(&self) -> String;
    /// Each agent it knows, by name, and its state.
    fn signals(&self) -> Signals<'_>;
}

/// Backspace's own supervision: the agents run in-process and their state is
/// Backspace's own, so there is nothing extra to report.
pub struct Native;

impl Supervisor for Native {
    fn name(&self) -> String {
        "native".into()
    }

    fn signals(&self) -> Signals<'_> {
        Box::pin(async { Ok(vec![]) })
    }
}

/// herdr, reached over its socket.
pub struct Herdr {
    version: String,
    socket: PathBuf,
}

/// `herdr 0.9.5`, `herdr v0.9.5 (abc)`: the first x.y.z.
pub fn parse_version(s: &str) -> Option<String> {
    s.split_whitespace().map(|w| w.trim_start_matches('v')).find(|w| {
        let parts: Vec<&str> = w.split('.').collect();
        parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    }).map(String::from)
}

fn default_socket() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".config/herdr/herdr.sock")
}

#[cfg(unix)]
fn request(socket: &std::path::Path, method: &str, params: Value) -> Result<Value> {
    use std::io::{BufRead, BufReader, Write};
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let id = format!("bs_{}", N.fetch_add(1, Ordering::Relaxed));
    let mut s = std::os::unix::net::UnixStream::connect(socket)?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    writeln!(s, "{}", json!({"id": id, "method": method, "params": params}))?;
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line)?;
    let v: Value = serde_json::from_str(&line)?;
    if let Some(e) = v.get("error") {
        return Err(anyhow!("herdr {method}: {}", e["message"].as_str().unwrap_or("error")));
    }
    Ok(v["result"].clone())
}

#[cfg(not(unix))]
fn request(_socket: &std::path::Path, method: &str, _params: Value) -> Result<Value> {
    Err(anyhow!("herdr {method}: its named-pipe socket on Windows is not supported by this adapter yet"))
}

impl Supervisor for Herdr {
    fn name(&self) -> String {
        format!("herdr {}", self.version)
    }

    fn signals(&self) -> Signals<'_> {
        let socket = self.socket.clone();
        Box::pin(async move {
            let v = tokio::task::spawn_blocking(move || request(&socket, "agent.list", json!({}))).await??;
            let list = v.get("agents").and_then(Value::as_array).cloned().or_else(|| v.as_array().cloned()).unwrap_or_default();
            Ok(list
                .iter()
                .map(|a| {
                    let name = a["name"].as_str().or_else(|| a["id"].as_str()).unwrap_or("?").to_string();
                    (name, Signal::parse(a["status"].as_str().unwrap_or("")))
                })
                .collect())
        })
    }
}

/// The supervisor the config asks for, or native and why not.
pub fn select(c: &HerdrConfig) -> (Box<dyn Supervisor>, Option<String>) {
    if !c.enabled {
        return (Box::new(Native), None);
    }
    let native = |why: String| -> (Box<dyn Supervisor>, Option<String>) { (Box::new(Native), Some(format!("herdr: {why}; the native supervisor runs instead"))) };
    // A binary that was just written can be briefly "busy" (ETXTBSY) while
    // another thread forks: try again rather than call it missing.
    let mut tries = 0;
    let out = loop {
        match std::process::Command::new(&c.bin).arg("--version").stdin(std::process::Stdio::null()).output() {
            Ok(o) => break o,
            Err(e) if e.raw_os_error() == Some(26) && tries < 20 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return native(format!("{} not found", c.bin)),
            Err(e) => return native(format!("{} could not be run ({e})", c.bin)),
        }
    };
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let Some(found) = parse_version(&text) else {
        return native(format!("could not read a version from `{} --version`", c.bin));
    };
    if found != c.version {
        return native(format!("version {found} is installed but {} is pinned; other versions are refused", c.version));
    }
    let socket = c.socket.as_deref().map(|s| match s.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join(rest),
        None => PathBuf::from(s),
    }).unwrap_or_else(default_socket);
    if let Err(e) = request(&socket, "ping", json!({})) {
        return native(format!("its socket {} does not answer ({e})", socket.display()));
    }
    (Box::new(Herdr { version: found, socket }), None)
}
