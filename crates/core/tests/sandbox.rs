//! M6 acceptance (PLAN §5.5, §11, §12): the execution engine.
//!
//! * Sandboxed worktrees. An agent's shell, and the test runs made for it,
//!   run in a sandbox (bubblewrap on Linux, Seatbelt on macOS): writes stay
//!   in its own worktree (plus git's object store and a private /tmp),
//!   Backspace's own folder and data are unreadable, the policy file, git's
//!   hooks and config and the sealed tests are read-only, secrets are taken
//!   out of the environment, and the network can be switched off.
//! * Executors respect allowlists: a coding-CLI worker runs in the same
//!   sandbox, so the model the gateway approved is the only one it is handed
//!   credentials for, and it cannot reach the policy or its lock.
//! * Where no sandbox can run, `mode = "required"` refuses the command;
//!   `auto` runs it with the environment scrubbed and says so.
//!
//! Tests that need a real sandbox say so and are skipped where there is none,
//! unless BACKSPACE_SANDBOX_TESTS=require (set in CI), which makes a missing
//! sandbox a failure.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use backspace_core::sandbox::{self, Backend, Jail, Profile, Sandbox, SandboxConfig, SandboxMode};
use backspace_core::tools::Workspace;
use backspace_core::{AgentStatus, Config, Harness, ProjectState};
use serde_json::{json, Value};

// ------------------------------------------------------------------ helpers

/// Point the sandbox at this package's launcher (Windows) before anything
/// probes for a backend: test binaries do not live beside it.
fn init() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::env::set_var("BACKSPACE_JAIL", env!("CARGO_BIN_EXE_backspace-jail")));
}

fn isolating() -> bool {
    init();
    let ok = Backend::probe().isolates();
    if !ok && std::env::var("BACKSPACE_SANDBOX_TESTS").as_deref() == Ok("require") {
        panic!("BACKSPACE_SANDBOX_TESTS=require but no sandbox backend works here ({})", Backend::probe().label());
    }
    if !ok {
        eprintln!("skipped: no sandbox backend here ({})", Backend::probe().label());
    }
    ok
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A project with a committed app.txt and backspace.toml, Backspace's own
/// folder holding state, and a worker's worktree under it.
struct Fixture {
    root: PathBuf,
    work: PathBuf,
    outside: PathBuf,
}

fn fixture(tag: &str) -> Fixture {
    init();
    let base = std::env::temp_dir().join(format!("bs-sbx-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let root = base.join("project");
    let outside = base.join("elsewhere");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.email", "t@example.com"]);
    git(&root, &["config", "user.name", "t"]);
    std::fs::write(root.join("app.txt"), "hello\n").unwrap();
    std::fs::write(root.join("backspace.toml"), "# policy\n").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    std::fs::create_dir_all(root.join(".backspace")).unwrap();
    std::fs::write(root.join(".backspace/.gitignore"), "*\n").unwrap();
    std::fs::write(root.join(".backspace/state.json"), "{\"secret\": \"state-7731\"}").unwrap();
    let work = root.join(".backspace/worktrees/t1");
    git(&root, &["worktree", "add", "-q", "-b", "backspace/t1", work.to_str().unwrap()]);
    Fixture { root, work, outside }
}

fn jailed(f: &Fixture, cfg: &SandboxConfig) -> Workspace {
    let sandbox = Sandbox::new(cfg);
    let profile = Profile::agent(&f.root, &f.work, cfg);
    Workspace::new(f.work.clone(), Duration::from_secs(20)).jailed(Arc::new(Jail { sandbox, profile }))
}

/// A path as bash wants it (Git's bash on Windows reads C:/x, not C:\x).
fn u(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
}

fn sh(ws: &Workspace, cmd: &str) -> (i32, String) {
    rt().block_on(ws.sh(cmd)).unwrap()
}

// ------------------------------------------------------- environment

#[test]
fn secret_looking_variables_are_named_by_pattern() {
    for k in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "GITHUB_TOKEN", "GH_TOKEN", "AWS_SECRET_ACCESS_KEY", "NPM_TOKEN", "DB_PASSWORD", "MY_CLIENT_SECRET", "BACKSPACE_BRIDGE_TOKEN", "AWS_SESSION_TOKEN", "GOOGLE_APPLICATION_CREDENTIALS"] {
        assert!(sandbox::is_secret_name(k), "{k} should count as a secret");
    }
    for k in ["PATH", "HOME", "CARGO_HOME", "RUST_LOG", "LANG", "TERM", "USER", "KEYBOARD_LAYOUT"] {
        assert!(!sandbox::is_secret_name(k), "{k} should not count as a secret");
    }
}

#[test]
fn an_agents_shell_gets_no_secrets_from_the_environment() {
    std::env::set_var("BS_M6_ENV_API_KEY", "leak-41");
    std::env::set_var("BS_M6_ENV_PLAIN", "plain-ok");
    let f = fixture("env");
    let (code, out) = sh(&jailed(&f, &SandboxConfig::default()), "echo \"[$BS_M6_ENV_API_KEY|$BS_M6_ENV_PLAIN]\"");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("[|plain-ok]"), "the key must be gone and ordinary variables kept: {out}");
}

#[test]
fn a_variable_the_project_passes_on_is_kept() {
    std::env::set_var("BS_M6_PASS_TOKEN", "wanted-9");
    let f = fixture("pass");
    let cfg = SandboxConfig { pass_env: vec!["BS_M6_PASS_TOKEN".into()], ..SandboxConfig::default() };
    let (_, out) = sh(&jailed(&f, &cfg), "echo \"[$BS_M6_PASS_TOKEN]\"");
    assert!(out.contains("[wanted-9]"), "{out}");
}

#[test]
fn mode_off_runs_the_shell_as_before() {
    std::env::set_var("BS_M6_OFF_API_KEY", "visible-3");
    let f = fixture("off");
    let cfg = SandboxConfig { mode: SandboxMode::Off, ..SandboxConfig::default() };
    let (_, out) = sh(&jailed(&f, &cfg), "echo \"[$BS_M6_OFF_API_KEY]\"");
    assert!(out.contains("[visible-3]"), "{out}");
}

#[test]
fn required_with_no_sandbox_refuses_the_command() {
    let f = fixture("req");
    let cfg = SandboxConfig { mode: SandboxMode::Required, ..SandboxConfig::default() };
    let sandbox = Sandbox::with_backend(SandboxMode::Required, Backend::Plain);
    let profile = Profile::agent(&f.root, &f.work, &cfg);
    let marker = f.work.join("ran.txt");
    let ws = Workspace::new(f.work.clone(), Duration::from_secs(5)).jailed(Arc::new(Jail { sandbox, profile }));
    let err = rt().block_on(ws.sh("echo x > ran.txt")).unwrap_err().to_string();
    assert!(err.contains("sandbox"), "the refusal names the sandbox: {err}");
    assert!(!marker.exists(), "a refused command does not run");
}

#[test]
fn the_shipped_config_sandboxes_in_auto_mode_with_the_network_on() {
    let cfg = Config::parse(backspace_core::config::DEFAULT_CONFIG).unwrap();
    assert_eq!(cfg.sandbox.mode, SandboxMode::Auto);
    assert!(cfg.sandbox.network);
    assert_eq!(cfg.sandbox, SandboxConfig::default());
}

#[test]
fn an_agents_profile_hides_backspace_data_and_credentials() {
    let f = fixture("prof");
    let p = Profile::agent(&f.root, &f.work, &SandboxConfig::default());
    assert!(p.hidden.iter().any(|h| h.ends_with(".backspace")), "{:?}", p.hidden);
    assert!(p.hidden.contains(&backspace_core::prefs::Prefs::data_dir()), "{:?}", p.hidden);
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        for d in [".ssh", ".aws", ".gnupg", ".netrc", ".git-credentials", ".config/gh", ".claude", ".codex"] {
            assert!(p.hidden.contains(&home.join(d)), "{d} not hidden: {:?}", p.hidden);
        }
    }
    assert!(p.read_only.iter().any(|r| r.ends_with("backspace.toml")), "{:?}", p.read_only);
    assert!(p.scrub_env);
    // A CLI keeps its own login, and the network it needs for its model.
    let c = Profile::cli(&f.root, &f.work, Path::new("/usr/bin/true"), &SandboxConfig::default());
    assert!(!c.scrub_env && c.network);
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        assert!(!c.hidden.contains(&home.join(".claude")) && c.hidden.contains(&home.join(".ssh")));
    }
}

// ------------------------------------------------------ the filesystem

#[test]
fn an_agent_cannot_read_backspaces_own_folder() {
    if !isolating() {
        return;
    }
    let f = fixture("state");
    let ws = jailed(&f, &SandboxConfig::default());
    let (_, out) = sh(&ws, &format!("cat {}/.backspace/state.json; cat ../../state.json; ls ..", u(&f.root)));
    assert!(!out.contains("state-7731"), "Backspace's state leaked: {out}");
    let (code, out) = sh(&ws, "cat app.txt");
    assert_eq!((code, out.trim()), (0, "hello"), "its own files are readable");
}

#[test]
fn writes_stay_in_the_agents_worktree() {
    if !isolating() {
        return;
    }
    let f = fixture("writes");
    let ws = jailed(&f, &SandboxConfig::default());
    sh(&ws, &format!("echo x > {}/main.txt; echo x > {}/f.txt; echo y > new.txt; mkdir -p d && echo z > d/e.txt", u(&f.root), u(&f.outside)));
    assert!(!f.root.join("main.txt").exists(), "wrote into the main checkout");
    assert!(!f.outside.join("f.txt").exists(), "wrote outside the project");
    assert_eq!(std::fs::read_to_string(f.work.join("new.txt")).unwrap(), "y\n");
    assert_eq!(std::fs::read_to_string(f.work.join("d/e.txt")).unwrap(), "z\n");
    let (code, out) = sh(&ws, "t=$(mktemp) && echo ok > $t && cat $t");
    assert_eq!((code, out.trim()), (0, "ok"), "a private temp folder works");
}

#[test]
fn the_policy_file_is_read_only_inside_the_sandbox() {
    if !isolating() {
        return;
    }
    let f = fixture("policy");
    let ws = jailed(&f, &SandboxConfig::default());
    let (_, out) = sh(&ws, "echo evil >> backspace.toml; sed -i s/policy/open/ backspace.toml; rm -f backspace.toml; cat backspace.toml");
    assert_eq!(std::fs::read_to_string(f.work.join("backspace.toml")).unwrap(), "# policy\n", "{out}");
    assert_eq!(std::fs::read_to_string(f.root.join("backspace.toml")).unwrap(), "# policy\n");
}

#[test]
fn sealed_tests_are_read_only_inside_the_sandbox() {
    if !isolating() {
        return;
    }
    let f = fixture("sealed");
    std::fs::create_dir_all(f.work.join("tests")).unwrap();
    std::fs::write(f.work.join("tests/a1.sh"), "exit 1\n").unwrap();
    let cfg = SandboxConfig::default();
    let mut profile = Profile::agent(&f.root, &f.work, &cfg);
    profile.read_only.push(f.work.join("tests/a1.sh"));
    let ws = Workspace::new(f.work.clone(), Duration::from_secs(20)).jailed(Arc::new(Jail { sandbox: Sandbox::new(&cfg), profile }));
    sh(&ws, "echo 'exit 0' > tests/a1.sh; rm -f tests/a1.sh");
    assert_eq!(std::fs::read_to_string(f.work.join("tests/a1.sh")).unwrap(), "exit 1\n");
}

#[test]
fn git_works_in_the_worktree_but_its_hooks_and_config_cannot_be_planted() {
    if !isolating() {
        return;
    }
    let f = fixture("git");
    let ws = jailed(&f, &SandboxConfig::default());
    let (code, out) = sh(&ws, "echo more >> app.txt && git add app.txt && git -c user.email=a@b -c user.name=a -c commit.gpgsign=false commit -qm work && git log --oneline | head -1");
    if Backend::probe().agents_commit() {
        assert_eq!(code, 0, "git commit in the worktree works: {out}");
        assert!(out.contains("work"), "{out}");
    } else {
        // Windows: git's store stays read-only to agents; Backspace commits.
        let (code, out) = sh(&ws, "git status --short && git diff --stat");
        assert_eq!(code, 0, "git reads the worktree: {out}");
        assert!(out.contains("app.txt"), "{out}");
    }
    let hooks = f.root.join(".git/hooks");
    sh(&ws, &format!("echo 'touch /tmp/pwned' > {0}/post-commit; chmod +x {0}/post-commit; git config core.hooksPath /tmp; git config --worktree core.fsmonitor 'touch /tmp/pwned'", u(&hooks)));
    assert!(!hooks.join("post-commit").exists(), "a hook was planted where Backspace's own git would run it");
    let config = std::fs::read_to_string(f.root.join(".git/config")).unwrap();
    assert!(!config.contains("hooksPath") && !config.contains("fsmonitor"), "{config}");
}

#[test]
fn a_hidden_path_from_the_config_is_unreadable() {
    if !isolating() {
        return;
    }
    let f = fixture("hide");
    std::fs::write(f.outside.join("token.txt"), "tok-5521").unwrap();
    let cfg = SandboxConfig { hide: vec![f.outside.display().to_string()], ..SandboxConfig::default() };
    let (_, out) = sh(&jailed(&f, &cfg), &format!("cat {}/token.txt", u(&f.outside)));
    assert!(!out.contains("tok-5521"), "{out}");
}

// ---------------------------------------------------------- the network

fn listener() -> (TcpListener, u16) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn({
        let l = l.try_clone().unwrap();
        move || {
            for s in l.incoming().flatten() {
                drop(s);
            }
        }
    });
    (l, port)
}

#[test]
fn with_the_network_off_nothing_can_be_reached() {
    if !isolating() {
        return;
    }
    let f = fixture("net");
    let (_l, port) = listener();
    let probe = format!("(exec 3<>/dev/tcp/127.0.0.1/{port}) 2>/dev/null && echo connected || echo refused");
    let (_, on) = sh(&jailed(&f, &SandboxConfig::default()), &probe);
    assert!(on.contains("connected"), "network on: {on}");
    if !Backend::probe().blocks_network() {
        eprintln!("{} cannot switch the network off", Backend::probe().label());
        return;
    }
    let off = SandboxConfig { network: false, ..SandboxConfig::default() };
    let (_, out) = sh(&jailed(&f, &off), &probe);
    assert!(out.contains("refused") && !out.contains("connected"), "network off: {out}");
}

// ---------------------------------------------------- coding-CLI executors

#[test]
fn a_cli_executor_keeps_its_login_but_not_backspaces_state() {
    if !isolating() {
        return;
    }
    std::env::set_var("BS_M6_CLI_API_KEY", "cli-login-8");
    let f = fixture("cli");
    let cfg = SandboxConfig::default();
    let sandbox = Sandbox::new(&cfg);
    let shell = if cfg!(windows) { r"C:\Program Files\Git\bin\bash.exe" } else { "/bin/sh" };
    let profile = Profile::cli(&f.root, &f.work, Path::new(shell), &cfg);
    let script = format!("echo \"[$BS_M6_CLI_API_KEY]\"; cat {}/.backspace/state.json; echo x > {}/main.txt; echo ok > mine.txt", u(&f.root), u(&f.root));
    let w = sandbox.wrap(&profile, &[shell.into(), "-c".into(), script]).unwrap();
    let mut cmd = Command::new(&w.argv[0]);
    cmd.args(&w.argv[1..]).current_dir(&f.work);
    if let Some(env) = &w.env {
        cmd.env_clear().envs(env.iter().cloned());
    }
    let out = cmd.output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("[cli-login-8]"), "the CLI keeps the login it was given: {text}");
    assert!(!text.contains("state-7731"), "{text}");
    assert!(!f.root.join("main.txt").exists());
    assert!(f.work.join("mine.txt").exists());
}

// --------------------------------------------------------- the harness

struct Mock {
    seen: Arc<Mutex<Vec<Value>>>,
    port: u16,
}

fn mock(respond: impl Fn(&Value) -> Value + Send + Sync + 'static) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (s2, respond) = (seen.clone(), Arc::new(respond));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let (seen, respond) = (s2.clone(), respond.clone());
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                if len == 0 {
                    return;
                }
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let req: Value = serde_json::from_slice(&body).unwrap();
                seen.lock().unwrap().push(req.clone());
                let out = respond(&req).to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).unwrap();
            });
        }
    });
    Mock { seen, port }
}

fn call(name: &str, args: Value) -> Value {
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
        {"id": format!("call_{n}"), "type": "function", "function": {"name": name, "arguments": args.to_string()}}]}}],
        "usage": {"prompt_tokens": 100, "completion_tokens": 10}})
}

fn say(t: &str) -> Value {
    json!({"choices": [{"finish_reason": "stop", "message": {"content": t}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
}

fn config_text(port: u16) -> String {
    format!(
        r#"
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"
pin_main = "mock-mid"
pin_sub = "mock-mid"

[orchestrator]
max_parallel_calls = 4
max_subagents = 4
auto_approve = true
bash_timeout_secs = 20

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
api_key_env = "NONE"

[[models]]
id = "mock-mid"
provider = "mock"
tier = 2
description = "mid"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000
"#
    )
}

fn wait(h: &Harness, what: &str, done: impl Fn(&ProjectState) -> bool) -> ProjectState {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let s = h.snapshot();
        if done(&s) {
            return s;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The lead's own shell, in a real project, runs in the sandbox: it gets no
/// secrets from the environment and cannot read Backspace's state.
#[test]
fn the_harness_runs_an_agents_bash_in_the_sandbox() {
    init();
    std::env::set_var("BS_M6_E2E_API_KEY", "e2e-key-6");
    let m = mock(|req| {
        let msgs = req["messages"].as_array().unwrap();
        let ran = msgs.iter().any(|m| m["role"] == "tool");
        if ran {
            say("done")
        } else {
            call("bash", json!({"command": "echo \"key=[$BS_M6_E2E_API_KEY]\"; cat ../../state.json 2>&1 | head -c 300; echo; echo sandbox-level=$BACKSPACE_SANDBOX"}))
        }
    });
    let ws = std::env::temp_dir().join(format!("bs-sbx-h-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("README.md"), "A project.\n").unwrap();
    std::fs::write(ws.join("backspace.toml"), config_text(m.port)).unwrap();
    let h = Harness::open(ws.clone()).unwrap();
    h.send("run the check");
    wait(&h, "the main agent to finish", |s| s.agents[0].status == AgentStatus::Idle && m.seen.lock().unwrap().len() >= 2);
    let seen = m.seen.lock().unwrap().clone();
    let result = seen.iter().flat_map(|r| r["messages"].as_array().unwrap().clone()).find(|m| m["role"] == "tool").unwrap()["content"].as_str().unwrap().to_string();
    assert!(result.contains("key=[]"), "the agent's shell got a secret: {result}");
    let level = Backend::probe().label();
    assert!(result.contains(&format!("sandbox-level={level}")), "the shell says which sandbox it is in: {result}");
    if isolating() {
        assert!(!result.contains("\"agents\""), "Backspace's state leaked: {result}");
    }
    let s = h.snapshot();
    assert!(s.agents[0].log.iter().any(|l| l.text.contains("sandbox:")), "the log says which sandbox runs");
}

// ------------------------------------------- Rampart (M5 decisions 1 and 2)

/// Rampart gets a shell only where the sandbox isolates it.
#[test]
fn rampart_gets_a_shell_only_inside_a_real_sandbox() {
    use backspace_core::rampart;
    assert_eq!(rampart::tools(false), vec!["read", "list", "submit_ramparts"]);
    assert_eq!(rampart::tools(true), vec!["read", "list", "bash", "submit_ramparts"]);
    let f = fixture("rampart");
    let p = Profile::rampart(&f.root, &f.work, &SandboxConfig::default());
    assert!(!p.network, "Rampart's shell has no network");
    assert!(p.scrub_env);
}

/// Rampart prefers a model other than the lead's, without requiring one.
#[test]
fn rampart_prefers_a_model_other_than_the_leads() {
    use backspace_core::config::ModelSpec;
    use backspace_core::rampart;
    let m = |id: &str, tier: u8, out: f64| ModelSpec {
        id: id.into(),
        provider: "mock".into(),
        tier,
        description: String::new(),
        input_usd_per_mtok: 1.0,
        output_usd_per_mtok: out,
        max_output_tokens: 1000,
        model: None,
        efforts: vec![],
        class: None,
    };
    let (a, b) = (m("mid-a", 2, 2.0), m("mid-b", 2, 3.0));
    let both = [&a, &b];
    assert_eq!(rampart::pick_cheap(&both, "mid-a").unwrap().id, "mid-b", "another model when one is allowed");
    assert_eq!(rampart::pick_cheap(&both, "other").unwrap().id, "mid-a", "otherwise the best cheap one");
    assert_eq!(rampart::pick_cheap(&[&a], "mid-a").unwrap().id, "mid-a", "the same model when it is the only one");
    assert!(rampart::pick_cheap(&[], "mid-a").is_none());
}
