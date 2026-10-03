//! Headless shell: stream logs, answer approvals on stdin.
//!
//!   backspace-cli init-config            write the default config to ./backspace.toml
//!   backspace-cli <workspace> "<goal>"   run a project
//!   backspace-cli serve <workspace> [addr]
//!                                        share a harness so other machines
//!                                        can follow it (default 127.0.0.1:7420;
//!                                        token from BACKSPACE_TOKEN or printed)

use std::io::{BufRead, Write};
use std::path::PathBuf;

use anyhow::{bail, Result};
use backspace_core::{config::DEFAULT_CONFIG, AgentStatus, Harness, LogKind};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
        [ws, goal @ ..] if !goal.is_empty() => run(PathBuf::from(ws), goal.join(" ")),
        _ => bail!(
            "usage: backspace-cli init-config | backspace-cli <workspace> <goal> | backspace-cli serve <workspace> [addr]"
        ),
    }
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
