//! Two wire formats cover almost every model: Anthropic Messages and
//! OpenAI-compatible Chat Completions. Conversations are kept in one
//! provider-neutral shape and translated at the edge.

use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::{ModelSpec, ProviderConfig, ProviderKind};
use crate::effort::Effort;
use crate::gateway::Authorization;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    /// Provider blocks we must echo back verbatim (e.g. Anthropic thinking).
    Opaque {
        value: Value,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Block>,
}

impl Message {
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![Block::Text { text: text.into() }],
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
}

/// A `String` that can be handed out as `&'static str`.
///
/// Tool definitions carry `&'static str`, but a tool set built at runtime
/// (Colony's, from another crate) comes back as owned strings. The set is
/// fixed for the life of the process, so leaking it once is bounded.
pub fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stop {
    /// A server tool (the advisor) paused the turn: send it again to go on.
    Pause,
    EndTurn,
    ToolUse,
    MaxTokens,
    Refusal,
}

#[derive(Clone, Debug)]
pub struct Completion {
    pub content: Vec<Block>,
    pub stop: Stop,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// The advisor's own (input, output) tokens, billed at its rates.
    pub advisor_tokens: (u64, u64),
    /// Input tokens as the provider counted them, cache reads and writes at
    /// face value. `input_tokens` weights them by price, for cost math.
    pub raw_input_tokens: u64,
}

impl Completion {
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| match b {
                Block::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub struct Request<'a> {
    pub provider: &'a ProviderConfig,
    pub model: &'a ModelSpec,
    pub effort: Effort,
    pub system: &'a str,
    pub messages: &'a [Message],
    pub tools: &'a [ToolDef],
    /// A stronger model the executor may consult (Anthropic's advisor tool).
    pub advisor: Option<&'a str>,
}

/// What the input of a request is priced at, relative to the model's input
/// rate, in the worst case: Anthropic bills a cache write at 1.25x.
pub fn input_price_factor(kind: ProviderKind) -> f64 {
    match kind {
        ProviderKind::Anthropic => 1.25,
        ProviderKind::Openai | ProviderKind::Cli => 1.0,
    }
}

/// A request as it will be sent, built once so that what is measured is what
/// is sent.
pub struct Prepared {
    pub url: String,
    pub body: Value,
    /// The exact bytes of the body.
    pub bytes: Vec<u8>,
    /// Messages and tool definitions in the wire body, as the provider counts them.
    pub messages: usize,
    pub tools: usize,
}

pub fn prepare(req: &Request) -> Result<Prepared> {
    let (url, body) = match req.provider.kind {
        ProviderKind::Anthropic => (
            format!("{}/v1/messages", req.provider.base_url.trim_end_matches('/')),
            anthropic_body(req),
        ),
        ProviderKind::Openai => (
            format!("{}/chat/completions", req.provider.base_url.trim_end_matches('/')),
            openai_body(req),
        ),
        ProviderKind::Cli => bail!("{} is a CLI harness, not an API", req.model.id),
    };
    let bytes = serde_json::to_vec(&body)?;
    let count = |k: &str| body[k].as_array().map_or(0, |a| a.len());
    // Anthropic keeps the system prompt outside `messages`; OpenAI's is the first one.
    let system = usize::from(req.provider.kind == ProviderKind::Anthropic);
    let (messages, tools) = (count("messages") + system, count("tools"));
    Ok(Prepared { url, body, bytes, messages, tools })
}

pub async fn complete(http: &reqwest::Client, req: Request<'_>, authorization: &Authorization<'_>) -> Result<Completion> {
    let prepared = prepare(&req)?;
    // A request that grew after it was reserved is not sent.
    authorization.check_request(crate::gateway::input_bound(prepared.bytes.len(), prepared.messages, prepared.tools))?;
    let (url, body) = (prepared.url, prepared.body);

    let mut attempt = 0;
    let v: Value = loop {
        let mut rb = http
            .post(&url)
            .json(&body)
            .timeout(Duration::from_secs(600));
        let key = req.provider.api_key();
        rb = match req.provider.kind {
            ProviderKind::Anthropic if req.advisor.is_some() => rb
                .header("x-api-key", key.unwrap_or_default())
                .header("anthropic-version", "2023-06-01")
                .header("anthropic-beta", "advisor-tool-2026-03-01"),
            ProviderKind::Anthropic => rb
                .header("x-api-key", key.unwrap_or_default())
                .header("anthropic-version", "2023-06-01"),
            ProviderKind::Openai | ProviderKind::Cli => match key {
                Some(k) => rb.bearer_auth(k),
                None => rb,
            },
        };
        let resp = rb.send().await;
        let retryable = match &resp {
            Ok(r) => r.status().as_u16() == 429 || r.status().is_server_error(),
            Err(e) => e.is_connect() || e.is_timeout(),
        };
        if retryable && attempt < 3 {
            attempt += 1;
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
            continue;
        }
        let resp = resp.context("inference request failed")?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            bail!(
                "{} {status}: {}",
                req.model.id,
                crate::router::truncate(&text, 600)
            );
        }
        break serde_json::from_str(&text).context("provider returned non-JSON")?;
    };

    match req.provider.kind {
        ProviderKind::Anthropic => parse_anthropic(v),
        _ => parse_openai(v),
    }
}

/// Run a turn through `backspace-runner` (a coding CLI, a local runtime, the
/// router). The one place this crate enters the runner, so every such call
/// says how it is metered. A reserved call is checked against its reservation
/// by the caller (a CLI turn reports usage as it goes; see `run_cli_turn`).
pub async fn run_target(
    http: &reqwest::Client,
    req: &backspace_runner::Request,
    on: backspace_runner::Sink<'_>,
    authorization: &Authorization<'_>,
) -> Result<()> {
    let out = backspace_runner::run(http, req, on).await;
    if let Authorization::Unmetered(why) = authorization {
        note_unmetered(*why);
    }
    out
}

/// Count a model call that is not metered by a reservation, by reason.
pub fn note_unmetered(why: crate::gateway::Unmetered) {
    let mut m = UNMETERED.lock().unwrap();
    *m.entry(why.why()).or_insert(0) += 1;
}

static UNMETERED: std::sync::Mutex<std::collections::BTreeMap<&'static str, u64>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// How many model calls each unmetered reason has covered in this process.
pub fn unmetered_calls() -> std::collections::BTreeMap<&'static str, u64> {
    UNMETERED.lock().unwrap().clone()
}

// ---------------------------------------------------------------- anthropic

fn anthropic_body(req: &Request) -> Value {
    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            let content: Vec<Value> = m
                .content
                .iter()
                .map(|b| match b {
                    Block::Text { text } => json!({"type": "text", "text": text}),
                    Block::ToolUse { id, name, input } => {
                        json!({"type": "tool_use", "id": id, "name": name, "input": input})
                    }
                    Block::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => json!({
                        "type": "tool_result", "tool_use_id": tool_use_id,
                        "content": content, "is_error": is_error,
                    }),
                    Block::Opaque { value } => value.clone(),
                })
                .collect();
            json!({"role": m.role, "content": content})
        })
        .collect();

    let mut tools: Vec<Value> = req
        .tools
        .iter()
        .map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.schema}))
        .collect();
    if let Some(a) = req.advisor {
        tools.push(json!({"type": "advisor_20260301", "name": "advisor", "model": a}));
    }

    let mut body = json!({
        "model": req.model.wire_model(),
        "max_tokens": req.model.max_output_tokens,
        // Stable prefix (tools + system) is cached across every turn of the loop.
        "cache_control": {"type": "ephemeral"},
        "system": req.system,
        "messages": messages,
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
    }
    if let Some(e) = req.effort.clamp_to(&req.model.efforts) {
        body["thinking"] = json!({"type": "adaptive"});
        body["output_config"] = json!({"effort": wire_effort(e)});
    }
    body
}

fn wire_effort(e: Effort) -> &'static str {
    match e {
        Effort::Ultra => "max",
        other => other.as_str(),
    }
}

fn parse_anthropic(v: Value) -> Result<Completion> {
    let stop = match v["stop_reason"].as_str() {
        Some("tool_use") => Stop::ToolUse,
        Some("pause_turn") => Stop::Pause,
        Some("max_tokens") => Stop::MaxTokens,
        Some("refusal") => Stop::Refusal,
        _ => Stop::EndTurn,
    };
    let content = v["content"]
        .as_array()
        .ok_or_else(|| anyhow!("anthropic response has no content"))?
        .iter()
        .map(|b| match b["type"].as_str() {
            Some("text") => Block::Text {
                text: b["text"].as_str().unwrap_or_default().into(),
            },
            Some("tool_use") => Block::ToolUse {
                id: b["id"].as_str().unwrap_or_default().into(),
                name: b["name"].as_str().unwrap_or_default().into(),
                input: b["input"].clone(),
            },
            _ => Block::Opaque { value: b.clone() },
        })
        .collect();
    let u = &v["usage"];
    // The advisor's tokens come per iteration, apart from the executor's.
    let (mut adv_in, mut adv_out) = (0, 0);
    for it in u["iterations"].as_array().into_iter().flatten().filter(|i| i["type"] == "advisor_message") {
        adv_in += it["input_tokens"].as_u64().unwrap_or(0);
        adv_out += it["output_tokens"].as_u64().unwrap_or(0);
    }
    // Cache reads bill at 0.1x and writes at 1.25x; fold them in so cost math
    // downstream stays a single multiply.
    Ok(Completion {
        content,
        stop,
        input_tokens: u["input_tokens"].as_u64().unwrap_or(0)
            + u["cache_read_input_tokens"].as_u64().unwrap_or(0) / 10
            + u["cache_creation_input_tokens"].as_u64().unwrap_or(0) * 5 / 4,
        output_tokens: u["output_tokens"].as_u64().unwrap_or(0),
        advisor_tokens: (adv_in, adv_out),
        raw_input_tokens: u["input_tokens"].as_u64().unwrap_or(0)
            + u["cache_read_input_tokens"].as_u64().unwrap_or(0)
            + u["cache_creation_input_tokens"].as_u64().unwrap_or(0),
    })
}

// ------------------------------------------------------------------- openai

fn openai_body(req: &Request) -> Value {
    let mut messages = vec![json!({"role": "system", "content": req.system})];
    for m in req.messages {
        match m.role {
            Role::User => {
                let mut text = Vec::new();
                for b in &m.content {
                    match b {
                        Block::ToolResult { tool_use_id, content, .. } => messages.push(
                            json!({"role": "tool", "tool_call_id": tool_use_id, "content": content}),
                        ),
                        Block::Text { text: t } => text.push(t.clone()),
                        _ => {}
                    }
                }
                if !text.is_empty() {
                    messages.push(json!({"role": "user", "content": text.join("\n")}));
                }
            }
            Role::Assistant => {
                let mut text = Vec::new();
                let mut calls = Vec::new();
                for b in &m.content {
                    match b {
                        Block::Text { text: t } => text.push(t.clone()),
                        Block::ToolUse { id, name, input } => calls.push(json!({
                            "id": id, "type": "function",
                            "function": {"name": name, "arguments": input.to_string()},
                        })),
                        _ => {}
                    }
                }
                let mut msg = json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { Value::String(text.join("\n")) }});
                if !calls.is_empty() {
                    msg["tool_calls"] = Value::Array(calls);
                }
                messages.push(msg);
            }
        }
    }
    let tools: Vec<Value> = req
        .tools
        .iter()
        .map(|t| json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.schema}}))
        .collect();
    let mut body = json!({
        "model": req.model.wire_model(),
        "max_tokens": req.model.max_output_tokens,
        "messages": messages,
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
    }
    if let Some(e) = req.effort.clamp_to(&req.model.efforts) {
        body["reasoning_effort"] = Value::String(wire_effort(e).into());
    }
    body
}

fn parse_openai(v: Value) -> Result<Completion> {
    let choice = &v["choices"][0];
    let msg = &choice["message"];
    let mut content = Vec::new();
    if let Some(t) = msg["content"].as_str().filter(|t| !t.is_empty()) {
        content.push(Block::Text { text: t.into() });
    }
    if let Some(calls) = msg["tool_calls"].as_array() {
        for c in calls {
            let args = c["function"]["arguments"].as_str().unwrap_or("{}");
            content.push(Block::ToolUse {
                id: c["id"].as_str().unwrap_or_default().into(),
                name: c["function"]["name"].as_str().unwrap_or_default().into(),
                input: serde_json::from_str(args).unwrap_or(Value::Object(Default::default())),
            });
        }
    }
    let has_calls = content.iter().any(|b| matches!(b, Block::ToolUse { .. }));
    let stop = match choice["finish_reason"].as_str() {
        _ if has_calls => Stop::ToolUse,
        Some("length") => Stop::MaxTokens,
        Some("content_filter") => Stop::Refusal,
        _ => Stop::EndTurn,
    };
    Ok(Completion {
        content,
        stop,
        input_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
        output_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0),
        advisor_tokens: (0, 0),
        raw_input_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, DEFAULT_CONFIG};

    #[test]
    fn anthropic_body_sets_effort_only_when_supported() {
        let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
        let p = &cfg.providers["anthropic"];
        let msgs = [Message::user_text("hi")];
        let sonnet = cfg.model("claude-sonnet-5-5").unwrap();
        let b = anthropic_body(&Request {
            provider: p,
            model: sonnet,
            effort: Effort::Ultra,
            system: "s",
            messages: &msgs,
            tools: &[],
            advisor: Some("claude-opus-5-5"),
        });
        assert_eq!(b["output_config"]["effort"], "max");
        assert_eq!(b["thinking"]["type"], "adaptive");
        // The advisor rides along as a server tool.
        assert_eq!(b["tools"][0]["type"], "advisor_20260301");
        assert_eq!(b["tools"][0]["model"], "claude-opus-5-5");
        let haiku = cfg.model("claude-haiku-4-5").unwrap();
        let b = anthropic_body(&Request {
            provider: p,
            model: haiku,
            effort: Effort::High,
            system: "s",
            messages: &msgs,
            tools: &[],
            advisor: None,
        });
        assert!(b.get("output_config").is_none());
    }

    #[test]
    fn openai_roundtrips_tool_calls() {
        let v = json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null,
            "tool_calls": [{"id": "c1", "function": {"name": "read", "arguments": "{\"path\":\"a\"}"}}]}}],
            "usage": {"prompt_tokens": 3, "completion_tokens": 4}});
        let c = parse_openai(v).unwrap();
        assert_eq!(c.stop, Stop::ToolUse);
        assert!(matches!(&c.content[0], Block::ToolUse { name, .. } if name == "read"));
    }
}
