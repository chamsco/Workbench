//! Chat: threads with one model behind each, outside any project. A thread
//! talks to one of
//!
//! - a coding CLI on this machine (Claude, Codex, Cursor, Grok, OpenCode)
//!   in one-shot mode, run in a scratch folder per thread;
//! - Ollama, for models on this machine;
//! - an OpenAI-compatible router the user added;
//! - Backspace Cloud (see [`crate::cloud`]): free with ads, or paid.
//!
//! Threads are JSON files in the data folder. Replies stream into the last
//! message; shells redraw on the shared change channel and read threads back
//! with [`Chats::thread`].

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::task::AbortHandle;

use crate::cloud::{Account, Ad};
use crate::prefs::Prefs;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum RouteKind {
    Cli,
    Local,
    Router,
    Cloud,
}

/// Who answers in a thread.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Route {
    pub kind: RouteKind,
    /// "claude", "codex"... for CLIs; "ollama"; the router id; "cloud".
    pub provider: String,
    /// None: the CLI's or router's own default.
    pub model: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Written, not yet picked up.
    Sending,
    /// The reply is arriving.
    Streaming,
    Done,
    Error,
    /// The user pressed stop.
    Stopped,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Msg {
    pub id: String,
    pub role: Role,
    pub text: String,
    pub at: u64,
    pub status: Status,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub reply_to: Option<String>,
    /// Tapbacks: ❤️ 👍 👎 😂 ‼️ ❓
    #[serde(default)]
    pub reactions: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// Which model answered, as the backend reported it.
    #[serde(default)]
    pub model: Option<String>,
    /// A sponsored card shown under this reply (Cloud free tier and
    /// overage only). Never part of the reply text.
    #[serde(default)]
    pub ad: Option<Ad>,
    #[serde(default)]
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub edited: bool,
    /// Who answered, when not the thread's own route (an @mention).
    #[serde(default)]
    pub via: Option<Route>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Thread {
    pub id: String,
    pub title: String,
    pub created: u64,
    pub updated: u64,
    #[serde(default)]
    pub pinned: bool,
    pub route: Route,
    pub messages: Vec<Msg>,
    /// The CLI's own session, for CLIs that can resume one (Claude).
    #[serde(default)]
    pub cli_session: Option<String>,
    /// Thread this one was branched from.
    #[serde(default)]
    pub branched_from: Option<String>,
}

/// The sidebar row.
#[derive(Serialize, Clone, Debug)]
pub struct ThreadInfo {
    pub id: String,
    pub title: String,
    pub updated: u64,
    pub pinned: bool,
    pub route: Route,
    pub preview: String,
    pub busy: bool,
    pub unread: bool,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn new_id() -> String {
    let n = now_ms();
    let r: u32 = rand_u32();
    format!("{n:x}{r:08x}")
}

fn rand_u32() -> u32 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(now_ms());
    h.finish() as u32
}

fn title_from(text: &str) -> String {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= 48 {
        return t;
    }
    let cut: String = t.chars().take(48).collect();
    match cut.rfind(' ') {
        Some(i) if i > 20 => format!("{}…", &cut[..i]),
        _ => format!("{cut}…"),
    }
}

// ---------------------------------------------------------------- base64

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for ch in b.chunks(3) {
        let n = ch.iter().fold(0u32, |a, &x| (a << 8) | x as u32) << (8 * (3 - ch.len()));
        for i in 0..4 {
            if i <= ch.len() {
                s.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' => continue,
            _ => return None,
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Strip ANSI escapes from CLI output.
fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if c != '\r' {
            out.push(c);
        }
    }
    out
}

/// `@codex ...` at the start of a message: answer with that CLI.
pub fn mention(text: &str) -> Option<Route> {
    let w = text.trim_start().strip_prefix('@')?;
    let name: String = w
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let provider = match name.to_lowercase().as_str() {
        "claude" | "claude-code" => "claude",
        "codex" => "codex",
        "cursor" => "cursor",
        "grok" => "grok",
        "opencode" => "opencode",
        _ => return None,
    };
    Some(Route {
        kind: RouteKind::Cli,
        provider: provider.into(),
        model: None,
    })
}

// ---------------------------------------------------------------- store

pub struct Chats {
    dir: PathBuf,
    threads: Mutex<BTreeMap<String, Thread>>,
    running: Mutex<HashMap<String, AbortHandle>>,
    /// Threads with a reply the user has not opened yet.
    unread: Mutex<std::collections::HashSet<String>>,
    notify: async_channel::Sender<()>,
    rt: tokio::runtime::Handle,
    http: reqwest::Client,
    /// The latest Cloud account seen in a reply.
    account: Mutex<Option<Account>>,
}

/// What a send needs from prefs, copied so a reply never holds the lock.
#[derive(Clone)]
pub struct Ctx {
    pub prefs: Prefs,
}

impl Chats {
    pub fn open(
        dir: PathBuf,
        rt: tokio::runtime::Handle,
        notify: async_channel::Sender<()>,
    ) -> Arc<Chats> {
        let _ = std::fs::create_dir_all(dir.join("threads"));
        let mut threads = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(dir.join("threads")) {
            for e in rd.flatten() {
                let Ok(s) = std::fs::read_to_string(e.path()) else {
                    continue;
                };
                if let Ok(mut t) = serde_json::from_str::<Thread>(&s) {
                    // A reply that was arriving when the app closed.
                    for m in &mut t.messages {
                        if matches!(m.status, Status::Sending | Status::Streaming) {
                            m.status = Status::Stopped;
                        }
                    }
                    threads.insert(t.id.clone(), t);
                }
            }
        }
        Arc::new(Chats {
            dir,
            threads: Mutex::new(threads),
            running: Mutex::new(HashMap::new()),
            unread: Mutex::new(Default::default()),
            notify,
            rt,
            http: reqwest::Client::new(),
            account: Mutex::new(None),
        })
    }

    fn poke(&self) {
        let _ = self.notify.try_send(());
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join("threads").join(format!("{id}.json"))
    }

    pub fn files_dir(&self, thread: &str) -> PathBuf {
        self.dir.join("files").join(thread)
    }

    fn save(&self, id: &str) {
        let t = self.threads.lock().unwrap().get(id).cloned();
        if let Some(t) = t {
            if let Ok(s) = serde_json::to_string_pretty(&t) {
                let _ = std::fs::write(self.path(id), s);
            }
        }
    }

    pub fn list(&self) -> Vec<ThreadInfo> {
        let running = self.running.lock().unwrap();
        let unread = self.unread.lock().unwrap();
        let mut v: Vec<ThreadInfo> = self
            .threads
            .lock()
            .unwrap()
            .values()
            .map(|t| ThreadInfo {
                id: t.id.clone(),
                title: t.title.clone(),
                updated: t.updated,
                pinned: t.pinned,
                route: t.route.clone(),
                preview: t
                    .messages
                    .iter()
                    .rev()
                    .find(|m| !m.text.is_empty())
                    .map(|m| {
                        let p: String = m.text.chars().take(90).collect();
                        p.replace('\n', " ")
                    })
                    .unwrap_or_default(),
                busy: running.contains_key(&t.id),
                unread: unread.contains(&t.id),
            })
            .collect();
        v.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.updated.cmp(&a.updated)));
        v
    }

    /// Read a thread; reading it marks it read.
    pub fn thread(&self, id: &str) -> Option<Thread> {
        self.unread.lock().unwrap().remove(id);
        self.threads.lock().unwrap().get(id).cloned()
    }

    pub fn account(&self) -> Option<Account> {
        self.account.lock().unwrap().clone()
    }

    pub fn set_account(&self, a: Option<Account>) {
        *self.account.lock().unwrap() = a;
        self.poke();
    }

    pub fn create(&self, route: Route) -> Thread {
        let now = now_ms();
        let t = Thread {
            id: new_id(),
            title: "New chat".into(),
            created: now,
            updated: now,
            pinned: false,
            route,
            messages: vec![],
            cli_session: None,
            branched_from: None,
        };
        self.threads.lock().unwrap().insert(t.id.clone(), t.clone());
        self.save(&t.id);
        self.poke();
        t
    }

    fn edit_thread<R>(&self, id: &str, f: impl FnOnce(&mut Thread) -> R) -> Result<R> {
        let r = {
            let mut ts = self.threads.lock().unwrap();
            let t = ts.get_mut(id).ok_or_else(|| anyhow!("no such chat"))?;
            f(t)
        };
        self.save(id);
        self.poke();
        Ok(r)
    }

    pub fn rename(&self, id: &str, title: &str) -> Result<()> {
        let title = title.trim();
        if title.is_empty() {
            bail!("a chat needs a name");
        }
        self.edit_thread(id, |t| t.title = title.to_string())
    }

    pub fn pin(&self, id: &str, pinned: bool) -> Result<()> {
        self.edit_thread(id, |t| t.pinned = pinned)
    }

    /// Switch who answers from the next message on. A CLI session does not
    /// carry over to another backend.
    pub fn set_route(&self, id: &str, route: Route) -> Result<()> {
        self.edit_thread(id, |t| {
            if t.route.kind != route.kind || t.route.provider != route.provider {
                t.cli_session = None;
            }
            t.route = route;
        })
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.stop(id);
        self.threads.lock().unwrap().remove(id);
        let _ = std::fs::remove_file(self.path(id));
        let _ = std::fs::remove_dir_all(self.files_dir(id));
        let _ = std::fs::remove_dir_all(self.dir.join("scratch").join(id));
        self.poke();
        Ok(())
    }

    /// Toggle a tapback on a message.
    pub fn react(&self, id: &str, msg: &str, emoji: &str) -> Result<()> {
        self.edit_thread(id, |t| {
            if let Some(m) = t.messages.iter_mut().find(|m| m.id == msg) {
                if let Some(i) = m.reactions.iter().position(|r| r == emoji) {
                    m.reactions.remove(i);
                } else {
                    // One tapback per message, like iMessage.
                    m.reactions.clear();
                    m.reactions.push(emoji.to_string());
                }
            }
        })
    }

    /// A new thread with the messages up to and including `msg`.
    pub fn branch(&self, id: &str, msg: &str) -> Result<Thread> {
        let src = self.thread(id).ok_or_else(|| anyhow!("no such chat"))?;
        let end = src
            .messages
            .iter()
            .position(|m| m.id == msg)
            .ok_or_else(|| anyhow!("no such message"))?;
        let now = now_ms();
        let t = Thread {
            id: new_id(),
            title: format!("{} (branch)", src.title),
            created: now,
            updated: now,
            pinned: false,
            route: src.route.clone(),
            messages: src.messages[..=end].to_vec(),
            cli_session: None,
            branched_from: Some(src.id.clone()),
        };
        let from = self.files_dir(&src.id);
        if from.is_dir() {
            let to = self.files_dir(&t.id);
            let _ = std::fs::create_dir_all(&to);
            if let Ok(rd) = std::fs::read_dir(&from) {
                for e in rd.flatten() {
                    let _ = std::fs::copy(e.path(), to.join(e.file_name()));
                }
            }
        }
        self.threads.lock().unwrap().insert(t.id.clone(), t.clone());
        self.save(&t.id);
        self.poke();
        Ok(t)
    }

    /// Save an attachment (sent from the shell as base64) into the thread.
    pub fn attach(&self, thread: &str, name: &str, mime: &str, b64: &str) -> Result<Attachment> {
        let bytes = b64_decode(b64.split_once(',').map_or(b64, |(_, d)| d))
            .ok_or_else(|| anyhow!("attachment is not valid base64"))?;
        if bytes.len() > 10 << 20 {
            bail!("attachments are limited to 10 MB");
        }
        let dir = self.files_dir(thread);
        std::fs::create_dir_all(&dir)?;
        let ext = Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>();
        let a = Attachment {
            id: format!("{}.{ext}", new_id()),
            name: name.chars().take(120).collect(),
            mime: mime.to_string(),
            size: bytes.len() as u64,
        };
        std::fs::write(dir.join(&a.id), bytes)?;
        Ok(a)
    }

    pub fn attachment_path(&self, thread: &str, id: &str) -> Result<PathBuf> {
        if id.contains('/') || id.contains('\\') || id.contains("..") {
            bail!("bad attachment id");
        }
        let p = self.files_dir(thread).join(id);
        if !p.is_file() {
            bail!("attachment is gone");
        }
        Ok(p)
    }

    pub fn is_busy(&self, id: &str) -> bool {
        self.running.lock().unwrap().contains_key(id)
    }

    /// Stop the reply in progress. The CLI process is killed with the task.
    pub fn stop(&self, id: &str) {
        if let Some(h) = self.running.lock().unwrap().remove(id) {
            h.abort();
        }
        let _ = self.edit_thread(id, |t| {
            if let Some(m) = t.messages.last_mut() {
                if matches!(m.status, Status::Sending | Status::Streaming) {
                    m.status = Status::Stopped;
                }
            }
        });
    }

    /// Send a user message and start the reply.
    pub fn send(
        self: &Arc<Self>,
        id: &str,
        text: &str,
        attachments: Vec<Attachment>,
        reply_to: Option<String>,
        ctx: Ctx,
    ) -> Result<()> {
        if text.trim().is_empty() && attachments.is_empty() {
            bail!("nothing to send");
        }
        if self.is_busy(id) {
            bail!("wait for the reply, or stop it first");
        }
        let now = now_ms();
        self.edit_thread(id, |t| {
            if t.messages.is_empty() || t.title == "New chat" {
                let base = if text.trim().is_empty() {
                    attachments
                        .first()
                        .map(|a| a.name.clone())
                        .unwrap_or_default()
                } else {
                    text.to_string()
                };
                t.title = title_from(&base);
            }
            t.messages.push(Msg {
                id: new_id(),
                role: Role::User,
                text: text.trim_end().to_string(),
                at: now,
                status: Status::Done,
                error: None,
                reply_to,
                reactions: vec![],
                attachments,
                model: None,
                ad: None,
                cost_usd: None,
                edited: false,
                via: None,
            });
            t.updated = now;
        })?;
        self.reply(id, ctx, mention(text))
    }

    /// Ask again: drop the last reply (and anything after the last user
    /// message) and run it again.
    pub fn retry(self: &Arc<Self>, id: &str, ctx: Ctx) -> Result<()> {
        if self.is_busy(id) {
            bail!("a reply is still arriving");
        }
        self.edit_thread(id, |t| {
            while t.messages.last().is_some_and(|m| m.role == Role::Assistant) {
                t.messages.pop();
            }
            // The CLI session has the old answer in it.
            t.cli_session = None;
        })?;
        let again = self
            .threads
            .lock()
            .unwrap()
            .get(id)
            .and_then(|t| {
                t.messages
                    .iter()
                    .rev()
                    .find(|m| m.role == Role::User)
                    .map(|m| m.text.clone())
            })
            .and_then(|t| mention(&t));
        self.reply(id, ctx, again)
    }

    /// Change a user message and ask again from there.
    pub fn edit(self: &Arc<Self>, id: &str, msg: &str, text: &str, ctx: Ctx) -> Result<()> {
        if self.is_busy(id) {
            bail!("a reply is still arriving");
        }
        self.edit_thread(id, |t| {
            if let Some(i) = t.messages.iter().position(|m| m.id == msg) {
                t.messages.truncate(i + 1);
                t.messages[i].text = text.trim_end().to_string();
                t.messages[i].edited = true;
                t.cli_session = None;
            }
        })?;
        self.reply(id, ctx, mention(text))
    }

    fn reply(self: &Arc<Self>, id: &str, ctx: Ctx, via: Option<Route>) -> Result<()> {
        let own = self
            .threads
            .lock()
            .unwrap()
            .get(id)
            .map(|t| t.route.clone())
            .ok_or_else(|| anyhow!("no such chat"))?;
        // An @mention hands this one reply to another CLI. The thread's own
        // CLI session then misses it, so the next reply gets the transcript.
        let via = via.filter(|v| v != &own);
        let mentioned = via.is_some();
        let route = via.unwrap_or(own);
        if mentioned {
            let _ = self.edit_thread(id, |t| t.cli_session = None);
        }
        let mid = new_id();
        self.edit_thread(id, |t| {
            t.messages.push(Msg {
                id: mid.clone(),
                role: Role::Assistant,
                text: String::new(),
                at: now_ms(),
                status: Status::Sending,
                error: None,
                reply_to: None,
                reactions: vec![],
                attachments: vec![],
                model: route.model.clone(),
                ad: None,
                cost_usd: None,
                edited: false,
                via: mentioned.then(|| route.clone()),
            });
        })?;
        let me = self.clone();
        let tid = id.to_string();
        let task = self.rt.spawn(async move {
            let res = me.run(&tid, &mid, &route, &ctx).await;
            if mentioned {
                let _ = me.edit_thread(&tid, |t| t.cli_session = None);
            }
            me.running.lock().unwrap().remove(&tid);
            let _ = me.edit_thread(&tid, |t| {
                if let Some(m) = t.messages.iter_mut().find(|m| m.id == mid) {
                    match res {
                        Ok(()) => {
                            m.status = Status::Done;
                            if m.text.trim().is_empty() {
                                m.text = "(no reply)".into();
                            }
                        }
                        Err(e) => {
                            m.status = Status::Error;
                            m.error = Some(format!("{e:#}"));
                        }
                    }
                }
                t.updated = now_ms();
            });
            me.unread.lock().unwrap().insert(tid);
            me.poke();
        });
        self.running
            .lock()
            .unwrap()
            .insert(id.to_string(), task.abort_handle());
        self.poke();
        Ok(())
    }

    fn with_msg(&self, id: &str, mid: &str, f: impl FnOnce(&mut Msg)) {
        if let Some(m) = self
            .threads
            .lock()
            .unwrap()
            .get_mut(id)
            .and_then(|t| t.messages.iter_mut().find(|m| m.id == mid))
        {
            f(m);
        }
        self.poke();
    }

    fn append(&self, id: &str, mid: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        self.with_msg(id, mid, |m| {
            m.status = Status::Streaming;
            m.text.push_str(delta);
        });
    }

    fn set_text(&self, id: &str, mid: &str, text: &str) {
        self.with_msg(id, mid, |m| {
            m.status = Status::Streaming;
            m.text = text.to_string();
        });
    }

    /// The conversation before the reply being written, oldest first.
    fn history(&self, id: &str) -> (Vec<Msg>, Option<String>) {
        let ts = self.threads.lock().unwrap();
        let t = &ts[id];
        let mut h: Vec<Msg> = t.messages.clone();
        h.pop(); // the empty reply
        (h, t.cli_session.clone())
    }

    async fn run(&self, id: &str, mid: &str, route: &Route, ctx: &Ctx) -> Result<()> {
        match route.kind {
            RouteKind::Cli => self.run_cli(id, mid, route).await,
            RouteKind::Local => {
                let url = ctx.prefs.ollama_url.trim_end_matches('/').to_string();
                self.run_ollama(id, mid, &url, route).await
            }
            RouteKind::Router => {
                let r = ctx
                    .prefs
                    .routers
                    .iter()
                    .find(|r| r.id == route.provider)
                    .cloned()
                    .ok_or_else(|| anyhow!("that router was removed in Settings"))?;
                self.run_openai(id, mid, &r.base_url, &r.api_key, route)
                    .await
            }
            RouteKind::Cloud => self.run_cloud(id, mid, ctx, route).await,
        }
    }

    // ------------------------------------------------------------ CLIs

    /// A transcript for CLIs that cannot resume a session.
    fn prompt_with_history(&self, id: &str, hist: &[Msg]) -> String {
        let last = hist.last().cloned();
        let mut p = String::new();
        if hist.len() > 1 {
            p.push_str("Conversation so far:\n\n");
            for m in &hist[..hist.len() - 1] {
                let who = match (&m.role, &m.via) {
                    (Role::User, _) => "User".to_string(),
                    (_, Some(v)) => format!("Assistant ({})", v.provider),
                    _ => "Assistant".to_string(),
                };
                let text: String = m.text.chars().take(4000).collect();
                p.push_str(&format!("{who}: {text}\n\n"));
            }
            p.push_str("Reply to the user's last message:\n\n");
        }
        if let Some(m) = last {
            p.push_str(&m.text);
            for a in &m.attachments {
                if let Ok(path) = self.attachment_path(id, &a.id) {
                    p.push_str(&format!("\n\n[Attached: {} at {}]", a.name, path.display()));
                }
            }
        }
        p
    }

    async fn run_cli(&self, id: &str, mid: &str, route: &Route) -> Result<()> {
        let bin_name = match route.provider.as_str() {
            "cursor" => "cursor-agent",
            other => other,
        };
        let bin = crate::harnesses::which(bin_name)
            .or_else(|| crate::harnesses::which("agy").filter(|_| route.provider == "antigravity"))
            .ok_or_else(|| anyhow!("`{bin_name}` is not installed or not on PATH"))?;
        let scratch = self.dir.join("scratch").join(id);
        std::fs::create_dir_all(&scratch)?;
        let (hist, session) = self.history(id);
        let mut cmd = tokio::process::Command::new(&bin);
        cmd.current_dir(&scratch)
            .env("PATH", crate::harnesses::path_env())
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let model = route.model.clone().filter(|m| !m.is_empty());
        let stdin_text: Option<String>;
        match route.provider.as_str() {
            "claude" => {
                cmd.args([
                    "-p",
                    "--output-format",
                    "stream-json",
                    "--verbose",
                    "--include-partial-messages",
                ]);
                if let Some(s) = &session {
                    cmd.args(["--resume", s]);
                }
                if let Some(m) = &model {
                    cmd.args(["--model", m]);
                }
                // With a session, only the new message; without, the
                // transcript (a branch, a retry, a switched backend).
                let last = hist.last().cloned();
                stdin_text = Some(match (&session, last) {
                    (Some(_), Some(m)) => {
                        let mut s = m.text.clone();
                        for a in &m.attachments {
                            if let Ok(p) = self.attachment_path(id, &a.id) {
                                s.push_str(&format!(
                                    "\n\n[Attached: {} at {}]",
                                    a.name,
                                    p.display()
                                ));
                            }
                        }
                        s
                    }
                    _ => self.prompt_with_history(id, &hist),
                });
            }
            "codex" => {
                cmd.args(["exec", "--json", "--skip-git-repo-check"]);
                if let Some(m) = &model {
                    cmd.args(["-m", m]);
                }
                cmd.arg("-");
                stdin_text = Some(self.prompt_with_history(id, &hist));
            }
            "cursor" => {
                cmd.args(["-p", "--output-format", "text"]);
                if let Some(m) = &model {
                    cmd.args(["--model", m]);
                }
                cmd.arg(self.prompt_with_history(id, &hist));
                stdin_text = None;
            }
            "opencode" => {
                cmd.arg("run");
                if let Some(m) = &model {
                    cmd.args(["-m", m]);
                }
                cmd.arg(self.prompt_with_history(id, &hist));
                stdin_text = None;
            }
            "grok" => {
                if let Some(m) = &model {
                    cmd.args(["-m", m]);
                }
                cmd.args(["-p", &self.prompt_with_history(id, &hist)]);
                stdin_text = None;
            }
            other => bail!("{other} has no one-shot mode to chat with; use it from a project"),
        }
        let mut child = cmd
            .spawn()
            .with_context(|| format!("starting {}", bin.display()))?;
        let mut stdin = child.stdin.take();
        if let (Some(mut w), Some(text)) = (stdin.take(), stdin_text) {
            w.write_all(text.as_bytes()).await?;
            w.shutdown().await?;
        }
        let mut err = child.stderr.take().unwrap();
        let err_task = tokio::spawn(async move {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s).await;
            s
        });
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut got_partial = false;
        let mut raw = String::new();
        let mut fail: Option<String> = None;
        while let Some(line) = lines.next_line().await? {
            match route.provider.as_str() {
                "claude" => {
                    let Ok(v) = serde_json::from_str::<Value>(&line) else {
                        continue;
                    };
                    match v["type"].as_str() {
                        Some("system") => {
                            if let Some(s) = v["session_id"].as_str() {
                                let s = s.to_string();
                                let _ = self.edit_thread(id, |t| t.cli_session = Some(s));
                            }
                            if let Some(m) = v["model"].as_str() {
                                let m = m.to_string();
                                self.with_msg(id, mid, |msg| msg.model = Some(m));
                            }
                        }
                        Some("stream_event") => {
                            let e = &v["event"];
                            if e["type"] == "content_block_delta"
                                && e["delta"]["type"] == "text_delta"
                            {
                                got_partial = true;
                                self.append(id, mid, e["delta"]["text"].as_str().unwrap_or(""));
                            } else if e["type"] == "message_start" {
                                // A new assistant turn after a tool call:
                                // separate it from the last one.
                                let need = self
                                    .threads
                                    .lock()
                                    .unwrap()
                                    .get(id)
                                    .and_then(|t| t.messages.iter().find(|m| m.id == mid))
                                    .is_some_and(|m| {
                                        !m.text.is_empty() && !m.text.ends_with("\n\n")
                                    });
                                if need {
                                    self.append(id, mid, "\n\n");
                                }
                            }
                        }
                        Some("assistant") if !got_partial => {
                            let text: String = v["message"]["content"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|b| b["text"].as_str())
                                        .collect::<Vec<_>>()
                                        .join("")
                                })
                                .unwrap_or_default();
                            self.append(id, mid, &text);
                        }
                        Some("result") => {
                            if let Some(s) = v["session_id"].as_str() {
                                let s = s.to_string();
                                let _ = self.edit_thread(id, |t| t.cli_session = Some(s));
                            }
                            let cost = v["total_cost_usd"].as_f64();
                            let is_err = v["is_error"].as_bool().unwrap_or(false);
                            let result = v["result"].as_str().unwrap_or("").to_string();
                            self.with_msg(id, mid, |m| {
                                m.cost_usd = cost;
                                if m.text.trim().is_empty() && !is_err {
                                    m.text = result.clone();
                                }
                            });
                            if is_err {
                                fail = Some(if result.is_empty() {
                                    "Claude reported an error".into()
                                } else {
                                    result
                                });
                            }
                        }
                        _ => {}
                    }
                }
                "codex" => {
                    let Ok(v) = serde_json::from_str::<Value>(&line) else {
                        raw.push_str(&line);
                        raw.push('\n');
                        continue;
                    };
                    match v["type"].as_str() {
                        Some("item.completed") | Some("item.updated") => {
                            let item = &v["item"];
                            let kind = item["type"].as_str().or(item["item_type"].as_str());
                            if matches!(kind, Some("agent_message") | Some("assistant_message")) {
                                if let Some(t) = item["text"].as_str() {
                                    // Each message is one paragraph of the reply.
                                    let cur = self
                                        .threads
                                        .lock()
                                        .unwrap()
                                        .get(id)
                                        .and_then(|t| t.messages.iter().find(|m| m.id == mid))
                                        .map(|m| m.text.clone())
                                        .unwrap_or_default();
                                    if v["type"] == "item.completed" {
                                        let sep = if cur.is_empty() { "" } else { "\n\n" };
                                        self.append(id, mid, &format!("{sep}{t}"));
                                    }
                                }
                            }
                        }
                        Some("error") => fail = v["message"].as_str().map(str::to_string),
                        Some("turn.failed") => {
                            fail = v["error"]["message"].as_str().map(str::to_string)
                        }
                        _ => {}
                    }
                }
                _ => {
                    raw.push_str(&line);
                    raw.push('\n');
                    let shown = plain(&raw);
                    self.set_text(id, mid, shown.trim());
                }
            }
        }
        let status = child.wait().await?;
        let stderr = err_task.await.unwrap_or_default();
        if let Some(f) = fail {
            bail!(f);
        }
        if !status.success() {
            let msg = plain(stderr.trim());
            let msg: String = msg
                .lines()
                .rev()
                .take(6)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            bail!(
                "{} exited with {}{}",
                bin_name,
                status
                    .code()
                    .map_or("a signal".into(), |c| format!("code {c}")),
                if msg.is_empty() {
                    String::new()
                } else {
                    format!(": {msg}")
                }
            );
        }
        Ok(())
    }

    // ------------------------------------------------------------ HTTP

    fn image_b64(&self, id: &str, m: &Msg) -> Vec<(String, String)> {
        m.attachments
            .iter()
            .filter(|a| a.mime.starts_with("image/"))
            .filter_map(|a| {
                let p = self.attachment_path(id, &a.id).ok()?;
                let b = std::fs::read(p).ok()?;
                Some((a.mime.clone(), b64_encode(&b)))
            })
            .collect()
    }

    fn text_with_files(&self, id: &str, m: &Msg) -> String {
        let mut s = m.text.clone();
        for a in m
            .attachments
            .iter()
            .filter(|a| !a.mime.starts_with("image/"))
        {
            if let Ok(p) = self.attachment_path(id, &a.id) {
                if let Ok(t) = std::fs::read_to_string(&p) {
                    let t: String = t.chars().take(60_000).collect();
                    s.push_str(&format!("\n\n--- {} ---\n{t}", a.name));
                }
            }
        }
        s
    }

    async fn run_ollama(&self, id: &str, mid: &str, url: &str, route: &Route) -> Result<()> {
        let model = route
            .model
            .clone()
            .filter(|m| !m.is_empty())
            .ok_or_else(|| anyhow!("pick an Ollama model for this chat"))?;
        let (hist, _) = self.history(id);
        let messages: Vec<Value> = hist
            .iter()
            .map(|m| {
                let mut v = json!({
                    "role": if m.role == Role::User { "user" } else { "assistant" },
                    "content": self.text_with_files(id, m),
                });
                let imgs: Vec<String> = self.image_b64(id, m).into_iter().map(|(_, b)| b).collect();
                if !imgs.is_empty() {
                    v["images"] = json!(imgs);
                }
                v
            })
            .collect();
        let resp = self
            .http
            .post(format!("{url}/api/chat"))
            .json(&json!({ "model": model, "messages": messages, "stream": true }))
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    anyhow!("Ollama is not running at {url}. Start it with `ollama serve`.")
                } else {
                    anyhow!(e)
                }
            })?;
        let status = resp.status();
        if !status.is_success() {
            let t = resp.text().await.unwrap_or_default();
            bail!("Ollama {status}: {}", crate::router::truncate(&t, 300));
        }
        self.with_msg(id, mid, |m| m.model = Some(model.clone()));
        let mut lines = Lines::new(resp);
        while let Some(line) = lines.next().await? {
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(e) = v["error"].as_str() {
                bail!("Ollama: {e}");
            }
            self.append(id, mid, v["message"]["content"].as_str().unwrap_or(""));
            if v["done"].as_bool() == Some(true) {
                break;
            }
        }
        Ok(())
    }

    fn openai_messages(&self, id: &str, hist: &[Msg]) -> Vec<Value> {
        hist.iter()
            .map(|m| {
                let role = if m.role == Role::User { "user" } else { "assistant" };
                let imgs = self.image_b64(id, m);
                let text = self.text_with_files(id, m);
                if imgs.is_empty() {
                    json!({ "role": role, "content": text })
                } else {
                    let mut parts = vec![json!({"type": "text", "text": text})];
                    for (mime, b) in imgs {
                        parts.push(json!({"type": "image_url", "image_url": {"url": format!("data:{mime};base64,{b}")}}));
                    }
                    json!({ "role": role, "content": parts })
                }
            })
            .collect()
    }

    async fn run_openai(
        &self,
        id: &str,
        mid: &str,
        base: &str,
        key: &str,
        route: &Route,
    ) -> Result<()> {
        let model = route
            .model
            .clone()
            .filter(|m| !m.is_empty())
            .ok_or_else(|| anyhow!("pick a model for this chat"))?;
        let (hist, _) = self.history(id);
        let mut rb = self
            .http
            .post(format!("{}/chat/completions", base.trim_end_matches('/')))
            .json(&json!({
                "model": model,
                "messages": self.openai_messages(id, &hist),
                "stream": true,
            }));
        if !key.is_empty() {
            rb = rb.bearer_auth(key);
        }
        let resp = rb.send().await.context("router request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let t = resp.text().await.unwrap_or_default();
            bail!("{status}: {}", crate::router::truncate(&t, 400));
        }
        self.with_msg(id, mid, |m| m.model = Some(model.clone()));
        let mut lines = Lines::new(resp);
        while let Some(line) = lines.next().await? {
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                break;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            if let Some(e) = v["error"]["message"].as_str() {
                bail!("{e}");
            }
            self.append(
                id,
                mid,
                v["choices"][0]["delta"]["content"].as_str().unwrap_or(""),
            );
        }
        Ok(())
    }

    async fn run_cloud(&self, id: &str, mid: &str, ctx: &Ctx, route: &Route) -> Result<()> {
        let cloud = &ctx.prefs.cloud;
        if cloud.token.is_empty() {
            bail!("Sign up for Backspace Cloud in Settings → Plan first");
        }
        let model = route.model.clone().unwrap_or_default();
        let (hist, _) = self.history(id);
        let resp = self
            .http
            .post(format!("{}/v1/chat", cloud.url.trim_end_matches('/')))
            .bearer_auth(&cloud.token)
            .json(&json!({ "model": model, "messages": self.openai_messages(id, &hist) }))
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    anyhow!("Backspace Cloud is unreachable at {}", cloud.url)
                } else {
                    anyhow!(e)
                }
            })?;
        let status = resp.status();
        if !status.is_success() {
            let v: Value = resp.json().await.unwrap_or(Value::Null);
            if let Some(a) = v
                .get("account")
                .and_then(|a| serde_json::from_value(a.clone()).ok())
            {
                self.set_account(Some(a));
            }
            bail!(
                "{}",
                v["error"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("Cloud: {status}"))
            );
        }
        let mut lines = Lines::new(resp);
        while let Some(line) = lines.next().await? {
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<Value>(data.trim()) else {
                continue;
            };
            match v["type"].as_str() {
                Some("delta") => self.append(id, mid, v["text"].as_str().unwrap_or("")),
                Some("model") => {
                    let m = v["model"].as_str().map(str::to_string);
                    self.with_msg(id, mid, |msg| msg.model = m);
                }
                Some("ad") => {
                    if let Ok(ad) = serde_json::from_value::<Ad>(v["ad"].clone()) {
                        self.with_msg(id, mid, |m| m.ad = Some(ad));
                    }
                }
                Some("done") => {
                    if let Ok(a) = serde_json::from_value::<Account>(v["account"].clone()) {
                        self.set_account(Some(a));
                    }
                    let cost = v["charged_usd"].as_f64();
                    self.with_msg(id, mid, |m| m.cost_usd = cost);
                    break;
                }
                Some("error") => bail!("{}", v["message"].as_str().unwrap_or("Cloud error")),
                _ => {}
            }
        }
        Ok(())
    }
}

/// Lines from a streamed HTTP body (NDJSON or SSE) without the `stream`
/// feature: read chunks, split on newlines.
struct Lines {
    resp: reqwest::Response,
    buf: Vec<u8>,
    done: bool,
}

impl Lines {
    fn new(resp: reqwest::Response) -> Self {
        Self {
            resp,
            buf: Vec::new(),
            done: false,
        }
    }

    async fn next(&mut self) -> Result<Option<String>> {
        loop {
            if let Some(i) = self.buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = self.buf.drain(..=i).collect();
                return Ok(Some(String::from_utf8_lossy(&line).trim_end().to_string()));
            }
            if self.done {
                if self.buf.is_empty() {
                    return Ok(None);
                }
                let line = String::from_utf8_lossy(&self.buf).trim_end().to_string();
                self.buf.clear();
                return Ok(Some(line));
            }
            match tokio::time::timeout(Duration::from_secs(300), self.resp.chunk()).await {
                Ok(Ok(Some(c))) => self.buf.extend_from_slice(&c),
                Ok(Ok(None)) => self.done = true,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => bail!("no data for 5 minutes"),
            }
        }
    }
}

// ---------------------------------------------------------------- links

#[derive(Serialize, Clone, Debug, Default)]
pub struct LinkPreview {
    pub url: String,
    pub site: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
}

fn meta(html: &str, keys: &[&str]) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    for key in keys {
        let mut from = 0;
        while let Some(i) = lower[from..].find(&format!("\"{key}\"")) {
            let at = from + i;
            let start = lower[..at].rfind('<').unwrap_or(at);
            let end = lower[at..].find('>').map_or(lower.len(), |e| at + e);
            let tag = &html[start..end];
            let tl = &lower[start..end];
            if let Some(c) = tl.find("content=") {
                let rest = &tag[c + 8..];
                let q = rest.chars().next().unwrap_or('"');
                if q == '"' || q == '\'' {
                    if let Some(e) = rest[1..].find(q) {
                        let v = unescape(&rest[1..1 + e]);
                        if !v.trim().is_empty() {
                            return Some(v.trim().to_string());
                        }
                    }
                }
            }
            from = at + key.len() + 2;
        }
    }
    None
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Title, description and image for a link, for the rich card under a
/// message. Reads at most 512 KB of the page.
pub async fn link_preview(http: &reqwest::Client, url: &str) -> Result<LinkPreview> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        bail!("not a web link");
    }
    let mut resp = http
        .get(url)
        .header("user-agent", "Mozilla/5.0 (Backspace link preview)")
        .timeout(Duration::from_secs(6))
        .send()
        .await?;
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let site = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| h.trim_start_matches("www.").to_string())
        })
        .unwrap_or_default();
    if ct.starts_with("image/") {
        return Ok(LinkPreview {
            url: url.into(),
            site,
            image: Some(url.into()),
            ..Default::default()
        });
    }
    let mut body = Vec::new();
    while let Some(c) = resp.chunk().await? {
        body.extend_from_slice(&c);
        if body.len() > 512 * 1024 {
            break;
        }
    }
    let html = String::from_utf8_lossy(&body).into_owned();
    let title = meta(&html, &["og:title", "twitter:title"]).or_else(|| {
        let l = html.to_ascii_lowercase();
        let s = l.find("<title")?;
        let s = s + l[s..].find('>')? + 1;
        let e = s + l[s..].find("</title>")?;
        Some(unescape(html[s..e].trim()))
    });
    let mut image = meta(&html, &["og:image", "twitter:image", "og:image:url"]);
    if let (Some(img), Ok(base)) = (&image, reqwest::Url::parse(url)) {
        image = base.join(img).ok().map(|u| u.to_string());
    }
    Ok(LinkPreview {
        url: url.into(),
        site,
        title,
        description: meta(
            &html,
            &["og:description", "description", "twitter:description"],
        ),
        image,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chats(dir: &Path) -> (Arc<Chats>, tokio::runtime::Runtime) {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let (tx, _rx) = async_channel::bounded(1);
        (Chats::open(dir.to_path_buf(), rt.handle().clone(), tx), rt)
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bs-chat-{name}-{}", now_ms()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn base64_roundtrip() {
        for s in ["", "a", "ab", "abc", "hello world!", "\u{1f600} ✓"] {
            assert_eq!(b64_decode(&b64_encode(s.as_bytes())).unwrap(), s.as_bytes());
        }
        assert_eq!(b64_encode(b"Man"), "TWFu");
        assert_eq!(b64_encode(b"Ma"), "TWE=");
    }

    #[test]
    fn titles_are_short() {
        assert_eq!(title_from("  hi   there "), "hi there");
        let t = title_from(
            "Explain the difference between processes and threads in operating systems please",
        );
        assert!(t.ends_with('…') && t.chars().count() <= 49, "{t}");
    }

    #[test]
    fn threads_persist_react_branch_delete() {
        let dir = tmp("persist");
        let route = Route {
            kind: RouteKind::Local,
            provider: "ollama".into(),
            model: Some("m".into()),
        };
        let (c, _rt) = chats(&dir);
        let t = c.create(route.clone());
        // A message written by hand, as if a reply had finished.
        c.edit_thread(&t.id, |t| {
            for (i, role) in [Role::User, Role::Assistant].into_iter().enumerate() {
                t.messages.push(Msg {
                    id: format!("m{i}"),
                    role,
                    text: format!("text {i}"),
                    at: 0,
                    status: Status::Streaming,
                    error: None,
                    reply_to: None,
                    reactions: vec![],
                    attachments: vec![],
                    model: None,
                    ad: None,
                    cost_usd: None,
                    edited: false,
                    via: None,
                });
            }
        })
        .unwrap();
        c.react(&t.id, "m1", "❤️").unwrap();
        c.react(&t.id, "m1", "👍").unwrap();
        assert_eq!(c.thread(&t.id).unwrap().messages[1].reactions, vec!["👍"]);
        let b = c.branch(&t.id, "m0").unwrap();
        assert_eq!(b.messages.len(), 1);
        assert_eq!(b.branched_from.as_deref(), Some(t.id.as_str()));
        drop(c);
        // Reopened: a reply cut off by quitting shows as stopped.
        let (c, _rt2) = chats(&dir);
        let back = c.thread(&t.id).unwrap();
        assert_eq!(back.messages[1].status, Status::Stopped);
        assert_eq!(c.list().len(), 2);
        c.delete(&b.id).unwrap();
        assert_eq!(c.list().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn link_meta() {
        let html = r#"<html><head><meta property="og:title" content="Hello &amp; welcome"><meta name="description" content='A page'></head></html>"#;
        assert_eq!(
            meta(html, &["og:title"]).as_deref(),
            Some("Hello & welcome")
        );
        assert_eq!(
            meta(html, &["og:description", "description"]).as_deref(),
            Some("A page")
        );
    }

    #[test]
    fn mentions_pick_a_cli() {
        assert_eq!(mention("@codex review this").unwrap().provider, "codex");
        assert_eq!(
            mention("  @Claude what do you think?").unwrap().provider,
            "claude"
        );
        assert!(mention("email me @ noon").is_none());
        assert!(mention("@nobody hi").is_none());
    }

    #[test]
    fn ansi_is_stripped() {
        assert_eq!(plain("\u{1b}[1mbold\u{1b}[0m\r\n"), "bold\n");
    }
}
