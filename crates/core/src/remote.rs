//! Follow and drive a harness on another machine.
//!
//! A machine shares its harness with `backspace-cli serve` (or the desktop
//! "Share this machine" setting): a small HTTP/1.1 JSON API behind a bearer
//! token. Another machine adds it by URL and token and gets the same live
//! state the local shell has. Plain HTTP: bind to localhost and reach it
//! through an SSH tunnel or a private network (Tailscale, WireGuard) rather
//! than exposing the port.
//!
//!   GET  /v1/hello                      {"app":"backspace","version":..}
//!   GET  /v1/state?since=N              long-poll: {"version":V,"state":ProjectState}
//!   POST /v1/send     {"text"}
//!   POST /v1/approve  {"id"}
//!   POST /v1/reject   {"id","feedback"}
//!   POST /v1/ticket   {"title","body"}  {"key"}
//!   POST /v1/files    {"agent"}         [FileEntry]
//!   POST /v1/file     {"path"}          {"text"}

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

use crate::files::FileEntry;
use crate::project::ProjectState;

/// What the server needs from a harness.
pub(crate) trait Api: Send + Sync + 'static {
    fn version(&self) -> watch::Receiver<u64>;
    fn snapshot(&self) -> ProjectState;
    fn send(&self, text: String);
    /// None approves; Some(feedback) rejects.
    fn resolve(&self, id: usize, feedback: Option<String>);
    fn file_ticket(&self, title: &str, body: &str) -> Result<String>;
    fn list_files(&self, agent: usize) -> Vec<FileEntry>;
    fn read_file(&self, path: &str) -> Result<String>;
}

/// A random 32-hex-digit token.
pub fn new_token() -> String {
    let mut bytes = [0u8; 16];
    let from_os = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut bytes))
        .is_ok();
    if !from_os {
        // Windows: std's RandomState is seeded by the OS.
        use std::hash::{BuildHasher, Hasher};
        for (i, chunk) in bytes.chunks_mut(8).enumerate() {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_usize(i);
            h.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos()),
            );
            chunk.copy_from_slice(&h.finish().to_le_bytes());
        }
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------- server

pub(crate) async fn serve(api: Arc<dyn Api>, addr: String, token: String) -> Result<()> {
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    let token = Arc::new(token);
    loop {
        let (stream, _) = listener.accept().await?;
        let (api, token) = (api.clone(), token.clone());
        tokio::spawn(async move {
            let _ = handle(stream, api, &token).await;
        });
    }
}

pub(crate) struct Request {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) query: String,
    pub(crate) auth: Option<String>,
    pub(crate) body: Vec<u8>,
}

pub(crate) async fn read_request(stream: &mut TcpStream) -> Result<Request> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            bail!("closed");
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > 16 * 1024 {
            bail!("headers too large");
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let method = first.next().unwrap_or("").to_string();
    let target = first.next().unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (mut len, mut auth) = (0usize, None);
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "content-length" => len = v.trim().parse().unwrap_or(0),
                "authorization" => auth = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }
    if len > 12 << 20 {
        bail!("body too large");
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < len {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            bail!("closed");
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(len);
    Ok(Request {
        method,
        path: path.to_string(),
        query: query.to_string(),
        auth,
        body,
    })
}

pub(crate) async fn respond(stream: &mut TcpStream, status: u16, body: &Value) -> Result<()> {
    let text = body.to_string();
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        text.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(text.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

/// Equal-length, branch-free comparison so timing does not leak the token.
pub(crate) fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

async fn handle(mut stream: TcpStream, api: Arc<dyn Api>, token: &str) -> Result<()> {
    let req = read_request(&mut stream).await?;
    if req.path == "/v1/hello" {
        return respond(
            &mut stream,
            200,
            &json!({"app": "backspace", "version": crate::update::VERSION}),
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
        match serde_json::from_slice(&req.body) {
            Ok(v) => v,
            Err(e) => return respond(&mut stream, 400, &json!({"error": e.to_string()})).await,
        }
    };
    let out: Result<Value> = match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/v1/state") => {
            let since: u64 = req
                .query
                .split('&')
                .find_map(|kv| kv.strip_prefix("since="))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let mut rx = api.version();
            let _ =
                tokio::time::timeout(Duration::from_secs(25), rx.wait_for(|v| *v > since)).await;
            let v = *rx.borrow();
            Ok(json!({"version": v, "state": api.snapshot()}))
        }
        ("POST", "/v1/send") => {
            let text = body["text"].as_str().unwrap_or("").to_string();
            if text.trim().is_empty() {
                Err(anyhow!("empty message"))
            } else {
                api.send(text);
                Ok(json!({}))
            }
        }
        ("POST", "/v1/approve") => match body["id"].as_u64() {
            Some(id) => {
                api.resolve(id as usize, None);
                Ok(json!({}))
            }
            None => Err(anyhow!("missing id")),
        },
        ("POST", "/v1/reject") => match (body["id"].as_u64(), body["feedback"].as_str()) {
            (Some(id), Some(fb)) if !fb.trim().is_empty() => {
                api.resolve(id as usize, Some(fb.to_string()));
                Ok(json!({}))
            }
            _ => Err(anyhow!("needs id and feedback")),
        },
        ("POST", "/v1/ticket") => api
            .file_ticket(
                body["title"].as_str().unwrap_or(""),
                body["body"].as_str().unwrap_or(""),
            )
            .map(|key| json!({ "key": key })),
        ("POST", "/v1/files") => Ok(json!(
            api.list_files(body["agent"].as_u64().unwrap_or(0) as usize)
        )),
        ("POST", "/v1/file") => api
            .read_file(body["path"].as_str().unwrap_or(""))
            .map(|t| json!({ "text": t })),
        _ => return respond(&mut stream, 404, &json!({"error": "no such endpoint"})).await,
    };
    match out {
        Ok(v) => respond(&mut stream, 200, &v).await,
        Err(e) => respond(&mut stream, 400, &json!({"error": e.to_string()})).await,
    }
}

// ---------------------------------------------------------------- client

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Link {
    Online,
    Connecting,
    Offline { error: String },
}

/// A remote harness, kept current by a long-poll loop.
pub struct Remote {
    pub name: String,
    pub url: String,
    token: String,
    http: reqwest::Client,
    rt: tokio::runtime::Handle,
    state: Mutex<Option<ProjectState>>,
    link: Mutex<Link>,
    poll: Mutex<Option<tokio::task::AbortHandle>>,
}

#[derive(Deserialize)]
struct StateMsg {
    version: u64,
    state: ProjectState,
}

impl Remote {
    pub fn connect(
        name: String,
        url: String,
        token: String,
        rt: tokio::runtime::Handle,
        notify: async_channel::Sender<()>,
    ) -> Arc<Remote> {
        let r = Arc::new(Remote {
            name,
            url: url.trim_end_matches('/').to_string(),
            token,
            http: reqwest::Client::new(),
            rt: rt.clone(),
            state: Mutex::new(None),
            link: Mutex::new(Link::Connecting),
            poll: Mutex::new(None),
        });
        let weak = Arc::downgrade(&r);
        let task = rt.spawn(async move {
            let mut since = 0u64;
            loop {
                let Some(r) = weak.upgrade() else { return };
                let res = r
                    .http
                    .get(format!("{}/v1/state?since={since}", r.url))
                    .bearer_auth(&r.token)
                    .timeout(Duration::from_secs(35))
                    .send()
                    .await;
                let res = match res {
                    Ok(resp) if resp.status() == reqwest::StatusCode::UNAUTHORIZED => {
                        Err(anyhow!("the token was refused"))
                    }
                    Ok(resp) => resp.error_for_status().map_err(anyhow::Error::from),
                    Err(e) => Err(anyhow!(short(&e))),
                };
                let msg = match res {
                    Ok(resp) => resp.json::<StateMsg>().await.map_err(anyhow::Error::from),
                    Err(e) => Err(e),
                };
                match msg {
                    Ok(m) => {
                        since = m.version;
                        *r.state.lock().unwrap() = Some(m.state);
                        *r.link.lock().unwrap() = Link::Online;
                        let _ = notify.try_send(());
                    }
                    Err(e) => {
                        let was = std::mem::replace(
                            &mut *r.link.lock().unwrap(),
                            Link::Offline {
                                error: e.to_string(),
                            },
                        );
                        if !matches!(was, Link::Offline { .. }) {
                            let _ = notify.try_send(());
                        }
                        since = 0;
                        drop(r);
                        tokio::time::sleep(Duration::from_secs(3)).await;
                    }
                }
            }
        });
        *r.poll.lock().unwrap() = Some(task.abort_handle());
        r
    }

    pub fn link(&self) -> Link {
        self.link.lock().unwrap().clone()
    }

    pub fn snapshot(&self) -> Option<ProjectState> {
        self.state.lock().unwrap().clone()
    }

    fn post(&self, path: &str, body: Value) -> impl std::future::Future<Output = Result<Value>> {
        let req = self
            .http
            .post(format!("{}{path}", self.url))
            .bearer_auth(&self.token)
            .timeout(Duration::from_secs(10))
            .json(&body);
        async move {
            let resp = req.send().await.map_err(|e| anyhow!(short(&e)))?;
            let status = resp.status();
            let v: Value = resp.json().await.unwrap_or(Value::Null);
            if !status.is_success() {
                bail!("{}", v["error"].as_str().unwrap_or(status.as_str()));
            }
            Ok(v)
        }
    }

    /// Fire and forget (the state poll shows the result).
    pub fn post_bg(&self, path: &'static str, body: Value) {
        let fut = self.post(path, body);
        self.rt.spawn(async move {
            let _ = fut.await;
        });
    }

    /// Wait for the answer; call from a UI thread, never from the runtime.
    pub fn post_wait(&self, path: &str, body: Value) -> Result<Value> {
        let fut = self.post(path, body);
        std::thread::scope(|s| s.spawn(|| self.rt.block_on(fut)).join())
            .map_err(|_| anyhow!("request panicked"))?
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        if let Some(t) = self.poll.lock().unwrap().take() {
            t.abort();
        }
    }
}

/// reqwest errors print the whole chain; keep the useful end.
fn short(e: &reqwest::Error) -> String {
    let mut s = e.to_string();
    let mut src = std::error::Error::source(e);
    while let Some(inner) = src {
        s = inner.to_string();
        src = inner.source();
    }
    if e.is_timeout() {
        "timed out".into()
    } else {
        s
    }
}

/// Check a URL and token before saving a machine.
pub async fn probe(url: &str, token: &str) -> Result<()> {
    let url = url.trim_end_matches('/');
    let http = reqwest::Client::new();
    let hello: Value = http
        .get(format!("{url}/v1/hello"))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| anyhow!("can't reach {url}: {}", short(&e)))?
        .json()
        .await
        .map_err(|_| anyhow!("{url} is not a Backspace harness"))?;
    if hello["app"] != "backspace" {
        bail!("{url} is not a Backspace harness");
    }
    let resp = http
        .get(format!("{url}/v1/state?since=0"))
        .bearer_auth(token)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| anyhow!(short(&e)))?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        bail!("{url} refused the token");
    }
    resp.error_for_status().map_err(|e| anyhow!(short(&e)))?;
    Ok(())
}
