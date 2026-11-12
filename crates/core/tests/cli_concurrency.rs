//! M1 hardening, second review, item 1: coding-CLI turns share a token budget
//! without sharing the same allowance. A turn that is let start holds what it
//! may use until it ends, so a second turn on the same budget waits for it
//! (it is not given the same tokens twice), and dollars never pass a CLI.
//!
//! The unit tests use the gateway's reservation table directly; the end-to-end
//! tests run fake `claude` scripts under a real harness. Nothing here is a
//! real CLI.
#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use backspace_core::config::ModelSpec;
use backspace_core::gateway::{BudgetLine, Refusal, Reservations, ScopedLine};
use backspace_core::{AgentKind, AgentStatus, Harness, LogKind};
use serde_json::{json, Value};

// ------------------------------------------------------------------ the table

fn cli_model(input: f64, output: f64) -> ModelSpec {
    ModelSpec {
        id: "cli-model".into(),
        provider: "the-cli".into(),
        model: None,
        tier: 3,
        description: String::new(),
        input_usd_per_mtok: input,
        output_usd_per_mtok: output,
        max_output_tokens: 0,
        efforts: vec![],
        class: None,
    }
}

fn scope(key: &str, usd: Option<f64>, tokens: Option<u64>, spent_tokens: u64) -> ScopedLine {
    ScopedLine {
        key: key.into(),
        line: BudgetLine { label: key.into(), limit_usd: usd, limit_tokens: tokens, spent_usd: 0.0, spent_tokens },
    }
}

#[test]
fn two_turns_on_one_budget_are_not_both_given_the_remaining_tokens() {
    let t = Arc::new(Reservations::default());
    let m = cli_model(0.0, 0.0);
    let lines = || vec![scope("project", None, Some(100_000), 0)];
    let first = t.admit_cli(&m, lines).expect("100,000 are free");
    assert_eq!(first.token_ceiling(), Some(100_000));
    assert_eq!(t.held("project").1, 100_000, "what the turn may use is held while it runs");
    let second = t.admit_cli(&m, lines);
    match second {
        Err(Refusal::Budget { detail, .. }) => assert!(detail.contains("held"), "it says the tokens are held by a running turn: {detail}"),
        other => panic!("the same 100,000 were handed out twice: {:?}", other.map(|r| r.token_ceiling())),
    }
}

#[test]
fn the_allowance_comes_back_when_the_turn_ends_less_what_it_used() {
    let t = Arc::new(Reservations::default());
    let m = cli_model(0.0, 0.0);
    let first = t.admit_cli(&m, || vec![scope("project", None, Some(100_000), 0)]).unwrap();
    drop(first);
    assert_eq!(t.held("project"), (0.0, 0), "nothing stays held");
    // The turn used 60,000 (the harness reports that as spent on the next look).
    let next = t.admit_cli(&m, || vec![scope("project", None, Some(100_000), 60_000)]).unwrap();
    assert_eq!(next.token_ceiling(), Some(40_000));
}

#[test]
fn the_hold_is_taken_in_every_scope_the_turn_counts_toward() {
    let t = Arc::new(Reservations::default());
    let m = cli_model(0.0, 0.0);
    // A task limit of its own and the project's: the turn gets the tighter, and holds it in both.
    let a = t.admit_cli(&m, || vec![scope("task:1", None, Some(50_000), 0), scope("project", None, Some(80_000), 0)]).unwrap();
    assert_eq!(a.token_ceiling(), Some(50_000), "the tighter of the two");
    assert_eq!(t.held("task:1").1, 50_000);
    assert_eq!(t.held("project").1, 50_000, "and it is held against the project as well");
    // Another task under the same project sees only what is left of the project.
    let b = t.admit_cli(&m, || vec![scope("task:2", None, Some(50_000), 0), scope("project", None, Some(80_000), 0)]).unwrap();
    assert_eq!(b.token_ceiling(), Some(30_000), "80,000 less the 50,000 the first turn holds");
    assert_eq!(t.held("task:1").1, 50_000, "the other task's own scope is not touched by it");
}

#[test]
fn parallel_admissions_on_one_budget_hand_out_it_at_most_once() {
    let t = Arc::new(Reservations::default());
    let m = Arc::new(cli_model(0.0, 0.0));
    let gate = Arc::new(std::sync::Barrier::new(12));
    let handles: Vec<_> = (0..12)
        .map(|_| {
            let (t, m, gate) = (t.clone(), m.clone(), gate.clone());
            std::thread::spawn(move || {
                gate.wait();
                t.admit_cli(&m, || vec![scope("project", None, Some(100_000), 0)]).map(|r| {
                    std::thread::sleep(Duration::from_millis(150));
                    r.token_ceiling()
                })
            })
        })
        .collect();
    let granted: Vec<u64> = handles.into_iter().filter_map(|h| h.join().unwrap().ok().flatten()).collect();
    assert!(granted.iter().sum::<u64>() <= 100_000, "{granted:?}");
    assert_eq!(granted.len(), 1, "with one shared allowance and no per-turn share, one turn at a time: {granted:?}");
}

#[test]
fn a_dollar_limit_never_binds_a_cli_turn_even_when_its_model_has_a_price() {
    let t = Arc::new(Reservations::default());
    // A configured API-equivalent price does not make the dollars real: a CLI turn adds none to
    // any scope, so a dollar limit alone would never be reached.
    let priced = cli_model(3.0, 15.0);
    let err = t.admit_cli(&priced, || vec![scope("project", Some(5.0), None, 0)]).unwrap_err();
    let text = err.to_string();
    assert!(text.contains("dollar") && text.contains("cli-model"), "{text}");
    // With a token limit on the same line it is allowed, and the tokens bind it.
    let ok = t.admit_cli(&priced, || vec![scope("project", Some(5.0), Some(10_000), 0)]).unwrap();
    assert_eq!(ok.token_ceiling(), Some(10_000));
}

// ------------------------------------------------------------- real harness

static BIN: Once = Once::new();

/// `claude`, slowly: it logs when it starts and ends, makes a file of its own, and reports
/// 300,000 input tokens once, at the end (as Claude Code does).
fn fake_claude() {
    BIN.call_once(|| {
        let bin = std::env::temp_dir().join(format!("bs-clicc-bin-{}", std::process::id()));
        std::fs::create_dir_all(&bin).unwrap();
        let p = bin.join("claude");
        std::fs::write(
            &p,
            r#"#!/bin/sh
cat > /dev/null
LOG="$PWD/../../../cli-runs.log"
echo "start $$ $(date +%s%N)" >> "$LOG"
sleep 3
echo made > "out-$$.txt"
echo "end $$ $(date +%s%N)" >> "$LOG"
echo '{"type":"system","subtype":"init","session_id":"fake-'$$'"}'
echo '{"type":"result","result":"made a file","session_id":"fake-'$$'","total_cost_usd":0.01,"is_error":false,"usage":{"input_tokens":300000,"output_tokens":10}}'
"#,
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("BACKSPACE_CLI_PATH", &bin);
    });
}

fn tool_call(name: &str, args: Value) -> Value {
    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
        {"id": "call_1", "type": "function", "function": {"name": name, "arguments": args.to_string()}}]}}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
}

/// The planner: `n` tickets, then hand them all to workers, then nothing more to say.
fn planner(listener: TcpListener, n_tickets: usize) {
    let n = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        let n = n.clone();
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
            let tickets: Vec<Value> = (0..n_tickets)
                .map(|i| {
                    json!({"key": format!("job{i}"), "title": format!("Job {i}"),
                        "what_to_build": "make a file", "acceptance": ["a file exists"], "check": "ls out-*.txt"})
                })
                .collect();
            let out = match n.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                0 => tool_call("create_tickets", json!({ "tickets": tickets })),
                1 => tool_call("work_tickets", json!({})),
                _ => json!({"choices": [{"finish_reason": "stop", "message": {"content": "done"}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}}),
            }
            .to_string();
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
        });
    }
}

fn project(tag: &str, n_tickets: usize, cli_prices: (f64, f64), orchestrator_extra: &str) -> (Harness, PathBuf) {
    fake_claude();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || planner(listener, n_tickets));
    let ws = std::env::temp_dir().join(format!("bs-clicc-{tag}-{}", std::process::id()));
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
pin_main = "mock-mid"
pin_sub = "cli-model"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = true
bash_timeout_secs = 10
{orchestrator_extra}

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
api_key_env = "NONE"

[providers.the-cli]
kind = "cli"
cli = "claude"

[[models]]
id = "mock-mid"
provider = "mock"
tier = 2
description = "mid"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000

[[models]]
id = "cli-model"
provider = "the-cli"
tier = 3
description = "a coding CLI"
input_usd_per_mtok = {}
output_usd_per_mtok = {}
max_output_tokens = 0
"#,
            cli_prices.0, cli_prices.1
        ),
    )
    .unwrap();
    (Harness::open(ws.clone()).unwrap(), ws)
}

fn log(ws: &Path) -> Vec<(String, u64, u64)> {
    std::fs::read_to_string(ws.join("cli-runs.log"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut p = l.split_whitespace();
            Some((p.next()?.to_string(), p.next()?.parse().ok()?, p.next()?.parse().ok()?))
        })
        .collect()
}

fn failed_workers(h: &Harness) -> Vec<(String, Vec<String>)> {
    h.snapshot()
        .agents
        .iter()
        .filter(|a| a.kind == AgentKind::Worker && a.status == AgentStatus::Failed)
        .map(|a| (a.key.clone(), a.log.iter().filter(|e| e.kind == LogKind::Error).map(|e| e.text.clone()).collect()))
        .collect()
}

fn wait(what: &str, within: Duration, h: &Harness, mut ok: impl FnMut(&Harness) -> bool) {
    let deadline = Instant::now() + within;
    loop {
        // Plans are approved as they come; deliveries merge on their own (auto_approve).
        for a in h.snapshot().pending_approvals() {
            h.approve(a.id);
        }
        if ok(h) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:#?}",
            h.snapshot().agents.iter().map(|a| (&a.key, a.status, a.log.iter().map(|e| e.text.clone()).collect::<Vec<_>>())).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn cli_workers_on_one_project_token_budget_run_one_at_a_time_and_stop_when_it_is_spent() {
    // 500,000 tokens for the project. Each turn reports 300,010 at its end and cannot be stopped
    // before then (Claude Code reports once), so the second turn is the one that may overshoot.
    let (h, ws) = project("shared", 3, (0.0, 0.0), "max_project_tokens = 500000");
    h.send("cli-project: make three files");
    wait("one worker to be refused", Duration::from_secs(90), &h, |h| !failed_workers(h).is_empty());
    wait("both admitted turns to end", Duration::from_secs(30), &h, |_| log(&ws).iter().filter(|(k, _, _)| k == "end").count() >= 2);

    let entries = log(&ws);
    let starts: Vec<_> = entries.iter().filter(|(k, _, _)| k == "start").cloned().collect();
    assert_eq!(starts.len(), 2, "the third worker found the budget spent and never started a CLI: {entries:?}");
    // Intervals [start, end] per process must not overlap: they shared one allowance.
    let mut spans: Vec<(u64, u64)> = starts
        .iter()
        .map(|(_, pid, s)| (*s, entries.iter().find(|(k, p, _)| k == "end" && p == pid).map(|(_, _, e)| *e).expect("each turn ended")))
        .collect();
    spans.sort();
    assert!(spans[1].0 >= spans[0].1, "the second turn started before the first one ended: {spans:?}");

    let failed = failed_workers(&h);
    let text = failed.iter().flat_map(|(_, e)| e.clone()).collect::<Vec<_>>().join("\n");
    assert!(text.contains("refused by project policy") && text.contains("budget"), "{text}");
    // Admitted usage: 300,010 plus a second turn let in with 199,990 left, which reported 300,010:
    // over the limit by one turn's overshoot and not by more.
    let used: u64 = h.snapshot().agents.iter().filter(|a| a.kind == AgentKind::Worker).map(|a| a.input_tokens + a.output_tokens).sum();
    assert!(used <= 500_000 + 300_010, "one overshoot at most: {used}");
}

#[test]
fn a_dollar_project_limit_refuses_a_priced_cli_worker_before_it_starts() {
    let (h, ws) = project("priced", 1, (3.0, 15.0), "max_project_usd = 5.0");
    h.send("cli-project: make a file");
    wait("the worker to be refused", Duration::from_secs(40), &h, |h| !failed_workers(h).is_empty());
    let text = failed_workers(&h).iter().flat_map(|(_, e)| e.clone()).collect::<Vec<_>>().join("\n");
    assert!(text.contains("refused by project policy") && text.contains("dollar") && text.contains("cli-model"), "{text}");
    assert_eq!(log(&ws).len(), 0, "no CLI process was started");
}
