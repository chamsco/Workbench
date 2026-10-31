//! M1 final hardening, decision C: the Jev router's own model is governed by an
//! explicit `[policy] routing_models` list, separate from the orchestration and
//! execution lists. Under an enforced policy the configured `jev_model` must be
//! named there or Jev is not called and the heuristic decides; a list naming a
//! model there never makes that model a candidate for either role; generated
//! policies name the configured routing model; a file that adds to the list is
//! a loosening and waits for `policy adopt`; legacy and open configurations
//! behave as before.
//!
//! Mock Jev on a local port. Nothing here calls a real Jev.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use backspace_core::config::{Config, DEFAULT_CONFIG};
use backspace_core::gateway::{adopt_policy, resolve_policy, Gateway, NoticeLevel, PolicyConfig, Refusal};
use backspace_core::router::{AgentRole, Decision, Router};
use backspace_core::{Harness, LogKind};
use serde_json::{json, Value};

static ENV: Once = Once::new();

fn env() {
    ENV.call_once(|| {
        let home = std::env::temp_dir().join(format!("bs-routepol-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("USERPROFILE", &home);
        std::env::set_var("BACKSPACE_DATA", home.join("data"));
        std::env::remove_var("BACKSPACE_CONFIG");
        std::env::set_var("BS_JEV_TEST_KEY", "test-key");
    });
}

struct Jev {
    port: u16,
    calls: Arc<AtomicUsize>,
    offered: Arc<Mutex<Vec<Vec<String>>>>,
}

fn jev() -> Jev {
    env();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let j = Jev { port, calls: Arc::default(), offered: Arc::default() };
    let (calls, offered) = (j.calls.clone(), j.offered.clone());
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let mut stream = stream.unwrap();
            let (calls, offered) = (calls.clone(), offered.clone());
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let out = if first.contains("/route") {
                    calls.fetch_add(1, Ordering::SeqCst);
                    if let Ok(v) = serde_json::from_slice::<Value>(&body) {
                        let ids = v["questions"]["model"]["criteria"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
                        offered.lock().unwrap().push(ids);
                    }
                    json!({"answers": {"model": {"choice": "mock-small", "confidence": 0.9}, "effort": {"score": 1.0}},
                        "usage": {"input_tokens": 100, "output_tokens": 20}})
                } else {
                    json!({"error": "not here"})
                }
                .to_string();
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    j
}

fn config(port: u16, policy: &str) -> String {
    format!(
        r#"
[router]
backend = "jev"
jev_endpoint = "http://127.0.0.1:{port}/route"
jev_model = "jev-test"
jev_api_key_env = "BS_JEV_TEST_KEY"
jev_usd_per_mtok = 2000.0
main_min_effort = "high"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
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

{policy}
"#
    )
}

/// An enforced policy that lists both models for both roles, plus whatever `extra` adds.
fn enforced(extra: &str) -> String {
    format!("[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-small\", \"mock-big\"]\nexecution_models = [\"mock-small\", \"mock-big\"]\nskills = []\nplugins = []\n{extra}")
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap()
}

fn route(j: &Jev, policy: &str) -> Decision {
    let cfg = Config::parse(&config(j.port, policy)).unwrap();
    rt().block_on(Router::new(reqwest::Client::new(), cfg).route(AgentRole::Main, "fix a typo in the readme", "")).unwrap()
}

// ------------------------------------------------------------ the gateway

fn pol(src: &str) -> PolicyConfig {
    #[derive(serde::Deserialize)]
    struct Wrap {
        #[serde(default)]
        policy: PolicyConfig,
    }
    toml::from_str::<Wrap>(src).unwrap().policy
}

#[test]
fn an_enforced_policy_allows_only_the_routing_models_it_names() {
    let g = Gateway::new(pol(&enforced("routing_models = [\"jev-test\"]\n")));
    assert!(g.check_routing_model("jev-test").is_ok());
    let err = g.check_routing_model("jev-other").unwrap_err();
    assert!(matches!(err, Refusal::RoutingModel { .. }), "{err:?}");
    assert!(err.to_string().contains("refused by project policy") && err.to_string().contains("jev-other") && err.to_string().contains("routing"), "{err}");
}

#[test]
fn under_an_enforced_policy_no_list_and_an_empty_list_both_allow_nothing() {
    for extra in ["", "routing_models = []\n"] {
        let g = Gateway::new(pol(&enforced(extra)));
        assert!(g.check_routing_model("jev-test").is_err(), "`{extra}`: enforced means a model is allowed only if a list names it");
    }
}

#[test]
fn without_an_enforced_policy_the_routing_model_is_open_unless_a_list_says_otherwise() {
    // No [policy] at all, a partial policy with no mode, and legacy_open: open, as before.
    for src in ["", "[policy]\nskills = [\"tdd\"]\n", "[policy]\nmode = \"legacy_open\"\n"] {
        assert!(Gateway::new(pol(src)).check_routing_model("anything").is_ok(), "`{src}`");
    }
    // A list that is there is the whole of what is allowed, mode or not.
    let g = Gateway::new(pol("[policy]\nrouting_models = [\"jev-test\"]\n"));
    assert!(g.check_routing_model("jev-test").is_ok() && g.check_routing_model("jev-other").is_err());
}

#[test]
fn the_routing_model_is_matched_exactly() {
    let g = Gateway::new(pol(&enforced("routing_models = [\"jev-test\"]\n")));
    for near in ["JEV-TEST", " jev-test", "jev-test ", "jev"] {
        assert!(g.check_routing_model(near).is_err(), "`{near}`");
    }
}

// ----------------------------------------------------------- the router

#[test]
fn a_named_routing_model_is_called() {
    let j = jev();
    let d = route(&j, &enforced("routing_models = [\"jev-test\"]\n"));
    assert_eq!(j.calls.load(Ordering::SeqCst), 1, "{:?}", d.note);
    assert_eq!(d.source, "jev", "{:?}", d.note);
}

#[test]
fn an_enforced_policy_that_does_not_name_it_never_calls_jev_and_the_heuristic_decides() {
    for (what, extra) in [("no routing list", ""), ("an empty list", "routing_models = []\n"), ("another model", "routing_models = [\"jev-other\"]\n")] {
        let j = jev();
        let d = route(&j, &enforced(extra));
        assert_eq!(j.calls.load(Ordering::SeqCst), 0, "{what}: not one request went to Jev");
        assert!(d.source.starts_with("heuristic"), "{what}: {}", d.source);
        let note = d.note.clone().unwrap_or_default();
        assert!(note.contains("refused by project policy") && note.contains("jev-test") && note.contains("routing"), "{what}: the note says why: {note}");
        assert_eq!(d.router_cost_usd, 0.0, "{what}: nothing was spent");
    }
}

#[test]
fn naming_jev_in_the_ordinary_role_lists_does_not_permit_it() {
    // `jev-test` in orchestration and execution is not a routing permission.
    let j = jev();
    let policy = "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-small\", \"jev-test\"]\nexecution_models = [\"mock-small\", \"jev-test\"]\nskills = []\nplugins = []\n";
    let d = route(&j, policy);
    assert_eq!(j.calls.load(Ordering::SeqCst), 0);
    assert!(d.source.starts_with("heuristic"));
}

#[test]
fn the_routing_list_does_not_widen_what_jev_may_choose() {
    // `mock-big` is named for routing only. It is not an orchestration or execution model, so Jev is never offered it.
    let j = jev();
    let policy = "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-small\"]\nexecution_models = [\"mock-small\"]\nskills = []\nplugins = []\nrouting_models = [\"jev-test\", \"mock-big\"]\n";
    let d = route(&j, policy);
    assert_eq!(d.source, "jev");
    assert_eq!(j.offered.lock().unwrap().clone(), vec![vec!["mock-small".to_string()]], "the candidates are decided by the role lists alone");
}

#[test]
fn legacy_and_open_configurations_still_call_jev() {
    for (what, policy) in [
        ("no [policy]", String::new()),
        ("a partial policy", "[policy]\nskills = [\"tdd\"]\n".to_string()),
        ("legacy_open", "[policy]\nmode = \"legacy_open\"\n".to_string()),
        ("legacy_open with role lists", "[policy]\nmode = \"legacy_open\"\norchestration_models = [\"mock-small\"]\n".to_string()),
    ] {
        let j = jev();
        let d = route(&j, &policy);
        assert_eq!(j.calls.load(Ordering::SeqCst), 1, "{what}: {:?}", d.note);
        assert_eq!(d.source, "jev", "{what}");
    }
}

// ---------------------------------------------------- generated policies

#[test]
fn a_generated_policy_names_the_configured_routing_model() {
    let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
    assert_eq!(cfg.router.jev_model, "jev-latest");
    let p = cfg.effective_policy(&[]);
    assert_eq!(p.routing_models, Some(vec!["jev-latest".to_string()]), "the intended routing model, and no other");
    assert!(!p.orchestration_models.unwrap().contains(&"jev-latest".to_string()), "and the role lists are not widened to make Jev pass");
    let block = cfg.policy_init_block(&[]);
    assert!(block.contains("routing_models = [\"jev-latest\"]"), "`policy init` writes it down:\n{block}");
    // The block reads back to the same policy.
    let back = pol(&block);
    assert_eq!(back.routing_models, Some(vec!["jev-latest".to_string()]));
    assert!(back.check_complete().is_ok());
}

#[test]
fn a_new_project_is_enforced_and_lets_its_configured_router_run() {
    env();
    let ws = std::env::temp_dir().join(format!("bs-routepol-new-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    let cfg = Config::new_project(&ws);
    let g = Gateway::new(cfg.policy.clone());
    assert!(g.check_routing_model(&cfg.router.jev_model).is_ok(), "{:?}", cfg.policy);
    assert!(g.check_routing_model("something-else").is_err());
}

#[test]
fn a_list_the_person_wrote_is_not_widened() {
    let src = format!("{DEFAULT_CONFIG}\n{}", enforced("routing_models = []\n"));
    let p = Config::parse(&src).unwrap().effective_policy(&[]);
    assert_eq!(p.routing_models, Some(vec![]), "an empty list is the person's word that no routing model runs");
    // And one without the list stays closed in the generated form only if the person left Jev out on purpose:
    // `effective_policy` writes out what is allowed now, which for an enforced file with no list is nothing.
    let src = format!("{DEFAULT_CONFIG}\n{}", enforced(""));
    let cfg = Config::parse(&src).unwrap();
    let g = Gateway::new(cfg.effective_policy(&[]));
    assert!(g.check_routing_model(&cfg.router.jev_model).is_err(), "an enforced file that does not name it is not turned into one that does");
}

// -------------------------------------------- adoption: the list is policy

fn dir(tag: &str) -> std::path::PathBuf {
    env();
    let ws = std::env::temp_dir().join(format!("bs-routepol-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::canonicalize(&ws).unwrap()
}

#[test]
fn adding_a_routing_model_is_a_loosening_that_waits_for_a_person() {
    let ws = dir("adopt");
    let locked = pol(&enforced(""));
    resolve_policy(&ws, &locked).unwrap();
    let wider = pol(&enforced("routing_models = [\"jev-test\"]\n"));
    let (in_force, notices) = resolve_policy(&ws, &wider).unwrap();
    assert_eq!(in_force, locked, "the file that adds a routing model is not in force");
    let text = notices.iter().filter(|n| n.level == NoticeLevel::Warning).map(|n| n.text.clone()).collect::<Vec<_>>().join("\n");
    assert!(text.contains("routing_models") && text.contains("adopt"), "{text}");
    // A person adopting it makes it take effect.
    let loosened = adopt_policy(&ws, &wider).unwrap();
    assert!(loosened.iter().any(|l| l.contains("routing_models")), "{loosened:?}");
    assert_eq!(resolve_policy(&ws, &wider).unwrap().0, wider);
}

#[test]
fn removing_a_routing_model_tightens_and_is_adopted_by_itself() {
    let ws = dir("tighten");
    resolve_policy(&ws, &pol(&enforced("routing_models = [\"jev-test\"]\n"))).unwrap();
    let narrower = pol(&enforced("routing_models = []\n"));
    let (in_force, notices) = resolve_policy(&ws, &narrower).unwrap();
    assert_eq!(in_force, narrower);
    assert!(notices.is_empty(), "{notices:?}");
}

#[test]
fn leaving_the_list_out_of_an_enforced_file_is_not_a_loosening() {
    // For an enforced policy, no list and an empty list mean the same, so neither one opens anything.
    let ws = dir("same");
    resolve_policy(&ws, &pol(&enforced("routing_models = []\n"))).unwrap();
    let (_, notices) = resolve_policy(&ws, &pol(&enforced(""))).unwrap();
    assert!(notices.is_empty(), "{notices:?}");
}

#[test]
fn dropping_enforcement_while_the_list_is_absent_is_a_loosening() {
    let ws = dir("mode");
    resolve_policy(&ws, &pol(&enforced(""))).unwrap();
    // Without `mode = "enforced"` a missing routing list is an open one.
    let open = pol("[policy]\norchestration_models = [\"mock-small\", \"mock-big\"]\nexecution_models = [\"mock-small\", \"mock-big\"]\nskills = []\nplugins = []\n");
    let (in_force, notices) = resolve_policy(&ws, &open).unwrap();
    assert_eq!(in_force, pol(&enforced("")), "the locked policy stays");
    assert!(!notices.is_empty());
}

// ---------------------------------------------------------- a real harness

fn open(tag: &str, cfg: String) -> Harness {
    let ws = dir(&format!("h-{tag}"));
    std::fs::write(ws.join("backspace.toml"), cfg).unwrap();
    Harness::open(ws).unwrap()
}

fn routed(h: &Harness) -> String {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let s = h.snapshot();
        let text = s.agents[0].log.iter().filter(|e| e.kind == LogKind::System && e.text.starts_with("routed by")).map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
        if !text.is_empty() {
            return text;
        }
        assert!(Instant::now() < deadline, "never routed: {:?}", s.agents[0].log.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_project_that_does_not_name_its_routing_model_says_so_when_it_opens_and_never_calls_jev() {
    let j = jev();
    let h = open("unnamed", config(j.port, &enforced("")));
    let notes = h.policy_notices().into_iter().map(|n| n.text).collect::<Vec<_>>().join("\n");
    assert!(notes.contains("routing_models") && notes.contains("jev-test"), "the person is told when the project opens: {notes}");
    h.send("fix a typo in the readme");
    let text = routed(&h);
    assert!(text.contains("routed by heuristic") && text.contains("routing"), "{text}");
    assert_eq!(j.calls.load(Ordering::SeqCst), 0);
    assert_eq!(h.snapshot().router_cost_usd, 0.0);
}

#[test]
fn a_project_that_names_it_routes_through_jev() {
    let j = jev();
    let h = open("named", config(j.port, &enforced("routing_models = [\"jev-test\"]\n")));
    let notes = h.policy_notices().into_iter().map(|n| n.text).collect::<Vec<_>>().join("\n");
    assert!(!notes.contains("routing_models"), "nothing to warn about: {notes}");
    h.send("fix a typo in the readme");
    let text = routed(&h);
    assert!(text.contains("routed by jev"), "{text}");
    assert_eq!(j.calls.load(Ordering::SeqCst), 1);
}
