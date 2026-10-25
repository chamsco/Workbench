//! Usage stats from other tools' session logs (docs/ROADMAP-OBSERVABILITY.md).
//!
//! Read-only readers for Claude Code (`~/.claude/projects/**/*.jsonl`) and
//! Codex (`~/.codex/sessions/**/*.jsonl`) that fold each log into one
//! [`Session`] and the whole window into a [`Summary`]. Only counts, timings
//! and tool names are kept: never prompts, tool input or tool output.
//! What a log doesn't record stays `None` and is shown as "not logged".

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const DAY_MS: u64 = 86_400_000;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }
    fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_write += o.cache_write;
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Session {
    /// First 8 characters of the log's session id.
    pub id: String,
    /// "claude-code" or "codex".
    pub tool: String,
    /// Last folder of the working directory.
    pub project: String,
    pub subagent: bool,
    pub start: u64,
    pub end: u64,
    /// Typed or sent by a person or app (not tool results, notifications, meta).
    pub prompts: u32,
    pub tool_calls: u32,
    pub errors: u32,
    pub tokens: Tokens,
    /// Tokens per model; synthetic messages are left out.
    pub models: BTreeMap<String, Tokens>,
    pub calls_by_tool: BTreeMap<String, u32>,
    /// Seconds from tool call to tool result, per tool.
    pub latency_s: BTreeMap<String, Vec<f64>>,
    /// Model-call errors such as `usage_limit_reached` (not tool errors).
    pub api_errors: BTreeMap<String, u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub since: u64,
    pub until: u64,
    pub sessions: Vec<Session>,
    pub tool_calls: u32,
    pub errors: u32,
    pub prompts: u32,
    pub tokens: Tokens,
    /// `None` when no call finished (shown as "not logged").
    pub p95_latency_s: Option<f64>,
}

/// Unix ms from `2026-10-25T00:38:55.123Z` (UTC, optional fraction).
pub fn parse_ms(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[10] != b'T' {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, m, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (h, mi, sec) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    let frac = s.get(19..).and_then(|r| r.strip_prefix('.')).map_or(0, |r| {
        let digits: String = r.chars().take_while(|c| c.is_ascii_digit()).take(3).collect();
        format!("{digits:0<3}").parse::<i64>().unwrap_or(0)
    });
    // Days since 1970-01-01 (civil-from-days inverse).
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let ms = ((days * 24 + h) * 60 + mi) * 60 + sec;
    u64::try_from(ms * 1000 + frac).ok()
}

fn jsonl(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .map(|t| t.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
        .unwrap_or_default()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "jsonl") {
            out.push(p);
        }
    }
}

fn project_of(cwd: &str) -> String {
    cwd.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or("").to_string()
}

fn short(id: &str) -> String {
    id.chars().take(8).collect()
}

/// Ids already counted, shared across files: resumed and compacted sessions
/// and subagent files re-copy history.
#[derive(Default)]
pub struct Seen {
    messages: HashSet<String>,
    calls: HashSet<String>,
}

/// One Claude Code log. Streamed content blocks share a `message.id`, so
/// tokens are counted once per id; a call counts once per `tool_use.id`.
pub fn read_claude(path: &Path, since: u64, seen: &mut Seen) -> Option<Session> {
    let subagent = path.components().any(|c| c.as_os_str() == "subagents");
    let stem = path.file_stem()?.to_string_lossy().replace("agent-", "");
    let mut s = Session { id: short(&stem), tool: "claude-code".into(), subagent, ..Default::default() };
    let mut pending: HashMap<String, (String, u64)> = HashMap::new();
    for o in jsonl(path) {
        let Some(t) = o["timestamp"].as_str().and_then(parse_ms) else { continue };
        if t < since {
            continue;
        }
        if s.start == 0 || t < s.start {
            s.start = t;
        }
        s.end = s.end.max(t);
        if s.project.is_empty() {
            s.project = o["cwd"].as_str().map(project_of).unwrap_or_default();
        }
        match o["type"].as_str() {
            Some("assistant") => {
                let m = &o["message"];
                if let Some(e) = o["apiError"].as_str() {
                    *s.api_errors.entry(e.chars().take(40).collect()).or_default() += 1;
                }
                let model = m["model"].as_str().unwrap_or("");
                if model.is_empty() || model == "<synthetic>" {
                    continue;
                }
                if let Some(id) = m["id"].as_str() {
                    if seen.messages.insert(id.to_string()) {
                        let u = &m["usage"];
                        let g = |k: &str| u[k].as_u64().unwrap_or(0);
                        let tk = Tokens {
                            input: g("input_tokens"),
                            output: g("output_tokens"),
                            cache_read: g("cache_read_input_tokens"),
                            cache_write: g("cache_creation_input_tokens"),
                        };
                        s.tokens.add(&tk);
                        s.models.entry(model.to_string()).or_default().add(&tk);
                    }
                }
                for b in m["content"].as_array().into_iter().flatten() {
                    let ty = b["type"].as_str().unwrap_or("");
                    if let (Some(id), true) = (b["tool_use_id"].as_str(), ty.ends_with("_result")) {
                        // Server-side tools (advisor, web search) answer in the assistant turn.
                        finish(&mut s, &mut pending, id, t, b["content"]["type"].as_str().is_some_and(|x| x.contains("error")));
                    } else if ty == "tool_use" || ty == "server_tool_use" {
                        let Some(id) = b["id"].as_str() else { continue };
                        if seen.calls.insert(id.to_string()) {
                            let name = b["name"].as_str().unwrap_or("unknown").to_string();
                            s.tool_calls += 1;
                            *s.calls_by_tool.entry(name.clone()).or_default() += 1;
                            pending.insert(id.to_string(), (name, t));
                        }
                    }
                }
            }
            Some("user") => {
                let c = &o["message"]["content"];
                let mut has_result = false;
                for b in c.as_array().into_iter().flatten() {
                    if b["type"] == "tool_result" {
                        has_result = true;
                        if let Some(id) = b["tool_use_id"].as_str() {
                            finish(&mut s, &mut pending, id, t, b["is_error"] == true);
                        }
                    }
                }
                let typed = match c {
                    Value::String(x) => !x.trim().is_empty(),
                    Value::Array(a) => a.iter().any(|b| b["type"] == "text"),
                    _ => false,
                };
                let skip = has_result
                    || o["isMeta"] == true
                    || o["isCompactSummary"] == true
                    || o["isSidechain"] == true
                    || o["turnOrigin"] == "task_notification";
                if typed && !skip {
                    s.prompts += 1;
                }
            }
            _ => {}
        }
    }
    (s.start != 0).then_some(s)
}

fn finish(s: &mut Session, pending: &mut HashMap<String, (String, u64)>, id: &str, t: u64, failed: bool) {
    let Some((name, t0)) = pending.remove(id) else { return };
    s.latency_s.entry(name).or_default().push(t.saturating_sub(t0) as f64 / 1000.0);
    if failed {
        s.errors += 1;
    }
}

/// One Codex rollout. Tokens come from per-response `token_usage_record`s;
/// the `thread_token_usage` field is cumulative and is never summed.
pub fn read_codex(path: &Path, since: u64) -> Option<Session> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let mut s = Session { id: short(&stem), tool: "codex".into(), ..Default::default() };
    let mut model = String::from("unknown");
    let mut pending: HashMap<String, (String, u64)> = HashMap::new();
    for o in jsonl(path) {
        let p = &o["payload"];
        if o["type"] == "session_meta" {
            if let Some(c) = p["cwd"].as_str() {
                s.project = project_of(c);
            }
            if let Some(i) = p["id"].as_str() {
                s.id = short(i);
            }
        }
        let Some(t) = o["timestamp"].as_str().and_then(parse_ms) else { continue };
        if t < since {
            continue;
        }
        if s.start == 0 || t < s.start {
            s.start = t;
        }
        s.end = s.end.max(t);
        let pty = p["type"].as_str().unwrap_or("");
        match (o["type"].as_str().unwrap_or(""), pty) {
            ("turn_context", _) => {
                if let Some(m) = p["model"].as_str() {
                    model = m.to_string();
                }
            }
            ("token_usage_record", _) => {
                let u = &p["usage"];
                let g = |k: &str| u[k].as_u64().unwrap_or(0);
                let cached = g("cached_input_tokens");
                let tk = Tokens {
                    input: g("input_tokens").saturating_sub(cached),
                    output: g("output_tokens"),
                    cache_read: cached,
                    cache_write: g("cache_write_input_tokens"),
                };
                s.tokens.add(&tk);
                s.models.entry(model.clone()).or_default().add(&tk);
            }
            ("response_item", "message") if p["role"] == "user" => {
                let text: String = p["content"].as_array().into_iter().flatten().filter_map(|c| c["text"].as_str()).collect();
                // Developer/system context starts with a tag; a prompt doesn't.
                if !text.trim().is_empty() && !text.trim_start().starts_with('<') {
                    s.prompts += 1;
                }
            }
            ("response_item", "function_call" | "custom_tool_call" | "local_shell_call") => {
                let id = p["call_id"].as_str().or(p["id"].as_str()).unwrap_or("").to_string();
                let name = p["name"].as_str().unwrap_or(pty).to_string();
                s.tool_calls += 1;
                *s.calls_by_tool.entry(name.clone()).or_default() += 1;
                pending.insert(id, (name, t));
            }
            ("response_item", "function_call_output" | "custom_tool_call_output") => {
                if let Some(id) = p["call_id"].as_str() {
                    let out = p["output"].to_string();
                    let failed = out.contains("\"success\":false") || out.contains("\"exit_code\":1");
                    finish(&mut s, &mut pending, id, t, failed);
                }
            }
            _ => {}
        }
    }
    (s.start != 0).then_some(s)
}

/// Reads every log under `home` modified inside the window.
/// `exclude` drops sessions whose id starts with it (a session that is still running).
pub fn summarize(home: &Path, now_ms: u64, days: u32, exclude: &str) -> Summary {
    let since = now_ms.saturating_sub(days as u64 * DAY_MS);
    let fresh = |p: &PathBuf| {
        std::fs::metadata(p).and_then(|m| m.modified()).ok().is_some_and(|m| {
            let ms = m.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
            ms + DAY_MS >= since
        })
    };
    let mut seen = Seen::default();
    let mut sessions = Vec::new();
    let mut files = Vec::new();
    walk(&home.join(".claude").join("projects"), &mut files);
    files.sort();
    sessions.extend(files.iter().filter(|p| fresh(p)).filter_map(|p| read_claude(p, since, &mut seen)));
    let mut files = Vec::new();
    walk(&home.join(".codex").join("sessions"), &mut files);
    files.sort();
    sessions.extend(files.iter().filter(|p| fresh(p)).filter_map(|p| read_codex(p, since)));
    if !exclude.is_empty() {
        sessions.retain(|s| !s.id.starts_with(exclude));
    }
    sessions.sort_by_key(|s| std::cmp::Reverse(s.tokens.total()));
    let mut sum = Summary { since, until: now_ms, ..Default::default() };
    let mut lat: Vec<f64> = Vec::new();
    for s in &sessions {
        sum.tool_calls += s.tool_calls;
        sum.errors += s.errors;
        sum.prompts += s.prompts;
        sum.tokens.add(&s.tokens);
        lat.extend(s.latency_s.values().flatten());
    }
    lat.sort_by(|a, b| a.total_cmp(b));
    sum.p95_latency_s = (!lat.is_empty()).then(|| lat[((lat.len() - 1) as f64 * 0.95) as usize]);
    sum.sessions = sessions;
    sum
}

/// A tool whose logs live on this device, and whether Backspace can read them yet.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Source {
    pub id: &'static str,
    pub name: &'static str,
    /// Its data folder exists on this device.
    pub found: bool,
    /// A reader exists (otherwise it's listed so you can see what's missing).
    pub supported: bool,
    pub note: &'static str,
}

/// Every agent tool Backspace knows a log location for.
pub fn sources(home: &Path) -> Vec<Source> {
    let has = |rel: &str| home.join(rel).exists();
    vec![
        Source { id: "claude-code", name: "Claude Code", found: has(".claude/projects"), supported: true, note: "JSONL sessions" },
        Source { id: "codex", name: "Codex", found: has(".codex/sessions"), supported: true, note: "JSONL rollouts" },
        Source { id: "opencode", name: "OpenCode", found: has(".local/share/opencode/opencode.db"), supported: false, note: "SQLite database; needs a SQLite reader" },
        Source { id: "cursor", name: "Cursor", found: has(".cursor/ai-tracking"), supported: false, note: "SQLite tracking data; needs a SQLite reader" },
        Source { id: "gemini", name: "Gemini CLI", found: has(".gemini/tmp"), supported: false, note: "chat JSON; reader not written yet" },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str, body: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bs-usage-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(format!("{name}.jsonl"));
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn sources_report_what_exists() {
        let d = std::env::temp_dir().join(format!("bs-src-{}", std::process::id()));
        std::fs::create_dir_all(d.join(".codex/sessions")).unwrap();
        let v = sources(&d);
        assert!(v.iter().find(|s| s.id == "codex").unwrap().found);
        assert!(!v.iter().find(|s| s.id == "claude-code").unwrap().found);
        assert!(v.iter().filter(|s| s.supported).count() == 2);
    }

    #[test]
    fn parses_iso_times() {
        assert_eq!(parse_ms("1970-01-01T00:00:01.5Z"), Some(1500));
        assert_eq!(parse_ms("2026-10-25T00:38:55.000Z"), Some(1_792_888_735_000));
        assert_eq!(parse_ms("nope"), None);
    }

    #[test]
    fn claude_counts_once_per_message_and_call() {
        // Two streamed blocks of one message repeat the same usage and tool_use.
        let a = r#"{"type":"assistant","timestamp":"2026-10-25T00:00:01Z","cwd":"C:\\x\\proj","message":{"id":"m1","model":"claude-opus-5-5","usage":{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":100,"cache_creation_input_tokens":7},"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{}}]}}"#;
        let r = r#"{"type":"user","timestamp":"2026-10-25T00:00:03Z","message":{"content":[{"type":"tool_result","tool_use_id":"t1","is_error":true,"content":"secret"}]}}"#;
        let p = r#"{"type":"user","timestamp":"2026-10-25T00:00:04Z","message":{"content":"hello"}}"#;
        let n = r#"{"type":"user","timestamp":"2026-10-25T00:00:05Z","turnOrigin":"task_notification","message":{"content":"x"}}"#;
        let syn = r#"{"type":"assistant","timestamp":"2026-10-25T00:00:06Z","message":{"id":"m2","model":"<synthetic>","usage":{"input_tokens":99},"content":[]}}"#;
        let path = tmp("c1", &[a, a, r, p, n, syn].join("\n"));
        let mut seen = Seen::default();
        let s = read_claude(&path, 0, &mut seen).unwrap();
        assert_eq!((s.tool_calls, s.errors, s.prompts), (1, 1, 1));
        assert_eq!(s.tokens, Tokens { input: 10, output: 5, cache_read: 100, cache_write: 7 });
        assert_eq!(s.project, "proj");
        assert_eq!(s.latency_s["Bash"], vec![2.0]);
        // A second file re-copying the same history adds nothing.
        let again = read_claude(&tmp("c2", a), 0, &mut seen).unwrap();
        assert_eq!((again.tokens.total(), again.tool_calls), (0, 0));
    }

    #[test]
    fn window_uses_record_time() {
        let old = r#"{"type":"user","timestamp":"2026-10-01T00:00:00Z","message":{"content":"old"}}"#;
        let path = tmp("c3", old);
        assert!(read_claude(&path, parse_ms("2026-10-18T00:00:00Z").unwrap(), &mut Seen::default()).is_none());
    }

    #[test]
    fn codex_sums_records_not_cumulative() {
        let meta = r#"{"type":"session_meta","timestamp":"2026-10-24T18:50:00Z","payload":{"id":"abcdef12-0000","cwd":"/w/space"}}"#;
        let ctx = r#"{"type":"turn_context","timestamp":"2026-10-24T18:50:01Z","payload":{"model":"gpt-x"}}"#;
        let dev = r#"{"type":"response_item","timestamp":"2026-10-24T18:50:02Z","payload":{"type":"message","role":"user","content":[{"text":"<env>"}]}}"#;
        let usr = r#"{"type":"response_item","timestamp":"2026-10-24T18:50:03Z","payload":{"type":"message","role":"user","content":[{"text":"do it"}]}}"#;
        let tok = r#"{"type":"token_usage_record","timestamp":"2026-10-24T18:50:04Z","payload":{"usage":{"input_tokens":100,"cached_input_tokens":40,"output_tokens":9},"thread_token_usage":{"total_tokens":999999}}}"#;
        let s = read_codex(&tmp("x1", &[meta, ctx, dev, usr, tok, tok].join("\n")), 0).unwrap();
        assert_eq!(s.id, "abcdef12");
        assert_eq!(s.prompts, 1);
        assert_eq!(s.tokens, Tokens { input: 120, output: 18, cache_read: 80, cache_write: 0 });
        assert_eq!(s.models["gpt-x"].total(), 218);
        assert_eq!(s.tool_calls, 0);
    }

    /// `cargo test -p backspace-core --lib real_logs -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_logs() {
        let home = PathBuf::from(std::env::var("USERPROFILE").or(std::env::var("HOME")).unwrap());
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
        let s = summarize(&home, now, 7, &std::env::var("EXCLUDE").unwrap_or_default());
        println!("sessions={} calls={} errors={} prompts={} tokens={} p95={:?}", s.sessions.len(), s.tool_calls, s.errors, s.prompts, s.tokens.total(), s.p95_latency_s);
    }
}
