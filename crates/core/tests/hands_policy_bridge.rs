//! M4 closure: the bridge answers `browser_task`'s policy question with the
//! project's own lists, at the asking agent's role. A fake CLI planner (an
//! orchestration agent) asks through curl, the way `backspace mcp` would.
#![cfg(unix)]

use std::time::{Duration, Instant};

use backspace_core::{AgentStatus, Harness, Overrides};

#[test]
fn the_bridge_checks_a_browser_decider_against_the_project_policy() {
    let bin = std::env::temp_dir().join(format!("bs-hands-bridge-bin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&bin);
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("deny.json"), r#"{"me":"main","models":["jev-latest"]}"#).unwrap();
    std::fs::write(bin.join("allow.json"), r#"{"me":"main","models":["claude-code"]}"#).unwrap();
    std::fs::write(bin.join("worker.json"), r#"{"me":"nobody","models":["claude-code"]}"#).unwrap();
    let script = r#"#!/bin/sh
d=$(dirname "$0")
cat > /dev/null
ask() { curl -s -o "$d/$1.body" -w '%{http_code}' -H "Authorization: Bearer $BACKSPACE_BRIDGE_TOKEN" -H 'Content-Type: application/json' --data-binary @"$d/$1.json" "$BACKSPACE_BRIDGE/v1/policy/model" > "$d/$1.code"; }
ask deny
ask allow
ask worker
printf '{"type":"result","result":"ok","session_id":"s1","total_cost_usd":0.01,"is_error":false}\n'
"#;
    let p = bin.join("claude");
    std::fs::write(&p, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("BACKSPACE_CLI_PATH", &bin);

    let ws = std::env::temp_dir().join(format!("bs-hands-bridge-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
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

[policy]
mode = "enforced"
orchestration_models = ["claude-code"]
execution_models = ["claude-code"]
skills = []
plugins = []
unpinned_cli_models = ["claude-code"]

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
    h.send("plan something");
    let deadline = Instant::now() + Duration::from_secs(40);
    while !(bin.join("worker.code").exists() && h.snapshot().agents[0].status == AgentStatus::Idle) {
        assert!(Instant::now() < deadline, "the CLI never asked: {:?}", h.snapshot().agents[0].log);
        std::thread::sleep(Duration::from_millis(20));
    }
    let said = |n: &str| (std::fs::read_to_string(bin.join(format!("{n}.code"))).unwrap(), std::fs::read_to_string(bin.join(format!("{n}.body"))).unwrap());
    let (code, body) = said("deny");
    assert_eq!(code, "400", "{body}");
    assert!(body.contains("jev-latest") && body.contains("policy"), "{body}");
    let (code, body) = said("allow");
    assert_eq!(code, "200", "{body}");
    let (code, body) = said("worker");
    assert_eq!(code, "400", "an agent the project does not have gets no answer: {body}");
}
