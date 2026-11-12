//! M2 acceptance: a turn in a coding CLI is a call of its model's class. The
//! CLI worker is billed to a plan ($0 here) and is tier 3, so it is a
//! frontier call all the same. Its own process: the fake `claude` is found
//! through an environment variable that tests in one process would share.
#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use backspace_core::{AgentStatus, Harness};
use serde_json::{json, Value};

fn tool_call(name: &str, args: Value) -> Value {
    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
        {"id": "call_1", "type": "function", "function": {"name": name, "arguments": args.to_string()}}]}}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
}

fn serve(listener: TcpListener) {
    let n = std::sync::atomic::AtomicUsize::new(0);
    let n = std::sync::Arc::new(n);
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
                return; // the router's liveness probe
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
                2 => tool_call("submit_deliverable", json!({"summary": "hello.txt", "files": ["hello.txt"]})),
                _ => json!({"choices": [{"finish_reason": "stop", "message": {"content": "done"}}],
                    "usage": {"prompt_tokens": 10, "completion_tokens": 5}}),
            }
            .to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).unwrap();
        });
    }
}

#[test]
fn a_cli_workers_turn_is_a_frontier_call_even_though_it_costs_nothing() {
    let bin = std::env::temp_dir().join(format!("bs-metrics-cli-bin-{}", std::process::id()));
    std::fs::create_dir_all(&bin).unwrap();
    let script = bin.join("claude");
    std::fs::write(
        &script,
        r#"#!/bin/sh
cat > /dev/null
printf 'hello\n' > hello.txt
echo '{"type":"system","subtype":"init","session_id":"fake-1"}'
echo '{"type":"result","result":"Created hello.txt saying hello","session_id":"fake-1","total_cost_usd":0.01,"is_error":false}'
"#,
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::env::set_var("BACKSPACE_CLI_PATH", &bin);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || serve(listener));
    let ws = std::env::temp_dir().join(format!("bs-metrics-cli-{}", std::process::id()));
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
pin_sub = "claude-code"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
api_key_env = "NONE"

[providers.claude-code]
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
id = "claude-code"
provider = "claude-code"
tier = 3
description = "Claude Code"
input_usd_per_mtok = 0.0
output_usd_per_mtok = 0.0
max_output_tokens = 0
"#
        ),
    )
    .unwrap();
    let h = Harness::open(ws.clone()).unwrap();
    h.send("cli-project: say hello");

    let deadline = Instant::now() + Duration::from_secs(40);
    let mut answered = vec![];
    loop {
        let s = h.snapshot();
        assert!(Instant::now() < deadline, "timed out: {:#?}", s.agents.iter().map(|a| (&a.key, a.status, &a.log)).collect::<Vec<_>>());
        if s.agents[0].status == AgentStatus::Approved {
            break;
        }
        for a in s.pending_approvals() {
            if !answered.contains(&a.id) {
                answered.push(a.id);
                h.approve(a.id);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let m = h.metrics();
    assert_eq!(m.frontier_calls, 1, "the planner is mid; only the CLI turn is frontier: {m:?}");
    assert!(m.calls >= 4, "three planner calls and the CLI turn: {m:?}");
    assert_eq!((m.tasks, m.merged), (1, 1));
    let _ = std::fs::remove_dir_all(&bin);
}
