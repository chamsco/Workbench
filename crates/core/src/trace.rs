//! Agent tracing: every reply is a trace, every tool call a span inside it.
//!
//! Spans follow OpenTelemetry (ids, parents, nanosecond times, attributes
//! named after the GenAI semantic conventions: `gen_ai.request.model`,
//! `gen_ai.usage.input_tokens`…). Traces are kept on this machine in
//! `<data>/traces/<trace id>.json` and shown under each reply. With an
//! endpoint set in Settings → Tracing they're also sent as OTLP/HTTP JSON
//! to any collector (`<endpoint>/v1/traces`), with prompts and tool output
//! only if you allow it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::chat::now_ms;
use backspace_runner::Event;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub span_id: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub name: String,
    /// "reply", "tool".
    pub kind: String,
    /// Unix ms.
    pub start: u64,
    #[serde(default)]
    pub end: Option<u64>,
    #[serde(default)]
    pub attrs: BTreeMap<String, Value>,
    #[serde(default)]
    pub error: Option<String>,
    /// What went in and came out (a tool's input and output).
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub output: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Trace {
    pub trace_id: String,
    pub thread: String,
    pub message: String,
    pub spans: Vec<Span>,
}

/// Where traces go besides this machine.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Export {
    /// An OTLP/HTTP endpoint (without `/v1/traces`); empty: keep traces local.
    pub endpoint: String,
    /// Extra headers, e.g. `authorization: Bearer …`.
    pub headers: BTreeMap<String, String>,
    /// Send prompts, answers and tool output too (off: timings, models,
    /// token counts and tool names only).
    pub content: bool,
}

fn hex(n: usize) -> String {
    let mut s = String::new();
    while s.len() < n {
        s.push_str(&format!("{:08x}", crate::chat::rand_u32()));
    }
    s.truncate(n);
    s
}

/// Records one reply as it happens.
pub struct Recorder {
    pub trace: Trace,
    open: BTreeMap<String, usize>,
}

impl Recorder {
    pub fn start(thread: &str, message: &str, name: &str, attrs: BTreeMap<String, Value>, input: &str) -> Self {
        let root = Span {
            span_id: hex(16),
            parent: None,
            name: name.into(),
            kind: "reply".into(),
            start: now_ms(),
            attrs,
            input: crate::router::truncate(input, 8000).into(),
            ..Default::default()
        };
        Self {
            trace: Trace { trace_id: hex(32), thread: thread.into(), message: message.into(), spans: vec![root] },
            open: BTreeMap::new(),
        }
    }

    fn root(&mut self) -> &mut Span {
        &mut self.trace.spans[0]
    }

    pub fn on(&mut self, e: &Event) {
        let now = now_ms();
        match e {
            Event::Model { model } => {
                self.root().attrs.insert("gen_ai.response.model".into(), json!(model));
            }
            Event::Session { id } => {
                self.root().attrs.insert("session.id".into(), json!(id));
            }
            Event::Usage { input, output } => {
                let r = self.root();
                r.attrs.insert("gen_ai.usage.input_tokens".into(), json!(input));
                r.attrs.insert("gen_ai.usage.output_tokens".into(), json!(output));
            }
            Event::Cost { usd } => {
                self.root().attrs.insert("cost.usd".into(), json!(usd));
            }
            Event::Text { text } => self.root().output.push_str(text),
            Event::Replace { text } => self.root().output = text.clone(),
            Event::ToolStart { id, name, input } => {
                let parent = Some(self.trace.spans[0].span_id.clone());
                self.trace.spans.push(Span {
                    span_id: hex(16),
                    parent,
                    name: name.clone(),
                    kind: "tool".into(),
                    start: now,
                    input: input.clone(),
                    attrs: BTreeMap::from([("gen_ai.tool.name".into(), json!(name)), ("gen_ai.tool.call.id".into(), json!(id))]),
                    ..Default::default()
                });
                self.open.insert(id.clone(), self.trace.spans.len() - 1);
            }
            Event::ToolEnd { id, output, error } => {
                if let Some(i) = self.open.remove(id) {
                    let s = &mut self.trace.spans[i];
                    s.end = Some(now);
                    s.output = output.clone();
                    if *error {
                        s.error = Some("tool reported an error".into());
                    }
                }
            }
            Event::Break | Event::Final { .. } => {}
        }
    }

    /// Close every span; returns the finished trace.
    pub fn finish(mut self, error: Option<String>) -> Trace {
        let now = now_ms();
        for s in &mut self.trace.spans {
            if s.end.is_none() {
                s.end = Some(now);
            }
        }
        let r = &mut self.trace.spans[0];
        r.output = crate::router::truncate(&r.output, 8000).into();
        r.error = error;
        self.trace
    }
}

pub fn dir(data: &Path) -> PathBuf {
    data.join("traces")
}

pub fn save(data: &Path, t: &Trace) -> Result<()> {
    let d = dir(data);
    std::fs::create_dir_all(&d)?;
    std::fs::write(d.join(format!("{}.json", t.trace_id)), serde_json::to_vec(t)?)?;
    Ok(())
}

pub fn load(data: &Path, id: &str) -> Option<Trace> {
    if !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    std::fs::read(dir(data).join(format!("{id}.json"))).ok().and_then(|b| serde_json::from_slice(&b).ok())
}

fn attr(k: &str, v: &Value) -> Value {
    let value = match v {
        Value::String(s) => json!({"stringValue": s}),
        Value::Bool(b) => json!({"boolValue": b}),
        Value::Number(n) if n.is_i64() || n.is_u64() => json!({"intValue": n.to_string()}),
        Value::Number(n) => json!({"doubleValue": n.as_f64()}),
        other => json!({"stringValue": other.to_string()}),
    };
    json!({"key": k, "value": value})
}

/// The trace as an OTLP/HTTP JSON body.
pub fn otlp(t: &Trace, content: bool) -> Value {
    let spans: Vec<Value> = t
        .spans
        .iter()
        .map(|s| {
            let mut attrs: Vec<Value> = s.attrs.iter().map(|(k, v)| attr(k, v)).collect();
            attrs.push(attr("backspace.thread", &json!(t.thread)));
            if content {
                if !s.input.is_empty() {
                    attrs.push(attr("input.value", &json!(s.input)));
                }
                if !s.output.is_empty() {
                    attrs.push(attr("output.value", &json!(s.output)));
                }
            }
            let mut v = json!({
                "traceId": t.trace_id,
                "spanId": s.span_id,
                "name": s.name,
                "kind": if s.kind == "tool" { 1 } else { 3 },
                "startTimeUnixNano": (s.start as u128 * 1_000_000).to_string(),
                "endTimeUnixNano": (s.end.unwrap_or(s.start) as u128 * 1_000_000).to_string(),
                "attributes": attrs,
                "status": match &s.error { Some(e) => json!({"code": 2, "message": e}), None => json!({"code": 1}) },
            });
            if let Some(p) = &s.parent {
                v["parentSpanId"] = json!(p);
            }
            v
        })
        .collect();
    json!({"resourceSpans": [{
        "resource": {"attributes": [attr("service.name", &json!("backspace")), attr("service.version", &json!(env!("CARGO_PKG_VERSION")))]},
        "scopeSpans": [{"scope": {"name": "backspace.chat"}, "spans": spans}],
    }]})
}

/// Send a trace to the configured collector. Errors are returned for
/// Settings' "Send a test trace"; replies ignore them.
pub async fn export(http: &reqwest::Client, cfg: &Export, t: &Trace) -> Result<()> {
    if cfg.endpoint.trim().is_empty() {
        return Ok(());
    }
    let base = cfg.endpoint.trim().trim_end_matches('/');
    let url = if base.ends_with("/v1/traces") { base.to_string() } else { format!("{base}/v1/traces") };
    let mut rb = http.post(&url).timeout(std::time::Duration::from_secs(15)).json(&otlp(t, cfg.content));
    for (k, v) in &cfg.headers {
        rb = rb.header(k.as_str(), v.as_str());
    }
    let r = rb.send().await?;
    if !r.status().is_success() {
        anyhow::bail!("{url}: {}", r.status());
    }
    Ok(())
}

/// Send one small trace to check a collector's address and sign-in.
pub async fn send_test(cfg: &Export) -> Result<()> {
    if cfg.endpoint.trim().is_empty() {
        anyhow::bail!("enter the collector's address first");
    }
    let mut r = Recorder::start("test", "test", "Backspace test trace", BTreeMap::new(), "");
    r.on(&Event::ToolStart { id: "t".into(), name: "check".into(), input: String::new() });
    r.on(&Event::ToolEnd { id: "t".into(), output: String::new(), error: false });
    export(&reqwest::Client::new(), cfg, &r.finish(None)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_tools_and_exports_otlp() {
        let mut r = Recorder::start("th1", "m1", "reply · Kira", BTreeMap::from([("gen_ai.system".into(), json!("claude"))]), "hello");
        for e in [
            Event::Model { model: "claude-x".into() },
            Event::Text { text: "Looking.".into() },
            Event::ToolStart { id: "t1".into(), name: "Bash".into(), input: "ls".into() },
            Event::ToolEnd { id: "t1".into(), output: "a.txt".into(), error: false },
            Event::ToolStart { id: "t2".into(), name: "Read".into(), input: "b".into() },
            Event::ToolEnd { id: "t2".into(), output: "no such file".into(), error: true },
            Event::Usage { input: 10, output: 3 },
        ] {
            r.on(&e);
        }
        let t = r.finish(None);
        assert_eq!(t.trace_id.len(), 32);
        assert_eq!(t.spans.len(), 3);
        assert_eq!(t.spans[1].parent.as_deref(), Some(t.spans[0].span_id.as_str()));
        assert_eq!(t.spans[1].output, "a.txt");
        assert!(t.spans[2].error.is_some());
        assert_eq!(t.spans[0].attrs["gen_ai.usage.input_tokens"], json!(10));
        let o = otlp(&t, false);
        let spans = &o["resourceSpans"][0]["scopeSpans"][0]["spans"];
        assert_eq!(spans.as_array().unwrap().len(), 3);
        assert_eq!(spans[2]["status"]["code"], 2);
        assert!(!o.to_string().contains("a.txt"), "no content unless allowed");
        assert!(otlp(&t, true).to_string().contains("a.txt"));
        let dir = std::env::temp_dir().join(format!("bs-trace-{}", now_ms()));
        save(&dir, &t).unwrap();
        assert_eq!(load(&dir, &t.trace_id).unwrap(), t);
        assert!(load(&dir, "../x").is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn exports_to_a_collector() {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let got = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut buf = vec![0u8; 65536];
            let mut n = 0;
            loop {
                n += s.read(&mut buf[n..]).unwrap();
                let t = String::from_utf8_lossy(&buf[..n]).to_string();
                if let Some(i) = t.find("\r\n\r\n") {
                    let len: usize = t[..i].lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap())).unwrap_or(0);
                    if n >= i + 4 + len {
                        s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}").unwrap();
                        return t;
                    }
                }
            }
        });
        let cfg = Export { endpoint: format!("http://{addr}"), headers: BTreeMap::from([("authorization".into(), "Bearer k1".into())]), content: false };
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(send_test(&cfg)).unwrap();
        let req = got.join().unwrap();
        assert!(req.starts_with("POST /v1/traces "), "{req}");
        assert!(req.to_lowercase().contains("authorization: bearer k1"));
        assert!(req.contains("\"resourceSpans\"") && req.contains("Backspace test trace"));
    }
}
