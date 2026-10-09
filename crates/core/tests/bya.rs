//! Bring your agent: a thread whose route is a remote A2A agent sends each
//! message with `message/send`, keeps the agent's context id as the session,
//! and gets a trace like any other reply.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use backspace_core::chat::{Chats, Ctx, Route, RouteKind, Status};
use backspace_core::prefs::{Prefs, RemoteAgent};

/// A tiny A2A agent: answers each message with "You said: …", and records
/// the context ids it was sent.
fn agent() -> (String, Arc<Mutex<Vec<String>>>) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(vec![]));
    let s2 = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let mut buf = vec![0u8; 65536];
            let mut n = 0;
            let body = loop {
                n += s.read(&mut buf[n..]).unwrap();
                let t = String::from_utf8_lossy(&buf[..n]).to_string();
                if let Some(i) = t.find("\r\n\r\n") {
                    let len: usize = t[..i].lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap())).unwrap_or(0);
                    if n >= i + 4 + len {
                        break t[i + 4..].to_string();
                    }
                }
            };
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            let msg = &v["params"]["message"];
            s2.lock().unwrap().push(msg["contextId"].as_str().unwrap_or("-").to_string());
            let text = msg["parts"][0]["text"].as_str().unwrap_or("");
            let last = text.rsplit("\n\n").next().unwrap_or("");
            let out = serde_json::json!({"jsonrpc": "2.0", "id": v["id"], "result": {
                "kind": "task", "id": "t1", "contextId": "ctx-42", "status": {"state": "completed"},
                "artifacts": [{"parts": [{"kind": "text", "text": format!("You said: {last}")}]}]}})
            .to_string();
            let resp = format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{out}", out.len());
            s.write_all(resp.as_bytes()).unwrap();
        }
    });
    (url, seen)
}

fn wait_idle(chats: &Arc<Chats>, id: &str) {
    let start = Instant::now();
    while chats.is_busy(id) {
        assert!(start.elapsed() < Duration::from_secs(30), "never finished");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn chat_with_a_remote_a2a_agent() {
    let dir = std::env::temp_dir().join(format!("bs-bya-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (url, seen) = agent();
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let (tx, _rx) = async_channel::bounded(16);
    let chats = Chats::open(dir.join("chats"), rt.handle().clone(), tx);
    let mut prefs = Prefs::default();
    prefs.remote_agents.push(RemoteAgent { id: "workbot".into(), name: "WorkBot".into(), description: String::new(), url, token: String::new() });
    let ctx = Ctx { prefs, memory: None, agents: None, plan: false };

    let t = chats.create(Route { kind: RouteKind::A2a, provider: "workbot".into(), model: None });
    chats.send(&t.id, "file my taxi receipt", vec![], None, ctx.clone()).unwrap();
    wait_idle(&chats, &t.id);
    chats.send(&t.id, "and the hotel", vec![], None, ctx.clone()).unwrap();
    wait_idle(&chats, &t.id);

    let t = chats.thread(&t.id).unwrap();
    let replies: Vec<&str> = t.messages.iter().filter(|m| m.status == Status::Done && m.role == backspace_core::chat::Role::Assistant).map(|m| m.text.as_str()).collect();
    assert_eq!(replies, vec!["You said: file my taxi receipt", "You said: and the hotel"]);
    // The second message carries on the agent's own context.
    assert_eq!(*seen.lock().unwrap(), vec!["-".to_string(), "ctx-42".to_string()]);
    let tr = chats.trace(t.messages.last().unwrap().trace.as_deref().unwrap()).unwrap();
    assert_eq!(tr.spans[0].attrs["backspace.route"], "a2a");
    assert_eq!(tr.spans[1].name, "a2a task");
    let _ = std::fs::remove_dir_all(&dir);
}
