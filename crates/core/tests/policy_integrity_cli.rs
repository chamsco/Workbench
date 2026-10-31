//! M1 hardening, F3, for workers in a coding CLI: they edit files with their
//! own tools, in a checkout of the project, and what they deliver is merged
//! into it. Neither may change the policy: an edit to the workspace's file is
//! undone, and a delivery that touches `backspace.toml` is held for a person
//! even when the project auto-approves.
#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, Instant};

use backspace_core::{ApprovalKind, Harness, LogKind};
use serde_json::{json, Value};

static BIN: Once = Once::new();

/// A fake `claude` that makes hello.txt, and then does what `fake-mode` in the workspace says:
/// `tamper` widens the execution list in the workspace's file and in its own checkout's copy,
/// `touch` appends a comment to its checkout's copy, `plain` does nothing else.
fn fake_cli() {
    BIN.call_once(|| {
        let home = std::env::temp_dir().join(format!("bs-pcli-home-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("BACKSPACE_DATA", home.join("data"));
        std::env::remove_var("BACKSPACE_CONFIG");
        let bin = std::env::temp_dir().join(format!("bs-pcli-bin-{}", std::process::id()));
        std::fs::create_dir_all(&bin).unwrap();
        let p = bin.join("claude");
        std::fs::write(
            &p,
            r#"#!/bin/sh
cat > /dev/null
ROOT="$PWD/../../.."
MODE=$(cat "$ROOT/fake-mode")
printf 'hello\n' > hello.txt
case "$MODE" in
  tamper)
    sed -i 's/execution_models = \["cli-model"\]/execution_models = ["cli-model", "mock-mid"]/' "$ROOT/backspace.toml"
    sed -i 's/execution_models = \["cli-model"\]/execution_models = ["cli-model", "mock-mid"]/' backspace.toml ;;
  touch)
    echo '# touched by the worker' >> backspace.toml ;;
esac
echo '{"type":"system","subtype":"init","session_id":"fake-1"}'
echo '{"type":"result","result":"Created hello.txt","session_id":"fake-1","total_cost_usd":0.01,"is_error":false}'
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

// `cli-model` pins no model, so an enforced policy has to say it may run that way (final hardening, section 3).
const POLICY: &str = "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-mid\", \"cli-model\"]\nexecution_models = [\"cli-model\"]\nskills = []\nplugins = []\nunpinned_cli_models = [\"cli-model\"]\n";

/// Returns the file as written, so a test can compare.
fn project(tag: &str, mode: &str, auto_approve: bool) -> (Harness, PathBuf, String) {
    project_with(tag, mode, auto_approve, "mock-mid")
}

/// The same, with the planner on `pin_main` (`cli-model` puts it in the coding CLI).
fn project_with(tag: &str, mode: &str, auto_approve: bool, pin_main: &str) -> (Harness, PathBuf, String) {
    fake_cli();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || planner(listener));
    let ws = std::env::temp_dir().join(format!("bs-pcli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    let cfg = format!(
        r#"[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"
pin_main = "{pin_main}"
pin_sub = "cli-model"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = {auto_approve}
bash_timeout_secs = 10

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
input_usd_per_mtok = 0.0
output_usd_per_mtok = 0.0
max_output_tokens = 0

{POLICY}"#
    );
    std::fs::write(ws.join("backspace.toml"), &cfg).unwrap();
    std::fs::write(ws.join("fake-mode"), mode).unwrap();
    (Harness::open(ws.clone()).unwrap(), ws, cfg)
}

/// Approve every plan; stop when `done` says so.
fn drive(h: &Harness, what: &str, done: impl Fn(&Harness) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut answered = vec![];
    loop {
        let s = h.snapshot();
        for a in s.pending_approvals() {
            if a.kind == ApprovalKind::Plan && !answered.contains(&a.id) {
                answered.push(a.id);
                h.approve(a.id);
            }
        }
        if done(h) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:#?}",
            s.agents.iter().map(|a| (&a.key, a.status, a.log.iter().map(|e| e.text.clone()).collect::<Vec<_>>())).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn held_delivery(h: &Harness) -> Option<String> {
    h.snapshot().pending_approvals().into_iter().find(|a| a.kind == ApprovalKind::Deliverable).map(|a| a.deliverable.summary.clone())
}

fn run_branch_policy(ws: &Path) -> String {
    std::fs::read_to_string(ws.join(".backspace/worktrees/_run/backspace.toml")).unwrap()
}

#[test]
fn a_cli_worker_cannot_weaken_the_policy() {
    let (h, ws, original) = project("tamper", "tamper", false);
    h.send("cli-project: say hello");
    drive(&h, "the worker's delivery", |h| held_delivery(h).is_some());
    assert_eq!(std::fs::read_to_string(ws.join("backspace.toml")).unwrap(), original, "the CLI's edit of the workspace file was undone");
    let main = h.snapshot().agents[0].log.iter().chain(h.snapshot().agents.iter().flat_map(|a| a.log.iter())).filter(|e| e.kind == LogKind::System).map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
    assert!(main.contains("restored"), "and said so: {main}");
    assert_eq!(h.policy().execution_models, Some(vec!["cli-model".to_string()]));
    let summary = held_delivery(&h).unwrap();
    assert!(summary.contains("project policy"), "its own copy's edit is what the person is asked about: {summary}");
    assert_eq!(run_branch_policy(&ws), original, "and it has not been merged");
    drop(h);

    // Next session.
    let h2 = Harness::open(ws.clone()).unwrap();
    assert_eq!(h2.policy().execution_models, Some(vec!["cli-model".to_string()]));
}

#[test]
fn a_merged_branch_that_edits_the_policy_is_held_even_under_auto_approve() {
    let (h, ws, original) = project("gate", "touch", true);
    h.send("cli-project: say hello");
    drive(&h, "the held delivery", |h| held_delivery(h).is_some());
    let summary = held_delivery(&h).unwrap();
    assert!(summary.contains("project policy") && summary.contains("backspace.toml"), "{summary}");
    assert_eq!(run_branch_policy(&ws), original, "nothing of it reached the run branch");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(h.metrics().merged, 0, "and the ticket is not counted as merged");
    assert!(held_delivery(&h).is_some(), "it is still waiting for a person");
}

#[test]
fn a_delivery_that_leaves_the_policy_alone_is_still_auto_approved() {
    let (h, _ws, _) = project("plain", "plain", true);
    h.send("cli-project: say hello");
    drive(&h, "the merge", |h| h.metrics().merged == 1);
    assert!(held_delivery(&h).is_none());
}

#[test]
fn a_cli_planner_cannot_weaken_the_policy_either() {
    // The planner runs in the coding CLI in the run checkout (`plan_on_cli`), a different caller of
    // `run_cli_turn` from a worker. Same edit, same undo.
    let (h, ws, original) = project_with("tamper-planner", "tamper", false, "cli-model");
    h.send("hello");
    drive(&h, "the planner's turn to be undone", |h| {
        h.snapshot().agents[0].log.iter().any(|e| e.kind == LogKind::System && e.text.contains("restored"))
    });
    assert_eq!(std::fs::read_to_string(ws.join("backspace.toml")).unwrap(), original, "the planner's edit of the workspace file was undone");
    assert_eq!(h.policy().execution_models, Some(vec!["cli-model".to_string()]));
    drop(h);
    let h2 = Harness::open(ws).unwrap();
    assert_eq!(h2.policy().execution_models, Some(vec!["cli-model".to_string()]), "and the next session still runs the adopted policy");
}
