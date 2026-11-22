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

use anyhow::{anyhow, bail, Context, Result};
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
    /// The reasoning the target showed while it worked, when it shows any.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub thinking: String,
    /// What the reply did while it was written, in order: the reasoning, then each tool call, with
    /// when each started and ended (unix ms). Written live, so the view can show a step running.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acts: Vec<Act>,
    /// Where in `text` (UTF-16 units, as the face counts) one assistant turn
    /// ended and the next began, i.e. after a tool call. The face draws each
    /// stretch as its own bubble.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub breaks: Vec<usize>,
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
    /// Why nothing is arriving right now, when the target says so (OpenCode retrying a provider
    /// that is throttling). Live only; cleared by the next text, thought or tool call.
    #[serde(skip)]
    pub wait: Option<String>,
    /// Sent in Plan mode: its reply, and any retry, resume or edit of it, reads and proposes only.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub plan: bool,
    /// The spec gate's answer, on a Plan reply that wrote `SPEC.md`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<crate::spec::Verdict>,
}

/// The reasoning ends when the reply starts writing or calls a tool.
fn end_thinking(m: &mut Msg) {
    if let Some(a) = m.acts.last_mut().filter(|a| a.name == "Thinking" && a.end.is_none()) {
        a.end = Some(now_ms());
    }
}

/// The branch checked out at `dir` or the repo above it, read from git's HEAD file (no process:
/// `list` runs on every change). A detached HEAD reads as its short commit; a worktree's `.git`
/// file points at its HEAD.
fn git_branch(dir: &str) -> Option<String> {
    let mut at = Some(Path::new(dir));
    while let Some(d) = at {
        let git = d.join(".git");
        let head = if git.is_file() {
            let to = std::fs::read_to_string(&git).ok()?;
            let gd = Path::new(to.trim().strip_prefix("gitdir:")?.trim());
            std::fs::read_to_string(if gd.is_absolute() { gd.to_path_buf() } else { d.join(gd) }.join("HEAD")).ok()
        } else {
            std::fs::read_to_string(git.join("HEAD")).ok()
        };
        if let Some(h) = head {
            let h = h.trim();
            return Some(h.strip_prefix("ref: refs/heads/").map_or_else(|| h.chars().take(7).collect(), str::to_string)).filter(|b| !b.is_empty());
        }
        at = d.parent();
    }
    None
}

/// The maker of a model, from its id or the CLI that ran it ("opus" in Claude Code is Claude's).
pub fn vendor(model: Option<&str>, provider: &str) -> String {
    let m = model.unwrap_or("").to_lowercase();
    let has = |k: &[&str]| k.iter().any(|k| m.contains(k));
    let named = if has(&["claude", "anthropic", "opus", "sonnet", "haiku", "fable"]) {
        "Claude"
    } else if has(&["gpt", "openai", "codex"]) || m.starts_with("o3") || m.starts_with("o4") {
        "GPT"
    } else if has(&["gemini", "gemma"]) {
        "Gemini"
    } else if has(&["grok", "x-ai", "xai"]) {
        "Grok"
    } else if has(&["deepseek"]) {
        "DeepSeek"
    } else if has(&["qwen"]) {
        "Qwen"
    } else if has(&["kimi", "moonshot"]) {
        "Kimi"
    } else if has(&["glm", "zhipu", "z-ai"]) {
        "GLM"
    } else if has(&["mistral", "devstral", "codestral"]) {
        "Mistral"
    } else if has(&["llama"]) {
        "Llama"
    } else {
        ""
    };
    if !named.is_empty() {
        return named.into();
    }
    match provider {
        "claude" => "Claude".into(),
        "codex" => "GPT".into(),
        "gemini" => "Gemini".into(),
        "grok" => "Grok".into(),
        // A model we do not know by name: its own name, else the CLI's.
        _ if !m.is_empty() => model.unwrap_or("").rsplit('/').next().unwrap_or("").to_string(),
        "opencode" => "OpenCode".into(),
        p => p.to_string(),
    }
}

/// The makers behind a stream's replies, the stream's own first: "Claude", "Claude + GPT",
/// "Claude + more". A stream with no replies yet names who will answer.
fn models_label(t: &Thread) -> String {
    let mut seen: Vec<String> = Vec::new();
    for m in t.messages.iter().filter(|m| m.role == Role::Assistant) {
        let r = m.via.as_ref().unwrap_or(&t.route);
        let v = vendor(m.model.as_deref().or(r.model.as_deref()), &r.provider);
        if !v.is_empty() && !seen.contains(&v) {
            seen.push(v);
        }
    }
    if seen.is_empty() {
        seen.push(vendor(t.route.model.as_deref(), &t.route.provider));
    }
    match seen.len() {
        1 => seen.remove(0),
        2 => format!("{} + {}", seen[0], seen[1]),
        _ => format!("{} + more", seen[0]),
    }
}

/// The file a tool call names in its input (JSON text). The runner may have cut the text, so it
/// is read as a string after the key when the whole does not parse.
fn act_path(input: &str) -> Option<String> {
    const KEYS: [&str; 4] = ["file_path", "filePath", "notebook_path", "path"];
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(input) {
        return KEYS.iter().find_map(|k| v.get(*k)?.as_str()).filter(|p| !p.is_empty()).map(str::to_string);
    }
    KEYS.iter().find_map(|k| {
        let rest = &input[input.find(&format!("\"{k}\":\""))? + k.len() + 4..];
        let mut out = String::new();
        let mut esc = false;
        for ch in rest.chars() {
            match (esc, ch) {
                (true, c) => {
                    out.push(if c == 'n' { '\n' } else { c });
                    esc = false;
                }
                (false, '\\') => esc = true,
                (false, '"') => return Some(out).filter(|p| !p.is_empty()),
                (false, c) => out.push(c),
            }
        }
        None
    })
}

/// One step of a reply: the reasoning ("Thinking") or a tool call.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Act {
    /// The tool call's id; empty for the reasoning.
    #[serde(default)]
    pub id: String,
    /// "Thinking", or the tool's name ("Read", "Bash", ...).
    pub name: String,
    /// The tool's input as the runner reported it (JSON text, short), for a one-line label.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub input: String,
    /// The file the call works on, taken from the whole input before `input` is cut: a write's
    /// path came after its content and was lost to the cut ("Edited a file").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// What the call printed, trimmed to the tail, once it ended; for "Thinking", its reasoning.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub output: String,
    pub start: u64,
    #[serde(default)]
    pub end: Option<u64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub error: bool,
    /// How much of the reply's text (bytes) was written when this step began, so the view can set
    /// the step between the paragraphs it fell between.
    #[serde(default)]
    pub at: usize,
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
            thinking: String::new(),
            acts: Vec::new(),
            breaks: Vec::new(),
            edited: false,
            via: None,
            author: None,
            author_name: None,
            trace: None,
            interrupted: false,
            wait: None,
            plan: false,
            spec: None,
        }
    }
}

/// How long a thread lives. A chat normally outlives the app; these are the two
/// ways it does not.
///
/// `Ghost` is never written to disk at all and never appears in the list, so
/// closing it destroys it. `Temporal` is written like any other thread but
/// forgets itself a while after its last message: a chat you want for an hour,
/// or for a conversation about something sensitive. Both only ever hold the
/// in-memory log, and neither is a privacy boundary — a reply that went to a
/// provider over the network has already left this machine.
#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Life {
    #[default]
    Keep,
    Ghost,
    Temporal,
}

impl Life {
    pub fn is_ghost(self) -> bool {
        matches!(self, Life::Ghost)
    }
    /// Ghost threads are never saved, which is what makes them disappear.
    pub fn saves(self) -> bool {
        !self.is_ghost()
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
    /// Put away from the sidebar, not deleted.
    #[serde(default)]
    pub archived: bool,
    pub route: Route,
    pub messages: Vec<Msg>,
    /// How long this thread lives (see `Life`). Absent in older files: Keep.
    #[serde(default)]
    pub life: Life,
    /// Epoch ms after which a `Temporal` thread forgets itself. `None` means it
    /// does not expire. Refreshed by every message, so it counts from the last
    /// thing said rather than from when the thread started.
    #[serde(default)]
    pub expires_at: Option<u64>,
    /// For `Life::Temporal`: the TTL to re-arm from the last message.
    #[serde(default)]
    pub ttl_minutes: Option<u64>,
    /// The CLI's own session, for CLIs that can resume one (Claude).
    #[serde(default)]
    pub cli_session: Option<String>,
    /// Thread this one was branched from.
    #[serde(default)]
    pub branched_from: Option<String>,
    /// A side stream: the main stream it sits beside (board "Side streams A"). Side streams
    /// share the main's tab group and sidebar row; they never nest, so this is always a main.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_of: Option<String>,
    /// Pair: the project folder a CLI works in, with leave to edit it.
    #[serde(default)]
    pub project: Option<String>,
    /// The folder this stream is filed in (a `folders::Folder` id); None is
    /// unfiled. Only a way to organise the list: it says nothing about where
    /// the CLI runs, which is `project`. Absent in older files: unfiled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
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
    /// `/goal pause`: the goal stays but the stream stops carrying on by itself.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub goal_paused: bool,
    /// Never listed or written to disk: a one-off ask (`dreaming`), or a ghost
    /// chat. `Life::Ghost` turns this on for the whole session.
    #[serde(skip)]
    pub hidden: bool,
}

/// What a side stream starts knowing (`Chats::side`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SideCtx {
    None,
    #[default]
    LastReply,
    Whole,
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
    /// How long the new thread lives. Ghost threads are never saved.
    #[serde(default)]
    pub life: Life,
    /// For `Life::Temporal`: minutes until it forgets itself, counted from its
    /// last message. Ignored for the other kinds.
    #[serde(default)]
    pub ttl_minutes: Option<u64>,
}

/// The sidebar row.
#[derive(Serialize, Clone, Debug)]
pub struct ThreadInfo {
    pub id: String,
    pub title: String,
    /// When the thread was added (ms), for sorting and grouping the sidebar.
    pub created: u64,
    pub updated: u64,
    pub pinned: bool,
    /// Put away from the sidebar (`Thread::archived`).
    pub archived: bool,
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
    /// A ghost chat: listed while you are in it, gone when you leave.
    pub ghost: bool,
    /// A temporal chat: when it forgets itself, for the countdown.
    pub expires_at: Option<u64>,
    /// A side stream's main stream (`Thread::side_of`).
    pub side_of: Option<String>,
    /// The branch checked out in the stream's folder; None when it has no folder or no repo.
    pub branch: Option<String>,
    /// Who wrote its replies, by maker: "Claude", "Claude + GPT", "Claude + more".
    pub models: String,
}

/// See `Chats::activity`.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Activity {
    pub prompts: Vec<Prompt>,
    /// Epoch hours (ms / 3,600,000) with at least one message.
    pub hours: Vec<u64>,
    pub sessions: usize,
}

/// One message you (or an agent, `agent`) sent, and the provider it went to.
#[derive(Serialize, Clone, Debug)]
pub struct Prompt {
    pub at: u64,
    pub provider: String,
    pub agent: bool,
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

/// Written by, or sent from, another harness tile (relay.rs).
fn is_tile(m: &Msg) -> bool {
    m.author.as_deref().is_some_and(|a| a.starts_with("tile:"))
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
    /// Harness tiles (relay.rs): letters waiting for a busy tile, per thread.
    inbox: Mutex<HashMap<String, std::collections::VecDeque<Inbound>>>,
    /// The letter a tile's running turn answers, so its answer can go back.
    answering: Mutex<HashMap<String, Inbound>>,
    /// Hand-offs so far in the chain each thread's latest turn belongs to;
    /// a turn the user started is 0.
    hops: Mutex<HashMap<String, u8>>,
}

/// A message from another tile, waiting or being answered.
#[derive(Clone, Debug)]
struct Inbound {
    /// The sending tile's thread.
    from: String,
    text: String,
    /// Send the answer back to `from`.
    reply: bool,
    /// Instead of a turn for `from`, the answer is posted in its thread (an
    /// @mention the user typed there).
    as_note: bool,
    hops: u8,
    /// Who first asked and their exact words, when this letter is a hand-on.
    origin: Option<(String, String)>,
}

/// OpenUI's prompt for rich answers (charts, tables, steps, forms), set by
/// the chat face from the component library it renders with; None: plain
/// Markdown only (Settings → Chat → Rich answers).
// ponytail: one global for the one chat face; per-window if there are ever two.
pub static GENUI_PROMPT: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

/// What a reply says when its stream's goal is met (`/goal`).
pub const GOAL_MET: &str = "GOAL MET";
/// How a stream with a goal is told to carry on; these turns count toward [`GOAL_TURNS`].
pub const GOAL_NUDGE: &str = "↻ Keep going toward the goal:";
/// Turns a goal may take on its own, in a row, before it pauses for the user.
pub const GOAL_TURNS: usize = 8;

const ASK_MODE: &str = "# Ask mode\n\nThe user is asking a question, not asking for changes. Read whatever you need and answer it. Don't edit, create or delete any file, and don't run commands.";

const PLAN_MODE: &str = "# Plan mode\n\nThe user wants a plan, not changes. Read whatever you need, run read-only commands, then reply with a short plan: the files to change, what changes in each, and how to check it. Don't edit, create or delete any file.";

/// What a send needs from prefs, copied so a reply never holds the lock.
#[derive(Clone)]
pub struct Ctx {
    pub prefs: Prefs,
    pub memory: Option<Arc<crate::memory::Memory>>,
    pub agents: Option<Arc<crate::agents::Agents>>,
    /// Code chat's Plan mode: read the project and propose; change nothing.
    pub plan: bool,
    /// Ask mode (Ask · Plan · Act, Charles 22 Nov): answer from what it reads; no edits, no
    /// commands.
    pub ask: bool,
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

/// One piece of a context window, for the agent page's map of it.
#[derive(Serialize, Clone, Debug)]
pub struct ContextPart {
    /// "Instructions", "Memory", "Conversation" or "Files".
    pub group: String,
    pub label: String,
    pub tokens: u64,
}

/// What a thread's next reply reads, and the window it reads it into.
#[derive(Serialize, Clone, Debug)]
pub struct ContextMap {
    /// The model's window in tokens, as far as the app knows it.
    pub window: u64,
    pub model: String,
    pub parts: Vec<ContextPart>,
    pub last_input: Option<u64>,
}

/// Four characters a token: close enough for English and code to size a map.
pub fn estimate_tokens(text: &str) -> u64 {
    (text.chars().count() as u64).div_ceil(4)
}

/// A model's context window, from its name or its provider's usual one.
pub fn window_of(route: &Route) -> u64 {
    let m = route.model.as_deref().unwrap_or("").to_lowercase();
    let p = route.provider.to_lowercase();
    if m.contains("[1m]") || m.contains("gemini") || p == "gemini" {
        1_000_000
    } else if m.contains("gpt-5") || p == "codex" {
        400_000
    } else if m.contains("claude") || m.contains("opus") || m.contains("sonnet") || m.contains("haiku") || p == "claude" {
        200_000
    } else {
        128_000
    }
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
            inbox: Mutex::new(HashMap::new()),
            answering: Mutex::new(HashMap::new()),
            hops: Mutex::new(HashMap::new()),
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
        if let Some(t) = t.filter(|t| !t.hidden && t.life.saves()) {
            if let Ok(s) = serde_json::to_string_pretty(&t) {
                let _ = std::fs::write(self.path(id), s);
            }
        }
    }

    /// What a thread's life means for `updated` changing: keep its deadline
    /// moving, or nothing at all.
    fn touched(t: &mut Thread) {
        if t.life == Life::Temporal {
            if let Some(ttl) = t.ttl_minutes {
                t.expires_at = Some(now_ms() + ttl.saturating_mul(60_000));
            }
        }
    }

    /// Forget every expired temporal thread, and return the ones that went.
    ///
    /// Called on a timer by the shell and once when the app opens, so a chat
    /// that expired while nothing was running is gone before it is listed.
    pub fn purge_expired(&self) -> Vec<String> {
        self.purge_at(now_ms())
    }

    /// `purge_expired` with the clock given, so a test can step past a deadline
    /// without waiting for it.
    pub fn purge_at(&self, now: u64) -> Vec<String> {
        let gone: Vec<String> = self
            .threads
            .lock()
            .unwrap()
            .values()
            .filter(|t| t.life == Life::Temporal && t.expires_at.is_some_and(|e| e <= now))
            .map(|t| t.id.clone())
            .collect();
        for id in &gone {
            self.threads.lock().unwrap().remove(id);
            let _ = std::fs::remove_file(self.path(id));
            // Its attachments go with it: the files are the conversation.
            let _ = std::fs::remove_dir_all(self.files_dir(id));
        }
        if !gone.is_empty() {
            self.poke();
        }
        gone
    }

    /// Forget every ghost thread, and return the ones that went.
    ///
    /// A ghost is never on disk, so there is nothing to purge after a restart —
    /// but one can still be in memory when the mode is entered again (a delete
    /// that failed, a second window over the same core). Whirl scrubs its
    /// incognito threads on the way in for the same reason.
    pub fn purge_ghosts(&self) -> Vec<String> {
        let gone: Vec<String> = self
            .threads
            .lock()
            .unwrap()
            .values()
            .filter(|t| t.life.is_ghost())
            .map(|t| t.id.clone())
            .collect();
        for id in &gone {
            let _ = self.delete(id);
        }
        gone
    }

    /// Give a thread a bounded life, or take the bound away. Ghost hides it
    /// from the list at once; Keep on a temporary thread stops it expiring.
    pub fn set_life(&self, id: &str, life: Life, ttl_minutes: Option<u64>) -> Result<Thread> {
        let t = {
            let mut ts = self.threads.lock().unwrap();
            let t = ts.get_mut(id).context("no such chat")?;
            t.life = life;
            t.ttl_minutes = match life {
                Life::Temporal => Some(ttl_minutes.unwrap_or(60).max(1)),
                _ => None,
            };
            t.hidden = life.is_ghost();
            if life == Life::Temporal {
                t.expires_at = Some(now_ms() + t.ttl_minutes.unwrap().saturating_mul(60_000));
            } else {
                t.expires_at = None;
            }
            t.clone()
        };
        // Ghost: the file that was already there has to go, or it would come
        // back the next time the app opens.
        if !life.saves() {
            let _ = std::fs::remove_file(self.path(id));
        } else {
            self.save(id);
        }
        self.poke();
        Ok(t)
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
                created: t.created,
                updated: t.updated,
                pinned: t.pinned,
                archived: t.archived,
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
                ghost: t.life.is_ghost(),
                side_of: t.side_of.clone(),
                expires_at: if t.life == Life::Temporal { t.expires_at } else { None },
                branch: t.project.as_deref().and_then(git_branch),
                models: models_label(t),
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

    /// What happened since `since` (epoch ms), for Home's "Last 30 days" card:
    /// every prompt with who answered it, the hours anything was said in, and
    /// how many threads were active. Days are left to the caller, which knows
    /// the local time zone.
    pub fn activity(&self, since: u64) -> Activity {
        let mut a = Activity::default();
        let mut hours = std::collections::BTreeSet::new();
        for t in self.threads.lock().unwrap().values().filter(|t| !t.hidden) {
            let mut live = false;
            for m in t.messages.iter().filter(|m| m.at >= since) {
                live = true;
                hours.insert(m.at / 3_600_000);
                if m.role == Role::User {
                    a.prompts.push(Prompt {
                        at: m.at,
                        provider: m.via.as_ref().unwrap_or(&t.route).provider.clone(),
                        agent: m.author.is_some(),
                    });
                }
            }
            if live {
                a.sessions += 1;
            }
        }
        a.prompts.sort_by_key(|p| p.at);
        a.hours = hours.into_iter().collect();
        a
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
        let mut t = Thread {
            id: new_id(),
            title: scope
                .title
                .clone()
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| if group { "New group".into() } else { "New chat".into() }),
            created: now,
            updated: now,
            pinned: false,
            archived: false,
            route,
            messages: vec![],
            cli_session: None,
            branched_from: None,
            side_of: None,
            project: scope.project.filter(|p| !p.is_empty()),
            folder: None,
            app: scope.app,
            system: scope.system,
            agent: scope.agent,
            members: scope.members,
            goal: scope.goal.filter(|g| !g.trim().is_empty()),
            goal_paused: false,
            life: scope.life,
            expires_at: None,
            ttl_minutes: if scope.life == Life::Temporal {
                Some(scope.ttl_minutes.unwrap_or(60).max(1))
            } else {
                None
            },
            // A ghost chat is hidden from the moment it exists, so nothing ever
            // offers to reopen it.
            hidden: scope.life.is_ghost(),
        };
        if t.life == Life::Temporal {
            t.expires_at = Some(now + t.ttl_minutes.unwrap_or(60).saturating_mul(60_000));
        }
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

    /// Point a stream at a project folder (the folder button on a stream that has none). The CLI's
    /// own session is dropped: a resumed session belongs to the folder it ran in, so the next turn
    /// starts a fresh one with the conversation so far.
    pub fn set_project(&self, id: &str, project: Option<String>) -> Result<()> {
        self.edit_thread(id, |t| {
            if t.project != project {
                t.project = project;
                t.cli_session = None;
            }
        })
    }

    pub fn pin(&self, id: &str, pinned: bool) -> Result<()> {
        self.edit_thread(id, |t| t.pinned = pinned)
    }

    /// Put a stream away from the sidebar, or bring it back. Nothing is deleted.
    pub fn archive(&self, id: &str, archived: bool) -> Result<()> {
        self.edit_thread(id, |t| t.archived = archived)
    }

    /// Clear a stream's unread mark without opening it.
    pub fn mark_read(&self, id: &str) {
        if self.unread.lock().unwrap().remove(id) {
            self.poke();
        }
    }

    /// File a thread in a folder, or take it out. Only the folder changes: not
    /// `project` (where a CLI runs), and not `updated` either, since filing is
    /// not activity and must not reorder the list (pin and rename leave it too).
    /// Whether the folder exists is for `folders::Folders` to check; that is
    /// why this is not public.
    pub(crate) fn set_folder(&self, id: &str, folder: Option<String>) -> Result<()> {
        {
            let mut ts = self.threads.lock().unwrap();
            let t = ts.get_mut(id).ok_or_else(|| anyhow!("no such chat"))?;
            if t.hidden || !t.life.saves() {
                bail!("a ghost chat is never saved, so it cannot be filed");
            }
            if t.folder == folder {
                return Ok(());
            }
            t.folder = folder;
        }
        self.save(id);
        self.poke();
        Ok(())
    }

    /// Is there a saved thread with this id that can be filed?
    pub(crate) fn can_file(&self, id: &str) -> bool {
        self.threads.lock().unwrap().get(id).is_some_and(|t| !t.hidden && t.life.saves())
    }

    /// Take every thread out of `folder` (it was deleted). The threads stay.
    pub(crate) fn unfile_all(&self, folder: &str) {
        let filed: Vec<String> = self
            .threads
            .lock()
            .unwrap()
            .values()
            .filter(|t| t.folder.as_deref() == Some(folder))
            .map(|t| t.id.clone())
            .collect();
        for id in filed {
            let _ = self.edit_thread(&id, |t| t.folder = None);
        }
    }

    /// `list()` with each thread's folder id beside it, for `folders.rs`
    /// (`ThreadInfo` is what the shells build rows from, and stays as it is).
    pub(crate) fn list_with_folder(&self) -> Vec<(ThreadInfo, Option<String>)> {
        let infos = self.list();
        let ts = self.threads.lock().unwrap();
        infos
            .into_iter()
            .map(|i| {
                let folder = ts.get(&i.id).and_then(|t| t.folder.clone());
                (i, folder)
            })
            .collect()
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

    /// A side stream beside `id` (board "Side streams A"): same CLI, model, folder and filing,
    /// its own CLI session, so the main never waits on it. `ctx` is what it starts knowing:
    /// nothing, the main's last reply, or the whole conversation. The caller sends the question.
    pub fn side(&self, id: &str, question: &str, ctx: SideCtx) -> Result<Thread> {
        let src = self.thread(id).ok_or_else(|| anyhow!("no such chat"))?;
        // A side of a side sits beside the same main: one level, one group.
        let main = match &src.side_of {
            Some(m) => self.thread(m).unwrap_or_else(|| src.clone()),
            None => src.clone(),
        };
        let from = if src.side_of.is_some() { &src } else { &main };
        let messages = match ctx {
            SideCtx::None => Vec::new(),
            SideCtx::LastReply => from.messages.iter().rev().find(|m| m.role != Role::User && !m.text.is_empty()).cloned().into_iter().collect(),
            SideCtx::Whole => from.messages.clone(),
        };
        let mut title: String = question.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(40).collect();
        if title.is_empty() {
            title = "Side stream".into();
        }
        let now = now_ms();
        let mut t = self.create_in(from.route.clone(), Scope { project: from.project.clone(), title: Some(title), ..Scope::default() });
        t.side_of = Some(main.id.clone());
        t.folder = main.folder.clone();
        t.messages = messages;
        t.created = now;
        self.threads.lock().unwrap().insert(t.id.clone(), t.clone());
        self.save(&t.id);
        Ok(t)
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
            archived: false,
            route: src.route.clone(),
            messages: src.messages[..=end].to_vec(),
            cli_session: None,
            branched_from: Some(src.id.clone()),
            side_of: None,
            project: src.project.clone(),
            // A branch is the same work, so it stays where its source is filed.
            folder: src.folder.clone(),
            app: src.app.clone(),
            system: src.system.clone(),
            agent: src.agent.clone(),
            members: src.members.clone(),
            goal: src.goal.clone(),
            goal_paused: false,
            life: Life::Keep,
            expires_at: None,
            ttl_minutes: None,
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
        // A stopped tile owes nobody an answer; the next letter waits for `pump`.
        self.answering.lock().unwrap().remove(id);
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
        let mid = new_id();
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
                id: mid.clone(),
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
                thinking: String::new(),
                acts: Vec::new(),
                breaks: Vec::new(),
                edited: false,
                via: None,
                author: None,
                author_name: None,
                trace: None,
                interrupted: false,
                wait: None,
                plan: ctx.plan,
                spec: None,
            });
            t.updated = now;
            Chats::touched(t);
        })?;
        // The folder as it is before this message's work starts: a checkpoint to compare with or
        // go back to. Off the send path, as a big folder takes a moment to hash.
        if let Some(dir) = self.thread(id).and_then(|t| t.project).filter(|d| std::path::Path::new(d).is_dir()) {
            let (sid, mid) = (id.to_string(), mid.clone());
            std::thread::spawn(move || {
                crate::checkpoint::take(std::path::Path::new(&dir), &sid, &mid);
            });
        }
        self.hops.lock().unwrap().insert(id.to_string(), 0);
        // `@codex ...` naming another harness tile hands the message to that
        // tile: it answers in its own session, and the answer is posted here.
        let tiles = crate::relay::tiles(&self.dir);
        if let Some((to, rest)) = crate::relay::mention(&tiles, id, text).filter(|(_, r)| !r.is_empty()) {
            let text = self.with_context(id, &tiles, &rest);
            let inb = Inbound { from: id.to_string(), text, reply: true, as_note: true, hops: 1, origin: Some(("the user".into(), rest.clone())) };
            return self.take_letter(&to.thread.clone(), inb, ctx);
        }
        self.reply(id, ctx, mention(text))
    }

    // ------------------------------------------------------------ tiles

    /// A letter from one harness tile to another (relay.rs), from a tile's
    /// `message_tile` call.
    pub fn deliver(self: &Arc<Self>, l: &crate::relay::Letter, ctx: Ctx) -> Result<()> {
        let hops = self.hops.lock().unwrap().get(&l.from).copied().unwrap_or(0) + 1;
        if hops > crate::relay::MAX_HOPS {
            let why = format!(
                "(Not delivered: this request has been handed on {} times already. Ask again to carry on.)",
                crate::relay::MAX_HOPS
            );
            self.note(&l.from, &l.to, &why);
            bail!("too many hand-offs");
        }
        // A tile passing work on rewrites it in its own words. Say what the
        // chain started from, verbatim, so the receiver is not left with a
        // summary of a summary.
        let origin = self.answering.lock().unwrap().get(&l.from).map(|i| i.origin.clone().unwrap_or_else(|| (self.tile_label(&i.from), i.text.clone())));
        let text = match &origin {
            Some((who, said)) => format!("{}\n\n---\nThe original request, verbatim, from {who}:\n\n{said}", l.text),
            None => l.text.clone(),
        };
        let inb = Inbound { from: l.from.clone(), text, reply: l.reply, as_note: false, hops, origin };
        self.take_letter(&l.to, inb, ctx)
    }

    /// Deliver what tiles posted, and start the next waiting letter of every
    /// tile that is free. The app calls this every second or so.
    pub fn pump(self: &Arc<Self>, ctx: Ctx) {
        for l in crate::relay::drain(&self.dir) {
            let _ = self.deliver(&l, ctx.clone());
        }
        for sp in crate::relay::drain_spawns(&self.dir) {
            let _ = self.spawn_sub(&sp, ctx.clone());
        }
        let ready: Vec<String> = self
            .inbox
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, q)| !q.is_empty())
            .map(|(id, _)| id.clone())
            .collect();
        for id in ready.into_iter().filter(|id| !self.is_busy(id)) {
            let next = self.inbox.lock().unwrap().get_mut(&id).and_then(|q| q.pop_front());
            if let Some(n) = next {
                let _ = self.start_letter(&id, n, ctx.clone());
            }
        }
        self.pursue_goals(ctx);
    }

    /// `/goal` (Charles, 22 Nov): a stream with a goal keeps working until its reply says the
    /// goal is met, up to [`GOAL_TURNS`] turns in a row without the user, then pauses.
    fn pursue_goals(self: &Arc<Self>, ctx: Ctx) {
        let due: Vec<(String, String, bool)> = {
            let all = self.threads.lock().unwrap();
            all.values()
                .filter(|t| !t.goal_paused && t.members.is_empty() && !t.archived)
                .filter_map(|t| {
                    let g = t.goal.clone()?;
                    let last = t.messages.last()?;
                    if last.role != Role::Assistant || last.status != Status::Done {
                        return None;
                    }
                    let met = last.text.contains(GOAL_MET);
                    let run = t.messages.iter().rev().filter(|m| m.role == Role::User).take_while(|m| m.text.starts_with(GOAL_NUDGE)).count();
                    Some((t.id.clone(), g, met || run >= GOAL_TURNS)).filter(|_| met || !self.is_busy(&t.id))
                })
                .collect()
        };
        for (id, goal, stop) in due {
            if self.is_busy(&id) || self.inbox.lock().unwrap().get(&id).is_some_and(|q| !q.is_empty()) {
                continue;
            }
            if stop {
                let met = self.thread(&id).and_then(|t| t.messages.last().map(|m| m.text.contains(GOAL_MET))).unwrap_or(false);
                let _ = self.edit_thread(&id, |t| {
                    if met {
                        t.goal = None;
                    } else {
                        t.goal_paused = true;
                    }
                });
                let note = if met { format!("(Goal met: {goal})") } else { format!("(Goal paused after {GOAL_TURNS} turns on its own: {goal}. `/goal resume` carries on.)") };
                self.note(&id, &id, &note);
                continue;
            }
            let _ = self.send(&id, &format!("{GOAL_NUDGE} {goal}"), Vec::new(), None, ctx.clone());
        }
    }

    /// `/goal clear`.
    pub fn clear_goal(&self, id: &str) -> Result<()> {
        self.edit_thread(id, |t| {
            t.goal = None;
            t.goal_paused = false;
        })
    }

    /// Set (Some), pause or resume a stream's goal.
    pub fn set_goal(&self, id: &str, goal: Option<&str>, paused: bool) -> Result<()> {
        self.edit_thread(id, |t| {
            if let Some(g) = goal {
                t.goal = Some(g.trim().to_string()).filter(|g| !g.is_empty());
            }
            t.goal_paused = paused;
        })?;
        Ok(())
    }

    /// A sub-agent a stream asked for (`delegate`): a new stream on that
    /// provider, in the parent's project, given only the task. Its answer goes
    /// back to the parent as the parent's next turn; it counts as a hand-off,
    /// so sub-agents can't nest past [`crate::relay::MAX_HOPS`].
    pub fn spawn_sub(self: &Arc<Self>, sp: &crate::relay::Spawn, ctx: Ctx) -> Result<String> {
        let parent = self.peek(&sp.from).ok_or_else(|| anyhow!("the stream that asked is gone"))?;
        let hops = self.hops.lock().unwrap().get(&sp.from).copied().unwrap_or(0) + 1;
        if hops > crate::relay::MAX_HOPS {
            self.note(&sp.from, &sp.from, &format!("(Sub-agent not started: work has been handed on {} times already.)", crate::relay::MAX_HOPS));
            bail!("too many hand-offs");
        }
        let main = parent.side_of.clone().unwrap_or_else(|| parent.id.clone());
        // A crew member of a familiar: its standing thread under the captain gets the job.
        if let Some(name) = sp.crew.as_deref() {
            let agents = ctx.agents.clone().ok_or_else(|| anyhow!("agents are not open here"))?;
            let captain = parent.agent.as_deref().and_then(|id| agents.get(id)).and_then(|a| a.lead).ok_or_else(|| anyhow!("only a familiar has a crew"))?;
            let member = captain
                .crew
                .iter()
                .filter_map(|id| agents.get(id))
                .find(|a| a.name.eq_ignore_ascii_case(name.trim()) || a.id == name)
                .ok_or_else(|| anyhow!("no crew member named {name}"))?;
            let child = match self.crew_thread(&main, &member.id) {
                Some(id) => id,
                None => {
                    let c = self.create_in(member.route.clone(), Scope { project: parent.project.clone(), agent: Some(member.id.clone()), title: Some(member.name.clone()), ..Scope::default() });
                    self.edit_thread(&c.id, |t| {
                        t.side_of = Some(main.clone());
                        t.folder = parent.folder.clone();
                    })?;
                    c.id
                }
            };
            let inb = Inbound { from: sp.from.clone(), text: sp.task.clone(), reply: true, as_note: false, hops, origin: None };
            self.take_letter(&child, inb, ctx)?;
            return Ok(child);
        }
        let title = sp.title.clone().unwrap_or_else(|| sp.task.lines().next().unwrap_or("Side thread").chars().take(60).collect());
        let route = Route { kind: RouteKind::Cli, provider: sp.provider.clone(), model: sp.model.clone() };
        // A side thread of the stream that asked (Charles, 22 Nov: a stream is a coordinator, its
        // side threads are the normal threads it spawns), filed beside it and starting with the
        // context it chose.
        let messages = match sp.context.as_deref() {
            // The stream is mid-turn when it delegates: only what is finished goes along.
            Some("whole") => parent.messages.iter().filter(|m| m.status == Status::Done).cloned().collect(),
            Some("last") => parent.messages.iter().rev().find(|m| m.role != Role::User && m.status == Status::Done && !m.text.is_empty()).cloned().into_iter().collect(),
            _ => Vec::new(),
        };
        let child = self.create_in(route, Scope { project: parent.project.clone(), title: Some(title), ..Scope::default() });
        self.edit_thread(&child.id, |t| {
            t.side_of = Some(main.clone());
            t.folder = parent.folder.clone();
            t.messages = messages;
        })?;
        let inb = Inbound { from: sp.from.clone(), text: sp.task.clone(), reply: true, as_note: false, hops, origin: None };
        self.take_letter(&child.id, inb, ctx)?;
        Ok(child.id)
    }

    /// Open a familiar's thread (Charles, 22 Nov): its captain's own stream in its project, which
    /// starts with the captain's intro so the user knows what it needs.
    pub fn start_familiar(&self, agents: &crate::agents::Agents, a: &crate::agents::Agent) -> Result<String> {
        let lead = a.lead.as_ref().ok_or_else(|| anyhow!("{} is not a familiar", a.name))?;
        let crew: Vec<_> = lead.crew.iter().filter_map(|id| agents.get(id)).collect();
        let t = self.create_in(
            a.route.clone(),
            Scope { project: Some(lead.project.clone()), agent: Some(a.id.clone()), title: Some(a.name.clone()), ..Scope::default() },
        );
        let mut m = Msg::new(Role::Assistant, crate::agents::intro(a, &crew), Status::Done);
        m.author = Some(a.id.clone());
        m.author_name = Some(a.name.clone());
        self.edit_thread(&t.id, |t| t.messages.push(m))?;
        Ok(t.id)
    }

    /// The thread a familiar's crew member keeps under its captain's stream: the same one job after
    /// job, so it remembers the last one (nobody is killed when a job ends).
    pub fn crew_thread(&self, main: &str, agent: &str) -> Option<String> {
        let all = self.threads.lock().unwrap();
        all.values().filter(|t| t.side_of.as_deref() == Some(main) && t.agent.as_deref() == Some(agent) && !t.archived).max_by_key(|t| t.updated).map(|t| t.id.clone())
    }

    /// A side thread hands its work back (Charles, 22 Nov: "take on the work or send it back"):
    /// its last reply goes to its stream as the stream's next turn, to judge and carry on.
    pub fn send_back(self: &Arc<Self>, side: &str, ctx: Ctx) -> Result<()> {
        let t = self.thread(side).ok_or_else(|| anyhow!("no such chat"))?;
        let main = t.side_of.clone().ok_or_else(|| anyhow!("this is not a side thread"))?;
        let last = t
            .messages
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant && m.status == Status::Done && !m.text.trim().is_empty())
            .map(|m| m.text.clone())
            .ok_or_else(|| anyhow!("this side thread has no finished reply to send back yet"))?;
        let text = format!("Result from the side thread \"{}\":\n\n{last}", t.title);
        let inb = Inbound { from: side.to_string(), text, reply: false, as_note: false, hops: 1, origin: None };
        self.take_letter(&main, inb, ctx)
    }

    /// Letters waiting for a tile while it works.
    pub fn waiting(&self, id: &str) -> usize {
        self.inbox.lock().unwrap().get(id).map_or(0, |q| q.len())
    }

    fn tile_label(&self, id: &str) -> String {
        crate::relay::tiles(&self.dir)
            .into_iter()
            .find(|t| t.thread == id)
            .map(|t| t.label)
            .or_else(|| self.peek(id).map(|t| if t.side_of.is_some() || t.title.starts_with("↳ ") { format!("{} side thread", t.route.provider) } else { t.route.provider }))
            .unwrap_or_else(|| "another harness".into())
    }

    /// What the user typed after `@tile`, with this tile's last answer, so
    /// "@codex review that" means something over there.
    fn with_context(&self, id: &str, tiles: &[crate::relay::Tile], ask: &str) -> String {
        let last = self.peek(id).and_then(|t| {
            t.messages
                .iter()
                .rev()
                .find(|m| m.role == Role::Assistant && m.status == Status::Done && !m.text.trim().is_empty())
                .map(|m| m.text.clone())
        });
        let me = crate::relay::find(tiles, id).map(|t| t.label.clone()).unwrap_or_else(|| "the other tile".into());
        match last {
            Some(t) => {
                let snip: String = t.chars().take(4000).collect();
                format!("{ask}\n\n---\nFor context, what `{me}` last said in its session:\n\n{snip}")
            }
            None => ask.to_string(),
        }
    }

    /// Hand a letter to a tile now, or queue it while the tile is busy.
    fn take_letter(self: &Arc<Self>, to: &str, inb: Inbound, ctx: Ctx) -> Result<()> {
        if !self.threads.lock().unwrap().contains_key(to) {
            bail!("that tile's session is gone");
        }
        if self.is_busy(to) || self.waiting(to) > 0 {
            self.inbox.lock().unwrap().entry(to.to_string()).or_default().push_back(inb);
            self.poke();
            return Ok(());
        }
        self.start_letter(to, inb, ctx)
    }

    /// A letter as the tile's next turn: posted under the sender's name, then answered.
    fn start_letter(self: &Arc<Self>, to: &str, inb: Inbound, ctx: Ctx) -> Result<()> {
        let mut m = Msg::new(Role::User, inb.text.clone(), Status::Done);
        m.author = Some(format!("tile:{}", inb.from));
        m.author_name = Some(self.tile_label(&inb.from));
        // The tile is answering from before its letter shows until its turn is
        // running, so it never looks idle in between.
        self.hops.lock().unwrap().insert(to.to_string(), inb.hops);
        self.answering.lock().unwrap().insert(to.to_string(), inb);
        if let Err(e) = self.edit_thread(to, |t| {
            t.updated = m.at;
            Chats::touched(t);
            t.messages.push(m);
        }) {
            self.answering.lock().unwrap().remove(to);
            return Err(e);
        }
        let res = self.reply(to, ctx, None);
        if res.is_err() {
            self.answering.lock().unwrap().remove(to);
        }
        res
    }

    /// Something another tile said, posted in this thread without a turn. A
    /// resumed CLI session reads it with the next message (`request`).
    fn note(&self, to: &str, from: &str, text: &str) {
        let mut m = Msg::new(Role::Assistant, text.to_string(), Status::Done);
        m.author = Some(format!("tile:{from}"));
        m.author_name = Some(self.tile_label(from));
        let _ = self.edit_thread(to, |t| {
            t.updated = m.at;
            Chats::touched(t);
            t.messages.push(m);
        });
        self.unread.lock().unwrap().insert(to.to_string());
        self.poke();
    }

    /// A tile finished a turn: its answer goes back to whoever asked, and the
    /// next letter waiting for it starts.
    fn after_turn(self: &Arc<Self>, tid: &str, mid: &str, ctx: &Ctx) {
        // Still answering until the answer is on its way: the asker's turn
        // starts (or its letter waits) before this tile lets go.
        let inb = self.answering.lock().unwrap().get(tid).cloned();
        if let Some(inb) = inb.filter(|i| i.reply) {
            let label = self.tile_label(tid);
            let m = self.peek(tid).and_then(|t| t.messages.into_iter().find(|m| m.id == mid));
            let body = match m {
                Some(m) if m.status == Status::Error => format!("(`{label}` failed: {})", m.error.unwrap_or_default()),
                Some(m) if m.status == Status::Done => m.text,
                _ => format!("(`{label}` stopped before it answered)"),
            };
            if inb.as_note {
                self.note(&inb.from, tid, &body);
            } else {
                let back = Inbound { from: tid.to_string(), text: body, reply: false, as_note: false, hops: inb.hops, origin: None };
                let _ = self.take_letter(&inb.from, back, ctx.clone());
            }
        }
        self.answering.lock().unwrap().remove(tid);
        let next = self.inbox.lock().unwrap().get_mut(tid).and_then(|q| q.pop_front());
        if let Some(n) = next {
            let _ = self.start_letter(tid, n, ctx.clone());
        }
    }

    /// Something from outside for an agent (its webhook): posted in its
    /// thread under `from` rather than as the user, then answered.
    pub fn event(self: &Arc<Self>, id: &str, from: &str, text: &str, ctx: Ctx) -> Result<()> {
        if self.is_busy(id) {
            bail!("busy");
        }
        let mut m = Msg::new(Role::User, text.to_string(), Status::Done);
        m.author_name = Some(from.to_string());
        self.edit_thread(id, |t| {
            t.updated = m.at;
            Chats::touched(t);
            t.messages.push(m);
        })?;
        self.reply(id, ctx, None)
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
            archived: false,
            route: route.clone(),
            messages: vec![user, reply],
            cli_session: None,
            branched_from: None,
            side_of: None,
            project: None,
            folder: None,
            app: None,
            system: Some(system.to_string()),
            agent: None,
            members: vec![],
            goal: None,
            goal_paused: false,
            // Dreaming: a one-off ask, so it is gone the same way a ghost is.
            life: Life::Ghost,
            expires_at: None,
            ttl_minutes: None,
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
        // Held until the handle is in: a reply that fails at once (nothing
        // listening) must not remove itself before it is added, or the thread
        // would read as busy for good.
        let mut running = self.running.lock().unwrap();
        let task = self.rt.spawn(async move {
            let mut res = me.run(&tid, &mid, &route, &ctx, &call).await;
            // Auto fallback: the agent's sheet may carry a chain. Clear the
            // half-written reply, then try the next route once each.
            if res.is_err() {
                let chain = call.agent.as_ref().map(|a| a.fallbacks.clone()).unwrap_or_default();
                for fb in chain {
                    if fb == route {
                        continue;
                    }
                    me.with_msg(&tid, &mid, |m| {
                        m.text.clear();
                        m.error = None;
                        m.status = Status::Sending;
                    });
                    res = me.run(&tid, &mid, &fb, &ctx, &call).await;
                    if res.is_ok() {
                        me.with_msg(&tid, &mid, |m| {
                            m.via = Some(fb.clone());
                            if fb.model.is_some() {
                                m.model = fb.model.clone();
                            }
                        });
                        break;
                    }
                }
            }
            if mentioned {
                let _ = me.edit_thread(&tid, |t| t.cli_session = None);
            }
            me.running.lock().unwrap().remove(&tid);
            let spec = res.is_ok() && call.agent.is_none() && (ctx.plan || me.plan_of(&tid));
            me.finish(&tid, &mid, res);
            if spec {
                me.file_spec(&tid, &mid);
            }
            me.after_turn(&tid, &mid, &ctx);
            me.unread.lock().unwrap().insert(tid);
            me.poke();
        });
        running.insert(id.to_string(), task.abort_handle());
        drop(running);
        self.poke();
    }

    /// Mark a reply done or failed.
    fn finish(&self, tid: &str, mid: &str, res: Result<()>) {
        let _ = self.edit_thread(tid, |t| {
            if let Some(m) = t.messages.iter_mut().find(|m| m.id == mid) {
                // Whatever was still running when the reply ended ends with it.
                let at = now_ms();
                for a in m.acts.iter_mut().filter(|a| a.end.is_none()) {
                    a.end = Some(at);
                }
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
            Chats::touched(t);
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
        // Held until the handle is in (see `spawn_reply`).
        let mut running = self.running.lock().unwrap();
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
        running.insert(id.to_string(), task.abort_handle());
        drop(running);
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
            m.wait = None;
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
        let project = project.map(PathBuf::from).filter(|p| p.is_dir());
        let brief = call
            .agent
            .as_ref()
            .and_then(|a| ctx.agents.as_ref().map(|ag| ag.brief_in(a, project.as_deref())));
        // A bot working on a project reads that project's notes; its own notes
        // ("agent:<id>") are the fallback when it is not on one.
        let scope = match &call.agent {
            Some(a) => project.as_ref().map(|p| p.display().to_string()).or_else(|| Some(format!("agent:{}", a.id))),
            None => project.as_ref().map(|p| p.display().to_string()),
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
    /// Plan mode is the last user message's, so a retry, resume or edit keeps it.
    fn plan_of(&self, id: &str) -> bool {
        let ts = self.threads.lock().unwrap();
        ts.get(id).and_then(|t| t.messages.iter().rev().find(|m| m.role == Role::User)).is_some_and(|m| m.plan)
    }

    /// A Plan reply in a project: the app writes the spec it carries and the
    /// rules judge it (spec.rs); the verdict sits on the reply.
    fn file_spec(&self, id: &str, mid: &str) {
        let Some(project) = self.project_of(id) else { return };
        let Some(text) = self.peek(id).and_then(|t| t.messages.iter().find(|m| m.id == mid).map(|m| m.text.clone())) else {
            return;
        };
        let verdict = match crate::spec::file(&project, &text, now_ms()) {
            Ok(v) => v,
            Err(e) => Some(crate::spec::Verdict { passed: false, gaps: vec![format!("Couldn't write {}: {e:#}", crate::spec::FILE)], at: now_ms() }),
        };
        if let Some(v) = verdict {
            let _ = self.edit_thread(id, |t| {
                if let Some(m) = t.messages.iter_mut().find(|m| m.id == mid) {
                    m.spec = Some(v);
                }
            });
        }
    }

    /// What the gate found missing in this thread's latest spec, if it refused it.
    fn spec_gaps(&self, id: &str) -> Vec<String> {
        let ts = self.threads.lock().unwrap();
        ts.get(id)
            .and_then(|t| t.messages.iter().rev().find_map(|m| m.spec.as_ref()))
            .map(|v| v.gaps.clone())
            .unwrap_or_default()
    }

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
        let res = crate::provider::run_target(&self.http, &req, &mut |e| {
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
        }, &crate::gateway::Authorization::Unmetered(crate::gateway::Unmetered::AgentChat))
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

    /// What the next reply in this thread would read, piece by piece: the
    /// thread's and the agent's brief, each memory note, each message. Token
    /// counts are estimates (four characters a token); `last_input` is what
    /// the last traced reply actually read, when its provider reported it.
    pub fn context_map(&self, id: &str, ctx: &Ctx) -> Option<ContextMap> {
        let t = self.peek(id)?;
        let agent = t.agent.as_ref().and_then(|a| ctx.agents.as_ref().and_then(|ag| ag.get(a)));
        let project = t.project.clone().map(PathBuf::from).filter(|p| p.is_dir());
        let mut parts: Vec<ContextPart> = Vec::new();
        fn add(parts: &mut Vec<ContextPart>, group: &str, label: String, tokens: u64) {
            if tokens > 0 {
                parts.push(ContextPart { group: group.into(), label, tokens });
            }
        }
        let push = |parts: &mut Vec<ContextPart>, group: &str, label: String, text: &str| add(parts, group, label, estimate_tokens(text));
        if let Some(sys) = &t.system {
            push(&mut parts, "Instructions", "The app's brief".into(), sys);
        }
        if let (Some(a), Some(ag)) = (&agent, &ctx.agents) {
            push(&mut parts, "Instructions", format!("{}'s brief", a.name), &ag.brief_in(a, project.as_deref()));
        }
        let scope = match &agent {
            Some(a) => project.as_ref().map(|p| p.display().to_string()).or_else(|| Some(format!("agent:{}", a.id))),
            None => project.as_ref().map(|p| p.display().to_string()),
        };
        let mem_on = ctx.prefs.memory_on && agent.as_ref().is_none_or(|a| a.memory);
        if let (true, Some(m)) = (mem_on, &ctx.memory) {
            for n in m.context_notes(scope.as_deref()) {
                push(&mut parts, "Memory", n.text.chars().take(80).collect(), &n.text);
            }
        }
        for m in t.messages.iter().filter(|m| !m.text.trim().is_empty()) {
            let who = if m.role == Role::User { "You" } else { m.author_name.as_deref().unwrap_or("Reply") };
            let first: String = m.text.split_whitespace().take(10).collect::<Vec<_>>().join(" ");
            push(&mut parts, "Conversation", format!("{who}: {first}"), &m.text);
            for a in &m.attachments {
                add(&mut parts, "Files", a.name.clone(), (a.size / 4).max(1));
            }
        }
        let last_input = t
            .messages
            .iter()
            .rev()
            .filter_map(|m| m.trace.as_ref())
            .find_map(|tid| crate::trace::load(&self.dir, tid))
            .and_then(|tr| tr.spans.first().and_then(|s| s.attrs.get("gen_ai.usage.input_tokens")).and_then(|v| v.as_u64()));
        let route = agent.as_ref().map(|a| a.route.clone()).unwrap_or_else(|| t.route.clone());
        Some(ContextMap { window: window_of(&route), model: route.model.clone().unwrap_or_else(|| route.provider.clone()), parts, last_input })
    }

    // ------------------------------------------------------------ runner

    /// This thread's history as the runner's turns.
    fn turns(&self, id: &str, hist: &[Msg]) -> Vec<Turn> {
        hist.iter()
            .map(|m| Turn {
                user: m.role == Role::User,
                // A letter from another tile says who it is from.
                text: match (&m.author_name, m.role == Role::User && is_tile(m)) {
                    (Some(who), true) => format!("[From `{who}`, another harness tiled next to you]\n\n{}", m.text),
                    _ => m.text.clone(),
                },
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
            // This machine's model server: whatever it reaches, keyless on loopback.
            RouteKind::Router if route.provider == crate::model_gateway::ROUTER_ID => {
                Target::OpenAi { base: ctx.prefs.model_serve.local_url(), key: String::new() }
            }
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
        // A resumed CLI session saw only its own turns. What was said here
        // without it since (another tile's answer, an @tile hand-off) goes in
        // front of the new message.
        if session.is_some() && !call.resume && hist.len() > 1 {
            let last = hist.len() - 1;
            let own = |m: &Msg| m.role == Role::Assistant && !is_tile(m) && m.via.is_none();
            if let Some(i) = hist[..last].iter().rposition(own) {
                let missed: Vec<String> = hist[i + 1..last]
                    .iter()
                    .map(|m| {
                        let who = match (&m.author_name, m.role) {
                            (Some(n), _) if is_tile(m) => format!("`{n}`"),
                            (_, Role::User) => "The user".into(),
                            _ => "Another model".into(),
                        };
                        format!("{who}: {}", m.text.chars().take(4000).collect::<String>())
                    })
                    .collect();
                if let (false, Some(t)) = (missed.is_empty(), turns.last_mut()) {
                    t.text = format!("Said in this session while you were not answering:\n\n{}\n\n---\n\n{}", missed.join("\n\n"), t.text);
                }
            }
        }
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

When structure helps the reader (a comparison, steps, a table, a chart, a form, follow-up choices), put it in one fenced ```openui block written in OpenUI Lang; keep the rest in Markdown. Most answers need none.

The language is the one defined below and nothing else. Three things separate a block that draws from one that draws nothing at all:

- Statements, one per line, each `name = Expression`. The block's entry point is `root = Card(...)`; every other statement must be referenced by one, or it is silently dropped.
- Arguments are POSITIONAL, in the order the signature below lists them. Never `children=[...]`, never `subtitle=`: one named argument and the whole block parses to nothing.
- Only the components in the grammar below exist. There is no FormInput, FormSelect, FormTextarea or TextAreaMulti, and a statement has no methods (`Form.getFieldValue(...)` is not a thing).

Ask the reader for input with the real form parts: `f = FormControl(\"Name\", Input(\"name\", \"Your name\", \"text\"))`, then `form = Form(\"myform\", Button(\"Send\"), [f])`, then put `form` in the Card's children. If you are not sure the block parses, write Markdown instead: a block that cannot be drawn is worse than no block.

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
        // A thread that carries a project folder works there (that is how a bot
        // works on a project); without one it keeps the bot's own workspace.
        let thread_project = self.project_of(id);
        let project = thread_project.clone().or(agent_dir);
        // Code chat runs the CLI the way the picker says, like the agents do.
        if call.agent.is_none() && project.is_some() {
            req.permission = Some(ctx.prefs.cli_permission.clone()).filter(|p| !p.is_empty());
            req.effort = ctx.prefs.cli_effort.clone();
        }
        let plan = ctx.plan || self.plan_of(id);
        req.edit = project.is_some() && !plan;
        if let Some(g) = self.peek(id).filter(|t| t.members.is_empty()).and_then(|t| t.goal) {
            req.system = Some(format!(
                "{}# Goal\n\nThis stream's goal: {g}\nKeep working toward it, one step per reply. When it is fully met and checked, end your reply with the line `{GOAL_MET}`. Don't write that line before.",
                req.system.take().map(|s| s + "\n\n").unwrap_or_default()
            ));
        }
        if plan {
            req.permission = Some("plan".into());
            // In a project, Plan mode specifies: it grills, then writes SPEC.md.
            let mode = match self.project_of(id).filter(|_| call.agent.is_none()) {
                Some(p) => crate::spec::prompt(&p, &self.spec_gaps(id)),
                None => PLAN_MODE.to_string(),
            };
            req.system = Some(format!("{}{mode}", req.system.take().map(|s| s + "\n\n").unwrap_or_default()));
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
        // The account picked for this CLI (Settings > Subscriptions): its own profile
        // folder, so the turn runs on that login and never on another's.
        if route.kind == RouteKind::Cli {
            let accounts = crate::accounts::Accounts::load(&crate::accounts::Accounts::path());
            req.env.extend(accounts.env_now(&route.provider));
        }
        // What `backspace mcp` reads goes in its own server entry: Codex starts
        // MCP servers with a filtered environment, so the CLI's would not reach it.
        let mut env = serde_json::Map::new();
        if let Some(c) = &computer {
            env.insert(crate::agents::ENV_COMPUTER.into(), serde_json::to_string(c)?.into());
        }
        // Agents and Pair may search memory and save to their own notes.
        let mem_scope = match &call.agent {
            Some(a) if a.memory => thread_project.as_ref().map(|p| p.display().to_string()).or_else(|| Some(format!("agent:{}", a.id))),
            Some(_) => None,
            None => thread_project.as_ref().map(|p| p.display().to_string()),
        }
        .filter(|_| ctx.prefs.memory_on && ctx.memory.is_some());
        if let Some(s) = &mem_scope {
            env.insert(crate::mcp::ENV_MEMORY.into(), s.clone().into());
        }
        // A harness tile: who else is in the tab, and how to reach them.
        let tile = crate::relay::brief(&crate::relay::tiles(&self.dir), id);
        if let Some(b) = &tile {
            let b = if matches!(route.provider.as_str(), "claude" | "codex") {
                b.clone()
            } else {
                // No MCP for this CLI here: the same thing from its shell.
                let exe = std::env::current_exe().map(|e| e.display().to_string()).unwrap_or_else(|_| "backspace".into());
                format!("{b}\nYou have no `message_tile` tool: run `\"{exe}\" tile send <name> \"text\"` in your shell instead (`tile list` lists them).\n")
            };
            req.system = Some(format!("{}{b}", req.system.take().map(|s| s + "\n\n").unwrap_or_default()));
            env.insert(crate::relay::ENV_TILE.into(), id.into());
            req.env.push((crate::relay::ENV_TILE.into(), id.into()));
        }
        // Every stream may start sub-agents on any provider (`delegate`), at the top level only
        // when it was not itself handed the work twice already (MAX_HOPS).
        let can_delegate = call.agent.is_none() && matches!(route.provider.as_str(), "claude" | "codex") && route.kind == RouteKind::Cli;
        if can_delegate {
            env.insert(crate::relay::ENV_STREAM.into(), id.into());
        }
        // A bot's rules, however it is called (its thread or a group): the
        // CLI's own tools it may not use, and what `backspace mcp` leaves out.
        let mut deny = call.agent.as_ref().map(|a| a.deny()).unwrap_or_default();
        if ctx.ask && !plan {
            req.edit = false;
            for r in ["edit", "run"] {
                if !deny.iter().any(|d| d == r) {
                    deny.push(r.into());
                }
            }
            req.system = Some(format!("{}{ASK_MODE}", req.system.take().map(|s| s + "\n\n").unwrap_or_default()));
        }
        req.deny = crate::agents::denied_tools(&deny);
        req.own_mcp_only = !deny.is_empty();
        if !deny.is_empty() {
            env.insert(crate::mcp::ENV_DENY.into(), deny.join(",").into());
        }
        // Its browser and desktop use.
        let hands = call.agent.as_ref().and_then(|a| a.hands_env());
        if let Some(h) = &hands {
            env.insert(crate::hands::ENV_HANDS.into(), serde_json::to_string(h)?.into());
        }
        let mut servers = serde_json::Map::new();
        // Installed apps' tools, the agent's computer, memory and hands, and the
        // other harness tiles, through `backspace mcp`.
        if !crate::mcp::app_tools(&deny).is_empty() || computer.is_some() || mem_scope.is_some() || hands.is_some() || tile.is_some() || can_delegate {
            if let Ok(exe) = std::env::current_exe() {
                servers.insert("backspace".into(), json!({"command": exe.display().to_string(), "args": ["mcp"], "env": env}));
            }
        }
        // A bot's connections (outside MCP servers), less those it was not given.
        if let Some(a) = call.agent.as_ref().filter(|a| a.allows("apps")) {
            for c in ctx.prefs.connections.iter().filter(|c| !c.id.is_empty() && !a.apps_off.contains(&c.id)) {
                servers.insert(c.id.clone(), c.server());
            }
        }
        req.mcp_allow.extend(servers.keys().map(|k| format!("mcp__{k}")));
        if !servers.is_empty() {
            req.mcp = Some(json!({ "mcpServers": servers }));
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
            RunEvent::Text { text } => {
                if !text.is_empty() {
                    self.with_msg(id, mid, |m| end_thinking(m));
                }
                self.append(id, mid, &text)
            }
            RunEvent::Wait { text } => self.with_msg(id, mid, |m| m.wait = Some(text).filter(|t| !t.is_empty())),
            RunEvent::Thinking { text } => self.with_msg(id, mid, |m| {
                m.status = Status::Streaming;
                m.wait = None;
                m.thinking.push_str(&text);
                if !m.acts.last().is_some_and(|a| a.name == "Thinking" && a.end.is_none()) {
                    let at = m.text.len();
                    m.acts.push(Act { name: "Thinking".into(), start: now_ms(), at, ..Default::default() });
                }
                // Each Thought keeps its own text, so every one of them folds open.
                if let Some(a) = m.acts.last_mut() {
                    a.output.push_str(&text);
                }
            }),
            RunEvent::ToolStart { id: tid, name, input } => self.with_msg(id, mid, |m| {
                // A reply that opens with a tool call is under way, not "waiting for the reply".
                m.status = Status::Streaming;
                m.wait = None;
                end_thinking(m);
                let path = act_path(&input);
                let input: String = input.chars().take(400).collect();
                let at = m.text.len();
                m.acts.push(Act { id: tid, name, input, path, start: now_ms(), at, ..Default::default() });
            }),
            RunEvent::ToolEnd { id: tid, output, error } => self.with_msg(id, mid, |m| {
                if let Some(a) = m.acts.iter_mut().rev().find(|a| a.id == tid && a.end.is_none()) {
                    a.end = Some(now_ms());
                    a.error = error;
                    let n = output.chars().count();
                    a.output = output.chars().skip(n.saturating_sub(600)).collect();
                }
            }),
            RunEvent::Break => {
                // The next stretch is a new turn: note where this one ended,
                // and keep the plain text readable as paragraphs.
                self.with_msg(id, mid, |m| {
                    if m.text.is_empty() {
                        return;
                    }
                    let end = m.text.trim_end().encode_utf16().count();
                    if m.breaks.last() != Some(&end) {
                        m.breaks.push(end);
                    }
                    if !m.text.ends_with("\n\n") {
                        m.text.push_str("\n\n");
                    }
                });
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
            RunEvent::Usage { .. } => {}
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

    #[test]
    fn models_name_their_makers() {
        assert_eq!(vendor(Some("claude-sonnet-4-5"), "opencode"), "Claude");
        assert_eq!(vendor(Some("opus"), "claude"), "Claude");
        assert_eq!(vendor(None, "claude"), "Claude");
        assert_eq!(vendor(None, "codex"), "GPT");
        assert_eq!(vendor(Some("gpt-5.1-codex"), "codex"), "GPT");
        assert_eq!(vendor(Some("o3"), "router"), "GPT");
        assert_eq!(vendor(Some("opencode/deepseek-v4-flash-free"), "opencode"), "DeepSeek");
        assert_eq!(vendor(Some("opencode/big-pickle"), "opencode"), "big-pickle");
        assert_eq!(vendor(None, "opencode"), "OpenCode");
        let mut t: Thread = serde_json::from_value(serde_json::json!({ "id": "x", "title": "x", "created": 1, "updated": 1, "route": { "kind": "cli", "provider": "claude", "model": null }, "messages": [] })).unwrap();
        assert_eq!(models_label(&t), "Claude");
        let reply = |model: &str| {
            let mut m = Msg::new(Role::Assistant, "ok".into(), Status::Done);
            m.model = Some(model.into());
            m
        };
        t.messages = vec![reply("claude-opus-4-1"), reply("gpt-5"), reply("claude-sonnet-4-5")];
        assert_eq!(models_label(&t), "Claude + GPT");
        t.messages.push(reply("gemini-2.5-pro"));
        assert_eq!(models_label(&t), "Claude + more");
    }

    #[test]
    fn a_streams_branch_comes_from_git_head() {
        let d = std::env::temp_dir().join(format!("bs-branch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".git")).unwrap();
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join(".git/HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        assert_eq!(git_branch(d.to_str().unwrap()).as_deref(), Some("feature/x"));
        assert_eq!(git_branch(d.join("src").to_str().unwrap()).as_deref(), Some("feature/x"), "a folder inside the repo");
        std::fs::write(d.join(".git/HEAD"), "3c465fed43ee025767f612cd143e50c3484351d5\n").unwrap();
        assert_eq!(git_branch(d.to_str().unwrap()).as_deref(), Some("3c465fe"), "detached");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_steps_file_is_read_before_its_input_is_cut() {
        assert_eq!(act_path(r#"{"content":"hi","file_path":"/w/a.rs"}"#).as_deref(), Some("/w/a.rs"));
        assert_eq!(act_path(r#"{"filePath":"C:\\x\\index.html","content":"<html"#).as_deref(), Some("C:\\x\\index.html"));
        assert_eq!(act_path(r#"{"command":"ls"}"#), None);
    }

    fn chats(dir: &Path) -> (Arc<Chats>, tokio::runtime::Runtime) {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let (tx, _rx) = async_channel::bounded(1);
        (Chats::open(dir.to_path_buf(), rt.handle().clone(), tx), rt)
    }

    /// The route the other tests in here use: a local model, so nothing reaches
    /// the network.
    fn route() -> Route {
        Route { kind: RouteKind::Local, provider: "ollama".into(), model: Some("m".into()) }
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
    fn context_map_counts_the_brief_and_each_message() {
        let dir = tmp("ctxmap");
        let (c, _rt) = chats(&dir);
        let t = c.create(route());
        c.edit_thread(&t.id, |t| {
            t.system = Some("x".repeat(400));
            t.messages.push(Msg::new(Role::User, "a".repeat(40), Status::Done));
            t.messages.push(Msg::new(Role::Assistant, "b".repeat(80), Status::Done));
            t.messages.push(Msg::new(Role::Assistant, String::new(), Status::Streaming));
        })
        .unwrap();
        let ctx = Ctx { prefs: Prefs::default(), memory: None, agents: None, plan: false, ask: false };
        let m = c.context_map(&t.id, &ctx).unwrap();
        let got: Vec<(&str, u64)> = m.parts.iter().map(|p| (p.group.as_str(), p.tokens)).collect();
        assert_eq!(got, vec![("Instructions", 100), ("Conversation", 10), ("Conversation", 20)]);
        assert!(m.parts[1].label.starts_with("You: "));
        assert_eq!(window_of(&Route { kind: RouteKind::Cli, provider: "claude".into(), model: None }), 200_000);
        assert_eq!(m.last_input, None);
    }

    #[test]
    fn activity_counts_prompts_hours_and_sessions() {
        let dir = tmp("activity");
        let (c, _rt) = chats(&dir);
        let old = c.create(route());
        let t = c.create(route());
        let msg = |id: &str, role, at, author: Option<&str>| Msg {
            id: id.into(),
            role,
            text: "x".into(),
            at,
            status: Status::Done,
            error: None,
            reply_to: None,
            reactions: vec![],
            attachments: vec![],
            model: None,
            ad: None,
            cost_usd: None,
            thinking: String::new(),
            acts: Vec::new(),
            breaks: Vec::new(),
            edited: false,
            via: None,
            author: author.map(Into::into),
            author_name: None,
            trace: None,
            interrupted: false,
            wait: None,
            plan: false,
            spec: None,
        };
        let h = 3_600_000;
        c.edit_thread(&old.id, |t| t.messages.push(msg("o", Role::User, h, None))).unwrap();
        c.edit_thread(&t.id, |t| {
            t.messages.push(msg("a", Role::User, 100 * h, None));
            t.messages.push(msg("b", Role::Assistant, 100 * h + 5, None));
            t.messages.push(msg("c", Role::User, 102 * h, Some("planner")));
        })
        .unwrap();
        let a = c.activity(50 * h);
        assert_eq!(a.sessions, 1);
        assert_eq!(a.hours, vec![100, 102]);
        assert_eq!(a.prompts.len(), 2);
        assert_eq!(a.prompts[0].provider, "ollama");
        assert!(!a.prompts[0].agent && a.prompts[1].agent);
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
                    thinking: String::new(),
                    acts: Vec::new(),
                    breaks: Vec::new(),
                    edited: false,
                    via: None,
                    author: None,
                    author_name: None,
                    trace: None,
                    interrupted: false,
                    wait: None,
                    plan: false,
                    spec: None,
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

    // ------------------------------------------------------- ghost and temporal

    /// A ghost thread is not on disk and not in the list, from the moment it
    /// exists. That is the whole promise: closing it destroys it.
    #[test]
    fn a_ghost_thread_is_never_written_and_never_listed() {
        let dir = tmp("ghost");
        let (c, rt) = chats(&dir);
        let g = c.create_in(
            route(),
            Scope { life: Life::Ghost, ..Default::default() },
        );
        assert_eq!(g.life, Life::Ghost);
        assert!(g.hidden, "nothing may offer to reopen it");
        assert!(!c.path(&g.id).exists(), "a ghost was written to disk");
        assert!(c.list().iter().all(|t| t.id != g.id), "a ghost was listed");

        // A message goes in normally; it is still not on disk.
        c.edit_thread(&g.id, |t| {
            t.messages.push(Msg::new(Role::User, "hello".into(), Status::Done));
        })
        .unwrap();
        assert!(!c.path(&g.id).exists(), "editing a ghost wrote it to disk");

        // It is reachable while you are in it, and gone when the app restarts.
        assert!(c.thread(&g.id).is_some());
        drop(c);
        let (c2, _rt2) = chats(&dir);
        assert!(c2.thread(&g.id).is_none(), "a ghost came back after a restart");
        assert!(c2.threads.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
        drop(rt);
    }

    /// Entering the mode again forgets anything a previous one left behind, and
    /// leaves ordinary chats alone. Whirl's `purgeIncognito` on enter.
    #[test]
    fn purging_ghosts_removes_only_ghosts() {
        let dir = tmp("ghost-purge");
        let (c, rt) = chats(&dir);
        let g = c.create_in(
            route(),
            Scope { life: Life::Ghost, ..Default::default() },
        );
        let kept = c.create(route());
        // An attachment of the ghost's goes with it: the files are the chat.
        std::fs::create_dir_all(c.files_dir(&g.id)).unwrap();
        std::fs::write(c.files_dir(&g.id).join("shot.png"), b"x").unwrap();

        assert_eq!(c.purge_ghosts(), vec![g.id.clone()]);
        assert!(c.thread(&g.id).is_none(), "a ghost outlived the purge");
        assert!(!c.files_dir(&g.id).exists(), "its files outlived it");
        assert!(c.thread(&kept.id).is_some(), "a kept chat was purged");
        assert!(c.purge_ghosts().is_empty(), "purging twice is a no-op");
        let _ = std::fs::remove_dir_all(&dir);
        drop(rt);
    }

    /// Turning a saved chat into a ghost takes the file with it, so it cannot
    /// be resurrected on the next launch.
    #[test]
    fn making_a_thread_a_ghost_deletes_the_file_it_had() {
        let dir = tmp("ghost-after");
        let (c, rt) = chats(&dir);
        let t = c.create(route());
        c.edit_thread(&t.id, |t| {
            t.messages.push(Msg::new(Role::User, "on disk".into(), Status::Done));
        })
        .unwrap();
        assert!(c.path(&t.id).exists(), "a normal thread is saved");

        c.set_life(&t.id, Life::Ghost, None).unwrap();
        assert!(!c.path(&t.id).exists(), "the saved file should be gone");
        assert!(c.thread(&t.id).is_some(), "but the session keeps it until you leave");
        // A later edit still does not write one.
        c.edit_thread(&t.id, |t| {
            t.messages.push(Msg::new(Role::User, "still here".into(), Status::Done));
        })
        .unwrap();
        assert!(!c.path(&t.id).exists());

        // Back to Keep: it is saved again, so the choice is reversible.
        c.set_life(&t.id, Life::Keep, None).unwrap();
        assert!(c.path(&t.id).exists(), "a kept thread is written");
        let _ = std::fs::remove_dir_all(&dir);
        drop(rt);
    }

    /// A temporal chat is an ordinary saved chat until its deadline passes.
    #[test]
    fn a_temporal_thread_forgets_itself_after_its_deadline() {
        let dir = tmp("temporal");
        let (c, rt) = chats(&dir);
        let t = c.create_in(
            route(),
            Scope { life: Life::Temporal, ttl_minutes: Some(30), ..Default::default() },
        );
        assert_eq!(t.ttl_minutes, Some(30));
        let deadline = t.expires_at.expect("a temporal thread has a deadline");
        assert!(deadline > now_ms(), "the deadline is in the future");
        assert!(c.path(&t.id).exists(), "it is saved like any other chat");
        assert!(c.list().iter().any(|x| x.id == t.id), "and it is listed");
        assert_eq!(
            c.list().iter().find(|x| x.id == t.id).unwrap().expires_at,
            Some(deadline),
            "the countdown reaches the UI"
        );

        // Before the deadline nothing happens.
        assert!(c.purge_at(deadline - 1).is_empty(), "purged early");
        assert!(c.thread(&t.id).is_some());

        // At it, the thread and its files are gone.
        assert_eq!(c.purge_at(deadline), vec![t.id.clone()]);
        assert!(c.thread(&t.id).is_none(), "still in memory");
        assert!(!c.path(&t.id).exists(), "still on disk");
        assert!(c.list().iter().all(|x| x.id != t.id), "still listed");

        // And it does not come back on the next launch either.
        drop(c);
        let (c2, _rt2) = chats(&dir);
        assert!(c2.thread(&t.id).is_none());
        let _ = std::fs::remove_dir_all(&dir);
        drop(rt);
    }

    /// Every message re-arms the deadline, so a temporal chat that is being used
    /// does not vanish mid-conversation.
    #[test]
    fn talking_to_a_temporal_thread_moves_its_deadline() {
        let dir = tmp("temporal-touch");
        let (c, rt) = chats(&dir);
        let t = c.create_in(
            route(),
            Scope { life: Life::Temporal, ttl_minutes: Some(5), ..Default::default() },
        );
        let first = t.expires_at.unwrap();
        // A message from outside the composer (a webhook, say) counts too.
        c.edit_thread(&t.id, |t| {
            t.updated = now_ms();
            Chats::touched(t);
        })
        .unwrap();
        let after = c.thread(&t.id).unwrap().expires_at.unwrap();
        assert!(after >= first, "the deadline did not move");
        // A kept thread has no deadline at all.
        let k = c.create(route());
        assert_eq!(k.life, Life::Keep);
        assert!(k.expires_at.is_none(), "a normal chat does not expire");
        c.edit_thread(&k.id, |t| Chats::touched(t)).unwrap();
        assert!(c.thread(&k.id).unwrap().expires_at.is_none(), "and it stays that way");
        let _ = std::fs::remove_dir_all(&dir);
        drop(rt);
    }

    /// An older prefs-era thread file has no `life` field and must still load as
    /// an ordinary chat.
    #[test]
    fn a_thread_file_without_a_life_loads_as_a_normal_chat() {
        let t: Thread = serde_json::from_value(serde_json::json!({
            "id": "abc", "title": "Old", "created": 1, "updated": 2,
            "route": {"kind": "local", "provider": "ollama", "model": "m"},
            "messages": []
        }))
        .unwrap();
        assert_eq!(t.life, Life::Keep);
        assert!(t.expires_at.is_none() && t.ttl_minutes.is_none());
        assert!(!t.hidden);
        // And the enum round-trips as the words the prefs file would hold.
        assert_eq!(serde_json::to_string(&Life::Ghost).unwrap(), "\"ghost\"");
        assert_eq!(serde_json::from_str::<Life>("\"temporal\"").unwrap(), Life::Temporal);
        assert_eq!(serde_json::from_str::<Life>("\"keep\"").unwrap(), Life::Keep);
    }

    // ------------------------------------------------------------ tiles

    /// A context whose local model answers nothing, quickly: nothing listens on port 9.
    fn dead_ctx() -> Ctx {
        let mut prefs = Prefs::default();
        prefs.ollama_url = "http://127.0.0.1:9".into();
        Ctx { prefs, memory: None, agents: None, plan: false, ask: false }
    }

    fn settle(c: &Chats, ids: &[&str]) {
        for _ in 0..200 {
            if ids.iter().all(|id| !c.is_busy(id) && c.waiting(id) == 0 && !c.answering.lock().unwrap().contains_key(*id)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("tiles never settled");
    }

    /// Two tiles, `claude` and `codex`, in a fresh data folder.
    fn two_tiles(name: &str) -> (Arc<Chats>, tokio::runtime::Runtime, String, String) {
        let dir = tmp(name);
        let (c, rt) = chats(&dir);
        let a = c.create(route()).id;
        let b = c.create(route()).id;
        let tile = |t: &str, l: &str| crate::relay::Tile { thread: t.into(), label: l.into(), runs: String::new(), coordinator: false };
        crate::relay::set_tiles(&dir, vec![tile(&a, "claude"), tile(&b, "codex")]).unwrap();
        (c, rt, a, b)
    }

    #[test]
    fn a_letter_is_a_turn_and_its_answer_goes_back() {
        let (c, _rt, a, b) = two_tiles("letter");
        let l = crate::relay::Letter { from: a.clone(), to: b.clone(), text: "run the tests".into(), reply: true };
        c.deliver(&l, dead_ctx()).unwrap();
        settle(&c, &[&a, &b]);
        let bt = c.thread(&b).unwrap();
        assert_eq!(bt.messages[0].author_name.as_deref(), Some("claude"));
        assert_eq!(bt.messages[0].text, "run the tests");
        assert_eq!(bt.messages[1].role, Role::Assistant);
        // The answer (here a failure) came back to `claude` as its next turn,
        // and that turn's answer does not bounce back again.
        let at = c.thread(&a).unwrap();
        assert!(at.messages[0].author_name.as_deref() == Some("codex") && at.messages[0].text.contains("`codex` failed"), "{:?}", at.messages[0]);
        assert_eq!(at.messages.len(), 2);
        assert_eq!(c.thread(&b).unwrap().messages.len(), 2);
    }

    #[test]
    fn a_side_stream_sits_beside_its_main_and_starts_from_the_last_reply() {
        let dir = tmp("side");
        let (c, _rt) = chats(&dir);
        let main = c.create_in(route(), Scope { project: Some("/work/app".into()), ..Scope::default() }).id;
        {
            let mut g = c.threads.lock().unwrap();
            let t = g.get_mut(&main).unwrap();
            t.messages.push(Msg::new(Role::User, "fix the redirect".into(), Status::Done));
            t.messages.push(Msg::new(Role::Assistant, "The cookie is set after the 302.".into(), Status::Done));
        }
        let s = c.side(&main, "why   does a 302 drop cookies?", SideCtx::LastReply).unwrap();
        assert_eq!(s.side_of.as_deref(), Some(main.as_str()));
        assert_eq!(s.project.as_deref(), Some("/work/app"), "same folder as the main");
        assert_eq!(s.title, "why does a 302 drop cookies?");
        assert_eq!(s.messages.len(), 1, "only the last reply");
        assert_eq!(s.messages[0].text, "The cookie is set after the 302.");
        assert!(c.side(&main, "q", SideCtx::None).unwrap().messages.is_empty());
        assert_eq!(c.side(&main, "q", SideCtx::Whole).unwrap().messages.len(), 2);
        let nested = c.side(&s.id, "deeper", SideCtx::None).unwrap();
        assert_eq!(nested.side_of.as_deref(), Some(main.as_str()), "a side of a side sits beside the same main");
        assert_eq!(c.list().iter().filter(|t| t.side_of.as_deref() == Some(main.as_str())).count(), 4);
    }

    #[test]
    fn a_delegated_task_runs_in_its_own_stream_and_its_answer_wakes_the_parent() {
        let dir = tmp("delegate");
        let (c, _rt) = chats(&dir);
        let parent = c.create_in(route(), Scope { project: Some("/work/app".into()), ..Scope::default() }).id;
        let sp = crate::relay::Spawn { from: parent.clone(), provider: "cauldron-test-missing-cli".into(), model: Some("m1".into()), title: Some("Audit billing".into()), task: "Audit the billing page".into(), context: None, crew: None };
        let child = c.spawn_sub(&sp, dead_ctx()).unwrap();
        settle(&c, &[&parent, &child]);
        let ct = c.thread(&child).unwrap();
        assert_eq!(ct.title, "Audit billing");
        assert_eq!(ct.side_of.as_deref(), Some(parent.as_str()), "a side thread of the stream that asked");
        assert_eq!(ct.project.as_deref(), Some("/work/app"), "the sub-agent works in the parent's project");
        assert_eq!((ct.route.provider.as_str(), ct.route.model.as_deref()), ("cauldron-test-missing-cli", Some("m1")));
        assert_eq!(ct.messages[0].text, "Audit the billing page", "only the task, never the parent's history");
        let pt = c.thread(&parent).unwrap();
        assert!(pt.messages[0].text.contains("failed"), "its answer (here a failure) comes back as the parent's turn: {:?}", pt.messages[0]);
        assert!(pt.messages[0].author_name.as_deref().is_some_and(|n| n.contains("side thread")), "{:?}", pt.messages[0].author_name);

        // Through the outbox, as `backspace mcp` posts it: pump starts it.
        std::fs::create_dir_all(dir.join("relay/spawn")).unwrap();
        std::fs::write(dir.join("relay/spawn/0000000000001-x.json"), serde_json::to_vec(&sp).unwrap()).unwrap();
        let before = c.list().len();
        c.pump(dead_ctx());
        assert_eq!(c.list().len(), before + 1);
        assert!(crate::relay::drain_spawns(&dir).is_empty(), "taken once");
    }

    #[test]
    fn a_side_thread_starts_with_the_context_asked_for_and_sends_its_work_back() {
        let dir = tmp("sendback");
        let (c, _rt) = chats(&dir);
        let parent = c.create_in(route(), Scope { project: Some("/work/app".into()), ..Scope::default() }).id;
        c.edit_thread(&parent, |t| {
            t.messages.push(Msg::new(Role::User, "plan the release".into(), Status::Done));
            t.messages.push(Msg::new(Role::Assistant, "Three parts: tests, docs, lint.".into(), Status::Done));
            t.messages.push(Msg::new(Role::Assistant, String::new(), Status::Streaming));
        })
        .unwrap();
        let sp = crate::relay::Spawn { from: parent.clone(), provider: "cauldron-test-missing-cli".into(), model: None, title: Some("Docs".into()), task: "Write the docs".into(), context: Some("whole".into()), crew: None };
        let child = c.spawn_sub(&sp, dead_ctx()).unwrap();
        settle(&c, &[&parent, &child]);
        let ct = c.thread(&child).unwrap();
        assert_eq!(ct.messages[0].text, "plan the release", "the whole finished conversation goes along");
        assert_eq!(ct.messages[1].text, "Three parts: tests, docs, lint.");
        assert!(ct.messages.iter().any(|m| m.text == "Write the docs"), "then the task");
        // Its reply goes back to the stream as the stream's next turn.
        c.edit_thread(&child, |t| t.messages.push(Msg::new(Role::Assistant, "Docs written.".into(), Status::Done))).unwrap();
        let before = c.thread(&parent).unwrap().messages.len();
        c.send_back(&child, dead_ctx()).unwrap();
        settle(&c, &[&parent]);
        let pt = c.thread(&parent).unwrap();
        let got = pt.messages[before..].iter().find(|m| m.role == Role::User).expect("a letter from the side thread");
        assert!(got.text.contains("Docs written.") && got.text.contains("\"Docs\""), "{}", got.text);
        assert!(c.send_back(&parent, dead_ctx()).is_err(), "a stream has nothing to send back to");
    }

    #[test]
    fn a_familiar_opens_with_its_intro_and_its_crew_keeps_one_thread() {
        let dir = tmp("familiar");
        let (c, _rt) = chats(&dir);
        let agents = Arc::new(crate::agents::Agents::open(dir.join("agents")));
        let tester = agents.save(crate::agents::Agent { job: "Writes and runs the tests.".into(), ..crate::agents::Agent::new("Tester", route()) }).unwrap();
        let mut cap = crate::agents::Agent::new("Ez", route());
        cap.lead = Some(crate::agents::Lead { project: "/work/app".into(), crew: vec![tester.id.clone()] });
        let cap = agents.save(cap).unwrap();
        let main = c.start_familiar(&agents, &cap).unwrap();
        let t = c.thread(&main).unwrap();
        assert_eq!((t.agent.as_deref(), t.project.as_deref(), t.title.as_str()), (Some(cap.id.as_str()), Some("/work/app"), "Ez"));
        let hi = &t.messages[0];
        assert!(hi.text.starts_with("I'm Ez, your captain for **app**") && hi.text.contains("What to start on now") && hi.text.contains("**Tester**"), "{}", hi.text);
        assert!(agents.brief_in(&cap, None).contains("- Tester: Writes and runs the tests."), "the captain knows its crew");

        let ctx = Ctx { agents: Some(agents.clone()), ..dead_ctx() };
        let job = |task: &str| crate::relay::Spawn { from: main.clone(), provider: String::new(), model: None, title: None, task: task.into(), context: None, crew: Some("tester".into()) };
        let first = c.spawn_sub(&job("Test the login"), ctx.clone()).unwrap();
        settle(&c, &[&main, &first]);
        let second = c.spawn_sub(&job("Test the logout"), ctx.clone()).unwrap();
        settle(&c, &[&main, &second]);
        assert_eq!(first, second, "a crew member keeps its thread between jobs");
        let ct = c.thread(&first).unwrap();
        assert_eq!((ct.side_of.as_deref(), ct.agent.as_deref()), (Some(main.as_str()), Some(tester.id.as_str())));
        assert!(ct.messages.iter().any(|m| m.text == "Test the login") && ct.messages.iter().any(|m| m.text == "Test the logout"));
        let bad = crate::relay::Spawn { crew: Some("Nobody".into()), ..job("x") };
        assert!(c.spawn_sub(&bad, ctx).is_err(), "only its own crew");
    }

    #[test]
    fn a_goal_keeps_the_stream_going_until_its_reply_says_it_is_met() {
        let dir = tmp("goal");
        let (c, _rt) = chats(&dir);
        let id = c.create_in(route(), Scope::default()).id;
        c.set_goal(&id, Some("Ship the landing page"), false).unwrap();
        c.edit_thread(&id, |t| t.messages.push(Msg::new(Role::Assistant, "Step one done.".into(), Status::Done))).unwrap();
        c.pump(dead_ctx());
        settle(&c, &[&id]);
        let t = c.thread(&id).unwrap();
        assert!(t.messages.iter().any(|m| m.role == Role::User && m.text == format!("{GOAL_NUDGE} Ship the landing page")), "it carries on by itself");
        c.edit_thread(&id, |t| t.messages.push(Msg::new(Role::Assistant, format!("All checked.\n{GOAL_MET}"), Status::Done))).unwrap();
        c.pump(dead_ctx());
        let t = c.thread(&id).unwrap();
        assert!(t.goal.is_none(), "met: the goal goes");
        assert!(t.messages.last().unwrap().text.starts_with("(Goal met"));
        // Paused: nothing more happens.
        c.set_goal(&id, Some("Another"), true).unwrap();
        c.edit_thread(&id, |t| t.messages.push(Msg::new(Role::Assistant, "Waiting.".into(), Status::Done))).unwrap();
        let n = c.thread(&id).unwrap().messages.len();
        c.pump(dead_ctx());
        assert_eq!(c.thread(&id).unwrap().messages.len(), n);
    }

    #[test]
    fn a_busy_tile_gets_its_letter_after_the_turn() {
        let (c, rt, a, b) = two_tiles("busy");
        let fake = rt.spawn(async { tokio::time::sleep(Duration::from_secs(60)).await });
        c.running.lock().unwrap().insert(b.clone(), fake.abort_handle());
        let l = crate::relay::Letter { from: a.clone(), to: b.clone(), text: "later".into(), reply: false };
        c.deliver(&l, dead_ctx()).unwrap();
        assert_eq!(c.waiting(&b), 1);
        assert!(c.thread(&b).unwrap().messages.is_empty(), "nothing reaches a tile mid-turn");
        c.stop(&b);
        c.pump(dead_ctx());
        settle(&c, &[&b]);
        assert_eq!(c.thread(&b).unwrap().messages[0].text, "later");
        assert!(c.thread(&a).unwrap().messages.is_empty(), "no answer was asked for");
    }

    #[test]
    fn the_second_hop_is_the_last() {
        let (c, _rt, a, b) = two_tiles("twohops");
        assert_eq!(crate::relay::MAX_HOPS, 2);
        let l = crate::relay::Letter { from: a.clone(), to: b.clone(), text: "x".into(), reply: false };
        c.hops.lock().unwrap().insert(a.clone(), 1);
        assert!(c.deliver(&l, dead_ctx()).is_ok(), "the second hop goes through");
        c.hops.lock().unwrap().insert(a.clone(), 2);
        assert!(c.deliver(&l, dead_ctx()).is_err(), "a third is refused");
    }

    #[test]
    fn a_forwarded_letter_carries_the_first_senders_words() {
        let (c, _rt, a, b) = two_tiles("origin");
        // `claude` is answering a request the user typed; it forwards a summary.
        let user = Inbound { from: "user".into(), text: "make the build 2x faster, no new deps".into(), reply: true, as_note: true, hops: 1, origin: Some(("the user".into(), "make the build 2x faster, no new deps".into())) };
        c.answering.lock().unwrap().insert(a.clone(), user);
        c.hops.lock().unwrap().insert(a.clone(), 1);
        let l = crate::relay::Letter { from: a.clone(), to: b.clone(), text: "speed up the build".into(), reply: false };
        c.deliver(&l, dead_ctx()).unwrap();
        settle(&c, &[&b]);
        let m = &c.thread(&b).unwrap().messages[0];
        assert!(m.text.starts_with("speed up the build"), "{}", m.text);
        assert!(m.text.contains("the user") && m.text.contains("make the build 2x faster, no new deps"), "{}", m.text);
    }

    #[test]
    fn a_chain_of_hand_offs_stops() {
        let (c, _rt, a, b) = two_tiles("hops");
        c.hops.lock().unwrap().insert(a.clone(), crate::relay::MAX_HOPS);
        let l = crate::relay::Letter { from: a.clone(), to: b.clone(), text: "again".into(), reply: true };
        assert!(c.deliver(&l, dead_ctx()).is_err());
        assert!(c.thread(&b).unwrap().messages.is_empty());
        assert!(c.thread(&a).unwrap().messages[0].text.contains("Not delivered"));
    }

    #[test]
    fn at_a_tile_hands_over_and_posts_the_answer_here() {
        let (c, _rt, a, b) = two_tiles("mention");
        c.send(&a, "@codex check the build", vec![], None, dead_ctx()).unwrap();
        settle(&c, &[&a, &b]);
        let bt = c.thread(&b).unwrap();
        assert!(bt.messages[0].text.starts_with("check the build"));
        let at = c.thread(&a).unwrap();
        assert_eq!(at.messages.len(), 2, "the user's message and codex's answer, and no turn of claude's");
        assert_eq!((at.messages[1].role, at.messages[1].author_name.as_deref()), (Role::Assistant, Some("codex")));
    }

    #[test]
    fn a_resumed_session_reads_what_it_missed() {
        let (c, _rt, a, _b) = two_tiles("missed");
        let mut own = Msg::new(Role::Assistant, "done with the parser".into(), Status::Done);
        own.model = Some("m".into());
        let ask = Msg::new(Role::User, "@codex review it".into(), Status::Done);
        let mut note = Msg::new(Role::Assistant, "two bugs in lexer.rs".into(), Status::Done);
        note.author = Some("tile:x".into());
        note.author_name = Some("codex".into());
        let now = Msg::new(Role::User, "fix what codex found".into(), Status::Done);
        let empty = Msg::new(Role::Assistant, String::new(), Status::Sending);
        c.edit_thread(&a, |t| {
            t.cli_session = Some("s1".into());
            t.messages = vec![Msg::new(Role::User, "write a parser".into(), Status::Done), own, ask, note, now, empty];
        })
        .unwrap();
        let req = c.request(&a, &route(), &dead_ctx(), &Call::default()).unwrap();
        let last = &req.history.last().unwrap().text;
        assert!(last.contains("The user: @codex review it") && last.contains("`codex`: two bugs") && last.ends_with("fix what codex found"), "{last}");
    }
}
