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
//! * **Anywhere else** (Windows, or Linux without bubblewrap): nothing
//!   isolates. `mode = "auto"` still runs the command, with secrets taken out
//!   of the environment, and the agent's log says so; `"required"` refuses it.
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
            Backend::Plain => "none",
        }
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
