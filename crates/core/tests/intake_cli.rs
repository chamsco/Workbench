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
here=$(basename "$(pwd)")
input=$(cat)
# Only the planner keeps notes: a worker's worktree is committed and merged,
# and its notes would collide with the planner's.
if [ "$here" = _run ]; then
  n=$(( $(cat ./.n 2>/dev/null || echo 0) + 1 ))
  echo $n > ./.n
  printf '%s' "$*" > "./.args-$n"
  printf '%s' "$input" > "./.in-$n"
fi
post() {
  curl -s --noproxy '*' -X POST "$BACKSPACE_BRIDGE$1" -H "authorization: Bearer $BACKSPACE_BRIDGE_TOKEN" -d "$2"
}
check=true
[ -f ./.check-false ] && check=false
two='{"me":"main","tickets":[{"key":"a","title":"A","what_to_build":"x","acceptance":["x"]},{"key":"b","title":"B","what_to_build":"y","acceptance":["y"]}]}'
one="{\"me\":\"main\",\"tickets\":[{\"key\":\"fix-readme\",\"title\":\"Fix the README typo\",\"what_to_build\":\"Correct teh to the in README.md\",\"acceptance\":[\"README.md no longer contains teh\"],\"check\":\"$check\"}]}"
other='{"me":"main","tickets":[{"key":"second","title":"Second","what_to_build":"z","acceptance":["z"]}]}'
reply() { printf '{"type":"result","result":"%s","session_id":"s1","total_cost_usd":0.01,"is_error":false}\n' "$1"; }
case "$here" in
  _run)
    case "$*" in
      *"one-shot agent"*)
        post /v1/plan/create "$two" > ./.two
        post /v1/plan/create "$one" > ./.one
        if [ -f ./.dispatch ]; then
          post /v1/plan/create "$other" > ./.second
          post /v1/plan/dispatch '{"me":"main"}' > ./.dispatched
        fi
        reply "queued" ;;
      *) reply "ok" ;;
    esac ;;
  *)
    # A worker, in its own worktree: it does the work the ticket names.
    sed -i.bak 's/teh/the/' README.md && rm -f README.md.bak
    reply "Fixed the typo" ;;
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
    open_with(tag, false)
}

/// `workers_on_cli`: tickets are handed to the same fake `claude` in their own worktree.
fn open_with(tag: &str, workers_on_cli: bool) -> (Harness, PathBuf, PathBuf) {
    fake_claude();
    let ws = std::env::temp_dir().join(format!("bs-intake-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("README.md"), "Say teh word.\n").unwrap();
    let pin_sub = if workers_on_cli { "pin_sub = \"claude-code\"" } else { "" };
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
{pin_sub}

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
"#
        .replace("{pin_sub}", pin_sub),
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
    let s = wait(&h, "the refusal", |s| {
        s.agents[0].log.iter().any(|l| l.kind == LogKind::Error && l.text.to_lowercase().contains("research"))
    });
    std::thread::sleep(Duration::from_millis(300));
    let refusal = s.agents[0].log.iter().find(|l| l.kind == LogKind::Error).unwrap().text.to_lowercase();
    assert!(refusal.contains("coding cli") && refusal.contains("api model"), "it says why and what to do: {refusal}");
    assert_eq!(launches(&run), 0, "the CLI never ran");
    assert!(!ws.join(".backspace/reports").exists(), "no report was filed");
    assert!(s.tickets.is_empty() && s.approvals.is_empty() && s.agents.len() == 1);
    assert_eq!((s.intake.path, s.intake.pending.clone()), (None, None), "the refusal leaves nothing half-started");
    assert_eq!(s.agents[0].status, AgentStatus::Idle);
    assert_eq!(h.metrics().calls, 0, "no spend");
    // And the next message is a fresh task, not stuck on research.
    h.send("hmm");
    wait(&h, "the next task to be asked about", |s| asked(s) >= 1 && s.intake.pending.is_some());
}

#[test]
fn a_cli_research_task_is_refused_before_the_cli_runs() {
    refused_research("research-refused", &["research the pros and cons of sqlite and write up a report, no code"]);
}

#[test]
fn a_cli_task_the_user_answers_as_research_is_refused_too() {
    refused_research("research-answered", &["hmm", "research"]);
}

/// Approve every deliverable, never a plan: a one-shot has no plan gate.
fn approve_deliverables(h: &Harness, what: &str, done: impl Fn(&ProjectState) -> bool) -> ProjectState {
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut answered = vec![];
    loop {
        let s = h.snapshot();
        if done(&s) {
            return s;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:#?}\n{:#?}",
            s.agents.iter().map(|a| (&a.key, a.status, &a.log)).collect::<Vec<_>>(),
            s.tickets
        );
        for a in s.pending_approvals() {
            assert_ne!(a.kind, ApprovalKind::Plan, "a one-shot has no plan gate");
            if !answered.contains(&a.id) {
                answered.push(a.id);
                h.approve(a.id);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_cli_one_shot_dispatches_a_worker_and_its_path_ends_with_its_ticket() {
    let (h, ws, run) = open_with("dispatch", true);
    std::fs::write(run.join(".dispatch"), "").unwrap();
    h.send("fix a typo in the readme, quick small change");
    let s = approve_deliverables(&h, "the one-shot to finish", |s| s.agents[0].status == AgentStatus::Approved);

    // What the bridge allowed, in code: one validated ticket, a refusal for two
    // and for a second, and a dispatch.
    let read = |f: &str| std::fs::read_to_string(run.join(f)).unwrap_or_default().to_lowercase();
    assert!(read(".two").contains("exactly one"), "{}", read(".two"));
    assert!(read(".one").contains("fix-readme"), "{}", read(".one"));
    assert!(read(".second").contains("already has its ticket"), "{}", read(".second"));
    assert!(read(".dispatched").contains("queued"), "{}", read(".dispatched"));

    // The worker ran in its own worktree, was approved, and its work merged.
    assert_eq!(s.tickets.len(), 1, "{:?}", s.tickets);
    assert_eq!(s.tickets[0].state, TicketState::Done, "{:?}", s.tickets[0]);
    let w = s.agents.iter().find(|a| a.ticket.as_deref() == Some("fix-readme")).expect("a worker ran");
    assert_ne!(w.worktree.as_deref(), Some(run.as_path()), "not in the planner's directory");
    assert_eq!(std::fs::read_to_string(run.join("README.md")).unwrap(), "Say the word.\n");
    assert!(ws.join(".backspace/handoff/fix-readme/spec.md").is_file());
    assert!(!s.approvals.iter().any(|a| a.kind == ApprovalKind::Plan));
    assert_eq!(launches(&run), 2, "the dispatch, then the planner told how it went");

    // The ticket settled, so the task is over: its path is released and the
    // next message is a new task, asked about afresh.
    assert_eq!(s.intake.path, None);
    assert_eq!(s.intake.spec_key, None);
    h.send("hmm");
    wait(&h, "the next task to be asked about", |s| asked(s) == 1 && s.intake.pending.is_some());
    assert_eq!(launches(&run), 2, "asking launched nothing");
}

#[test]
fn a_cli_one_shot_whose_ticket_fails_also_releases_its_path() {
    let (h, _ws, run) = open_with("failing", true);
    std::fs::write(run.join(".dispatch"), "").unwrap();
    std::fs::write(run.join(".check-false"), "").unwrap(); // the ticket's check can never pass
    h.send("fix a typo in the readme, quick small change");
    let s = approve_deliverables(&h, "the ticket to fail", |s| {
        s.tickets.first().is_some_and(|t| t.state == TicketState::Failed) && s.intake.path.is_none() && s.agents[0].status == AgentStatus::Idle
    });
    assert_eq!(s.tickets.len(), 1);
    assert_ne!(s.agents[0].status, AgentStatus::Approved, "a failed ticket is not a delivered task");
    assert!(s.agents[0].deliverable.is_none());
    assert_eq!(s.intake.spec_key, None);
}
