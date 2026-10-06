//! `backspace mcp`: a Model Context Protocol server on stdio that coding
//! CLIs (Claude Code) start to get Backspace's tools:
//!
//! - the project's message board (`list_agents`, `send_message`,
//!   `read_messages`), when started for a worker (board.rs has the bridge);
//! - the agent's own computer (`computer_run`, `computer_read`,
//!   `computer_write`), when started for an agent that has one (agents.rs);
//! - memory (`memory_search`, `memory_save`), when started with a memory
//!   scope (memory.rs);
//! - every enabled app's tools (apps.rs).
//!
//! Which tools it lists depends only on the environment the CLI was given.

use std::io::{BufRead, Write};

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

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

fn memory_save(scope: &str, text: &str) -> Result<String> {
    let text = text.trim();
    if text.is_empty() {
        bail!("a note needs some text");
    }
    if text.chars().count() > 400 {
        bail!("keep a note to one line (under 400 characters)");
    }
    if crate::memory::looks_secret(text) {
        bail!("that looks like a secret; memory does not keep credentials");
    }
    let m = crate::memory::Memory::open(crate::prefs::Prefs::data_dir());
    let n = m.add(text, Some(scope.to_string()), "agent")?;
    Ok(format!("Saved (id {}). The user can see and undo it in Memory.", n.id))
}

/// Tools from installed apps, as (MCP name, app id, tool).
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

/// Serve MCP on stdio until the client closes it.
pub fn serve() -> Result<()> {
    // The board needs a project's bridge; app tools work without one.
    let b = crate::board::bridge();
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
                    "list_agents" => board().and_then(crate::board::do_agents),
                    "send_message" => board().and_then(|b| {
                        crate::board::do_send(b, a["to"].as_str().unwrap_or(""), a["text"].as_str().unwrap_or(""))
                    }),
                    "read_messages" => board().and_then(crate::board::do_read),
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
