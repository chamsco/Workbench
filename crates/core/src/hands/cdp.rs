//! The agents' Chrome: one browser with its own profile under
//! `<data>/hands/chrome`, started once and left running so a sign-in made
//! there lasts, and a Chrome DevTools Protocol client for it.
//!
//! Every `backspace mcp` process is short-lived (one CLI turn), so nothing
//! here holds state in memory: the port is read from Chrome's own
//! `DevToolsActivePort` file and each agent's tab id from its folder.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// Where Chrome is: `prefs.hands.chrome`, then BACKSPACE_CHROME, then the
/// usual install paths of Chrome, Edge, Brave and Chromium.
pub fn find_chrome(pref: &str) -> Option<PathBuf> {
    let env = std::env::var("BACKSPACE_CHROME").unwrap_or_default();
    for p in [pref, env.as_str()] {
        if !p.trim().is_empty() && Path::new(p.trim()).exists() {
            return Some(PathBuf::from(p.trim()));
        }
    }
    let mut paths: Vec<PathBuf> = vec![];
    if cfg!(target_os = "macos") {
        for app in ["Google Chrome", "Google Chrome Canary", "Microsoft Edge", "Brave Browser", "Chromium"] {
            paths.push(format!("/Applications/{app}.app/Contents/MacOS/{app}").into());
            if let Some(h) = std::env::var_os("HOME") {
                paths.push(PathBuf::from(h).join(format!("Applications/{app}.app/Contents/MacOS/{app}")));
            }
        }
    } else if cfg!(windows) {
        for root in ["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"] {
            if let Some(r) = std::env::var_os(root) {
                let r = PathBuf::from(r);
                paths.push(r.join("Google/Chrome/Application/chrome.exe"));
                paths.push(r.join("Microsoft/Edge/Application/msedge.exe"));
                paths.push(r.join("BraveSoftware/Brave-Browser/Application/brave.exe"));
            }
        }
    } else {
        let path = std::env::var_os("PATH").unwrap_or_default();
        for name in ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "microsoft-edge", "brave-browser"] {
            paths.extend(std::env::split_paths(&path).map(|d| d.join(name)));
        }
    }
    paths.into_iter().find(|p| p.exists())
}

/// The browser's DevTools websocket, starting Chrome if it isn't running.
pub fn ensure(data: &Path, chrome_pref: &str, headless: bool) -> Result<String> {
    let profile = data.join("hands").join("chrome");
    if let Some(ws) = running(&profile) {
        return Ok(ws);
    }
    let exe = find_chrome(chrome_pref).ok_or_else(|| {
        anyhow!("no Chrome, Edge, Brave or Chromium found; install one or set its path in Settings → Agents → Browser")
    })?;
    std::fs::create_dir_all(&profile)?;
    let _ = std::fs::remove_file(profile.join("DevToolsActivePort"));
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("--remote-debugging-port=0")
        .arg(format!("--user-data-dir={}", profile.display()))
        .args(["--no-first-run", "--no-default-browser-check", "--window-size=1180,860"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if headless {
        cmd.arg("--headless=new");
    }
    // Chrome must outlive this process (one CLI turn) and the CLI's own
    // process group, so the next turn finds the same tabs and sign-ins.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn().with_context(|| format!("starting {}", exe.display()))?;
    let until = Instant::now() + Duration::from_secs(20);
    while Instant::now() < until {
        if let Some(ws) = running(&profile) {
            return Ok(ws);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("{} started but never opened its DevTools port", exe.display())
}

/// The agents' Chrome's websocket if it is running; never starts it.
pub fn running_ws(data: &Path) -> Option<String> {
    running(&data.join("hands").join("chrome"))
}

/// The websocket of a Chrome already running on this profile, if it answers.
fn running(profile: &Path) -> Option<String> {
    let f = std::fs::read_to_string(profile.join("DevToolsActivePort")).ok()?;
    let mut lines = f.lines();
    let port: u16 = lines.next()?.trim().parse().ok()?;
    let path = lines.next()?.trim().to_string();
    // A stale file from a Chrome that quit: nothing listens on the port.
    let mut s = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(300)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    write!(s, "GET /json/version HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n").ok()?;
    // Chrome may hold the connection open after the reply: read what came.
    let mut buf = vec![0u8; 8192];
    let mut body = Vec::new();
    while let Ok(n) = s.read(&mut buf) {
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
        if body.windows(20).any(|w| w == b"webSocketDebuggerUrl") {
            return Some(format!("ws://127.0.0.1:{port}{path}"));
        }
    }
    None
}

/// A DevTools connection to the browser; page calls go through a flat
/// session (`attach`).
pub struct Cdp {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
    next: u64,
    pub session: Option<String>,
}

impl Cdp {
    pub fn connect(url: &str) -> Result<Cdp> {
        let (ws, _) = tungstenite::connect(url).context("connecting to Chrome's DevTools")?;
        if let MaybeTlsStream::Plain(s) = ws.get_ref() {
            s.set_read_timeout(Some(Duration::from_secs(30)))?;
        }
        Ok(Cdp { ws, next: 1, session: None })
    }

    /// A browser-level call.
    pub fn browser(&mut self, method: &str, params: Value) -> Result<Value> {
        self.send(method, params, None)
    }

    /// A call on the attached page.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let s = self.session.clone().ok_or_else(|| anyhow!("no page attached"))?;
        self.send(method, params, Some(&s))
    }

    fn send(&mut self, method: &str, params: Value, session: Option<&str>) -> Result<Value> {
        let id = self.next;
        self.next += 1;
        let mut msg = json!({"id": id, "method": method, "params": params});
        if let Some(s) = session {
            msg["sessionId"] = json!(s);
        }
        self.ws.send(Message::text(msg.to_string()))?;
        loop {
            let m = self.ws.read().context("Chrome closed the DevTools connection")?;
            let Message::Text(t) = m else { continue };
            let v: Value = serde_json::from_str(t.as_str()).unwrap_or(Value::Null);
            // Events and other calls' replies go by.
            if v["id"].as_u64() != Some(id) {
                continue;
            }
            if let Some(e) = v.get("error") {
                bail!("{method}: {}", e["message"].as_str().unwrap_or("failed"));
            }
            return Ok(v["result"].clone());
        }
    }

    /// Attach to a tab by target id, keeping background tabs rendering.
    pub fn attach(&mut self, target: &str) -> Result<()> {
        let r = self.browser("Target.attachToTarget", json!({"targetId": target, "flatten": true}))?;
        self.session = r["sessionId"].as_str().map(String::from);
        self.call("Emulation.setFocusEmulationEnabled", json!({"enabled": true}))?;
        Ok(())
    }

    /// Whether a tab with this id is still open.
    pub fn has_target(&mut self, target: &str) -> bool {
        self.browser("Target.getTargets", json!({}))
            .map(|r| r["targetInfos"].as_array().is_some_and(|ts| ts.iter().any(|t| t["targetId"] == target && t["type"] == "page")))
            .unwrap_or(false)
    }

    /// Evaluate an expression in the page and return its value; a thrown
    /// exception (the page navigated mid-read, most often) is an error.
    pub fn eval(&mut self, expr: &str, await_promise: bool) -> Result<Value> {
        let r = self.call("Runtime.evaluate", json!({"expression": expr, "returnByValue": true, "awaitPromise": await_promise}))?;
        if r.get("exceptionDetails").is_some() {
            bail!(Stale("the document changed during evaluation".into()));
        }
        Ok(r["result"]["value"].clone())
    }
}

/// A decision that no longer refers to the observed page.
#[derive(Debug)]
pub struct Stale(pub String);

impl std::fmt::Display for Stale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Stale {}

pub fn is_stale(e: &anyhow::Error) -> bool {
    e.downcast_ref::<Stale>().is_some()
}
