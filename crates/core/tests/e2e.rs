//! Drives a full project against a scripted OpenAI-compatible mock model:
//! main spawns two dependent sub-agents, one gets rejected once, everything
//! is approved, main delivers.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use backspace_core::{AgentStatus, ApprovalState, Harness};
use serde_json::{json, Value};

fn tool_call(name: &str, args: Value) -> Value {
    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
        {"id": format!("call_{name}_{}", rand_suffix()), "type": "function",
         "function": {"name": name, "arguments": args.to_string()}}]}}],
        "usage": {"prompt_tokens": 1000, "completion_tokens": 100}})
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn text(t: &str) -> Value {
    json!({"choices": [{"finish_reason": "stop", "message": {"content": t}}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
}

/// The whole "model": decides the next move from the conversation so far.
fn respond(req: &Value) -> Value {
    let msgs = req["messages"].as_array().unwrap();
    let system = msgs[0]["content"].as_str().unwrap();
    let last_tool = msgs
        .iter()
        .rev()
        .find(|m| m["role"] == "tool")
        .map(|m| m["content"].as_str().unwrap_or(""));

    if let Some(r) = respond_nested(msgs, system, last_tool) {
        return r;
    }

    if system.starts_with("You are the lead agent") {
        return match last_tool {
            None => tool_call(
                "spawn_agents",
                json!({"agents": [
                    {"key": "a", "title": "Write A", "brief": "scaffold a.txt containing A"},
                    {"key": "b", "title": "Write B", "brief": "scaffold b.txt containing B", "depends_on": ["a"]}
                ]}),
            ),
            Some(t) if t.contains("APPROVED") => tool_call(
                "submit_deliverable",
                json!({"summary": "a.txt and b.txt", "files": ["a.txt", "b.txt"]}),
            ),
            Some(t) => text(&format!("unexpected: {t}")),
        };
    }

    let user = msgs[1]["content"].as_str().unwrap();
    let (file, body) = if user.contains("scaffold a.txt") {
        ("a.txt", "A")
    } else {
        ("b.txt", "B")
    };
    if file == "b.txt" {
        assert!(
            user.contains("Dependency `a`"),
            "b should see a's deliverable"
        );
    }
    match last_tool {
        None => tool_call("write", json!({"path": file, "content": body})),
        Some(t) if t.starts_with("wrote") || t.contains("REJECTED") => tool_call(
            "submit_deliverable",
            json!({"summary": format!("wrote {file}"), "files": [file]}),
        ),
        Some(t) => text(&format!("unexpected: {t}")),
    }
}

/// main -> lead -> leaf. The lead, not the human, reviews the leaf and sends
/// it back once.
fn respond_nested(msgs: &[Value], system: &str, last_tool: Option<&str>) -> Option<Value> {
    let first = msgs[1]["content"].as_str().unwrap_or("");
    let last = msgs.last().unwrap();
    let tool_results = msgs.iter().filter(|m| m["role"] == "tool").count();
    if system.starts_with("You are the lead agent") && first.contains("nested") {
        return Some(match last_tool {
            None => tool_call(
                "spawn_agents",
                json!({"agents": [
                    {"key": "lead", "title": "Backend lead", "brief": "lead the backend team", "budget_usd": 5.0}
                ]}),
            ),
            Some(t) if t.contains("APPROVED by the user") => {
                tool_call("submit_deliverable", json!({"summary": "nested done"}))
            }
            Some(t) => text(&format!("unexpected: {t}")),
        });
    }
    if first.contains("scaffold leaf.txt") {
        assert!(
            !system.contains("# Delegation"),
            "max depth agent must not delegate"
        );
        assert!(
            first.contains("lead the backend team"),
            "leaf should see its goal ancestry"
        );
        let revised = last["role"] == "user"
            && last["content"]
                .as_str()
                .unwrap_or("")
                .contains("requested changes");
        return Some(match last_tool {
            _ if revised => tool_call("write", json!({"path": "leaf.txt", "content": "v2"})),
            None => tool_call("write", json!({"path": "leaf.txt", "content": "v1"})),
            Some(t) if t.starts_with("wrote") => tool_call(
                "submit_deliverable",
                json!({"summary": "wrote leaf", "files": ["leaf.txt"]}),
            ),
            Some(t) => text(&format!("unexpected: {t}")),
        });
    }
    if first.contains("lead the backend team") {
        assert!(
            system.contains("# Delegation"),
            "depth-1 agent should be allowed to delegate"
        );
        assert!(
            first.contains("Your budget"),
            "budget should be in the brief"
        );
        let reviews = msgs
            .iter()
            .filter(|m| {
                m["role"] == "tool"
                    && m["content"]
                        .as_str()
                        .unwrap_or("")
                        .contains("SUBMITTED FOR YOUR REVIEW")
            })
            .count();
        return Some(match (last_tool, reviews) {
            (None, _) => tool_call(
                "spawn_agents",
                json!({"agents": [
                    {"key": "leaf", "title": "Leaf", "brief": "scaffold leaf.txt"}
                ]}),
            ),
            (Some(_), 1) if tool_results == 1 => {
                tool_call("revise_agent", json!({"key": "leaf", "feedback": "say v2"}))
            }
            (Some(_), 2) => tool_call(
                "submit_deliverable",
                json!({"summary": "leaf reviewed", "files": ["leaf.txt"]}),
            ),
            (Some(t), _) => text(&format!("unexpected: {t}")),
        });
    }
    None
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

#[test]
fn full_project_with_rejection() {
    let (h, ws) = open_project("flat");
    h.send("build files a and b");

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut rejected_once = false;
    loop {
        assert!(
            Instant::now() < deadline,
            "timed out: {:#?}",
            h.snapshot().agents
        );
        let s = h.snapshot();
        if s.agents[0].status == AgentStatus::Approved {
            break;
        }
        if let Some(a) = s.pending_approvals().next() {
            if s.agents[a.agent].key == "a" && !rejected_once {
                rejected_once = true;
                h.reject(a.id, "add a trailing newline");
            } else {
                h.approve(a.id);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let s = h.snapshot();
    assert_eq!(s.agents.len(), 3);
    assert!(s.agents.iter().all(|a| a.status == AgentStatus::Approved));
    assert!(s
        .approvals
        .iter()
        .any(|a| matches!(a.state, ApprovalState::Rejected { .. })));
    // Main is routed at >= high on the strong model; subs on the cheap one.
    assert_eq!(s.agents[0].decision.as_ref().unwrap().model, "mock-big");
    assert_eq!(s.agents[1].decision.as_ref().unwrap().model, "mock-small");
    assert!(s.total_cost_usd > 0.0);
    assert_eq!(std::fs::read_to_string(ws.join("a.txt")).unwrap(), "A");
    assert_eq!(std::fs::read_to_string(ws.join("b.txt")).unwrap(), "B");
    assert!(ws.join(".backspace/state.json").is_file());
    std::fs::remove_dir_all(&ws).unwrap();
}

#[test]
fn nested_agents_review_their_reports() {
    let (h, ws) = open_project("nested");
    h.send("nested build");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        assert!(
            Instant::now() < deadline,
            "timed out: {:#?}",
            h.snapshot().agents
        );
        let s = h.snapshot();
        if s.agents[0].status == AgentStatus::Approved {
            break;
        }
        if let Some(a) = s.pending_approvals().next() {
            assert_ne!(
                s.agents[a.agent].key, "leaf",
                "depth-2 work must not reach the human"
            );
            h.approve(a.id);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let s = h.snapshot();
    let leaf = s.agents.iter().find(|a| a.key == "leaf").unwrap();
    let lead = s.agents.iter().find(|a| a.key == "lead").unwrap();
    assert_eq!((leaf.depth, leaf.parent), (2, Some(lead.id)));
    assert_eq!(
        s.approvals.len(),
        2,
        "only lead and main are human-reviewed"
    );
    assert_eq!(std::fs::read_to_string(ws.join("leaf.txt")).unwrap(), "v2");
    assert!(s.subtree_cost(lead.id) > lead.cost_usd);
    std::fs::remove_dir_all(&ws).unwrap();
}
