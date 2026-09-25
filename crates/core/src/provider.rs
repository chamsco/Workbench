//! Two wire formats cover almost every model: Anthropic Messages and
//! OpenAI-compatible Chat Completions. Conversations are kept in one
//! provider-neutral shape and translated at the edge.

use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::{ModelSpec, ProviderConfig, ProviderKind};
use crate::effort::Effort;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stop {
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
}

pub async fn complete(http: &reqwest::Client, req: Request<'_>) -> Result<Completion> {
    let (url, body) = match req.provider.kind {
        ProviderKind::Anthropic => (
            format!(
                "{}/v1/messages",
                req.provider.base_url.trim_end_matches('/')
            ),
            anthropic_body(&req),
        ),
        ProviderKind::Openai => (
            format!(
                "{}/chat/completions",
                req.provider.base_url.trim_end_matches('/')
            ),
            openai_body(&req),
        ),
    };

    let mut attempt = 0;
    let v: Value = loop {
        let mut rb = http
            .post(&url)
            .json(&body)
            .timeout(Duration::from_secs(600));
        let key = req.provider.api_key();
        rb = match req.provider.kind {
            ProviderKind::Anthropic => rb
                .header("x-api-key", key.unwrap_or_default())
                .header("anthropic-version", "2023-06-01"),
            ProviderKind::Openai => match key {
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
        ProviderKind::Openai => parse_openai(v),
    }
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

    let tools: Vec<Value> = req
        .tools
        .iter()
        .map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.schema}))
        .collect();

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
    // Cache reads bill at 0.1x and writes at 1.25x; fold them in so cost math
    // downstream stays a single multiply.
    Ok(Completion {
        content,
        stop,
        input_tokens: u["input_tokens"].as_u64().unwrap_or(0)
            + u["cache_read_input_tokens"].as_u64().unwrap_or(0) / 10
            + u["cache_creation_input_tokens"].as_u64().unwrap_or(0) * 5 / 4,
        output_tokens: u["output_tokens"].as_u64().unwrap_or(0),
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
        let sonnet = cfg.model("claude-sonnet-5").unwrap();
        let b = anthropic_body(&Request {
            provider: p,
            model: sonnet,
            effort: Effort::Ultra,
            system: "s",
            messages: &msgs,
            tools: &[],
        });
        assert_eq!(b["output_config"]["effort"], "max");
        assert_eq!(b["thinking"]["type"], "adaptive");
        let haiku = cfg.model("claude-haiku-4-5").unwrap();
        let b = anthropic_body(&Request {
            provider: p,
            model: haiku,
            effort: Effort::High,
            system: "s",
            messages: &msgs,
            tools: &[],
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
