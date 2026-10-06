//! Remote agents over A2A (Agent2Agent, a2a-protocol.org): an agent
//! publishes a card at `/.well-known/agent-card.json` and answers JSON-RPC
//! `message/send` at the URL the card names. Backspace sends the turn,
//! keeps the agent's `contextId` as the session, and waits for a task the
//! agent is still working on (polling `tasks/get`).

use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{prompt, truncate, with_system, Event, Request, Sink};

/// What an agent says about itself.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Card {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Where to send messages.
    pub url: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub skills: Vec<Skill>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Skill {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// Read an agent's card from its base URL (or the card's own URL).
pub async fn card(http: &reqwest::Client, url: &str, token: Option<&str>) -> Result<Card> {
    let url = url.trim().trim_end_matches('/');
    let tries: Vec<String> = if url.ends_with(".json") {
        vec![url.to_string()]
    } else {
        vec![format!("{url}/.well-known/agent-card.json"), format!("{url}/.well-known/agent.json")]
    };
    let mut last = anyhow!("no agent card");
    for u in tries {
        let mut rb = http.get(&u).timeout(Duration::from_secs(15));
        if let Some(t) = token {
            rb = rb.bearer_auth(t);
        }
        match rb.send().await {
            Ok(r) if r.status().is_success() => {
                let mut c: Card = r.json().await.map_err(|e| anyhow!("{u} is not an agent card: {e}"))?;
                if c.url.is_empty() {
                    c.url = url.to_string();
                }
                return Ok(c);
            }
            Ok(r) => last = anyhow!("{u}: {}", r.status()),
            Err(e) => last = anyhow!("{u}: {e}"),
        }
    }
    Err(last)
}

fn id() -> String {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("bs-{n:x}")
}

async fn rpc(http: &reqwest::Client, url: &str, token: Option<&str>, method: &str, params: Value) -> Result<Value> {
    let mut rb = http
        .post(url)
        .timeout(Duration::from_secs(600))
        .json(&json!({"jsonrpc": "2.0", "id": id(), "method": method, "params": params}));
    if let Some(t) = token {
        rb = rb.bearer_auth(t);
    }
    let r = rb.send().await.map_err(|e| if e.is_connect() { anyhow!("the agent at {url} is unreachable") } else { anyhow!(e) })?;
    let status = r.status();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
        bail!("agent: {}", e["message"].as_str().unwrap_or("error"));
    }
    if !status.is_success() {
        bail!("agent: {status}");
    }
    Ok(v["result"].clone())
}

/// Text in a list of parts (`kind` or the older `type`).
fn parts_text(parts: &Value) -> String {
    parts
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["kind"].as_str().or(p["type"].as_str()).unwrap_or("text") == "text")
        .filter_map(|p| p["text"].as_str())
        .collect::<Vec<_>>()
        .join("")
}

/// The answer in a task: its artifacts, else its status message.
fn task_text(t: &Value) -> String {
    let arts: Vec<String> = t["artifacts"].as_array().into_iter().flatten().map(|a| parts_text(&a["parts"])).filter(|s| !s.is_empty()).collect();
    if !arts.is_empty() {
        return arts.join("\n\n");
    }
    parts_text(&t["status"]["message"]["parts"])
}

const DONE: &[&str] = &["completed", "canceled", "failed", "rejected", "input-required", "auth-required", "unknown"];

pub(crate) async fn run(http: &reqwest::Client, req: &Request, url: &str, token: Option<&str>, on: Sink<'_>) -> Result<()> {
    let mut msg = json!({
        "role": "user",
        "kind": "message",
        "messageId": id(),
        "parts": [{"kind": "text", "text": with_system(req, prompt(req))}],
    });
    if let Some(c) = &req.session {
        msg["contextId"] = json!(c);
    }
    let mut res = rpc(http, url, token, "message/send", json!({"message": msg, "configuration": {"blocking": true}})).await?;
    if let Some(c) = res["contextId"].as_str() {
        on(Event::Session { id: c.into() });
    }
    // A plain message: that's the answer.
    if res["kind"] == "message" || (res["parts"].is_array() && res["status"].is_null()) {
        on(Event::Text { text: parts_text(&res["parts"]) });
        return Ok(());
    }
    // A task: wait until it's done.
    let start = Instant::now();
    let task_id = res["id"].as_str().unwrap_or("").to_string();
    on(Event::ToolStart { id: task_id.clone(), name: "a2a task".into(), input: truncate(&task_id, 200).into() });
    loop {
        let state = res["status"]["state"].as_str().unwrap_or("unknown").to_string();
        if DONE.contains(&state.as_str()) {
            let text = task_text(&res);
            let failed = matches!(state.as_str(), "failed" | "rejected" | "canceled");
            on(Event::ToolEnd { id: task_id.clone(), output: state.clone(), error: failed });
            if failed {
                bail!("the agent's task {state}{}", if text.is_empty() { String::new() } else { format!(": {text}") });
            }
            on(Event::Text { text });
            if state != "completed" {
                on(Event::Text { text: format!("\n\n(The agent is waiting: {state}.)") });
            }
            return Ok(());
        }
        if start.elapsed() > Duration::from_secs(1800) || task_id.is_empty() {
            bail!("the agent didn't finish in 30 minutes");
        }
        tokio::time::sleep(Duration::from_millis(1500)).await;
        res = rpc(http, url, token, "tasks/get", json!({"id": task_id})).await?;
    }
}
