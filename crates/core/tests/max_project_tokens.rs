//! M1 hardening, third review, item 3: `[orchestrator] max_project_tokens`,
//! the project-wide token limit added in the second round, checked as a
//! supported setting: it is parsed and written back, an old config without it
//! loads unchanged, it is the scope API reservations and CLI admissions share,
//! and the project's recorded spend counts against it.
//!
//! These verify a setting that already existed. The unit tests use the
//! reservation table; the end-to-end tests run a real harness against a mock
//! API. The CLI side of the same scope (three fake workers) is
//! `cli_concurrency.rs`. No real provider or CLI is called.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Barrier, Mutex};
use std::time::{Duration, Instant};

use backspace_core::config::{Config, ModelSpec};
use backspace_core::gateway::{input_bound, BudgetLine, Gateway, PolicyConfig, Refusal, Reservations, Role, ScopedLine};
use backspace_core::{Harness, LogKind, ProjectState};
use serde_json::{json, Value};

// ------------------------------------------------------------------ the setting

const BASE: &str = r#"
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10
{extra}

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:1/v1"
api_key_env = "NONE"

[[models]]
id = "mock-small"
provider = "mock"
tier = 1
description = "cheap"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000
"#;

fn parse(extra: &str) -> Config {
    Config::parse(&BASE.replace("{extra}", extra)).unwrap_or_else(|e| panic!("{e:#}"))
}

#[test]
fn the_setting_is_read_from_the_orchestrator_table() {
    assert_eq!(parse("max_project_tokens = 123456").orchestrator.max_project_tokens, Some(123_456));
}

#[test]
fn a_config_without_it_loads_as_before_with_no_project_token_limit() {
    let cfg = parse("");
    assert_eq!(cfg.orchestrator.max_project_tokens, None);
    assert_eq!(cfg.orchestrator.max_project_usd, None);
}

#[test]
fn the_setting_survives_being_written_back() {
    let cfg = parse("max_project_tokens = 777\nmax_project_usd = 3.5");
    let text = toml::to_string(&cfg).unwrap();
    let again = Config::parse(&text).unwrap();
    assert_eq!(again.orchestrator.max_project_tokens, Some(777));
    assert_eq!(again.orchestrator.max_project_usd, Some(3.5));
}

#[test]
fn the_shipped_default_config_names_it_and_loads_without_it() {
    let default = include_str!("../default-config.toml");
    assert!(default.contains("max_project_tokens"), "the example config documents the setting");
    assert_eq!(Config::parse(default).unwrap().orchestrator.max_project_tokens, None, "and leaves it unset, so no project gets a limit it did not ask for");
}

#[test]
fn a_negative_or_fractional_limit_is_not_a_limit() {
    let bad = |v: &str| Config::parse(&BASE.replace("{extra}", &format!("max_project_tokens = {v}")));
    assert!(bad("-5").is_err(), "a token count is a whole, non-negative number");
    assert!(bad("1.5").is_err());
}

// ------------------------------------------------- the shared scope, in the table

fn model(max_out: u32) -> ModelSpec {
    ModelSpec {
        id: "m".into(),
        provider: "p".into(),
        model: None,
        tier: 1,
        description: String::new(),
        input_usd_per_mtok: 1.0,
        output_usd_per_mtok: 2.0,
        max_output_tokens: max_out,
        efforts: vec![],
        class: None,
    }
}

fn cli() -> ModelSpec {
    ModelSpec { id: "cli-model".into(), provider: "the-cli".into(), input_usd_per_mtok: 0.0, output_usd_per_mtok: 0.0, max_output_tokens: 0, ..model(0) }
}

fn project(limit: u64, spent: u64) -> Vec<ScopedLine> {
    vec![ScopedLine {
        key: "project".into(),
        line: BudgetLine { label: "project".into(), limit_usd: None, limit_tokens: Some(limit), spent_usd: 0.0, spent_tokens: spent },
    }]
}

fn gw() -> Gateway {
    Gateway::new(PolicyConfig::default())
}

#[test]
fn an_api_call_is_refused_and_not_queued_while_a_cli_turn_holds_the_project_tokens() {
    let t = Arc::new(Reservations::default());
    let turn = t.admit_cli(&cli(), || project(100_000, 0)).unwrap();
    assert_eq!(turn.token_ceiling(), Some(100_000));
    let refused = t.reserve(&gw(), Role::Execution, &model(1000), 5_000, 1.0, || project(100_000, 0)).err().expect("no tokens are left to reserve");
    assert!(matches!(refused, Refusal::Budget { .. }), "{refused}");
    assert!(refused.to_string().contains("budget"), "{refused}");
    // Not queued: the call came straight back. Once the turn ends the same call is admitted.
    drop(turn);
    t.reserve(&gw(), Role::Execution, &model(1000), 5_000, 1.0, || project(100_000, 0)).expect("the tokens came back");
}

#[test]
fn a_cli_turn_is_given_what_the_api_calls_in_flight_have_not_reserved() {
    let t = Arc::new(Reservations::default());
    let api = t.reserve(&gw(), Role::Execution, &model(1000), 9_000, 1.0, || project(100_000, 0)).unwrap();
    assert_eq!(api.reserved_tokens(), 10_000, "9,000 in and the 1,000 it may write");
    let turn = t.admit_cli(&cli(), || project(100_000, 0)).unwrap();
    assert_eq!(turn.token_ceiling(), Some(90_000), "the project limit less the API call's hold");
    assert_eq!(t.held("project").1, 100_000, "together they hold exactly the limit");
}

#[test]
fn what_the_project_has_already_spent_comes_off_every_new_hold() {
    let t = Arc::new(Reservations::default());
    assert_eq!(t.admit_cli(&cli(), || project(100_000, 40_000)).unwrap().token_ceiling(), Some(60_000));
    let t = Arc::new(Reservations::default());
    let r = t.reserve(&gw(), Role::Execution, &model(1000), 5_000, 1.0, || project(100_000, 96_000)).err();
    assert!(r.is_some(), "4,000 are left and the call could take 6,000");
}

#[test]
fn api_calls_and_cli_turns_started_together_never_hold_more_than_the_project_limit() {
    const LIMIT: u64 = 100_000;
    for round in 0..20 {
        let t = Arc::new(Reservations::default());
        let gate = Arc::new(Barrier::new(16));
        let handles: Vec<_> = (0..16)
            .map(|i| {
                let (t, gate) = (t.clone(), gate.clone());
                std::thread::spawn(move || {
                    gate.wait();
                    // Even threads are API calls (11,000 each at worst), odd ones are CLI turns.
                    let held = if i % 2 == 0 {
                        t.reserve(&gw(), Role::Execution, &model(1000), 10_000, 1.0, || project(LIMIT, 0)).ok().map(|r| (r.reserved_tokens(), Box::new(r) as Box<dyn std::any::Any + Send>))
                    } else {
                        t.admit_cli(&cli(), || project(LIMIT, 0)).ok().map(|r| (r.token_ceiling().unwrap(), Box::new(r) as Box<dyn std::any::Any + Send>))
                    };
                    // Everything granted is still in flight when the others ask.
                    std::thread::sleep(Duration::from_millis(60));
                    held
                })
            })
            .collect();
        let granted: Vec<u64> = handles.into_iter().filter_map(|h| h.join().unwrap().map(|(n, _)| n)).collect();
        assert!(!granted.is_empty(), "round {round}: someone must have been let in");
        assert!(granted.iter().sum::<u64>() <= LIMIT, "round {round}: {granted:?} were granted against a limit of {LIMIT}");
    }
}

// ------------------------------------------------------- through a real harness

type Seen = Arc<Mutex<Vec<Value>>>;

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

fn tool_call(name: &str, args: Value, prompt_tokens: u64) -> Value {
    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
        {"id": "call_1", "type": "function", "function": {"name": name, "arguments": args.to_string()}}]}}],
        "usage": {"prompt_tokens": prompt_tokens, "completion_tokens": 100}})
}

fn text(t: &str) -> Value {
    json!({"choices": [{"finish_reason": "stop", "message": {"content": t}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
}

fn open(tag: &str, port: u16, router: &str, orchestrator: &str, rest: &str) -> Harness {
    let ws: PathBuf = std::env::temp_dir().join(format!("bs-mpt-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(
        ws.join("backspace.toml"),
        format!(
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

{rest}
"#
        ),
    )
    .unwrap();
    Harness::open(ws).unwrap()
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

#[test]
fn a_call_that_cannot_fit_the_project_token_limit_is_refused_before_it_is_sent() {
    let (port, seen) = mock(|_, _| text("hi"));
    let h = open("fit-learn", port, "pin_main = \"mock-small\"", "", "");
    h.send("say hi");
    wait(&h, "the first request", |_| count(&seen) >= 1);
    let req = seen.lock().unwrap()[0].clone();
    let bytes = serde_json::to_vec(&req).unwrap().len();
    let bound = input_bound(bytes, req["messages"].as_array().map_or(0, |a| a.len()), req["tools"].as_array().map_or(0, |a| a.len()));
    drop(h);

    // A project limit below the input bound alone: nothing can be sent.
    let (port, seen) = mock(|_, _| text("the provider should not have been called"));
    let h = open("fit-tight", port, "pin_main = \"mock-small\"", &format!("max_project_tokens = {}", bound - 1), "");
    h.send("say hi");
    let s = wait(&h, "the refusal", |s| refused(s, 0));
    assert_eq!(count(&seen), 0, "the request was never sent");
    assert!(errors(&s, 0).join("\n").contains("project"), "and the refusal names the project's budget: {:?}", errors(&s, 0));
    drop(h);

    // Room for the worst case: it goes through, so the refusal above was the limit.
    let (port, seen) = mock(|_, _| text("hi"));
    let h = open("fit-roomy", port, "pin_main = \"mock-small\"", &format!("max_project_tokens = {}", bound + 1000), "");
    h.send("say hi");
    wait(&h, "the first request", |_| count(&seen) >= 1);
    assert!(errors(&h.snapshot(), 0).is_empty());
}

#[test]
fn the_tokens_one_call_used_are_counted_against_the_next_one() {
    // The first reply reports 999,000 prompt tokens against a 1,000,000 project limit: it fits when
    // asked (worst case far below), and leaves less than any call's input allowance afterwards.
    let (port, seen) = mock(|n, _| match n {
        0 => tool_call("skill", json!({"name": "tdd"}), 999_000),
        _ => text("the provider should not have been called twice"),
    });
    let h = open("counted", port, "pin_main = \"mock-small\"", "max_project_tokens = 1000000", "[policy]\nskills = [\"tdd\"]\n");
    h.send("say hi");
    let s = wait(&h, "the refusal", |s| refused(s, 0));
    assert_eq!(count(&seen), 1, "{:?}", errors(&s, 0));
    assert!(s.agents[0].input_tokens >= 999_000, "the provider's figure is the agent's count: {}", s.agents[0].input_tokens);
}

#[test]
fn agents_started_together_cannot_jointly_pass_the_project_token_limit() {
    let triage = |_: usize, _: &Value| tool_call("triage_ticket", json!({"state": "wontfix", "category": "bug", "notes": "n"}), 10);
    let (port, seen) = mock(triage);
    let h = open("conc-learn", port, "pin_sub = \"mock-small\"", "", "");
    h.file_ticket("Crash on empty input", "The parser panics on an empty file").unwrap();
    wait(&h, "the first request", |_| count(&seen) >= 1);
    let req = seen.lock().unwrap()[0].clone();
    let bytes = serde_json::to_vec(&req).unwrap().len();
    let worst = input_bound(bytes, req["messages"].as_array().map_or(0, |a| a.len()), req["tools"].as_array().map_or(0, |a| a.len())) + 1000;
    drop(h);

    // A limit for two and a half such calls; four agents at once, each call taking a second and a half.
    let (port, seen) = mock(move |n, req| {
        std::thread::sleep(Duration::from_millis(1500));
        triage(n, req)
    });
    let limit = worst * 5 / 2;
    let h = open("conc-race", port, "pin_sub = \"mock-small\"", &format!("max_project_tokens = {limit}"), "");
    for i in 0..4 {
        h.file_ticket(&format!("Crash {i}"), "The parser panics on an empty file").unwrap();
    }
    wait(&h, "one request", |_| count(&seen) >= 1);
    std::thread::sleep(Duration::from_millis(3500));
    let made = count(&seen);
    assert!(made <= 2, "{made} calls were made against a token limit that covers two at their worst case ({worst} each, limit {limit})");
    let s = h.snapshot();
    let told: Vec<_> = s.agents.iter().flat_map(|a| errors(&s, a.id)).collect();
    assert!(told.iter().any(|e| e.contains("refused by project policy") && e.contains("project")), "the agents that did not fit were told why: {told:?}");
}
