//! `backspace-cli deliver` and `status`: Verify and Ship from the command
//! line on rules alone (PRD §4.4-4.5). Unix-only: the acceptance commands are shell.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

fn project(tag: &str, test: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bs-deliver-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join(".backspace/planning")).unwrap();
    let d = std::fs::canonicalize(&d).unwrap();
    git(&d, &["init", "-q"]);
    std::fs::write(d.join("a.txt"), "hi\n").unwrap();
    git(&d, &["add", "a.txt"]);
    git(&d, &["commit", "-q", "-m", "start"]);
    let plan = format!(r#"{{"title":"Keep a","goal":"a.txt exists","acceptance":[{{"criterion":"a.txt exists","test":"{test}"}}]}}"#);
    std::fs::write(d.join(backspace_core::deliver::PLAN_JSON), plan).unwrap();
    std::fs::write(d.join(".backspace/phase.json"), r#"{"phase":"test","gates":[]}"#).unwrap();
    d
}

fn cli(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_backspace-cli"))
        .args(args)
        .current_dir(cwd)
        .env_remove("BACKSPACE_CONFIG")
        .env("HOME", cwd.join(".home"))
        .env("BACKSPACE_DATA", cwd.join(".home/data"))
        .output()
        .unwrap()
}

fn out(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

#[test]
fn deliver_refuses_a_run_without_finished_tickets_and_exits_nonzero() {
    let d = project("notickets", "test -f a.txt");
    let o = cli(&d, &["deliver", "."]);
    assert_eq!(o.status.code(), Some(2), "{}", out(&o));
    assert!(out(&o).contains("there are no tickets yet"), "{}", out(&o));
    assert!(!d.join(backspace_core::deliver::DELIVERY_JSON).exists());
    let s = cli(&d, &["status", "."]);
    assert!(s.status.success(), "{}", out(&s));
    let text = out(&s);
    assert!(text.contains("phase: Test") && text.contains("A1 pass") && text.contains("issue: there are no tickets yet"), "{text}");
}

#[test]
fn an_unknown_ship_target_is_refused() {
    let d = project("target", "true");
    let o = cli(&d, &["deliver", ".", "--ship", "merge"]);
    assert!(!o.status.success());
    assert!(out(&o).contains("local, branch or pr"), "{}", out(&o));
}
