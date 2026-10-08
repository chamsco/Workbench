//! One turn on any model or agent, as one stream of events.
//!
//! Backspace talks to many things that answer: coding CLIs on your own
//! subscription (Claude Code, Codex, Cursor, OpenCode, Grok), a model in
//! Ollama, any OpenAI-compatible API (OpenRouter, LM Studio, vLLM, xAI,
//! Meta's Muse API…), and remote agents that speak A2A. This crate hides the
//! differences: build a [`Request`], call [`run`], and get [`Event`]s (text,
//! tool calls, the session to resume, cost) as they happen.
//!
//! It knows nothing about chats, threads, prefs or a UI, so another app can
//! use it as is (MIT). Finding the CLIs on a machine is the caller's job:
//! pass the binary in [`Target::Cli`].

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub mod a2a;

/// Who answers.
#[derive(Clone, Debug)]
pub enum Target {
    /// A coding CLI, run headless: `provider` is claude, codex, cursor,
    /// opencode or grok; `bin` is the executable to start.
    Cli { provider: String, bin: PathBuf },
    /// Ollama's own API at `url` (e.g. http://localhost:11434).
    Ollama { url: String },
    /// Any OpenAI-compatible Chat Completions API (`base` ends before
    /// `/chat/completions`).
    OpenAi { base: String, key: String },
    /// A remote agent speaking A2A (JSON-RPC `message/send`) at `url`.
    A2a { url: String, token: Option<String> },
}

/// A file that came with a message.
#[derive(Clone, Debug)]
pub struct File {
    pub name: String,
    pub mime: String,
    pub path: PathBuf,
}

/// One message of the conversation so far. The last one is what to answer.
#[derive(Clone, Debug)]
pub struct Turn {
    pub user: bool,
    pub text: String,
    /// For an answer from someone else than the target (another CLI, an
    /// agent): their name, shown in transcripts.
    pub who: Option<String>,
    pub files: Vec<File>,
}

impl Turn {
    pub fn user(text: impl Into<String>) -> Self {
        Self { user: true, text: text.into(), who: None, files: vec![] }
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub target: Target,
    pub model: Option<String>,
    /// Extra instructions (a brief, memory notes).
    pub system: Option<String>,
    pub history: Vec<Turn>,
    /// A session to resume (Claude Code's session id, an A2A context id).
    /// With one, only the last turn is sent.
    pub session: Option<String>,
    /// Where a CLI runs.
    pub cwd: PathBuf,
    /// Whether a CLI may edit files and run commands in `cwd`.
    pub edit: bool,
    /// Extra environment for a CLI (PATH included, if you need it).
    pub env: Vec<(String, String)>,
    /// An MCP config for CLIs that take one (`{"mcpServers": {...}}`).
    pub mcp: Option<Value>,
    /// Tool name prefixes the CLI may call without asking ("mcp__backspace").
    pub mcp_allow: Vec<String>,
    /// More folders a CLI may read and edit.
    pub add_dirs: Vec<PathBuf>,
    /// With `edit`, what a CLI may do without asking: "edits" (the
    /// default: edit files, run the allowed tools), "auto" (the CLI's own
    /// reviewer decides) or "full" (anything; no sandbox).
    pub permission: Option<String>,
    /// Reasoning effort for CLIs that take one: low, medium, high, xhigh,
    /// max (ultra means each CLI's top).
    pub effort: Option<String>,
    /// A stronger model the CLI may consult (Claude Code's `--advisor`).
    pub advisor: Option<String>,
    /// Extra subagents for CLIs that take them (Claude Code's `--agents`).
    pub agents: Option<Value>,
}

impl Request {
    pub fn new(target: Target, history: Vec<Turn>) -> Self {
        Self {
            target,
            model: None,
            system: None,
            history,
            session: None,
            cwd: std::env::temp_dir(),
            edit: false,
            env: vec![],
            mcp: None,
            mcp_allow: vec![],
            add_dirs: vec![],
            permission: None,
            effort: None,
            advisor: None,
            agents: None,
        }
    }
}

/// What happens during a turn, in order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// More of the answer.
    Text { text: String },
    /// A new paragraph starts (after a tool call): add a blank line if the
    /// answer so far doesn't end with one.
    Break,
    /// The whole answer so far (CLIs that print plain text).
    Replace { text: String },
    /// The final answer as the target reported it; use it if nothing streamed.
    Final { text: String },
    Model { model: String },
    /// The session to pass back next turn to resume.
    Session { id: String },
    Cost { usd: f64 },
    Usage { input: u64, output: u64 },
    ToolStart { id: String, name: String, input: String },
    ToolEnd { id: String, output: String, error: bool },
}

pub type Sink<'a> = &'a mut (dyn FnMut(Event) + Send);

/// Run one turn. Dropping the future stops it (a CLI is killed).
pub async fn run(http: &reqwest::Client, req: &Request, on: Sink<'_>) -> Result<()> {
    match &req.target {
        Target::Cli { provider, bin } => run_cli(req, provider, bin, on).await,
        Target::Ollama { url } => run_ollama(http, req, url.trim_end_matches('/'), on).await,
        Target::OpenAi { base, key } => run_openai(http, req, base, key, on).await,
        Target::A2a { url, token } => a2a::run(http, req, url, token.as_deref(), on).await,
    }
}

/// Run one turn and return the answer's text (for background jobs).
pub async fn ask(http: &reqwest::Client, req: &Request) -> Result<String> {
    let mut text = String::new();
    let mut fin = String::new();
    run(http, req, &mut |e| match e {
        Event::Text { text: t } => text.push_str(&t),
        Event::Break if !text.is_empty() && !text.ends_with("\n\n") => text.push_str("\n\n"),
        Event::Replace { text: t } => text = t,
        Event::Final { text: t } => fin = t,
        _ => {}
    })
    .await?;
    Ok(if text.trim().is_empty() { fin } else { text })
}

// ------------------------------------------------------------ helpers

/// Strip ANSI escapes and carriage returns from CLI output.
pub fn plain(s: &str) -> String {
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

pub fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn attached(t: &Turn) -> String {
    t.files.iter().map(|f| format!("\n\n[Attached: {} at {}]", f.name, f.path.display())).collect()
}

/// The conversation as one prompt, for targets that can't resume a session.
pub fn transcript(hist: &[Turn]) -> String {
    let mut p = String::new();
    if hist.len() > 1 {
        p.push_str("Conversation so far:\n\n");
        for m in &hist[..hist.len() - 1] {
            let who = match (m.user, &m.who) {
                (true, _) => "User".to_string(),
                (false, Some(w)) => format!("Assistant ({w})"),
                _ => "Assistant".to_string(),
            };
            let text: String = m.text.chars().take(4000).collect();
            p.push_str(&format!("{who}: {text}\n\n"));
        }
        p.push_str("Reply to the user's last message:\n\n");
    }
    if let Some(m) = hist.last() {
        p.push_str(&m.text);
        p.push_str(&attached(m));
    }
    p
}

/// What to send: the last turn alone with a session, the transcript without.
fn prompt(req: &Request) -> String {
    match (&req.session, req.history.last()) {
        (Some(_), Some(m)) => format!("{}{}", m.text, attached(m)),
        _ => transcript(&req.history),
    }
}

fn with_system(req: &Request, p: String) -> String {
    match &req.system {
        Some(s) => format!("{s}\n\n---\n\n{p}"),
        None => p,
    }
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn images(t: &Turn) -> Vec<(String, String)> {
    t.files
        .iter()
        .filter(|f| f.mime.starts_with("image/"))
        .filter_map(|f| Some((f.mime.clone(), b64(&std::fs::read(&f.path).ok()?))))
        .collect()
}

/// The turn's text with its text files inlined (for HTTP targets).
fn text_with_files(t: &Turn) -> String {
    let mut s = t.text.clone();
    for f in t.files.iter().filter(|f| !f.mime.starts_with("image/")) {
        if let Ok(body) = std::fs::read_to_string(&f.path) {
            let body: String = body.chars().take(60_000).collect();
            s.push_str(&format!("\n\n--- {} ---\n{body}", f.name));
        }
    }
    s
}

fn tool_text(v: &Value) -> String {
    let s = match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    truncate(&s, 4000).to_string()
}

// ------------------------------------------------------------ CLIs

/// Kills a process and its children when dropped (Windows; elsewhere
/// `kill_on_drop` reaches the CLI itself, which is enough).
struct KillTree(Option<u32>);

impl Drop for KillTree {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(pid) = self.0 {
            let _ = std::process::Command::new("taskkill")
                .args(["/T", "/F", "/PID", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

/// A JSON value as a TOML inline value (for Codex's `-c key=value`). JSON
/// strings are valid TOML basic strings.
fn toml_inline(v: &Value) -> String {
    match v {
        Value::Object(m) => format!(
            "{{{}}}",
            m.iter().map(|(k, v)| format!("{}={}", serde_json::to_string(k).unwrap_or_default(), toml_inline(v))).collect::<Vec<_>>().join(",")
        ),
        Value::Array(a) => format!("[{}]", a.iter().map(toml_inline).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

async fn run_cli(req: &Request, provider: &str, bin: &PathBuf, on: Sink<'_>) -> Result<()> {
    let mut cmd = tokio::process::Command::new(bin);
    cmd.current_dir(&req.cwd)
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Started from inside another Claude Code session (a terminal running
    // it), the CLI would inherit that session's identity: drop it so each
    // run gets its own session to resume.
    for k in ["CLAUDECODE", "CLAUDE_CODE_SESSION_ID", "CLAUDE_CODE_REMOTE_SESSION_ID", "CLAUDE_CODE_CHILD_SESSION", "CLAUDE_PID"] {
        cmd.env_remove(k);
    }
    for (k, v) in &req.env {
        cmd.env(k, v);
    }
    let model = req.model.clone().filter(|m| !m.is_empty());
    let stdin_text: Option<String>;
    match provider {
        "claude" => {
            cmd.args(["-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages"]);
            if let Some(s) = &req.session {
                cmd.args(["--resume", s]);
            }
            if let Some(m) = &model {
                cmd.args(["--model", m]);
            }
            if let Some(s) = &req.system {
                cmd.args(["--append-system-prompt", s]);
            }
            if let Some(cfg) = &req.mcp {
                cmd.args(["--mcp-config", &cfg.to_string()]);
            }
            for d in &req.add_dirs {
                cmd.args(["--add-dir", &d.display().to_string()]);
            }
            if let Some(e) = &req.effort {
                // Claude Code tops out at max.
                cmd.args(["--effort", match e.as_str() { "ultra" => "max", "minimal" => "low", e => e }]);
            }
            if let Some(a) = &req.advisor {
                cmd.args(["--advisor", a]);
            }
            if let Some(a) = &req.agents {
                cmd.args(["--agents", &a.to_string()]);
            }
            let allow: String = req.mcp_allow.iter().map(|p| format!(",{p}")).collect();
            if req.edit {
                let mode = match req.permission.as_deref() {
                    Some("auto") => "auto",
                    Some("full") => "bypassPermissions",
                    _ => "acceptEdits",
                };
                cmd.args(["--permission-mode", mode, "--allowedTools", &format!("Bash,Edit,Write,Read,Glob,Grep,WebFetch{allow}")]);
            } else if !allow.is_empty() {
                cmd.args(["--allowedTools", allow.trim_start_matches(',')]);
            }
            stdin_text = Some(prompt(req));
        }
        "codex" => {
            // `--full-auto` is gone from recent Codex; the sandbox says it.
            cmd.args(["exec", "--json", "--skip-git-repo-check"]);
            match (req.edit, req.permission.as_deref()) {
                (true, Some("auto")) => cmd.arg("--approve-for-me"),
                (true, Some("full")) => cmd.arg("--dangerously-bypass-approvals-and-sandbox"),
                (true, _) => cmd.args(["-s", "workspace-write"]),
                (false, _) => cmd.args(["-s", "read-only"]),
            };
            if let Some(m) = &model {
                cmd.args(["-m", m]);
            }
            if let Some(e) = &req.effort {
                // Codex tops out at xhigh.
                let e = if e == "max" || e == "ultra" { "xhigh" } else { e.as_str() };
                cmd.args(["-c", &format!("model_reasoning_effort=\"{e}\"")]);
            }
            if let Some(cfg) = &req.mcp {
                for (name, srv) in cfg["mcpServers"].as_object().into_iter().flatten() {
                    cmd.args(["-c", &format!("mcp_servers.{name}={}", toml_inline(srv))]);
                }
            }
            if let Some(s) = &req.session {
                cmd.args(["resume", s, "-"]);
                stdin_text = Some(prompt(req));
            } else {
                cmd.arg("-");
                stdin_text = Some(with_system(req, transcript(&req.history)));
            }
        }
        "cursor" => {
            cmd.args(["-p", "--output-format", "text"]);
            if req.edit {
                cmd.arg("--force");
            }
            if let Some(m) = &model {
                cmd.args(["--model", m]);
            }
            cmd.arg(with_system(req, transcript(&req.history)));
            stdin_text = None;
        }
        "opencode" => {
            cmd.arg("run");
            if let Some(m) = &model {
                cmd.args(["-m", m]);
            }
            cmd.arg(with_system(req, transcript(&req.history)));
            stdin_text = None;
        }
        "grok" => {
            if let Some(m) = &model {
                cmd.args(["-m", m]);
            }
            cmd.args(["-p", &with_system(req, transcript(&req.history))]);
            stdin_text = None;
        }
        other => bail!("{other} has no one-shot mode to chat with; use it from a project"),
    }
    let mut child = cmd.spawn().with_context(|| format!("starting {}", bin.display()))?;
    // Dropping the turn kills the CLI; on Windows its `.cmd` shim's node
    // child would survive that, so the whole tree goes.
    let mut tree = KillTree(child.id());
    if let (Some(mut w), Some(text)) = (child.stdin.take(), stdin_text) {
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
    // CLIs repeat the session id on many lines: pass on each one once.
    let mut session: Option<String> = None;
    let on = &mut |e: Event| {
        if let Event::Session { id } = &e {
            if session.as_deref() == Some(id.as_str()) {
                return;
            }
            session = Some(id.clone());
        }
        on(e)
    };
    let mut got_partial = false;
    let mut raw = String::new();
    let mut fail: Option<String> = None;
    while let Some(line) = lines.next_line().await? {
        match provider {
            "claude" => claude_line(&line, &mut got_partial, &mut fail, on),
            "codex" => {
                if serde_json::from_str::<Value>(&line).is_err() {
                    raw.push_str(&line);
                    raw.push('\n');
                    continue;
                }
                codex_line(&line, &mut fail, on);
            }
            _ => {
                raw.push_str(&line);
                raw.push('\n');
                on(Event::Replace { text: plain(&raw).trim().to_string() });
            }
        }
    }
    let status = child.wait().await?;
    tree.0 = None;
    let stderr = err_task.await.unwrap_or_default();
    if let Some(f) = fail {
        bail!(f);
    }
    if !status.success() {
        let msg = plain(stderr.trim());
        let tail: Vec<&str> = msg.lines().rev().take(6).collect();
        let msg = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
        let name = bin.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| provider.into());
        bail!(
            "{name} exited with {}{}",
            status.code().map_or("a signal".into(), |c| format!("code {c}")),
            if msg.is_empty() { String::new() } else { format!(": {msg}") }
        );
    }
    Ok(())
}

/// One line of `claude -p --output-format stream-json`.
fn claude_line(line: &str, got_partial: &mut bool, fail: &mut Option<String>, on: Sink<'_>) {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return };
    match v["type"].as_str() {
        Some("system") => {
            if let Some(s) = v["session_id"].as_str() {
                on(Event::Session { id: s.into() });
            }
            if let Some(m) = v["model"].as_str() {
                on(Event::Model { model: m.into() });
            }
        }
        Some("stream_event") => {
            let e = &v["event"];
            if e["type"] == "content_block_delta" && e["delta"]["type"] == "text_delta" {
                *got_partial = true;
                on(Event::Text { text: e["delta"]["text"].as_str().unwrap_or("").into() });
            } else if e["type"] == "message_start" {
                // A new assistant turn after a tool call.
                on(Event::Break);
            }
        }
        Some("assistant") => {
            for b in v["message"]["content"].as_array().into_iter().flatten() {
                match b["type"].as_str() {
                    Some("text") if !*got_partial => on(Event::Text { text: b["text"].as_str().unwrap_or("").into() }),
                    Some("tool_use") => on(Event::ToolStart {
                        id: b["id"].as_str().unwrap_or("").into(),
                        name: b["name"].as_str().unwrap_or("tool").into(),
                        input: truncate(&b["input"].to_string(), 2000).into(),
                    }),
                    _ => {}
                }
            }
        }
        Some("user") => {
            for b in v["message"]["content"].as_array().into_iter().flatten() {
                if b["type"] == "tool_result" {
                    on(Event::ToolEnd {
                        id: b["tool_use_id"].as_str().unwrap_or("").into(),
                        output: tool_text(&b["content"]),
                        error: b["is_error"].as_bool().unwrap_or(false),
                    });
                }
            }
        }
        Some("result") => {
            if let Some(s) = v["session_id"].as_str() {
                on(Event::Session { id: s.into() });
            }
            if let Some(c) = v["total_cost_usd"].as_f64() {
                on(Event::Cost { usd: c });
            }
            let u = &v["usage"];
            if u.is_object() {
                let input = u["input_tokens"].as_u64().unwrap_or(0)
                    + u["cache_read_input_tokens"].as_u64().unwrap_or(0)
                    + u["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                on(Event::Usage { input, output: u["output_tokens"].as_u64().unwrap_or(0) });
            }
            let result = v["result"].as_str().unwrap_or("").to_string();
            if v["is_error"].as_bool().unwrap_or(false) {
                *fail = Some(if result.is_empty() { "Claude reported an error".into() } else { result });
            } else {
                on(Event::Final { text: result });
            }
        }
        _ => {}
    }
}

/// One line of `codex exec --json`.
fn codex_line(line: &str, fail: &mut Option<String>, on: Sink<'_>) {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return };
    let item = &v["item"];
    let kind = item["type"].as_str().or(item["item_type"].as_str()).unwrap_or("");
    let id = item["id"].as_str().unwrap_or("").to_string();
    match v["type"].as_str() {
        Some("thread.started") => {
            if let Some(t) = v["thread_id"].as_str() {
                on(Event::Session { id: t.into() });
            }
        }
        Some("item.started") if !matches!(kind, "agent_message" | "assistant_message" | "reasoning") => {
            let input = item["command"].as_str().map(String::from).unwrap_or_else(|| {
                let t = item["tool"].as_str().unwrap_or("");
                format!("{t} {}", item["arguments"]).trim().to_string()
            });
            on(Event::ToolStart { id, name: kind.into(), input: truncate(&input, 2000).into() });
        }
        Some("item.completed") => match kind {
            "agent_message" | "assistant_message" => {
                if let Some(t) = item["text"].as_str() {
                    // Each message is one paragraph of the reply.
                    on(Event::Break);
                    on(Event::Text { text: t.into() });
                }
            }
            "reasoning" => {}
            _ => {
                let out = item["aggregated_output"].as_str().map(String::from).unwrap_or_else(|| tool_text(&item["result"]));
                let error = item["status"] == "failed" || item["exit_code"].as_i64().is_some_and(|c| c != 0);
                on(Event::ToolEnd { id, output: truncate(&out, 4000).into(), error });
            }
        },
        Some("turn.completed") => {
            let u = &v["usage"];
            on(Event::Usage {
                input: u["input_tokens"].as_u64().unwrap_or(0),
                output: u["output_tokens"].as_u64().unwrap_or(0),
            });
        }
        Some("error") => *fail = v["message"].as_str().map(str::to_string),
        Some("turn.failed") => *fail = v["error"]["message"].as_str().map(str::to_string),
        _ => {}
    }
}

// ------------------------------------------------------------ HTTP

/// Lines from a streamed HTTP body (NDJSON or SSE).
pub struct Lines {
    resp: reqwest::Response,
    buf: Vec<u8>,
    done: bool,
}

impl Lines {
    pub fn new(resp: reqwest::Response) -> Self {
        Self { resp, buf: Vec::new(), done: false }
    }

    pub async fn next(&mut self) -> Result<Option<String>> {
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

async fn run_ollama(http: &reqwest::Client, req: &Request, url: &str, on: Sink<'_>) -> Result<()> {
    let model = req.model.clone().filter(|m| !m.is_empty()).ok_or_else(|| anyhow!("pick an Ollama model for this chat"))?;
    let messages: Vec<Value> = req
        .system
        .as_ref()
        .map(|s| json!({ "role": "system", "content": s }))
        .into_iter()
        .chain(req.history.iter().map(|m| {
            let mut v = json!({ "role": if m.user { "user" } else { "assistant" }, "content": text_with_files(m) });
            let imgs: Vec<String> = images(m).into_iter().map(|(_, b)| b).collect();
            if !imgs.is_empty() {
                v["images"] = json!(imgs);
            }
            v
        }))
        .collect();
    let resp = http
        .post(format!("{url}/api/chat"))
        .json(&json!({ "model": model, "messages": messages, "stream": true }))
        .send()
        .await
        .map_err(|e| if e.is_connect() { anyhow!("Ollama is not running at {url}. Start it with `ollama serve`.") } else { anyhow!(e) })?;
    let status = resp.status();
    if !status.is_success() {
        let t = resp.text().await.unwrap_or_default();
        bail!("Ollama {status}: {}", truncate(&t, 300));
    }
    on(Event::Model { model: model.clone() });
    let mut lines = Lines::new(resp);
    while let Some(line) = lines.next().await? {
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        if let Some(e) = v["error"].as_str() {
            bail!("Ollama: {e}");
        }
        on(Event::Text { text: v["message"]["content"].as_str().unwrap_or("").into() });
        if v["done"].as_bool() == Some(true) {
            if let (Some(i), Some(o)) = (v["prompt_eval_count"].as_u64(), v["eval_count"].as_u64()) {
                on(Event::Usage { input: i, output: o });
            }
            break;
        }
    }
    Ok(())
}

/// The conversation in Chat Completions shape (images as data URLs).
pub fn openai_messages(system: Option<&str>, hist: &[Turn]) -> Vec<Value> {
    system
        .map(|s| json!({ "role": "system", "content": s }))
        .into_iter()
        .chain(hist.iter().map(|m| {
            let role = if m.user { "user" } else { "assistant" };
            let imgs = images(m);
            let text = text_with_files(m);
            if imgs.is_empty() {
                json!({ "role": role, "content": text })
            } else {
                let mut parts = vec![json!({"type": "text", "text": text})];
                for (mime, b) in imgs {
                    parts.push(json!({"type": "image_url", "image_url": {"url": format!("data:{mime};base64,{b}")}}));
                }
                json!({ "role": role, "content": parts })
            }
        }))
        .collect()
}

async fn run_openai(http: &reqwest::Client, req: &Request, base: &str, key: &str, on: Sink<'_>) -> Result<()> {
    let model = req.model.clone().filter(|m| !m.is_empty()).ok_or_else(|| anyhow!("pick a model for this chat"))?;
    let mut rb = http.post(format!("{}/chat/completions", base.trim_end_matches('/'))).json(&json!({
        "model": model,
        "messages": openai_messages(req.system.as_deref(), &req.history),
        "stream": true,
        "stream_options": {"include_usage": true},
    }));
    if !key.is_empty() {
        rb = rb.bearer_auth(key);
    }
    let resp = rb.send().await.context("request failed")?;
    let status = resp.status();
    if !status.is_success() {
        let t = resp.text().await.unwrap_or_default();
        bail!("{status}: {}", truncate(&t, 400));
    }
    on(Event::Model { model: model.clone() });
    let mut lines = Lines::new(resp);
    while let Some(line) = lines.next().await? {
        let Some(data) = line.strip_prefix("data:") else { continue };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
        if let Some(e) = v["error"]["message"].as_str() {
            bail!("{e}");
        }
        if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
            on(Event::Text { text: t.into() });
        }
        let u = &v["usage"];
        if u.is_object() {
            on(Event::Usage { input: u["prompt_tokens"].as_u64().unwrap_or(0), output: u["completion_tokens"].as_u64().unwrap_or(0) });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(lines: &[&str], f: fn(&str, &mut Vec<Event>)) -> Vec<Event> {
        let mut out = vec![];
        for l in lines {
            f(l, &mut out);
        }
        out
    }

    #[test]
    fn claude_stream_with_a_tool_call() {
        let lines = [
            r#"{"type":"system","subtype":"init","session_id":"s1","model":"claude-x"}"#,
            r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Looking."}}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Looking."},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":[{"type":"text","text":"a.txt"}]}]}}"#,
            r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"One file."}}}"#,
            r#"{"type":"result","session_id":"s1","total_cost_usd":0.01,"is_error":false,"result":"One file.","usage":{"input_tokens":10,"cache_read_input_tokens":5,"output_tokens":7}}"#,
        ];
        let (mut got, mut fail, mut ev) = (false, None, vec![]);
        for l in lines {
            claude_line(l, &mut got, &mut fail, &mut |e| ev.push(e));
        }
        assert!(ev.contains(&Event::Session { id: "s1".into() }));
        assert!(ev.contains(&Event::ToolStart { id: "t1".into(), name: "Bash".into(), input: r#"{"command":"ls"}"#.into() }));
        assert!(ev.contains(&Event::ToolEnd { id: "t1".into(), output: "a.txt".into(), error: false }));
        assert!(ev.contains(&Event::Usage { input: 15, output: 7 }));
        assert!(ev.contains(&Event::Cost { usd: 0.01 }));
        let text: String = ev.iter().filter_map(|e| if let Event::Text { text } = e { Some(text.as_str()) } else { None }).collect();
        assert_eq!(text, "Looking.One file.", "partial deltas only, the full assistant text is not repeated");
    }

    #[test]
    fn codex_stream() {
        let lines = [
            r#"{"type":"thread.started","thread_id":"th1"}"#,
            r#"{"type":"item.started","item":{"id":"i1","type":"command_execution","command":"cargo test"}}"#,
            r#"{"type":"item.completed","item":{"id":"i1","type":"command_execution","command":"cargo test","aggregated_output":"ok","exit_code":0,"status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"Tests pass."}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":100,"output_tokens":20}}"#,
        ];
        let ev = collect(&lines, |l, out| codex_line(l, &mut None, &mut |e| out.push(e)));
        assert_eq!(ev[0], Event::Session { id: "th1".into() });
        assert_eq!(ev[1], Event::ToolStart { id: "i1".into(), name: "command_execution".into(), input: "cargo test".into() });
        assert_eq!(ev[2], Event::ToolEnd { id: "i1".into(), output: "ok".into(), error: false });
        assert_eq!(ev[4], Event::Text { text: "Tests pass.".into() });
        assert_eq!(ev[5], Event::Usage { input: 100, output: 20 });
    }

    #[test]
    fn transcripts_and_base64() {
        let h = vec![Turn::user("hi"), Turn { user: false, text: "hello".into(), who: Some("codex".into()), files: vec![] }, Turn::user("again")];
        let t = transcript(&h);
        assert!(t.contains("User: hi") && t.contains("Assistant (codex): hello") && t.ends_with("again"));
        assert_eq!(b64(b"Man"), "TWFu");
        assert_eq!(b64(b"Ma"), "TWE=");
        assert_eq!(plain("\u{1b}[31mred\u{1b}[0m\r"), "red");
    }
}
