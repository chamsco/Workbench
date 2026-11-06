//! Delegation decisions A and B (2026-11-06): a worker starts from the
//! person's own words and the agreed spec, not from 600-character summaries
//! of its ancestors' briefs, and hands back a structured record that the
//! tickets depending on it receive along with the diff statistics.
//!
//! Drives a whole project against a scripted OpenAI-compatible mock, like
//! e2e.rs: main plans two tickets (b blocked by a), the person rejects the
//! first plan with feedback, a's worker first submits without a record.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use backspace_core::{AgentStatus, ApprovalKind, CheckRun, Deliverable, Harness, LogKind, ProjectState};
use serde_json::{json, Value};

/// Past the old 600-character cut, so only a verbatim copy carries it.
const TAIL: &str = "TAIL-MARKER-ONLY-IN-THE-ORIGINAL";
const FEEDBACK: &str = "Use Postgres, not SQLite. FEEDBACK-MARKER";

fn original_request() -> String {
    format!("handoff-project: build the two files. {} {TAIL}", "Some detail the person cared about. ".repeat(30))
}

fn tool_call(name: &str, args: Value) -> Value {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
        {"id": format!("call_{n}"), "type": "function",
         "function": {"name": name, "arguments": args.to_string()}}]}}],
        "usage": {"prompt_tokens": 100, "completion_tokens": 10}})
}

fn text(t: &str) -> Value {
    json!({"choices": [{"finish_reason": "stop", "message": {"content": t}}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
}

fn respond(req: &Value) -> Value {
    let msgs = req["messages"].as_array().unwrap();
    let s = |m: &Value| m["content"].as_str().unwrap_or("").to_string();
    let system = s(&msgs[0]);
    let first = msgs.get(1).map(s).unwrap_or_default();
    let last_tool = msgs.iter().rev().find(|m| m["role"] == "tool").map(s);
    let main = system.starts_with("You are the lead agent");
    let worker = system.starts_with("You are a Backspace ticket agent");
    let plan = json!({"tickets": [
        {"key": "a", "title": "Write A", "what_to_build": "create a.txt containing A",
         "acceptance": ["a.txt says A"], "check": "grep -q A a.txt"},
        {"key": "b", "title": "Write B", "what_to_build": "create b.txt containing B",
         "acceptance": ["b.txt says B"], "check": "grep -q B b.txt", "blocked_by": ["a"]}
    ]});
    let record = |assumption: &str, question: &str, cmd: &str| json!({
        "checks_run": [{"command": cmd, "result": "matched", "exit_status": 0}],
        "assumptions": [assumption], "open_questions": [question]
    });
    if main && first.contains("handoff-project") {
        return match last_tool.as_deref() {
            None => tool_call("create_tickets", plan),
            Some(t) if t.contains("Plan REJECTED") => tool_call("create_tickets", plan),
            Some(t) if t.contains("Plan APPROVED") => tool_call("work_tickets", json!({})),
            // The lead's own delivery needs no record: only ticket workers do.
            Some(t) if t.contains("APPROVED by the user and merged") => {
                tool_call("submit_deliverable", json!({"summary": "a.txt and b.txt"}))
            }
            Some(t) => text(&format!("unexpected: {t}")),
        };
    }
    if worker && first.starts_with("# a:") {
        return match last_tool.as_deref() {
            None => tool_call("write", json!({"path": "a.txt", "content": "A"})),
            // Prose alone: refused, nothing reaches review.
            Some(t) if t.starts_with("wrote") => tool_call("submit_deliverable", json!({"summary": "wrote a.txt"})),
            Some(t) if t.contains("needs its record") => {
                let mut input = record("A-ASSUMPTION: uppercase A", "A-QUESTION: should b be lowercase?", "grep -q A a.txt");
                input["summary"] = json!("wrote a.txt");
                tool_call("submit_deliverable", input)
            }
            Some(t) => text(&format!("unexpected: {t}")),
        };
    }
    if worker && first.starts_with("# b:") {
        return match last_tool.as_deref() {
            None => tool_call("write", json!({"path": "b.txt", "content": "B"})),
            Some(t) if t.starts_with("wrote") => {
                let mut input = record("B-ASSUMPTION", "none", "grep -q B b.txt");
                input["summary"] = json!("wrote b.txt");
                input["open_questions"] = json!([]);
                tool_call("submit_deliverable", input)
            }
            Some(t) => text(&format!("unexpected: {t}")),
        };
    }
    text(&format!("unexpected: {first}"))
}

fn serve(listener: TcpListener) {
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
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
            let out = respond(&req).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).unwrap();
        });
    }
}

fn open_project(tag: &str) -> (Harness, PathBuf) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || serve(listener));
    let ws = std::env::temp_dir().join(format!("bs-handoff-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    let cfg = format!(
        r#"
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

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
api_key_env = "NONE"

[[models]]
id = "mock-big"
provider = "mock"
tier = 4
description = "strong"
input_usd_per_mtok = 10.0
output_usd_per_mtok = 20.0
max_output_tokens = 1000
efforts = ["low", "medium", "high"]
"#
    );
    std::fs::write(ws.join("backspace.toml"), cfg).unwrap();
    (Harness::open(ws.clone()).unwrap(), ws)
}

/// Runs the project to the end: rejects the first plan with FEEDBACK,
/// approves everything else.
fn run(tag: &str) -> (ProjectState, PathBuf) {
    let (h, ws) = open_project(tag);
    h.send(original_request());
    let deadline = Instant::now() + Duration::from_secs(40);
    let (mut answered, mut rejected) = (Vec::new(), false);
    loop {
        let s = h.snapshot();
        assert!(
            Instant::now() < deadline,
            "timed out: {:#?}",
            s.agents.iter().map(|a| (&a.key, a.status, &a.log)).collect::<Vec<_>>()
        );
        if s.agents[0].status == AgentStatus::Approved {
            return (s, ws);
        }
        for a in s.pending_approvals() {
            if !answered.contains(&a.id) {
                answered.push(a.id);
                if a.kind == ApprovalKind::Plan && !rejected {
                    rejected = true;
                    h.reject(a.id, FEEDBACK);
                } else {
                    h.approve(a.id);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn first_message(s: &ProjectState, ticket: &str) -> String {
    let agent = s.agents.iter().find(|a| a.ticket.as_deref() == Some(ticket)).unwrap();
    agent.log.iter().find(|e| e.kind == LogKind::User).map(|e| e.text.clone()).unwrap_or_default()
}

#[test]
fn workers_start_from_the_persons_words_and_get_structured_records() {
    let (s, ws) = run("flow");
    let original = original_request();

    // A: the person's words, verbatim, in a request.md the worker can read.
    let request = std::fs::read_to_string(ws.join(".backspace/worktrees/a/.backspace/handoff/a/request.md")).unwrap();
    assert!(request.contains(&original), "request.md must hold the original request verbatim:\n{request}");
    assert!(request.contains(FEEDBACK), "the person's rejection feedback is part of their request:\n{request}");
    assert!(!request.contains("a.txt and b.txt"), "agents' words are not the person's:\n{request}");
    let a_first = first_message(&s, "a");
    assert!(a_first.contains(TAIL), "the original request reaches the worker uncut:\n{a_first}");
    assert!(a_first.contains(".backspace/handoff/a/"), "the worker is pointed at its handoff folder:\n{a_first}");
    assert!(!a_first.contains("Why this ticket exists"), "no summary chain of ancestor briefs:\n{a_first}");
    let ticket = std::fs::read_to_string(ws.join(".backspace/worktrees/a/.backspace/handoff/a/ticket-a.md")).unwrap();
    assert!(ticket.contains("create a.txt containing A"));
    // The handoff never reaches a commit.
    let tracked = std::process::Command::new("git")
        .args(["ls-tree", "-r", "--name-only", "backspace/a"])
        .current_dir(&ws)
        .output()
        .unwrap();
    let tracked = String::from_utf8(tracked.stdout).unwrap();
    assert!(!tracked.contains("handoff"), "{tracked}");

    // B: a worker's prose alone is refused, and the record is kept.
    let a = s.agents.iter().find(|x| x.ticket.as_deref() == Some("a")).unwrap();
    assert!(
        a.log.iter().any(|e| e.text.contains("needs its record")),
        "a delivery without a record must be refused: {:#?}",
        a.log
    );
    let ad = a.deliverable.as_ref().unwrap();
    assert!(ad.reported);
    assert_eq!(
        ad.checks_run,
        vec![CheckRun { command: "grep -q A a.txt".into(), result: "matched".into(), exit_status: Some(0) }]
    );
    assert_eq!(ad.assumptions, vec!["A-ASSUMPTION: uppercase A".to_string()]);
    assert_eq!(ad.open_questions, vec!["A-QUESTION: should b be lowercase?".to_string()]);

    // The dependent ticket gets a's record and diff statistics, not just its prose.
    let b_first = first_message(&s, "b");
    for want in ["A-ASSUMPTION: uppercase A", "A-QUESTION: should b be lowercase?", "grep -q A a.txt", "a.txt", "backspace/a"] {
        assert!(b_first.contains(want), "b should see `{want}` from a's record:\n{b_first}");
    }
    assert!(b_first.contains("diff_stat") && b_first.contains("1 file changed"), "b should see a's diff stat:\n{b_first}");
    let dep = std::fs::read_to_string(ws.join(".backspace/worktrees/b/.backspace/handoff/b/deps/a.md")).unwrap();
    assert!(dep.contains("A-ASSUMPTION: uppercase A"), "{dep}");
    std::fs::remove_dir_all(&ws).unwrap();
}

#[test]
fn a_record_left_unreported_reads_as_unknown_not_as_none() {
    let d = Deliverable { summary: "did it".into(), files: vec!["x.rs".into()], ..Default::default() };
    let r = d.record();
    assert!(r.contains("not reported"), "{r}");
    assert!(!r.contains("**assumptions:**"), "{r}");

    let d = Deliverable { reported: true, ..d };
    let r = d.record();
    assert!(r.contains("**assumptions:**\n- none"), "{r}");
    assert!(r.contains("**files_changed:**\n- x.rs"), "{r}");
}
