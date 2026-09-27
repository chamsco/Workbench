//! Drives whole projects against a scripted OpenAI-compatible mock model.
//! The mock decides each move from the conversation so far, so these tests
//! exercise the real harness: tickets, plan approval, worktrees and merges,
//! check gates, escalation, nested review and triage.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use backspace_core::{
    AgentKind, AgentStatus, ApprovalKind, ApprovalState, Effort, Harness, ProjectState, TicketState,
};
use serde_json::{json, Value};

fn tool_call(name: &str, args: Value) -> Value {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
        {"id": format!("call_{n}"), "type": "function",
         "function": {"name": name, "arguments": args.to_string()}}]}}],
        "usage": {"prompt_tokens": 1000, "completion_tokens": 100}})
}

fn text(t: &str) -> Value {
    json!({"choices": [{"finish_reason": "stop", "message": {"content": t}}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
}

struct Convo<'a> {
    system: &'a str,
    first: &'a str,
    last_tool: Option<&'a str>,
    last_user: &'a str,
    tools_seen: Vec<&'a str>,
}

fn read(req: &Value) -> Convo<'_> {
    let msgs = req["messages"].as_array().unwrap();
    fn s(m: &Value) -> &str {
        m["content"].as_str().unwrap_or("")
    }
    Convo {
        system: s(&msgs[0]),
        first: msgs.get(1).map(s).unwrap_or(""),
        last_tool: msgs.iter().rev().find(|m| m["role"] == "tool").map(s),
        // Only a user message that is the latest turn (reopen, nudge).
        last_user: msgs
            .last()
            .filter(|m| m["role"] == "user")
            .map(s)
            .unwrap_or(""),
        tools_seen: msgs.iter().filter(|m| m["role"] == "tool").map(s).collect(),
    }
}

/// The whole "model".
fn respond(req: &Value) -> Value {
    let c = read(req);
    let main = c.system.starts_with("You are the lead agent");
    let worker = c.system.starts_with("You are a Backspace ticket agent");
    let triage = c.system.starts_with("You are a Backspace triage agent");
    let reopened = c.last_user.contains("reviewer requested changes");
    let unexpected = |t: &str| text(&format!("unexpected: {t}"));

    // ---- flat project: plan gate, check gate, rejection, blocked_by
    if main && c.first.contains("flat") {
        return match c.last_tool {
            None => tool_call(
                "create_tickets",
                json!({"tickets": [
                    {"key": "a", "title": "Write A", "what_to_build": "scaffold a.txt containing A",
                     "acceptance": ["a.txt says A"], "check": "grep -q A a.txt"}
                ]}),
            ),
            Some(t) if t.contains("Plan REJECTED") => tool_call(
                "create_tickets",
                json!({"tickets": [
                    {"key": "a", "title": "Write A", "what_to_build": "scaffold a.txt containing A",
                     "acceptance": ["a.txt says A"], "check": "grep -q A a.txt"},
                    {"key": "b", "title": "Write B", "what_to_build": "scaffold b.txt containing B",
                     "acceptance": ["b.txt says B"], "check": "grep -q A a.txt && grep -q B b.txt", "blocked_by": ["a"]}
                ]}),
            ),
            Some(t) if t.contains("Plan APPROVED") => tool_call("work_tickets", json!({})),
            Some(t) if t.contains("APPROVED by the user and merged") => tool_call(
                "submit_deliverable",
                json!({"summary": "a.txt and b.txt", "files": ["a.txt", "b.txt"]}),
            ),
            Some(t) => unexpected(t),
        };
    }
    if worker && c.first.starts_with("# a:") {
        return match c.last_tool {
            // Submit before doing the work: the check must bounce it.
            None => tool_call("submit_deliverable", json!({"summary": "done?"})),
            Some(t) if t.starts_with("CHECK FAILED") => {
                tool_call("write", json!({"path": "a.txt", "content": "A"}))
            }
            Some(t) if t.starts_with("wrote") || t.contains("REJECTED") => tool_call(
                "submit_deliverable",
                json!({"summary": "wrote a.txt", "files": ["a.txt"]}),
            ),
            Some(t) => unexpected(t),
        };
    }
    if worker && c.first.starts_with("# b:") {
        assert!(
            c.first.contains("done and merged into your base"),
            "b should see a's result"
        );
        return match c.last_tool {
            None => tool_call("write", json!({"path": "b.txt", "content": "B"})),
            Some(t) if t.starts_with("wrote") => tool_call(
                "submit_deliverable",
                json!({"summary": "wrote b.txt", "files": ["b.txt"]}),
            ),
            Some(t) => unexpected(t),
        };
    }

    // ---- nested: main -> lead -> leaf, lead reviews and revises leaf
    if main && c.first.contains("nested") {
        return match c.last_tool {
            None => tool_call(
                "create_tickets",
                json!({"tickets": [
                    {"key": "lead", "title": "Backend lead", "what_to_build": "lead the backend team",
                     "acceptance": ["leaf.txt exists"], "budget_usd": 5.0}
                ]}),
            ),
            Some(t) if t.contains("Plan APPROVED") => {
                tool_call("work_tickets", json!({"keys": ["lead"]}))
            }
            Some(t) if t.contains("APPROVED by the user and merged") => {
                tool_call("submit_deliverable", json!({"summary": "nested done"}))
            }
            Some(t) => unexpected(t),
        };
    }
    if worker && c.first.starts_with("# leaf:") {
        assert!(
            !c.system.contains("# Delegation"),
            "max-depth agent must not delegate"
        );
        assert!(
            c.first.contains("lead the backend team"),
            "leaf should see its goal ancestry"
        );
        return match c.last_tool {
            _ if reopened => tool_call("write", json!({"path": "leaf.txt", "content": "v2"})),
            None => tool_call("write", json!({"path": "leaf.txt", "content": "v1"})),
            Some(t) if t.starts_with("wrote") => tool_call(
                "submit_deliverable",
                json!({"summary": "wrote leaf", "files": ["leaf.txt"]}),
            ),
            Some(t) => unexpected(t),
        };
    }
    if worker && c.first.starts_with("# lead:") {
        assert!(
            c.system.contains("# Delegation"),
            "depth-1 agent should be allowed to delegate"
        );
        assert!(
            c.first.contains("Your budget"),
            "budget should be in the brief"
        );
        let reviews = c
            .tools_seen
            .iter()
            .filter(|t| t.contains("MERGED, FOR YOUR REVIEW"))
            .count();
        return match c.last_tool {
            None => tool_call(
                "create_tickets",
                json!({"tickets": [
                    {"key": "leaf", "title": "Leaf", "what_to_build": "scaffold leaf.txt", "acceptance": ["exists"]}
                ]}),
            ),
            Some(t) if t.starts_with("Created") => tool_call("work_tickets", json!({})),
            Some(_) if reviews == 1 => tool_call(
                "revise_ticket",
                json!({"key": "leaf", "feedback": "say v2"}),
            ),
            Some(_) if reviews == 2 => tool_call(
                "submit_deliverable",
                json!({"summary": "leaf reviewed", "files": ["leaf.txt"]}),
            ),
            Some(t) => unexpected(t),
        };
    }

    // ---- triage
    if triage {
        assert!(
            c.system.contains("# Skill: triage"),
            "triage skill should be injected"
        );
        return match c.last_tool {
            None => tool_call("bash", json!({"command": "ls"})),
            Some(_) => tool_call(
                "triage_ticket",
                json!({"state": "ready_for_agent", "category": "bug",
                "notes": "reproduced", "acceptance": ["no crash on empty input"], "check": "true"}),
            ),
        };
    }
    unexpected(c.first)
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

    let ws = std::env::temp_dir().join(format!("bs-e2e-{tag}-{}", std::process::id()));
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
efforts = ["low", "medium", "high"]
"#
        ),
    )
    .unwrap();

    (Harness::open(ws.clone()).unwrap(), ws)
}

/// Poll until `done`, answering each approval once with `decide`
/// (`Ok` approves, `Err(feedback)` rejects).
fn run_until(
    h: &Harness,
    mut decide: impl FnMut(&ProjectState, usize) -> Result<(), String>,
    done: impl Fn(&ProjectState) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut answered = Vec::new();
    loop {
        let s = h.snapshot();
        assert!(
            Instant::now() < deadline,
            "timed out: {:#?}\n{:#?}",
            s.agents
                .iter()
                .map(|a| (&a.key, a.status, &a.log))
                .collect::<Vec<_>>(),
            s.tickets
        );
        if done(&s) {
            return;
        }
        for a in s.pending_approvals() {
            if !answered.contains(&a.id) {
                answered.push(a.id);
                match decide(&s, a.id) {
                    Ok(()) => h.approve(a.id),
                    Err(fb) => h.reject(a.id, fb),
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn git_branches(ws: &Path) -> String {
    let out = Command::new("git")
        .args(["branch", "--list"])
        .current_dir(ws)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn flat_project_with_gates_escalation_and_merges() {
    let (h, ws) = open_project("flat");
    h.send("flat build");
    let mut rejected_plan = false;
    let mut rejected_a = false;
    run_until(
        &h,
        |s, id| {
            let ap = &s.approvals[id];
            if ap.kind == ApprovalKind::Plan && !rejected_plan {
                rejected_plan = true;
                return Err("also add b".into());
            }
            if ap.tickets == ["a"] && !rejected_a {
                rejected_a = true;
                return Err("looks thin".into());
            }
            Ok(())
        },
        |s| s.agents[0].status == AgentStatus::Approved,
    );

    let s = h.snapshot();
    // Plan gate: first plan rejected and discarded, second approved.
    let plans: Vec<_> = s
        .approvals
        .iter()
        .filter(|a| a.kind == ApprovalKind::Plan)
        .collect();
    assert_eq!(plans.len(), 2);
    assert!(matches!(plans[0].state, ApprovalState::Rejected { .. }));
    assert_eq!(s.tickets.len(), 2);
    assert!(
        s.tickets.iter().all(|t| t.state == TicketState::Done),
        "{:#?}",
        s.tickets
    );

    // Worktrees and merges: both files landed on the main branch.
    assert_eq!(std::fs::read_to_string(ws.join("a.txt")).unwrap(), "A");
    assert_eq!(std::fs::read_to_string(ws.join("b.txt")).unwrap(), "B");
    assert!(ws.join(".backspace/worktrees/a/.git").exists());
    let branches = git_branches(&ws);
    assert!(
        branches.contains("backspace/a") && branches.contains("backspace/b"),
        "{branches}"
    );

    // Escalation: a started cheap, failed its check, then was rejected once.
    let a = s
        .agents
        .iter()
        .find(|x| x.ticket.as_deref() == Some("a"))
        .unwrap();
    assert_eq!(a.escalations.len(), 2, "{:?}", a.escalations);
    assert!(
        a.escalations[0].starts_with("mock-small") && a.escalations[0].contains("check failed")
    );
    assert!(a.escalations[1].contains("rejected by the user"));
    assert_eq!(a.decision.as_ref().unwrap().model, "mock-big");
    let b = s
        .agents
        .iter()
        .find(|x| x.ticket.as_deref() == Some("b"))
        .unwrap();
    assert!(b.escalations.is_empty());
    assert!(b
        .deliverable
        .as_ref()
        .unwrap()
        .diff_stat
        .as_ref()
        .unwrap()
        .contains("b.txt"));

    // Ticket files and the outcome log exist for later learning.
    let tickets: Vec<_> = std::fs::read_dir(ws.join(".backspace/tickets"))
        .unwrap()
        .collect();
    assert_eq!(tickets.len(), 2);
    let outcomes = std::fs::read_to_string(ws.join(".backspace/outcomes.jsonl")).unwrap();
    assert_eq!(outcomes.lines().count(), 2);
    std::fs::remove_dir_all(&ws).unwrap();
}

#[test]
fn nested_leads_review_and_revise_their_reports() {
    let (h, ws) = open_project("nested");
    h.send("nested build");
    run_until(
        &h,
        |s, id| {
            assert_ne!(
                s.approvals[id].tickets,
                ["leaf"],
                "depth-2 work must not reach the human"
            );
            Ok(())
        },
        |s| s.agents[0].status == AgentStatus::Approved,
    );
    let s = h.snapshot();
    let lead = s
        .agents
        .iter()
        .find(|a| a.ticket.as_deref() == Some("lead"))
        .unwrap();
    let leaf = s
        .agents
        .iter()
        .find(|a| a.ticket.as_deref() == Some("leaf"))
        .unwrap();
    assert_eq!((leaf.depth, leaf.parent), (2, Some(lead.id)));
    assert_eq!(s.approvals.len(), 3, "plan, lead, main");
    // leaf -> lead's branch -> main's branch, including the revision.
    assert_eq!(std::fs::read_to_string(ws.join("leaf.txt")).unwrap(), "v2");
    assert_eq!(leaf.escalations.len(), 1);
    assert!(s.subtree_cost(lead.id) > lead.cost_usd);
    std::fs::remove_dir_all(&ws).unwrap();
}

#[test]
fn filed_tickets_get_triaged() {
    let (h, ws) = open_project("triage");
    let msg = h
        .file_ticket("Crash on empty input", "The parser panics on an empty file")
        .unwrap();
    assert!(msg.contains("crash-on-empty-input-1"), "{msg}");
    run_until(
        &h,
        |_, _| Ok(()),
        |s| s.tickets[0].state != TicketState::NeedsTriage,
    );
    let s = h.snapshot();
    let t = &s.tickets[0];
    assert_eq!(t.state, TicketState::ReadyForAgent);
    assert_eq!(t.check.as_deref(), Some("true"));
    assert!(t.notes.iter().any(|n| n.contains("reproduced")));
    let triager = s
        .agents
        .iter()
        .find(|a| a.kind == AgentKind::Triage)
        .unwrap();
    assert_eq!(triager.decision.as_ref().unwrap().effort, Effort::Low);
    std::fs::remove_dir_all(&ws).unwrap();
}
