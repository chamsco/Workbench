//! Agents with a job (after Noodle): a name, a backstory that sets the
//! role, an avatar, the model or CLI that runs it, its own folder, and
//! optionally its own computer. You talk to one in its thread, or put
//! several in a group with a goal, where they read each other's messages and
//! take turns (see chat.rs).
//!
//! Each agent works in its own folder (`<data>/agents/<id>/workspace`) until
//! you share more folders with it. Its computer is where it runs things that
//! should not touch your machine:
//!
//! - `folder`: just its folder on this machine (the default);
//! - `docker`: a container of its own on this machine, kept between uses
//!   (`/home/agent` is a folder under the agent's directory), optionally a
//!   full desktop you can open (a webtop image on a local port);
//! - `ssh`: a VPS or any machine you can ssh to with a key.
//!
//! CLIs reach the computer through `backspace mcp` (computer_run,
//! computer_read, computer_write), which reads BACKSPACE_COMPUTER.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::chat::{new_id, now_ms, Route};

pub const ENV_COMPUTER: &str = "BACKSPACE_COMPUTER";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Agent {
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// The backstory: who it is and what it does. Its system prompt.
    #[serde(default)]
    pub job: String,
    #[serde(default)]
    pub avatar: Avatar,
    pub route: Route,
    /// Folders it may use besides its own.
    #[serde(default)]
    pub shared: Vec<String>,
    #[serde(default)]
    pub computer: Computer,
    /// Give it the memory notes (global and its own).
    #[serde(default = "yes")]
    pub memory: bool,
    /// Its rules: what it may not do ("edit", "run", "web", "apps").
    #[serde(default)]
    pub off: Vec<String>,
    /// Apps and connections it may not use, by id; the rest it may.
    #[serde(default)]
    pub apps_off: Vec<String>,
    /// Its webhook's secret (Reach this bot); empty while the webhook is off.
    #[serde(default)]
    pub hook: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub updated: u64,
}

fn yes() -> bool {
    true
}

impl Agent {
    pub fn allows(&self, rule: &str) -> bool {
        !self.off.iter().any(|o| o == rule)
    }

    /// What its rules deny, as `backspace mcp` reads it (BACKSPACE_DENY): the
    /// rules turned off, and "app:<id>" for each app or connection it may not use.
    pub fn deny(&self) -> Vec<String> {
        self.off.iter().cloned().chain(self.apps_off.iter().map(|a| format!("app:{a}"))).collect()
    }
}

/// The CLI tools (Claude Code's names) that a bot's rules turn off. Any rule
/// also takes the tools that hand work to something the rules don't cover
/// (a cloud agent, another session, a later run).
pub fn denied_tools(deny: &[String]) -> Vec<String> {
    let handoff = if deny.is_empty() { &[][..] } else { &["RemoteTrigger", "SendMessage", "CronCreate", "ScheduleWakeup"][..] };
    deny.iter()
        .flat_map(|d| match d.as_str() {
            "edit" => &["Edit", "Write", "NotebookEdit", "EnterWorktree"][..],
            "run" => &["Bash"][..],
            "web" => &["WebFetch", "WebSearch"][..],
            _ => &[][..],
        })
        .chain(handoff)
        .map(|t| t.to_string())
        .collect()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Avatar {
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub color: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Computer {
    /// "folder", "docker" or "ssh".
    pub kind: String,
    /// docker: the image (default ubuntu:24.04; a webtop image gives a desktop).
    #[serde(default)]
    pub image: String,
    /// docker: publish the image's web desktop (port 3000) on localhost.
    #[serde(default)]
    pub desktop: bool,
    /// ssh: host, user, port and private key file.
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub key: String,
    /// ssh: a web desktop on the VPS (noVNC...), if it has one.
    #[serde(default)]
    pub desktop_url: String,
}

impl Default for Computer {
    fn default() -> Self {
        Self {
            kind: "folder".into(),
            image: String::new(),
            desktop: false,
            host: String::new(),
            user: String::new(),
            port: 0,
            key: String::new(),
            desktop_url: String::new(),
        }
    }
}

pub const DEFAULT_IMAGE: &str = "ubuntu:24.04";
pub const DESKTOP_IMAGE: &str = "lscr.io/linuxserver/webtop:ubuntu-xfce";

/// What the computer is doing, for the agent's card.
#[derive(Serialize, Clone, Debug, Default)]
pub struct ComputerStatus {
    pub kind: String,
    /// "running", "stopped", "missing", "unreachable", "ready" (folder)
    pub state: String,
    pub detail: String,
    /// A web desktop to open, when there is one.
    pub desktop: Option<String>,
}

/// Everything `backspace mcp` needs to drive an agent's computer, passed in
/// BACKSPACE_COMPUTER as JSON.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ComputerEnv {
    pub agent: String,
    pub name: String,
    pub computer: Computer,
    /// docker: the host folder mounted at /home/agent.
    pub home: String,
}

pub struct Agents {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl Agents {
    pub fn open(dir: PathBuf) -> Self {
        Self {
            dir,
            lock: Mutex::new(()),
        }
    }

    fn agent_dir(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    /// The agent's own folder (created on first use).
    pub fn workspace(&self, id: &str) -> PathBuf {
        let p = self.agent_dir(id).join("workspace");
        let _ = std::fs::create_dir_all(&p);
        p
    }

    fn home(&self, id: &str) -> PathBuf {
        let p = self.agent_dir(id).join("computer");
        let _ = std::fs::create_dir_all(&p);
        p
    }

    pub fn list(&self) -> Vec<Agent> {
        let mut out: Vec<Agent> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str().and_then(|id| self.get(id)))
            .collect();
        out.sort_by(|a, b| a.created.cmp(&b.created));
        out
    }

    pub fn get(&self, id: &str) -> Option<Agent> {
        if id.is_empty() || id.contains(['/', '\\', '.']) {
            return None;
        }
        let b = std::fs::read(self.agent_dir(id).join("agent.json")).ok()?;
        serde_json::from_slice(&b).ok()
    }

    /// Create (empty id) or update an agent.
    pub fn save(&self, mut a: Agent) -> Result<Agent> {
        let _g = self.lock.lock().unwrap();
        a.name = a.name.trim().chars().take(40).collect();
        if a.name.is_empty() {
            bail!("an agent needs a name");
        }
        match a.computer.kind.as_str() {
            "folder" | "" => a.computer.kind = "folder".into(),
            "docker" => {}
            "ssh" => {
                if a.computer.host.trim().is_empty() {
                    bail!("an SSH computer needs a host");
                }
            }
            k => bail!("unknown computer kind {k}"),
        }
        let now = now_ms();
        if a.id.is_empty() {
            a.id = new_id();
            a.created = now;
        } else if let Some(old) = self.get(&a.id) {
            a.created = old.created;
        } else {
            bail!("no such agent");
        }
        a.updated = now;
        let d = self.agent_dir(&a.id);
        std::fs::create_dir_all(&d)?;
        std::fs::write(d.join("agent.json"), serde_json::to_vec_pretty(&a)?)?;
        let _ = self.workspace(&a.id);
        Ok(a)
    }

    /// Remove the agent. Its folder and computer files go too; a container
    /// is removed.
    pub fn delete(&self, id: &str) -> Result<()> {
        let a = self.get(id).ok_or_else(|| anyhow!("no such agent"))?;
        if a.computer.kind == "docker" {
            let _ = docker(&["rm", "-f", &container(id)]);
        }
        std::fs::remove_dir_all(self.agent_dir(id))?;
        Ok(())
    }

    /// The brief a reply runs with: who it is, where it works, its computer.
    pub fn brief(&self, a: &Agent) -> String {
        let mut s = format!(
            "You are {}, one of the user's agents in Backspace.\n\n{}\n\nYour own folder is {} (your working directory). Keep your files there.",
            a.name,
            if a.job.trim().is_empty() { "Help the user with whatever they ask." } else { a.job.trim() },
            self.workspace(&a.id).display()
        );
        if !a.shared.is_empty() {
            s.push_str(&format!(" The user also shared: {}.", a.shared.join(", ")));
        }
        match a.computer.kind.as_str() {
            "docker" => s.push_str(" You have your own Linux computer (a container); run commands, install software and keep files there with the computer_run, computer_read and computer_write tools. Prefer it over this machine for anything that installs or runs code."),
            "ssh" => s.push_str(&format!(" You have your own computer, a server at {}; use the computer_run, computer_read and computer_write tools to work on it.", a.computer.host)),
            _ => {}
        }
        s
    }

    pub fn computer_env(&self, a: &Agent) -> Option<ComputerEnv> {
        (a.computer.kind == "docker" || a.computer.kind == "ssh").then(|| ComputerEnv {
            agent: a.id.clone(),
            name: a.name.clone(),
            computer: a.computer.clone(),
            home: self.home(&a.id).display().to_string(),
        })
    }

    pub fn status(&self, id: &str) -> Result<ComputerStatus> {
        let a = self.get(id).ok_or_else(|| anyhow!("no such agent"))?;
        let c = &a.computer;
        Ok(match c.kind.as_str() {
            "docker" => {
                let name = container(id);
                match docker(&["inspect", "-f", "{{.State.Running}}", &name]) {
                    Ok(o) if o.trim() == "true" => ComputerStatus {
                        kind: "docker".into(),
                        state: "running".into(),
                        detail: image_of(c).into(),
                        desktop: desktop_port(&name).map(|p| format!("http://127.0.0.1:{p}")),
                    },
                    Ok(_) => ComputerStatus { kind: "docker".into(), state: "stopped".into(), detail: image_of(c).into(), desktop: None },
                    Err(e) if e.to_string().to_lowercase().contains("no such") => {
                        ComputerStatus { kind: "docker".into(), state: "missing".into(), detail: image_of(c).into(), desktop: None }
                    }
                    Err(e) => ComputerStatus { kind: "docker".into(), state: "unreachable".into(), detail: e.to_string(), desktop: None },
                }
            }
            "ssh" => ComputerStatus {
                kind: "ssh".into(),
                state: "ready".into(),
                detail: format!("{}@{}", if c.user.is_empty() { "root" } else { &c.user }, c.host),
                desktop: (!c.desktop_url.is_empty()).then(|| c.desktop_url.clone()),
            },
            _ => ComputerStatus {
                kind: "folder".into(),
                state: "ready".into(),
                detail: self.workspace(id).display().to_string(),
                desktop: None,
            },
        })
    }

    /// Start (creating if needed) a docker computer.
    pub fn start(&self, id: &str) -> Result<ComputerStatus> {
        let a = self.get(id).ok_or_else(|| anyhow!("no such agent"))?;
        if a.computer.kind == "docker" {
            let env = self.computer_env(&a).unwrap();
            ensure_container(&env)?;
        }
        self.status(id)
    }

    pub fn stop(&self, id: &str) -> Result<ComputerStatus> {
        let a = self.get(id).ok_or_else(|| anyhow!("no such agent"))?;
        if a.computer.kind == "docker" {
            docker(&["stop", "-t", "2", &container(id)])?;
        }
        self.status(id)
    }
}

fn image_of(c: &Computer) -> &str {
    if !c.image.trim().is_empty() {
        c.image.trim()
    } else if c.desktop {
        DESKTOP_IMAGE
    } else {
        DEFAULT_IMAGE
    }
}

pub fn container(id: &str) -> String {
    format!("backspace-agent-{id}")
}

fn docker(args: &[&str]) -> Result<String> {
    let o = Command::new("docker")
        .args(args)
        .env("PATH", crate::harnesses::path_env())
        .output()
        .context("Docker is not installed (or not on PATH)")?;
    if !o.status.success() {
        bail!("{}", String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).to_string())
}

fn desktop_port(name: &str) -> Option<u16> {
    let o = docker(&["port", name, "3000/tcp"]).ok()?;
    o.lines().next()?.rsplit(':').next()?.trim().parse().ok()
}

/// The agent's container, running. Created on first use with its home
/// folder mounted, so files survive a removed container.
pub fn ensure_container(env: &ComputerEnv) -> Result<()> {
    let name = container(&env.agent);
    match docker(&["inspect", "-f", "{{.State.Running}}", &name]) {
        Ok(o) if o.trim() == "true" => return Ok(()),
        Ok(_) => {
            docker(&["start", &name])?;
            return Ok(());
        }
        Err(_) => {}
    }
    let image = image_of(&env.computer).to_string();
    let mount = format!("{}:/home/agent", env.home);
    let host: String = env.name.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).take(30).collect();
    let host = if host.is_empty() { "agent".to_string() } else { host };
    let mut args: Vec<String> = vec![
        "run".into(), "-d".into(), "--name".into(), name.clone(),
        "--label".into(), "backspace.agent=1".into(),
        "-v".into(), mount, "-w".into(), "/home/agent".into(),
        "--hostname".into(), host,
    ];
    if env.computer.desktop {
        args.extend(["-p".into(), "127.0.0.1::3000".into(), "--shm-size".into(), "1g".into()]);
        args.push(image);
    } else {
        args.push(image);
        args.extend(["sleep".into(), "infinity".into()]);
    }
    let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    docker(&a).map(|_| ())
}

const OUT_LIMIT: usize = 30_000;

fn clip(s: String) -> String {
    if s.len() <= OUT_LIMIT {
        return s;
    }
    let mut end = OUT_LIMIT;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… ({} more bytes)", &s[..end], s.len() - end)
}

fn ssh_cmd(c: &Computer) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=10"]);
    if c.port != 0 {
        cmd.args(["-p", &c.port.to_string()]);
    }
    if !c.key.trim().is_empty() {
        cmd.args(["-i", c.key.trim()]);
    }
    let user = if c.user.trim().is_empty() { "root" } else { c.user.trim() };
    cmd.arg(format!("{user}@{}", c.host.trim()));
    cmd
}

/// Run a shell command on the agent's computer, with a time limit.
pub fn run(env: &ComputerEnv, command: &str, timeout: Duration) -> Result<String> {
    let mut cmd = match env.computer.kind.as_str() {
        "docker" => {
            ensure_container(env)?;
            let mut c = Command::new("docker");
            c.args(["exec", "-w", "/home/agent", &container(&env.agent), "sh", "-lc", command]);
            c
        }
        "ssh" => {
            let mut c = ssh_cmd(&env.computer);
            c.arg(command);
            c
        }
        k => bail!("{k} computers run commands through the CLI itself"),
    };
    output_with_timeout(&mut cmd, None, timeout)
}

/// Write a file on the agent's computer (stdin piped to `cat >`).
pub fn write(env: &ComputerEnv, path: &str, content: &str) -> Result<String> {
    let q = shell_quote(path);
    let script = format!("mkdir -p \"$(dirname {q})\" && cat > {q}");
    let mut cmd = match env.computer.kind.as_str() {
        "docker" => {
            ensure_container(env)?;
            let mut c = Command::new("docker");
            c.args(["exec", "-i", "-w", "/home/agent", &container(&env.agent), "sh", "-c", &script]);
            c
        }
        "ssh" => {
            let mut c = ssh_cmd(&env.computer);
            c.arg(script);
            c
        }
        k => bail!("{k} computers write files through the CLI itself"),
    };
    output_with_timeout(&mut cmd, Some(content.as_bytes()), Duration::from_secs(60))?;
    Ok(format!("wrote {} bytes to {path}", content.len()))
}

pub fn read(env: &ComputerEnv, path: &str) -> Result<String> {
    run(env, &format!("cat {}", shell_quote(path)), Duration::from_secs(60))
}

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn output_with_timeout(cmd: &mut Command, input: Option<&[u8]>, timeout: Duration) -> Result<String> {
    use std::io::{Read, Write};
    use std::process::Stdio;
    let mut child = cmd
        .env("PATH", crate::harnesses::path_env())
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting the computer's command")?;
    if let Some(i) = input {
        child.stdin.take().unwrap().write_all(i)?;
    }
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let ro = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out.read_to_end(&mut b);
        b
    });
    let re = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err.read_to_end(&mut b);
        b
    });
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            bail!("timed out after {}s", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let o = String::from_utf8_lossy(&ro.join().unwrap_or_default()).to_string();
    let e = String::from_utf8_lossy(&re.join().unwrap_or_default()).to_string();
    let text = if e.trim().is_empty() { o } else { format!("{o}{}{e}", if o.is_empty() { "" } else { "\n[stderr]\n" }) };
    if !status.success() {
        bail!("exit {}: {}", status.code().unwrap_or(-1), clip(text));
    }
    Ok(clip(text))
}

/// Folder paths are kept as the user typed them; resolve `~`.
pub fn expand(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(h) = std::env::var_os("HOME") {
            return Path::new(&h).join(rest);
        }
    }
    PathBuf::from(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(name: &str) -> Agent {
        Agent {
            id: String::new(),
            name: name.into(),
            job: "Writes release notes.".into(),
            avatar: Avatar::default(),
            route: Route { kind: crate::chat::RouteKind::Cli, provider: "claude".into(), model: None },
            shared: vec![],
            computer: Computer::default(),
            memory: true,
            off: vec![],
            apps_off: vec![],
            hook: String::new(),
            created: 0,
            updated: 0,
        }
    }

    #[test]
    fn rules_become_denied_tools() {
        let mut a = agent("Kira");
        assert!(a.deny().is_empty() && a.allows("run"));
        a.off = vec!["run".into(), "web".into()];
        a.apps_off = vec!["github".into()];
        assert!(!a.allows("run") && a.allows("edit"));
        assert_eq!(a.deny(), ["run", "web", "app:github"]);
        assert_eq!(denied_tools(&a.deny()), ["Bash", "WebFetch", "WebSearch", "RemoteTrigger", "SendMessage", "CronCreate", "ScheduleWakeup"]);
        assert!(denied_tools(&[]).is_empty());
    }

    #[test]
    fn save_list_brief_delete() {
        let dir = std::env::temp_dir().join(format!("bs-agents-{}", now_ms()));
        let ag = Agents::open(dir.clone());
        assert!(ag.save(agent("  ")).is_err());
        let a = ag.save(agent("Kira")).unwrap();
        assert!(!a.id.is_empty());
        std::thread::sleep(std::time::Duration::from_millis(3));
        let mut b = agent("Andre");
        b.computer = Computer { kind: "ssh".into(), ..Computer::default() };
        assert!(ag.save(b.clone()).is_err(), "ssh needs a host");
        b.computer.host = "10.0.0.5".into();
        let b = ag.save(b).unwrap();
        assert_eq!(ag.list().iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Kira", "Andre"]);
        let brief = ag.brief(&b);
        assert!(brief.contains("You are Andre") && brief.contains("10.0.0.5") && brief.contains("computer_run"));
        assert!(ag.computer_env(&a).is_none());
        assert_eq!(ag.status(&a.id).unwrap().state, "ready");
        assert!(ag.get("../x").is_none());
        ag.delete(&a.id).unwrap();
        assert_eq!(ag.list().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Runs only where Docker answers (skipped elsewhere).
    #[test]
    fn docker_computer() {
        if docker(&["info", "--format", "{{.ServerVersion}}"]).is_err() {
            eprintln!("no docker; skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!("bs-agents-dk-{}", now_ms()));
        let ag = Agents::open(dir.clone());
        let mut a = agent("Box");
        a.computer = Computer { kind: "docker".into(), image: "alpine:3.20".into(), ..Computer::default() };
        let a = ag.save(a).unwrap();
        let env = ag.computer_env(&a).unwrap();
        assert_eq!(ag.start(&a.id).unwrap().state, "running");
        assert!(write(&env, "notes/hello.txt", "hi there").unwrap().contains("8 bytes"));
        assert_eq!(read(&env, "notes/hello.txt").unwrap(), "hi there");
        assert!(run(&env, "uname -s", Duration::from_secs(30)).unwrap().contains("Linux"));
        assert!(run(&env, "exit 3", Duration::from_secs(30)).unwrap_err().to_string().contains("exit 3"));
        // Files live in the agent's folder on this machine.
        assert!(dir.join(&a.id).join("computer/notes/hello.txt").exists());
        assert_eq!(ag.stop(&a.id).unwrap().state, "stopped");
        ag.delete(&a.id).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("a'b c"), "'a'\\''b c'");
    }
}
