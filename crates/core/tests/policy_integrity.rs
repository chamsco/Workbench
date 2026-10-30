//! M1 hardening, F3: an agent with a shell cannot change the policy the next
//! session runs under. The policy a project adopted is kept outside the
//! workspace; a workspace file that loosens it is ignored with a warning until
//! a person adopts it; and a change an agent makes to the file or the lock
//! during an action is undone and said so.
//!
//! What this does not cover, on purpose, is in docs/cauldron/M1-HARDENING-NOTES.md:
//! a process an agent leaves running can write after the last check, and only
//! a sandbox (M6) or a separate OS user stops that.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use backspace_core::gateway::{adopt_policy, lock_path, resolve_policy, NoticeLevel, PolicyConfig};
use backspace_core::{Harness, LogKind, ProjectState};
use serde_json::{json, Value};

static ENV: Once = Once::new();

fn env() {
    ENV.call_once(|| {
        let home = std::env::temp_dir().join(format!("bs-integrity-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("BACKSPACE_DATA", home.join("data"));
        std::env::remove_var("BACKSPACE_CONFIG");
    });
}

fn pol(src: &str) -> PolicyConfig {
    #[derive(serde::Deserialize)]
    struct Wrap {
        #[serde(default)]
        policy: PolicyConfig,
    }
    toml::from_str::<Wrap>(src).unwrap().policy
}

fn dir(tag: &str) -> PathBuf {
    env();
    let ws = std::env::temp_dir().join(format!("bs-integrity-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::canonicalize(&ws).unwrap()
}

fn warnings(notices: &[backspace_core::gateway::PolicyNotice]) -> String {
    notices.iter().filter(|n| n.level == NoticeLevel::Warning).map(|n| n.text.clone()).collect::<Vec<_>>().join("\n")
}

const FULL: &str = "[policy]\nmode = \"enforced\"\norchestration_models = [\"a\"]\nexecution_models = [\"a\", \"b\"]\nskills = [\"tdd\"]\nplugins = []\n[policy.budget]\nmax_usd = 2.0\nmax_tokens = 100000\n";

// ---------------------------------------------------------------- the lock

#[test]
fn the_first_open_adopts_the_file_and_keeps_the_lock_outside_the_workspace() {
    let ws = dir("first");
    let file = pol(FULL);
    let (policy, notices) = resolve_policy(&ws, &file).unwrap();
    assert_eq!(policy, file);
    assert!(warnings(&notices).is_empty(), "{notices:?}");
    let lock = lock_path(&ws);
    assert!(lock.is_file(), "{} was not written", lock.display());
    assert!(!lock.starts_with(&ws), "the lock is where the workspace's own tools can't write it by accident");
    let text = std::fs::read_to_string(&lock).unwrap();
    assert!(text.contains("execution_models") && text.contains("max_tokens"), "{text}");
}

#[test]
fn each_workspace_has_its_own_lock() {
    let (a, b) = (dir("lock-a"), dir("lock-b"));
    assert_ne!(lock_path(&a), lock_path(&b));
    resolve_policy(&a, &pol(FULL)).unwrap();
    resolve_policy(&b, &pol("[policy]\nmode = \"enforced\"\norchestration_models = []\nexecution_models = []\nskills = []\nplugins = []\n")).unwrap();
    assert_eq!(resolve_policy(&a, &pol(FULL)).unwrap().0, pol(FULL), "b's lock does not leak into a");
}

#[test]
fn tightening_is_adopted_and_loosening_is_ignored_with_a_warning() {
    let ws = dir("tighten");
    resolve_policy(&ws, &pol(FULL)).unwrap();

    // Stricter in every way a file can be: fewer models, a lower budget.
    let tighter = pol(&FULL.replace("[\"a\", \"b\"]", "[\"a\"]").replace("2.0", "1.0"));
    let (policy, notices) = resolve_policy(&ws, &tighter).unwrap();
    assert_eq!(policy, tighter);
    assert!(warnings(&notices).is_empty(), "{notices:?}");
    assert!(std::fs::read_to_string(lock_path(&ws)).unwrap().contains("1.0"), "the lock followed it");

    // Back to the old, wider file: that is a loosening of what is adopted now.
    let (policy, notices) = resolve_policy(&ws, &pol(FULL)).unwrap();
    assert_eq!(policy, tighter, "the wider file is not in force");
    let w = warnings(&notices);
    assert!(w.contains("execution_models") && w.contains("`b`"), "it says what it ignored: {w}");
    assert!(w.contains("max_usd"), "{w}");
    assert!(w.contains("policy adopt"), "and how a person accepts it: {w}");
}

#[test]
fn every_way_of_loosening_is_ignored() {
    let ws = dir("loosen-all");
    let locked = pol(FULL);
    resolve_policy(&ws, &locked).unwrap();
    let cases: Vec<(&str, String)> = vec![
        ("a model added", FULL.replace("orchestration_models = [\"a\"]", "orchestration_models = [\"a\", \"z\"]")),
        ("a list removed", FULL.replace("execution_models = [\"a\", \"b\"]\n", "")),
        ("a skill added", FULL.replace("skills = [\"tdd\"]", "skills = [\"tdd\", \"grilling\"]")),
        ("a plugin added", FULL.replace("plugins = []", "plugins = [\"x\"]")),
        ("the usd limit raised", FULL.replace("max_usd = 2.0", "max_usd = 20.0")),
        ("the usd limit removed", FULL.replace("max_usd = 2.0\n", "")),
        ("the token limit removed", FULL.replace("max_tokens = 100000\n", "")),
        ("enforced dropped", FULL.replace("mode = \"enforced\"\n", "")),
        ("legacy open", FULL.replace("mode = \"enforced\"", "mode = \"legacy_open\"")),
        ("no policy at all", String::new()),
    ];
    for (what, src) in cases {
        let (policy, notices) = resolve_policy(&ws, &pol(&src)).unwrap();
        assert_eq!(policy, locked, "{what}: the adopted policy stays");
        assert!(!warnings(&notices).is_empty(), "{what}: and it is said");
    }
}

#[test]
fn a_file_that_tightens_one_thing_and_loosens_another_is_not_adopted_in_part() {
    let ws = dir("mixed");
    let locked = pol(FULL);
    resolve_policy(&ws, &locked).unwrap();
    let mixed = pol(&FULL.replace("orchestration_models = [\"a\"]", "orchestration_models = []").replace("plugins = []", "plugins = [\"x\"]"));
    let (policy, notices) = resolve_policy(&ws, &mixed).unwrap();
    assert_eq!(policy, locked, "nothing from a file that loosens anything");
    assert!(warnings(&notices).contains("plugins"));
}

#[test]
fn a_lock_that_cannot_be_read_stops_the_project_opening_rather_than_loosening_it() {
    let ws = dir("corrupt");
    resolve_policy(&ws, &pol(FULL)).unwrap();
    std::fs::write(lock_path(&ws), "this is = not [valid toml").unwrap();
    let err = resolve_policy(&ws, &pol(&FULL.replace("[\"a\", \"b\"]", "[\"a\", \"b\", \"c\"]"))).unwrap_err();
    let e = format!("{err:#}");
    assert!(e.contains("policy lock") && e.contains(&lock_path(&ws).display().to_string()), "it says which file and what to do: {e}");
    // Not a quiet fall-back to whatever the file says: the user decides, by removing the lock.
    assert!(std::fs::read_to_string(lock_path(&ws)).unwrap().contains("not [valid"), "and it did not overwrite the lock");
}

#[test]
fn a_person_adopting_a_loosening_makes_it_take_effect_and_stay() {
    let ws = dir("adopt");
    resolve_policy(&ws, &pol(FULL)).unwrap();
    let wider = pol(&FULL.replace("[\"a\", \"b\"]", "[\"a\", \"b\", \"c\"]"));
    assert_eq!(resolve_policy(&ws, &wider).unwrap().0, pol(FULL), "ignored until adopted");
    adopt_policy(&ws, &wider).unwrap();
    let (policy, notices) = resolve_policy(&ws, &wider).unwrap();
    assert_eq!(policy, wider);
    assert!(warnings(&notices).is_empty(), "{notices:?}");
    assert_eq!(resolve_policy(&ws, &wider).unwrap().0, wider, "and it stays adopted on the next open");
}

#[test]
fn ignoring_a_loosening_does_not_wear_off() {
    let ws = dir("persist");
    resolve_policy(&ws, &pol(FULL)).unwrap();
    let wider = pol(&FULL.replace("plugins = []", "plugins = [\"x\"]"));
    for _ in 0..5 {
        let (policy, notices) = resolve_policy(&ws, &wider).unwrap();
        assert_eq!(policy.plugins, Some(vec![]));
        assert!(warnings(&notices).contains("plugins"), "every open says it");
    }
}

// ---------------------------------------------------- an agent with a shell

type Seen = Arc<Mutex<Vec<Value>>>;

fn mock(script: impl Fn(usize) -> Value + Send + Sync + 'static) -> (u16, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen: Seen = Arc::default();
    let (seen2, script) = (seen.clone(), Arc::new(script));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let (seen, script) = (seen2.clone(), script.clone());
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
                let n = {
                    let mut g = seen.lock().unwrap();
                    g.push(req);
                    g.len() - 1
                };
                let out = script(n).to_string();
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    (port, seen)
}

fn bash_then_done(command: String) -> impl Fn(usize) -> Value + Send + Sync + 'static {
    move |n| match n {
        0 => json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
            {"id": "c1", "type": "function", "function": {"name": "bash", "arguments": json!({"command": command}).to_string()}}]}}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}}),
        _ => json!({"choices": [{"finish_reason": "stop", "message": {"content": "done"}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}}),
    }
}

const ENFORCED: &str = "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-big\"]\nexecution_models = [\"mock-big\"]\nskills = []\nplugins = []\n";

fn write_project(ws: &Path, port: u16, policy: &str) -> String {
    let cfg = format!(
        r#"[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"
pin_main = "mock-big"
pin_sub = "mock-small"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
api_key_env = "NONE"

[[models]]
id = "mock-small"
provider = "mock"
tier = 1
description = "cheap"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000

[[models]]
id = "mock-big"
provider = "mock"
tier = 4
description = "strong"
input_usd_per_mtok = 10.0
output_usd_per_mtok = 20.0
max_output_tokens = 1000

{policy}"#
    );
    std::fs::write(ws.join("backspace.toml"), &cfg).unwrap();
    cfg
}

fn wait(h: &Harness, what: &str, ok: impl Fn(&ProjectState) -> bool) -> ProjectState {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let s = h.snapshot();
        if ok(&s) {
            return s;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}: {:#?}", s.agents.iter().map(|a| (&a.key, a.status, &a.log)).collect::<Vec<_>>());
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn system_log(s: &ProjectState, agent: usize) -> String {
    s.agents[agent].log.iter().filter(|e| e.kind == LogKind::System).map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn bash_cannot_weaken_the_policy_for_the_next_session() {
    let ws = |p: &Path| p.display().to_string();
    let variants: Vec<(&str, Box<dyn Fn(&str) -> String>)> = vec![
        ("sed -i widens a list", Box::new(|ws| format!("sed -i 's/execution_models = \\[\"mock-big\"\\]/execution_models = [\"mock-big\", \"mock-small\"]/' {ws}/backspace.toml"))),
        ("a redirect drops a list", Box::new(|ws| format!("grep -v '^execution_models' {ws}/backspace.toml > /tmp/bs-int-edit && cat /tmp/bs-int-edit > {ws}/backspace.toml"))),
        ("mv and rewrite without the policy", Box::new(|ws| format!("mv {ws}/backspace.toml {ws}/backspace.toml.bak && sed '/^\\[policy\\]/,$d' {ws}/backspace.toml.bak > {ws}/backspace.toml"))),
    ];
    for (i, (what, command)) in variants.iter().enumerate() {
        let dir = dir(&format!("weaken-{i}"));
        let (port, seen) = mock(bash_then_done(command(&ws(&dir))));
        let original = write_project(&dir, port, ENFORCED);

        let h = Harness::open(dir.clone()).unwrap();
        h.send("tidy the config");
        wait(&h, "the second request, which carries the command's result", |_| seen.lock().unwrap().len() >= 2);
        assert_eq!(std::fs::read_to_string(dir.join("backspace.toml")).unwrap(), original, "{what}: the file is as it was");
        let log = system_log(&h.snapshot(), 0);
        assert!(log.contains("backspace.toml") && log.contains("restored"), "{what}: and the restoring is on the record: {log}");
        drop(h);

        // The next session, with no help from the file: the same limits.
        let (port2, seen2) = mock(|_| json!({"choices": [{"finish_reason": "stop", "message": {"content": "the provider should not have been called"}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1}}));
        write_project(&dir, port2, ENFORCED);
        let h2 = Harness::open(dir.clone()).unwrap();
        assert_eq!(h2.policy().execution_models, Some(vec!["mock-big".to_string()]), "{what}");
        h2.file_ticket("Crash on empty input", "The parser panics").unwrap();
        wait(&h2, "the triage agent's refusal", |s| s.agents.iter().any(|a| a.log.iter().any(|e| e.text.contains("refused by project policy"))));
        assert_eq!(seen2.lock().unwrap().len(), 0, "{what}: mock-small is still not an allowed execution model");
    }
}

#[test]
fn a_loosened_file_found_when_the_project_opens_is_not_in_force() {
    // Whatever wrote it (a background process, a merge, a person in an editor): the adopted policy rules.
    let dir = dir("found-loose");
    let (port, seen) = mock(|_| json!({"choices": [{"finish_reason": "stop", "message": {"content": "the provider should not have been called"}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1}}));
    write_project(&dir, port, ENFORCED);
    drop(Harness::open(dir.clone()).unwrap());

    write_project(&dir, port, &ENFORCED.replace("execution_models = [\"mock-big\"]", "execution_models = [\"mock-big\", \"mock-small\"]"));
    let h = Harness::open(dir.clone()).unwrap();
    assert_eq!(h.policy().execution_models, Some(vec!["mock-big".to_string()]));
    let log = system_log(&h.snapshot(), 0);
    assert!(log.contains("execution_models") && log.contains("policy adopt"), "{log}");
    h.file_ticket("Crash on empty input", "The parser panics").unwrap();
    wait(&h, "the refusal", |s| s.agents.iter().any(|a| a.log.iter().any(|e| e.text.contains("refused by project policy"))));
    assert_eq!(seen.lock().unwrap().len(), 0);
}

#[test]
fn the_lock_is_restored_after_a_command_that_rewrites_it() {
    let variants: Vec<(&str, Box<dyn Fn(&str) -> String>)> = vec![
        ("truncated", Box::new(|lock| format!(": > '{lock}'"))),
        ("widened", Box::new(|lock| format!("sed -i 's/mock-big/mock-big\", \"mock-small/' '{lock}'"))),
        ("deleted", Box::new(|lock| format!("rm -f '{lock}'"))),
    ];
    for (i, (what, command)) in variants.iter().enumerate() {
        let dir = dir(&format!("lock-{i}"));
        let lock = lock_path(&dir);
        let (port, seen) = mock(bash_then_done(command(&lock.display().to_string())));
        write_project(&dir, port, ENFORCED);
        let h = Harness::open(dir.clone()).unwrap();
        let before = std::fs::read(&lock).expect("the lock exists once the project is open");
        h.send("look around");
        wait(&h, "the second request", |_| seen.lock().unwrap().len() >= 2);
        assert_eq!(std::fs::read(&lock).unwrap(), before, "{what}: the lock is as it was");
        let log = system_log(&h.snapshot(), 0);
        assert!(log.contains("policy lock") && log.contains("restored"), "{what}: {log}");
    }
}

#[test]
fn a_command_that_leaves_the_policy_alone_is_not_reported() {
    let dir = dir("quiet");
    let (port, seen) = mock(bash_then_done("echo hello".into()));
    write_project(&dir, port, ENFORCED);
    let h = Harness::open(dir.clone()).unwrap();
    h.send("say hello");
    wait(&h, "the second request", |_| seen.lock().unwrap().len() >= 2);
    assert!(!system_log(&h.snapshot(), 0).contains("restored"));
}

#[test]
fn a_person_editing_the_file_between_agent_actions_is_not_undone() {
    // The guard answers for what happens during an agent's action, not for the file in general.
    let dir = dir("person");
    let (port, seen) = mock(bash_then_done("echo hello".into()));
    let original = write_project(&dir, port, ENFORCED);
    let h = Harness::open(dir.clone()).unwrap();
    let tighter = original.replace("execution_models = [\"mock-big\"]", "execution_models = []");
    std::fs::write(dir.join("backspace.toml"), &tighter).unwrap();
    h.send("say hello");
    wait(&h, "the second request", |_| seen.lock().unwrap().len() >= 2);
    assert_eq!(std::fs::read_to_string(dir.join("backspace.toml")).unwrap(), tighter);
}
