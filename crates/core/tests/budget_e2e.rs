//! M1 hardening, F2, through the real harness: the bound comes from the
//! request that is sent, concurrent agents share one budget, a call that costs
//! more than it reserved is recorded, token budgets count the provider's own
//! numbers, and the advisor is only asked for when its worst case fits too.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use backspace_core::gateway::input_bound;
use backspace_core::{AgentKind, Harness, LogKind, ProjectState};
use serde_json::{json, Value};

type Seen = Arc<Mutex<Vec<Value>>>;

/// A model API on localhost that answers whatever shape `script` builds and
/// records every request it receives.
fn mock(script: impl Fn(usize, &Value) -> Value + Send + Sync + 'static) -> (u16, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen: Seen = Arc::default();
    let (seen2, script) = (seen.clone(), Arc::new(script));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let (seen, script) = (seen2.clone(), script.clone());
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                if len == 0 {
                    return;
                }
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let req: Value = serde_json::from_slice(&body).unwrap();
                let n = {
                    let mut g = seen.lock().unwrap();
                    g.push(req.clone());
                    g.len() - 1
                };
                let out = script(n, &req).to_string();
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    (port, seen)
}

fn text(t: &str) -> Value {
    json!({"choices": [{"finish_reason": "stop", "message": {"content": t}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
}

fn tool_call(name: &str, args: Value, prompt_tokens: u64) -> Value {
    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
        {"id": "call_1", "type": "function", "function": {"name": name, "arguments": args.to_string()}}]}}],
        "usage": {"prompt_tokens": prompt_tokens, "completion_tokens": 100}})
}

fn a_text(t: &str, usage: Value) -> Value {
    json!({"content": [{"type": "text", "text": t}], "stop_reason": "end_turn", "usage": usage})
}

fn a_tool(name: &str, args: Value, usage: Value) -> Value {
    json!({"content": [{"type": "tool_use", "id": "tu_1", "name": name, "input": args}], "stop_reason": "tool_use", "usage": usage})
}

/// A project against the mock. `kind` is the wire format of provider `mock`.
fn open(tag: &str, port: u16, kind: &str, router: &str, orchestrator: &str, rest: &str) -> (Harness, PathBuf) {
    let ws = std::env::temp_dir().join(format!("bs-bud-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    let base = if kind == "anthropic" { format!("http://127.0.0.1:{port}") } else { format!("http://127.0.0.1:{port}/v1") };
    let cfg = format!(
        r#"
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"
{router}

[orchestrator]
max_parallel_calls = 4
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10
{orchestrator}

[providers.mock]
kind = "{kind}"
base_url = "{base}"
api_key_env = "NONE"

[[models]]
id = "mock-small"
provider = "mock"
tier = 1
description = "cheap"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000

[[models]]
id = "mock-dear-input"
provider = "mock"
tier = 2
description = "input costs $1000 per million, output nothing"
input_usd_per_mtok = 1000.0
output_usd_per_mtok = 0.0
max_output_tokens = 1000

[[models]]
id = "mock-adv"
provider = "mock"
tier = 4
description = "advisor"
input_usd_per_mtok = 15.0
output_usd_per_mtok = 75.0
max_output_tokens = 8000

{rest}
"#
    );
    std::fs::write(ws.join("backspace.toml"), cfg).unwrap();
    (Harness::open(ws.clone()).unwrap(), ws)
}

fn wait(h: &Harness, what: &str, ok: impl Fn(&ProjectState) -> bool) -> ProjectState {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let s = h.snapshot();
        if ok(&s) {
            return s;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:#?}",
            s.agents.iter().map(|a| (&a.key, a.status, &a.log)).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn errors(s: &ProjectState, agent: usize) -> Vec<String> {
    s.agents[agent].log.iter().filter(|e| e.kind == LogKind::Error).map(|e| e.text.clone()).collect()
}

fn refused(s: &ProjectState, agent: usize) -> bool {
    errors(s, agent).iter().any(|e| e.contains("refused by project policy"))
}

fn count(seen: &Seen) -> usize {
    seen.lock().unwrap().len()
}

/// What the harness measures of a request: the bytes of the body it sent.
fn bound_of(req: &Value, system_is_extra: bool) -> u64 {
    let bytes = serde_json::to_vec(req).unwrap().len();
    let messages = req["messages"].as_array().map_or(0, |a| a.len()) + usize::from(system_is_extra);
    let tools = req["tools"].as_array().map_or(0, |a| a.len());
    input_bound(bytes, messages, tools)
}

fn system_log(s: &ProjectState, agent: usize) -> String {
    s.agents[agent].log.iter().filter(|e| e.kind == LogKind::System).map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

// ------------------------------------------------------------------- bound

/// Run `message` once with no budget to learn the size of the first request,
/// then again with a token budget of exactly that many bytes.
fn refused_when_the_budget_is_the_body_size(tag: &str, message: &str) {
    let (port, seen) = mock(|_, _| text("hi"));
    let (h, _) = open(&format!("{tag}-learn"), port, "openai", "pin_main = \"mock-small\"", "", "");
    h.send(message);
    wait(&h, "the first request", |_| count(&seen) >= 1);
    let body = serde_json::to_vec(&seen.lock().unwrap()[0]).unwrap().len();
    drop(h);

    let policy = |tokens: usize| format!("[policy.budget]\nmax_tokens = {tokens}\n");
    let (port, seen) = mock(|_, _| text("the provider should not have been called"));
    let (h, _) = open(&format!("{tag}-tight"), port, "openai", "pin_main = \"mock-small\"", "", &policy(body));
    h.send(message);
    let s = wait(&h, "the refusal", |s| refused(s, 0));
    assert!(errors(&s, 0).join("\n").contains("budget"), "{:?}", errors(&s, 0));
    assert_eq!(count(&seen), 0, "a {body}-token budget cannot cover a {body}-byte request, and the old estimate (about {}) said it could", body / 3);
    drop(h);

    // The same message with room to spare goes through, so the refusal above is about the budget.
    let (port, seen) = mock(|_, _| text("hi"));
    let (h, _) = open(&format!("{tag}-roomy"), port, "openai", "pin_main = \"mock-small\"", "", &policy(body * 3));
    h.send(message);
    wait(&h, "the first request", |_| count(&seen) >= 1);
    assert!(errors(&h.snapshot(), 0).is_empty());
}

#[test]
fn the_input_bound_is_read_from_the_request_that_was_sent() {
    refused_when_the_budget_is_the_body_size("ascii", "say hi");
}

#[test]
fn a_unicode_prompt_is_bounded_by_its_bytes() {
    refused_when_the_budget_is_the_body_size("unicode", &"漢字仮名交じり文🦀".repeat(400));
}

#[test]
fn source_code_in_the_prompt_is_bounded_by_its_bytes() {
    refused_when_the_budget_is_the_body_size("code", &"fn f(x: &[u8]) -> u32 { x.iter().map(|b| (*b as u32) ^ 0x5a).sum() }\n".repeat(300));
}

#[test]
fn the_anthropic_prompt_is_reserved_at_the_cache_write_price() {
    let anthropic_usage = json!({"input_tokens": 10, "output_tokens": 5});
    let u = anthropic_usage.clone();
    let (port, seen) = mock(move |_, _| a_text("hi", u.clone()));
    let (h, _) = open("price-learn", port, "anthropic", "pin_main = \"mock-dear-input\"", "", "");
    h.send("say hi");
    wait(&h, "the first request", |_| count(&seen) >= 1);
    // The Anthropic body keeps the system prompt outside `messages`.
    let bound = bound_of(&seen.lock().unwrap()[0], true) as f64;
    drop(h);

    let (port, seen) = mock(move |_, _| a_text("the provider should not have been called", anthropic_usage.clone()));
    let (h, _) = open("price-tight", port, "anthropic", "pin_main = \"mock-dear-input\"", "", "[policy.budget]\nmax_usd = 0.0001\n");
    h.send("say hi");
    let s = wait(&h, "the refusal", |s| refused(s, 0));
    assert_eq!(count(&seen), 0);
    let e = errors(&s, 0).join("\n");
    let quoted: f64 = e.split("about $").nth(1).and_then(|r| r.split(')').next()).and_then(|n| n.parse().ok()).unwrap_or_else(|| panic!("no prompt price in: {e}"));
    let listed = bound * 1000.0 / 1e6;
    assert!((quoted / listed - 1.25).abs() < 0.01, "the prompt was priced at {quoted}, listed price would be {listed}: {e}");
}

// ------------------------------------------------------------- concurrency

#[test]
fn concurrent_agents_cannot_jointly_pass_the_project_budget() {
    // Learn what one triage call can cost at most on mock-small.
    let triage = |_: usize, _: &Value| tool_call("triage_ticket", json!({"state": "wontfix", "category": "bug", "notes": "n"}), 10);
    let (port, seen) = mock(triage);
    let (h, _) = open("conc-learn", port, "openai", "pin_sub = \"mock-small\"", "", "");
    h.file_ticket("Crash on empty input", "The parser panics on an empty file").unwrap();
    wait(&h, "the first request", |_| count(&seen) >= 1);
    let first = seen.lock().unwrap()[0].clone();
    drop(h);
    let worst = bound_of(&first, false) as f64 * 1.0 / 1e6 + 1000.0 * 2.0 / 1e6;

    // A budget for two and a half such calls, four agents at once, each call taking a second and a half.
    let (port, seen) = mock(move |n, req| {
        std::thread::sleep(Duration::from_millis(1500));
        triage(n, req)
    });
    let (h, _) = open("conc-race", port, "openai", "pin_sub = \"mock-small\"", &format!("max_project_usd = {}", worst * 2.5), "");
    for i in 0..4 {
        h.file_ticket(&format!("Crash {i}"), "The parser panics on an empty file").unwrap();
    }
    wait(&h, "one request", |_| count(&seen) >= 1);
    std::thread::sleep(Duration::from_millis(3500));
    let made = count(&seen);
    assert!(made <= 2, "{made} calls were made against a budget that covers two at their worst case ({worst:.6} each)");
    let s = h.snapshot();
    assert!(
        s.agents.iter().any(|a| a.kind == AgentKind::Triage && errors(&s, a.id).iter().any(|e| e.contains("refused by project policy") && e.contains("project"))),
        "the agents that did not fit were told why"
    );
    assert!(s.total_cost_usd < worst, "what was actually spent is recorded, not what was reserved: {}", s.total_cost_usd);
}

// --------------------------------------------------------------- overshoot

#[test]
fn a_call_that_costs_more_than_it_reserved_is_logged_and_stops_the_next_one() {
    // The reply claims ten million prompt tokens: $10 on mock-small, against a $0.50 budget.
    let (port, seen) = mock(|n, _| match n {
        0 => tool_call("skill", json!({"name": "tdd"}), 10_000_000),
        _ => text("the provider should not have been called twice"),
    });
    let (h, _) = open("overshoot", port, "openai", "pin_main = \"mock-small\"", "", "[policy]\nskills = [\"tdd\"]\n[policy.budget]\nmax_usd = 0.5\n");
    h.send("say hi");
    let s = wait(&h, "the refusal", |s| refused(s, 0));
    assert_eq!(count(&seen), 1);
    let log = system_log(&s, 0);
    assert!(log.contains("overshoot") && log.contains("mock-small"), "the overspend is on the record: {log}");
    assert!(s.agents[0].cost_usd > 9.0, "and what was really spent is what counts: {}", s.agents[0].cost_usd);
}

// ----------------------------------------------------------- raw token count

#[test]
fn token_budgets_count_the_providers_tokens_not_the_billing_weighted_ones() {
    // 5,000,000 cache-read tokens bill at a tenth (500,000 equivalent), but they are 5,000,000 tokens.
    let usage = json!({"input_tokens": 10, "cache_read_input_tokens": 5_000_000, "output_tokens": 5});
    let (port, seen) = mock(move |n, _| match n {
        0 => a_tool("skill", json!({"name": "tdd"}), usage.clone()),
        _ => a_text("the provider should not have been called twice", usage.clone()),
    });
    let (h, _) = open(
        "raw-tokens",
        port,
        "anthropic",
        "pin_main = \"mock-small\"",
        "",
        "[policy]\nskills = [\"tdd\"]\n[policy.budget]\nmax_tokens = 1000000\n",
    );
    h.send("say hi");
    let s = wait(&h, "the refusal", |s| refused(s, 0));
    assert_eq!(count(&seen), 1, "{:?}", errors(&s, 0));
    assert!(errors(&s, 0).join("\n").contains("budget"));
    assert!(s.agents[0].input_tokens >= 5_000_000, "the agent's token count is the provider's: {}", s.agents[0].input_tokens);
}

// ------------------------------------------------------------------ advisor

fn has_advisor(req: &Value) -> bool {
    req["tools"].as_array().is_some_and(|t| t.iter().any(|t| t["type"] == "advisor_20260301"))
}

#[test]
fn the_advisor_is_left_off_when_its_reserve_does_not_fit() {
    let usage = json!({"input_tokens": 10, "output_tokens": 5});
    let advisor = "[advisor]\nmodel = \"mock-adv\"\n";

    // Room for the advisor's worst case as well: it is offered.
    let u = usage.clone();
    let (port, seen) = mock(move |_, _| a_text("hi", u.clone()));
    let (h, _) = open("adv-roomy", port, "anthropic", "pin_main = \"mock-small\"", "max_project_usd = 50.0", advisor);
    h.send("say hi");
    wait(&h, "the first request", |_| count(&seen) >= 1);
    assert!(has_advisor(&seen.lock().unwrap()[0]), "control: with $50 the advisor is attached");
    drop(h);

    // Room for the executor's call but not for an advisor that may read the transcript and write 4,096 tokens.
    let (port, seen) = mock(move |_, _| a_text("hi", usage.clone()));
    let (h, _) = open("adv-tight", port, "anthropic", "pin_main = \"mock-small\"", "max_project_usd = 0.35", advisor);
    h.send("say hi");
    wait(&h, "the first request", |_| count(&seen) >= 1);
    let req = seen.lock().unwrap()[0].clone();
    assert!(!has_advisor(&req), "the advisor's spend cannot be bounded under $0.35, so it is not offered: {}", req["tools"]);
    let s = h.snapshot();
    assert!(errors(&s, 0).is_empty(), "the call itself still goes ahead: {:?}", errors(&s, 0));
    assert!(system_log(&s, 0).contains("advisor"), "and the log says the advisor was left off: {}", system_log(&s, 0));
}
