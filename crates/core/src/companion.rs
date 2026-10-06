//! The phone companion's link to this machine: a small HTTP API on the
//! local network, guarded by a token the phone gets by scanning a QR code
//! in Settings. Plan and protocol: docs/companion.md.
//!
//! It serves what a phone needs away from the desk: the chat threads (read,
//! send, react, stop), Memory (read, add), and the open project's state with
//! its approvals (approve, reject). The mobile app itself comes later; this
//! is the contract it will be built against.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};

use crate::chat::{Route, Scope};
use crate::fleet::Fleet;
use crate::remote::{read_request, respond, same};

/// Protocol version the phone checks on /v1/hello.
pub const PROTOCOL: u32 = 1;

/// The machine's address on the local network, if it has one. Connecting a
/// UDP socket sends nothing; it only picks the outgoing interface.
pub fn lan_ip() -> Option<std::net::IpAddr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.168.0.1:9").or_else(|_| s.connect("10.0.0.1:9")).ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

/// What the QR code holds: `backspace://pair?v=1&url=...&token=...&name=...`.
pub fn pair_link(url: &str, token: &str, name: &str) -> String {
    let enc = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    format!("backspace://pair?v={PROTOCOL}&url={}&token={}&name={}", enc(url), enc(token), enc(name))
}

pub(crate) async fn serve(fleet: std::sync::Weak<Fleet>, addr: String, token: String) -> Result<()> {
    let listener = TcpListener::bind(&addr).await.map_err(|e| anyhow!("binding {addr}: {e}"))?;
    let token = Arc::new(token);
    loop {
        let (stream, _) = listener.accept().await?;
        let (fleet, token) = (fleet.clone(), token.clone());
        tokio::spawn(async move {
            let _ = handle(stream, fleet, &token).await;
        });
    }
}

async fn handle(mut stream: TcpStream, fleet: std::sync::Weak<Fleet>, token: &str) -> Result<()> {
    let req = read_request(&mut stream).await?;
    let Some(f) = fleet.upgrade() else {
        return respond(&mut stream, 503, &json!({"error": "shutting down"})).await;
    };
    if req.path == "/v1/hello" {
        let name = std::env::var("HOSTNAME").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "Backspace".into());
        return respond(
            &mut stream,
            200,
            &json!({"app": "backspace", "kind": "companion", "protocol": PROTOCOL, "version": crate::update::VERSION, "name": name}),
        )
        .await;
    }
    let authed = req
        .auth
        .as_deref()
        .and_then(|a| a.strip_prefix("Bearer "))
        .is_some_and(|t| same(t, token));
    if !authed {
        return respond(&mut stream, 401, &json!({"error": "bad token"})).await;
    }
    let body: Value = if req.body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&req.body).unwrap_or(Value::Null)
    };
    // Blocking work (file IO, the harness's locks) off the async threads.
    let (method, path, query) = (req.method.clone(), req.path.clone(), req.query.clone());
    let f2 = f.clone();
    let out: Result<Value> = tokio::task::spawn_blocking(move || -> Result<Value> {
        let f = f2;
        let q = |k: &str| {
            query
                .split('&')
                .find_map(|kv| kv.strip_prefix(&format!("{k}=")))
                .map(|v| v.to_string())
        };
        let s = |k: &str| body[k].as_str().unwrap_or("").to_string();
        let chats = f.chats();
        Ok(match (method.as_str(), path.as_str()) {
            ("GET", "/v1/chats") => json!(chats.list().into_iter().filter(|t| t.app.is_none()).collect::<Vec<_>>()),
            ("GET", "/v1/chat") => {
                let id = q("id").unwrap_or_default();
                json!(chats.thread(&id).filter(|t| t.app.is_none()).ok_or_else(|| anyhow!("no such chat"))?)
            }
            ("POST", "/v1/chat/new") => {
                let route: Route = serde_json::from_value(body["route"].clone())
                    .ok()
                    .or_else(|| f.prefs().default_route)
                    .ok_or_else(|| anyhow!("pick a model"))?;
                let project = body["project"].as_str().map(String::from);
                json!(chats.create_in(route, Scope { project, ..Default::default() }))
            }
            ("POST", "/v1/chat/send") => {
                let text = s("text");
                if text.trim().is_empty() {
                    anyhow::bail!("empty message");
                }
                let reply_to = body["reply_to"].as_str().map(String::from);
                chats.send(&s("id"), &text, vec![], reply_to, f.chat_ctx())?;
                json!({"ok": true})
            }
            ("POST", "/v1/chat/react") => {
                chats.react(&s("id"), &s("msg"), &s("emoji"))?;
                json!({"ok": true})
            }
            ("POST", "/v1/chat/stop") => {
                chats.stop(&s("id"));
                json!({"ok": true})
            }
            ("GET", "/v1/routes") => json!(f
                .harnesses()
                .into_iter()
                .filter(|h| h.enabled && h.chat)
                .map(|h| json!({"id": h.id, "name": h.name, "kind": h.kind, "models": h.models}))
                .collect::<Vec<_>>()),
            ("GET", "/v1/memory") => json!(f.memory().list()),
            ("POST", "/v1/memory") => json!(f.memory().add(&s("text"), body["project"].as_str().map(String::from), "phone")?),
            ("GET", "/v1/project") => match f.local() {
                Some(h) => {
                    let st = h.snapshot();
                    json!({"open": true, "workspace": h.root().display().to_string(), "state": st})
                }
                None => json!({"open": false}),
            },
            ("POST", "/v1/approve") => {
                let h = f.local().ok_or_else(|| anyhow!("no project is open"))?;
                h.approve(body["id"].as_u64().ok_or_else(|| anyhow!("id"))? as usize);
                json!({"ok": true})
            }
            ("POST", "/v1/reject") => {
                let h = f.local().ok_or_else(|| anyhow!("no project is open"))?;
                h.reject(body["id"].as_u64().ok_or_else(|| anyhow!("id"))? as usize, s("feedback"));
                json!({"ok": true})
            }
            _ => anyhow::bail!("not found: {method} {path}"),
        })
    })
    .await
    .map_err(|e| anyhow!("{e}"))?;
    match out {
        Ok(v) => respond(&mut stream, 200, &v).await,
        Err(e) => {
            let msg = e.to_string();
            let code = if msg.starts_with("not found") { 404 } else { 400 };
            respond(&mut stream, code, &json!({"error": msg})).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_link_escapes() {
        let l = pair_link("http://192.168.1.4:7421", "a b&c", "Sam's Mac");
        assert_eq!(
            l,
            "backspace://pair?v=1&url=http%3A%2F%2F192.168.1.4%3A7421&token=a%20b%26c&name=Sam%27s%20Mac"
        );
    }
}
