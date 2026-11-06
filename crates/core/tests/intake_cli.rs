//! M3 acceptance for a planner that lives in a coding CLI. The CLI keeps its
//! own file and shell tools, and code cannot gate them until M6's sandbox, so
//! only the long path runs there:
//!
//! * intake (classify, ask) runs before the CLI is launched;
//! * research and one-shot are both refused before the CLI runs, whether the
//!   classifier chose them, the user answered them, or an earlier session saved
//!   them: no launch, no model call, no ticket, nothing half-started, and the
//!   task is not turned into a long one;
//! * a long task launches the CLI planner as it always did.
//!
//! The same one-shot works through the native/API planner (tests/intake.rs).
//!
//! Unix only: the fake `claude` is a shell script.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, Instant};

use backspace_core::{AgentStatus, Harness, LogKind, Overrides, ProjectState};

/// One fake `claude` for the whole file. It keeps what it saw, and what the
/// bridge answered, in its working directory (each test has its own).
fn fake_claude() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let bin = std::env::temp_dir().join(format!("bs-intake-cli-bin-{}", std::process::id()));
        std::fs::create_dir_all(&bin).unwrap();
        let script = r#"#!/bin/sh
# A CLI planner that only ever answers "ok" and keeps notes of what it was
# asked. A refused task must leave these notes absent: the CLI never ran.
n=$(( $(cat ./.n 2>/dev/null || echo 0) + 1 ))
echo $n > ./.n
input=$(cat)
printf '%s' "$*" > "./.args-$n"
printf '%s' "$input" > "./.in-$n"
printf '{"type":"result","result":"ok","session_id":"s1","total_cost_usd":0.01,"is_error":false}\n'
"#;
        let p = bin.join("claude");
        std::fs::write(&p, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("BACKSPACE_CLI_PATH", &bin);
    });
}

fn open(tag: &str) -> (Harness, PathBuf, PathBuf) {
    fake_claude();
    let ws = std::env::temp_dir().join(format!("bs-intake-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("README.md"), "Say teh word.\n").unwrap();
    std::fs::write(
        ws.join("backspace.toml"),
        r#"
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10

[intake]
enabled = true

[providers.claude-code]
kind = "cli"
cli = "claude"

[[models]]
id = "claude-code"
provider = "claude-code"
tier = 3
description = "Claude Code"
input_usd_per_mtok = 0.0
output_usd_per_mtok = 0.0
max_output_tokens = 0
"#,
    )
    .unwrap();
    let h = Harness::open_with(ws.clone(), Overrides { planner: Some("claude-code".into()), ..Default::default() }).unwrap();
    let run = ws.join(".backspace/worktrees/_run");
    (h, ws, run)
}

fn wait(h: &Harness, what: &str, done: impl Fn(&ProjectState) -> bool) -> ProjectState {
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        let s = h.snapshot();
        if done(&s) {
            return s;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:#?}",
            s.agents.iter().map(|a| (&a.key, a.status, &a.log)).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn launches(run: &Path) -> u32 {
    std::fs::read_to_string(run.join(".n")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

fn asked(s: &ProjectState) -> usize {
    s.agents[0]
        .log
        .iter()
        .filter(|l| {
            let t = l.text.to_lowercase();
            l.kind == LogKind::Assistant && t.contains("one-shot") && t.contains("long") && t.contains("research")
        })
        .count()
}

#[test]
fn an_unsure_task_is_asked_about_before_the_cli_is_launched() {
    let (h, _ws, run) = open("ask");
    h.send("hmm");
    wait(&h, "the question", |s| asked(s) == 1 && s.agents[0].status == AgentStatus::Idle);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(launches(&run), 0, "no CLI turn (and so no spend) before the user answers");
    h.send("long");
    wait(&h, "the CLI to run", |_| launches(&run) == 1);
    let said = std::fs::read_to_string(run.join(".in-1")).unwrap();
    assert!(said.contains("hmm"), "the CLI is given the task: {said}");
}

/// A research task in a coding CLI cannot be kept to "read and report": the
/// CLI has its own file and shell tools, and nothing in code gates them. So it
/// is refused before the CLI runs, and costs nothing.
fn refused_research(tag: &str, say: &[&str]) {
    let (h, ws, run) = open(tag);
    for (i, m) in say.iter().enumerate() {
        h.send(*m);
        if i + 1 < say.len() {
            wait(&h, "the question", |s| asked(s) == 1 && s.agents[0].status == AgentStatus::Idle);
        }
    }
    assert_refused(&h, &ws, &run, "research", &["research", "coding cli", "api model", "m6"]);
}

#[test]
fn a_cli_research_task_is_refused_before_the_cli_runs() {
    refused_research("research-refused", &["research the pros and cons of sqlite and write up a report, no code"]);
}

#[test]
fn a_cli_task_the_user_answers_as_research_is_refused_too() {
    refused_research("research-answered", &["hmm", "research"]);
}

// ---- a one-shot is refused in a coding CLI too (until M6's sandbox)
//
// The bridge can hold a CLI to one ticket, but not the CLI's own file and shell
// tools: it could edit the project, run git or push instead of handing the
// ticket to a worker. So, like research, a one-shot does not run in a CLI.

const ONE_SHOT_TASK: &str = "fix a typo in the readme, quick small change";

/// The refusal, everything it must leave clean, and that the project is not stuck.
fn assert_refused(h: &Harness, ws: &Path, run: &Path, what: &str, mentions: &[&str]) {
    let s = wait(h, "the refusal", |s| {
        s.agents[0].log.iter().any(|l| l.kind == LogKind::Error && l.text.to_lowercase().contains(what))
    });
    std::thread::sleep(Duration::from_millis(300));
    let refusal = s.agents[0].log.iter().rev().find(|l| l.kind == LogKind::Error).unwrap().text.to_lowercase();
    for m in mentions {
        assert!(refusal.contains(m), "the refusal should mention `{m}`: {refusal}");
    }
    assert_eq!(launches(run), 0, "the CLI never ran");
    assert!(s.tickets.is_empty() && s.approvals.is_empty() && s.agents.len() == 1, "no task was created");
    assert!(!ws.join(".backspace/handoff").exists() && !ws.join(".backspace/reports").exists(), "no spec or report was written");
    assert_eq!((s.intake.path, s.intake.pending.clone(), s.intake.spec_key.clone()), (None, None, None), "nothing half-started, and not turned into a long task");
    assert_eq!(s.agents[0].status, AgentStatus::Idle);
    assert_eq!(h.metrics().calls, 0, "no model call, no spend");
    // The next message is a fresh task, not stuck behind the refusal.
    h.send("hmm");
    wait(h, "the next task to be asked about", |s| asked(s) >= 1 && s.intake.pending.is_some());
    assert_eq!(launches(run), 0);
}

const ONE_SHOT_REFUSAL: &[&str] = &["one-shot", "coding cli", "m6", "api"];

#[test]
fn a_cli_one_shot_the_classifier_chooses_is_refused_before_the_cli_runs() {
    let (h, ws, run) = open("oneshot-classified");
    h.send(ONE_SHOT_TASK);
    assert_refused(&h, &ws, &run, "one-shot", ONE_SHOT_REFUSAL);
}

#[test]
fn a_cli_task_the_user_answers_as_one_shot_is_refused_too() {
    let (h, ws, run) = open("oneshot-answered");
    h.send("hmm");
    wait(&h, "the question", |s| asked(s) == 1 && s.agents[0].status == AgentStatus::Idle);
    h.send("one-shot");
    assert_refused(&h, &ws, &run, "one-shot", ONE_SHOT_REFUSAL);
}

/// A task path saved by an earlier session (an API planner, say) and opened
/// again with a coding-CLI planner: refused on the next message, before the CLI.
fn saved_path_is_refused(tag: &str, saved: serde_json::Value, what: &str, mentions: &[&str]) {
    let (h, ws, run) = open(tag);
    h.send("hmm");
    wait(&h, "the question", |s| asked(s) == 1 && s.agents[0].status == AgentStatus::Idle);
    drop(h);
    let file = ws.join(".backspace/state.json");
    let mut state: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    state["intake"] = saved;
    std::fs::write(&file, state.to_string()).unwrap();

    let h = Harness::open_with(ws.clone(), Overrides { planner: Some("claude-code".into()), ..Default::default() }).unwrap();
    assert!(h.snapshot().intake.path.is_some(), "the saved path was restored");
    assert_eq!(launches(&run), 0, "opening launches nothing");
    h.send("carry on with it");
    assert_refused(&h, &ws, &run, what, mentions);
}

#[test]
fn a_saved_one_shot_is_refused_after_a_restart_with_a_cli_planner() {
    saved_path_is_refused(
        "saved-oneshot",
        serde_json::json!({"path": "one_shot", "pending": null, "spec_key": "fix-readme"}),
        "one-shot",
        ONE_SHOT_REFUSAL,
    );
}

#[test]
fn a_saved_research_task_is_refused_after_a_restart_with_a_cli_planner() {
    saved_path_is_refused(
        "saved-research",
        serde_json::json!({"path": "research", "pending": null, "spec_key": null}),
        "research",
        &["research", "coding cli", "api model", "m6"],
    );
}

#[test]
fn a_cli_long_task_still_launches_the_planner_as_before() {
    let (h, _ws, run) = open("long-cli");
    h.send("build a dashboard system platform across several services");
    let s = wait(&h, "the CLI to run", |_| launches(&run) == 1);
    wait(&h, "the turn to end", |s| s.agents[0].status == AgentStatus::Idle);
    let args = std::fs::read_to_string(run.join(".args-1")).unwrap();
    assert!(!args.contains("one-shot agent") && !args.contains("research agent"), "the long path's brief, not a restricted one: {args}");
    assert_eq!(h.snapshot().intake.path, Some(backspace_core::decision::TaskType::Long));
    assert_eq!(asked(&h.snapshot()), 0);
    assert!(!s.agents[0].log.iter().any(|l| l.kind == LogKind::Error), "{:?}", s.agents[0].log);
}
