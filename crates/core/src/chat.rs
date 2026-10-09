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
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use backspace_runner::{Event as RunEvent, File as RunFile, Lines, Request, Target, Turn};
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
    /// A remote agent you connected over A2A (prefs `remote_agents`).
    A2a,
}

/// Who answers in a thread.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Route {
    pub kind: RouteKind,
    /// "claude", "codex"... for CLIs; "ollama"; the router id; "cloud";
    /// the remote agent's id.
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
    /// The agent that wrote it (agent threads and groups), and its name then.
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub author_name: Option<String>,
    /// The trace of how this reply was made (trace.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<String>,
    /// Cut off because Backspace closed while it was being written; it can
    /// be resumed (`Chats::resume`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub interrupted: bool,
}

impl Msg {
    fn new(role: Role, text: String, status: Status) -> Msg {
        Msg {
            id: new_id(),
            role,
            text,
            at: now_ms(),
            status,
            error: None,
            reply_to: None,
            reactions: vec![],
            attachments: vec![],
            model: None,
            ad: None,
            cost_usd: None,
            edited: false,
            via: None,
            author: None,
            author_name: None,
            trace: None,
            interrupted: false,
        }
    }
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
    /// Pair: the project folder a CLI works in, with leave to edit it.
    #[serde(default)]
    pub project: Option<String>,
    /// The installed app this thread belongs to (its agent's chat).
    #[serde(default)]
    pub app: Option<String>,
    /// Extra instructions for every reply (an app's agent brief).
    #[serde(default)]
    pub system: Option<String>,
    /// A thread with one of your agents.
    #[serde(default)]
    pub agent: Option<String>,
    /// A group: its agents, in turn order, and what it is for.
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub goal: Option<String>,
    /// A one-off ask (dreaming): never listed or written to disk.
    #[serde(skip)]
    pub hidden: bool,
}

/// Where a new thread lives: the plain chat list, a project (Pair) or an app.
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Scope {
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub goal: Option<String>,
    /// A group's name.
    #[serde(default)]
    pub title: Option<String>,
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
    pub project: Option<String>,
    pub app: Option<String>,
    pub agent: Option<String>,
    pub members: Vec<String>,
    /// Why its last reply failed (a usage limit, a crash), if it did.
    pub failed: Option<String>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Members a message names, as `@Name` or the bare name at a word boundary.
fn named_in(roster: &[crate::agents::Agent], text: &str) -> Vec<crate::agents::Agent> {
    let low = text.to_lowercase();
    let mut hits: Vec<(usize, crate::agents::Agent)> = roster
        .iter()
        .filter_map(|a| {
            let n = a.name.to_lowercase();
            let at = low.find(&format!("@{n}"))?;
            Some((at, a.clone()))
        })
        .collect();
    hits.sort_by_key(|(i, _)| *i);
    hits.into_iter().map(|(_, a)| a).collect()
}

fn is_pass(text: &str) -> bool {
    let t = text.trim().trim_matches(|c: char| c == '.' || c == '*' || c == '`').trim();
    t.eq_ignore_ascii_case("pass")
}

pub(crate) fn new_id() -> String {
    let n = now_ms();
    let r: u32 = rand_u32();
    format!("{n:x}{r:08x}")
}

pub(crate) fn rand_u32() -> u32 {
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

/// OpenUI's prompt for rich answers (charts, tables, steps, forms), set by
/// the chat face from the component library it renders with; None: plain
/// Markdown only (Settings → Chat → Rich answers).
// ponytail: one global for the one chat face; per-window if there are ever two.
pub static GENUI_PROMPT: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

const PLAN_MODE: &str = "# Plan mode\n\nThe user wants a plan, not changes. Read whatever you need, run read-only commands, then reply with a short plan: the files to change, what changes in each, and how to check it. Don't edit, create or delete any file.";

/// What a send needs from prefs, copied so a reply never holds the lock.
#[derive(Clone)]
pub struct Ctx {
    pub prefs: Prefs,
    pub memory: Option<Arc<crate::memory::Memory>>,
    pub agents: Option<Arc<crate::agents::Agents>>,
    /// Code chat's Plan mode: read the project and propose; change nothing.
    pub plan: bool,
}

/// One reply's circumstances: which agent writes it, and for a group the
/// transcript it answers instead of the thread's own history.
#[derive(Clone, Default)]
struct Call {
    agent: Option<crate::agents::Agent>,
    transcript: Option<String>,
    /// Carry on with a stopped reply instead of starting a new one.
    resume: bool,
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
                            m.interrupted = m.role == Role::Assistant;
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
        if let Some(t) = t.filter(|t| !t.hidden) {
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
            .filter(|t| !t.hidden)
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
                project: t.project.clone(),
                app: t.app.clone(),
                agent: t.agent.clone(),
                members: t.members.clone(),
                failed: t.messages.last().filter(|m| m.role != Role::User).and_then(|m| m.error.clone()),
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

    /// Read a thread without marking it read (background jobs).
    pub fn peek(&self, id: &str) -> Option<Thread> {
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
        self.create_in(route, Scope::default())
    }

    pub fn create_in(&self, route: Route, scope: Scope) -> Thread {
        let now = now_ms();
        let group = !scope.members.is_empty();
        let t = Thread {
            id: new_id(),
            title: scope
                .title
                .clone()
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| if group { "New group".into() } else { "New chat".into() }),
            created: now,
            updated: now,
            pinned: false,
            route,
            messages: vec![],
            cli_session: None,
            branched_from: None,
            project: scope.project.filter(|p| !p.is_empty()),
            app: scope.app,
            system: scope.system,
            agent: scope.agent,
            members: scope.members,
            goal: scope.goal.filter(|g| !g.trim().is_empty()),
            hidden: false,
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
            project: src.project.clone(),
            app: src.app.clone(),
            system: src.system.clone(),
            agent: src.agent.clone(),
            members: src.members.clone(),
            goal: src.goal.clone(),
            hidden: false,
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
            if t.members.is_empty() && (t.messages.is_empty() || t.title == "New chat") {
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
                author: None,
                author_name: None,
                trace: None,
                interrupted: false,
            });
            t.updated = now;
        })?;
        self.reply(id, ctx, mention(text))
    }

    /// One question, one answer, outside any chat: a thread that is never
    /// listed or saved, run on `route` with `system` as its only brief (no
    /// memory notes), then dropped. For background jobs such as dreaming.
    pub async fn ask(self: &Arc<Self>, route: Route, system: &str, prompt: &str, ctx: Ctx) -> Result<String> {
        let now = now_ms();
        let id = format!("ask-{}", new_id());
        let user = Msg::new(Role::User, prompt.to_string(), Status::Done);
        let mut reply = Msg::new(Role::Assistant, String::new(), Status::Streaming);
        reply.at = now;
        let mid = reply.id.clone();
        let t = Thread {
            id: id.clone(),
            title: "ask".into(),
            created: now,
            updated: now,
            pinned: false,
            route: route.clone(),
            messages: vec![user, reply],
            cli_session: None,
            branched_from: None,
            project: None,
            app: None,
            system: Some(system.to_string()),
            agent: None,
            members: vec![],
            goal: None,
            hidden: true,
        };
        self.threads.lock().unwrap().insert(id.clone(), t);
        let ctx = Ctx { memory: None, ..ctx };
        let res = self.run(&id, &mid, &route, &ctx, &Call::default()).await;
        let t = self.threads.lock().unwrap().remove(&id);
        let _ = std::fs::remove_dir_all(self.dir.join("scratch").join(&id));
        res?;
        let m = t.and_then(|t| t.messages.into_iter().find(|m| m.id == mid));
        match m {
            Some(m) if m.error.is_some() => Err(anyhow!(m.error.unwrap_or_default())),
            Some(m) => Ok(m.text),
            None => Err(anyhow!("no answer")),
        }
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
        let (own, agent_id, members) = self
            .threads
            .lock()
            .unwrap()
            .get(id)
            .map(|t| (t.route.clone(), t.agent.clone(), t.members.clone()))
            .ok_or_else(|| anyhow!("no such chat"))?;
        if !members.is_empty() {
            return self.reply_group(id, ctx, members);
        }
        // An agent's thread answers with the agent's model and brief.
        let agent = agent_id.and_then(|a| ctx.agents.as_ref()?.get(&a));
        let own = agent.as_ref().map(|a| a.route.clone()).unwrap_or(own);
        // An @mention hands this one reply to another CLI. The thread's own
        // CLI session then misses it, so the next reply gets the transcript.
        let via = via.filter(|v| v != &own);
        let mentioned = via.is_some();
        let route = via.unwrap_or(own);
        if mentioned {
            let _ = self.edit_thread(id, |t| t.cli_session = None);
        }
        let agent = if mentioned { None } else { agent };
        let mut msg = Msg::new(Role::Assistant, String::new(), Status::Sending);
        msg.model = route.model.clone();
        msg.via = mentioned.then(|| route.clone());
        msg.author = agent.as_ref().map(|a| a.id.clone());
        msg.author_name = agent.as_ref().map(|a| a.name.clone());
        let mid = msg.id.clone();
        self.edit_thread(id, |t| t.messages.push(msg))?;
        self.spawn_reply(id, mid, route, ctx, Call { agent, transcript: None, resume: false }, mentioned);
        Ok(())
    }

    /// Carry on with the last reply after it stopped (Backspace closed, or
    /// you pressed Stop): on the CLI's own session when there is one, else
    /// from the conversation plus what was written so far.
    pub fn resume(self: &Arc<Self>, id: &str, ctx: Ctx) -> Result<()> {
        if self.is_busy(id) {
            bail!("wait for the reply, or stop it first");
        }
        let (own, agent_id, members, last) = self
            .threads
            .lock()
            .unwrap()
            .get(id)
            .map(|t| (t.route.clone(), t.agent.clone(), t.members.clone(), t.messages.last().cloned()))
            .ok_or_else(|| anyhow!("no such chat"))?;
        if !members.is_empty() {
            bail!("in a group, send your message again instead");
        }
        let last = last
            .filter(|m| m.role == Role::Assistant && matches!(m.status, Status::Stopped | Status::Error))
            .ok_or_else(|| anyhow!("there's no stopped reply to carry on with"))?;
        let agent = agent_id.and_then(|a| ctx.agents.as_ref()?.get(&a));
        let mentioned = last.via.is_some();
        let route = last.via.clone().unwrap_or_else(|| agent.as_ref().map(|a| a.route.clone()).unwrap_or(own));
        let agent = if mentioned { None } else { agent };
        self.edit_thread(id, |t| {
            if let Some(m) = t.messages.last_mut() {
                m.interrupted = false;
                m.error = None;
                m.status = Status::Streaming;
                if !m.text.is_empty() && !m.text.ends_with("\n\n") {
                    m.text.push_str("\n\n");
                }
            }
        })?;
        self.spawn_reply(id, last.id, route, ctx, Call { agent, transcript: None, resume: true }, mentioned);
        Ok(())
    }

    fn spawn_reply(self: &Arc<Self>, id: &str, mid: String, route: Route, ctx: Ctx, call: Call, mentioned: bool) {
        let me = self.clone();
        let tid = id.to_string();
        let task = self.rt.spawn(async move {
            let res = me.run(&tid, &mid, &route, &ctx, &call).await;
            if mentioned {
                let _ = me.edit_thread(&tid, |t| t.cli_session = None);
            }
            me.running.lock().unwrap().remove(&tid);
            me.finish(&tid, &mid, res);
            me.unread.lock().unwrap().insert(tid);
            me.poke();
        });
        self.running
            .lock()
            .unwrap()
            .insert(id.to_string(), task.abort_handle());
        self.poke();
    }

    /// Mark a reply done or failed.
    fn finish(&self, tid: &str, mid: &str, res: Result<()>) {
        let _ = self.edit_thread(tid, |t| {
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
    }

    /// A group's turn: the agents the user named (or all of them, in order)
    /// each write their part, reading everything said so far. An agent with
    /// nothing to add says PASS and its turn leaves no message. Naming
    /// another member (@Name) gives them a turn too, up to a limit.
    fn reply_group(self: &Arc<Self>, id: &str, ctx: Ctx, members: Vec<String>) -> Result<()> {
        const MAX_TURNS: usize = 8;
        let agents = ctx.agents.clone().ok_or_else(|| anyhow!("agents are not available here"))?;
        let roster: Vec<crate::agents::Agent> = members.iter().filter_map(|m| agents.get(m)).collect();
        if roster.is_empty() {
            bail!("this group has no agents left; add some");
        }
        let last_user = self
            .thread(id)
            .and_then(|t| t.messages.iter().rev().find(|m| m.role == Role::User).map(|m| m.text.clone()))
            .unwrap_or_default();
        let named = named_in(&roster, &last_user);
        let mut queue: std::collections::VecDeque<crate::agents::Agent> =
            if named.is_empty() { roster.clone().into() } else { named.into() };
        let me = self.clone();
        let tid = id.to_string();
        let task = self.rt.spawn(async move {
            let mut turns = 0;
            while let Some(a) = queue.pop_front() {
                if turns >= MAX_TURNS {
                    break;
                }
                turns += 1;
                let mut msg = Msg::new(Role::Assistant, String::new(), Status::Sending);
                msg.author = Some(a.id.clone());
                msg.author_name = Some(a.name.clone());
                msg.model = a.route.model.clone();
                let mid = msg.id.clone();
                if me.edit_thread(&tid, |t| t.messages.push(msg)).is_err() {
                    break;
                }
                let call = Call {
                    resume: false,
                    transcript: Some(me.group_transcript(&tid, &a, &roster)),
                    agent: Some(a.clone()),
                };
                let res = me.run(&tid, &mid, &a.route, &ctx, &call).await;
                let failed = res.is_err();
                me.finish(&tid, &mid, res);
                let text = me
                    .thread(&tid)
                    .and_then(|t| t.messages.iter().find(|m| m.id == mid).map(|m| m.text.clone()))
                    .unwrap_or_default();
                if !failed && is_pass(&text) {
                    let _ = me.edit_thread(&tid, |t| t.messages.retain(|m| m.id != mid));
                    continue;
                }
                for other in named_in(&roster, &text) {
                    if other.id != a.id && !queue.iter().any(|q| q.id == other.id) {
                        queue.push_back(other);
                    }
                }
            }
            me.running.lock().unwrap().remove(&tid);
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

    /// What one member of a group reads before writing its turn.
    fn group_transcript(&self, tid: &str, a: &crate::agents::Agent, roster: &[crate::agents::Agent]) -> String {
        let Some(t) = self.thread(tid) else {
            return String::new();
        };
        let others: Vec<String> = roster
            .iter()
            .filter(|r| r.id != a.id)
            .map(|r| {
                let job: String = r.job.split(['.', '\n']).next().unwrap_or("").chars().take(120).collect();
                if job.trim().is_empty() { r.name.clone() } else { format!("{} ({})", r.name, job.trim()) }
            })
            .collect();
        let mut s = format!(
            "You are in a group chat called \"{}\"{}. With you: the user{}.\n\nThe conversation so far:\n\n",
            t.title,
            t.goal.as_deref().map(|g| format!(", whose goal is: {g}")).unwrap_or_default(),
            if others.is_empty() { String::new() } else { format!(", {}", others.join(", ")) }
        );
        for m in t.messages.iter().filter(|m| !(m.role == Role::Assistant && m.text.is_empty())) {
            let who = match (&m.role, &m.author_name) {
                (Role::User, _) => "User".to_string(),
                (_, Some(n)) => n.clone(),
                _ => "Assistant".to_string(),
            };
            let text: String = m.text.chars().take(4000).collect();
            s.push_str(&format!("[{who}]: {text}\n\n"));
        }
        s.push_str(&format!(
            "Write {}'s next message: your own part only, building on what the others said instead of repeating it. Keep it short unless the work needs more. To hand something to another member, mention them as @Name. If you have nothing useful to add right now, reply with exactly PASS.",
            a.name
        ));
        s
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
        // A reply to a specific message carries a quote of it, so the model
        // knows which part of the conversation it answers.
        for m in h.iter_mut() {
            let Some(q) = m.reply_to.as_ref().and_then(|r| t.messages.iter().find(|x| &x.id == r)) else {
                continue;
            };
            let who = if q.role == Role::User { "my earlier message" } else { "your earlier reply" };
            let snip: String = q.text.chars().take(400).collect();
            m.text = format!("> Replying to {who}: {}\n\n{}", snip.replace('\n', " "), m.text);
        }
        (h, t.cli_session.clone())
    }

    /// History for one reply: a group member reads the transcript it was
    /// given (as one message, no CLI session); everyone else the thread.
    fn history_for(&self, id: &str, call: &Call) -> (Vec<Msg>, Option<String>) {
        match &call.transcript {
            Some(t) => (vec![Msg::new(Role::User, t.clone(), Status::Done)], None),
            None => self.history(id),
        }
    }

    /// The thread's own brief (an app's), the agent's brief, and the memory
    /// notes for its project or agent, as one system prompt.
    fn system_text(&self, id: &str, ctx: &Ctx, call: &Call) -> Option<String> {
        let (sys, project) = {
            let ts = self.threads.lock().unwrap();
            let t = ts.get(id)?;
            (t.system.clone(), t.project.clone())
        };
        let brief = call
            .agent
            .as_ref()
            .and_then(|a| ctx.agents.as_ref().map(|ag| ag.brief(a)));
        // An agent's own notes live under the scope "agent:<id>".
        let scope = match &call.agent {
            Some(a) => Some(format!("agent:{}", a.id)),
            None => project,
        };
        let mem = ctx
            .memory
            .as_ref()
            .filter(|_| ctx.prefs.memory_on && call.agent.as_ref().is_none_or(|a| a.memory))
            .and_then(|m| m.context(scope.as_deref()));
        let parts: Vec<String> = [sys, brief, mem].into_iter().flatten().filter(|s| !s.trim().is_empty()).collect();
        (!parts.is_empty()).then(|| parts.join("\n\n"))
    }

    /// Pair: the project folder this thread's CLI works (and edits) in.
    fn project_of(&self, id: &str) -> Option<PathBuf> {
        let ts = self.threads.lock().unwrap();
        ts.get(id)
            .and_then(|t| t.project.clone())
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
    }

    async fn run(&self, id: &str, mid: &str, route: &Route, ctx: &Ctx, call: &Call) -> Result<()> {
        if route.kind == RouteKind::Cloud {
            return self.run_cloud(id, mid, ctx, route, call).await;
        }
        let req = self.request(id, route, ctx, call)?;
        // Groups answer a transcript: no session to resume or keep.
        let keep_session = call.transcript.is_none();
        let who = call.agent.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| route.provider.clone());
        let mut attrs = std::collections::BTreeMap::from([
            ("gen_ai.system".to_string(), json!(route.provider)),
            ("backspace.route".to_string(), json!(format!("{:?}", route.kind).to_lowercase())),
        ]);
        if let Some(m) = &req.model {
            attrs.insert("gen_ai.request.model".into(), json!(m));
        }
        if let Some(a) = &call.agent {
            attrs.insert("gen_ai.agent.name".into(), json!(a.name));
            attrs.insert("gen_ai.agent.id".into(), json!(a.id));
        }
        let input = req.history.last().map(|t| t.text.clone()).unwrap_or_default();
        let mut rec = crate::trace::Recorder::start(id, mid, &format!("reply · {who}"), attrs, &input);
        // The final answer counts only if nothing streamed in this run
        // (a resumed reply already has text of its own).
        let mut streamed = false;
        let res = backspace_runner::run(&self.http, &req, &mut |e| {
            rec.on(&e);
            let e = match e {
                RunEvent::Final { text } if !streamed && !text.is_empty() => RunEvent::Text { text },
                RunEvent::Final { .. } => return,
                e => e,
            };
            if matches!(e, RunEvent::Text { .. } | RunEvent::Replace { .. }) {
                streamed = true;
            }
            self.on_event(id, mid, keep_session, e)
        })
        .await;
        let t = rec.finish(res.as_ref().err().map(|e| e.to_string()));
        if crate::trace::save(&self.dir, &t).is_ok() {
            let tid = t.trace_id.clone();
            self.with_msg(id, mid, |m| m.trace = Some(tid));
        }
        let cfg = ctx.prefs.tracing.clone();
        if !cfg.endpoint.trim().is_empty() {
            let http = self.http.clone();
            tokio::spawn(async move {
                let _ = crate::trace::export(&http, &cfg, &t).await;
            });
        }
        res
    }

    /// A reply's trace.
    pub fn trace(&self, id: &str) -> Option<crate::trace::Trace> {
        crate::trace::load(&self.dir, id)
    }

    // ------------------------------------------------------------ runner

    /// This thread's history as the runner's turns.
    fn turns(&self, id: &str, hist: &[Msg]) -> Vec<Turn> {
        hist.iter()
            .map(|m| Turn {
                user: m.role == Role::User,
                text: m.text.clone(),
                who: m.author_name.clone().or_else(|| m.via.as_ref().map(|v| v.provider.clone())),
                files: m
                    .attachments
                    .iter()
                    .filter_map(|a| {
                        Some(RunFile { name: a.name.clone(), mime: a.mime.clone(), path: self.attachment_path(id, &a.id).ok()? })
                    })
                    .collect(),
            })
            .collect()
    }

    /// Everything a reply needs, for the runner: who answers, the brief and
    /// memory, the history, and for a CLI where it runs and what it may use.
    fn request(&self, id: &str, route: &Route, ctx: &Ctx, call: &Call) -> Result<Request> {
        let target = match route.kind {
            RouteKind::Cli => {
                let bin_name = match route.provider.as_str() {
                    "cursor" => "cursor-agent",
                    other => other,
                };
                let bin = crate::harnesses::which(bin_name)
                    .or_else(|| crate::harnesses::which("agy").filter(|_| route.provider == "antigravity"))
                    .ok_or_else(|| anyhow!("`{bin_name}` is not installed or not on PATH"))?;
                Target::Cli { provider: route.provider.clone(), bin }
            }
            RouteKind::Local => Target::Ollama { url: ctx.prefs.ollama_url.trim_end_matches('/').to_string() },
            RouteKind::Router => {
                let r = ctx
                    .prefs
                    .routers
                    .iter()
                    .find(|r| r.id == route.provider)
                    .cloned()
                    .ok_or_else(|| anyhow!("that router was removed in Settings"))?;
                Target::OpenAi { base: r.base_url, key: r.api_key }
            }
            RouteKind::A2a => {
                let a = ctx
                    .prefs
                    .remote_agents
                    .iter()
                    .find(|a| a.id == route.provider)
                    .cloned()
                    .ok_or_else(|| anyhow!("that agent was removed in Settings"))?;
                Target::A2a { url: a.url, token: Some(a.token).filter(|t| !t.is_empty()) }
            }
            RouteKind::Cloud => bail!("Cloud replies don't go through the runner"),
        };
        let (hist, session) = self.history_for(id, call);
        let mut turns = self.turns(id, &hist);
        if call.resume {
            const GO_ON: &str = "You were cut off before you finished (the app closed, or the user paused you). \
                Carry on from exactly where you stopped. Don't repeat what you already said or did; \
                check the state of files or commands first if you need to.";
            if session.is_some() {
                turns = vec![Turn::user(GO_ON)];
            } else {
                let partial = self.peek(id).and_then(|t| t.messages.last().map(|m| m.text.trim().to_string())).unwrap_or_default();
                if !partial.is_empty() {
                    turns.push(Turn { user: false, text: partial, who: None, files: vec![] });
                }
                turns.push(Turn::user(GO_ON));
            }
        }
        let mut req = Request::new(target, turns);
        req.model = route.model.clone().filter(|m| !m.is_empty());
        req.system = self.system_text(id, ctx, call);
        // Rich answers in plain chats (not coding agents or Pair, which work in files).
        let plain = call.agent.is_none() && self.threads.lock().unwrap().get(id).is_some_and(|t| t.project.is_none() && t.agent.is_none());
        if let Some(g) = GENUI_PROMPT.read().unwrap().clone().filter(|_| plain) {
            let rich = format!("# Rich answers

When structure helps the reader (a comparison, steps, a table, a chart, a form, follow-up choices), put it in a fenced ```openui block written in OpenUI Lang; keep the rest in Markdown. Most answers need none.

{g}");
            req.system = Some(match req.system.take() {
                Some(s) => format!("{s}

{rich}"),
                None => rich,
            });
        }
        req.session = session;
        if route.kind != RouteKind::Cli {
            return Ok(req);
        }
        // Pair: in the project, allowed to edit it. An agent: in its own
        // folder, allowed to edit it. Otherwise a scratch folder per thread.
        let agent_dir = call.agent.as_ref().and_then(|a| ctx.agents.as_ref().map(|ag| ag.workspace(&a.id)));
        let computer = call.agent.as_ref().and_then(|a| ctx.agents.as_ref().and_then(|ag| ag.computer_env(a)));
        let project = agent_dir.or_else(|| self.project_of(id));
        // Code chat runs the CLI the way the picker says, like the agents do.
        if call.agent.is_none() && project.is_some() {
            req.permission = Some(ctx.prefs.cli_permission.clone()).filter(|p| !p.is_empty());
            req.effort = ctx.prefs.cli_effort.clone();
        }
        req.edit = project.is_some() && !ctx.plan;
        if ctx.plan {
            req.permission = Some("plan".into());
            req.system = Some(format!("{}{PLAN_MODE}", req.system.take().map(|s| s + "\n\n").unwrap_or_default()));
        }
        req.cwd = match project {
            Some(p) => p,
            None => {
                let scratch = self.dir.join("scratch").join(id);
                std::fs::create_dir_all(&scratch)?;
                scratch
            }
        };
        req.env.push(("PATH".into(), crate::harnesses::path_env().to_string_lossy().to_string()));
        if let Some(c) = &computer {
            req.env.push((crate::agents::ENV_COMPUTER.into(), serde_json::to_string(c)?));
        }
        // Agents and Pair may search memory and save to their own notes.
        let mem_scope = match &call.agent {
            Some(a) if a.memory => Some(format!("agent:{}", a.id)),
            Some(_) => None,
            None => self.threads.lock().unwrap().get(id).and_then(|t| t.project.clone()),
        }
        .filter(|_| ctx.prefs.memory_on && ctx.memory.is_some());
        if let Some(s) = &mem_scope {
            req.env.push((crate::mcp::ENV_MEMORY.into(), s.clone()));
        }
        // Installed apps' tools, the agent's computer and memory, through `backspace mcp`.
        if !crate::mcp::app_tools().is_empty() || computer.is_some() || mem_scope.is_some() {
            if let Ok(exe) = std::env::current_exe() {
                req.mcp = Some(json!({"mcpServers": {"backspace": {"command": exe.display().to_string(), "args": ["mcp"]}}}));
                req.mcp_allow.push("mcp__backspace".into());
            }
        }
        req.add_dirs = call.agent.iter().flat_map(|a| a.shared.iter()).map(|d| crate::agents::expand(d)).collect();
        Ok(req)
    }

    /// Apply one runner event to the reply being written.
    fn on_event(&self, id: &str, mid: &str, keep_session: bool, e: RunEvent) {
        // Each tool call is a checkpoint (before and after): what's written so far is
        // saved, so a reply cut off by quitting can be resumed from there.
        if matches!(e, RunEvent::ToolStart { .. } | RunEvent::ToolEnd { .. }) {
            self.save(id);
        }
        match e {
            RunEvent::Text { text } => self.append(id, mid, &text),
            RunEvent::Break => {
                let need = self
                    .threads
                    .lock()
                    .unwrap()
                    .get(id)
                    .and_then(|t| t.messages.iter().find(|m| m.id == mid))
                    .is_some_and(|m| !m.text.is_empty() && !m.text.ends_with("\n\n"));
                if need {
                    self.append(id, mid, "\n\n");
                }
            }
            RunEvent::Replace { text } => self.set_text(id, mid, &text),
            // Turned into text by `run` when nothing else came.
            RunEvent::Final { .. } => {}
            RunEvent::Model { model } => self.with_msg(id, mid, |m| m.model = Some(model)),
            RunEvent::Session { id: s } => {
                if keep_session {
                    let _ = self.edit_thread(id, |t| t.cli_session = Some(s));
                }
            }
            RunEvent::Cost { usd } => self.with_msg(id, mid, |m| m.cost_usd = Some(usd)),
            RunEvent::Usage { .. } | RunEvent::ToolStart { .. } | RunEvent::ToolEnd { .. } => {}
        }
    }

    // ------------------------------------------------------------ Cloud

    async fn run_cloud(&self, id: &str, mid: &str, ctx: &Ctx, route: &Route, call: &Call) -> Result<()> {
        let cloud = &ctx.prefs.cloud;
        if cloud.token.is_empty() {
            bail!("Sign up for Backspace Cloud in Settings → Plan first");
        }
        let model = route.model.clone().unwrap_or_default();
        let (hist, _) = self.history_for(id, call);
        let resp = self
            .http
            .post(format!("{}/v1/chat", cloud.url.trim_end_matches('/')))
            .bearer_auth(&cloud.token)
            .json(&json!({ "model": model, "messages": backspace_runner::openai_messages(self.system_text(id, ctx, call).as_deref(), &self.turns(id, &hist)) }))
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
                    author: None,
                    author_name: None,
                    trace: None,
                    interrupted: false,
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
        assert_eq!(backspace_runner::plain("\u{1b}[1mbold\u{1b}[0m\r\n"), "bold\n");
    }
}
