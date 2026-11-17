//! Headless shell: stream logs, answer approvals on stdin.
//!
//!   backspace-cli init-config            write the default config, with an explicit
//!                                        [policy], to ./backspace.toml
//!   backspace-cli policy init            print a [policy] block that allows what
//!                                        this project allows today
//!   backspace-cli policy adopt           accept ./backspace.toml's policy as the one
//!                                        to run under (a person's step)
//!   backspace-cli <workspace> "<goal>" [--ship local|branch|pr]
//!                                        run a project; once its tickets are
//!                                        done, Verify, Ship and Learn follow
//!                                        on rules alone (default: local, the
//!                                        run branch for you to merge)
//!   backspace-cli status <workspace>     where the project stands, the last
//!                                        verification and delivery
//!   backspace-cli deliver <workspace> [--ship local|branch|pr]
//!                                        carry on from Execute (retry Verify,
//!                                        Ship, Learn) without running agents
//!   backspace-cli serve <workspace> [addr]
//!                                        share a harness so other machines
//!                                        can follow it (default 127.0.0.1:7420;
//!                                        token from BACKSPACE_TOKEN or printed)
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
            // A new project starts with the policy written out: every model
            // the default config has, the skills found here, no plugins.
            let skills = skill_names(&std::env::current_dir()?);
            let block = backspace_core::Config::parse(DEFAULT_CONFIG)?.policy_init_block(&skills);
            std::fs::write("backspace.toml", format!("{DEFAULT_CONFIG}\n{block}"))?;
            println!("wrote backspace.toml, with a [policy] that allows the models and skills listed in it; narrow it to what this project needs");
            Ok(())
        }
        [cmd, ws, rest @ ..] if cmd == "serve" && rest.len() <= 1 => serve(
            PathBuf::from(ws),
            rest.first().map_or("127.0.0.1:7420", |s| s.as_str()),
        ),
        [cmd, rest @ ..] if cmd == "eval" && !rest.is_empty() => eval(rest),
        [cmd, sub] if cmd == "policy" && (sub == "init" || sub == "adopt") => policy(sub),
        [cmd] if cmd == "scan" => scan(),
        // The tiny local models, and which one small delegated jobs would use.
        [cmd] if cmd == "models" => models(),
        [cmd, rest @ ..] if cmd == "cloud" && rest.len() <= 1 => cloud(
            rest.first().map_or("127.0.0.1:7430", |s| s.as_str()),
        ),
        // After Execute: Verify, Ship and Learn on rules alone (deliver.rs).
        [cmd, ws, rest @ ..] if cmd == "status" && rest.is_empty() => status(PathBuf::from(ws)),
        [cmd, ws, rest @ ..] if cmd == "deliver" => deliver(PathBuf::from(ws), ship_flag(rest)?),
        [ws, goal @ ..] if !goal.is_empty() => {
            let (goal, target) = split_ship(goal)?;
            run(PathBuf::from(ws), goal.join(" "), target)
        }
        _ => bail!(
            "usage: backspace-cli init-config | <workspace> <goal> [--ship local|branch|pr] | status <workspace> | deliver <workspace> [--ship local|branch|pr] | serve <workspace> [addr] | scan | models | cloud [addr] | eval <traces.jsonl> [--threshold N] | policy init | policy adopt"
        ),
    }
}

/// The router eval (PLAN §10) over a file of labeled traces, one JSON object
/// per line: `{"id", "text", "label"}` and optionally `input_tokens` and
/// `output_tokens`. Prints the report as Markdown.
fn eval(args: &[String]) -> Result<()> {
    use backspace_core::decision::{KeywordDecider, DEFAULT_TRIAGE_THRESHOLD};
    let (file, rest) = args.split_first().expect("checked by the caller");
    let threshold = match rest {
        [] => DEFAULT_TRIAGE_THRESHOLD,
        [flag, v] if flag == "--threshold" => v.parse().map_err(|_| anyhow::anyhow!("--threshold takes a number, got `{v}`"))?,
        _ => bail!("usage: backspace-cli eval <traces.jsonl> [--threshold N]"),
    };
    let src = std::fs::read_to_string(file).map_err(|e| anyhow::anyhow!("cannot read {file}: {e}"))?;
    let traces = backspace_core::eval::parse_traces(&src)?;
    let (cfg, _) = backspace_core::Config::load(&std::env::current_dir()?)?;
    let report = backspace_core::eval::evaluate(&KeywordDecider, &traces, threshold, &cfg)?;
    print!("{}", report.to_markdown());
    Ok(())
}

fn skill_names(workspace: &std::path::Path) -> Vec<String> {
    backspace_core::skills::Skills::load(workspace).by_name.keys().cloned().collect()
}

/// `policy init` prints a `[policy]` block that allows what this project
/// allows now; `policy adopt` records the policy in `backspace.toml` as the
/// one to run under (a person's step; an agent has no business here).
fn policy(sub: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let (cfg, path) = backspace_core::Config::load(&cwd)?;
    match sub {
        "init" => {
            print!("{}", cfg.policy_init_block(&skill_names(&cwd)));
            eprintln!("Put this in backspace.toml in place of any [policy] section. It is printed, not written: the policy is yours to edit.");
        }
        _ => {
            // Without a file the policy is the built-in default one (see
            // `Config::new_project`), which is adopted the same way.
            let from = path.map_or_else(|| "the built-in default config".to_string(), |p| p.display().to_string());
            let root = dunce::canonicalize(&cwd)?;
            let loosened = backspace_core::gateway::adopt_policy(&root, &cfg.policy)?;
            println!("adopted the policy in {from} for {}", root.display());
            if !loosened.is_empty() {
                println!("it is looser than the one adopted before:");
                for l in loosened {
                    println!("  {l}");
                }
            }
        }
    }
    Ok(())
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

fn ship_flag(rest: &[String]) -> Result<backspace_core::deliver::Target> {
    match rest {
        [] => Ok(backspace_core::deliver::Target::Local),
        [f, t] if f == "--ship" => backspace_core::deliver::Target::parse(t),
        other => bail!("expected --ship local|branch|pr, got {other:?}"),
    }
}

/// A trailing `--ship <target>` off the goal's words.
fn split_ship(goal: &[String]) -> Result<(&[String], backspace_core::deliver::Target)> {
    if goal.len() >= 2 && goal[goal.len() - 2] == "--ship" {
        let t = backspace_core::deliver::Target::parse(&goal[goal.len() - 1])?;
        let words = &goal[..goal.len() - 2];
        if words.is_empty() {
            bail!("a goal is needed before --ship");
        }
        return Ok((words, t));
    }
    Ok((goal, backspace_core::deliver::Target::Local))
}

fn status(ws: PathBuf) -> Result<()> {
    let root = dunce::canonicalize(&ws)?;
    let p = backspace_core::phase::PhaseState::load(&root);
    println!("phase: {:?} (stage {})", p.phase, p.phase.stage().key());
    for g in p.gates.iter().rev().take(6).rev() {
        println!("  {:?} {} by {:?}: {}", g.phase, if g.passed { "passed" } else { "refused" }, g.by, g.note);
    }
    if let Some(r) = backspace_core::deliver::last_verify(&root) {
        println!("\nverify: {} on {}", if r.passed { "passed" } else { "not passed" }, &r.head[..r.head.len().min(10)]);
        for c in &r.criteria {
            println!("  {} {} `{}`", c.key, if c.passed { "pass" } else { "FAIL" }, c.command);
        }
        for i in r.issues.iter().chain(&r.weakened) {
            println!("  issue: {i}");
        }
    }
    if let Some(d) = backspace_core::deliver::last_delivery(&root) {
        println!("\ndelivered: {:?} {} at {}{}", d.target, d.branch, &d.commit[..d.commit.len().min(10)], d.pr_url.map(|u| format!(" ({u})")).unwrap_or_default());
        for l in &d.limitations {
            println!("  open: {l}");
        }
    }
    Ok(())
}

/// Verify, Ship and Learn for a harness whose agents have finished.
fn after_execute(h: &Harness, target: backspace_core::deliver::Target) -> Result<bool> {
    use backspace_core::deliver::{self, Step, TicketFacts};
    let root = h.root().to_path_buf();
    let snap = h.snapshot();
    let facts: Vec<TicketFacts> = snap.tickets.iter().map(TicketFacts::of).collect();
    let memory = backspace_core::memory::Memory::open(backspace_core::prefs::Prefs::data_dir());
    let mut learn = || -> Result<String> {
        let notes = h.take_notes();
        let fates = backspace_core::stages::rule_fates(&notes);
        match backspace_core::stages::file(&memory, &backspace_core::prefs::Prefs::data_dir(), &root.display().to_string(), &notes, &fates) {
            Ok((dropped, Some(r))) => Ok(format!("Librarian: kept {}, merged {}, dropped {}", r.kept.len(), r.merged.len(), r.dropped.len().max(dropped.len()))),
            Ok((dropped, None)) => Ok(format!("Librarian: nothing to file ({} dropped)", dropped.len())),
            Err(e) => {
                h.return_notes(notes);
                Err(e)
            }
        }
    };
    let steps = deliver::advance_rules(&root, &facts, &deliver::ShipOpts { target, gh: None }, &mut learn)?;
    let mut delivered = false;
    for s in steps {
        match s {
            Step::Executed => println!("execute: every ticket finished"),
            Step::Verified(r) => {
                println!("verify: {} ({} criteria) — {}", if r.passed { "passed" } else { "NOT passed" }, r.criteria.len(), root.join(deliver::VERIFY_MD).display());
                for i in r.issues.iter().chain(&r.weakened) {
                    println!("  issue: {i}");
                }
            }
            Step::Shipped(d) => {
                delivered = true;
                println!("ship: {:?} {} at {}{}", d.target, d.branch, &d.commit[..d.commit.len().min(10)], d.pr_url.as_ref().map(|u| format!(" — {u}")).unwrap_or_default());
                for l in &d.limitations {
                    println!("  open: {l}");
                }
            }
            Step::Learned(Ok(n)) => println!("learn: {n}"),
            Step::Learned(Err(e)) => println!("learn: didn't finish ({e}); the delivery stands"),
        }
    }
    Ok(delivered || backspace_core::phase::PhaseState::load(&root).phase == backspace_core::phase::Phase::Done)
}

fn deliver(ws: PathBuf, target: backspace_core::deliver::Target) -> Result<()> {
    let h = Harness::open(ws)?;
    if !after_execute(&h, target)? {
        // Not delivered: a refusing gate, said above. Scripts see it.
        std::process::exit(2);
    }
    Ok(())
}

fn run(ws: PathBuf, goal: String, target: backspace_core::deliver::Target) -> Result<()> {
    let h = Harness::open(ws)?;
    for n in h.policy_notices() {
        let tag = if n.level == backspace_core::gateway::NoticeLevel::Warning { "policy warning" } else { "policy" };
        eprintln!("{tag}: {}", n.text);
    }
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
            if main.status == AgentStatus::Approved {
                // Specify/Plan means no planned run to verify (a short-path
                // answer); the error says so.
                if let Err(e) = after_execute(&h, target) {
                    println!("delivery: {e:#}");
                }
            }
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
