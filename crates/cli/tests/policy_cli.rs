//! M1 hardening, F1 and F3, from the command line: a new `backspace.toml`
//! starts with an explicit policy, `policy init` shows what a project allows
//! today so it can be written down, and `policy adopt` is the step a person
//! takes to accept a policy that loosens the adopted one.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use backspace_core::gateway::{lock_path, resolve_policy, PolicyMode};
use backspace_core::Config;

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bs-policy-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::canonicalize(&d).unwrap()
}

fn cli(cwd: &Path, data: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_backspace-cli"))
        .args(args)
        .current_dir(cwd)
        .env_remove("BACKSPACE_CONFIG")
        .env("HOME", data)
        .env("BACKSPACE_DATA", data.join("data"))
        .output()
        .unwrap()
}

fn out(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

#[test]
fn init_config_writes_a_config_with_an_explicit_enforced_policy() {
    let d = dir("init-config");
    let o = cli(&d, &d, &["init-config"]);
    assert!(o.status.success(), "{}", out(&o));
    let text = std::fs::read_to_string(d.join("backspace.toml")).unwrap();
    let cfg = Config::parse(&text).expect("the file it wrote is a valid config");
    assert_eq!(cfg.policy.mode, Some(PolicyMode::Enforced), "{text}");
    for m in &cfg.models {
        assert!(cfg.policy.orchestration_models.as_ref().unwrap().contains(&m.id), "{}", m.id);
        assert!(cfg.policy.execution_models.as_ref().unwrap().contains(&m.id), "{}", m.id);
    }
    assert!(cfg.policy.skills.is_some() && cfg.policy.plugins == Some(vec![]));
}

#[test]
fn policy_init_prints_a_block_that_keeps_what_the_project_allows_today() {
    let d = dir("init");
    let base = backspace_core::config::DEFAULT_CONFIG;
    std::fs::write(d.join("backspace.toml"), base).unwrap();
    let o = cli(&d, &d, &["policy", "init"]);
    assert!(o.status.success(), "{}", out(&o));
    let block = String::from_utf8_lossy(&o.stdout).to_string();
    assert!(block.contains("[policy]") && block.contains("mode = \"enforced\""), "{block}");
    let before = Config::parse(base).unwrap();
    let after = Config::parse(&format!("{base}\n{block}")).unwrap();
    for m in &before.models {
        assert!(after.policy.orchestration_models.as_ref().unwrap().contains(&m.id));
        assert!(after.policy.execution_models.as_ref().unwrap().contains(&m.id));
    }
    // It prints; it does not edit the file for them.
    assert_eq!(std::fs::read_to_string(d.join("backspace.toml")).unwrap(), base);
}

#[test]
fn policy_adopt_records_a_loosening_a_person_has_reviewed() {
    let d = dir("adopt");
    let base = backspace_core::config::DEFAULT_CONFIG;
    let with = |models: &str| format!("{base}\n[policy]\nmode = \"enforced\"\norchestration_models = {models}\nexecution_models = {models}\nskills = []\nplugins = []\n");
    let id = Config::parse(base).unwrap().models[0].id.clone();
    let other = Config::parse(base).unwrap().models[1].id.clone();
    std::env::set_var("BACKSPACE_DATA", d.join("data"));

    std::fs::write(d.join("backspace.toml"), with(&format!("[\"{id}\"]"))).unwrap();
    let o = cli(&d, &d, &["policy", "adopt"]);
    assert!(o.status.success(), "{}", out(&o));
    assert!(lock_path(&d).is_file(), "the lock is written outside the workspace");

    // A wider file is not in force until a person adopts it.
    let wider = with(&format!("[\"{id}\", \"{other}\"]"));
    std::fs::write(d.join("backspace.toml"), &wider).unwrap();
    let file = Config::parse(&wider).unwrap().policy;
    let (policy, _) = resolve_policy(&d, &file).unwrap();
    assert_eq!(policy.execution_models, Some(vec![id.clone()]));

    let o = cli(&d, &d, &["policy", "adopt"]);
    assert!(o.status.success(), "{}", out(&o));
    let said = out(&o);
    assert!(said.contains("execution_models") && said.contains(&other), "it tells them what they are accepting: {said}");
    let (policy, notices) = resolve_policy(&d, &file).unwrap();
    assert_eq!(policy.execution_models, Some(vec![id, other]));
    assert!(notices.iter().all(|n| n.level != backspace_core::gateway::NoticeLevel::Warning), "{notices:?}");
}
