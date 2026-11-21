//! OpenCode through its own server (`opencode serve`), not `opencode run`.
//!
//! `opencode run` prints the finished answer only and starts a new session each time, so every
//! turn had to carry the whole conversation as one prompt: OpenCode then saw its own earlier
//! replies (compaction notes included) as the user's words and echoed them. The server keeps a
//! real session we resume by id, takes the reasoning level as a variant, and its event stream
//! gives the answer token by token, tool calls as they run, and a sure end of turn
//! (`session.idle`).
//!
//! One server per turn, on a free local port with a one-time password, killed with the turn.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

use crate::{opencode_config, prompt, tokio_cmd, truncate, Event, KillTree, Request, Sink};

const USER: &str = "opencode";

/// What the server is told to do with a permission it asks about: edits and commands go ahead
/// when the turn may edit ("always" under full access), and are refused when it may not.
pub(crate) fn permission_reply(req: &Request) -> &'static str {
    match (req.edit, req.permission.as_deref()) {
        (true, Some("full")) => "always",
        (true, _) => "once",
        (false, _) => "reject",
    }
}

/// "provider/model" as the server's `{providerID, modelID}`; the model part may hold more slashes.
pub(crate) fn model_ref(m: &str) -> Option<Value> {
    let (p, id) = m.split_once('/')?;
    (!p.is_empty() && !id.is_empty()).then(|| json!({"providerID": p, "modelID": id}))
}

/// The prompt body: the text (the last turn with a session, the transcript without), the model,
/// the brief as OpenCode's system prompt and the reasoning level as its variant.
pub(crate) fn body(req: &Request, resumed: bool) -> Value {
    let text = if resumed { prompt(req) } else { crate::transcript(&req.history) };
    let mut b = json!({"parts": [{"type": "text", "text": text}]});
    if let Some(m) = req.model.as_deref().and_then(model_ref) {
        b["model"] = m;
    }
    if let Some(s) = &req.system {
        b["system"] = json!(s);
    }
    if let Some(e) = &req.effort {
        // OpenCode's variants run minimal..max; it ignores one a model doesn't have.
        b["variant"] = json!(if e == "ultra" { "max" } else { e.as_str() });
    }
    b
}

fn secret() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut s = String::new();
    for i in 0..4u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i ^ std::process::id() as u64);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        s.push_str(&format!("{:016x}", h.finish()));
    }
    s
}

fn free_port() -> Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

/// What one turn's events add up to, kept apart from the HTTP so a test can feed it lines.
#[derive(Default)]
pub(crate) struct Turn {
    pub sid: String,
    assistant: HashSet<String>,
    /// Part id → its type ("text", "reasoning", "tool"…).
    kinds: HashMap<String, String>,
    /// Text already passed on per text or reasoning part, to fill in what came without deltas.
    sent: HashMap<String, usize>,
    tools: HashSet<String>,
    said: bool,
    model: bool,
    pub cost: f64,
    pub input: u64,
    pub output: u64,
    pub fail: Option<String>,
    pub done: bool,
    /// Permission requests to answer: (request id).
    pub asks: Vec<String>,
}

impl Turn {
    pub fn new(sid: &str) -> Self {
        Self { sid: sid.into(), ..Default::default() }
    }

    /// One event of the server's stream.
    pub fn on(&mut self, v: &Value, on: Sink<'_>) {
        let p = &v["properties"];
        let mine = |sid: &Value| sid.as_str() == Some(self.sid.as_str());
        match v["type"].as_str().unwrap_or_default() {
            "message.updated" if mine(&p["info"]["sessionID"]) || mine(&p["sessionID"]) => {
                let info = &p["info"];
                if info["role"] == "assistant" {
                    if let Some(id) = info["id"].as_str() {
                        self.assistant.insert(id.into());
                    }
                    if !self.model {
                        if let Some(m) = info["modelID"].as_str() {
                            self.model = true;
                            let m = match info["providerID"].as_str() {
                                Some(pr) => format!("{pr}/{m}"),
                                None => m.into(),
                            };
                            on(Event::Model { model: m });
                        }
                    }
                }
            }
            "message.part.updated" if mine(&p["sessionID"]) || mine(&p["part"]["sessionID"]) => {
                let part = &p["part"];
                let (Some(pid), Some(kind)) = (part["id"].as_str(), part["type"].as_str()) else { return };
                if !part["messageID"].as_str().is_some_and(|m| self.assistant.contains(m)) {
                    return;
                }
                self.kinds.insert(pid.into(), kind.into());
                match kind {
                    "text" => {
                        // A provider that sends no deltas: the finished part brings the text.
                        let text = part["text"].as_str().unwrap_or_default();
                        let sent = self.sent.get(pid).copied().unwrap_or(0);
                        if text.len() > sent && part["time"]["end"].is_number() {
                            if let Some(rest) = text.get(sent..) {
                                self.text(pid, rest, on);
                            }
                        }
                    }
                    // The same for reasoning that came without deltas.
                    "reasoning" => {
                        let text = part["text"].as_str().unwrap_or_default();
                        let sent = self.sent.get(pid).copied().unwrap_or(0);
                        if text.len() > sent && part["time"]["end"].is_number() {
                            if let Some(rest) = text.get(sent..) {
                                self.sent.insert(pid.into(), text.len());
                                on(Event::Thinking { text: rest.into() });
                            }
                        }
                    }
                    "tool" => {
                        let id = part["callID"].as_str().unwrap_or(pid).to_string();
                        let st = &part["state"];
                        let status = st["status"].as_str().unwrap_or_default();
                        if !self.tools.contains(&id) && matches!(status, "running" | "completed" | "error") {
                            self.tools.insert(id.clone());
                            on(Event::Break);
                            on(Event::ToolStart { id: id.clone(), name: part["tool"].as_str().unwrap_or("tool").into(), input: truncate(&st["input"].to_string(), 2000).into() });
                        }
                        match status {
                            "completed" => on(Event::ToolEnd { id, output: truncate(st["output"].as_str().unwrap_or_default(), 4000).into(), error: false }),
                            "error" => on(Event::ToolEnd { id, output: st["error"].as_str().unwrap_or_default().into(), error: true }),
                            _ => {}
                        }
                    }
                    "step-finish" => {
                        self.cost += part["cost"].as_f64().unwrap_or(0.0);
                        let t = &part["tokens"];
                        self.input += t["input"].as_u64().unwrap_or(0) + t["cache"]["read"].as_u64().unwrap_or(0) + t["cache"]["write"].as_u64().unwrap_or(0);
                        self.output += t["output"].as_u64().unwrap_or(0) + t["reasoning"].as_u64().unwrap_or(0);
                    }
                    _ => {}
                }
            }
            "message.part.delta" if mine(&p["sessionID"]) => {
                let (Some(pid), Some(d)) = (p["partID"].as_str(), p["delta"].as_str()) else { return };
                if p["field"].as_str().is_some_and(|f| f != "text") {
                    return;
                }
                match self.kinds.get(pid).map(String::as_str) {
                    Some("text") => self.text(pid, d, on),
                    Some("reasoning") => {
                        *self.sent.entry(pid.into()).or_insert(0) += d.len();
                        on(Event::Thinking { text: d.into() });
                    }
                    _ => {}
                }
            }
            "permission.asked" if mine(&p["sessionID"]) => {
                if let Some(id) = p["id"].as_str() {
                    self.asks.push(id.into());
                }
            }
            "session.error" if mine(&p["sessionID"]) => {
                let e = &p["error"];
                // Stopping the turn is not a failure.
                if e["name"] != "MessageAbortedError" {
                    let msg = e["data"]["message"].as_str().or(e["name"].as_str()).unwrap_or("OpenCode reported an error");
                    self.fail = Some(msg.to_string());
                }
            }
            "session.idle" if mine(&p["sessionID"]) => self.done = true,
            "session.status" if mine(&p["sessionID"]) && p["status"]["type"] == "idle" => self.done = true,
            _ => {}
        }
    }

    fn text(&mut self, pid: &str, d: &str, on: Sink<'_>) {
        if d.is_empty() {
            return;
        }
        // A new text part after a tool call starts a new paragraph.
        if !self.sent.contains_key(pid) && self.said {
            on(Event::Break);
        }
        *self.sent.entry(pid.into()).or_insert(0) += d.len();
        self.said = true;
        on(Event::Text { text: d.into() });
    }
}

/// One turn on OpenCode's server.
pub(crate) async fn run(req: &Request, bin: &PathBuf, on: Sink<'_>) -> Result<()> {
    let port = free_port()?;
    let pass = secret();
    let mut cmd = match req.wrap.split_first() {
        Some((first, rest)) => {
            let mut c = tokio_cmd(first);
            c.args(rest).arg(bin);
            c
        }
        None => tokio_cmd(bin),
    };
    cmd.args(["serve", "--hostname", "127.0.0.1", "--port", &port.to_string()])
        .current_dir(&req.cwd)
        .env("NO_COLOR", "1")
        .env("OPENCODE_SERVER_PASSWORD", &pass)
        .env("OPENCODE_SERVER_USERNAME", USER)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (k, v) in &req.env {
        cmd.env(k, v);
    }
    for k in &req.env_remove {
        cmd.env_remove(k);
    }
    // The same MCP servers the other CLIs get, as OpenCode's inline config.
    if let Some(cfg) = req.mcp.as_ref().and_then(opencode_config) {
        cmd.env("OPENCODE_CONFIG_CONTENT", cfg.to_string());
    }
    let mut child = cmd.spawn().with_context(|| format!("starting {}", bin.display()))?;
    let _tree = KillTree(child.id());
    let mut err = child.stderr.take().unwrap();
    let err_task = tokio::spawn(async move {
        let mut s = String::new();
        let _ = err.read_to_string(&mut s).await;
        s
    });
    // Wait for "opencode server listening on http://…".
    let mut out = BufReader::new(child.stdout.take().unwrap()).lines();
    let base = tokio::time::timeout(Duration::from_secs(60), async {
        while let Some(l) = out.next_line().await? {
            if let Some(i) = l.find("http://") {
                return Ok::<_, anyhow::Error>(Some(l[i..].trim().trim_end_matches('/').to_string()));
            }
        }
        Ok(None)
    })
    .await;
    let base = match base {
        Ok(Ok(Some(b))) => b,
        _ => {
            let _ = child.start_kill();
            let msg = crate::plain(err_task.await.unwrap_or_default().trim());
            let tail: Vec<&str> = msg.lines().rev().take(6).collect();
            let msg = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
            bail!("OpenCode's server did not start{}", if msg.is_empty() { String::new() } else { format!(": {msg}") });
        }
    };
    // Keep reading its output so the pipe never fills.
    tokio::spawn(async move { while let Ok(Some(_)) = out.next_line().await {} });

    // Local only: never through a proxy.
    let http = reqwest::Client::builder().no_proxy().build()?;
    let dir = req.cwd.display().to_string();
    let get = |path: &str| http.get(format!("{base}{path}")).basic_auth(USER, Some(&pass)).query(&[("directory", &dir)]);
    let post = |path: &str| http.post(format!("{base}{path}")).basic_auth(USER, Some(&pass)).query(&[("directory", &dir)]);

    // Resume the stream's session when OpenCode still has it, otherwise start one.
    let mut resumed = false;
    let mut sid = None;
    if let Some(s) = req.session.as_deref().filter(|s| s.starts_with("ses")) {
        if get(&format!("/session/{s}")).send().await.is_ok_and(|r| r.status().is_success()) {
            sid = Some(s.to_string());
            resumed = true;
        }
    }
    let sid = match sid {
        Some(s) => s,
        None => {
            let r: Value = post("/session").json(&json!({})).send().await?.error_for_status()?.json().await?;
            r["id"].as_str().ok_or_else(|| anyhow!("OpenCode made no session"))?.to_string()
        }
    };
    on(Event::Session { id: sid.clone() });

    // Listen before asking, so no token is missed.
    let mut events = get("/event").send().await?.error_for_status().context("OpenCode's event stream")?;
    let mut buf = String::new();
    let mut turn = Turn::new(&sid);
    let mut sent = false;
    loop {
        let chunk = match tokio::time::timeout(Duration::from_secs(600), events.chunk()).await {
            Ok(c) => c?,
            Err(_) => bail!("OpenCode went quiet for ten minutes"),
        };
        let Some(chunk) = chunk else { break };
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(i) = buf.find("\n\n") {
            let block: String = buf.drain(..i + 2).collect();
            for l in block.lines() {
                let Some(data) = l.strip_prefix("data:") else { continue };
                let Ok(v) = serde_json::from_str::<Value>(data.trim()) else { continue };
                if v["type"] == "server.connected" && !sent {
                    sent = true;
                    let r = post(&format!("/session/{sid}/prompt_async")).json(&body(req, resumed)).send().await?;
                    if !r.status().is_success() {
                        let s = r.status();
                        bail!("OpenCode refused the message ({s}): {}", truncate(&r.text().await.unwrap_or_default(), 400));
                    }
                    continue;
                }
                if !sent {
                    continue;
                }
                turn.on(&v, on);
                for id in std::mem::take(&mut turn.asks) {
                    let _ = post(&format!("/session/{sid}/permissions/{id}")).json(&json!({"response": permission_reply(req)})).send().await;
                }
            }
        }
        if turn.done {
            break;
        }
    }
    if turn.cost > 0.0 {
        on(Event::Cost { usd: turn.cost });
    }
    if turn.input + turn.output > 0 {
        on(Event::Usage { input: turn.input, output: turn.output });
    }
    if let Some(f) = turn.fail {
        bail!(f);
    }
    if !turn.done {
        bail!("OpenCode's server stopped before the answer was done");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{body, model_ref, permission_reply, Turn};
    use crate::{Event, Request, Target};
    use serde_json::{json, Value};

    fn req() -> Request {
        let mut r = Request::new(Target::Cli { provider: "opencode".into(), bin: "opencode".into() }, vec![crate::Turn::user("first"), crate::Turn { user: false, text: "reply".into(), who: None, files: vec![] }, crate::Turn::user("second")]);
        r.model = Some("openrouter/qwen/qwen3-coder".into());
        r
    }

    #[::core::prelude::v1::test]
    fn a_resumed_session_gets_the_last_turn_only_and_the_model_splits_at_the_first_slash() {
        assert_eq!(model_ref("openrouter/qwen/qwen3-coder"), Some(json!({"providerID": "openrouter", "modelID": "qwen/qwen3-coder"})));
        assert_eq!(model_ref("nomodel"), None);
        let mut r = req();
        r.session = Some("ses_1".into());
        r.effort = Some("ultra".into());
        let b = body(&r, true);
        assert_eq!(b["parts"][0]["text"], "second");
        assert_eq!(b["variant"], "max");
        assert_eq!(b["model"]["modelID"], "qwen/qwen3-coder");
        // A new session hears the conversation once.
        let b = body(&r, false);
        assert!(b["parts"][0]["text"].as_str().unwrap().contains("reply"));
    }

    #[::core::prelude::v1::test]
    fn permissions_follow_the_access_level() {
        let mut r = req();
        assert_eq!(permission_reply(&r), "reject");
        r.edit = true;
        assert_eq!(permission_reply(&r), "once");
        r.permission = Some("full".into());
        assert_eq!(permission_reply(&r), "always");
    }

    #[::core::prelude::v1::test]
    fn server_events_become_tokens_tools_usage_and_an_end() {
        let ev = |t: &str, p: Value| json!({"type": t, "properties": p});
        let s = "ses_a";
        let lines = vec![
            ev("message.updated", json!({"sessionID": s, "info": {"id": "msg_u", "role": "user", "sessionID": s}})),
            ev("message.part.updated", json!({"sessionID": s, "part": {"id": "prt_u", "messageID": "msg_u", "type": "text", "text": "hi", "sessionID": s}})),
            ev("message.updated", json!({"sessionID": s, "info": {"id": "msg_a", "role": "assistant", "providerID": "opencode", "modelID": "beam-free", "sessionID": s}})),
            ev("message.part.updated", json!({"sessionID": s, "part": {"id": "prt_r", "messageID": "msg_a", "type": "reasoning", "text": ""}})),
            ev("message.part.delta", json!({"sessionID": s, "partID": "prt_r", "field": "text", "delta": "thinking"})),
            // Its finished part repeats what the delta sent: nothing new.
            ev("message.part.updated", json!({"sessionID": s, "part": {"id": "prt_r", "messageID": "msg_a", "type": "reasoning", "text": "thinking", "time": {"start": 1, "end": 2}}})),
            // Reasoning that came whole, with no deltas.
            ev("message.part.updated", json!({"sessionID": s, "part": {"id": "prt_r2", "messageID": "msg_a", "type": "reasoning", "text": "more", "time": {"start": 1, "end": 2}}})),
            ev("message.part.updated", json!({"sessionID": s, "part": {"id": "prt_1", "messageID": "msg_a", "type": "text", "text": ""}})),
            ev("message.part.delta", json!({"sessionID": s, "partID": "prt_1", "field": "text", "delta": "Hello "})),
            ev("message.part.delta", json!({"sessionID": "ses_other", "partID": "prt_1", "field": "text", "delta": "NOT MINE"})),
            ev("message.part.delta", json!({"sessionID": s, "partID": "prt_1", "field": "text", "delta": "there"})),
            ev("message.part.updated", json!({"sessionID": s, "part": {"id": "prt_t", "messageID": "msg_a", "type": "tool", "callID": "call_1", "tool": "bash", "state": {"status": "running", "input": {"command": "ls"}}}})),
            ev("permission.asked", json!({"sessionID": s, "id": "per_1"})),
            ev("message.part.updated", json!({"sessionID": s, "part": {"id": "prt_t", "messageID": "msg_a", "type": "tool", "callID": "call_1", "tool": "bash", "state": {"status": "completed", "input": {"command": "ls"}, "output": "a.txt"}}})),
            // A text part that came whole, with no deltas.
            ev("message.part.updated", json!({"sessionID": s, "part": {"id": "prt_2", "messageID": "msg_a", "type": "text", "text": "Done.", "time": {"start": 1, "end": 2}}})),
            ev("message.part.updated", json!({"sessionID": s, "part": {"id": "prt_f", "messageID": "msg_a", "type": "step-finish", "cost": 0.25, "tokens": {"input": 10, "output": 4, "reasoning": 1, "cache": {"read": 2, "write": 0}}}})),
            ev("session.idle", json!({"sessionID": s})),
        ];
        let mut t = Turn::new(s);
        let mut got = Vec::new();
        for l in &lines {
            t.on(l, &mut |e| got.push(e));
        }
        let text: String = got.iter().filter_map(|e| if let Event::Text { text } = e { Some(text.as_str()) } else { None }).collect();
        assert_eq!(text, "Hello thereDone.");
        let thought: Vec<&str> = got.iter().filter_map(|e| if let Event::Thinking { text } = e { Some(text.as_str()) } else { None }).collect();
        assert_eq!(thought, ["thinking", "more"]);
        assert!(got.iter().any(|e| matches!(e, Event::Model { model } if model == "opencode/beam-free")));
        assert!(got.iter().any(|e| matches!(e, Event::ToolStart { id, name, .. } if id == "call_1" && name == "bash")));
        assert!(got.iter().any(|e| matches!(e, Event::ToolEnd { output, error: false, .. } if output == "a.txt")));
        assert_eq!(got.iter().filter(|e| matches!(e, Event::ToolStart { .. })).count(), 1);
        // The paragraph after the tool starts on its own.
        let i_tool = got.iter().position(|e| matches!(e, Event::ToolEnd { .. })).unwrap();
        assert!(got[i_tool..].iter().any(|e| matches!(e, Event::Break)));
        assert_eq!(t.asks, ["per_1"]);
        assert_eq!((t.input, t.output, t.cost), (12, 5, 0.25));
        assert!(t.done && t.fail.is_none());
    }

    #[::core::prelude::v1::test]
    fn an_error_fails_the_turn_but_stopping_does_not() {
        let mut t = Turn::new("ses_a");
        t.on(&json!({"type": "session.error", "properties": {"sessionID": "ses_a", "error": {"name": "APIError", "data": {"message": "route not found"}}}}), &mut |_| {});
        assert_eq!(t.fail.as_deref(), Some("route not found"));
        let mut t = Turn::new("ses_a");
        t.on(&json!({"type": "session.error", "properties": {"sessionID": "ses_a", "error": {"name": "MessageAbortedError"}}}), &mut |_| {});
        assert!(t.fail.is_none());
    }

    /// Against a real `opencode` (OPENCODE_BIN) and the model in OPENCODE_TEST_MODEL: two turns
    /// in one session, the second resumed. Skipped when they are not set.
    #[::core::prelude::v1::test]
    fn live_two_turns_resume_one_session() {
        let (Ok(bin), Ok(model)) = (std::env::var("OPENCODE_BIN"), std::env::var("OPENCODE_TEST_MODEL")) else { return };
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let http = reqwest::Client::new();
            let dir = std::env::temp_dir().join(format!("oc-live-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let mut r = Request::new(Target::Cli { provider: "opencode".into(), bin: bin.into() }, vec![crate::Turn::user("one")]);
            r.model = Some(model);
            r.cwd = dir;
            let (mut text, mut sid, mut deltas) = (String::new(), None, 0);
            crate::run(&http, &r, &mut |e| match e {
                Event::Text { text: t } => {
                    deltas += 1;
                    text.push_str(&t)
                }
                Event::Session { id } => sid = Some(id),
                _ => {}
            })
            .await
            .unwrap();
            assert!(deltas > 1, "{text}");
            r.session = sid.clone();
            r.history.push(crate::Turn { user: false, text: text.clone(), who: None, files: vec![] });
            r.history.push(crate::Turn::user("two"));
            let mut sid2 = None;
            let mut text2 = String::new();
            crate::run(&http, &r, &mut |e| match e {
                Event::Text { text: t } => text2.push_str(&t),
                Event::Session { id } => sid2 = Some(id),
                _ => {}
            })
            .await
            .unwrap();
            assert_eq!(sid, sid2);
            eprintln!("turn 1: {text}\nturn 2: {text2}");
        });
    }
}
