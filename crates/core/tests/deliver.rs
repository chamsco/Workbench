//! Verify, Ship and the delivery record (PRD §4.4-4.5): the stages after
//! Execute, on a real git repository with the run branch the harness uses.
//! Unix-only: the acceptance commands and the fake `gh` are shell.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

use backspace_core::deliver::{self, ShipOpts, Target, TicketFacts};
use backspace_core::phase::{Phase, PhaseState};
use backspace_core::ticket::TicketState;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"]).args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bs-deliver-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A project whose user branch has one commit with a test file, and whose
/// run branch (checked out at .backspace/worktrees/_run, as the harness
/// does) adds `feature.txt`. The plan has one criterion per `tests`.
fn project(name: &str, tests: &[&str]) -> PathBuf {
    let root = tmp(name);
    git(&root, &["init", "-q"]);
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::write(root.join("tests/foo_test.rs"), "#[test]\nfn foo() { assert!(true); }\n").unwrap();
    std::fs::write(root.join("README.md"), "hi\n").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "start"]);
    let run = root.join(".backspace/worktrees/_run");
    git(&root, &["branch", "backspace/run"]);
    git(&root, &["worktree", "add", "-q", run.to_str().unwrap(), "backspace/run"]);
    std::fs::write(root.join(".backspace/.gitignore"), "*\n").unwrap();
    std::fs::write(run.join("feature.txt"), "done\n").unwrap();
    git(&run, &["add", "-A"]);
    git(&run, &["commit", "-q", "-m", "feature"]);
    let acceptance: Vec<_> = tests.iter().enumerate().map(|(i, t)| serde_json::json!({"criterion": format!("criterion {}", i + 1), "test": t})).collect();
    let plan = serde_json::json!({"title": "Add the feature", "goal": "feature.txt exists", "acceptance": acceptance});
    std::fs::create_dir_all(root.join(".backspace/planning")).unwrap();
    std::fs::write(root.join(deliver::PLAN_JSON), plan.to_string()).unwrap();
    let mut p = PhaseState::default();
    p.phase = Phase::Test;
    p.save(&root).unwrap();
    root
}

fn run_commit(root: &Path, files: &[(&str, Option<&str>)]) {
    let run = root.join(".backspace/worktrees/_run");
    for (f, body) in files {
        match body {
            Some(b) => std::fs::write(run.join(f), b).unwrap(),
            None => std::fs::remove_file(run.join(f)).unwrap(),
        }
    }
    git(&run, &["add", "-A"]);
    git(&run, &["commit", "-q", "-m", "more"]);
}

fn done(keys: &[(&str, &[&str])]) -> Vec<TicketFacts> {
    keys.iter().map(|(k, c)| TicketFacts { key: k.to_string(), state: TicketState::Done, covers: c.iter().map(|s| s.to_string()).collect() }).collect()
}

#[test]
fn verify_passes_when_every_criterion_passes_and_moves_to_ship() {
    let root = project("pass", &["test -f feature.txt"]);
    let r = deliver::verify(&root, &done(&[("T-1", &["A1"])])).unwrap();
    assert!(r.passed, "{:?}", r.issues);
    assert_eq!(r.criteria.len(), 1);
    assert!(r.criteria[0].passed);
    assert_eq!(r.head, git(&root, &["rev-parse", "backspace/run"]));
    assert_eq!(PhaseState::load(&root).phase, Phase::Ship);
    assert!(root.join(deliver::VERIFY_MD).exists());
}

#[test]
fn a_failing_criterion_keeps_the_project_in_verify() {
    let root = project("fail", &["test -f feature.txt", "test -f missing.txt"]);
    let r = deliver::verify(&root, &done(&[("T-1", &["A1", "A2"])])).unwrap();
    assert!(!r.passed);
    assert!(r.issues.iter().any(|i| i.contains("A2")), "{:?}", r.issues);
    let p = PhaseState::load(&root);
    assert_eq!(p.phase, Phase::Test);
    assert!(!p.gates.last().unwrap().passed);
}

#[test]
fn passing_tests_are_not_delivery_unfinished_tickets_fail_verify() {
    let root = project("ticket", &["true"]);
    let mut t = done(&[("T-1", &["A1"])]);
    t.push(TicketFacts { key: "T-2".into(), state: TicketState::InProgress, covers: vec![] });
    let r = deliver::verify(&root, &t).unwrap();
    assert!(!r.passed);
    assert!(r.issues.iter().any(|i| i.contains("T-2")), "{:?}", r.issues);
}

#[test]
fn a_criterion_no_ticket_covers_is_reported() {
    let root = project("cover", &["true", "true"]);
    let r = deliver::verify(&root, &done(&[("T-1", &["A1"])])).unwrap();
    assert!(!r.passed);
    assert!(r.issues.iter().any(|i| i.contains("A2") && i.contains("no ticket")), "{:?}", r.issues);
}

#[test]
fn a_deleted_test_file_is_a_weakened_suite() {
    let root = project("deleted", &["true"]);
    run_commit(&root, &[("tests/foo_test.rs", None)]);
    let r = deliver::verify(&root, &done(&[("T-1", &["A1"])])).unwrap();
    assert!(!r.passed);
    assert!(r.weakened.iter().any(|w| w.contains("tests/foo_test.rs")), "{:?}", r.weakened);
}

#[test]
fn an_added_ignore_or_removed_assert_is_a_weakened_suite() {
    let root = project("ignored", &["true"]);
    run_commit(&root, &[("tests/foo_test.rs", Some("#[test]\n#[ignore]\nfn foo() {}\n"))]);
    let r = deliver::verify(&root, &done(&[("T-1", &["A1"])])).unwrap();
    assert!(!r.passed);
    assert!(r.weakened.iter().any(|w| w.contains("ignore")), "{:?}", r.weakened);
    assert!(r.weakened.iter().any(|w| w.contains("assert")), "{:?}", r.weakened);
}

#[test]
fn a_changed_sealed_test_fails_verify() {
    let root = project("sealed", &["true"]);
    let seal = serde_json::json!({
        "files": [{"path": "tests/foo_test.rs", "sha256": "0000", "bytes": 1, "command": "true", "criteria": ["A1"]}],
        "suite_sha256": "x", "spec_sha256": "x", "base": "copy", "rampart_model": "m",
        "vetting": {"tests": []}, "human_checked": true, "sealed_at": 0
    });
    std::fs::create_dir_all(root.join(".backspace/rampart")).unwrap();
    std::fs::write(root.join(backspace_core::rampart::SEAL), seal.to_string()).unwrap();
    let r = deliver::verify(&root, &done(&[("T-1", &["A1"])])).unwrap();
    assert!(!r.passed);
    assert!(r.issues.iter().any(|i| i.contains("hash-locked")), "{:?}", r.issues);
}

#[test]
fn ship_is_refused_without_a_passing_verify() {
    let root = project("noverify", &["false"]);
    let _ = deliver::verify(&root, &done(&[("T-1", &["A1"])])).unwrap();
    let mut p = PhaseState::load(&root);
    p.phase = Phase::Ship;
    p.save(&root).unwrap();
    let e = deliver::ship(&root, &ShipOpts { target: Target::Local, ..Default::default() }).unwrap_err();
    assert!(format!("{e:#}").contains("verif"), "{e:#}");
}

#[test]
fn a_change_after_verify_sends_the_work_back_to_verify() {
    let root = project("stale", &["true"]);
    assert!(deliver::verify(&root, &done(&[("T-1", &["A1"])])).unwrap().passed);
    run_commit(&root, &[("late.txt", Some("x"))]);
    let e = deliver::ship(&root, &ShipOpts { target: Target::Local, ..Default::default() }).unwrap_err();
    assert!(format!("{e:#}").contains("changed since"), "{e:#}");
    assert_eq!(PhaseState::load(&root).phase, Phase::Test);
}

#[test]
fn shipping_locally_writes_the_delivery_record_and_moves_to_learn() {
    let root = project("local", &["true"]);
    let user_head = git(&root, &["rev-parse", "HEAD"]);
    assert!(deliver::verify(&root, &done(&[("T-1", &["A1"])])).unwrap().passed);
    let d = deliver::ship(&root, &ShipOpts { target: Target::Local, ..Default::default() }).unwrap();
    assert_eq!(d.commit, git(&root, &["rev-parse", "backspace/run"]));
    assert_eq!(d.status, "delivered");
    assert!(d.pr_url.is_none());
    assert!(!d.limitations.is_empty(), "a local delivery says it still needs a merge");
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), user_head, "Ship never merges into your branch");
    assert!(root.join(deliver::DELIVERY_MD).exists());
    let back: deliver::DeliveryRecord = serde_json::from_str(&std::fs::read_to_string(root.join(deliver::DELIVERY_JSON)).unwrap()).unwrap();
    assert_eq!(back, d);
    assert_eq!(PhaseState::load(&root).phase, Phase::Learn);
}

#[test]
fn shipping_a_branch_pushes_the_run_to_origin() {
    let root = project("branch", &["true"]);
    let origin = tmp("branch-origin");
    git(&origin, &["init", "-q", "--bare"]);
    git(&root, &["remote", "add", "origin", origin.to_str().unwrap()]);
    assert!(deliver::verify(&root, &done(&[("T-1", &["A1"])])).unwrap().passed);
    let d = deliver::ship(&root, &ShipOpts { target: Target::Branch, ..Default::default() }).unwrap();
    assert!(d.branch.starts_with("cauldron/"), "{}", d.branch);
    assert_eq!(git(&origin, &["rev-parse", &d.branch]), d.commit);
}

#[test]
fn shipping_a_pull_request_uses_gh_and_records_its_url() {
    let root = project("pr", &["true"]);
    let origin = tmp("pr-origin");
    git(&origin, &["init", "-q", "--bare"]);
    git(&root, &["remote", "add", "origin", origin.to_str().unwrap()]);
    let bin = tmp("pr-bin");
    let gh = bin.join("gh");
    std::fs::write(&gh, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$(dirname \"$0\")/args\"\necho https://github.com/o/r/pull/7\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(deliver::verify(&root, &done(&[("T-1", &["A1"])])).unwrap().passed);
    let d = deliver::ship(&root, &ShipOpts { target: Target::PullRequest, gh: Some(gh.clone()), ..Default::default() }).unwrap();
    assert_eq!(d.pr_url.as_deref(), Some("https://github.com/o/r/pull/7"));
    let args = std::fs::read_to_string(bin.join("args")).unwrap();
    assert!(args.contains("pr\ncreate") && args.contains(&d.branch), "{args}");
    assert!(!args.contains("merge"), "Ship opens a PR, never merges: {args}");
}

#[test]
fn a_pull_request_without_gh_is_a_missing_credential_not_a_delivery() {
    let root = project("nogh", &["true"]);
    let origin = tmp("nogh-origin");
    git(&origin, &["init", "-q", "--bare"]);
    git(&root, &["remote", "add", "origin", origin.to_str().unwrap()]);
    assert!(deliver::verify(&root, &done(&[("T-1", &["A1"])])).unwrap().passed);
    let e = deliver::ship(&root, &ShipOpts { target: Target::PullRequest, gh: Some("/nonexistent/gh".into()), ..Default::default() }).unwrap_err();
    assert!(format!("{e:#}").contains("gh"), "{e:#}");
    assert_eq!(PhaseState::load(&root).phase, Phase::Ship, "nothing was delivered");
    assert!(!root.join(deliver::DELIVERY_JSON).exists());
}

#[test]
fn an_old_commit_phase_reads_as_learn() {
    let root = tmp("legacy-phase");
    std::fs::create_dir_all(root.join(".backspace")).unwrap();
    std::fs::write(root.join(".backspace/phase.json"), r#"{"phase":"commit","gates":[]}"#).unwrap();
    assert_eq!(PhaseState::load(&root).phase, Phase::Learn);
}
