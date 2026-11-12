//! M2 acceptance, through the real harness: every model call lands in the
//! ledger, frontier calls are counted by the model's class, and the §3.4
//! metrics are written to `.backspace/metrics.jsonl` as the run goes.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use backspace_core::metrics::RunMetrics;
use backspace_core::Harness;
use serde_json::{json, Value};

fn mock(reply: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
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
                    return; // the router's liveness probe
                }
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let out = json!({"choices": [{"finish_reason": "stop", "message": {"content": reply}}],
                    "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
                .to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).unwrap();
            });
        }
    });
    port
}

fn open(tag: &str, port: u16, pin_main: &str) -> (Harness, PathBuf) {
    let ws = std::env::temp_dir().join(format!("bs-metrics-{tag}-{}", std::process::id()));
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
id = "mock-mid"
provider = "mock"
tier = 2
description = "mid"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000

[[models]]
id = "mock-front"
provider = "mock"
tier = 4
description = "frontier"
input_usd_per_mtok = 10.0
output_usd_per_mtok = 20.0
max_output_tokens = 1000
"#
        ),
    )
    .unwrap();
    (Harness::open(ws.clone()).unwrap(), ws)
}

/// The last line of the metrics file, once there is one with `calls` in it.
fn last_line(ws: &PathBuf, calls: u64) -> RunMetrics {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(s) = std::fs::read_to_string(ws.join(".backspace/metrics.jsonl")) {
            if let Some(m) = s.lines().last().and_then(|l| serde_json::from_str::<RunMetrics>(l).ok()) {
                if m.calls >= calls {
                    return m;
                }
            }
        }
        assert!(Instant::now() < deadline, "no metrics line with {calls} call(s) was written");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_frontier_planner_turn_is_one_frontier_call_with_its_tokens_and_cost() {
    let (h, ws) = open("front", mock("hi"), "mock-front");
    h.send("say hi");
    let m = last_line(&ws, 1);
    assert_eq!((m.calls, m.frontier_calls, m.total_tokens), (1, 1, 15));
    assert!((m.total_usd - (10.0 * 10.0 + 5.0 * 20.0) / 1e6).abs() < 1e-12, "{}", m.total_usd);
    assert_eq!(m.human_tokens_typed, 2, "\"say hi\" is 6 characters");
    assert_eq!(m.human_tokens_read, 1, "\"hi\" is 2 characters");
    assert_eq!(m.unit, "ticket");
    assert_eq!((m.merged, m.cost_to_green_usd, m.usd_per_merged), (0, None, None));
}

#[test]
fn a_mid_model_turn_is_not_a_frontier_call() {
    let (h, ws) = open("mid", mock("hi"), "mock-mid");
    h.send("say hi");
    let m = last_line(&ws, 1);
    assert_eq!((m.calls, m.frontier_calls), (1, 0));
}

#[test]
fn the_harness_reports_the_same_numbers_it_wrote() {
    let (h, ws) = open("same", mock("hi"), "mock-front");
    h.send("say hi");
    let written = last_line(&ws, 1);
    assert_eq!(h.metrics(), written);
}

#[test]
fn the_file_only_ever_grows_by_whole_json_lines() {
    let (h, ws) = open("lines", mock("hi"), "mock-mid");
    h.send("say hi");
    last_line(&ws, 1);
    h.send("and again");
    last_line(&ws, 2);
    let s = std::fs::read_to_string(ws.join(".backspace/metrics.jsonl")).unwrap();
    for l in s.lines() {
        let v: Value = serde_json::from_str(l).unwrap();
        assert!(v["calls"].is_u64());
    }
}
