//! M1 hardening, F4: a worker in a coding CLI is held to the task budget.
//! A turn is not started when the task is already at its limit, a turn that
//! reports usage past the limit is stopped, and a dollar budget cannot be
//! trusted to a model that has no price, so it is refused.
//!
//! One process, one bin folder for the fake CLIs (found through an
//! environment variable); each project counts how often its CLI was run in a
//! file in its own workspace.
#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, Instant};

use backspace_core::{AgentKind, AgentStatus, Harness, LogKind};
use serde_json::{json, Value};

static BIN: Once = Once::new();

/// `claude` reports 600,000 input tokens when its turn ends and changes nothing, so the ticket's
/// check fails and a second round would follow. `codex` reports the same the moment it starts
/// and then goes on working for a minute.
fn fake_clis() {
    BIN.call_once(|| {
        let bin = std::env::temp_dir().join(format!("bs-clibud-bin-{}", std::process::id()));
        std::fs::create_dir_all(&bin).unwrap();
        let scripts = [
            (
                "claude",
                r#"#!/bin/sh
cat > /dev/null
echo run >> "$PWD/../../../cli-runs.log"
echo '{"type":"system","subtype":"init","session_id":"fake-1"}'
echo '{"type":"result","result":"I looked around","session_id":"fake-1","total_cost_usd":0.01,"is_error":false,"usage":{"input_tokens":600000,"output_tokens":10}}'
"#,
            ),
            (
                "codex",
                r#"#!/bin/sh
cat > /dev/null
echo run >> "$PWD/../../../cli-runs.log"
echo '{"type":"turn.completed","usage":{"input_tokens":600000,"output_tokens":10}}'
exec sleep 60
"#,
            ),
        ];
        for (name, body) in scripts {
            let p = bin.join(name);
            std::fs::write(&p, body).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::env::set_var("BACKSPACE_CLI_PATH", &bin);
    });
}

fn tool_call(name: &str, args: Value) -> Value {
    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
        {"id": "call_1", "type": "function", "function": {"name": name, "arguments": args.to_string()}}]}}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
}

/// The planner: one ticket, then hand it to a worker, then nothing more to say.
fn planner(listener: TcpListener) {
    let n = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
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
            let out = match n.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                0 => tool_call(
                    "create_tickets",
                    json!({"tickets": [{"key": "hello", "title": "Say hello",
                        "what_to_build": "create hello.txt containing hello",
                        "acceptance": ["hello.txt says hello"], "check": "grep -q hello hello.txt"}]}),
                ),
                1 => tool_call("work_tickets", json!({})),
                _ => json!({"choices": [{"finish_reason": "stop", "message": {"content": "done"}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}}),
            }
            .to_string();
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
        });
    }
}

fn project(tag: &str, cli: &str, policy: &str) -> (Harness, PathBuf) {
    fake_clis();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || planner(listener));
    let ws = std::env::temp_dir().join(format!("bs-clibud-{tag}-{}", std::process::id()));
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
auto_approve = false
bash_timeout_secs = 10

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
api_key_env = "NONE"

[providers.the-cli]
kind = "cli"
cli = "{cli}"

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
input_usd_per_mtok = 0.0
output_usd_per_mtok = 0.0
max_output_tokens = 0

{policy}
"#
        ),
    )
    .unwrap();
    (Harness::open(ws.clone()).unwrap(), ws)
}

/// Approve the plan, then wait for the worker to fail, and give back what it said.
fn worker_failure(h: &Harness, within: Duration) -> Vec<String> {
    let deadline = Instant::now() + within;
    let mut answered = vec![];
    loop {
        let s = h.snapshot();
        for a in s.pending_approvals() {
            if !answered.contains(&a.id) {
                answered.push(a.id);
                h.approve(a.id);
            }
        }
        if let Some(w) = s.agents.iter().find(|a| a.kind == AgentKind::Worker && a.status == AgentStatus::Failed) {
            return w.log.iter().filter(|e| e.kind == LogKind::Error).map(|e| e.text.clone()).collect();
        }
        assert!(
            Instant::now() < deadline,
            "no worker failed within {within:?}: {:#?}",
            s.agents.iter().map(|a| (&a.key, a.status, a.log.iter().map(|e| e.text.clone()).collect::<Vec<_>>())).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn runs(ws: &Path) -> usize {
    std::fs::read_to_string(ws.join("cli-runs.log")).map_or(0, |t| t.lines().count())
}

#[test]
fn a_cli_turn_is_refused_when_the_task_budget_is_spent() {
    // 500,000 tokens a task. The first turn reports 600,000 and fixes nothing, so the loop would go on.
    let (h, ws) = project("spent", "claude", "[policy.budget]\nmax_tokens = 500000\n");
    h.send("cli-project: say hello");
    let errors = worker_failure(&h, Duration::from_secs(40)).join("\n");
    assert!(errors.contains("refused by project policy") && errors.contains("budget"), "{errors}");
    assert_eq!(runs(&ws), 1, "the second round must not start a second CLI process");
}

#[test]
fn a_running_cli_turn_is_stopped_when_it_crosses_the_limit() {
    // Codex reports its usage while it works. It does so at once, and then keeps going for a minute.
    let (h, ws) = project("running", "codex", "[policy.budget]\nmax_tokens = 500000\n");
    h.send("cli-project: say hello");
    let started = Instant::now();
    let errors = worker_failure(&h, Duration::from_secs(25)).join("\n");
    assert!(errors.contains("budget"), "{errors}");
    assert!(started.elapsed() < Duration::from_secs(25), "it was stopped, not waited for");
    assert_eq!(runs(&ws), 1);
}

#[test]
fn a_dollar_budget_with_an_unpriced_cli_model_fails_closed() {
    // A subscription CLI has no price to count against dollars, so a dollar limit would not bind it.
    let (h, ws) = project("unpriced", "claude", "[policy.budget]\nmax_usd = 5.0\n");
    h.send("cli-project: say hello");
    let errors = worker_failure(&h, Duration::from_secs(40)).join("\n");
    assert!(errors.contains("refused by project policy") && errors.contains("cli-model") && errors.contains("dollar"), "{errors}");
    assert_eq!(runs(&ws), 0, "the CLI was never started");
}

#[test]
fn a_cli_worker_without_any_budget_runs_as_before() {
    let (h, ws) = project("free", "claude", "");
    h.send("cli-project: say hello");
    // Its check fails (it changes nothing), but it is let run: four rounds, then it gives up.
    let errors = worker_failure(&h, Duration::from_secs(60)).join("\n");
    assert!(!errors.contains("refused by project policy"), "{errors}");
    assert_eq!(runs(&ws), 4);
}
