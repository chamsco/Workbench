//! M4 acceptance for a long-path planner that lives in a coding CLI. It
//! reaches the planning stage through the bridge (`/v1/plan/spec`, the MCP
//! tool `submit_spec`), and `/v1/plan/create` is held to the same gate as the
//! API planner: its baseline commands wait for the user's approval, and no
//! ticket is made until the checklist passed and the user said yes.
//!
//! Unix only: the fake `claude` is a shell script that calls the bridge with
//! curl, the way the MCP server would.
#![cfg(unix)]

use std::path::PathBuf;
use std::sync::Once;
use std::time::{Duration, Instant};

use backspace_core::planning;
use backspace_core::{AgentStatus, Harness, Overrides, ProjectState};

fn bin() -> PathBuf {
    std::env::temp_dir().join(format!("bs-plan-cli-bin-{}", std::process::id()))
}

const SPEC: &str = r#"{"me":"main","title":"Greeting file","goal":"The app writes hello.txt so that the greeting survives a restart.","acceptance":[{"criterion":"hello.txt exists","test":"test -f hello.txt"}],"non_goals":["Translations."],"constraints":["No new dependencies."],"dependencies":["None"],"baseline":{"kind":"feature"}}"#;
const TICKETS: &str = r#"{"me":"main","tickets":[{"key":"greet","title":"Write the greeting","what_to_build":"Write hello.txt","acceptance":["hello.txt says hello"]}]}"#;

/// A CLI planner that, on its first run, tries to plan, then submits the spec;
/// on its second (after the user's reply) plans again. What the bridge said is
/// kept beside the script, out of the worktree.
fn fake_claude() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let bin = bin();
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("spec.json"), SPEC).unwrap();
        std::fs::write(bin.join("tickets.json"), TICKETS).unwrap();
        let script = r#"#!/bin/sh
d=$(dirname "$0")
n=$(( $(cat "$d/n" 2>/dev/null || echo 0) + 1 ))
echo $n > "$d/n"
cat > /dev/null
post() { curl -s -o "$d/$2.body" -w '%{http_code}' -H "Authorization: Bearer $BACKSPACE_BRIDGE_TOKEN" -H 'Content-Type: application/json' --data-binary @"$d/$3" "$BACKSPACE_BRIDGE$1" > "$d/$2.code"; }
if [ $n -eq 1 ]; then
  post /v1/plan/create early tickets.json
  post /v1/plan/spec ask spec.json
elif [ $n -eq 2 ]; then
  post /v1/plan/spec spec spec.json
else
  post /v1/plan/create after tickets.json
fi
printf '{"type":"result","result":"ok","session_id":"s1","total_cost_usd":0.01,"is_error":false}\n'
"#;
        let p = bin.join("claude");
        std::fs::write(&p, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("BACKSPACE_CLI_PATH", &bin);
    });
}

fn wait(h: &Harness, what: &str, done: impl Fn(&ProjectState) -> bool) -> ProjectState {
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        let s = h.snapshot();
        if done(&s) {
            return s;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}: {:#?}", s.agents.iter().map(|a| (&a.key, a.status, &a.log)).collect::<Vec<_>>());
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn said(name: &str) -> (String, String) {
    let b = bin();
    (std::fs::read_to_string(b.join(format!("{name}.code"))).unwrap_or_default(), std::fs::read_to_string(b.join(format!("{name}.body"))).unwrap_or_default())
}

#[test]
fn a_cli_planner_reaches_the_planning_stage_through_the_bridge() {
    fake_claude();
    let ws = std::env::temp_dir().join(format!("bs-plan-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("README.md"), "A project.\n").unwrap();
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
    h.send("build a dashboard system platform across several services");
    wait(&h, "the commands", |s| s.planning.awaiting_commands && s.agents[0].status == AgentStatus::Idle);
    let (code, body) = said("early");
    assert_eq!(code, "400", "{body}");
    assert!(body.contains("submit_spec"), "{body}");
    let (code, body) = said("ask");
    assert_eq!(code, "200", "{body}");
    assert!(body.to_lowercase().contains("have not run"), "{body}");
    assert!(!ws.join(planning::BASELINE).exists(), "nothing ran before the yes");

    h.send("yes");
    let s = wait(&h, "the spec", |s| s.planning.awaiting_yes && s.agents[0].status == AgentStatus::Idle);
    let (code, body) = said("spec");
    assert_eq!(code, "200", "{body}");
    assert!(body.to_lowercase().contains("checklist passed"), "{body}");
    assert!(ws.join(planning::SPEC).is_file());
    assert!(s.tickets.is_empty());

    h.send("yes");
    let s = wait(&h, "the plan", |s| !s.tickets.is_empty());
    let (code, body) = said("after");
    assert_eq!(code, "200", "{body}");
    assert_eq!(s.planning.approved_sha256, s.planning.spec_sha256);
}
