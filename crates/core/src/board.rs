//! Agents talking to each other, whatever runs them.
//!
//! Each project has a message board. Agents on Backspace's own loop get
//! `send_message` / `read_messages` / `list_agents` tools, and anything
//! addressed to them is added to their next turn. Agents running in a
//! coding CLI (Claude Code, Codex, Cursor, Grok, OpenCode) reach the same
//! board from their shell:
//!
//!   backspace msg agents
//!   backspace msg send <agent|main|all> "text"
//!   backspace msg read
//!
//! (any Backspace binary answers `msg`; the environment it was started
//! with says which project and which agent it is). Claude Code also gets
//! it as MCP tools through `backspace mcp`.
//!
//! The bridge behind it is a token-protected HTTP listener on 127.0.0.1,
//! one per open project, started with the harness.

use std::collections::HashMap;
use std::io::{BufRead, Write};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::project::ProjectState;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BoardMsg {
    pub id: usize,
    /// Sender's agent key ("main", a ticket key) or "you" for the human.
    pub from: String,
    /// An agent key, or "all".
    pub to: String,
    pub text: String,
    pub at: u64,
}

pub const ENV_URL: &str = "BACKSPACE_BRIDGE";
pub const ENV_TOKEN: &str = "BACKSPACE_BRIDGE_TOKEN";
pub const ENV_AGENT: &str = "BACKSPACE_AGENT";

/// Check the recipient and append. Returns the message.
pub fn post(s: &mut ProjectState, from: &str, to: &str, text: &str) -> Result<BoardMsg> {
    let to = to.trim();
    let text = text.trim();
    if text.is_empty() {
        bail!("empty message");
    }
    if text.len() > 8000 {
        bail!("messages are limited to 8000 characters");
    }
    // An agent key, or a ticket key meaning its newest agent.
    let to = if to == "all" || s.agents.iter().any(|a| a.key == to) {
        to.to_string()
    } else if let Some(a) = s
        .agents
        .iter()
        .rev()
        .find(|a| a.ticket.as_deref() == Some(to))
    {
        a.key.clone()
    } else {
        let keys: Vec<&str> = s.agents.iter().map(|a| a.key.as_str()).collect();
        bail!("no agent `{to}`; agents are: {}, or `all`", keys.join(", "));
    };
    let to = to.as_str();
    let m = BoardMsg {
        id: s.board.len(),
        from: from.to_string(),
        to: to.to_string(),
        text: text.to_string(),
        at: crate::chat::now_ms(),
    };
    s.board.push(m.clone());
    Ok(m)
}

/// Messages for `me` (to it or to all, not from it) after `cursor`.
pub fn unread<'a>(s: &'a ProjectState, me: &str, cursor: usize) -> Vec<&'a BoardMsg> {
    s.board
        .iter()
        .skip(cursor)
        .filter(|m| m.from != me && (m.to == me || m.to == "all"))
        .collect()
}

pub fn format(msgs: &[&BoardMsg]) -> String {
    if msgs.is_empty() {
        return "No new messages.".into();
    }
    msgs.iter()
        .map(|m| {
            let to = if m.to == "all" { " (to everyone)" } else { "" };
            format!("[{}]{to}: {}", m.from, m.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn agents_line(s: &ProjectState) -> String {
    s.agents
        .iter()
        .map(|a| {
            let on = a
                .decision
                .as_ref()
                .map(|d| d.model.clone())
                .unwrap_or_else(|| "not started".into());
            format!("{} — {} ({:?}, {on})", a.key, a.title, a.status)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Per-agent read positions, kept by the harness.
#[derive(Default)]
pub struct Cursors(HashMap<String, usize>);

impl Cursors {
    pub fn take(&mut self, s: &ProjectState, me: &str) -> String {
        let c = self.0.get(me).copied().unwrap_or(0);
        let out = format(&unread(s, me, c));
        self.0.insert(me.to_string(), s.board.len());
        out
    }
    /// New messages as owned values, advancing the cursor.
    pub fn drain(&mut self, s: &ProjectState, me: &str) -> Vec<BoardMsg> {
        let c = self.0.get(me).copied().unwrap_or(0);
        let out = unread(s, me, c).into_iter().cloned().collect();
        self.0.insert(me.to_string(), s.board.len());
        out
    }
}

// ---------------------------------------------------------------- client

struct Bridge {
    url: String,
    token: String,
    me: String,
}

fn bridge() -> Result<Bridge> {
    let get = |k: &str| {
        std::env::var(k)
            .map_err(|_| anyhow!("{k} is not set: run this from an agent Backspace started"))
    };
    Ok(Bridge {
        url: get(ENV_URL)?,
        token: get(ENV_TOKEN)?,
        me: get(ENV_AGENT)?,
    })
}

fn call(b: &Bridge, path: &str, body: Value) -> Result<Value> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let r = reqwest::Client::builder()
            .no_proxy()
            .build()?
            .post(format!("{}{path}", b.url))
            .bearer_auth(&b.token)
            .json(&body)
            .send()
            .await?;
        let status = r.status();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            bail!("{}", v["error"].as_str().unwrap_or("bridge error"));
        }
        Ok(v)
    })
}

fn do_send(b: &Bridge, to: &str, text: &str) -> Result<String> {
    call(
        b,
        "/v1/board/post",
        json!({"from": b.me, "to": to, "text": text}),
    )?;
    Ok(format!("sent to {to}"))
}
fn do_read(b: &Bridge) -> Result<String> {
    Ok(call(b, "/v1/board/read", json!({"me": b.me}))?["text"]
        .as_str()
        .unwrap_or("")
        .to_string())
}
fn do_agents(b: &Bridge) -> Result<String> {
    Ok(call(b, "/v1/board/agents", json!({}))?["text"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

/// `msg ...` and `mcp` for any Backspace binary. None when `args` (after
/// the program name) are not one of these, so the binary carries on.
pub fn client_main(args: &[String]) -> Option<i32> {
    match args.first().map(String::as_str) {
        Some("msg") => Some(match msg(&args[1..]) {
            Ok(out) => {
                println!("{out}");
                0
            }
            Err(e) => {
                eprintln!("backspace msg: {e:#}");
                1
            }
        }),
        Some("mcp") => Some(match mcp() {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("backspace mcp: {e:#}");
                1
            }
        }),
        _ => None,
    }
}

fn msg(args: &[String]) -> Result<String> {
    let b = bridge()?;
    match args.first().map(String::as_str) {
        Some("send") if args.len() >= 3 => do_send(&b, &args[1], &args[2..].join(" ")),
        Some("read") => do_read(&b),
        Some("agents") => do_agents(&b),
        _ => bail!("usage: msg agents | msg send <agent|main|all> <text> | msg read"),
    }
}

/// A minimal MCP server on stdio: three tools over the bridge.
/// Tools from installed apps, as (MCP name, app id, tool).
/// The agent's computer, when the CLI was started for one.
fn computer_env() -> Option<crate::agents::ComputerEnv> {
    std::env::var(crate::agents::ENV_COMPUTER).ok().and_then(|v| serde_json::from_str(&v).ok())
}

/// The memory scope a CLI was started for ("agent:<id>" or a project
/// folder): it may search memory and save notes there.
pub const ENV_MEMORY: &str = "BACKSPACE_MEMORY";

fn memory_scope() -> Option<String> {
    std::env::var(ENV_MEMORY).ok().filter(|s| !s.is_empty())
}

fn memory_search(scope: &str, query: &str) -> Result<String> {
    if query.trim().is_empty() {
        bail!("say what to look for");
    }
    let m = crate::memory::Memory::open(crate::prefs::Prefs::data_dir());
    let (notes, lines) = m.search(query, &[None, Some(scope.to_string())], 20);
    if notes.is_empty() && lines.is_empty() {
        return Ok("Nothing in memory matches.".into());
    }
    let mut out = String::new();
    for n in notes {
        let w = if n.project.is_none() { "everywhere" } else { "yours" };
        out.push_str(&format!("- {} ({w}, {})\n", n.text, crate::memory::date(n.updated)));
    }
    for l in lines {
        out.push_str(&format!("- {l}\n"));
    }
    Ok(out)
}

/// Things a note must never hold.
pub(crate) fn looks_secret(s: &str) -> bool {
    let l = s.to_lowercase();
    ["sk-", "ghp_", "github_pat_", "xoxb-", "akia", "-----begin", "password:", "password=", "api_key=", "token="]
        .iter()
        .any(|p| l.contains(p))
}

fn memory_save(scope: &str, text: &str) -> Result<String> {
    let text = text.trim();
    if text.is_empty() {
        bail!("a note needs some text");
    }
    if text.chars().count() > 400 {
        bail!("keep a note to one line (under 400 characters)");
    }
    if looks_secret(text) {
        bail!("that looks like a secret; memory does not keep credentials");
    }
    let m = crate::memory::Memory::open(crate::prefs::Prefs::data_dir());
    let n = m.add(text, Some(scope.to_string()), "agent")?;
    Ok(format!("Saved (id {}). The user can see and undo it in Memory.", n.id))
}

pub(crate) fn app_tools() -> Vec<(String, String, crate::apps::Tool)> {
    let apps = crate::apps::Apps::open(crate::prefs::Prefs::data_dir().join("apps"));
    apps.list()
        .into_iter()
        .filter(|a| a.enabled)
        .flat_map(|a| {
            let id = a.manifest.id.clone();
            a.manifest
                .tools
                .into_iter()
                .map(move |t| (format!("{}__{}", id.replace('-', "_"), t.name), id.clone(), t))
        })
        .collect()
}

fn mcp() -> Result<()> {
    // The board needs a project's bridge; app tools work without one.
    let b = bridge();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        let Ok(req) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(id) = req.get("id").cloned() else {
            continue; // a notification
        };
        let result = match req["method"].as_str().unwrap_or("") {
            "initialize" => Ok(json!({
                "protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "backspace", "version": env!("CARGO_PKG_VERSION")},
            })),
            "ping" => Ok(json!({})),
            "tools/list" => {
                let mut tools: Vec<Value> = if b.is_ok() {
                    vec![
                        json!({"name": "list_agents", "description": "The agents working on this project: key, title, status and what runs them.",
                         "inputSchema": {"type": "object", "properties": {}}}),
                        json!({"name": "send_message", "description": "Message another agent by key, `main` (the lead), or `all`. Use it to coordinate: interfaces you are adding, files you are changing, questions for whoever owns something.",
                         "inputSchema": {"type": "object", "properties": {"to": {"type": "string"}, "text": {"type": "string"}}, "required": ["to", "text"]}}),
                        json!({"name": "read_messages", "description": "New messages sent to you or to everyone since you last read.",
                         "inputSchema": {"type": "object", "properties": {}}}),
                    ]
                } else {
                    vec![]
                };
                if computer_env().is_some() {
                    tools.push(json!({"name": "computer_run", "description": "Run a shell command on your own computer (your container or server) and get its output. Install software, run code and keep files there.",
                        "inputSchema": {"type": "object", "properties": {"command": {"type": "string"}, "timeout_secs": {"type": "integer"}}, "required": ["command"]}}));
                    tools.push(json!({"name": "computer_read", "description": "Read a file on your own computer.",
                        "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}}));
                    tools.push(json!({"name": "computer_write", "description": "Write a file on your own computer (creates folders as needed).",
                        "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}, "content": {"type": "string"}}, "required": ["path", "content"]}}));
                }
                if memory_scope().is_some() {
                    tools.push(json!({"name": "memory_search", "description": "Search the user's memory (an Agent Memory Repo: notes and any topic files) for what this task needs: preferences, decisions, facts about people and projects, saved queries.",
                        "inputSchema": {"type": "object", "properties": {"query": {"type": "string", "description": "Words that must all appear"}}, "required": ["query"]}}));
                    tools.push(json!({"name": "memory_save", "description": "Save one line worth knowing in later sessions: a preference, a decision, a fact the user would otherwise repeat. Not for things cheap to rediscover or only true for this task. Never secrets. Saved to your own notes; the user can see and undo it.",
                        "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}}));
                }
                for (name, app, t) in app_tools() {
                    tools.push(json!({"name": name, "description": format!("[{app} app] {}", t.description), "inputSchema": t.input_schema}));
                }
                Ok(json!({ "tools": tools }))
            }
            "tools/call" => {
                let a = &req["params"]["arguments"];
                let name = req["params"]["name"].as_str().unwrap_or("");
                let board = || b.as_ref().map_err(|e| anyhow!("{e}"));
                let r = match name {
                    "list_agents" => board().and_then(do_agents),
                    "send_message" => board().and_then(|b| {
                        do_send(b, a["to"].as_str().unwrap_or(""), a["text"].as_str().unwrap_or(""))
                    }),
                    "read_messages" => board().and_then(do_read),
                    "computer_run" | "computer_read" | "computer_write" => match computer_env() {
                        None => Err(anyhow!("you have no computer of your own")),
                        Some(env) => match name {
                            "computer_run" => crate::agents::run(
                                &env,
                                a["command"].as_str().unwrap_or(""),
                                std::time::Duration::from_secs(a["timeout_secs"].as_u64().unwrap_or(120).clamp(1, 1800)),
                            ),
                            "computer_read" => crate::agents::read(&env, a["path"].as_str().unwrap_or("")),
                            _ => crate::agents::write(&env, a["path"].as_str().unwrap_or(""), a["content"].as_str().unwrap_or("")),
                        },
                    },
                    "memory_search" | "memory_save" => match memory_scope() {
                        None => Err(anyhow!("memory is not open to this session")),
                        Some(scope) if name == "memory_search" => memory_search(&scope, a["query"].as_str().unwrap_or("")),
                        Some(scope) => memory_save(&scope, a["text"].as_str().unwrap_or("")),
                    },
                    other => match app_tools().into_iter().find(|(n, _, _)| n == other) {
                        Some((_, app, t)) => {
                            let input = if a.is_null() { json!({}) } else { a.clone() };
                            crate::apps::Apps::open(crate::prefs::Prefs::data_dir().join("apps")).run_tool(&app, &t.name, &input)
                        }
                        None => Err(anyhow!("unknown tool {other}")),
                    },
                };
                Ok(match r {
                    Ok(t) => json!({"content": [{"type": "text", "text": t}]}),
                    Err(e) => {
                        json!({"content": [{"type": "text", "text": e.to_string()}], "isError": true})
                    }
                })
            }
            m => Err(json!({"code": -32601, "message": format!("method not found: {m}")})),
        };
        let resp = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
        };
        writeln!(out, "{resp}")?;
        out.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_and_read() {
        let mut s = ProjectState::empty("p");
        let mut a = serde_json::from_value::<crate::project::AgentRecord>(json!({
            "id": 0, "role": "main", "kind": "main", "parent": null, "depth": 0, "key": "main",
            "title": "Main", "brief": "", "depends_on": [], "status": "idle", "decision": null,
            "log": [], "cost_usd": 0.0, "input_tokens": 0, "output_tokens": 0, "deliverable": null,
            "budget_usd": null, "ticket": null, "branch": null, "worktree": null, "escalations": []
        }))
        .unwrap();
        s.agents.push(a.clone());
        a.id = 1;
        a.key = "api".into();
        s.agents.push(a);
        assert!(post(&mut s, "api", "nobody", "hi").is_err());
        post(&mut s, "api", "main", "schema is in db.sql").unwrap();
        post(&mut s, "main", "all", "freeze the API").unwrap();
        let mut c = Cursors::default();
        assert_eq!(c.take(&s, "main"), "[api]: schema is in db.sql");
        assert_eq!(c.take(&s, "main"), "No new messages.");
        assert_eq!(c.take(&s, "api"), "[main] (to everyone): freeze the API");
    }
}
