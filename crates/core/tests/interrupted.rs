//! A worker a restart cut off (the audit's P0-2). It is not a failed agent:
//! its branch, worktree and — for a CLI worker — its own session are all still
//! on disk. Opening the project marks it `interrupted`, and re-dispatching the
//! ticket carries on in that session instead of starting over.

use std::path::Path;

use backspace_core::{AgentKind, AgentStatus, Harness, TicketState};
use serde_json::json;

/// A fake `claude` that records how it was called and what it was told, then
/// answers in Claude Code's stream-json. It also writes the file the ticket's
/// check looks for.
///
/// Node and a `.cmd` shim on Windows, a shell script elsewhere: the worker's
/// CLI path is real, so the test runs on both.
fn fake_claude(dir: &Path) {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let js = r#"
const fs = require("fs"), path = require("path");
const here = __dirname;
let input = "";
try { input = fs.readFileSync(0, "utf8"); } catch (e) { input = ""; }
fs.writeFileSync(path.join(here, "args"), process.argv.slice(2).join(" "));
fs.writeFileSync(path.join(here, "input"), input);
fs.writeFileSync(path.join(process.cwd(), "resumed.txt"), "resumed\n");
process.stdout.write(JSON.stringify({type:"system", subtype:"init", session_id:"cli-7"}) + "\n");
process.stdout.write(JSON.stringify({type:"result", result:"Carried on: resumed.txt is there", session_id:"cli-7", is_error:false}) + "\n");
"#;
    std::fs::write(bin.join("claude.js"), js).unwrap();

    #[cfg(windows)]
    {
        // `which()` on Windows only spawns .exe/.cmd, and node.exe is on PATH.
        std::fs::write(
            bin.join("claude.cmd"),
            "@echo off\r\nnode \"%~dp0claude.js\" %*\r\n",
        )
        .unwrap();
    }
    #[cfg(unix)]
    {
        let p = bin.join("claude");
        std::fs::write(
            &p,
            format!("#!/bin/sh\nexec node \"$(dirname \"$0\")/claude.js\" \"$@\"\n"),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::env::set_var("BACKSPACE_CLI_PATH", &bin);
}

/// A project as it was on disk when the app closed mid-ticket: one interrupted
/// CLI worker with a saved session, and its ticket in progress.
fn interrupted_project(dir: &Path) -> std::path::PathBuf {
    let ws = dir.join("project");
    std::fs::create_dir_all(ws.join(".backspace")).unwrap();
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
pin_sub = "claude-code"

[orchestrator]
max_parallel_calls = 1
max_subagents = 2
auto_approve = true
bash_timeout_secs = 10

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

    let state = json!({
        "name": "project",
        "workspace": ws.display().to_string(),
        "agents": [
            {
                "id": 0, "role": "main", "kind": "main", "parent": null, "depth": 0, "key": "main",
                "title": "Main agent", "brief": "ship it", "depends_on": [], "status": "idle",
                "decision": null, "log": [], "cost_usd": 0.0, "input_tokens": 0, "output_tokens": 0,
                "deliverable": null, "budget_usd": null, "ticket": null, "branch": null,
                "worktree": null, "escalations": []
            },
            {
                "id": 1, "role": "sub", "kind": "worker", "parent": 0, "depth": 1, "key": "add-resume",
                "title": "Add resume", "brief": "", "depends_on": [],
                // As the state file looked while it was working.
                "status": "running",
                "decision": {"model": "claude-code", "effort": "medium", "source": "pinned",
                             "confidence": 1.0, "router_cost_usd": 0.0, "note": null},
                "log": [{"kind": "user", "text": "# add-resume: Add resume\nWrite resumed.txt"}],
                "cost_usd": 0.0, "input_tokens": 0, "output_tokens": 0, "deliverable": null,
                "budget_usd": null, "ticket": "add-resume", "branch": "backspace/add-resume",
                "worktree": null, "escalations": [],
                // The session the CLI was answering in when the process died.
                "ses": "cli-7"
            }
        ],
        "approvals": [],
        "tickets": [{
            "num": 1, "key": "add-resume", "title": "Add resume", "category": "task",
            "state": "in_progress", "what_to_build": "Write resumed.txt",
            "acceptance": ["resumed.txt exists"], "check": "test -f resumed.txt",
            "out_of_scope": [], "blocked_by": [], "owner": 0, "assignee": 1,
            "budget_usd": null, "notes": []
        }],
        "total_cost_usd": 0.0,
        "router_cost_usd": 0.0,
        "config_source": null,
        "board": [],
        "planner_session": null,
        "canvas_ops": [],
        "options": []
    });
    // A worker's own worktree, exactly as `git::worktree_path` names it and as
    // a previous run would have left it: a real git worktree on its own branch,
    // so the resumed run commits and checks *there*.
    let wt = ws.join(".backspace/worktrees/add-resume");
    let (root, wt2) = (ws.clone(), wt.clone());
    let run = |args: &[&str], cwd: &Path| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .env("GIT_AUTHOR_NAME", "WorkBuddy AI")
            .env("GIT_AUTHOR_EMAIL", "noreply@workbuddy.ai")
            .env("GIT_COMMITTER_NAME", "WorkBuddy AI")
            .env("GIT_COMMITTER_EMAIL", "noreply@workbuddy.ai")
            .output()
            .expect("git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    run(&["init", "-q"], &root);
    run(&["add", "-A"], &root);
    run(&["commit", "-q", "-m", "start"], &root);
    run(&["worktree", "add", "-q", "-b", "backspace/add-resume", wt2.to_str().unwrap()], &root);

    let mut state = state;
    state["agents"][1]["worktree"] = json!(wt.display().to_string());
    std::fs::write(ws.join(".backspace/state.json"), state.to_string()).unwrap();
    ws
}

#[test]
fn a_restart_carries_a_cli_worker_on_in_its_own_session() {
    let dir = std::env::temp_dir().join(format!("bs-interrupted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    fake_claude(&dir);
    let ws = interrupted_project(&dir);
    std::env::set_var("BACKSPACE_PREFS", dir.join("prefs.json"));
    std::env::set_var("BACKSPACE_DATA", dir.join("data"));

    let h = Harness::open(ws.clone()).expect("reopen the project");

    // Reopening does not pretend the work failed: it says it was cut off, and
    // keeps the session it can carry on in.
    let a = h.snapshot().agents[1].clone();
    assert_eq!(a.kind, AgentKind::Worker);
    assert_eq!(a.status, AgentStatus::Interrupted, "not failed: nothing about the work failed");
    assert_eq!(a.ses.as_deref(), Some("cli-7"));
    assert!(
        a.log.iter().any(|l| l.text.starts_with("interrupted:")),
        "the agent is told what happened: {:?}",
        a.log.iter().map(|l| &l.text).collect::<Vec<_>>()
    );

    // An agent that is still working cannot be "resumed": this is not a retry
    // button. (Ours is interrupted, so it can.)
    let msg = h.resume(1).expect("carry on");
    assert!(msg.contains("add-resume"), "{msg}");

    // Give the background turn time to land.
    let start = std::time::Instant::now();
    loop {
        let s = h.snapshot();
        let t = s.tickets.iter().find(|t| t.key == "add-resume").unwrap();
        if t.state == TicketState::InReview || t.state == TicketState::Done || t.state == TicketState::Failed {
            break;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(60), "the resumed ticket never settled: {:?} {}", t.state, t.notes.join("; "));
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // It answered in the session it had, not a fresh one.
    let args = std::fs::read_to_string(dir.join("bin/args")).unwrap();
    assert!(args.contains("--resume cli-7"), "the CLI must be asked to resume: {args}");
    // And it did not start over: the prompt is the ticket's, not a re-plan.
    let input = std::fs::read_to_string(dir.join("bin/input")).unwrap();
    assert!(input.contains("add-resume"), "{input}");
    // The worktree it already had is the one it worked in, and its file is
    // there (the run did not recreate the worktree).
    assert!(
        ws.join(".backspace/worktrees/add-resume/resumed.txt").is_file(),
        "the resumed worker wrote into its own worktree"
    );
    // The ticket went through the gate, so this was the ordinary path.
    let s = h.snapshot();
    let t = s.tickets.iter().find(|t| t.key == "add-resume").unwrap();
    assert!(
        matches!(t.state, TicketState::InReview | TicketState::Done),
        "a resumed ticket goes through the same check and review: {:?} ({})",
        t.state,
        t.notes.join("; ")
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_an_interrupted_agent_can_carry_on() {
    let dir = std::env::temp_dir().join(format!("bs-interrupted-no-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    fake_claude(&dir);
    // A worker that finished its ticket: reopening leaves its status alone, so
    // "resume" must refuse rather than run the ticket again.
    let ws = interrupted_project(&dir);
    std::env::set_var("BACKSPACE_PREFS", dir.join("prefs.json"));
    std::env::set_var("BACKSPACE_DATA", dir.join("data"));
    let path = ws.join(".backspace/state.json");
    let mut state: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    state["agents"][1]["status"] = json!("approved");
    std::fs::write(&path, state.to_string()).unwrap();

    let h = Harness::open(ws.clone()).expect("open");
    assert_eq!(h.snapshot().agents[1].status, AgentStatus::Approved, "a finished agent stays finished");
    match h.resume(1) {
        Ok(m) => panic!("a finished agent must not be resumed: {m}"),
        Err(e) => {
            let err = e.to_string();
            assert!(err.contains("Approved"), "{err}");
            assert!(err.contains("dispatched again"), "and it says what to do instead: {err}");
        }
    }
    // The main agent is never resumed either: it takes messages.
    match h.resume(0) {
        Ok(m) => panic!("the main agent must not be resumed: {m}"),
        Err(e) => assert!(e.to_string().contains("send it a message"), "{e}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}
