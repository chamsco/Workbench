//! M1 hardening, second review, item 2: a Jev routing call is checked against
//! the project's budget before it is sent. Its worst case (the request as it
//! goes, bounded in bytes, plus an assumed answer) is held while it is in
//! flight; if it does not fit, Jev is not called and the heuristic decides.
//! A router with no budget around it (a script, the eval) still says so.
//!
//! Mock Jev server on a local port; nothing here leaves the machine.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use backspace_core::config::Config;
use backspace_core::gateway::{BudgetLine, Refusal, Reservations, ScopedLine, ROUTER_OUTPUT_TOKENS};
use backspace_core::router::{AgentRole, Router};
use backspace_core::{Harness, LogKind};
use serde_json::json;

static ENV: Once = Once::new();

fn env() {
    ENV.call_once(|| {
        let home = std::env::temp_dir().join(format!("bs-routerbud-home-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("BACKSPACE_DATA", home.join("data"));
        std::env::remove_var("BACKSPACE_CONFIG");
        std::env::set_var("BS_JEV_TEST_KEY", "test-key");
    });
}

/// Answers `/route` like Jev does and counts how often, keeping the size of each body.
struct Jev {
    port: u16,
    calls: Arc<AtomicUsize>,
    bodies: Arc<Mutex<Vec<usize>>>,
}

fn jev() -> Jev {
    env();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let (calls, bodies) = (Arc::new(AtomicUsize::new(0)), Arc::new(Mutex::new(vec![])));
    let (c2, b2) = (calls.clone(), bodies.clone());
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let mut stream = stream.unwrap();
            let (calls, bodies) = (c2.clone(), b2.clone());
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
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
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let out = if first.contains("/route") {
                    calls.fetch_add(1, Ordering::SeqCst);
                    bodies.lock().unwrap().push(len);
                    json!({"answers": {"model": {"choice": "mock-small", "confidence": 0.9}, "effort": {"score": 1.0}},
                        "usage": {"input_tokens": 100, "output_tokens": 20}})
                } else {
                    // The model API: not what these tests are about.
                    json!({"error": "not here"})
                }
                .to_string();
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    Jev { port, calls, bodies }
}

fn config(port: u16, orchestrator_extra: &str) -> String {
    format!(
        r#"
[router]
backend = "jev"
jev_endpoint = "http://127.0.0.1:{port}/route"
jev_model = "jev-test"
jev_api_key_env = "BS_JEV_TEST_KEY"
jev_usd_per_mtok = 2000.0
main_min_effort = "high"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10
{orchestrator_extra}

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
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
id = "mock-big"
provider = "mock"
tier = 4
description = "strong"
input_usd_per_mtok = 10.0
output_usd_per_mtok = 20.0
max_output_tokens = 1000
"#
    )
}

fn project_scope(limit_usd: f64) -> impl Fn() -> Vec<ScopedLine> + Send + Sync + 'static {
    move || {
        vec![ScopedLine {
            key: "project".into(),
            line: BudgetLine { label: "project".into(), limit_usd: Some(limit_usd), limit_tokens: None, spent_usd: 0.0, spent_tokens: 0 },
        }]
    }
}

fn route(r: &Router) -> backspace_core::router::Decision {
    tokio::runtime::Runtime::new().unwrap().block_on(r.route(AgentRole::Main, "fix a typo in the readme", "")).unwrap()
}

// ------------------------------------------------------------------ the router

#[test]
fn a_refused_reservation_means_jev_is_never_called() {
    let j = jev();
    let cfg = Config::parse(&config(j.port, "")).unwrap();
    let r = Router::new(reqwest::Client::new(), cfg).with_meter(|_usd, _tokens| {
        Err(Refusal::Budget { label: "project".into(), detail: "nothing left".into() })
    });
    let d = route(&r);
    assert_eq!(j.calls.load(Ordering::SeqCst), 0, "no request went out");
    assert!(d.source.starts_with("heuristic"), "the heuristic decided: {}", d.source);
    let note = d.note.clone().unwrap_or_default();
    assert!(note.contains("refused by project policy") && note.contains("nothing left"), "{note}");
    assert_eq!(d.router_cost_usd, 0.0);
}

#[test]
fn the_reservation_covers_the_request_that_is_sent_and_is_released_after() {
    let j = jev();
    let cfg = Config::parse(&config(j.port, "")).unwrap();
    let table = Arc::new(Reservations::default());
    let asked = Arc::new(Mutex::new(vec![]));
    let (t2, a2) = (table.clone(), asked.clone());
    let r = Router::new(reqwest::Client::new(), cfg).with_meter(move |usd, tokens| {
        a2.lock().unwrap().push((usd, tokens));
        t2.reserve_fixed("the router", usd, tokens, project_scope(1000.0))
    });
    let d = route(&r);
    assert_eq!(d.source, "jev", "{d:?}");
    assert_eq!(j.calls.load(Ordering::SeqCst), 1);
    let sent = j.bodies.lock().unwrap()[0] as u64;
    let (usd, tokens) = asked.lock().unwrap()[0];
    assert!(tokens >= sent + ROUTER_OUTPUT_TOKENS, "held {tokens} tokens for a {sent}-byte request and a {ROUTER_OUTPUT_TOKENS}-token answer");
    assert!((usd - tokens as f64 * 2000.0 / 1e6).abs() < 1e-9, "priced at the router's own rate: ${usd} for {tokens}");
    assert!(d.router_cost_usd > 0.0 && d.router_cost_usd < usd, "the real cost ({}) is below the bound ({usd})", d.router_cost_usd);
    assert_eq!(table.held("project"), (0.0, 0), "released once the call is over");
}

#[test]
fn only_a_router_without_a_budget_is_unmetered() {
    let j = jev();
    let cfg = Config::parse(&config(j.port, "")).unwrap();
    let before = backspace_core::provider::unmetered_calls().get("Jev router").copied().unwrap_or(0);
    route(&Router::new(reqwest::Client::new(), cfg.clone()));
    let mid = backspace_core::provider::unmetered_calls().get("Jev router").copied().unwrap_or(0);
    assert_eq!(mid, before + 1, "a router with no budget declares the call unmetered");
    let table = Arc::new(Reservations::default());
    let r = Router::new(reqwest::Client::new(), cfg).with_meter(move |usd, tokens| table.reserve_fixed("the router", usd, tokens, project_scope(1000.0)));
    route(&r);
    let after = backspace_core::provider::unmetered_calls().get("Jev router").copied().unwrap_or(0);
    assert_eq!(after, mid, "a router with a budget does not");
}

// ----------------------------------------------------------------- the harness

fn open(tag: &str, port: u16, orchestrator_extra: &str) -> Harness {
    let ws = std::env::temp_dir().join(format!("bs-routerbud-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("backspace.toml"), config(port, orchestrator_extra)).unwrap();
    Harness::open(ws).unwrap()
}

fn routed(h: &Harness) -> String {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let s = h.snapshot();
        let text = s.agents[0].log.iter().filter(|e| e.kind == LogKind::System && e.text.starts_with("routed by")).map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
        if !text.is_empty() {
            return text;
        }
        assert!(Instant::now() < deadline, "the project never routed: {:?}", s.agents[0].log.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_project_too_close_to_its_limit_does_not_call_jev() {
    let j = jev();
    // About $3.40 of worst case at $2,000 a million tokens, against a $0.50 limit.
    let h = open("tight", j.port, "max_project_usd = 0.5");
    h.send("fix a typo in the readme");
    let text = routed(&h);
    assert_eq!(j.calls.load(Ordering::SeqCst), 0, "{text}");
    assert!(text.contains("jev unavailable, heuristic used") && text.contains("refused by project policy") && text.contains("project"), "{text}");
    assert_eq!(h.snapshot().router_cost_usd, 0.0);
}

#[test]
fn a_project_with_room_calls_jev_and_its_cost_counts_toward_the_limit() {
    let j = jev();
    let h = open("roomy", j.port, "max_project_usd = 50.0");
    h.send("fix a typo in the readme");
    let text = routed(&h);
    assert_eq!(j.calls.load(Ordering::SeqCst), 1, "{text}");
    assert!(text.contains("routed by jev"), "{text}");
    let s = h.snapshot();
    assert!(s.router_cost_usd > 0.2 && s.total_cost_usd >= s.router_cost_usd, "120 tokens at $2,000 a million: {} / {}", s.router_cost_usd, s.total_cost_usd);
}
