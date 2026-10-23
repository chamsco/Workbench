//! This machine's model server: one OpenAI-compatible URL for every model it
//! can reach, the way Atomic Chat serves `localhost:1337`.
//!
//! Behind `http://127.0.0.1:7422/v1` sit this machine's runtimes (Ollama, LM
//! Studio, llama.cpp, MLX, model_server.rs), the servers on the local network
//! the user added, and every connected machine's own models through its share
//! link. A request names a model; the gateway finds who serves it, rewrites
//! the model id to that runtime's own and streams the answer back untouched.
//! Clients never need to know which engine, or which machine, answered.
//!
//!   GET  /v1/hello                  {"app":"backspace","kind":"models",..}
//!   GET  /v1/models                 OpenAI model list
//!   POST /v1/chat/completions       proxied, streamed
//!   POST /v1/completions | /v1/embeddings | /v1/responses
//!
//! Who sees what:
//! - A tool on this machine sees everything, keyless (loopback, no Origin).
//! - Another machine (only once `allow_lan` is on) needs the token and sees
//!   this machine's own runtimes only, never what it reaches in turn, so two
//!   servers that list each other cannot loop a request between them.
//! - A browser page needs the token too: anything carrying an `Origin` is a
//!   page, and a page has no business driving this machine's GPU.
//!
//! The share link (remote.rs) and the phone link (companion.rs) serve the same
//! routes under `/v1/llm`, behind their own tokens and with the same
//! this-machine-only rule, so a connected machine or the phone can chat with
//! the models on the desk.
//!
//! [`discover`] finds servers on the local network: Backspace model servers
//! answer a UDP broadcast, and the usual runtime ports (Ollama, LM Studio,
//! Atomic Chat, llama.cpp) are swept on this machine's /24.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream, UdpSocket};

use crate::model_server::{
    detect, host_of, is_loopback, openai_base, remote_runtimes, LocalRuntime,
};
use crate::prefs::Prefs;
use crate::remote::{read_request, respond, same, Request};

/// The router id Chat and the small jobs use for this server
/// (`router:backspace` in the provider list, `backspace:<model>` for a job).
pub const ROUTER_ID: &str = "backspace";

/// Backspace model servers answer discovery on this UDP port.
pub const DISCOVERY_PORT: u16 = 7422;
const ASK: &[u8] = b"backspace-models?";

/// After this a probed catalog is refreshed in the background. A model pulled
/// a moment ago shows up sooner anyway: an unknown id re-probes once before 404.
const FRESH: Duration = Duration::from_secs(15);

/// The runtime ports swept on the local network, with what usually listens.
const LAN_PORTS: &[(u16, &str)] = &[
    (11434, "Ollama"),
    (1234, "LM Studio"),
    (1337, "Atomic Chat"),
    (8080, "llama.cpp"),
];

/// The OpenAI routes a request may be forwarded to; nothing else upstream is
/// reachable through here.
const ROUTES: &[&str] = &[
    "/chat/completions",
    "/completions",
    "/embeddings",
    "/responses",
];

// ---------------------------------------------------------------- catalog

/// One model the gateway serves, and who actually runs it.
#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct Served {
    /// What clients ask for: the runtime's own id on this machine,
    /// `<host>/<id>` for another machine's.
    pub id: String,
    /// What the runtime calls it.
    pub model: String,
    /// The runtime's OpenAI-compatible root.
    pub base: String,
    #[serde(skip)]
    pub key: String,
    pub runtime: String,
    /// None: this machine.
    pub host: Option<String>,
}

/// Every model the running runtimes list, first one wins on a clash, so this
/// machine's own (listed first) shadow nothing and are never shadowed.
pub fn served(runtimes: &[LocalRuntime]) -> Vec<Served> {
    let mut out: Vec<Served> = Vec::new();
    for rt in runtimes.iter().filter(|r| r.running) {
        let base = openai_base(rt.kind, &rt.base_url);
        for m in &rt.models {
            let id = match &rt.host {
                None => m.id.clone(),
                Some(h) => format!("{h}/{}", m.id),
            };
            if out.iter().any(|s| s.id == id) {
                continue;
            }
            out.push(Served {
                id,
                model: m.id.clone(),
                base: base.clone(),
                key: rt.key.clone(),
                runtime: rt.id.clone(),
                host: rt.host.clone(),
            });
        }
    }
    out
}

/// The model a client asked for: its exact id, else the one runtime model of
/// that name (`qwen3:1.7b` when only the studio box has it).
pub fn find<'a>(all: &'a [Served], id: &str, local_only: bool) -> Option<&'a Served> {
    let ok = |s: &&Served| !local_only || s.host.is_none();
    all.iter()
        .filter(ok)
        .find(|s| s.id == id)
        .or_else(|| all.iter().filter(ok).find(|s| s.model == id))
}

static CACHE: Mutex<Option<(Instant, Vec<Served>)>> = Mutex::new(None);
static REFRESHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// What every reachable runtime serves. Only the very first call waits for
/// the probes; after that a stale answer is returned at once while one
/// background refresh runs, because another server probing this one gives up
/// after a few seconds and a cold probe of every runtime can take that long.
/// `force` waits for a fresh probe (a model nobody listed yet). Blocking: call
/// it off the async threads.
pub fn catalog(force: bool) -> Vec<Served> {
    if !force {
        if let Some((at, v)) = CACHE.lock().unwrap().as_ref() {
            if at.elapsed() >= FRESH && !REFRESHING.swap(true, std::sync::atomic::Ordering::SeqCst)
            {
                std::thread::spawn(|| {
                    probe_all();
                    REFRESHING.store(false, std::sync::atomic::Ordering::SeqCst);
                });
            }
            return v.clone();
        }
    }
    probe_all()
}

fn probe_all() -> Vec<Served> {
    let mut prefs = Prefs::load();
    // A router pointed at this very server would have it probe itself.
    let me = prefs.model_serve.port();
    prefs
        .routers
        .retain(|r| !(is_loopback(&r.base_url) && port_of(&r.base_url) == Some(me)));
    let (local, remote) = std::thread::scope(|s| {
        let l = s.spawn(|| detect(&prefs, &[]));
        let r = s.spawn(|| remote_runtimes(&prefs));
        (l.join().unwrap_or_default(), r.join().unwrap_or_default())
    });
    let all: Vec<LocalRuntime> = local.into_iter().chain(remote).collect();
    let v = served(&all);
    *CACHE.lock().unwrap() = Some((Instant::now(), v.clone()));
    v
}

fn port_of(url: &str) -> Option<u16> {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let authority = rest.split(['/', '?', '#']).next()?;
    authority.rsplit_once(':')?.1.parse().ok()
}

// ---------------------------------------------------------------- server

pub async fn serve(addr: String, token: String) -> Result<()> {
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    let token = std::sync::Arc::new(token);
    // Warm the catalog, so the first client is not the one that waits.
    tokio::task::spawn_blocking(|| catalog(false));
    loop {
        let (stream, peer) = listener.accept().await?;
        let token = token.clone();
        tokio::spawn(async move {
            let _ = handle(stream, peer.ip().is_loopback(), &token).await;
        });
    }
}

async fn handle(mut stream: TcpStream, loopback: bool, token: &str) -> Result<()> {
    let req = read_request(&mut stream).await?;
    if req.path == "/v1/hello" {
        return respond(&mut stream, 200, &hello()).await;
    }
    let authed = req
        .auth
        .as_deref()
        .and_then(|a| a.strip_prefix("Bearer "))
        .is_some_and(|t| same(t, token));
    if (!loopback || req.origin.is_some()) && !authed {
        return respond(
            &mut stream,
            401,
            &error("this server wants its token as a Bearer key", "auth"),
        )
        .await;
    }
    let Some(rest) = req.path.strip_prefix("/v1") else {
        return respond(&mut stream, 404, &error("not found", "not_found")).await;
    };
    let rest = rest.to_string();
    handle_llm(&mut stream, &req, &rest, !loopback).await
}

fn hello() -> Value {
    json!({"app": "backspace", "kind": "models", "name": machine_name(), "version": crate::update::VERSION})
}

/// The OpenAI routes, for this server and for the share and phone links
/// (`/v1/llm` + `rest`). The caller has already checked who is asking;
/// `local_only` keeps the answer to this machine's own runtimes.
pub(crate) async fn handle_llm(
    stream: &mut TcpStream,
    req: &Request,
    rest: &str,
    local_only: bool,
) -> Result<()> {
    if req.method == "GET" && (rest == "/models" || rest == "/models/") {
        let all = tokio::task::spawn_blocking(|| catalog(false)).await?;
        let data: Vec<Value> = all
            .iter()
            .filter(|s| !local_only || s.host.is_none())
            .map(|s| json!({"id": s.id, "object": "model", "owned_by": s.runtime, "host": s.host}))
            .collect();
        return respond(stream, 200, &json!({"object": "list", "data": data})).await;
    }
    if req.method != "POST" || !ROUTES.contains(&rest) {
        return respond(
            stream,
            404,
            &error(&format!("no route {} {rest}", req.method), "not_found"),
        )
        .await;
    }
    let body: Value = match serde_json::from_slice(&req.body) {
        Ok(v) => v,
        Err(e) => {
            return respond(
                stream,
                400,
                &error(
                    &format!("the body is not JSON: {e}"),
                    "invalid_request_error",
                ),
            )
            .await
        }
    };
    let Some(want) = body["model"].as_str().map(str::to_string) else {
        return respond(
            stream,
            400,
            &error("say which model", "invalid_request_error"),
        )
        .await;
    };
    let w = want.clone();
    let hit = tokio::task::spawn_blocking(move || {
        find(&catalog(false), &w, local_only)
            .cloned()
            .or_else(|| find(&catalog(true), &w, local_only).cloned())
    })
    .await?;
    let Some(s) = hit else {
        return respond(
            stream,
            404,
            &error(&format!("no runtime serves `{want}`"), "model_not_found"),
        )
        .await;
    };
    // A downloaded file: Brainpowr Engine loads it now if it is not in memory,
    // and the request goes to the session it answers on.
    let mut s = s;
    if s.runtime == crate::model_engine::RUNTIME_ID {
        let id = s.model.clone();
        let loaded = tokio::task::spawn_blocking(move || crate::model_engine::engine().ensure(&id)).await?;
        match loaded {
            Ok(session) => s.base = session.base_url,
            Err(e) => {
                return respond(stream, 503, &error(&format!("{} could not be loaded: {e:#}", s.model), "model_load_failed")).await;
            }
        }
        let res = forward(stream, &s, rest, body).await;
        // The idle timer counts from the end of an answer, not its start.
        crate::model_engine::engine().touch(&s.model);
        return res;
    }
    forward(stream, &s, rest, body).await
}

/// Send the request to the runtime that serves the model and copy its answer
/// back as it arrives, so a streamed reply streams.
async fn forward(stream: &mut TcpStream, s: &Served, path: &str, mut body: Value) -> Result<()> {
    body["model"] = json!(s.model);
    // no_proxy: an office proxy must not swallow a request to this machine or
    // the next desk. No overall timeout: a long answer is a long answer.
    let http = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .build()?;
    let mut rb = http.post(format!("{}{path}", s.base)).json(&body);
    if !s.key.is_empty() {
        rb = rb.bearer_auth(&s.key);
    }
    let mut resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            let at = s.host.as_deref().unwrap_or("this machine");
            return respond(
                stream,
                502,
                &error(
                    &format!("{} on {at} did not answer: {e}", s.runtime),
                    "upstream",
                ),
            )
            .await;
        }
    };
    let status = resp.status();
    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let head = format!(
        "HTTP/1.1 {} {}\r\ncontent-type: {ctype}\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n",
        status.as_u16(),
        status.canonical_reason().unwrap_or("Error"),
    );
    stream.write_all(head.as_bytes()).await?;
    // A client that hangs up ends this loop with an error, which drops the
    // upstream request and stops the generation.
    while let Some(chunk) = resp.chunk().await? {
        stream.write_all(&chunk).await?;
        stream.flush().await?;
    }
    Ok(())
}

/// An error the OpenAI SDKs print properly.
fn error(message: &str, kind: &str) -> Value {
    json!({"error": {"message": message, "type": kind}})
}

pub fn machine_name() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Backspace".into())
}

// ---------------------------------------------------------------- discovery

/// Answer "who serves models?" broadcasts with this server's name and port.
/// Runs only while the server is open to the network.
pub async fn answer_discovery(port: u16) -> Result<()> {
    let sock = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT))
        .await
        .with_context(|| format!("binding udp {DISCOVERY_PORT}"))?;
    let mut buf = [0u8; 64];
    loop {
        let (n, from) = sock.recv_from(&mut buf).await?;
        if &buf[..n] == ASK {
            let reply =
                json!({"app": "backspace", "kind": "models", "name": machine_name(), "port": port});
            let _ = sock.send_to(reply.to_string().as_bytes(), from).await;
        }
    }
}

/// A model server found on the local network.
#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct Found {
    pub name: String,
    /// OpenAI form, ready to save as a [`crate::prefs::LanServer`].
    pub base_url: String,
    /// "Backspace", "Ollama", "LM Studio"...
    pub kind: String,
    /// It answered 401: add it with its token.
    pub needs_token: bool,
    pub models: Vec<String>,
    /// Already in Settings.
    pub added: bool,
}

/// Look for model servers on the local network: a UDP broadcast for
/// Backspace servers, then a sweep of the usual runtime ports on this
/// machine's /24. Blocking, a few seconds at most; never this machine itself.
pub fn discover(prefs: &Prefs) -> Vec<Found> {
    let Some(std::net::IpAddr::V4(me)) = crate::companion::lan_ip() else {
        return vec![];
    };
    let hits = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map(|rt| rt.block_on(async { tokio::join!(broadcast(me), sweep(me)) }))
        .unwrap_or_default();
    let (beacons, open) = hits;
    let mut found: Vec<Found> = beacons;
    // Probe each open port for a model list; a port that answers with
    // anything else (8080 is every dev server's) is not a model server.
    let probed: Vec<Found> = std::thread::scope(|s| {
        let hs: Vec<_> = open
            .into_iter()
            .filter(|(ip, port, _)| {
                !found
                    .iter()
                    .any(|f| f.base_url.contains(&format!("{ip}:{port}")))
            })
            .map(|(ip, port, kind)| {
                s.spawn(move || {
                    let base = format!("http://{ip}:{port}/v1");
                    probe_found(&format!("{kind} on {ip}"), &base, kind)
                })
            })
            .collect();
        hs.into_iter()
            .filter_map(|h| h.join().ok().flatten())
            .collect()
    });
    found.extend(probed);
    for f in &mut found {
        f.added = prefs
            .lan_models
            .iter()
            .any(|l| l.base_url.trim_end_matches('/') == f.base_url.trim_end_matches('/'));
    }
    found
}

fn probe_found(name: &str, base: &str, kind: &str) -> Option<Found> {
    match crate::model_server::get_json(&format!("{base}/models"), None) {
        Ok(v) => {
            let data = v["data"].as_array()?;
            Some(Found {
                name: name.into(),
                base_url: base.into(),
                kind: kind.into(),
                needs_token: false,
                models: data
                    .iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_string))
                    .collect(),
                added: false,
            })
        }
        Err(e) if e.contains("401") || e.contains("403") => Some(Found {
            name: name.into(),
            base_url: base.into(),
            kind: kind.into(),
            needs_token: true,
            models: vec![],
            added: false,
        }),
        Err(_) => None,
    }
}

/// Backspace servers that answer the broadcast within a second.
async fn broadcast(me: std::net::Ipv4Addr) -> Vec<Found> {
    let Ok(sock) = UdpSocket::bind("0.0.0.0:0").await else {
        return vec![];
    };
    let _ = sock.set_broadcast(true);
    let o = me.octets();
    for to in [
        "255.255.255.255".to_string(),
        format!("{}.{}.{}.255", o[0], o[1], o[2]),
    ] {
        let _ = sock.send_to(ASK, (to.as_str(), DISCOVERY_PORT)).await;
    }
    let mut out: Vec<Found> = Vec::new();
    let mut buf = [0u8; 512];
    let until = tokio::time::Instant::now() + Duration::from_millis(1200);
    while let Ok(Ok((n, from))) = tokio::time::timeout_at(until, sock.recv_from(&mut buf)).await {
        if from.ip() == std::net::IpAddr::V4(me) {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(&buf[..n]) else {
            continue;
        };
        if v["app"] != "backspace" || v["kind"] != "models" {
            continue;
        }
        let port = v["port"].as_u64().unwrap_or(DISCOVERY_PORT as u64);
        let base = format!("http://{}:{port}/v1", from.ip());
        if out.iter().any(|f| f.base_url == base) {
            continue;
        }
        out.push(Found {
            name: v["name"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| host_of(&base)),
            base_url: base,
            kind: "Backspace".into(),
            needs_token: true,
            models: vec![],
            added: false,
        });
    }
    out
}

/// Every (ip, port) on the /24 that accepts a connection on a runtime port.
/// 96 at a time: macOS gives an app 256 file descriptors.
async fn sweep(me: std::net::Ipv4Addr) -> Vec<(std::net::Ipv4Addr, u16, &'static str)> {
    use futures::StreamExt;
    let o = me.octets();
    let targets: Vec<(std::net::Ipv4Addr, u16, &'static str)> = (1..=254u8)
        .map(|d| std::net::Ipv4Addr::new(o[0], o[1], o[2], d))
        .filter(|ip| *ip != me)
        .flat_map(|ip| LAN_PORTS.iter().map(move |(p, k)| (ip, *p, *k)))
        .collect();
    futures::stream::iter(targets)
        .map(|(ip, port, kind)| async move {
            let ok =
                tokio::time::timeout(Duration::from_millis(300), TcpStream::connect((ip, port)))
                    .await
                    .is_ok_and(|r| r.is_ok());
            ok.then_some((ip, port, kind))
        })
        .buffer_unordered(96)
        .filter_map(|x| async move { x })
        .collect()
        .await
}

/// Check a server before saving it: it must list models with that token.
pub fn check_lan(base_url: &str, token: &str) -> Result<Vec<String>> {
    let v = crate::model_server::get_json(
        &format!("{}/models", base_url.trim_end_matches('/')),
        Some(token),
    )
    .map_err(|e| anyhow!("{base_url}: {e}"))?;
    let data = v["data"]
        .as_array()
        .ok_or_else(|| anyhow!("{base_url} answered, but not with a model list"))?;
    Ok(data
        .iter()
        .filter_map(|m| m["id"].as_str().map(str::to_string))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_server::{LocalKind, LocalModel};

    fn rt(
        id: &str,
        kind: LocalKind,
        url: &str,
        host: Option<&str>,
        models: &[&str],
    ) -> LocalRuntime {
        LocalRuntime {
            id: id.into(),
            name: id.into(),
            kind,
            base_url: url.into(),
            running: true,
            models: models
                .iter()
                .map(|m| LocalModel {
                    id: (*m).into(),
                    runtime: id.into(),
                    ..Default::default()
                })
                .collect(),
            host: host.map(str::to_string),
            key: if host.is_some() {
                "tok".into()
            } else {
                String::new()
            },
            ..Default::default()
        }
    }

    #[test]
    fn served_prefixes_other_machines_and_keeps_the_first_of_a_clash() {
        let mut stopped = rt(
            "llamacpp",
            LocalKind::LlamaCpp,
            "http://127.0.0.1:8080/v1",
            None,
            &["x"],
        );
        stopped.running = false;
        let all = served(&[
            rt(
                "ollama",
                LocalKind::Ollama,
                "http://localhost:11434",
                None,
                &["qwen3:1.7b", "gemma3:4b"],
            ),
            rt(
                "lmstudio",
                LocalKind::LmStudio,
                "http://127.0.0.1:1234/v1",
                None,
                &["qwen3:1.7b"],
            ),
            stopped,
            rt(
                "lan:studio",
                LocalKind::Custom,
                "http://192.168.1.20:11434/v1",
                Some("studio"),
                &["qwen3:32b"],
            ),
        ]);
        let ids: Vec<&str> = all.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["qwen3:1.7b", "gemma3:4b", "studio/qwen3:32b"]);
        // Ollama is reached on its OpenAI root.
        assert_eq!(all[0].base, "http://localhost:11434/v1");
        assert_eq!(all[0].runtime, "ollama");
        assert_eq!(all[2].model, "qwen3:32b");
        assert_eq!(all[2].key, "tok");
    }

    #[test]
    fn find_by_id_then_by_runtime_name_and_never_past_this_machine_for_others() {
        let all = served(&[
            rt(
                "ollama",
                LocalKind::Ollama,
                "http://localhost:11434",
                None,
                &["qwen3:1.7b"],
            ),
            rt(
                "machine:studio",
                LocalKind::Custom,
                "http://10.0.0.5:7420/v1/llm",
                Some("studio"),
                &["qwen3:32b"],
            ),
        ]);
        assert_eq!(
            find(&all, "studio/qwen3:32b", false).unwrap().model,
            "qwen3:32b"
        );
        assert_eq!(
            find(&all, "qwen3:32b", false).unwrap().id,
            "studio/qwen3:32b"
        );
        assert_eq!(find(&all, "qwen3:1.7b", true).unwrap().runtime, "ollama");
        // Another machine asking only gets this machine's own models.
        assert!(find(&all, "studio/qwen3:32b", true).is_none());
        assert!(find(&all, "qwen3:32b", true).is_none());
        assert!(find(&all, "nope", false).is_none());
    }

    #[test]
    fn port_of_reads_the_authority() {
        assert_eq!(port_of("http://127.0.0.1:7422/v1"), Some(7422));
        assert_eq!(port_of("http://localhost/v1"), None);
    }

    /// The whole path, on loopback: a fake runtime streams two SSE chunks and
    /// the gateway hands them through with the model id rewritten upstream.
    #[test]
    fn forward_streams_the_answer_and_rewrites_the_model() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            use tokio::io::AsyncReadExt;
            let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let up_addr = upstream.local_addr().unwrap();
            let seen = tokio::spawn(async move {
                let (mut s, _) = upstream.accept().await.unwrap();
                let req = read_request(&mut s).await.unwrap();
                s.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\ndata: {\"a\":1}\n\ndata: [DONE]\n\n").await.unwrap();
                (req.path, serde_json::from_slice::<Value>(&req.body).unwrap())
            });
            let gw = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let gw_addr = gw.local_addr().unwrap();
            let served = Served {
                id: "studio/m".into(),
                model: "m".into(),
                base: format!("http://{up_addr}/v1"),
                key: String::new(),
                runtime: "lan:studio".into(),
                host: Some("studio".into()),
            };
            tokio::spawn(async move {
                let (mut s, _) = gw.accept().await.unwrap();
                let req = read_request(&mut s).await.unwrap();
                let body: Value = serde_json::from_slice(&req.body).unwrap();
                forward(&mut s, &served, "/chat/completions", body).await.unwrap();
            });
            let mut c = TcpStream::connect(gw_addr).await.unwrap();
            let body = r#"{"model":"studio/m","stream":true,"messages":[]}"#;
            c.write_all(format!("POST /v1/chat/completions HTTP/1.1\r\ncontent-length: {}\r\n\r\n{body}", body.len()).as_bytes())
                .await
                .unwrap();
            let mut out = String::new();
            c.read_to_string(&mut out).await.unwrap();
            assert!(out.starts_with("HTTP/1.1 200 OK\r\ncontent-type: text/event-stream"), "{out}");
            assert!(out.ends_with("data: {\"a\":1}\n\ndata: [DONE]\n\n"), "{out}");
            let (path, sent) = seen.await.unwrap();
            assert_eq!(path, "/v1/chat/completions");
            assert_eq!(sent["model"], "m");
            assert_eq!(sent["stream"], true);
        });
    }
}
