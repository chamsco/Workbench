//! Familiars (Copilots board round 4, 17 Nov): a goal handed to a harness on
//! the long path, what the app shows of it, and the drive that carries a
//! finished run through Verify, Ship and Learn.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

use backspace_core::deliver::{self, Target, TicketFacts};
use backspace_core::ticket::TicketState;
use backspace_core::familiar::{self, State};
use backspace_core::phase::{Phase, PhaseState};
use backspace_core::project::{AgentStatus, LogEntry, LogKind, ProjectState};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bs-familiar-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn records_round_trip_and_get_a_short_name_from_the_goal() {
    let data = tmp("store");
    let folder = tmp("store-folder");
    let r = familiar::add(&data, "Add a CSV export to the report command, with headers and quoting", &folder, Target::PullRequest).unwrap();
    assert_eq!(r.name, "Add a CSV export to");
    assert_eq!(r.target, Target::PullRequest);
    let all = familiar::load(&data);
    assert_eq!(all, vec![r.clone()]);
    familiar::remove(&data, &r.id).unwrap();
    assert!(familiar::load(&data).is_empty());
}

#[test]
fn a_familiar_needs_a_folder_and_a_goal() {
    let data = tmp("needs");
    assert!(familiar::add(&data, "  ", &tmp("needs-folder"), Target::Local).is_err());
    assert!(familiar::add(&data, "goal", Path::new("/nonexistent/folder/x"), Target::Local).is_err());
}

fn state() -> ProjectState {
    let mut s: ProjectState = serde_json::from_value(serde_json::json!({
        "name": "p", "workspace": "/p", "agents": [], "approvals": [], "tickets": [],
        "total_cost_usd": 0.5, "router_cost_usd": 0.0, "config_source": null
    }))
    .unwrap();
    let mut main: backspace_core::project::AgentRecord = serde_json::from_value(serde_json::json!({
        "id": 0, "role": "main", "kind": "main", "parent": null, "depth": 0, "key": "main", "title": "Main agent",
        "brief": "", "depends_on": [], "status": "running", "decision": null, "log": [], "cost_usd": 0.0,
        "input_tokens": 0, "output_tokens": 0, "deliverable": null, "budget_usd": null, "ticket": null, "branch": null, "worktree": null, "escalations": []
    }))
    .unwrap();
    main.log.push(LogEntry { kind: LogKind::System, text: "intake: long path".into() });
    s.agents.push(main);
    s
}

#[test]
fn the_view_says_needs_you_when_the_spec_waits_for_a_yes() {
    let root = tmp("view-spec");
    let rec = familiar::Record { id: "f1".into(), name: "CSV".into(), goal: "g".into(), folder: root.clone(), target: Target::Local, created: 0 };
    let mut s = state();
    s.planning.awaiting_yes = true;
    s.agents[0].status = AgentStatus::Idle;
    let v = familiar::view(&rec, &s, &root);
    assert_eq!(v.state, State::NeedsYou);
    assert!(v.ask.as_deref().unwrap().contains("spec"), "{:?}", v.ask);
    assert_eq!(v.yes_reply.as_deref(), Some("yes"));
}

#[test]
fn the_view_carries_the_delivery_and_a_refused_verify() {
    let root = tmp("view-deliver");
    let rec = familiar::Record { id: "f2".into(), name: "CSV".into(), goal: "g".into(), folder: root.clone(), target: Target::Local, created: 0 };
    let mut s = state();
    s.agents[0].status = AgentStatus::Approved;
    let mut p = PhaseState::default();
    p.phase = Phase::Test;
    p.save(&root).unwrap();
    let report = deliver::VerifyReport { passed: false, head: "abc".into(), issues: vec!["A2 is not met".into()], ..Default::default() };
    std::fs::write(root.join(deliver::VERIFY_JSON), serde_json::to_string(&report).unwrap()).unwrap();
    let v = familiar::view(&rec, &s, &root);
    assert_eq!(v.state, State::NeedsYou);
    assert_eq!(v.stage, Phase::Test);
    assert!(v.ask.as_deref().unwrap().contains("A2 is not met"));
    assert!(v.verify.is_some() && v.delivery.is_none());
    assert!(v.events.iter().any(|e| e.text.contains("intake")), "the main agent's log lines are its events");
}

#[test]
fn drive_waits_for_the_main_agent_then_ships_once() {
    let root = tmp("drive");
    git(&root, &["init", "-q"]);
    std::fs::write(root.join("a.txt"), "a").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "s"]);
    std::fs::create_dir_all(root.join(".backspace/planning")).unwrap();
    std::fs::write(root.join(deliver::PLAN_JSON), r#"{"title":"Keep a","goal":"g","acceptance":[{"criterion":"a","test":"test -f a.txt"}]}"#).unwrap();
    let mut p = PhaseState::default();
    p.phase = Phase::Execute;
    p.save(&root).unwrap();
    let t = vec![TicketFacts { key: "T-1".into(), state: TicketState::Done, covers: vec!["A1".into()] }];
    let mut learned = 0;
    let mut learn = || -> anyhow::Result<String> {
        learned += 1;
        Ok("filed".into())
    };
    // Still running: nothing moves.
    assert!(familiar::drive(&root, AgentStatus::Running, &t, Target::Local, &mut learn).unwrap().is_none());
    let steps = familiar::drive(&root, AgentStatus::Approved, &t, Target::Local, &mut learn).unwrap().unwrap();
    assert_eq!(steps.len(), 4, "{steps:?}");
    assert_eq!(PhaseState::load(&root).phase, Phase::Done);
    // Done: a later change drives nothing again.
    assert!(familiar::drive(&root, AgentStatus::Approved, &t, Target::Local, &mut learn).unwrap().is_none());
    drop(learn);
    assert_eq!(learned, 1);
}

#[test]
fn drive_does_not_rerun_a_refused_verify_on_the_same_commit() {
    let root = tmp("drive-refused");
    git(&root, &["init", "-q"]);
    std::fs::write(root.join("a.txt"), "a").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "s"]);
    std::fs::create_dir_all(root.join(".backspace/planning")).unwrap();
    std::fs::write(root.join(deliver::PLAN_JSON), r#"{"title":"T","goal":"g","acceptance":[{"criterion":"a","test":"false"}]}"#).unwrap();
    let mut p = PhaseState::default();
    p.phase = Phase::Test;
    p.save(&root).unwrap();
    let t = vec![TicketFacts { key: "T-1".into(), state: TicketState::Done, covers: vec!["A1".into()] }];
    let mut learn = || -> anyhow::Result<String> { Ok(String::new()) };
    assert!(familiar::drive(&root, AgentStatus::Approved, &t, Target::Local, &mut learn).unwrap().is_some());
    let gates = PhaseState::load(&root).gates.len();
    assert!(familiar::drive(&root, AgentStatus::Approved, &t, Target::Local, &mut learn).unwrap().is_none(), "same commit, same answer: not run again");
    assert_eq!(PhaseState::load(&root).gates.len(), gates);
}
