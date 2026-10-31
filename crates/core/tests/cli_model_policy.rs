//! M1 final hardening, section 3: the model a coding CLI actually runs.
//!
//! A CLI entry in `[[models]]` is one id in the project's lists, but the CLI
//! runs whatever model its own configuration, environment or defaults pick.
//! Under an enforced policy Backspace now does one of two things for every CLI
//! turn, planner or worker:
//!
//! * the entry pins a model (`model = "…"`) on a CLI whose command line is
//!   documented to win over its environment and settings (Claude Code
//!   `--model`, Codex `-m`): the pinned model is checked against the role's list
//!   and passed on the command line;
//! * it does not (no `model`, or a CLI whose precedence is not verified): the
//!   turn is refused unless `[policy] unpinned_cli_models` names the entry, which
//!   is the written exception.
//!
//! Legacy and open policies are untouched.
//!
//! Fake `claude`, `codex` and `cursor` scripts record the arguments and the
//! environment they were started with. They show what Backspace passes, not how
//! the real CLI then interprets it.
#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use backspace_core::config::{Config, DEFAULT_CONFIG};
use backspace_core::gateway::{Gateway, PolicyConfig, Refusal};
use backspace_core::{Harness, LogKind};
use serde_json::{json, Value};

static BIN: Once = Once::new();

const SCRIPT_HEAD: &str = r#"#!/bin/sh
cat > /dev/null
d="$PWD"
# a ticket worktree has a .git file; the workspace has the .git directory
while [ ! -d "$d/.git" ] && [ "$d" != "/" ]; do d=$(dirname "$d"); done
printf '%s\n' "$@" > "$d/cli-args.txt"
env | grep -E '^(ANTHROPIC_MODEL|CODEX_MODEL)=' > "$d/cli-env.txt"
echo made > out.txt
"#;

fn fakes() {
    BIN.call_once(|| {
        let home = std::env::temp_dir().join(format!("bs-climodel-home-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("BACKSPACE_DATA", home.join("data"));
        std::env::remove_var("BACKSPACE_CONFIG");
        // A person's shell (or a profile) naming a model of its own. The pin is on the command line.
        std::env::set_var("ANTHROPIC_MODEL", "hostile-env-model");
        std::env::set_var("CODEX_MODEL", "hostile-env-model");
        let bin = std::env::temp_dir().join(format!("bs-climodel-bin-{}", std::process::id()));
        std::fs::create_dir_all(&bin).unwrap();
        let claude = format!(
            "{SCRIPT_HEAD}echo '{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"fake-1\"}}'\necho '{{\"type\":\"result\",\"result\":\"made a file\",\"session_id\":\"fake-1\",\"total_cost_usd\":0.01,\"is_error\":false}}'\n"
        );
        let codex = format!(
            "{SCRIPT_HEAD}echo '{{\"type\":\"thread.started\",\"thread_id\":\"fake-cx\"}}'\necho '{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"made a file\"}}}}'\necho '{{\"type\":\"turn.completed\",\"usage\":{{\"input_tokens\":10,\"output_tokens\":5}}}}'\n"
        );
        for (name, body) in [("claude", claude.clone()), ("codex", codex), ("cursor", claude)] {
            let p = bin.join(name);
            std::fs::write(&p, body).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::env::set_var("BACKSPACE_CLI_PATH", &bin);
    });
}

fn tool_call(name: &str, args: Value) -> Value {
    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
        {"id": "call_1", "type": "function", "function": {"name": name, "arguments": args.to_string()}}]}}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
}

fn planner(listener: TcpListener) {
    let n = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        let n = n.clone();
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
            let out = match n.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                0 => tool_call(
                    "create_tickets",
                    json!({"tickets": [{"key": "job", "title": "Make a file", "what_to_build": "make a file",
                        "acceptance": ["a file exists"], "check": "ls out.txt"}]}),
                ),
                1 => tool_call("work_tickets", json!({})),
                _ => json!({"choices": [{"finish_reason": "stop", "message": {"content": "done"}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}}),
            }
            .to_string();
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
        });
    }
}

/// A coding-CLI entry: its id, the CLI behind it, and the model it pins (if any).
#[derive(Clone, Copy)]
struct Entry {
    id: &'static str,
    cli: &'static str,
    model: Option<&'static str>,
}

const CLAUDE_PINNED: Entry = Entry { id: "cc", cli: "claude", model: Some("claude-pin-1") };
const CLAUDE_UNPINNED: Entry = Entry { id: "cc", cli: "claude", model: None };
const CODEX_PINNED: Entry = Entry { id: "cx", cli: "codex", model: Some("codex-pin-1") };
const CODEX_UNPINNED: Entry = Entry { id: "cx", cli: "codex", model: None };

/// An enforced policy with these lists. `unpinned: None` writes no such list.
fn enforced(orch: &[&str], exec: &[&str], unpinned: Option<&[&str]>) -> String {
    let q = |l: &[&str]| format!("[{}]", l.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", "));
    let mut s = format!("[policy]\nmode = \"enforced\"\norchestration_models = {}\nexecution_models = {}\nskills = []\nplugins = []\n", q(orch), q(exec));
    if let Some(u) = unpinned {
        s.push_str(&format!("unpinned_cli_models = {}\n", q(u)));
    }
    s
}

fn config_text(port: u16, policy: &str, entries: &[Entry], pin_main: &str, pin_sub: &str) -> String {
    let mut clis: Vec<&str> = entries.iter().map(|e| e.cli).collect();
    clis.sort();
    clis.dedup();
    let providers: String = clis.iter().map(|c| format!("[providers.the-{c}]\nkind = \"cli\"\ncli = \"{c}\"\n\n")).collect();
    let models: String = entries
        .iter()
        .map(|e| {
            format!(
                "[[models]]\nid = \"{}\"\nprovider = \"the-{}\"\n{}tier = 3\ndescription = \"a coding CLI\"\ninput_usd_per_mtok = 0.0\noutput_usd_per_mtok = 0.0\nmax_output_tokens = 0\n\n",
                e.id,
                e.cli,
                e.model.map(|m| format!("model = \"{m}\"\n")).unwrap_or_default()
            )
        })
        .collect();
    format!(
        r#"
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"
pin_main = "{pin_main}"
pin_sub = "{pin_sub}"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
max_depth = 1
auto_approve = true
bash_timeout_secs = 10

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
api_key_env = "NONE"

{providers}[[models]]
id = "mock-mid"
provider = "mock"
tier = 2
description = "mid"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000

{models}{policy}
"#
    )
}

fn workspace(tag: &str) -> PathBuf {
    fakes();
    let ws = std::env::temp_dir().join(format!("bs-climodel-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    ws
}

fn mock_planner() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || planner(listener));
    port
}

fn project(tag: &str, policy: &str, entries: &[Entry], pin_main: &str, pin_sub: &str) -> (Harness, PathBuf) {
    let ws = workspace(tag);
    std::fs::write(ws.join("backspace.toml"), config_text(mock_planner(), policy, entries, pin_main, pin_sub)).unwrap();
    (Harness::open(ws.clone()).unwrap(), ws)
}

struct Ran {
    args: Vec<String>,
    env: String,
    log: String,
}

fn drive(h: &Harness, ws: &Path, message: &str) -> Ran {
    h.send(message);
    let file = ws.join("cli-args.txt");
    let deadline = Instant::now() + Duration::from_secs(40);
    while !file.is_file() {
        for a in h.snapshot().pending_approvals() {
            h.approve(a.id);
        }
        assert!(
            Instant::now() < deadline,
            "the CLI was never started: {:#?}",
            h.snapshot().agents.iter().map(|a| (&a.key, a.status, a.log.iter().map(|e| e.text.clone()).collect::<Vec<_>>())).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(30));
    }
    std::thread::sleep(Duration::from_millis(300));
    Ran {
        args: std::fs::read_to_string(&file).unwrap().lines().map(String::from).collect(),
        env: std::fs::read_to_string(ws.join("cli-env.txt")).unwrap_or_default(),
        log: h
            .snapshot()
            .agents
            .iter()
            .flat_map(|a| a.log.iter().filter(|e| e.kind == LogKind::System).map(|e| e.text.clone()))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// A worker turn: the planner is an API model, the ticket goes to `entry`.
fn worker(tag: &str, policy: &str, entries: &[Entry], entry: &str) -> Ran {
    let (h, ws) = project(tag, policy, entries, "mock-mid", entry);
    drive(&h, &ws, "cli-project: make a file")
}

/// A planner turn: the planner itself runs in the CLI.
fn planning(tag: &str, policy: &str, entries: &[Entry], entry: &str) -> Ran {
    let (h, ws) = project(tag, policy, entries, entry, entry);
    drive(&h, &ws, "hello")
}

/// What the agents logged as errors once `message` had time to be acted on, and whether a CLI ever started.
fn refused(tag: &str, policy: &str, entries: &[Entry], pin_main: &str, pin_sub: &str, message: &str) -> (String, bool) {
    let (h, ws) = project(tag, policy, entries, pin_main, pin_sub);
    h.send(message);
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut errs = String::new();
    while Instant::now() < deadline {
        for a in h.snapshot().pending_approvals() {
            h.approve(a.id);
        }
        errs = h.snapshot().agents.iter().flat_map(|a| a.log.iter().filter(|e| e.kind == LogKind::Error).map(|e| e.text.clone())).collect::<Vec<_>>().join("\n");
        if errs.contains("refused by project policy") {
            break;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    std::thread::sleep(Duration::from_millis(300));
    (errs, ws.join("cli-args.txt").is_file())
}

fn has_pair(args: &[String], flag: &str, value: &str) -> bool {
    args.windows(2).any(|w| w[0] == flag && w[1] == value)
}

fn model_flags(args: &[String]) -> Vec<&String> {
    args.windows(2).filter(|w| w[0] == "--model" || w[0] == "-m").map(|w| &w[1]).collect()
}

// ------------------------------------------------------------ the gateway

fn pol(src: &str) -> PolicyConfig {
    #[derive(serde::Deserialize)]
    struct Wrap {
        #[serde(default)]
        policy: PolicyConfig,
    }
    toml::from_str::<Wrap>(src).unwrap().policy
}

#[test]
fn under_an_enforced_policy_a_cli_entry_runs_unpinned_only_if_the_policy_says_so() {
    let g = Gateway::new(pol(&enforced(&["cc"], &["cc"], Some(&["cc"]))));
    assert!(g.check_cli_model("cc", false).is_ok());
    let err = g.check_cli_model("other", false).unwrap_err();
    assert!(matches!(err, Refusal::CliModel { .. }), "{err:?}");
    let text = err.to_string();
    assert!(text.contains("refused by project policy") && text.contains("other") && text.contains("unpinned_cli_models"), "{text}");
    // A pinned entry is not asking for the exception.
    assert!(g.check_cli_model("other", true).is_ok());
}

#[test]
fn an_enforced_policy_without_the_list_or_with_an_empty_one_accepts_no_unpinned_cli() {
    for u in [None, Some(&[][..])] {
        let g = Gateway::new(pol(&enforced(&["cc"], &["cc"], u)));
        assert!(g.check_cli_model("cc", false).is_err(), "{u:?}");
    }
}

#[test]
fn without_an_enforced_policy_unpinned_is_open_unless_a_list_says_otherwise() {
    for src in ["", "[policy]\nskills = [\"tdd\"]\n", "[policy]\nmode = \"legacy_open\"\n"] {
        assert!(Gateway::new(pol(src)).check_cli_model("anything", false).is_ok(), "`{src}`");
    }
    let g = Gateway::new(pol("[policy]\nunpinned_cli_models = [\"cc\"]\n"));
    assert!(g.check_cli_model("cc", false).is_ok() && g.check_cli_model("cx", false).is_err());
}

// ----------------------------------------------- a pinned model is checked and passed

#[test]
fn a_pinned_claude_code_entry_is_started_on_its_model() {
    let r = worker("pin-claude", &enforced(&["mock-mid"], &["cc", "claude-pin-1", "opus", "haiku"], None), &[CLAUDE_PINNED], "cc");
    assert!(has_pair(&r.args, "--model", "claude-pin-1"), "{:?}", r.args);
    assert_eq!(model_flags(&r.args).len(), 1, "one model, not two: {:?}", r.args);
}

#[test]
fn a_pinned_codex_entry_is_started_on_its_model() {
    let r = worker("pin-codex", &enforced(&["mock-mid"], &["cx", "codex-pin-1"], None), &[CODEX_PINNED], "cx");
    assert!(has_pair(&r.args, "-m", "codex-pin-1"), "{:?}", r.args);
    assert_eq!(model_flags(&r.args).len(), 1, "{:?}", r.args);
}

#[test]
fn the_environment_cannot_change_a_pinned_model() {
    // The process environment names a model of its own (set for this whole test binary). Backspace does
    // not rely on the CLI ignoring it: the model it validated is on the command line, which Claude Code and
    // Codex document as taking precedence. The fake CLI sees the variable, and the argument.
    let r = worker("pin-env", &enforced(&["mock-mid"], &["cc", "claude-pin-1", "opus", "haiku"], None), &[CLAUDE_PINNED], "cc");
    assert!(r.env.contains("ANTHROPIC_MODEL=hostile-env-model"), "the test is only meaningful if the CLI could see it: {:?}", r.env);
    assert!(has_pair(&r.args, "--model", "claude-pin-1"), "{:?}", r.args);
    assert!(!r.args.iter().any(|a| a.contains("hostile")), "{:?}", r.args);
}

#[test]
fn a_pinned_model_the_execution_list_does_not_name_is_refused_before_the_cli_starts() {
    // The entry itself is listed; the model it would run is not.
    let (errs, started) = refused("pin-worker-refused", &enforced(&["mock-mid"], &["cc"], None), &[CLAUDE_PINNED], "mock-mid", "cc", "cli-project: make a file");
    assert!(errs.contains("refused by project policy") && errs.contains("claude-pin-1") && errs.contains("execution"), "{errs}");
    assert!(!started, "Claude Code was never started");
}

#[test]
fn a_pinned_model_the_orchestration_list_does_not_name_is_refused_for_the_planner() {
    let (errs, started) = refused("pin-planner-refused", &enforced(&["mock-mid", "cc"], &["cc", "claude-pin-1"], None), &[CLAUDE_PINNED], "cc", "cc", "hello");
    assert!(errs.contains("refused by project policy") && errs.contains("claude-pin-1") && errs.contains("orchestration"), "{errs}");
    assert!(!started, "the planner never started");
}

#[test]
fn planner_and_worker_are_checked_against_their_own_lists() {
    // `claude-pin-1` is an orchestration model and not an execution one.
    let policy = enforced(&["mock-mid", "cc", "claude-pin-1", "opus", "haiku"], &["cc"], None);
    let r = planning("split-planner", &policy, &[CLAUDE_PINNED], "cc");
    assert!(has_pair(&r.args, "--model", "claude-pin-1"), "the planner runs: {:?}", r.args);
    let (errs, started) = refused("split-worker", &policy, &[CLAUDE_PINNED], "mock-mid", "cc", "cli-project: make a file");
    assert!(errs.contains("claude-pin-1") && errs.contains("execution"), "the worker does not: {errs}");
    assert!(!started);
}

#[test]
fn a_planner_is_checked_as_the_entry_it_was_pinned_to_not_the_first_one_on_that_cli() {
    // Two entries on Claude Code. `ok` is listed; `bad` pins a model no list names. The planner is `bad`.
    let ok = Entry { id: "ok", cli: "claude", model: Some("claude-ok-1") };
    let bad = Entry { id: "bad", cli: "claude", model: Some("claude-bad-1") };
    let policy = enforced(&["mock-mid", "ok", "claude-ok-1", "bad"], &["ok", "claude-ok-1", "bad"], None);
    let (errs, started) = refused("planner-entry", &policy, &[ok, bad], "bad", "ok", "hello");
    assert!(errs.contains("refused by project policy") && errs.contains("claude-bad-1"), "the pinned entry is the one checked: {errs}");
    assert!(!started);
}

// ------------------------------------------------------- an unpinned entry needs the exception

#[test]
fn an_unpinned_entry_under_an_enforced_policy_is_refused_unless_named() {
    for (what, entry, planner_cli) in [("claude worker", CLAUDE_UNPINNED, false), ("codex worker", CODEX_UNPINNED, false), ("claude planner", CLAUDE_UNPINNED, true), ("codex planner", CODEX_UNPINNED, true)] {
        let policy = enforced(&["mock-mid", entry.id], &[entry.id], None);
        let (pin_main, pin_sub, msg) = if planner_cli { (entry.id, entry.id, "hello") } else { ("mock-mid", entry.id, "cli-project: make a file") };
        let (errs, started) = refused(&format!("unpinned-{}", what.replace(' ', "-")), &policy, &[entry], pin_main, pin_sub, msg);
        assert!(errs.contains("refused by project policy") && errs.contains(entry.id) && errs.contains("unpinned_cli_models"), "{what}: {errs}");
        assert!(!started, "{what}: the CLI was never started");
    }
}

#[test]
fn an_entry_the_policy_names_as_unpinned_runs_without_a_model_argument_and_says_so() {
    for (what, entry) in [("claude", CLAUDE_UNPINNED), ("codex", CODEX_UNPINNED)] {
        let policy = enforced(&["mock-mid", entry.id], &[entry.id, "opus", "haiku"], Some(&[entry.id]));
        let r = worker(&format!("excepted-{what}"), &policy, &[entry], entry.id);
        assert!(model_flags(&r.args).is_empty(), "{what}: Backspace names no model, the CLI's own configuration does: {:?}", r.args);
        assert!(r.log.contains("unpinned_cli_models") || r.log.contains("its own configuration"), "{what}: the log records the exception: {}", r.log);
    }
}

#[test]
fn the_exception_is_per_entry() {
    let other = Entry { id: "other", cli: "claude", model: None };
    let policy = enforced(&["mock-mid", "cc", "other"], &["cc", "other"], Some(&["cc"]));
    let (errs, started) = refused("exception-per-entry", &policy, &[CLAUDE_UNPINNED, other], "mock-mid", "other", "cli-project: make a file");
    assert!(errs.contains("other") && errs.contains("unpinned_cli_models"), "{errs}");
    assert!(!started);
}

#[test]
fn a_pin_on_a_cli_whose_command_line_precedence_is_not_verified_counts_as_unpinned() {
    // The runner can pass `--model` to Cursor, but nothing here shows it beats the CLI's own settings.
    let cursor = Entry { id: "cu", cli: "cursor", model: Some("cursor-pin-1") };
    let policy = enforced(&["mock-mid", "cu"], &["cu", "cursor-pin-1"], None);
    let (errs, started) = refused("cursor-pin", &policy, &[cursor], "mock-mid", "cu", "cli-project: make a file");
    assert!(errs.contains("refused by project policy") && errs.contains("cu") && errs.contains("unpinned_cli_models"), "{errs}");
    assert!(!started);
}

#[test]
fn an_empty_model_is_not_a_pin() {
    let blank = Entry { id: "cc", cli: "claude", model: Some("") };
    let policy = enforced(&["mock-mid", "cc"], &["cc"], None);
    let (errs, started) = refused("blank-pin", &policy, &[blank], "mock-mid", "cc", "cli-project: make a file");
    assert!(errs.contains("unpinned_cli_models"), "{errs}");
    assert!(!started);
}

// ------------------------------------------------------------- the config on disk is not re-read

#[test]
fn editing_backspace_toml_after_the_project_opened_does_not_change_the_model() {
    let (h, ws) = project("edit-after-open", &enforced(&["mock-mid"], &["cc", "claude-pin-1", "opus", "haiku"], None), &[CLAUDE_PINNED], "mock-mid", "cc");
    // A later edit that pins a different model, and widens the lists to allow it.
    let edited = config_text(1, &enforced(&["mock-mid"], &["cc", "claude-evil-1", "opus", "haiku"], None), &[Entry { model: Some("claude-evil-1"), ..CLAUDE_PINNED }], "mock-mid", "cc");
    std::fs::write(ws.join("backspace.toml"), edited).unwrap();
    let r = drive(&h, &ws, "cli-project: make a file");
    assert!(has_pair(&r.args, "--model", "claude-pin-1"), "{:?}", r.args);
    assert!(!r.args.iter().any(|a| a.contains("evil")), "{:?}", r.args);
}

// -------------------------------------------------------------------------- legacy

#[test]
fn a_project_with_no_policy_runs_as_it_always_did() {
    // No [policy]: the entry runs, Backspace names no model, and nothing is asked of the person.
    let r = worker("legacy", "", &[CLAUDE_PINNED], "cc");
    assert!(model_flags(&r.args).is_empty(), "{:?}", r.args);
    let r = worker("legacy-unpinned", "", &[CLAUDE_UNPINNED], "cc");
    assert!(model_flags(&r.args).is_empty(), "{:?}", r.args);
    assert!(!r.log.contains("refused by project policy"), "{}", r.log);
}

#[test]
fn legacy_open_runs_unpinned_too() {
    let r = worker("legacy-open", "[policy]\nmode = \"legacy_open\"\n", &[CODEX_UNPINNED], "cx");
    assert!(model_flags(&r.args).is_empty(), "{:?}", r.args);
}

// ------------------------------------------------------ generated policies and notices

#[test]
fn a_generated_policy_names_the_cli_entries_that_run_unpinned_and_the_models_it_pins() {
    let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
    let p = cfg.effective_policy(&[]);
    let unpinned = p.unpinned_cli_models.clone().expect("a generated policy states the exception");
    for e in ["claude-code", "codex", "cursor", "grok", "opencode"] {
        assert!(unpinned.contains(&e.to_string()), "{e} is a CLI entry with no model: {unpinned:?}");
    }
    assert!(!unpinned.contains(&"claude-sonnet-5-5".to_string()), "an API model is not a CLI: {unpinned:?}");
    assert!(cfg.policy_init_block(&[]).contains("unpinned_cli_models = ["), "`policy init` writes it down");
    // An entry that pins a model: its model is a model the config uses, and it is not an exception.
    let src = format!("{DEFAULT_CONFIG}\n[[models]]\nid = \"pinned-cc\"\nprovider = \"claude-code\"\nmodel = \"claude-pin-1\"\ntier = 3\ndescription = \"x\"\ninput_usd_per_mtok = 0.0\noutput_usd_per_mtok = 0.0\nmax_output_tokens = 0\n");
    let p = Config::parse(&src).unwrap().effective_policy(&[]);
    assert!(p.execution_models.unwrap().contains(&"claude-pin-1".to_string()));
    assert!(p.orchestration_models.unwrap().contains(&"claude-pin-1".to_string()));
    assert!(!p.unpinned_cli_models.unwrap().contains(&"pinned-cc".to_string()));
}

#[test]
fn a_list_the_person_wrote_is_not_widened_by_the_generated_exception() {
    let src = format!("{DEFAULT_CONFIG}\n{}", enforced(&["claude-code"], &["claude-code"], None));
    let p = Config::parse(&src).unwrap().effective_policy(&[]);
    assert_eq!(p.unpinned_cli_models, Some(vec![]), "an enforced file with no such list accepts no unpinned CLI, and generating it does not change that");
    let src = format!("{DEFAULT_CONFIG}\n{}", enforced(&["claude-code"], &["claude-code"], Some(&["codex"])));
    let p = Config::parse(&src).unwrap().effective_policy(&[]);
    assert_eq!(p.unpinned_cli_models, Some(vec!["codex".to_string()]));
}

#[test]
fn a_new_project_lets_its_own_cli_entries_run() {
    let ws = workspace("new-project");
    let cfg = Config::new_project(&ws);
    let g = Gateway::new(cfg.policy.clone());
    assert!(g.check_cli_model("claude-code", false).is_ok(), "{:?}", cfg.policy.unpinned_cli_models);
}

#[test]
fn an_enforced_project_with_an_unpinned_cli_entry_it_does_not_name_says_so_when_it_opens() {
    let (h, _) = project("notice", &enforced(&["mock-mid", "cc"], &["cc"], None), &[CLAUDE_UNPINNED], "mock-mid", "cc");
    let notes = h.policy_notices().into_iter().map(|n| n.text).collect::<Vec<_>>().join("\n");
    assert!(notes.contains("unpinned_cli_models") && notes.contains("cc"), "{notes}");
}

#[test]
fn adding_an_unpinned_exception_is_a_loosening_that_waits_for_a_person() {
    use backspace_core::gateway::{adopt_policy, resolve_policy};
    let ws = std::fs::canonicalize(workspace("adopt")).unwrap();
    let locked = pol(&enforced(&["cc"], &["cc"], None));
    resolve_policy(&ws, &locked).unwrap();
    let wider = pol(&enforced(&["cc"], &["cc"], Some(&["cc"])));
    let (in_force, notices) = resolve_policy(&ws, &wider).unwrap();
    assert_eq!(in_force, locked, "the file that excepts a CLI is not in force");
    assert!(notices.iter().any(|n| n.text.contains("unpinned_cli_models") && n.text.contains("adopt")), "{notices:?}");
    assert!(adopt_policy(&ws, &wider).unwrap().iter().any(|l| l.contains("unpinned_cli_models")));
    assert_eq!(resolve_policy(&ws, &wider).unwrap().0, wider);
    // Narrowing it again needs no one.
    let (in_force, notices) = resolve_policy(&ws, &locked).unwrap();
    assert_eq!(in_force, locked);
    assert!(notices.is_empty(), "{notices:?}");
}
