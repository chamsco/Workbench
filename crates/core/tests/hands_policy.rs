//! M4 closure: `browser_task`'s decision model (hands/decide.rs) is a model
//! call. Inside a project it is checked against the project's model policy
//! through the bridge before any browser or model is touched; the policy
//! audit (policy_matrix.rs, M1-BUDGET-BOUNDARIES.md) lists it with its row.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use backspace_core::hands::decide::{Decider, Endpoint};
use backspace_core::hands::{self, Env};
use serde_json::{json, Value};

#[test]
fn the_models_a_decider_calls_are_named() {
    let ep = |m: &str| Endpoint { base: "http://x".into(), model: m.into(), key: String::new() };
    assert_eq!(hands::decider_models(&Decider::Local(ep("qwen3:4b"))), ["qwen3:4b"]);
    assert_eq!(hands::decider_models(&Decider::Jev { key: "k".into(), model: "jev-latest".into(), text: Some(ep("inception/mercury-2.5")) }), ["jev-latest", "inception/mercury-2.5"]);
    assert_eq!(hands::decider_models(&Decider::Jev { key: "k".into(), model: "jev-latest".into(), text: None }), ["jev-latest"]);
}

/// A bridge that refuses every policy question and keeps what it was asked.
fn refusing_bridge() -> (u16, Arc<Mutex<Vec<(String, Value)>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(vec![]));
    let s2 = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let path = first.split_whitespace().nth(1).unwrap_or("").to_string();
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
            s2.lock().unwrap().push((path, serde_json::from_slice(&body).unwrap_or(Value::Null)));
            let out = json!({"error": "the project's model policy does not allow `jev-latest` for this agent"}).to_string();
            write!(stream, "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).unwrap();
        }
    });
    (port, seen)
}

#[test]
fn a_browser_task_in_a_project_asks_the_policy_first_and_a_refusal_runs_nothing() {
    let data = std::env::temp_dir().join(format!("bs-hands-policy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).unwrap();
    let prefs = data.join("prefs.json");
    std::fs::write(&prefs, json!({"hands": {"decider": "jev"}}).to_string()).unwrap();
    // Chrome must never be reached: point it at something that cannot start.
    let (port, seen) = refusing_bridge();
    std::env::set_var("BACKSPACE_DATA", &data);
    std::env::set_var("BACKSPACE_PREFS", &prefs);
    std::env::set_var("TYPESAFE_API_KEY", "test-key");
    std::env::remove_var("TYPESAFE_MODEL");
    std::env::set_var("BACKSPACE_BRIDGE", format!("http://127.0.0.1:{port}"));
    std::env::set_var("BACKSPACE_BRIDGE_TOKEN", "t");
    std::env::set_var("BACKSPACE_AGENT", "w1");

    let env = Env { who: "w1".into(), browser: true, desktop: false };
    let e = format!("{:#}", hands::call(&env, "browser_task", &json!({"goal": "book a table"})).expect_err("refused by the policy"));
    assert!(e.contains("policy") && e.contains("jev-latest"), "{e}");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "one question, before anything else: {seen:?}");
    assert_eq!(seen[0].0, "/v1/policy/model");
    assert_eq!(seen[0].1["me"], "w1");
    assert!(seen[0].1["models"].as_array().unwrap().iter().any(|m| m == "jev-latest"), "{:?}", seen[0].1);
    assert!(!data.join("hands/agents/w1/tab").exists(), "no browser was opened");
}
