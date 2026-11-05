//! M3 acceptance for a planner that lives in a coding CLI. The CLI keeps its
//! own tools, so what code can enforce here is narrower than on the API path:
//! intake (classify, ask) runs before the CLI is launched, and Backspace's own
//! planning tools (`/v1/plan/*`, which the MCP bridge calls) obey the task's
//! path. What the CLI does with its *own* file and shell tools is not gated by
//! M3 (sandboxing is M6); docs/cauldron/M3-REPORT.md says so.
//!
//! Unix only: the fake `claude` is a shell script.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, Instant};

use backspace_core::{AgentStatus, ApprovalKind, Harness, LogKind, Overrides, ProjectState, TicketState};

/// One fake `claude` for the whole file. It keeps what it saw, and what the
/// bridge answered, in its working directory (each test has its own).
fn fake_claude() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let bin = std::env::temp_dir().join(format!("bs-intake-cli-bin-{}", std::process::id()));
        std::fs::create_dir_all(&bin).unwrap();
        let script = r#"#!/bin/sh
n=$(( $(cat ./.n 2>/dev/null || echo 0) + 1 ))
echo $n > ./.n
input=$(cat)
printf '%s' "$*" > "./.args-$n"
printf '%s' "$input" > "./.in-$n"
plan() {
  curl -s --noproxy '*' -X POST "$BACKSPACE_BRIDGE/v1/plan/create" -H "authorization: Bearer $BACKSPACE_BRIDGE_TOKEN" -d "$1"
}
two='{"me":"main","tickets":[{"key":"a","title":"A","what_to_build":"x","acceptance":["x"]},{"key":"b","title":"B","what_to_build":"y","acceptance":["y"]}]}'
one='{"me":"main","tickets":[{"key":"fix-readme","title":"Fix the README typo","what_to_build":"Correct teh to the in README.md","acceptance":["README.md no longer contains teh"],"check":"true"}]}'
reply() { printf '{"type":"result","result":"%s","session_id":"s1","total_cost_usd":0.01,"is_error":false}\n' "$1"; }
case "$*" in
  *"research agent"*)
    plan "$one" > ./.research-plan
    reply "FINDING: alpha is the answer" ;;
  *"one-shot agent"*)
    plan "$two" > ./.two
    plan "$one" > ./.one
    reply "queued" ;;
  *) reply "ok" ;;
esac
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

#[test]
fn a_cli_research_task_cannot_plan_and_its_answer_is_filed_as_the_report() {
    let (h, ws, run) = open("research");
    h.send("research the pros and cons of sqlite and write up a report, no code");
    let s = wait(&h, "the report", |s| s.agents[0].status == AgentStatus::Approved);
    let args = std::fs::read_to_string(run.join(".args-1")).unwrap();
    assert!(args.contains("research agent"), "the CLI is told it is on the research path: {args}");

    // Backspace's planning tool refused, in code.
    let refused = std::fs::read_to_string(run.join(".research-plan")).unwrap();
    assert!(refused.to_lowercase().contains("research"), "{refused}");
    assert!(s.tickets.is_empty() && s.approvals.is_empty());

    // The CLI's final answer is the report.
    let d = s.agents[0].deliverable.as_ref().expect("the report is the deliverable");
    assert!(d.files[0].starts_with(".backspace/reports/"), "{:?}", d.files);
    let text = std::fs::read_to_string(ws.join(&d.files[0])).unwrap();
    assert!(text.contains("FINDING: alpha is the answer"), "{text}");
    assert_eq!(s.intake.path, None);
}

#[test]
fn a_cli_one_shot_files_one_validated_ticket_with_no_plan_gate() {
    let (h, ws, run) = open("oneshot");
    h.send("fix a typo in the readme, quick small change");
    let s = wait(&h, "the turn", |s| s.agents[0].status == AgentStatus::Idle && launches(&run) == 1 && run.join(".one").exists());
    let args = std::fs::read_to_string(run.join(".args-1")).unwrap();
    assert!(args.contains("one-shot agent"), "{args}");

    let two = std::fs::read_to_string(run.join(".two")).unwrap();
    assert!(two.to_lowercase().contains("exactly one"), "two tickets are refused by code: {two}");
    assert_eq!(s.tickets.len(), 1, "{:?}", s.tickets);
    assert_eq!(s.tickets[0].key, "fix-readme");
    assert_eq!(s.tickets[0].state, TicketState::ReadyForAgent, "no plan gate on a one-shot");
    assert!(!s.approvals.iter().any(|a| a.kind == ApprovalKind::Plan));
    let spec = std::fs::read_to_string(ws.join(".backspace/handoff/fix-readme/spec.md")).unwrap();
    assert!(spec.contains("Correct teh to the in README.md"), "{spec}");
    assert_eq!(s.intake.path, Some(backspace_core::decision::TaskType::OneShot), "still the one-shot's until it is delivered");
}
