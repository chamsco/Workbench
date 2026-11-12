//! M1 hardening, F1: the policy is explicit for a new project, an old project
//! keeps working but is told so, and an empty list still means "nothing".
//!
//! One process-wide environment (HOME, the data folder, no BACKSPACE_CONFIG),
//! set once, because the new-project default and the local-model addition both
//! read machine state that the other tests in this crate leave alone.

use std::path::{Path, PathBuf};
use std::sync::Once;

use backspace_core::gateway::{Gateway, NoticeLevel, PolicyConfig, PolicyMode, Role};
use backspace_core::{Config, Harness, LogKind};

static ENV: Once = Once::new();

fn env() -> &'static Path {
    static HOME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ENV.call_once(|| {
        let home = std::env::temp_dir().join(format!("bs-policy-modes-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("USERPROFILE", &home);
        std::env::set_var("BACKSPACE_DATA", home.join("data"));
        std::env::remove_var("BACKSPACE_CONFIG");
        HOME.set(home).unwrap();
    });
    HOME.get().unwrap()
}

const BASE: &str = r#"
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

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:1/v1"
api_key_env = "NONE"

[[models]]
id = "mock-small"
provider = "mock"
tier = 1
description = "cheap"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000

[[models]]
id = "mock-big"
provider = "mock"
tier = 4
description = "strong"
input_usd_per_mtok = 10.0
output_usd_per_mtok = 20.0
max_output_tokens = 1000
"#;

fn workspace(tag: &str, config: &str) -> PathBuf {
    env();
    let ws = std::env::temp_dir().join(format!("bs-modes-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    if !config.is_empty() {
        std::fs::write(ws.join("backspace.toml"), config).unwrap();
    }
    ws
}

fn with_policy(policy: &str) -> String {
    format!("{BASE}\n{policy}\n")
}

fn warnings(h: &Harness) -> Vec<String> {
    h.policy_notices().into_iter().filter(|n| n.level == NoticeLevel::Warning).map(|n| n.text).collect()
}

// ------------------------------------------------------------- old projects

#[test]
fn no_policy_opens_with_a_visible_warning() {
    let ws = workspace("none", BASE);
    let h = Harness::open(ws).unwrap();
    let w = warnings(&h).join("\n");
    assert!(w.contains("no [policy]"), "{w}");
    for dim in ["orchestration_models", "execution_models", "skills", "plugins"] {
        assert!(w.contains(dim), "the warning names {dim}: {w}");
    }
    assert!(w.contains("legacy_open") && w.contains("enforced"), "it says how to settle it: {w}");
    // The user sees it where they look: the main agent's log, not only a return value.
    let log = h.snapshot().agents[0].log.iter().filter(|e| e.kind == LogKind::System).map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
    assert!(log.contains("no [policy]"), "{log}");
}

#[test]
fn partial_policy_warns_for_each_open_list() {
    let ws = workspace("partial", &with_policy("[policy]\nskills = [\"tdd\"]\n"));
    let h = Harness::open(ws).unwrap();
    let w = warnings(&h).join("\n");
    for open in ["orchestration_models", "execution_models", "plugins"] {
        assert!(w.contains(open), "{open} is open and must be named: {w}");
    }
    assert!(!w.contains("skills"), "skills is a list, so it is not named as open: {w}");
}

#[test]
fn legacy_open_mode_is_a_note_not_a_warning() {
    let ws = workspace("legacy", &with_policy("[policy]\nmode = \"legacy_open\"\n"));
    let h = Harness::open(ws).unwrap();
    assert!(warnings(&h).is_empty(), "{:?}", warnings(&h));
    let notes: Vec<_> = h.policy_notices().into_iter().filter(|n| n.level == NoticeLevel::Note).collect();
    assert!(!notes.is_empty(), "choosing legacy-open is still recorded");
    // It behaves exactly as the old rules did: nothing is restricted.
    let g = Gateway::new(h.policy());
    assert!(g.check_model(Role::Execution, "mock-big").is_ok() && g.skill_allowed("tdd"));
}

#[test]
fn a_policy_with_every_list_has_nothing_to_warn_about() {
    let ws = workspace("full", &with_policy("[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-big\"]\nexecution_models = [\"mock-small\"]\nskills = [\"tdd\"]\nplugins = []\n"));
    let h = Harness::open(ws).unwrap();
    assert!(warnings(&h).is_empty(), "{:?}", warnings(&h));
}

// ---------------------------------------------------------------- enforced

#[test]
fn enforced_mode_needs_every_list() {
    let err = Config::parse(&with_policy("[policy]\nmode = \"enforced\"\nskills = [\"tdd\"]\n")).unwrap_err();
    let e = format!("{err:#}");
    for missing in ["orchestration_models", "execution_models", "plugins"] {
        assert!(e.contains(missing), "the error names {missing}: {e}");
    }
    assert!(!e.contains("`skills`"), "skills is there: {e}");
    // And the project does not open on it.
    let ws = workspace("enforced-bad", &with_policy("[policy]\nmode = \"enforced\"\n"));
    assert!(Harness::open(ws).is_err());
    // All four, empty or not, is complete.
    Config::parse(&with_policy("[policy]\nmode = \"enforced\"\norchestration_models = []\nexecution_models = []\nskills = []\nplugins = []\n")).unwrap();
}

#[test]
fn an_empty_list_still_denies_everything() {
    let cfg = Config::parse(&with_policy(
        "[policy]\nmode = \"enforced\"\norchestration_models = []\nexecution_models = []\nskills = []\nplugins = []\n",
    ))
    .unwrap();
    let g = Gateway::new(cfg.policy.clone());
    for role in [Role::Orchestration, Role::Execution] {
        for m in ["mock-small", "mock-big", ""] {
            assert!(g.check_model(role, m).is_err(), "{role:?} {m}");
        }
        assert!(g.check_plugin(role, "anything").is_err());
        assert!(g.check_skill(role, "tdd").is_err());
    }
    assert!(!g.skill_allowed("tdd"));
    // Unqualified legacy rules agree: present-and-empty is not open.
    let legacy = Config::parse(&with_policy("[policy]\nmode = \"legacy_open\"\nexecution_models = []\n")).unwrap();
    assert!(Gateway::new(legacy.policy).check_model(Role::Execution, "mock-small").is_err());
}

// ------------------------------------------------------------- new projects

#[test]
fn new_project_default_is_enforced_with_explicit_lists() {
    let ws = workspace("new", "");
    let (cfg, source) = Config::load(&ws).unwrap();
    assert!(source.is_none(), "no config file was found, so this is the new-project default");
    assert_eq!(cfg.policy.mode, Some(PolicyMode::Enforced));
    let (orch, exec, skills, plugins) = (
        cfg.policy.orchestration_models.clone().expect("explicit"),
        cfg.policy.execution_models.clone().expect("explicit"),
        cfg.policy.skills.clone().expect("explicit"),
        cfg.policy.plugins.clone().expect("explicit"),
    );
    for m in &cfg.models {
        assert!(orch.contains(&m.id) && exec.contains(&m.id), "{} is a model the default config offers", m.id);
    }
    assert!(skills.contains(&"tdd".to_string()), "{skills:?}");
    assert!(plugins.is_empty(), "no plugin loader exists yet, so none is allowed: {plugins:?}");
    // A project opened on it has nothing to warn about.
    let h = Harness::open(ws).unwrap();
    assert!(warnings(&h).is_empty(), "{:?}", warnings(&h));
    assert_eq!(h.policy().mode, Some(PolicyMode::Enforced));
}

#[test]
fn the_shipped_default_file_is_not_a_policy() {
    // Existing tests build on default-config.toml and append their own [policy]:
    // it must stay free of one.
    let cfg = Config::parse(backspace_core::config::DEFAULT_CONFIG).unwrap();
    assert_eq!(cfg.policy, PolicyConfig::default());
}

#[test]
fn policy_init_reproduces_the_effective_permissions() {
    let cfg = Config::parse(BASE).unwrap();
    let skills: Vec<String> = ["tdd", "grilling"].map(String::from).to_vec();
    let block = cfg.policy_init_block(&skills);
    assert!(block.contains("[policy]") && block.contains("mode = \"enforced\""), "{block}");
    let after = Config::parse(&format!("{BASE}\n{block}")).unwrap();
    let (before_g, after_g) = (Gateway::new(cfg.policy.clone()), Gateway::new(after.policy.clone()));
    for m in &cfg.models {
        for role in [Role::Orchestration, Role::Execution] {
            assert_eq!(before_g.check_model(role, &m.id).is_ok(), after_g.check_model(role, &m.id).is_ok(), "{role:?} {}", m.id);
        }
    }
    for s in &skills {
        assert_eq!(before_g.skill_allowed(s), after_g.skill_allowed(s), "{s}");
    }
    assert_eq!(after.policy.plugins, Some(vec![]), "the old rules allowed no loader, and none is added");
    // A list the project already has is carried over as written, not widened.
    let partial = Config::parse(&with_policy("[policy]\nskills = [\"tdd\"]\n")).unwrap();
    let block = partial.policy_init_block(&skills);
    let after = Config::parse(&format!("{}\n{block}", with_policy(""))).unwrap();
    assert_eq!(after.policy.skills, Some(vec!["tdd".to_string()]), "{block}");
    assert!(after.policy.orchestration_models.as_ref().unwrap().contains(&"mock-big".to_string()));
}

#[test]
fn user_enabled_local_models_stay_usable_under_an_execution_list() {
    let home = env().to_path_buf();
    std::fs::create_dir_all(home.join(".config/backspace")).unwrap();
    std::fs::write(
        home.join(".config/backspace/prefs.json"),
        r#"{"subagent_model": "ollama:tiny", "local_tasks": ["scouts"]}"#,
    )
    .unwrap();
    let ws = workspace(
        "local",
        &with_policy("[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-big\"]\nexecution_models = [\"mock-small\"]\nskills = []\nplugins = []\n"),
    );
    let h = Harness::open(ws).unwrap();
    let p = h.policy();
    assert!(p.execution_models.as_ref().unwrap().contains(&"ollama:tiny".to_string()), "{p:?}");
    assert_eq!(p.orchestration_models, Some(vec!["mock-big".to_string()]), "it is a worker's model, not a planner's");
    let note = h.policy_notices().into_iter().map(|n| n.text).collect::<Vec<_>>().join("\n");
    assert!(note.contains("ollama:tiny"), "the addition is stated: {note}");
    drop(h);

    // An empty list is the user's word that nothing runs; a Settings toggle does not overrule it.
    let ws = workspace(
        "local-empty",
        &with_policy("[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-big\"]\nexecution_models = []\nskills = []\nplugins = []\n"),
    );
    let h = Harness::open(ws).unwrap();
    assert_eq!(h.policy().execution_models, Some(vec![]));
    std::fs::remove_file(home.join(".config/backspace/prefs.json")).unwrap();
}

#[test]
fn skills_present_but_not_allowed_are_named() {
    let ws = workspace("skills", &with_policy("[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-big\"]\nexecution_models = [\"mock-small\"]\nskills = [\"tdd\"]\nplugins = []\n"));
    std::fs::create_dir_all(ws.join(".backspace/skills/house-style")).unwrap();
    std::fs::write(ws.join(".backspace/skills/house-style/SKILL.md"), "---\nname: house-style\ndescription: how we write\n---\nbody\n").unwrap();
    let h = Harness::open(ws).unwrap();
    let w = warnings(&h).join("\n");
    assert!(w.contains("house-style") && w.contains("not allowed"), "{w}");
}
