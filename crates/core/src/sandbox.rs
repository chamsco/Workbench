//! M6 (PLAN §5.5, §11): the sandbox agents' commands run in.
//!
//! An agent's `bash`, the test runs made for it (checks, Rampart's vetting,
//! the flake and mutation runs) and a coding-CLI worker's whole process run
//! inside a sandbox built per command:
//!
//! * **Linux: bubblewrap.** A fresh mount namespace: the system read-only, a
//!   private `/tmp`, the project read-only, the agent's worktree and git's
//!   object store writable, Backspace's folder replaced by an empty one, the
//!   hidden paths covered, the read-only files bound over themselves. Its own
//!   PID namespace, and with `network = false` its own empty network.
//! * **macOS: Seatbelt** (`sandbox-exec`), with the same rules written as a
//!   profile: writes only to the worktree, git's store, a private temp
//!   folder and the home folder; reads and writes of the hidden paths denied.
//! * **Windows: low integrity.** The command runs from a token lowered to
//!   the Low mandatory level, in a job object that dies with it. Windows'
//!   mandatory integrity control then stops every write to anything not
//!   labelled Low: the worktree and a private temp folder are labelled Low,
//!   the policy and sealed tests are labelled Medium again, and Backspace's
//!   files and the credentials are labelled no-read-up. It cannot switch the
//!   network off, and git's store stays read-only to agents (Backspace
//!   commits for them).
//! * **Anywhere else** (Linux without bubblewrap): nothing isolates.
//!   `mode = "auto"` still runs the command, with secrets taken out of the
//!   environment, and the agent's log says so; `"required"` refuses it.
//!
//! What it does not do, stated in docs/cauldron/M6-NOTES.md: the home folder
//! stays writable (caches, a CLI's own state); other files in the system are
//! readable; the network is on by default, so what keeps an executor to the
//! approved model is that no other model's credentials reach it.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// The variable a sandboxed command finds its sandbox's name in.
pub const ENV_LEVEL: &str = "BACKSPACE_SANDBOX";

/// The first argument that makes any Backspace binary the Windows launcher.
pub const JAIL_ARG: &str = "__backspace_jail";

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum SandboxMode {
    /// The strongest sandbox this machine has; without one, run with the
    /// environment scrubbed and say so.
    #[default]
    Auto,
    /// Refuse a command no sandbox can isolate.
    Required,
    /// Run commands as they are (the behaviour before M6).
    Off,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SandboxConfig {
    pub mode: SandboxMode,
    /// Agents' commands may open network connections. Off, they reach
    /// nothing, not even localhost.
    pub network: bool,
    /// Secret-looking variables to hand on anyway.
    pub pass_env: Vec<String>,
    /// More paths agents may not read (`~/` is the home folder).
    pub hide: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        SandboxConfig { mode: SandboxMode::Auto, network: true, pass_env: vec![], hide: vec![] }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Backend {
    Bwrap(PathBuf),
    Seatbelt(PathBuf),
    /// Windows: the launcher's command line (a helper binary, or a Backspace
    /// binary and [`JAIL_ARG`]), which runs the command at low integrity.
    LowIntegrity(Vec<String>),
    /// Nothing isolates: only the environment is scrubbed.
    Plain,
}

fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file())
}

fn quiet(cmd: &mut Command) -> bool {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

impl Backend {
    /// The strongest backend that actually runs here, found once.
    pub fn probe() -> Backend {
        static FOUND: OnceLock<Backend> = OnceLock::new();
        FOUND
            .get_or_init(|| {
                if cfg!(target_os = "linux") {
                    if let Some(b) = on_path("bwrap").or_else(|| Some(PathBuf::from("/usr/bin/bwrap")).filter(|p| p.is_file())) {
                        // Installed is not enough: user namespaces can be off.
                        let mut c = Command::new(&b);
                        c.args(["--die-with-parent", "--unshare-pid", "--unshare-net", "--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc", "--tmpfs", "/tmp", "--", "true"]);
                        if quiet(&mut c) {
                            return Backend::Bwrap(b);
                        }
                    }
                }
                if cfg!(windows) {
                    if let Some(prefix) = jail_launcher() {
                        let mut c = Command::new(&prefix[0]);
                        c.args(&prefix[1..]).args(["--", "cmd.exe", "/c", "exit 0"]);
                        if quiet(&mut c) {
                            return Backend::LowIntegrity(prefix);
                        }
                    }
                }
                if cfg!(target_os = "macos") {
                    let b = PathBuf::from("/usr/bin/sandbox-exec");
                    if b.is_file() && quiet(Command::new(&b).args(["-p", "(version 1)(allow default)", "/usr/bin/true"])) {
                        return Backend::Seatbelt(b);
                    }
                }
                Backend::Plain
            })
            .clone()
    }

    pub fn isolates(&self) -> bool {
        !matches!(self, Backend::Plain)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Backend::Bwrap(_) => "bwrap",
            Backend::Seatbelt(_) => "seatbelt",
            Backend::LowIntegrity(_) => "lowil",
            Backend::Plain => "none",
        }
    }

    /// It can take the network away (`network = false`).
    pub fn blocks_network(&self) -> bool {
        matches!(self, Backend::Bwrap(_) | Backend::Seatbelt(_))
    }

    /// Agents may write git's store (commit) from inside it.
    pub fn agents_commit(&self) -> bool {
        matches!(self, Backend::Bwrap(_) | Backend::Seatbelt(_))
    }
}

/// What one sandboxed command may see and change.
#[derive(Clone, Debug)]
pub struct Profile {
    /// The project root: readable, not writable.
    pub project: PathBuf,
    /// The agent's own directory (its worktree): readable and writable, and
    /// where the command starts.
    pub work: PathBuf,
    /// More writable places (the home folder).
    pub writable: Vec<PathBuf>,
    /// Files and folders that stay read-only even inside writable places
    /// (the policy, sealed tests); one outside the project is also made
    /// visible (a CLI's binary in a temp folder).
    pub read_only: Vec<PathBuf>,
    /// Neither readable nor writable.
    pub hidden: Vec<PathBuf>,
    pub network: bool,
    /// Take secret-looking variables out of the environment.
    pub scrub_env: bool,
    /// Variables kept even though they look secret.
    pub pass_env: Vec<String>,
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// Credentials no agent's command needs.
const SECRET_DIRS: [&str; 14] = [
    ".ssh", ".aws", ".gnupg", ".netrc", ".git-credentials", ".config/gh", ".config/gcloud", ".azure", ".kube", ".docker/config.json",
    ".npmrc", ".pypirc", ".cargo/credentials.toml", ".cargo/credentials",
];

/// Coding CLIs' logins: hidden from agents' shells, kept for a CLI worker.
const CLI_DIRS: [&str; 6] = [".claude", ".claude.json", ".codex", ".config/opencode", ".cursor", ".grok"];

fn expand(p: &str) -> PathBuf {
    match (p.strip_prefix("~/"), home()) {
        (Some(rest), Some(h)) => h.join(rest),
        _ => PathBuf::from(p),
    }
}

impl Profile {
    /// An agent's shell and the test runs made in its directory.
    pub fn agent(project: &Path, work: &Path, cfg: &SandboxConfig) -> Profile {
        let mut p = Profile::base(project, work, cfg);
        if let Some(h) = home() {
            p.hidden.extend(CLI_DIRS.iter().map(|d| h.join(d)));
        }
        p
    }

    /// Rampart's shell: an agent's, with no network (M5 decision 2).
    pub fn rampart(project: &Path, work: &Path, cfg: &SandboxConfig) -> Profile {
        let mut p = Profile::agent(project, work, cfg);
        p.network = false;
        p
    }

    /// A coding-CLI worker: it keeps its environment (its login for the model
    /// the gateway approved) and the network its model is reached over, and
    /// its binary is visible wherever it lives.
    pub fn cli(project: &Path, work: &Path, bin: &Path, cfg: &SandboxConfig) -> Profile {
        let mut p = Profile::base(project, work, cfg);
        p.scrub_env = false;
        p.network = true;
        // Its logins can live in Backspace's data (accounts.rs): only the
        // policy locks there are hidden from it.
        let data = crate::prefs::Prefs::data_dir();
        p.hidden.retain(|h| *h != data);
        p.hidden.push(data.join("policy-locks"));
        if let Some(dir) = bin.parent().filter(|d| !d.as_os_str().is_empty() && *d != Path::new("/")) {
            p.read_only.push(dir.to_path_buf());
        }
        p
    }

    fn base(project: &Path, work: &Path, cfg: &SandboxConfig) -> Profile {
        let mut hidden = vec![project.join(".backspace"), crate::prefs::Prefs::data_dir()];
        if let Some(h) = home() {
            hidden.extend(SECRET_DIRS.iter().map(|d| h.join(d)));
        }
        hidden.extend(cfg.hide.iter().map(|s| expand(s)));
        let mut read_only = vec![work.join("backspace.toml")];
        if project != work {
            read_only.push(project.join("backspace.toml"));
        }
        Profile {
            project: project.to_path_buf(),
            work: work.to_path_buf(),
            writable: home().into_iter().collect(),
            read_only,
            hidden,
            network: cfg.network,
            scrub_env: true,
            pass_env: cfg.pass_env.clone(),
        }
    }
}

/// A command line to run in place of the one asked for, and the whole
/// environment to give it (None: inherit).
#[derive(Clone, Debug)]
pub struct Wrapped {
    pub argv: Vec<String>,
    pub env: Option<Vec<(String, String)>>,
}

#[derive(Clone, Debug)]
pub struct Sandbox {
    pub mode: SandboxMode,
    pub backend: Backend,
}

/// A sandbox and what it lets one agent do.
pub struct Jail {
    pub sandbox: Sandbox,
    pub profile: Profile,
}

/// A variable whose name says it holds a credential.
pub fn is_secret_name(name: &str) -> bool {
    let up = name.to_ascii_uppercase();
    if ["API_KEY", "APIKEY", "ACCESS_KEY", "PRIVATE_KEY"].iter().any(|s| up.contains(s)) {
        return true;
    }
    up.split(|c: char| !c.is_ascii_alphanumeric()).any(|t| {
        matches!(t, "KEY" | "TOKEN" | "SECRET" | "SECRETS" | "PASSWORD" | "PASSWD" | "PASSPHRASE" | "CREDENTIAL" | "CREDENTIALS" | "AUTH" | "COOKIE" | "PAT")
    })
}

/// This process's environment with the secrets taken out.
///
/// git's `GIT_CONFIG_COUNT` / `GIT_CONFIG_KEY_n` / `GIT_CONFIG_VALUE_n` are
/// settings, not secrets, and git refuses to run with a hole in them: they
/// are kept as a set and renumbered, less any setting that carries a
/// credential (an extra header, a credential helper's secret, a password).
pub fn scrubbed_env(pass: &[String]) -> Vec<(String, String)> {
    let vars: Vec<(String, String)> = std::env::vars().collect();
    let get = |k: &str| vars.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let mut out: Vec<(String, String)> = vars
        .iter()
        .filter(|(k, _)| !k.starts_with("GIT_CONFIG_KEY_") && !k.starts_with("GIT_CONFIG_VALUE_") && k != "GIT_CONFIG_COUNT")
        .filter(|(k, _)| !is_secret_name(k) || pass.iter().any(|p| p == k))
        .cloned()
        .collect();
    let count: usize = get("GIT_CONFIG_COUNT").and_then(|c| c.parse().ok()).unwrap_or(0);
    let mut kept = 0;
    for i in 0..count {
        let (Some(k), Some(v)) = (get(&format!("GIT_CONFIG_KEY_{i}")), get(&format!("GIT_CONFIG_VALUE_{i}"))) else { continue };
        let lk = k.to_ascii_lowercase();
        if ["extraheader", "password", "token", "secret", "credential.helper"].iter().any(|w| lk.contains(w)) {
            continue;
        }
        out.push((format!("GIT_CONFIG_KEY_{kept}"), k));
        out.push((format!("GIT_CONFIG_VALUE_{kept}"), v));
        kept += 1;
    }
    if kept > 0 {
        out.push(("GIT_CONFIG_COUNT".into(), kept.to_string()));
    }
    out
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The real path, for what exists; as given otherwise.
fn real(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// git's shared directory for a worktree: `<work>/.git` is a file naming
/// `<common>/worktrees/<name>`, or a directory for the main checkout.
fn git_common(work: &Path) -> Option<PathBuf> {
    let dot = work.join(".git");
    if dot.is_dir() {
        return Some(dot);
    }
    let text = std::fs::read_to_string(&dot).ok()?;
    let gitdir = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
    let gitdir = if gitdir.is_absolute() { gitdir } else { work.join(gitdir) };
    let common = std::fs::read_to_string(gitdir.join("commondir")).ok().map(|c| {
        let c = PathBuf::from(c.trim());
        if c.is_absolute() { c } else { gitdir.join(c) }
    });
    Some(real(&common.unwrap_or_else(|| gitdir.parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or(gitdir))))
}

impl Sandbox {
    pub fn new(cfg: &SandboxConfig) -> Sandbox {
        let backend = if cfg.mode == SandboxMode::Off { Backend::Plain } else { Backend::probe() };
        Sandbox { mode: cfg.mode, backend }
    }

    pub fn with_backend(mode: SandboxMode, backend: Backend) -> Sandbox {
        Sandbox { mode, backend }
    }

    /// The name agents and logs see: the backend, or "off".
    pub fn label(&self) -> &'static str {
        if self.mode == SandboxMode::Off { "off" } else { self.backend.label() }
    }

    /// One line for the agent's log.
    pub fn describe(&self) -> String {
        match (self.mode, &self.backend) {
            (SandboxMode::Off, _) => "sandbox: off (sandbox.mode = \"off\"); commands run as they are".into(),
            (_, Backend::Plain) => format!(
                "sandbox: none on this machine; commands run with secrets taken out of the environment but are not isolated{}",
                if self.mode == SandboxMode::Required { " (mode \"required\": they are refused)" } else { "" }
            ),
            (_, b @ Backend::LowIntegrity(_)) => format!("sandbox: {} (Windows low integrity); agents write only to their worktree, Backspace's folder and credentials are hidden; the network stays on and Backspace makes the commits", b.label()),
            (_, b) => format!("sandbox: {}; agents write only to their worktree, Backspace's folder and credentials are hidden", b.label()),
        }
    }

    /// The command line and environment `argv` runs with under `p`.
    pub fn wrap(&self, p: &Profile, argv: &[String]) -> Result<Wrapped> {
        if self.mode == SandboxMode::Off {
            return Ok(Wrapped { argv: argv.to_vec(), env: None });
        }
        if self.mode == SandboxMode::Required && !self.backend.isolates() {
            bail!("no sandbox can run on this machine (bubblewrap on Linux, Seatbelt on macOS), and sandbox.mode is \"required\", so the command was not run");
        }
        let mut env = p.scrub_env.then(|| scrubbed_env(&p.pass_env));
        if let Some(e) = env.as_mut() {
            e.retain(|(k, _)| k != ENV_LEVEL);
            e.push((ENV_LEVEL.into(), self.label().into()));
        }
        let argv = match &self.backend {
            Backend::Plain => argv.to_vec(),
            Backend::LowIntegrity(prefix) => {
                let tmp = win_prepare(p)?;
                let mut a = prefix.clone();
                a.extend(["--tmp".to_string(), s(&tmp), "--".to_string()]);
                a.extend(argv.iter().cloned());
                a
            }
            Backend::Bwrap(bin) => bwrap(bin, p, argv),
            Backend::Seatbelt(bin) => {
                let (tmp, profile) = seatbelt(p);
                let mut a = vec!["/usr/bin/env".to_string(), format!("TMPDIR={}/", s(&tmp)), format!("{ENV_LEVEL}={}", self.label())];
                a.extend([s(bin), "-p".into(), profile]);
                a.extend(argv.iter().cloned());
                a
            }
        };
        Ok(Wrapped { argv, env })
    }
}

fn bwrap(bin: &Path, p: &Profile, argv: &[String]) -> Vec<String> {
    let (project, work) = (real(&p.project), real(&p.work));
    let own = project.join(".backspace");
    let mut a: Vec<String> = vec![s(bin), "--die-with-parent".into(), "--unshare-pid".into()];
    if !p.network {
        a.push("--unshare-net".into());
    }
    let push = |a: &mut Vec<String>, flag: &str, path: &Path| {
        a.extend([flag.to_string(), s(path), s(path)]);
    };
    a.extend(["--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc", "--tmpfs", "/tmp"].map(String::from));
    for w in p.writable.iter().map(|w| real(w)).filter(|w| w.exists()) {
        push(&mut a, "--bind", &w);
    }
    push(&mut a, "--ro-bind", &project);
    let git = git_common(&work);
    if let Some(g) = &git {
        push(&mut a, "--bind", g);
    }
    let inside = work.starts_with(&own);
    if inside {
        a.extend(["--tmpfs".into(), s(&own)]);
        push(&mut a, "--bind", &work);
    } else {
        push(&mut a, "--bind", &work);
        if own.exists() && !own.starts_with(&work) || own.exists() && work == project {
            a.extend(["--tmpfs".into(), s(&own)]);
        }
    }
    if let Some(g) = &git {
        for f in ["hooks", "config", "info"] {
            let f = g.join(f);
            if f.exists() {
                push(&mut a, "--ro-bind", &f);
            }
        }
    }
    for h in p.hidden.iter().map(|h| real(h)) {
        // Backspace's own folder is done above; a hidden path holding the
        // agent's own directory cannot be covered without hiding it too.
        if h == own || work.starts_with(&h) || !h.exists() {
            continue;
        }
        if h.is_dir() {
            a.extend(["--tmpfs".into(), s(&h)]);
        } else {
            a.extend(["--ro-bind".into(), "/dev/null".into(), s(&h)]);
        }
    }
    for r in p.read_only.iter().map(|r| real(r)).filter(|r| r.exists()) {
        push(&mut a, "--ro-bind", &r);
    }
    a.extend(["--chdir".into(), s(&work), "--".into()]);
    a.extend(argv.iter().cloned());
    a
}

/// The quoted form of a path in a Seatbelt profile.
fn sb(p: &Path) -> String {
    format!("\"{}\"", s(p).replace('\\', "\\\\").replace('"', "\\\""))
}

/// A private temp folder and the Seatbelt profile for `p`.
fn seatbelt(p: &Profile) -> (PathBuf, String) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let tmp = real(&std::env::temp_dir()).join(format!("bs-sandbox-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::create_dir_all(&tmp);
    let (project, work) = (real(&p.project), real(&p.work));
    let mut r = String::from("(version 1)\n(allow default)\n(deny file-write* (subpath \"/\"))\n");
    r.push_str("(allow file-write* (subpath \"/dev\")");
    r.push_str(&format!(" (subpath {})", sb(&tmp)));
    for w in &p.writable {
        r.push_str(&format!(" (subpath {})", sb(&real(w))));
    }
    r.push_str(")\n");
    r.push_str(&format!("(deny file-write* (subpath {}))\n", sb(&project)));
    let git = git_common(&work);
    r.push_str(&format!("(allow file-write* (subpath {})", sb(&work)));
    if let Some(g) = &git {
        r.push_str(&format!(" (subpath {})", sb(g)));
    }
    r.push_str(")\n");
    for h in p.hidden.iter().map(|h| real(h)) {
        if work.starts_with(&h) && h != project.join(".backspace") {
            continue;
        }
        r.push_str(&format!("(deny file-read* file-write* (subpath {}))\n", sb(&h)));
    }
    // The worktree lives under Backspace's folder: open it again.
    if work.starts_with(project.join(".backspace")) {
        r.push_str(&format!("(allow file-read* file-write* (subpath {}))\n", sb(&work)));
        if let Some(g) = &git {
            r.push_str(&format!("(allow file-read* (subpath {}))\n", sb(&work)));
            let _ = g;
        }
    }
    let mut ro: Vec<PathBuf> = p.read_only.iter().map(|r| real(r)).collect();
    if let Some(g) = &git {
        ro.extend(["hooks", "config", "info"].map(|f| g.join(f)));
    }
    for f in ro {
        r.push_str(&format!("(deny file-write* (subpath {}))\n", sb(&f)));
    }
    if !p.network {
        r.push_str("(deny network*)\n");
    }
    (tmp, r)
}

/// How to start the Windows launcher: `BACKSPACE_JAIL`, a `backspace-jail`
/// beside this binary (or beside the folder a test binary is in), or this
/// binary itself when it is one of Backspace's (they all take [`JAIL_ARG`]).
fn jail_launcher() -> Option<Vec<String>> {
    if let Some(p) = std::env::var_os("BACKSPACE_JAIL") {
        return Some(vec![PathBuf::from(p).to_string_lossy().into_owned()]);
    }
    let exe = std::env::current_exe().ok()?;
    let name = format!("backspace-jail{}", std::env::consts::EXE_SUFFIX);
    let dir = exe.parent()?;
    for d in [Some(dir), dir.parent()].into_iter().flatten() {
        let p = d.join(&name);
        if p.is_file() {
            return Some(vec![s(&p)]);
        }
    }
    let stem = exe.file_stem()?.to_string_lossy().to_ascii_lowercase();
    (stem.starts_with("backspace") || stem.starts_with("cauldron")).then(|| vec![s(&exe), JAIL_ARG.to_string()])
}

/// The launcher itself: `[--tmp <dir>] -- <program> <args…>`. Runs the
/// program at low integrity in a job object, with this process's standard
/// handles and environment, and returns its exit code.
pub fn jail_run(args: &[String]) -> i32 {
    let mut tmp = None;
    let mut i = 0;
    while i < args.len() && args[i] != "--" {
        if args[i] == "--tmp" && i + 1 < args.len() {
            tmp = Some(args[i + 1].clone());
            i += 1;
        }
        i += 1;
    }
    let cmd = &args[(i + 1).min(args.len())..];
    if cmd.is_empty() {
        eprintln!("backspace jail: usage: [--tmp <dir>] -- <program> <args…>");
        return 2;
    }
    match win::run_low(cmd, tmp.as_deref()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("backspace jail: {e:#}");
            126
        }
    }
}

/// Labels for one Windows command: the worktree and a private temp folder
/// writable at low integrity, the read-only files back at Medium, the hidden
/// ones no-read-up. Returns the temp folder.
fn win_prepare(p: &Profile) -> Result<PathBuf> {
    use win::Label;
    let tmp = std::env::temp_dir().join("bs-sandbox-low");
    std::fs::create_dir_all(&tmp)?;
    win::label(&tmp, Label::LowWritable, true)?;
    // Hidden first, so the worktree's own label is set after.
    let own = p.project.join(".backspace");
    for h in &p.hidden {
        if h == &own {
            win_hide_backspace(&own)?;
        } else if h.exists() && !p.work.starts_with(h) {
            win::label(h, Label::NoReadUp, h.is_dir())?;
        }
    }
    win::label(&p.work, Label::LowWritable, true)?;
    for r in p.read_only.iter().filter(|r| r.exists()) {
        win::label(r, Label::Medium, r.is_dir())?;
    }
    if let Some(g) = git_common(&p.work) {
        for f in ["hooks", "config", "info"] {
            let f = g.join(f);
            if f.exists() {
                win::label(&f, Label::Medium, f.is_dir())?;
            }
        }
    }
    Ok(tmp)
}

/// Backspace's folder holds the worktrees, so it cannot be hidden whole:
/// every file in it is labelled no-read-up, except under the worktrees and
/// Rampart's checkout, which have labels of their own.
fn win_hide_backspace(own: &Path) -> Result<()> {
    if !own.exists() {
        return Ok(());
    }
    let skip = [own.join("worktrees"), own.join("rampart").join("tree")];
    let mut stack = vec![own.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)?.flatten() {
            let path = e.path();
            if skip.contains(&path) {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                win::label(&path, win::Label::NoReadUp, false)?;
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
mod win {
    //! Windows: low-integrity tokens, mandatory labels, job objects.
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use anyhow::{bail, Result};
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, LocalFree, HANDLE};
    use windows_sys::Win32::Security::Authorization::{ConvertStringSidToSidW, SetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        AddMandatoryAce, DuplicateTokenEx, InitializeAcl, SecurityImpersonation, SetTokenInformation, TokenIntegrityLevel, TokenPrimary, ACL,
        ACL_REVISION, CONTAINER_INHERIT_ACE, LABEL_SECURITY_INFORMATION, OBJECT_INHERIT_ACE, PSID, SID_AND_ATTRIBUTES, TOKEN_ADJUST_DEFAULT,
        TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
    use windows_sys::Win32::System::Environment::SetEnvironmentVariableW;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::SystemServices::{SE_GROUP_INTEGRITY, SYSTEM_MANDATORY_LABEL_NO_READ_UP, SYSTEM_MANDATORY_LABEL_NO_WRITE_UP};
    use windows_sys::Win32::System::Threading::{
        CreateProcessAsUserW, GetCurrentProcess, GetExitCodeProcess, OpenProcessToken, ResumeThread, WaitForSingleObject, CREATE_SUSPENDED,
        INFINITE, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOW,
    };

    #[derive(Clone, Copy)]
    pub enum Label {
        /// Low, no-write-up: a low-integrity process may write it.
        LowWritable,
        /// Medium, no-write-up: read-only to low integrity.
        Medium,
        /// Medium, no-read-up and no-write-up: hidden from low integrity.
        NoReadUp,
    }

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    struct Sid(PSID);
    impl Drop for Sid {
        fn drop(&mut self) {
            unsafe { LocalFree(self.0 as _) };
        }
    }

    fn sid(text: &str) -> Result<Sid> {
        let mut p: PSID = std::ptr::null_mut();
        let w = wide(std::ffi::OsStr::new(text));
        if unsafe { ConvertStringSidToSidW(w.as_ptr(), &mut p) } == 0 {
            bail!("ConvertStringSidToSidW({text}) failed: {}", unsafe { GetLastError() });
        }
        Ok(Sid(p))
    }

    /// Set `path`'s mandatory label; `inherit` for a folder's contents too.
    pub fn label(path: &Path, l: Label, inherit: bool) -> Result<()> {
        let (level, mask) = match l {
            Label::LowWritable => ("S-1-16-4096", SYSTEM_MANDATORY_LABEL_NO_WRITE_UP),
            Label::Medium => ("S-1-16-8192", SYSTEM_MANDATORY_LABEL_NO_WRITE_UP),
            Label::NoReadUp => ("S-1-16-8192", SYSTEM_MANDATORY_LABEL_NO_WRITE_UP | SYSTEM_MANDATORY_LABEL_NO_READ_UP),
        };
        let sid = sid(level)?;
        let mut buf = [0u64; 32];
        let acl = buf.as_mut_ptr() as *mut ACL;
        let flags = if inherit { OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE } else { 0 };
        unsafe {
            if InitializeAcl(acl, std::mem::size_of_val(&buf) as u32, ACL_REVISION) == 0 {
                bail!("InitializeAcl failed: {}", GetLastError());
            }
            if AddMandatoryAce(acl, ACL_REVISION, flags as _, mask, sid.0) == 0 {
                bail!("AddMandatoryAce failed: {}", GetLastError());
            }
            let w = wide(path.as_os_str());
            let r = SetNamedSecurityInfoW(
                w.as_ptr(),
                SE_FILE_OBJECT,
                LABEL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
                acl,
            );
            if r != 0 {
                bail!("labelling {} failed: error {r}", path.display());
            }
        }
        Ok(())
    }

    /// One argument as CreateProcess's command-line parser reads it back.
    fn quote(a: &str) -> String {
        if !a.is_empty() && !a.chars().any(|c| matches!(c, ' ' | '\t' | '\n' | '"')) {
            return a.to_string();
        }
        let mut out = String::from("\"");
        let mut slashes = 0;
        for c in a.chars() {
            match c {
                '\\' => slashes += 1,
                '"' => {
                    out.push_str(&"\\".repeat(slashes * 2 + 1));
                    out.push('"');
                    slashes = 0;
                    continue;
                }
                _ => {}
            }
            if c != '\\' {
                out.push_str(&"\\".repeat(slashes));
                slashes = 0;
                out.push(c);
            }
        }
        out.push_str(&"\\".repeat(slashes * 2));
        out.push('"');
        out
    }

    pub fn run_low(cmd: &[String], tmp: Option<&str>) -> Result<i32> {
        unsafe {
            if let Some(t) = tmp {
                for k in ["TMP", "TEMP", "TMPDIR"] {
                    let (k, v) = (wide(std::ffi::OsStr::new(k)), wide(std::ffi::OsStr::new(t)));
                    SetEnvironmentVariableW(k.as_ptr(), v.as_ptr());
                }
            }
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT | TOKEN_ASSIGN_PRIMARY, &mut token) == 0 {
                bail!("OpenProcessToken failed: {}", GetLastError());
            }
            let mut low: HANDLE = std::ptr::null_mut();
            let ok = DuplicateTokenEx(
                token,
                TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT | TOKEN_ASSIGN_PRIMARY,
                std::ptr::null(),
                SecurityImpersonation,
                TokenPrimary,
                &mut low,
            );
            CloseHandle(token);
            if ok == 0 {
                bail!("DuplicateTokenEx failed: {}", GetLastError());
            }
            let level = sid("S-1-16-4096")?;
            let tml = TOKEN_MANDATORY_LABEL { Label: SID_AND_ATTRIBUTES { Sid: level.0, Attributes: SE_GROUP_INTEGRITY as u32 } };
            if SetTokenInformation(low, TokenIntegrityLevel, &tml as *const _ as *const _, std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32) == 0 {
                CloseHandle(low);
                bail!("lowering the token's integrity failed: {}", GetLastError());
            }
            // A job that ends the command (and what it starts) with us.
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if !job.is_null() {
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info as *const _ as *const _, std::mem::size_of_val(&info) as u32);
            }
            let line: String = cmd.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ");
            let mut line_w = wide(std::ffi::OsStr::new(&line));
            let mut si: STARTUPINFOW = std::mem::zeroed();
            si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
            si.dwFlags = STARTF_USESTDHANDLES;
            si.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
            si.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE);
            si.hStdError = GetStdHandle(STD_ERROR_HANDLE);
            let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
            let ok = CreateProcessAsUserW(
                low,
                std::ptr::null(),
                line_w.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                CREATE_SUSPENDED,
                std::ptr::null(),
                std::ptr::null(),
                &si,
                &mut pi,
            );
            CloseHandle(low);
            if ok == 0 {
                bail!("starting {} at low integrity failed: {}", cmd[0], GetLastError());
            }
            if !job.is_null() {
                AssignProcessToJobObject(job, pi.hProcess);
            }
            ResumeThread(pi.hThread);
            CloseHandle(pi.hThread);
            WaitForSingleObject(pi.hProcess, INFINITE);
            let mut code = 1u32;
            GetExitCodeProcess(pi.hProcess, &mut code);
            CloseHandle(pi.hProcess);
            Ok(code as i32)
        }
    }
}

#[cfg(not(windows))]
mod win {
    use std::path::Path;

    use anyhow::{bail, Result};

    #[derive(Clone, Copy)]
    pub enum Label {
        LowWritable,
        Medium,
        NoReadUp,
    }

    pub fn label(_path: &Path, _l: Label, _inherit: bool) -> Result<()> {
        bail!("mandatory labels are a Windows feature")
    }

    pub fn run_low(_cmd: &[String], _tmp: Option<&str>) -> Result<i32> {
        bail!("the low-integrity launcher runs on Windows only")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_common_dir_of_a_linked_worktree() {
        let d = std::env::temp_dir().join(format!("bs-sbx-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("repo/.git/worktrees/t1")).unwrap();
        std::fs::create_dir_all(d.join("wt")).unwrap();
        std::fs::write(d.join("repo/.git/worktrees/t1/commondir"), "../..\n").unwrap();
        std::fs::write(d.join("wt/.git"), format!("gitdir: {}\n", d.join("repo/.git/worktrees/t1").display())).unwrap();
        assert_eq!(git_common(&d.join("wt")).unwrap(), real(&d.join("repo/.git")));
        assert_eq!(git_common(&d.join("repo")).unwrap(), d.join("repo/.git"));
    }
}
