//! Finding the coding CLIs, local models and routers on this machine: is it
//! installed, which version, is it signed in. Setup and Settings show the
//! result and let the user switch each one on or off; chats and projects
//! only offer the ones that are on.
//!
//! Probes run the CLIs' own `--version` and status commands (the same ones
//! you would type), with a short timeout, never a model request.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::prefs::{Prefs, RouterCfg};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A coding agent CLI (Codex, Claude Code...).
    Cli,
    /// Models on this machine (Ollama).
    Local,
    /// An OpenAI-compatible endpoint the user added.
    Router,
    /// A remote agent over A2A the user connected.
    Agent,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Auth {
    Authenticated,
    Unauthenticated,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HarnessInfo {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub auth: Auth,
    /// "ChatGPT Go Subscription", "API key", "3 models"...
    pub detail: Option<String>,
    /// What to tell the user when something is off.
    pub message: Option<String>,
    pub enabled: bool,
    /// The user flipped the switch (vs. the default from the scan).
    pub user_set: bool,
    /// Models this harness can chat with, when it says.
    pub models: Vec<String>,
    /// Can be used in Chat (one-shot prompts). All but Antigravity.
    pub chat: bool,
    pub install: &'static str,
}

struct Spec {
    id: &'static str,
    name: &'static str,
    bins: &'static [&'static str],
    install: &'static str,
    chat: bool,
}

const CLIS: &[Spec] = &[
    Spec {
        id: "codex",
        name: "Codex",
        bins: &["codex"],
        install: "npm i -g @openai/codex",
        chat: true,
    },
    Spec {
        id: "claude",
        name: "Claude",
        bins: &["claude"],
        install: "npm i -g @anthropic-ai/claude-code",
        chat: true,
    },
    Spec {
        id: "cursor",
        name: "Cursor",
        bins: &["cursor-agent"],
        install: "curl https://cursor.com/install -fsS | bash",
        chat: true,
    },
    Spec {
        id: "grok",
        name: "Grok",
        bins: &["grok"],
        install: "npm i -g @vibe-kit/grok-cli",
        chat: true,
    },
    Spec {
        id: "opencode",
        name: "OpenCode",
        bins: &["opencode"],
        install: "npm i -g opencode-ai",
        chat: true,
    },
    Spec {
        id: "antigravity",
        name: "Antigravity",
        bins: &["agy", "antigravity"],
        install: "https://antigravity.google",
        chat: false,
    },
];

/// The executable for a CLI id ("cursor" is `cursor-agent`).
pub fn bin_for(id: &str) -> &str {
    match id {
        "cursor" => "cursor-agent",
        "antigravity" => "agy",
        other => other,
    }
}

/// The ids Setup lists, in order.
pub fn known() -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<_> = CLIS.iter().map(|s| (s.id, s.name)).collect();
    v.push(("ollama", "Ollama"));
    v
}

// ---------------------------------------------------------------- PATH

/// Apps started from the Dock or a desktop launcher get a bare PATH, not
/// the one your shell builds. Ask a login shell once, then add the usual
/// install folders.
pub fn search_path() -> &'static Vec<PathBuf> {
    static PATH: OnceLock<Vec<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| {
        let mut dirs: Vec<PathBuf> = Vec::new();
        let mut add = |p: PathBuf| {
            if !p.as_os_str().is_empty() && !dirs.contains(&p) {
                dirs.push(p);
            }
        };
        if let Some(p) = std::env::var_os("PATH") {
            std::env::split_paths(&p).for_each(&mut add);
        }
        #[cfg(unix)]
        if std::env::var_os("BACKSPACE_NO_SHELL_PATH").is_none() {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            if let Some(out) = run(
                Path::new(&shell),
                &["-ilc", "printf '\\n__BS__%s' \"$PATH\""],
                Duration::from_secs(3),
            ) {
                if let Some(p) = out.stdout.rsplit("__BS__").next() {
                    std::env::split_paths(p.trim()).for_each(&mut add);
                }
            }
        }
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            for d in [
                ".local/bin",
                ".bun/bin",
                ".npm-global/bin",
                ".cargo/bin",
                ".opencode/bin",
                ".volta/bin",
                "Library/pnpm",
                ".antigravity/bin",
            ] {
                add(home.join(d));
            }
        }
        for d in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
            add(PathBuf::from(d));
        }
        dirs
    })
}

/// The PATH string to hand to child processes, so a CLI that spawns `node`
/// finds it too.
pub fn path_env() -> std::ffi::OsString {
    std::env::join_paths(search_path()).unwrap_or_default()
}

pub fn which(bin: &str) -> Option<PathBuf> {
    // `BACKSPACE_CLI_PATH` is searched first (tests, pinned installs).
    let extra: Vec<PathBuf> = std::env::var_os("BACKSPACE_CLI_PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for dir in extra.iter().chain(search_path().iter()) {
        // On Windows npm puts a POSIX shell shim with no extension next to
        // `<bin>.cmd`; only .exe/.cmd can be spawned (else os error 193).
        #[cfg(windows)]
        for ext in ["exe", "cmd"] {
            let p = dir.join(format!("{bin}.{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
        let p = dir.join(bin);
        if cfg!(unix) && p.is_file() {
            return Some(p);
        }
    }
    None
}

pub struct Out {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Run a command with a deadline; None if it could not start or ran over.
pub fn run(bin: &Path, args: &[&str], timeout: Duration) -> Option<Out> {
    run_with(Command::new(bin), args, timeout)
}

/// Run a CLI with the full search PATH, so one that spawns `node` finds it.
fn run_cli(bin: &Path, args: &[&str]) -> Option<Out> {
    let mut cmd = Command::new(bin);
    cmd.env("PATH", path_env());
    run_with(cmd, args, Duration::from_secs(8))
}

fn run_with(mut cmd: Command, args: &[&str], timeout: Duration) -> Option<Out> {
    cmd.args(args)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    let (mut so, mut se) = (child.stdout.take()?, child.stderr.take()?);
    let ro = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = so.read_to_string(&mut s);
        s
    });
    let re = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = se.read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    Some(Out {
        code: status.code().unwrap_or(-1),
        stdout: ro.join().unwrap_or_default(),
        stderr: re.join().unwrap_or_default(),
    })
}

/// "codex-cli 0.160.0" / "2.1.289 (Claude Code)" -> "0.160.0" / "2.1.289".
pub fn parse_version(s: &str) -> Option<String> {
    s.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ',')
        .map(|w| w.trim_start_matches('v'))
        .find(|w| {
            let mut parts = w.split('.');
            w.contains('.')
                && parts
                    .next()
                    .is_some_and(|p| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty())
        })
        .map(str::to_string)
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

// ---------------------------------------------------------------- auth

/// Decode a JWT's claims without verifying it: we only read our own file to
/// show which plan you are on.
fn jwt_claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = b64url(payload)?;
    serde_json::from_slice(&bytes).ok()
}

fn b64url(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

pub fn codex_plan_label(plan: &str) -> String {
    match plan {
        "free" => "ChatGPT Free Subscription".into(),
        "go" => "ChatGPT Go Subscription".into(),
        "plus" => "ChatGPT Plus Subscription".into(),
        "pro" => "ChatGPT Pro Subscription".into(),
        "team" => "ChatGPT Team Subscription".into(),
        "business" => "ChatGPT Business Subscription".into(),
        "enterprise" => "ChatGPT Enterprise Subscription".into(),
        "edu" => "ChatGPT Edu Subscription".into(),
        other => format!("ChatGPT {other}"),
    }
}

fn codex_auth(bin: &Path) -> (Auth, Option<String>) {
    let file = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| home().map(|h| h.join(".codex")))
        .map(|d| d.join("auth.json"));
    let json: Option<Value> = file
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|s| serde_json::from_str(&s).ok());
    if let Some(v) = &json {
        let plan = v["tokens"]["id_token"]
            .as_str()
            .and_then(jwt_claims)
            .and_then(|c| {
                c["https://api.openai.com/auth"]["chatgpt_plan_type"]
                    .as_str()
                    .map(str::to_string)
            });
        if let Some(plan) = plan {
            return (Auth::Authenticated, Some(codex_plan_label(&plan)));
        }
        if v["OPENAI_API_KEY"].as_str().is_some_and(|k| !k.is_empty()) {
            return (Auth::Authenticated, Some("API key".into()));
        }
    }
    match run_cli(bin, &["login", "status"]) {
        Some(o) if o.code == 0 => {
            let text = format!("{}{}", o.stdout, o.stderr);
            let detail = if text.contains("API key") {
                "API key"
            } else {
                "ChatGPT"
            };
            (Auth::Authenticated, Some(detail.into()))
        }
        Some(_) => (Auth::Unauthenticated, None),
        None => (Auth::Unknown, None),
    }
}

fn claude_auth(bin: &Path) -> (Auth, Option<String>) {
    let Some(o) = run_cli(bin, &["auth", "status"]) else {
        return (Auth::Unknown, None);
    };
    let v: Value = serde_json::from_str(o.stdout.trim()).unwrap_or(Value::Null);
    match v["loggedIn"].as_bool() {
        Some(true) => {
            let detail = v["subscriptionType"]
                .as_str()
                .map(|s| format!("Claude {} Subscription", title(s)))
                .or_else(|| match v["authMethod"].as_str() {
                    Some("api_key") => Some("API key".into()),
                    _ => None,
                });
            (Auth::Authenticated, detail)
        }
        Some(false) => (Auth::Unauthenticated, None),
        None if o.code == 0 && std::env::var_os("ANTHROPIC_API_KEY").is_some() => {
            (Auth::Authenticated, Some("API key".into()))
        }
        None => (Auth::Unknown, None),
    }
}

fn cursor_auth(bin: &Path) -> (Auth, Option<String>) {
    match run_cli(bin, &["status"]) {
        Some(o) => {
            let t = format!("{}{}", o.stdout, o.stderr).to_lowercase();
            if t.contains("not logged in") || t.contains("not authenticated") {
                (Auth::Unauthenticated, None)
            } else if t.contains("logged in") || t.contains("authenticated") {
                let email = t.split_whitespace().find(|w| w.contains('@')).map(|w| {
                    w.trim_matches(|c: char| !c.is_alphanumeric() && c != '@' && c != '.')
                        .to_string()
                });
                (Auth::Authenticated, email)
            } else {
                (Auth::Unknown, None)
            }
        }
        None => (Auth::Unknown, None),
    }
}

fn grok_auth() -> (Auth, Option<String>) {
    if ["XAI_API_KEY", "GROK_API_KEY"]
        .iter()
        .any(|k| std::env::var(k).is_ok_and(|v| !v.trim().is_empty()))
    {
        return (Auth::Authenticated, Some("API key".into()));
    }
    let settings = home().map(|h| h.join(".grok/user-settings.json"));
    if let Some(v) = settings
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
    {
        if v["apiKey"].as_str().is_some_and(|k| !k.is_empty()) {
            return (Auth::Authenticated, Some("API key".into()));
        }
    }
    (Auth::Unauthenticated, None)
}

fn opencode_auth() -> (Auth, Option<String>) {
    let file = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home().map(|h| h.join(".local/share")))
        .map(|d| d.join("opencode/auth.json"));
    let v: Option<Value> = file
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|s| serde_json::from_str(&s).ok());
    match v.as_ref().and_then(Value::as_object) {
        Some(m) if !m.is_empty() => {
            let names: Vec<&str> = m.keys().map(String::as_str).take(3).collect();
            (Auth::Authenticated, Some(names.join(", ")))
        }
        Some(_) => (Auth::Unauthenticated, None),
        None => (Auth::Unknown, None),
    }
}

fn title(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// The CLI's own sign-in command: `codex login` is Sign in with ChatGPT,
/// `claude auth login` signs Claude Code in. Backspace never sees the
/// credentials; the CLI keeps them where it always does.
pub fn login_args(id: &str) -> Option<&'static [&'static str]> {
    match id {
        "codex" => Some(&["login"]),
        "claude" => Some(&["auth", "login"]),
        "cursor" => Some(&["login"]),
        "opencode" => Some(&["auth", "login"]),
        _ => None,
    }
}

/// Open the CLI's sign-in in a terminal window. A terminal, not a hidden
/// child: these flows are interactive (pick a method, paste a code) and the
/// user should see what runs.
pub fn open_login(id: &str) -> anyhow::Result<()> {
    let args = login_args(id).ok_or_else(|| anyhow::anyhow!("{id} has no sign-in command"))?;
    let bin = which(bin_for(id)).ok_or_else(|| anyhow::anyhow!("{id} is not installed"))?;
    let line = format!("'{}' {}", bin.display(), args.join(" "));
    let mut cmd = if cfg!(windows) {
        // `start` opens a new console; /K keeps it so errors stay readable.
        // The bare name resolves through PATH below (cmd can't take Rust's quoting).
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "Sign in", "cmd", "/K", bin_for(id)]).args(args);
        c
    } else if cfg!(target_os = "macos") {
        let script = format!(
            "tell application \"Terminal\" to do script \"{}\"\ntell application \"Terminal\" to activate",
            line.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let mut c = Command::new("osascript");
        c.args(["-e", &script]);
        c
    } else {
        // ponytail: first terminal found wins; add a pref if someone needs another.
        let term = ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"]
            .into_iter()
            .find(|t| which(t).is_some())
            .ok_or_else(|| anyhow::anyhow!("no terminal found; run {line} yourself"))?;
        let mut c = Command::new(term);
        if term == "gnome-terminal" { c.arg("--") } else { c.arg("-e") };
        c.args(["sh", "-c", &format!("{line}; exec $SHELL")]);
        c
    };
    cmd.env("PATH", path_env()).spawn()?;
    Ok(())
}

// ---------------------------------------------------------------- probes

fn probe_cli(spec: &Spec) -> HarnessInfo {
    let mut h = HarnessInfo {
        id: spec.id.into(),
        name: spec.name.into(),
        kind: Kind::Cli,
        installed: false,
        path: None,
        version: None,
        auth: Auth::Unknown,
        detail: None,
        message: None,
        enabled: false,
        user_set: false,
        models: vec![],
        chat: spec.chat,
        install: spec.install,
    };
    let Some(bin) = spec.bins.iter().find_map(|b| which(b)) else {
        h.message = Some(format!("Not installed. Install with: {}", spec.install));
        return h;
    };
    h.installed = true;
    h.path = Some(bin.display().to_string());
    match run_cli(&bin, &["--version"]) {
        Some(o) => h.version = parse_version(&format!("{}\n{}", o.stdout, o.stderr)),
        None => {
            h.message = Some("Installed, but `--version` did not answer in time.".into());
            return h;
        }
    }
    let (auth, detail) = match spec.id {
        "codex" => codex_auth(&bin),
        "claude" => claude_auth(&bin),
        "cursor" => cursor_auth(&bin),
        "grok" => grok_auth(),
        "opencode" => opencode_auth(),
        _ => (Auth::Unknown, None),
    };
    h.auth = auth;
    h.detail = detail;
    if auth == Auth::Unauthenticated {
        h.message = Some(match login_args(spec.id) {
            Some(a) => format!("Not signed in. Sign in here, or run `{} {}` in a terminal.", spec.bins[0], a.join(" ")),
            None => "Not signed in: add an xAI API key (XAI_API_KEY) or set one in the CLI.".into(),
        });
    }
    if !spec.chat {
        h.message.get_or_insert_with(|| {
            "Projects only: it has no one-shot prompt mode for Chat.".into()
        });
    }
    h
}

fn http_json(url: &str, key: Option<&str>) -> Result<Value, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(4))
            .build()
            .map_err(|e| e.to_string())?;
        let mut rb = client.get(url);
        if let Some(k) = key.filter(|k| !k.is_empty()) {
            rb = rb.bearer_auth(k);
        }
        let r = rb.send().await.map_err(|e| {
            if e.is_connect() {
                "not running".to_string()
            } else {
                e.to_string()
            }
        })?;
        let status = r.status();
        if !status.is_success() {
            return Err(format!("HTTP {status}"));
        }
        r.json::<Value>().await.map_err(|e| e.to_string())
    })
}

fn probe_ollama(url: &str) -> HarnessInfo {
    let bin = which("ollama");
    let mut h = HarnessInfo {
        id: "ollama".into(),
        name: "Ollama".into(),
        kind: Kind::Local,
        installed: bin.is_some(),
        path: bin.as_ref().map(|b| b.display().to_string()),
        version: bin
            .as_ref()
            .and_then(|b| run_cli(b, &["--version"]))
            .and_then(|o| parse_version(&format!("{}\n{}", o.stdout, o.stderr))),
        auth: Auth::Unknown,
        detail: None,
        message: None,
        enabled: false,
        user_set: false,
        models: vec![],
        chat: true,
        install: "https://ollama.com/download",
    };
    match http_json(&format!("{}/api/tags", url.trim_end_matches('/')), None) {
        Ok(v) => {
            h.installed = true;
            h.auth = Auth::Authenticated;
            h.models = v["models"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|m| m["name"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            h.detail = Some(match h.models.len() {
                0 => "Running · no models pulled".into(),
                1 => "Running · 1 model".into(),
                n => format!("Running · {n} models"),
            });
            if h.models.is_empty() {
                h.message = Some("Pull a model first, e.g. `ollama pull llama3.2`.".into());
            }
        }
        Err(e) => {
            h.auth = Auth::Unauthenticated;
            h.message = Some(if h.installed {
                format!("Installed but the server at {url} is {e}. Start it with `ollama serve`.")
            } else {
                "Not installed. Runs models on this machine, offline.".into()
            });
        }
    }
    h
}

pub fn probe_router(r: &RouterCfg) -> HarnessInfo {
    let mut h = HarnessInfo {
        id: format!("router:{}", r.id),
        name: r.name.clone(),
        kind: Kind::Router,
        installed: true,
        path: Some(r.base_url.clone()),
        version: None,
        auth: Auth::Unknown,
        detail: None,
        message: None,
        enabled: false,
        user_set: false,
        models: vec![],
        chat: true,
        install: "",
    };
    match http_json(
        &format!("{}/models", r.base_url.trim_end_matches('/')),
        Some(&r.api_key),
    ) {
        Ok(v) => {
            h.auth = Auth::Authenticated;
            let mut all: Vec<String> = v["data"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|m| m["id"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            all.sort();
            h.models = if r.models.is_empty() {
                all
            } else {
                r.models.clone()
            };
            h.detail = Some(format!("{} models", h.models.len()));
        }
        Err(e) => {
            h.auth = if e.contains("401") || e.contains("403") {
                Auth::Unauthenticated
            } else {
                Auth::Unknown
            };
            h.message = Some(format!("{}: {e}", r.base_url));
            // Still usable with the models typed in by hand.
            h.models = r.models.clone();
        }
    }
    h
}

/// Probe everything in parallel. Takes a few seconds at most.
pub fn scan(prefs: &Prefs) -> Vec<HarnessInfo> {
    let ollama_url = prefs.ollama_url.clone();
    let mut out: Vec<HarnessInfo> = std::thread::scope(|s| {
        let clis: Vec<_> = CLIS.iter().map(|c| s.spawn(move || probe_cli(c))).collect();
        let ol = s.spawn(move || probe_ollama(&ollama_url));
        let rs: Vec<_> = prefs
            .routers
            .iter()
            .map(|r| s.spawn(move || probe_router(r)))
            .collect();
        clis.into_iter()
            .chain(std::iter::once(ol))
            .chain(rs)
            .filter_map(|h| h.join().ok())
            .collect()
    });
    out.extend(prefs.remote_agents.iter().map(remote_agent));
    apply_toggles(&mut out, &prefs.harnesses);
    out
}

/// A connected A2A agent as a provider. Not probed on every scan: its
/// card was read when it was connected, and a reply says if it's gone.
pub fn remote_agent(a: &crate::prefs::RemoteAgent) -> HarnessInfo {
    HarnessInfo {
        id: format!("a2a:{}", a.id),
        name: a.name.clone(),
        kind: Kind::Agent,
        installed: true,
        path: Some(a.url.clone()),
        version: None,
        auth: Auth::Authenticated,
        detail: Some(if a.description.is_empty() { "A2A agent".into() } else { a.description.clone() }),
        message: None,
        enabled: true,
        user_set: false,
        models: vec![],
        chat: true,
        install: "",
    }
}

/// On by default when it is usable; the user's switch wins.
pub fn apply_toggles(list: &mut [HarnessInfo], toggles: &BTreeMap<String, bool>) {
    for h in list {
        let usable = h.installed && h.auth != Auth::Unauthenticated;
        match toggles.get(&h.id) {
            Some(&on) => {
                h.enabled = on;
                h.user_set = true;
            }
            None => h.enabled = usable && (h.kind != Kind::Router || h.auth == Auth::Authenticated),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(
            parse_version("codex-cli 0.160.0").as_deref(),
            Some("0.160.0")
        );
        assert_eq!(
            parse_version("2.1.289 (Claude Code)").as_deref(),
            Some("2.1.289")
        );
        assert_eq!(
            parse_version("opencode v1.18.16\n").as_deref(),
            Some("1.18.16")
        );
        assert_eq!(
            parse_version("ollama version is 0.12.3").as_deref(),
            Some("0.12.3")
        );
        assert_eq!(parse_version("no version here"), None);
    }

    #[test]
    fn codex_plan_from_id_token() {
        // {"https://api.openai.com/auth":{"chatgpt_plan_type":"go"}}
        let claims = r#"{"https://api.openai.com/auth":{"chatgpt_plan_type":"go"}}"#;
        let enc: String = {
            const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
            let b = claims.as_bytes();
            let mut s = String::new();
            for ch in b.chunks(3) {
                let n = ch.iter().fold(0u32, |a, &x| (a << 8) | x as u32) << (8 * (3 - ch.len()));
                for i in 0..=ch.len() {
                    s.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
                }
            }
            s
        };
        let c = jwt_claims(&format!("h.{enc}.s")).unwrap();
        let plan = c["https://api.openai.com/auth"]["chatgpt_plan_type"]
            .as_str()
            .unwrap();
        assert_eq!(codex_plan_label(plan), "ChatGPT Go Subscription");
    }

    #[test]
    fn toggles_override_scan() {
        let mut v = vec![HarnessInfo {
            id: "claude".into(),
            name: "Claude".into(),
            kind: Kind::Cli,
            installed: true,
            path: None,
            version: None,
            auth: Auth::Authenticated,
            detail: None,
            message: None,
            enabled: false,
            user_set: false,
            models: vec![],
            chat: true,
            install: "",
        }];
        apply_toggles(&mut v, &BTreeMap::new());
        assert!(v[0].enabled);
        apply_toggles(&mut v, &BTreeMap::from([("claude".to_string(), false)]));
        assert!(!v[0].enabled && v[0].user_set);
    }
}
