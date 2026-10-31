//! M1 hardening, third review, item 2: what a coding CLI is allowed to invoke
//! beyond its own model. The project's model policy names the CLI as one
//! entry (`cli-model` here); the CLI runs the model its own configuration
//! picks. But the harness also hands Claude Code an advisor and a scout
//! subagent, each on a model named in `[advisor]`, and those must pass the
//! same lists: a model the policy does not name is not invoked on the
//! project's behalf.
//!
//! Fake `claude` that records the arguments it was started with. Nothing here
//! is a real CLI, so these tests show what Backspace passes it, not what Claude
//! Code then does with it.
#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use backspace_core::{Harness, LogKind};
use serde_json::{json, Value};

static BIN: Once = Once::new();

fn fake_claude() {
    BIN.call_once(|| {
        let home = std::env::temp_dir().join(format!("bs-clipol-home-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("BACKSPACE_DATA", home.join("data"));
        std::env::remove_var("BACKSPACE_CONFIG");
        let bin = std::env::temp_dir().join(format!("bs-clipol-bin-{}", std::process::id()));
        std::fs::create_dir_all(&bin).unwrap();
        let p = bin.join("claude");
        std::fs::write(
            &p,
            r#"#!/bin/sh
cat > /dev/null
d="$PWD"
# a ticket worktree has a .git file; the workspace has the .git directory
while [ ! -d "$d/.git" ] && [ "$d" != "/" ]; do d=$(dirname "$d"); done
printf '%s\n' "$@" > "$d/cli-args.txt"
echo made > out.txt
echo '{"type":"system","subtype":"init","session_id":"fake-1"}'
echo '{"type":"result","result":"made a file","session_id":"fake-1","total_cost_usd":0.01,"is_error":false}'
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
            let out = match n.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                0 => tool_call(
                    "create_tickets",
                    json!({"tickets": [{"key": "job", "title": "Make a file", "what_to_build": "make a file",
                        "acceptance": ["a file exists"], "check": "ls out.txt"}]}),
                ),
                1 => tool_call("work_tickets", json!({})),
                _ => json!({"choices": [{"finish_reason": "stop", "message": {"content": "done"}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}}),
            }
            .to_string();
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
        });
    }
}

/// Lists that name the CLI (and the mid model that plans) and nothing else.
const NARROW: &str = "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-mid\", \"cli-model\"]\nexecution_models = [\"cli-model\"]\nskills = []\nplugins = []\n";
const WITH_BOTH: &str = "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-mid\", \"cli-model\", \"opus\", \"haiku\"]\nexecution_models = [\"cli-model\", \"opus\", \"haiku\"]\nskills = []\nplugins = []\n";
/// `opus` is named for orchestration only, `haiku` for execution only.
const SPLIT: &str = "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-mid\", \"cli-model\", \"opus\"]\nexecution_models = [\"cli-model\", \"haiku\"]\nskills = []\nplugins = []\n";
const WITH_OPUS: &str = "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-mid\", \"cli-model\", \"opus\"]\nexecution_models = [\"cli-model\", \"opus\"]\nskills = []\nplugins = []\n";

fn project(tag: &str, policy: &str, pin_main: &str) -> (Harness, PathBuf) {
    fake_claude();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || planner(listener));
    let ws = std::env::temp_dir().join(format!("bs-clipol-{tag}-{}", std::process::id()));
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
pin_main = "{pin_main}"
pin_sub = "cli-model"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = true
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

{policy}
"#
        ),
    )
    .unwrap();
    (Harness::open(ws.clone()).unwrap(), ws)
}

/// Start a worker (the planner is an API model) and give back the arguments Claude Code was
/// started with, and the system log.
fn run(tag: &str, policy: &str) -> (Vec<String>, String) {
    started(tag, policy, "mock-mid", "cli-project: make a file")
}

/// The planner itself runs in Claude Code (`pin_main` names the CLI's entry).
fn run_planner(tag: &str, policy: &str) -> (Vec<String>, String) {
    started(tag, policy, "cli-model", "hello")
}

fn started(tag: &str, policy: &str, pin_main: &str, message: &str) -> (Vec<String>, String) {
    let (h, ws) = project(tag, policy, pin_main);
    h.send(message);
    let file = ws.join("cli-args.txt");
    let deadline = Instant::now() + Duration::from_secs(40);
    while !Path::new(&file).is_file() {
        for a in h.snapshot().pending_approvals() {
            h.approve(a.id);
        }
        assert!(
            Instant::now() < deadline,
            "the CLI was never started: {:#?}",
            h.snapshot().agents.iter().map(|a| (&a.key, a.status, a.log.iter().map(|e| e.text.clone()).collect::<Vec<_>>())).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(30));
    }
    std::thread::sleep(Duration::from_millis(300));
    let args = std::fs::read_to_string(&file).unwrap().lines().map(String::from).collect();
    let log = h
        .snapshot()
        .agents
        .iter()
        .flat_map(|a| a.log.iter().filter(|e| e.kind == LogKind::System).map(|e| e.text.clone()))
        .collect::<Vec<_>>()
        .join("\n");
    (args, log)
}

fn has_pair(args: &[String], flag: &str, value: &str) -> bool {
    args.windows(2).any(|w| w[0] == flag && w[1].contains(value))
}

#[test]
fn claude_codes_advisor_and_scouts_are_left_off_when_the_policy_does_not_name_their_models() {
    let (args, log) = run("narrow", NARROW);
    assert!(!args.iter().any(|a| a == "--advisor"), "the advisor `opus` is not in the lists: {args:?}");
    assert!(!args.iter().any(|a| a == "--agents"), "the scout on `haiku` is not in the lists: {args:?}");
    assert!(log.contains("advisor") && log.contains("scout") && log.contains("refused by project policy"), "and the log says why: {log}");
}

#[test]
fn they_are_attached_when_the_policy_names_both_models() {
    let (args, log) = run("both", WITH_BOTH);
    assert!(has_pair(&args, "--advisor", "opus"), "{args:?}");
    assert!(has_pair(&args, "--agents", "haiku"), "{args:?}");
    assert!(!log.contains("refused by project policy"), "{log}");
}

#[test]
fn each_is_decided_on_its_own() {
    let (args, _) = run("opus-only", WITH_OPUS);
    assert!(has_pair(&args, "--advisor", "opus"), "{args:?}");
    assert!(!args.iter().any(|a| a == "--agents"), "haiku is not in the lists: {args:?}");
}

#[test]
fn a_project_with_no_policy_keeps_what_it_always_got() {
    let (args, _) = run("open", "");
    assert!(has_pair(&args, "--advisor", "opus") && has_pair(&args, "--agents", "haiku"), "{args:?}");
}

#[test]
fn a_workers_advisor_is_checked_at_the_workers_role_not_the_planners() {
    // `opus` is on the orchestration list only. A worker carries out bounded work: execution.
    let (args, log) = run("split-worker", SPLIT);
    assert!(!args.iter().any(|a| a == "--advisor"), "`opus` is not an execution model: {args:?}");
    assert!(has_pair(&args, "--agents", "haiku"), "and `haiku`, which is, is attached: {args:?}");
    assert!(log.contains("advisor") && log.contains("refused by project policy") && log.contains("execution"), "{log}");
}

#[test]
fn a_planner_in_claude_code_gets_its_advisor_when_the_orchestration_list_names_it() {
    let (args, _) = run_planner("split-planner", SPLIT);
    assert!(has_pair(&args, "--advisor", "opus"), "the planner decides, so `opus` is checked against the orchestration list: {args:?}");
    assert!(has_pair(&args, "--agents", "haiku"), "scouts are execution work: {args:?}");
}

#[test]
fn a_planner_in_claude_code_loses_what_the_policy_does_not_name() {
    let (args, log) = run_planner("narrow-planner", NARROW);
    assert!(!args.iter().any(|a| a == "--advisor") && !args.iter().any(|a| a == "--agents"), "{args:?}");
    assert!(log.contains("refused by project policy"), "{log}");
}
