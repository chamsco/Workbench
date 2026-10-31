//! M1 hardening, third review, item 2: the advisor on an API turn (the Anthropic
//! server-side tool) is a model call made on the agent's behalf, so the
//! project's model lists apply to it at the agent's role: an advisor the lists
//! do not name is left off that call, and the call itself goes ahead.
//!
//! Mock Anthropic API on localhost; nothing here calls a real provider.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use backspace_core::Harness;
use serde_json::{json, Value};

type Seen = Arc<Mutex<Vec<Value>>>;

fn mock() -> (u16, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen: Seen = Arc::default();
    let seen2 = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let seen = seen2.clone();
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
                seen.lock().unwrap().push(serde_json::from_slice(&body).unwrap());
                let out = json!({"content": [{"type": "text", "text": "hi"}], "stop_reason": "end_turn", "usage": {"input_tokens": 10, "output_tokens": 5}}).to_string();
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    (port, seen)
}

fn open(tag: &str, port: u16, policy: &str) -> Harness {
    let ws = std::env::temp_dir().join(format!("bs-advpol-{tag}-{}", std::process::id()));
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
pin_main = "mock-small"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10

[providers.mock]
kind = "anthropic"
base_url = "http://127.0.0.1:{port}"
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
id = "mock-adv"
provider = "mock"
tier = 4
description = "advisor"
input_usd_per_mtok = 15.0
output_usd_per_mtok = 75.0
max_output_tokens = 8000

[advisor]
model = "mock-adv"

{policy}
"#
        ),
    )
    .unwrap();
    Harness::open(ws).unwrap()
}

fn first_request(h: &Harness, seen: &Seen) -> Value {
    h.send("say hi");
    let deadline = Instant::now() + Duration::from_secs(30);
    while seen.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "no request reached the provider");
        std::thread::sleep(Duration::from_millis(20));
    }
    seen.lock().unwrap()[0].clone()
}

fn has_advisor(req: &Value) -> bool {
    req["tools"].as_array().is_some_and(|t| t.iter().any(|t| t["type"] == "advisor_20260301"))
}

fn system_log(h: &Harness) -> String {
    h.snapshot().agents[0].log.iter().map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn an_advisor_the_lists_do_not_name_is_left_off_and_the_call_still_goes_ahead() {
    let (port, seen) = mock();
    let h = open("off", port, "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-small\"]\nexecution_models = [\"mock-small\"]\nskills = []\nplugins = []\n");
    let req = first_request(&h, &seen);
    assert!(!has_advisor(&req), "`mock-adv` is not in the orchestration list: {}", req["tools"]);
    std::thread::sleep(Duration::from_millis(200));
    assert!(!system_log(&h).contains("refused by project policy"), "the call itself was not refused: {}", system_log(&h));
}

#[test]
fn the_same_advisor_is_attached_once_the_lists_name_it() {
    let (port, seen) = mock();
    let h = open("on", port, "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-small\", \"mock-adv\"]\nexecution_models = [\"mock-small\"]\nskills = []\nplugins = []\n");
    let req = first_request(&h, &seen);
    assert!(has_advisor(&req), "control: named for the main agent's role, it is attached: {}", req["tools"]);
}

#[test]
fn it_is_checked_at_the_role_of_the_agent_that_calls_it() {
    // Named for execution only: the main agent (orchestration) does not get it.
    let (port, seen) = mock();
    let h = open("role", port, "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-small\"]\nexecution_models = [\"mock-small\", \"mock-adv\"]\nskills = []\nplugins = []\n");
    let req = first_request(&h, &seen);
    assert!(!has_advisor(&req), "{}", req["tools"]);
}
