//! Headless shell: stream logs, answer approvals on stdin.
//!
//!   backspace-cli init-config            write the default config to ./backspace.toml
//!   backspace-cli <workspace> "<goal>"   run a project
//!   backspace-cli serve <workspace> [addr]
//!                                        share a harness so other machines
//!                                        can follow it (default 127.0.0.1:7420;
//!                                        token from BACKSPACE_TOKEN or printed)
//!   backspace-cli serve-models [addr]    the model server alone (default
//!                                        127.0.0.1:7422): every model this
//!                                        machine reaches on one OpenAI URL;
//!                                        another machine needs the token
//!   backspace-cli engine [install [variant]]
//!                                        Brainpowr Engine: its llama.cpp build,
//!                                        the downloaded models, what is loaded;
//!                                        install puts the build for this machine
//!   backspace-cli scan                   which coding CLIs, local models and
//!                                        routers this machine has
//!   backspace-cli cloud [addr]           the development Backspace Cloud
//!                                        (default 127.0.0.1:7430); answers
//!                                        through the models in your config,
//!                                        or canned text with BACKSPACE_CLOUD_MOCK=1

use std::io::{BufRead, Write};
use std::path::PathBuf;

use anyhow::{bail, Result};
use backspace_core::{config::DEFAULT_CONFIG, AgentStatus, Harness, LogKind};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Agents in coding CLIs reach the message board through any Backspace
    // binary: `msg ...` and `mcp`.
    if let Some(code) =
        backspace_core::board::client_main(&std::env::args().skip(1).collect::<Vec<_>>())
    {
        std::process::exit(code);
    }
    match args.as_slice() {
        [cmd] if cmd == "init-config" => {
            if std::path::Path::new("backspace.toml").exists() {
                bail!("backspace.toml already exists");
            }
            std::fs::write("backspace.toml", DEFAULT_CONFIG)?;
            println!("wrote backspace.toml");
            Ok(())
        }
        [cmd, ws, rest @ ..] if cmd == "serve" && rest.len() <= 1 => serve(
            PathBuf::from(ws),
            rest.first().map_or("127.0.0.1:7420", |s| s.as_str()),
        ),
        [cmd] if cmd == "scan" => scan(),
        // The tiny local models, and which one small delegated jobs would use.
        [cmd] if cmd == "models" => models(),
        [cmd, rest @ ..] if cmd == "serve-models" && rest.len() <= 1 => serve_models(rest.first().map(String::as_str)),
        [cmd] if cmd == "engine" => engine_status(),
        [cmd, sub, rest @ ..] if cmd == "engine" && sub == "install" && rest.len() <= 1 => engine_install(rest.first().map(String::as_str)),
        [cmd, rest @ ..] if cmd == "cloud" && rest.len() <= 1 => cloud(
            rest.first().map_or("127.0.0.1:7430", |s| s.as_str()),
        ),
        [ws, goal @ ..] if !goal.is_empty() => run(PathBuf::from(ws), goal.join(" ")),
        _ => bail!(
            "usage: backspace-cli init-config | <workspace> <goal> | serve <workspace> [addr] | scan | models | serve-models [addr] | engine [install [variant]] | cloud [addr]"
        ),
    }
}

fn scan() -> Result<()> {
    use backspace_core::harnesses::Auth;
    let prefs = backspace_core::prefs::Prefs::load();
    for h in backspace_core::harnesses::scan(&prefs) {
        let state = if !h.installed {
            "not installed".to_string()
        } else {
            match h.auth {
                Auth::Authenticated => "authenticated".into(),
                Auth::Unauthenticated => "not signed in".into(),
                Auth::Unknown => "installed".into(),
            }
        };
        println!(
            "{:<12} {:<10} {:<14} {:<4} {}",
            h.name,
            h.version
                .as_deref()
                .map(|v| format!("v{v}"))
                .unwrap_or_default(),
            state,
            if h.enabled { "on" } else { "off" },
            h.detail.or(h.message).unwrap_or_default()
        );
    }
    Ok(())
}

/// Brainpowr Engine at a glance.
fn engine_status() -> Result<()> {
    let e = backspace_core::model_engine::engine();
    let st = e.status();
    println!("llama.cpp {}: {}", st.tag, st.binary.as_deref().unwrap_or("not installed (backspace-cli engine install)"));
    println!("builds for this machine: {}", st.variants.iter().map(|a| format!("{} ({})", a.variant, a.accel)).collect::<Vec<_>>().join(", "));
    for m in e.models() {
        println!(
            "    {:<44} {:>9}  {}",
            m.id,
            backspace_core::model_server::human_size(m.size),
            [m.meta.architecture.clone(), m.meta.context_length.map(|c| format!("ctx {c}"))].into_iter().flatten().collect::<Vec<_>>().join(" · ")
        );
    }
    Ok(())
}

/// Install the best llama.cpp build for this machine, or the one named.
fn engine_install(variant: Option<&str>) -> Result<()> {
    use backspace_core::brainpowr_engine::backend;
    let v = match variant {
        Some(v) => v.to_string(),
        None => backend::variants().first().map(|a| a.variant.to_string()).ok_or_else(|| anyhow::anyhow!("no llama.cpp build for this machine; install llama-server yourself"))?,
    };
    let home = backspace_core::model_engine::engine().home().to_path_buf();
    let stop = std::sync::atomic::AtomicBool::new(false);
    let last = std::sync::Mutex::new(0u64);
    let i = backend::install(&home, &v, &|done, total| {
        let pct = done * 100 / total.max(1);
        let mut l = last.lock().unwrap();
        if pct >= *l + 10 || done == total {
            *l = pct;
            eprintln!("{v}: {pct}%");
        }
    }, &stop)?;
    println!("installed llama.cpp {} {} → {}", i.tag, i.variant, home.join("backends").join(&i.tag).join(&i.variant).join(&i.server).display());
    Ok(())
}

/// The model server without the app, for a box on the network with a GPU and
/// no screen. Open to the network when the address is not loopback, and then
/// it answers discovery too.
fn serve_models(addr: Option<&str>) -> Result<()> {
    let prefs = backspace_core::prefs::Prefs::load();
    let addr = addr.map(str::to_string).unwrap_or_else(|| prefs.model_serve.bind_addr());
    let token = std::env::var("BACKSPACE_TOKEN").unwrap_or(prefs.model_serve.token);
    // 0.0.0.0 is every interface here, not "this machine only".
    let lan = !(addr.starts_with("127.") || addr.starts_with("localhost") || addr.starts_with("[::1]"));
    let port: u16 = addr.rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or(7422);
    println!("serving models at http://{addr}/v1");
    if lan {
        println!("other machines: Authorization: Bearer {token}");
    }
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        if lan {
            tokio::spawn(backspace_core::model_gateway::answer_discovery(port));
        }
        backspace_core::model_gateway::serve(addr, token).await
    })
}

/// The tiny local models (Settings → Local models, without the UI): what is
/// running, which models are small enough to delegate to, and what would be
/// used right now.
fn models() -> Result<()> {
    let prefs = backspace_core::prefs::Prefs::load();
    let runtimes = backspace_core::model_server::detect(&prefs, &[]);
    if runtimes.is_empty() {
        println!("no local runtime found (Ollama, LM Studio, llama.cpp, MLX)");
    }
    for r in &runtimes {
        println!(
            "{:<10} {:<34} {:<9} {}",
            r.id,
            r.base_url,
            if r.running { "running" } else { "stopped" },
            r.message.clone().unwrap_or_default()
        );
        for m in &r.models {
            println!(
                "    {:<44} {}{}",
                m.id,
                if m.tiny { "tiny" } else { "" },
                m.size_bytes.map(|b| format!("  {}", backspace_core::model_server::human_size(b))).unwrap_or_default()
            );
        }
    }
    for r in backspace_core::model_server::remote_runtimes(&prefs) {
        println!(
            "{:<10} {:<34} {:<9} {}",
            r.host.as_deref().unwrap_or(""),
            r.base_url,
            if r.running { "running" } else { "offline" },
            r.message.clone().unwrap_or_default()
        );
        for m in &r.models {
            println!("    {:<44} {}", m.id, if m.tiny { "tiny" } else { "" });
        }
    }
    match backspace_core::model_server::recommend(&runtimes) {
        Some((url, model)) => println!("\nsmall jobs could run on {model} at {url}"),
        None => println!("\nnothing running: small jobs stay on the main model"),
    }
    if let Some(chosen) = &prefs.subagent_model {
        let jobs = backspace_core::harness::local_jobs(&prefs);
        let which: Vec<&str> = jobs.keys().map(String::as_str).collect();
        println!(
            "configured for small jobs: {chosen} → {}",
            if which.is_empty() { "nothing".to_string() } else { which.join(", ") }
        );
    }
    Ok(())
}

fn cloud(addr: &str) -> Result<()> {
    let mock = std::env::var_os("BACKSPACE_CLOUD_MOCK").is_some();
    let dir = backspace_core::prefs::Prefs::data_dir().join("cloud-dev");
    println!(
        "Backspace Cloud (development) on http://{addr}{}",
        if mock { ", mock answers" } else { "" }
    );
    println!("accounts in {}", dir.display());
    println!("plans switch instantly here; nothing is billed.");
    tokio::runtime::Runtime::new()?.block_on(backspace_core::cloud::serve(addr, dir, mock))
}

/// Run a harness headless and share it; approvals come from whoever follows.
fn serve(ws: PathBuf, addr: &str) -> Result<()> {
    let h = Harness::open(ws)?;
    let token =
        std::env::var("BACKSPACE_TOKEN").unwrap_or_else(|_| backspace_core::remote::new_token());
    let _serving = h.serve(addr, &token)?;
    let name = h.snapshot().name;
    println!("serving `{name}` on http://{addr}");
    println!("token: {token}");
    if let Some((url, t)) = h.bridge() {
        println!("agent message board: {url} (token {t})");
    }
    println!("add it in Backspace: Settings → Machines → Add (address http://{addr})");
    if !addr.starts_with("127.") && !addr.starts_with("localhost") {
        println!("note: plain HTTP. Prefer a private network or an SSH tunnel.");
    }
    let changes = h.changes();
    loop {
        let _ = h.block_on(changes.recv());
    }
}

fn run(ws: PathBuf, goal: String) -> Result<()> {
    let h = Harness::open(ws)?;
    let changes = h.changes();
    h.send(goal);

    let mut seen: Vec<usize> = Vec::new();
    let mut asked: Vec<usize> = Vec::new();
    let stdin = std::io::stdin();
    loop {
        let _ = h.block_on(changes.recv());
        let s = h.snapshot();

        for a in &s.agents {
            if seen.len() <= a.id {
                seen.resize(a.id + 1, 0);
            }
            for e in &a.log[seen[a.id]..] {
                let tag = match e.kind {
                    LogKind::User => "user",
                    LogKind::Assistant => "says",
                    LogKind::ToolCall => "tool",
                    LogKind::ToolResult => " ok ",
                    LogKind::System => "sys ",
                    LogKind::Error => "ERR ",
                };
                let first = e.text.lines().next().unwrap_or("");
                println!("[{}:{}] {tag} {first}", a.id, a.key);
            }
            seen[a.id] = a.log.len();
        }

        for ap in s.pending_approvals() {
            if asked.contains(&ap.id) {
                continue;
            }
            asked.push(ap.id);
            let agent = &s.agents[ap.agent];
            println!(
                "\n=== approval #{} from `{}` ({}) ===\n{}\nfiles: {}",
                ap.id,
                agent.key,
                agent.title,
                ap.deliverable.summary,
                ap.deliverable.files.join(", ")
            );
            print!("approve? [y / feedback to reject] > ");
            std::io::stdout().flush()?;
            let mut line = String::new();
            stdin.lock().read_line(&mut line)?;
            let line = line.trim();
            if line.is_empty() || line.eq_ignore_ascii_case("y") {
                h.approve(ap.id);
            } else {
                h.reject(ap.id, line);
            }
        }

        let main = &s.agents[0];
        if matches!(main.status, AgentStatus::Approved | AgentStatus::Failed) {
            println!(
                "\nproject {:?}. total ${:.4} (router ${:.4})",
                main.status, s.total_cost_usd, s.router_cost_usd
            );
            return Ok(());
        }
        let just_sent = main.log.last().map(|e| e.kind) == Some(LogKind::User);
        if main.status == AgentStatus::Idle && !just_sent && s.pending_approvals().next().is_none()
        {
            print!("main is waiting for you > ");
            std::io::stdout().flush()?;
            let mut line = String::new();
            stdin.lock().read_line(&mut line)?;
            h.send(line.trim().to_string());
        }
    }
}
