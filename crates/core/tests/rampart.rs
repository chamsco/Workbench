//! M5 acceptance (PLAN §7, §12): Builder and Rampart.
//!
//! * Two separated orchestrators. Rampart is its own agent, started on a
//!   cheap rung, given the approved spec.md and the baseline and nothing of
//!   Builder's; in code it may only read its baseline checkout and submit a
//!   suite. Builder's workers cannot start until the suite is sealed.
//! * Tests are sealed and hash-locked. Builder cannot change, skip or delete
//!   a sealed test: its write is refused, and a delivery whose sealed files
//!   differ from their hashes is refused naming them.
//! * The ramparts are validated before sealing: every acceptance criterion
//!   has a test, every test fails on the baseline, and each runs three times
//!   (flake check). A person spot-checks the first suites. Once Builder
//!   passes, mutation testing on the changed lines gives the rampart score.
//! * Escalation triggers per §7.4: N failures, the same failure signature
//!   twice in a row, the cheap budget spent, or a frontier call expected to
//!   be cheaper than another retry. Then ONE frontier call with a failure
//!   digest; if that fails too, a human gets the digest. No frontier loops.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use backspace_core::planning::{self, PlanningState};
use backspace_core::rampart::{self, Attempt, Limit, Next, RampartConfig, RampartState, Suite, Trigger, Verdict};
use backspace_core::tools::Workspace;
use backspace_core::{AgentKind, AgentStatus, Harness, LogKind, ProjectState, TicketState};
use serde_json::{json, Value};

// ------------------------------------------------------------------ helpers

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bs-rampart-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ws(root: &Path) -> Workspace {
    Workspace { root: root.to_path_buf(), bash_timeout: Duration::from_secs(10) }
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
}

/// The plan every test here works from: app.sh prints hello.
fn plan_json() -> Value {
    json!({
        "title": "Greeting script",
        "goal": "app.sh prints hello so that the greeting can be scripted.",
        "acceptance": [
            {"criterion": "app.sh prints hello", "test": "sh app.sh | grep -q hello"},
            {"criterion": "app.sh exits 0", "test": "sh app.sh"}
        ],
        "non_goals": ["Translations."],
        "constraints": ["Plain sh."],
        "dependencies": ["None"],
        "baseline": {"kind": "feature"}
    })
}

fn spec_md() -> String {
    let p = planning::PlanSpec::from_json(&plan_json()).unwrap();
    let b = planning::Baseline { kind: "feature".into(), ok: true, note: "2 acceptance tests fail today".into(), ..Default::default() };
    let c = planning::check(&p, Some(&b));
    planning::render(&p, &b, &c)
}

/// A1: prints exactly hello (and says what it got); A2: exits 0.
fn good_suite() -> Value {
    json!({"tests": [
        {"path": "tests/a1.sh", "content": "out=$(sh app.sh 2>&1)\n[ \"$out\" = \"hello\" ] || { echo \"got $out\"; exit 1; }\n", "command": "sh tests/a1.sh", "criteria": ["A1"]},
        {"path": "tests/a2.sh", "content": "sh app.sh >/dev/null\n", "command": "sh tests/a2.sh", "criteria": ["A2"]}
    ]})
}

fn suite(v: Value) -> Suite {
    Suite::from_json(&v).unwrap()
}

// --------------------------------------------------------- suite and brief

#[test]
fn the_criteria_are_read_from_spec_md() {
    assert_eq!(rampart::criteria(&spec_md()), ["A1", "A2"]);
}

#[test]
fn a_suite_is_typed_and_its_shape_is_checked_before_anything_runs() {
    assert!(Suite::from_json(&json!({"tests": "x"})).is_err());
    assert!(Suite::from_json(&json!({"tests": [{"path": "a.sh"}]})).is_err(), "a test needs content, a command and criteria");
    assert!(rampart::shape_gaps(&suite(good_suite()), &spec_md()).is_empty(), "{:?}", rampart::shape_gaps(&suite(good_suite()), &spec_md()));

    let bad = |f: &dyn Fn(&mut Value)| {
        let mut v = good_suite();
        f(&mut v);
        rampart::shape_gaps(&suite(v), &spec_md()).join("\n")
    };
    for p in ["../a.sh", "/tmp/a.sh", ".backspace/x.sh", ".git/hooks/x", ""] {
        let g = bad(&|v| v["tests"][0]["path"] = json!(p));
        assert!(g.contains("path"), "{p}: {g}");
    }
    let g = bad(&|v| v["tests"][1]["path"] = json!("tests/a1.sh"));
    assert!(g.contains("tests/a1.sh") && g.to_lowercase().contains("twice"), "{g}");
    let g = bad(&|v| v["tests"][0]["command"] = json!("  "));
    assert!(g.to_lowercase().contains("command"), "{g}");
    let g = bad(&|v| v["tests"][0]["content"] = json!(""));
    assert!(g.to_lowercase().contains("empty"), "{g}");
    let g = bad(&|v| v["tests"][0]["criteria"] = json!(["A9"]));
    assert!(g.contains("A9") && g.contains("A1"), "an unknown criterion, and A1 left with no test: {g}");
    // Traceability: every criterion needs a test.
    let g = bad(&|v| v["tests"][1]["criteria"] = json!(["A1"]));
    assert!(g.contains("A2") && g.to_lowercase().contains("no test"), "{g}");
}

#[test]
fn the_suite_hash_covers_paths_contents_and_commands() {
    let a = suite(good_suite()).sha256();
    assert_eq!(a.len(), 64);
    assert_eq!(a, suite(good_suite()).sha256());
    for (k, v) in [("path", json!("tests/b1.sh")), ("content", json!("exit 1\n")), ("command", json!("bash tests/a1.sh"))] {
        let mut s = good_suite();
        s["tests"][0][k] = v;
        assert_ne!(a, suite(s).sha256(), "{k}");
    }
}

fn approved(root: &Path) -> PlanningState {
    std::fs::create_dir_all(root.join(planning::DIR)).unwrap();
    let md = spec_md();
    std::fs::write(root.join(planning::SPEC), &md).unwrap();
    std::fs::write(root.join(planning::BASELINE), r#"{"kind":"feature","ok":true,"note":"2 acceptance tests fail today"}"#).unwrap();
    let h = planning::sha256_hex(md.as_bytes());
    PlanningState { spec_sha256: Some(h.clone()), passed: true, approved_sha256: Some(h), ..Default::default() }
}

#[test]
fn rampart_is_given_the_approved_spec_and_the_baseline_and_nothing_else() {
    let root = scratch("brief");
    let st = approved(&root);
    let b = rampart::brief(&st, &root).unwrap();
    assert_eq!(b.spec, spec_md());
    assert_eq!(b.spec_sha256, st.approved_sha256.clone().unwrap());
    assert!(b.baseline.contains("2 acceptance tests fail today"));
    let m = b.message();
    assert!(m.contains(&spec_md()) && m.contains("2 acceptance tests fail today"), "{m}");

    // Not approved, or changed since the yes: nothing is given.
    let unapproved = PlanningState { approved_sha256: None, ..st.clone() };
    assert!(rampart::brief(&unapproved, &root).is_err());
    std::fs::write(root.join(planning::SPEC), "# edited\n").unwrap();
    let e = rampart::brief(&st, &root).unwrap_err().to_string();
    assert!(e.contains("spec.md"), "{e}");
}

// ----------------------------------------------------------------- vetting

fn tree(tag: &str) -> PathBuf {
    let t = scratch(tag);
    std::fs::write(t.join("README.md"), "before\n").unwrap();
    t
}

#[test]
fn every_test_runs_three_times_on_the_baseline_and_must_fail_every_time() {
    let t = tree("vet-ok");
    let v = rt().block_on(rampart::vet(&suite(good_suite()), &ws(&t), 3)).unwrap();
    assert_eq!(v.tests.len(), 2);
    for x in &v.tests {
        assert_eq!(x.exits.len(), 3, "{x:?}");
        assert!(x.exits.iter().all(|e| *e != 0), "{x:?}");
        assert_eq!(x.verdict(), Verdict::FailsOnBaseline);
    }
    assert!(v.passed() && v.gaps().is_empty(), "{:?}", v.gaps());
    // The tree is the baseline again afterwards.
    assert!(!t.join("tests/a1.sh").exists() && !t.join("tests/a2.sh").exists());
    assert_eq!(std::fs::read_to_string(t.join("README.md")).unwrap(), "before\n");
}

#[test]
fn a_test_that_passes_on_the_baseline_proves_nothing_and_is_refused() {
    let t = tree("vet-pass");
    let mut s = good_suite();
    s["tests"][1]["content"] = json!("true\n");
    let v = rt().block_on(rampart::vet(&suite(s), &ws(&t), 3)).unwrap();
    assert_eq!(v.tests[1].verdict(), Verdict::PassesOnBaseline);
    assert!(!v.passed());
    let g = v.gaps().join("\n");
    assert!(g.contains("tests/a2.sh") && g.to_lowercase().contains("passes on the baseline"), "{g}");
    assert!(!g.contains("tests/a1.sh"), "only the bad test is named: {g}");
}

#[test]
fn a_test_that_does_not_fail_every_time_is_flaky_and_refused() {
    let t = tree("vet-flaky");
    let mut s = good_suite();
    // Fails, passes, fails: a counter kept beside it.
    s["tests"][1]["content"] = json!("c=$(cat .n 2>/dev/null || echo 0); echo $((c+1)) > .n; [ $((c % 2)) -eq 1 ]\n");
    let v = rt().block_on(rampart::vet(&suite(s), &ws(&t), 3)).unwrap();
    assert_eq!(v.tests[1].exits.len(), 3);
    assert_eq!(v.tests[1].verdict(), Verdict::Flaky, "{:?}", v.tests[1]);
    assert!(!v.passed());
    assert!(v.gaps().join("\n").to_lowercase().contains("flaky"), "{:?}", v.gaps());
}

// ----------------------------------------------------------------- the seal

fn sealed(tag: &str) -> (PathBuf, Suite, rampart::Seal) {
    let root = scratch(tag);
    let t = tree(&format!("{tag}-tree"));
    let s = suite(good_suite());
    let v = rt().block_on(rampart::vet(&s, &ws(&t), 3)).unwrap();
    let seal = rampart::seal(&s, &v, "spechash", "abc123", "mock-mid", true).unwrap();
    rampart::write(&root, &s, &seal).unwrap();
    (root, s, seal)
}

#[test]
fn a_vetted_suite_is_sealed_with_a_hash_per_file() {
    let (root, s, seal) = sealed("seal");
    assert_eq!(seal.suite_sha256, s.sha256());
    assert_eq!(seal.spec_sha256, "spechash");
    assert_eq!(seal.base, "abc123");
    assert_eq!(seal.rampart_model, "mock-mid");
    assert!(seal.human_checked);
    assert_eq!(seal.files.len(), 2);
    for (f, t) in seal.files.iter().zip(&s.tests) {
        assert_eq!(f.path, t.path);
        assert_eq!(f.sha256, planning::sha256_hex(t.content.as_bytes()));
        assert_eq!(f.command, t.command);
        assert_eq!(f.criteria, t.criteria);
        assert_eq!(std::fs::read_to_string(root.join(rampart::SEALED_DIR).join(&f.path)).unwrap(), t.content);
    }
    let on_disk: rampart::Seal = serde_json::from_str(&std::fs::read_to_string(root.join(rampart::SEAL)).unwrap()).unwrap();
    assert_eq!(on_disk, seal);
    assert!(rampart::is_sealed(&seal, "tests/a1.sh") && rampart::is_sealed(&seal, "./tests/a1.sh"));
    assert!(!rampart::is_sealed(&seal, "app.sh"));
}

#[test]
fn a_suite_that_did_not_pass_vetting_cannot_be_sealed() {
    let t = tree("seal-refused");
    let mut v = good_suite();
    v["tests"][0]["content"] = json!("true\n");
    let s = suite(v);
    let vet = rt().block_on(rampart::vet(&s, &ws(&t), 3)).unwrap();
    let e = rampart::seal(&s, &vet, "h", "b", "m", false).unwrap_err().to_string();
    assert!(e.to_lowercase().contains("passes on the baseline"), "{e}");
}

#[test]
fn sealed_tests_are_hash_locked() {
    let (root, _, seal) = sealed("verify");
    let dir = scratch("verify-builder");
    rampart::place(&dir, &root, &seal).unwrap();
    assert!(rampart::verify(&dir, &seal).is_empty());

    std::fs::write(dir.join("tests/a1.sh"), "exit 0\n").unwrap();
    let v = rampart::verify(&dir, &seal);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].contains("tests/a1.sh") && v[0].to_lowercase().contains("changed"), "{v:?}");

    std::fs::remove_file(dir.join("tests/a2.sh")).unwrap();
    let v = rampart::verify(&dir, &seal).join("\n");
    assert!(v.contains("tests/a2.sh") && v.to_lowercase().contains("missing"), "{v}");
}

#[test]
fn builder_waits_for_the_seal_and_the_first_suites_wait_for_a_person() {
    let mut st = RampartState::default();
    let why = rampart::work_refusal(&st).expect("no seal, no work");
    assert!(why.contains("Rampart"), "{why}");

    let cfg = RampartConfig::default();
    assert_eq!((cfg.runs, cfg.spot_checks, cfg.max_failures), (3, 3, 3));
    assert!(rampart::needs_spot_check(&st, &cfg));
    st.suites_checked = 3;
    assert!(!rampart::needs_spot_check(&st, &cfg));
    st.suites_checked = 0;

    // Only a plain yes seals the pending suite.
    let pending = || rampart::Pending { suite: suite(good_suite()), vetting: Default::default(), model: "mock-mid".into(), base: "b".into() };
    st.pending = Some(pending());
    st.awaiting_yes = true;
    let (_, note, ok) = rampart::answer(&mut st, "yes but add a test for empty output");
    assert!(!ok && !st.awaiting_yes && st.pending.is_none(), "{note}");
    st.pending = Some(pending());
    st.awaiting_yes = true;
    let (pass_on, _, ok) = rampart::answer(&mut st, "yes");
    assert!(ok, "a plain yes");
    assert!(pass_on.contains("yes"));
    assert_eq!(st.suites_checked, 1);
}

// -------------------------------------------------------------- §7.4 limits

fn fail(out: &str, cost: f64) -> Attempt {
    rampart::attempt(&[("sh tests/a1.sh".into(), 1, out.into())], cost)
}

#[test]
fn a_signature_ignores_what_changes_between_identical_failures() {
    let a = rampart::signature(&[("cargo test".into(), 101, "test a ... FAILED\nfinished in 0.12s at /tmp/x-1234/y\nhash 9f8e7d6c5b4a".into())]);
    let b = rampart::signature(&[("cargo test".into(), 101, "test a ... FAILED\nfinished in 3.40s at /tmp/x-9876/y\nhash 0123456789ab".into())]);
    assert_eq!(a, b);
    let c = rampart::signature(&[("cargo test".into(), 101, "test b ... FAILED\nfinished in 0.12s".into())]);
    assert_ne!(a, c, "a different failing test is a different failure");
}

#[test]
fn n_failures_on_cheap_rungs_escalate_once_then_a_human() {
    let cfg = RampartConfig::default();
    let mut l = Limit::default();
    assert_eq!(l.record(fail("got apple", 0.01), &cfg, None), Next::Retry);
    assert_eq!(l.record(fail("got pear", 0.01), &cfg, None), Next::Retry);
    assert_eq!(l.record(fail("got plum", 0.01), &cfg, None), Next::Escalate(Trigger::Failures(3)));
    l.mark_escalated("mock-frontier");
    assert_eq!(l.escalated_to.as_deref(), Some("mock-frontier"));
    // After the one frontier call: a human, every time. No frontier loops.
    assert_eq!(l.record(fail("got kiwi", 0.5), &cfg, None), Next::HandOff);
    assert!(l.handed_off);
    assert_eq!(l.record(fail("got fig", 0.5), &cfg, Some(0.0001)), Next::HandOff);
}

#[test]
fn the_same_failure_twice_in_a_row_is_stagnation() {
    let cfg = RampartConfig::default();
    let mut l = Limit::default();
    assert_eq!(l.record(fail("got apple", 0.01), &cfg, None), Next::Retry);
    assert_eq!(l.record(fail("got apple", 0.01), &cfg, None), Next::Escalate(Trigger::Stagnation));
    // Not in a row: no stagnation.
    let mut l = Limit::default();
    l.record(fail("got apple", 0.01), &cfg, None);
    l.record(fail("got pear", 0.01), &cfg, None);
    let mut big = cfg.clone();
    big.max_failures = 10;
    assert_eq!(l.record(fail("got apple", 0.01), &big, None), Next::Retry);
}

#[test]
fn the_cheap_budget_spent_escalates() {
    let cfg = RampartConfig { cheap_budget_usd: Some(0.5), max_failures: 10, ..Default::default() };
    let mut l = Limit::default();
    assert_eq!(l.record(fail("got apple", 0.3), &cfg, None), Next::Retry);
    match l.record(fail("got pear", 0.3), &cfg, None) {
        Next::Escalate(Trigger::CheapBudget { spent, cap }) => {
            assert!((spent - 0.6).abs() < 1e-9 && (cap - 0.5).abs() < 1e-9, "{spent} {cap}");
        }
        n => panic!("{n:?}"),
    }
    assert!((l.cheap_spent() - 0.6).abs() < 1e-9);
}

#[test]
fn escalation_comes_sooner_when_a_frontier_call_is_expected_to_be_cheaper() {
    let cfg = RampartConfig { max_failures: 10, ..Default::default() };
    // A costly cheap rung that keeps failing against a cheap frontier call.
    let mut l = Limit::default();
    match l.record(fail("got apple", 1.0), &cfg, Some(0.01)) {
        Next::Escalate(Trigger::CheaperToEscalate { cheap, frontier }) => assert!(cheap > frontier, "{cheap} {frontier}"),
        n => panic!("{n:?}"),
    }
    // A cheap rung that is cheap stays.
    let mut l = Limit::default();
    assert_eq!(l.record(fail("got apple", 0.001), &cfg, Some(5.0)), Next::Retry);
}

#[test]
fn the_digest_carries_what_the_frontier_and_the_human_need() {
    let (_, _, seal) = sealed("digest");
    let cfg = RampartConfig::default();
    let mut l = Limit::default();
    for f in ["got apple", "got pear", "got plum"] {
        l.record(fail(f, 0.01), &cfg, None);
    }
    let d = rampart::digest("app.sh prints hello", &seal, &l);
    for want in ["app.sh prints hello", "sh tests/a1.sh", "got plum", "3", "A1"] {
        assert!(d.contains(want), "`{want}` missing from:\n{d}");
    }
    assert!(d.to_lowercase().contains(&Trigger::Failures(3).describe().to_lowercase()) || l.trigger.is_some(), "{d}");
    let mut huge = Limit::default();
    huge.record(fail(&"x".repeat(50_000), 0.01), &cfg, None);
    assert!(rampart::digest("g", &seal, &huge).chars().count() <= rampart::DIGEST_MAX);
}

// ---------------------------------------------------------------- mutation

#[test]
fn mutants_change_one_operator_or_literal_on_a_changed_line() {
    let src = "x=1\nif [ \"$a\" == \"$b\" ] && true; then echo hi; fi\n# a comment 1 == 2\ny=5\n";
    let all = rampart::mutants("app.sh", src, &[1, 2, 3, 4], 100);
    assert!(all.iter().all(|m| m.path == "app.sh" && m.content != src));
    assert!(all.iter().any(|m| m.line == 1 && m.to.contains("x=2")), "{all:?}");
    assert!(all.iter().any(|m| m.line == 2 && m.to.contains("!=")), "{all:?}");
    assert!(all.iter().any(|m| m.line == 2 && m.to.contains("||")), "{all:?}");
    assert!(all.iter().any(|m| m.line == 2 && m.to.contains("false")), "{all:?}");
    assert!(!all.iter().any(|m| m.line == 3), "comments are not mutated: {all:?}");
    for m in &all {
        let changed: Vec<usize> = src.lines().zip(m.content.lines()).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i + 1).collect();
        assert_eq!(changed, vec![m.line], "one line per mutant: {m:?}");
    }
    // Only the changed lines, at most `max`, the same every time.
    assert!(rampart::mutants("app.sh", src, &[4], 100).iter().all(|m| m.line == 4));
    assert_eq!(rampart::mutants("app.sh", src, &[1, 2, 4], 2).len(), 2);
    assert_eq!(rampart::mutants("app.sh", src, &[1, 2, 4], 100), rampart::mutants("app.sh", src, &[1, 2, 4], 100));
}

#[test]
fn changed_lines_come_from_a_zero_context_diff() {
    let diff = "diff --git a/app.sh b/app.sh\n--- a/app.sh\n+++ b/app.sh\n@@ -1,0 +1,3 @@\n+x=1\n+if\n+y\ndiff --git a/lib.sh b/lib.sh\n--- a/lib.sh\n+++ b/lib.sh\n@@ -4 +4 @@\n-a\n+b\n@@ -9,2 +10,0 @@\n-c\n-d\n";
    assert_eq!(rampart::changed_lines(diff), vec![("app.sh".to_string(), vec![1, 2, 3]), ("lib.sh".to_string(), vec![4])]);
}

#[test]
fn the_mutation_score_is_the_share_of_mutants_the_sealed_tests_catch() {
    let (root, _, seal) = sealed("mutation");
    let dir = scratch("mutation-builder");
    rampart::place(&dir, &root, &seal).unwrap();
    let app = "x=1\nif [ \"$x\" = \"1\" ]; then echo hello; fi\ny=5\n";
    std::fs::write(dir.join("app.sh"), app).unwrap();
    let m = rt().block_on(rampart::mutation_score(&ws(&dir), &seal, &[("app.sh".into(), vec![1, 2, 3])], 12)).unwrap();
    assert_eq!(m.total, 3, "{m:?}");
    assert_eq!(m.killed, 2, "{m:?}");
    assert_eq!(m.survived.len(), 1);
    assert!(m.survived[0].contains("app.sh:3"), "{m:?}");
    assert!((m.score().unwrap() - 2.0 / 3.0).abs() < 1e-9);
    assert_eq!(std::fs::read_to_string(dir.join("app.sh")).unwrap(), app, "every file is put back");
    assert_eq!(rampart::MutationScore::default().score(), None, "no mutants is no score, not 100%");
}

// ------------------------------------------------------------ the harness

struct Mock {
    port: u16,
    seen: Arc<Mutex<Vec<Value>>>,
}

fn mock(respond: impl Fn(&Value) -> Value + Send + Sync + 'static) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (s2, respond) = (seen.clone(), Arc::new(respond));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let (seen, respond) = (s2.clone(), respond.clone());
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
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
                if len == 0 {
                    return;
                }
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let req: Value = serde_json::from_slice(&body).unwrap();
                seen.lock().unwrap().push(req.clone());
                let out = respond(&req).to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).unwrap();
            });
        }
    });
    Mock { port, seen }
}

fn call(name: &str, args: Value) -> Value {
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
        {"id": format!("call_{n}"), "type": "function", "function": {"name": name, "arguments": args.to_string()}}]}}],
        "usage": {"prompt_tokens": 100, "completion_tokens": 10}})
}

fn say(t: &str) -> Value {
    json!({"choices": [{"finish_reason": "stop", "message": {"content": t}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
}

fn system(req: &Value) -> &str {
    req["messages"][0]["content"].as_str().unwrap_or("")
}

fn results(req: &Value) -> Vec<String> {
    req["messages"].as_array().unwrap().iter().filter(|m| m["role"] == "tool").map(|m| m["content"].as_str().unwrap_or("").to_string()).collect()
}

fn last_role(req: &Value) -> &str {
    req["messages"].as_array().and_then(|m| m.last()).and_then(|m| m["role"].as_str()).unwrap_or("")
}

fn users(req: &Value) -> Vec<String> {
    req["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "user")
        .map(|m| match &m["content"] {
            Value::String(s) => s.clone(),
            v => v.to_string(),
        })
        .collect()
}

fn last_call(req: &Value) -> String {
    req["messages"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|m| m["role"] == "assistant" && m["tool_calls"].is_array())
        .and_then(|m| m["tool_calls"][0]["function"]["name"].as_str())
        .unwrap_or("")
        .to_string()
}

fn tool_names(req: &Value) -> Vec<String> {
    req["tools"]
        .as_array()
        .map(|a| a.iter().filter_map(|t| t["function"]["name"].as_str().or(t["name"].as_str()).map(String::from)).collect())
        .unwrap_or_default()
}

const LONG_TASK: &str = "build a dashboard system platform across several services";

fn is_lead(req: &Value) -> bool {
    system(req).starts_with("You are the lead agent")
}

fn is_rampart(req: &Value) -> bool {
    system(req).starts_with("You are Rampart")
}

fn is_worker(req: &Value) -> bool {
    system(req).starts_with("You are a Backspace ticket agent")
}

/// The lead: plan, wait for the yes, make the one ticket, dispatch it, and
/// dispatch again when told the tests are sealed.
fn lead(req: &Value) -> Value {
    if last_role(req) == "user" {
        let u = users(req).pop().unwrap_or_default();
        if u.contains("approved the baseline commands") {
            return call("submit_spec", plan_json());
        }
        if u.contains("sealed") {
            return call("work_tickets", json!({}));
        }
        if u.contains("approved") {
            return call("create_tickets", json!({"tickets": [{"key": "greet", "title": "Write app.sh", "what_to_build": "app.sh prints hello", "acceptance": ["app.sh prints hello"]}]}));
        }
        return call("submit_spec", plan_json());
    }
    let last = results(req).pop().unwrap_or_default();
    match last_call(req).as_str() {
        "submit_spec" if last.contains("checklist passed") => say("waiting for your yes"),
        "submit_spec" => say("waiting"),
        "create_tickets" => call("work_tickets", json!({})),
        _ => say("waiting"),
    }
}

/// Rampart: a suite with a test that passes on the baseline first, then the
/// good one.
fn rampart_mock(req: &Value) -> Value {
    let rs = results(req);
    match rs.last() {
        None => {
            let mut bad = good_suite();
            bad["tests"][1]["content"] = json!("true\n");
            call("submit_ramparts", bad)
        }
        Some(r) if r.to_lowercase().contains("passes on the baseline") => call("submit_ramparts", good_suite()),
        Some(_) => say("done"),
    }
}

fn record(summary: &str) -> Value {
    json!({"summary": summary, "files": ["app.sh"], "checks_run": [], "assumptions": [], "open_questions": []})
}

/// A worker that writes app.sh printing `word` (one per attempt, by model)
/// and submits.
fn worker_writes(words: &'static [&'static str], frontier: &'static [&'static str]) -> impl Fn(&Value) -> Value + Send + Sync + 'static {
    move |req: &Value| {
        let on_frontier = req["model"] == "mock-frontier";
        let list = if on_frontier { frontier } else { words };
        // Attempts on this model so far: submits that came back.
        let msgs = req["messages"].as_array().unwrap();
        let mut n = 0;
        for m in msgs {
            if m["role"] == "user" && m["content"].as_str().is_some_and(|c| c.contains("ESCALATED")) {
                n = 0;
            }
            if m["role"] == "assistant" && m["tool_calls"][0]["function"]["name"] == "submit_deliverable" {
                n += 1;
            }
        }
        if last_call(req) == "write" {
            return call("submit_deliverable", record("wrote app.sh"));
        }
        match list.get(n) {
            Some(w) => call("write", json!({"path": "app.sh", "content": format!("x=1\nif [ \"$x\" = \"1\" ]; then echo {w}; fi\n")})),
            None => say("I give up"),
        }
    }
}

fn config_text(port: u16, extra: &str) -> String {
    format!(
        r#"
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"
pin_main = "mock-mid"
pin_sub = "mock-mid"

[orchestrator]
max_parallel_calls = 4
max_subagents = 4
auto_approve = true
bash_timeout_secs = 10

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{port}/v1"
api_key_env = "NONE"

[[models]]
id = "mock-mid"
provider = "mock"
tier = 2
description = "mid"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000

[[models]]
id = "mock-frontier"
provider = "mock"
tier = 4
description = "frontier"
input_usd_per_mtok = 15.0
output_usd_per_mtok = 75.0
max_output_tokens = 1000

[intake]
enabled = true

{extra}
"#
    )
}

fn open(tag: &str, port: u16, extra: &str) -> (Harness, PathBuf) {
    let ws = std::env::temp_dir().join(format!("bs-rampart-h-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("README.md"), "A project.\n").unwrap();
    std::fs::write(ws.join("backspace.toml"), config_text(port, extra)).unwrap();
    (Harness::open(ws.clone()).unwrap(), ws)
}

fn wait(h: &Harness, what: &str, done: impl Fn(&ProjectState) -> bool) -> ProjectState {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let s = h.snapshot();
        if done(&s) {
            return s;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:#?}",
            s.agents.iter().map(|a| (&a.key, a.status, a.log.iter().rev().take(6).collect::<Vec<_>>())).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn idle(s: &ProjectState) -> bool {
    s.agents[0].status == AgentStatus::Idle
}

/// Through planning to the Rampart suite put to the user.
fn to_suite(tag: &str, worker: impl Fn(&Value) -> Value + Send + Sync + 'static, extra: &str) -> (Mock, Harness, PathBuf) {
    let m = mock(move |req| {
        if is_lead(req) {
            lead(req)
        } else if is_rampart(req) {
            rampart_mock(req)
        } else if is_worker(req) {
            worker(req)
        } else {
            say("?")
        }
    });
    let (h, ws) = open(tag, m.port, extra);
    h.send(LONG_TASK);
    wait(&h, "the baseline commands", |s| s.planning.awaiting_commands && idle(s));
    h.send("yes");
    wait(&h, "the spec", |s| s.planning.awaiting_yes && idle(s));
    h.send("yes");
    (m, h, ws)
}

fn rampart_reqs(m: &Mock) -> Vec<Value> {
    m.seen.lock().unwrap().iter().filter(|r| is_rampart(r)).cloned().collect()
}

fn worker_reqs(m: &Mock) -> Vec<Value> {
    m.seen.lock().unwrap().iter().filter(|r| is_worker(r)).cloned().collect()
}

#[test]
fn builder_waits_while_a_separate_rampart_writes_the_tests_from_the_spec_alone() {
    let (m, h, ws) = to_suite("separate", worker_writes(&["hello"], &[]), "");
    let s = wait(&h, "the suite to be put to the user", |s| s.rampart.awaiting_yes && idle(s));

    // Builder made its ticket, but no worker started: work waits for the seal.
    assert_eq!(s.tickets.len(), 1);
    assert!(s.agents.iter().all(|a| a.kind != AgentKind::Worker), "{:?}", s.agents.iter().map(|a| a.kind).collect::<Vec<_>>());
    assert!(worker_reqs(&m).is_empty());
    let lead_saw = m.seen.lock().unwrap().iter().filter(|r| is_lead(r)).flat_map(results).collect::<Vec<_>>().join("\n");
    assert!(lead_saw.contains("Rampart"), "the lead is told why: {lead_saw}");

    // Rampart is its own agent, an orchestrator, on a cheap rung.
    let r = s.rampart.agent.expect("Rampart started");
    let a = &s.agents[r];
    assert_eq!(a.kind, AgentKind::Rampart);
    assert_ne!(r, 0);
    assert_eq!(a.decision.as_ref().map(|d| d.model.as_str()), Some("mock-mid"), "Rampart starts on a cheap rung");

    // It saw the approved spec and the baseline, and nothing of Builder's.
    let reqs = rampart_reqs(&m);
    assert!(!reqs.is_empty());
    let first = &users(&reqs[0])[0];
    let spec = std::fs::read_to_string(ws.join(planning::SPEC)).unwrap();
    assert!(first.contains(&spec), "{first}");
    assert!(first.contains("baseline"), "{first}");
    for builder in ["Write app.sh", "lead agent", LONG_TASK] {
        assert!(!reqs.iter().any(|r| r.to_string().contains(builder)), "Rampart saw Builder's `{builder}`");
    }
    // Its tools are reading its baseline checkout and submitting a suite.
    let mut tools = tool_names(&reqs[0]);
    tools.sort();
    assert_eq!(tools, ["list", "read", "submit_ramparts"]);

    // A test that passes on the baseline was sent back; the good suite waits
    // for a person's spot-check, with what was run.
    let rr = results(reqs.last().unwrap());
    assert!(rr[0].to_lowercase().contains("passes on the baseline") && rr[0].contains("tests/a2.sh"), "{}", rr[0]);
    assert!(s.rampart.seal.is_none());
    let ask = s.agents[0].log.iter().rev().find(|l| l.kind == LogKind::Assistant && l.text.contains("Rampart")).expect("the user is asked");
    assert!(ask.text.contains("tests/a1.sh") && ask.text.contains("sh tests/a2.sh") && ask.text.to_lowercase().contains("yes"), "{}", ask.text);
}

#[test]
fn after_a_yes_the_suite_is_sealed_and_builder_works_against_it() {
    let (m, h, ws) = to_suite("sealed", worker_writes(&["hello"], &[]), "");
    wait(&h, "the suite", |s| s.rampart.awaiting_yes && idle(s));
    h.send("yes");
    let s = wait(&h, "the ticket", |s| s.tickets.iter().any(|t| t.state == TicketState::Done) && idle(s));

    let seal = s.rampart.seal.clone().expect("sealed");
    assert!(seal.human_checked);
    assert_eq!(s.rampart.suites_checked, 1);
    assert_eq!(seal.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["tests/a1.sh", "tests/a2.sh"]);
    assert_eq!(seal.vetting.tests.iter().map(|t| t.exits.len()).collect::<Vec<_>>(), [3, 3]);
    let on_disk: rampart::Seal = serde_json::from_str(&std::fs::read_to_string(ws.join(rampart::SEAL)).unwrap()).unwrap();
    assert_eq!(on_disk, seal);
    // The sealed files are on the lead's branch, and so in every worker's.
    let run = ws.join(".backspace/worktrees/_run");
    assert!(rampart::verify(&run, &seal).is_empty(), "{:?}", rampart::verify(&run, &seal));

    // Builder saw the tests and nothing of how Rampart got them.
    let wr = worker_reqs(&m);
    assert!(!wr.is_empty());
    let rampart_said: Vec<String> = rampart_reqs(&m).iter().flat_map(results).collect();
    for r in &wr {
        let t = r.to_string();
        assert!(!t.contains("You are Rampart"), "Builder saw Rampart's brief");
        for said in &rampart_said {
            assert!(!t.contains(said.as_str()), "Builder saw what Rampart was told: {said}");
        }
    }
    // It passed them, three runs, and its change has a rampart score.
    let w = s.agents.iter().find(|a| a.kind == AgentKind::Worker).unwrap();
    let d = w.deliverable.clone().unwrap();
    let mu = d.mutation.expect("a mutation score");
    assert!(mu.total >= 1 && mu.score().is_some(), "{mu:?}");
    assert!(w.log.iter().any(|l| l.text.contains("sealed tests passed")), "{:?}", w.log);
}

#[test]
fn after_the_spot_checks_a_suite_is_sealed_without_asking_and_the_lead_is_told() {
    let (m, h, _ws) = to_suite("auto", worker_writes(&["hello"], &[]), "[rampart]\nspot_checks = 0\n");
    let s = wait(&h, "the ticket", |s| s.tickets.iter().any(|t| t.state == TicketState::Done) && idle(s));
    let seal = s.rampart.seal.clone().expect("sealed");
    assert!(!seal.human_checked);
    assert!(!s.agents[0].log.iter().any(|l| l.kind == LogKind::Assistant && l.text.contains("Rampart") && l.text.to_lowercase().contains("reply yes")));
    let lead_users: Vec<String> = m.seen.lock().unwrap().iter().filter(|r| is_lead(r)).flat_map(users).collect();
    assert!(lead_users.iter().any(|u| u.contains("sealed")), "{lead_users:?}");
}

#[test]
fn rampart_may_only_read_its_checkout_and_submit_in_code() {
    let m = mock(move |req| {
        if is_lead(req) {
            lead(req)
        } else if is_rampart(req) {
            match results(req).len() {
                0 => call("bash", json!({"command": "echo pwned > pwned.txt"})),
                1 => call("write", json!({"path": "pwned.txt", "content": "x"})),
                2 => call("create_tickets", json!({"tickets": []})),
                3 => call("read", json!({"path": "../../../worktrees/_run/README.md"})),
                _ => call("submit_ramparts", good_suite()),
            }
        } else {
            say("?")
        }
    });
    let (h, ws) = open("confined", m.port, "");
    h.send(LONG_TASK);
    wait(&h, "the baseline commands", |s| s.planning.awaiting_commands && idle(s));
    h.send("yes");
    wait(&h, "the spec", |s| s.planning.awaiting_yes && idle(s));
    h.send("yes");
    wait(&h, "the suite", |s| s.rampart.awaiting_yes && idle(s));
    let r = results(rampart_reqs(&m).last().unwrap());
    for (i, what) in ["bash", "write", "create_tickets"].iter().enumerate() {
        assert!(r[i].contains("not available"), "{what}: {}", r[i]);
    }
    assert!(r[3].to_lowercase().contains("outside") || r[3].to_lowercase().contains("escape"), "{}", r[3]);
    assert!(!ws.join(rampart::TREE).join("pwned.txt").exists());
    assert!(!ws.join("pwned.txt").exists());
}

#[test]
fn builder_cannot_change_a_sealed_test() {
    // The worker tries to rewrite a sealed test with its write tool, then
    // with bash, and submits.
    let worker = |req: &Value| -> Value {
        match results(req).len() {
            0 => call("write", json!({"path": "tests/a1.sh", "content": "exit 0\n"})),
            1 => call("bash", json!({"command": "echo 'exit 0' > tests/a1.sh && echo hello > out.txt"})),
            2 => call("write", json!({"path": "app.sh", "content": "echo bye\n"})),
            3 => call("submit_deliverable", record("done")),
            _ => say("I give up"),
        }
    };
    let (m, h, _ws) = to_suite("tamper", worker, "[rampart]\nspot_checks = 0\n");
    let s = wait(&h, "the worker to finish", |s| s.agents.iter().any(|a| a.kind == AgentKind::Worker && matches!(a.status, AgentStatus::Failed | AgentStatus::Approved)) && idle(s));
    let w = worker_reqs(&m).into_iter().max_by_key(|r| results(r).len()).unwrap();
    let r = results(&w);
    assert!(r[0].to_lowercase().contains("sealed") && r[0].contains("tests/a1.sh"), "the write is refused: {}", r[0]);
    assert!(r[3].to_lowercase().contains("sealed") && r[3].contains("tests/a1.sh") && r[3].to_lowercase().contains("changed"), "the delivery is refused: {}", r[3]);
    assert_ne!(s.tickets[0].state, TicketState::Done);
}

#[test]
fn n_sealed_failures_escalate_once_to_the_frontier_with_a_digest_then_a_human() {
    let (m, h, _ws) = to_suite("limit", worker_writes(&["apple", "pear", "plum"], &["kiwi"]), "[rampart]\nspot_checks = 0\n");
    let s = wait(&h, "the hand-off", |s| s.agents.iter().any(|a| a.kind == AgentKind::Worker && a.status == AgentStatus::Failed) && idle(s));
    let wr = worker_reqs(&m);
    let frontier: Vec<&Value> = wr.iter().filter(|r| r["model"] == "mock-frontier").collect();
    assert!(!frontier.is_empty(), "one frontier call was made");
    // Three failures on the cheap rung before it.
    let cheap_submits = wr.iter().filter(|r| r["model"] == "mock-mid" && last_call(r) == "write").count();
    assert_eq!(cheap_submits, 3, "three attempts on the cheap rung");
    // The frontier was sent the digest.
    let digest = users(frontier[0]).into_iter().find(|u| u.contains("ESCALATED")).expect("the digest");
    for want in ["app.sh prints hello", "got plum", "sh tests/a1.sh"] {
        assert!(digest.contains(want), "`{want}` not in {digest}");
    }
    // One frontier attempt, then the human: no frontier loop.
    let frontier_submits = frontier.iter().filter(|r| last_call(r) == "write").count();
    assert_eq!(frontier_submits, 1, "one frontier attempt");
    let w = s.agents.iter().find(|a| a.kind == AgentKind::Worker).unwrap();
    let l = &s.rampart.limits["greet"];
    assert_eq!(l.trigger, Some(Trigger::Failures(3)));
    assert_eq!(l.escalated_to.as_deref(), Some("mock-frontier"));
    assert!(l.handed_off && l.attempts.len() == 4, "{l:?}");
    let err = w.log.iter().rev().find(|e| e.kind == LogKind::Error).expect("handed off").text.clone();
    assert!(err.to_lowercase().contains("handed to you") && err.contains("got kiwi"), "{err}");
    assert_eq!(s.tickets[0].state, TicketState::Failed);
}

#[test]
fn the_same_failure_twice_escalates_at_once_and_the_frontier_can_finish() {
    let (m, h, _ws) = to_suite("stagnation", worker_writes(&["apple", "apple", "apple"], &["hello"]), "[rampart]\nspot_checks = 0\n");
    let s = wait(&h, "the ticket", |s| s.tickets.iter().any(|t| t.state == TicketState::Done) && idle(s));
    let l = &s.rampart.limits["greet"];
    assert_eq!(l.trigger, Some(Trigger::Stagnation));
    assert_eq!(l.attempts.len(), 2);
    assert!(!l.handed_off);
    assert!(worker_reqs(&m).iter().any(|r| r["model"] == "mock-frontier"));
}

#[test]
fn the_rampart_settings_have_their_defaults_in_the_shipped_config() {
    let cfg: backspace_core::config::Config = toml::from_str(backspace_core::config::DEFAULT_CONFIG).unwrap();
    assert_eq!(cfg.rampart, RampartConfig::default());
    assert!(backspace_core::config::DEFAULT_CONFIG.contains("[rampart]"));
}
