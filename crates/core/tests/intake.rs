//! M3 acceptance (PLAN §5.1, §6, §12): intake classifies a task before any
//! work, asks the user when unsure, and sends it down one of three paths.
//!
//! * research  -> a report on disk and no PR: no ticket, branch, worker or
//!                approval, and the planner cannot write, run commands or plan;
//! * one-shot  -> exactly one ticket from a validated lightweight spec, handed
//!                off as a file, which reaches a worker without a plan gate;
//! * long      -> unchanged: the full planner.
//!
//! The restrictions are asserted where code enforces them (the tool gate), not
//! where a prompt asks for them. Intake is off unless `[intake] enabled`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use backspace_core::config::Config;
use backspace_core::decision::{TaskType, Triage};
use backspace_core::intake::{self, OneShotSpec, Report};
use backspace_core::{AgentStatus, ApprovalKind, Harness, LogKind, ProjectState, TicketState};
use serde_json::{json, Value};

// ------------------------------------------------------------------ pure

#[test]
fn an_answer_is_a_label_a_number_or_nothing() {
    use TaskType::*;
    for (reply, want) in [
        ("research", Some(Research)),
        ("Research please", Some(Research)),
        ("it's a one-shot", Some(OneShot)),
        ("one shot", Some(OneShot)),
        ("ONE_SHOT", Some(OneShot)),
        ("long", Some(Long)),
        ("a long task", Some(Long)),
        ("1", Some(OneShot)),
        ("2)", Some(Long)),
        ("3.", Some(Research)),
        ("", None),
        ("hmm", None),
        ("belong", None),
        ("longitude research", Some(Research)),
        ("research or one-shot", None),
        ("4", None),
    ] {
        assert_eq!(intake::parse_answer(reply), want, "{reply:?}");
    }
}

#[test]
fn the_question_names_all_three_kinds_and_a_guess_when_there_is_one() {
    let q = intake::question(None);
    for word in ["one-shot", "long", "research"] {
        assert!(q.to_lowercase().contains(word), "{q}");
    }
    let q = intake::question(Some((TaskType::Research, 0.52)));
    assert!(q.contains("52%") && q.to_lowercase().contains("research"), "{q}");
}

#[test]
fn the_research_path_can_read_and_report_and_nothing_else() {
    for tool in ["read", "list", "submit_report"] {
        assert!(intake::allows(TaskType::Research, tool), "{tool}");
    }
    for tool in [
        "write", "edit", "bash", "create_tickets", "work_tickets", "revise_ticket", "file_ticket", "submit_deliverable",
        "scout", "produce", "skill", "colony_exec", "send_message", "show_options", "no_such_tool",
    ] {
        assert!(!intake::allows(TaskType::Research, tool), "{tool} must not be allowed on research");
    }
}

#[test]
fn the_one_shot_path_specifies_and_dispatches_but_never_builds_itself() {
    for tool in ["read", "list", "create_tickets", "work_tickets", "submit_deliverable"] {
        assert!(intake::allows(TaskType::OneShot, tool), "{tool}");
    }
    for tool in ["write", "edit", "bash", "revise_ticket", "submit_report", "scout", "no_such_tool"] {
        assert!(!intake::allows(TaskType::OneShot, tool), "{tool} must not be allowed on one-shot");
    }
}

#[test]
fn the_long_path_is_not_restricted() {
    for tool in ["write", "bash", "create_tickets", "work_tickets", "revise_ticket", "submit_deliverable", "scout"] {
        assert!(intake::allows(TaskType::Long, tool), "{tool}");
    }
}

fn ticket(extra: Value) -> Value {
    let mut t = json!({"key": "fix-readme", "title": "Fix the README typo", "what_to_build": "Correct `teh` to `the` in README.md",
        "acceptance": ["README.md no longer contains `teh`"], "check": "! grep -q teh README.md"});
    for (k, v) in extra.as_object().unwrap() {
        t[k] = v.clone();
    }
    t
}

#[test]
fn a_one_shot_spec_is_one_ticket_with_a_goal_and_an_acceptance_line() {
    let spec = OneShotSpec::from_tickets(&json!({"tickets": [ticket(json!({}))]})).unwrap();
    assert_eq!(spec.key, "fix-readme");
    assert_eq!(spec.goal, "Correct `teh` to `the` in README.md");
    assert_eq!(spec.acceptance, vec!["README.md no longer contains `teh`"]);
    assert_eq!(spec.check.as_deref(), Some("! grep -q teh README.md"));
    let md = spec.markdown();
    for part in ["Fix the README typo", "Correct `teh` to `the` in README.md", "- README.md no longer contains `teh`", "! grep -q teh README.md"] {
        assert!(md.contains(part), "{part} missing from:\n{md}");
    }
    // The check is optional on a one-shot.
    let no_check = ticket(json!({"check": null}));
    assert_eq!(OneShotSpec::from_tickets(&json!({"tickets": [no_check]})).unwrap().check, None);
}

#[test]
fn a_one_shot_spec_that_is_not_light_is_refused_with_the_reason() {
    let many = ticket(json!({"acceptance": ["a", "b", "c", "d", "e", "f"]}));
    for (what, input, why) in [
        ("no tickets", json!({"tickets": []}), "exactly one"),
        ("two tickets", json!({"tickets": [ticket(json!({})), ticket(json!({"key": "other"}))]}), "exactly one"),
        ("no acceptance", json!({"tickets": [ticket(json!({"acceptance": []}))]}), "acceptance"),
        ("blank acceptance", json!({"tickets": [ticket(json!({"acceptance": ["  "]}))]}), "acceptance"),
        ("too many acceptance lines", json!({"tickets": [many]}), "long"),
        ("a dependency", json!({"tickets": [ticket(json!({"blocked_by": ["x"]}))]}), "blocked_by"),
        ("a spec coverage claim", json!({"tickets": [ticket(json!({"covers": ["A1"]}))]}), "covers"),
        ("no key", json!({"tickets": [ticket(json!({"key": ""}))]}), "key"),
        ("tickets is not a list", json!({"tickets": "x"}), "tickets"),
    ] {
        let e = OneShotSpec::from_tickets(&input).expect_err(what).to_string();
        assert!(e.to_lowercase().contains(why), "{what}: {e}");
    }
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bs-intake-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn a_spec_is_handed_off_as_a_file() {
    let root = scratch("spec");
    let spec = OneShotSpec::from_tickets(&json!({"tickets": [ticket(json!({}))]})).unwrap();
    let rel = spec.write(&root).unwrap();
    assert_eq!(rel, ".backspace/handoff/fix-readme/spec.md");
    assert_eq!(std::fs::read_to_string(root.join(&rel)).unwrap(), spec.markdown());
}

#[test]
fn a_report_needs_a_title_and_a_body() {
    assert!(Report::from_json(&json!({"title": "", "body": "x"})).is_err());
    assert!(Report::from_json(&json!({"title": "T", "body": "  "})).is_err());
    assert!(Report::from_json(&json!({"title": "T"})).is_err());
    let r = Report::from_json(&json!({"title": "SQLite or Postgres", "body": "Findings."})).unwrap();
    assert_eq!((r.title.as_str(), r.body.as_str()), ("SQLite or Postgres", "Findings."));
}

#[test]
fn a_report_is_written_under_the_reports_folder_and_never_overwrites() {
    let root = scratch("report");
    let r = Report { title: "SQLite or Postgres".into(), body: "Findings.".into() };
    let a = r.write(&root).unwrap();
    let b = r.write(&root).unwrap();
    assert_ne!(a, b, "a second report with the same title must not replace the first");
    for rel in [&a, &b] {
        assert!(rel.starts_with(".backspace/reports/") && rel.ends_with(".md"), "{rel}");
        let text = std::fs::read_to_string(root.join(rel)).unwrap();
        assert!(text.starts_with("# SQLite or Postgres") && text.contains("Findings."), "{text}");
    }
    // A hostile title cannot walk out of the folder.
    let evil = Report { title: "../../etc/passwd: ünï \\ *".into(), body: "x".into() };
    let rel = evil.write(&root).unwrap();
    assert!(rel.starts_with(".backspace/reports/") && !rel.contains(".."), "{rel}");
    assert!(root.join(&rel).is_file());
    assert!(!root.join("..").join("..").join("etc").join("passwd.md").exists());
}

#[test]
fn intake_is_off_by_default_and_must_be_allowed_to_ask() {
    let base = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("default-config.toml")).unwrap();
    let cfg = Config::parse(&base).unwrap();
    assert!(!cfg.intake.enabled, "intake acts on an uncalibrated classifier, so it is opt-in");
    assert_eq!(cfg.intake.max_asks, 2);
    // A project's own file replaces the default wholesale, and without an
    // [intake] section it is off.
    let bare = config_text(1, "");
    assert!(!Config::parse(&bare).unwrap().intake.enabled);
    let ok = Config::parse(&config_text(1, "[intake]\nenabled = true\n")).unwrap();
    assert!(ok.intake.enabled && ok.intake.max_asks == 2);
    let zero = config_text(1, "[intake]\nenabled = true\nmax_asks = 0\n");
    let e = format!("{:#}", Config::parse(&zero).expect_err("max_asks = 0 would mean never asking"));
    assert!(e.contains("max_asks"), "{e}");
}

// --------------------------------------------------------------- harness

struct Mock {
    port: u16,
    /// Every chat request that reached the model (not the liveness probe).
    seen: Arc<Mutex<Vec<Value>>>,
    calls: Arc<AtomicUsize>,
}

fn mock(respond: impl Fn(&Value) -> Value + Send + Sync + 'static) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::new(AtomicUsize::new(0));
    let (s2, c2, respond) = (seen.clone(), calls.clone(), Arc::new(respond));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let (seen, calls, respond) = (s2.clone(), c2.clone(), respond.clone());
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
                    return; // the router's liveness probe
                }
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let req: Value = serde_json::from_slice(&body).unwrap();
                seen.lock().unwrap().push(req.clone());
                calls.fetch_add(1, Ordering::SeqCst);
                let out = respond(&req).to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).unwrap();
            });
        }
    });
    Mock { port, seen, calls }
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

fn first_user(req: &Value) -> &str {
    req["messages"][1]["content"].as_str().unwrap_or("")
}

/// The tool results so far, oldest first.
fn results(req: &Value) -> Vec<&str> {
    req["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "tool")
        .map(|m| m["content"].as_str().unwrap_or(""))
        .collect()
}

fn tool_names(req: &Value) -> Vec<String> {
    req["tools"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|t| t["function"]["name"].as_str().or(t["name"].as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn open(tag: &str, port: u16, intake: &str, seed: &[(&str, &str)]) -> (Harness, PathBuf) {
    let ws = std::env::temp_dir().join(format!("bs-intake-h-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    for (name, text) in seed {
        std::fs::write(ws.join(name), text).unwrap();
    }
    write_config(&ws, port, intake);
    (Harness::open(ws.clone()).unwrap(), ws)
}

fn write_config(ws: &Path, port: u16, intake: &str) {
    std::fs::write(ws.join("backspace.toml"), config_text(port, intake)).unwrap();
}

fn config_text(port: u16, intake: &str) -> String {
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
id = "mock-mid"
provider = "mock"
tier = 2
description = "mid"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000

[[models]]
id = "mock-front"
provider = "mock"
tier = 4
description = "frontier"
input_usd_per_mtok = 10.0
output_usd_per_mtok = 20.0
max_output_tokens = 1000

{intake}
"#
    )
}

const ON: &str = "[intake]\nenabled = true\n";

fn wait(h: &Harness, what: &str, done: impl Fn(&ProjectState) -> bool) -> ProjectState {
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        let s = h.snapshot();
        if done(&s) {
            return s;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:#?}",
            s.agents.iter().map(|a| (&a.key, a.status, &a.log)).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn asked(s: &ProjectState) -> usize {
    s.agents[0]
        .log
        .iter()
        .filter(|l| {
            let t = l.text.to_lowercase();
            l.kind == LogKind::Assistant && t.contains("one-shot") && t.contains("long") && t.contains("research")
        })
        .count()
}

fn branches(ws: &Path) -> String {
    let out = Command::new("git").args(["branch", "--list"]).current_dir(ws).output().unwrap();
    String::from_utf8(out.stdout).unwrap()
}

/// What the model says on each path. `trace` keeps the tool results the
/// planner got back, so a test can read what the code refused.
fn model(req: &Value) -> Value {
    let (sys, n) = (system(req), results(req).len());
    if sys.starts_with("You are the research agent") {
        let last = results(req).last().copied().unwrap_or("");
        return match n {
            0 => call("create_tickets", json!({"tickets": [ticket(json!({}))]})),
            1 => call("write", json!({"path": "evil.txt", "content": "x"})),
            2 => call("bash", json!({"command": "git push origin HEAD && touch pushed.txt"})),
            3 => call("list", json!({"path": "."})),
            4 => call("read", json!({"path": "notes.txt"})),
            5 => call("submit_report", json!({"title": "Notes review", "body": format!("The notes say: {last}")})),
            _ => say("The report is filed."),
        };
    }
    if sys.starts_with("You are the one-shot agent") {
        return match n {
            0 => call("create_tickets", json!({"tickets": [ticket(json!({})), ticket(json!({"key": "second"}))]})),
            1 => call("create_tickets", json!({"tickets": [ticket(json!({"acceptance": []}))]})),
            2 => call("write", json!({"path": "README.md", "content": "I did it myself"})),
            3 => call("create_tickets", json!({"tickets": [ticket(json!({}))]})),
            4 => call("work_tickets", json!({})),
            5 => call("submit_deliverable", json!({"summary": "typo fixed", "files": ["README.md"]})),
            _ => say("done"),
        };
    }
    if sys.starts_with("You are a Backspace ticket agent") {
        return match results(req).last() {
            None => call("write", json!({"path": "README.md", "content": "Say the word.\n"})),
            Some(t) if t.starts_with("wrote") => call("submit_deliverable", json!({"summary": "fixed the typo", "files": ["README.md"]})),
            Some(_) => say("worker done"),
        };
    }
    // The lead agent (the long path, or intake off).
    say("lead")
}

#[test]
fn with_intake_off_nothing_is_asked_and_the_model_gets_the_text() {
    let m = mock(model);
    let (h, _ws) = open("off", m.port, "", &[]);
    h.send("hmm");
    wait(&h, "the model to be called", |_| m.calls.load(Ordering::SeqCst) >= 1);
    let s = h.snapshot();
    assert_eq!(asked(&s), 0);
    assert_eq!(first_user(&m.seen.lock().unwrap()[0]), "hmm");
    assert_eq!(s.intake.pending, None);
}

#[test]
fn an_unsure_task_asks_the_user_and_calls_no_model() {
    let m = mock(model);
    let (h, _ws) = open("ask", m.port, ON, &[]);
    h.send("hmm");
    let s = wait(&h, "the question", |s| asked(s) == 1 && s.agents[0].status == AgentStatus::Idle);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(m.calls.load(Ordering::SeqCst), 0, "asking is code, not a model call");
    let p = s.intake.pending.expect("the question is waiting for an answer");
    assert_eq!((p.text.as_str(), p.asks), ("hmm", 1));
    assert_eq!(s.intake.path, None);
    assert_eq!(h.metrics().calls, 0);
}

#[test]
fn an_answer_starts_the_path_the_user_chose_on_the_original_task() {
    let m = mock(model);
    let (h, _ws) = open("answer", m.port, ON, &[("notes.txt", "alpha")]);
    h.send("hmm");
    wait(&h, "the question", |s| asked(s) == 1);
    h.send("research");
    wait(&h, "the model", |_| m.calls.load(Ordering::SeqCst) >= 1);
    let first = m.seen.lock().unwrap()[0].clone();
    assert!(system(&first).starts_with("You are the research agent"), "{}", system(&first));
    assert_eq!(first_user(&first), "hmm", "the model is given the task, not the answer");
    let s = h.snapshot();
    assert_eq!(s.intake.pending, None);
    assert_eq!(s.agents[0].decision.as_ref().unwrap().triage, Some(Triage::Confident { task: TaskType::Research, p: 1.0 }));
}

#[test]
fn two_unclear_answers_fall_to_the_long_path_and_keep_what_was_said() {
    let m = mock(model);
    let (h, _ws) = open("limit", m.port, ON, &[]);
    h.send("hmm");
    wait(&h, "the first question", |s| asked(s) == 1);
    h.send("dunno about it");
    wait(&h, "the second question", |s| asked(s) == 2);
    assert_eq!(m.calls.load(Ordering::SeqCst), 0);
    h.send("whatever happens");
    wait(&h, "the model", |_| m.calls.load(Ordering::SeqCst) >= 1);
    let s = h.snapshot();
    assert_eq!(asked(&s), 2, "never a third question");
    let first = m.seen.lock().unwrap()[0].clone();
    assert!(system(&first).starts_with("You are the lead agent"), "{}", system(&first));
    for said in ["hmm", "dunno about it", "whatever happens"] {
        assert!(first_user(&first).contains(said), "{said} was dropped: {}", first_user(&first));
    }
    assert!(s.agents[0].log.iter().any(|l| l.kind == LogKind::System && l.text.contains("long")), "the fall-back is said");
    assert_eq!(s.intake.path, Some(TaskType::Long));
}

#[test]
fn the_pending_question_survives_a_restart() {
    let m = mock(model);
    let (h, ws) = open("restart", m.port, ON, &[("notes.txt", "alpha")]);
    h.send("hmm");
    wait(&h, "the question", |s| asked(s) == 1);
    drop(h);
    let h = Harness::open(ws.clone()).unwrap();
    assert_eq!(h.snapshot().intake.pending.as_ref().map(|p| p.text.as_str()), Some("hmm"));
    h.send("research");
    wait(&h, "the model", |_| m.calls.load(Ordering::SeqCst) >= 1);
    assert!(system(&m.seen.lock().unwrap()[0]).starts_with("You are the research agent"));
}

#[test]
fn a_confident_long_task_keeps_the_full_planner() {
    let m = mock(model);
    let (h, _ws) = open("long", m.port, ON, &[]);
    h.send("build a dashboard system platform across several services");
    wait(&h, "the model", |_| m.calls.load(Ordering::SeqCst) >= 1);
    let s = h.snapshot();
    assert_eq!(asked(&s), 0, "a confident task is not asked about");
    let first = m.seen.lock().unwrap()[0].clone();
    assert!(system(&first).starts_with("You are the lead agent"), "{}", system(&first));
    let tools = tool_names(&first);
    for t in ["create_tickets", "work_tickets", "write", "bash", "submit_deliverable"] {
        assert!(tools.iter().any(|x| x == t), "{t} missing from {tools:?}");
    }
    assert_eq!(s.intake.path, Some(TaskType::Long));
}

#[test]
fn research_returns_a_report_and_no_pr() {
    let m = mock(model);
    let (h, ws) = open("research", m.port, ON, &[("notes.txt", "alpha")]);
    let before = branches(&ws);
    h.send("research the pros and cons of sqlite and write up a report, no code");
    let s = wait(&h, "the report", |s| s.agents[0].status == AgentStatus::Approved);

    // The report: a file under .backspace/reports, carrying what was read.
    let dir = ws.join(".backspace/reports");
    let files: Vec<_> = std::fs::read_dir(&dir).expect("a reports folder").map(|e| e.unwrap().path()).collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let text = std::fs::read_to_string(&files[0]).unwrap();
    assert!(text.starts_with("# Notes review") && text.contains("alpha"), "{text}");
    let d = s.agents[0].deliverable.as_ref().expect("the report is the deliverable");
    assert_eq!(d.files.len(), 1);
    assert!(d.files[0].starts_with(".backspace/reports/"), "{:?}", d.files);
    assert!(s.artifacts.iter().any(|a| a.kind == "document" && a.path.as_deref() == Some(d.files[0].as_str())), "the user sees the report as a document");

    // No PR: nothing was planned, branched, spawned, approved or merged.
    assert!(s.tickets.is_empty(), "{:?}", s.tickets);
    assert!(s.approvals.is_empty(), "{:?}", s.approvals);
    assert_eq!(s.agents.len(), 1, "no worker or scout was spawned");
    assert_eq!(branches(&ws), before, "no branch was created");
    assert!(!ws.join("evil.txt").exists() && !ws.join(".backspace/worktrees/_run/evil.txt").exists(), "write was refused");
    assert!(!ws.join("pushed.txt").exists() && !ws.join(".backspace/worktrees/_run/pushed.txt").exists(), "bash was refused");

    // What the code said to each attempt, and what the model was offered.
    let last = m.seen.lock().unwrap().iter().filter(|r| system(r).starts_with("You are the research agent")).last().unwrap().clone();
    let r = results(&last);
    for (i, tool) in ["create_tickets", "write", "bash"].iter().enumerate() {
        assert!(r[i].contains(tool) && r[i].contains("research"), "{tool}: {}", r[i]);
    }
    assert!(r[3].contains("notes.txt"), "list shows the workspace: {}", r[3]);
    let offered = tool_names(&last);
    for t in ["create_tickets", "work_tickets", "write", "edit", "bash", "submit_deliverable"] {
        assert!(!offered.iter().any(|x| x == t), "{t} was offered on research: {offered:?}");
    }
    for t in ["read", "list", "submit_report"] {
        assert!(offered.iter().any(|x| x == t), "{t} not offered: {offered:?}");
    }
    assert_eq!(s.intake.path, None, "a finished research task does not hold the next one");
}

#[test]
fn a_task_after_a_report_is_classified_afresh() {
    let m = mock(model);
    let (h, _ws) = open("fresh", m.port, ON, &[("notes.txt", "alpha")]);
    h.send("research the pros and cons of sqlite and write up a report, no code");
    wait(&h, "the report", |s| s.agents[0].status == AgentStatus::Approved);
    let calls = m.calls.load(Ordering::SeqCst);
    h.send("build a dashboard system platform across several services");
    wait(&h, "the next task's model call", |_| m.calls.load(Ordering::SeqCst) > calls);
    let seen = m.seen.lock().unwrap();
    let next = &seen[calls];
    assert!(system(next).starts_with("You are the lead agent"), "{}", system(next));
    assert_eq!(first_user(next), "build a dashboard system platform across several services");
}

#[test]
fn one_shot_reaches_a_worker_with_a_lightweight_spec_and_no_plan_gate() {
    let m = mock(model);
    let (h, ws) = open("oneshot", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send("fix a typo in the readme, quick small change");
    // The user approves the deliverable (merge stays a human decision); a plan
    // gate must never be asked for, because the lightweight spec replaces it.
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut answered = vec![];
    let s = loop {
        let s = h.snapshot();
        assert!(Instant::now() < deadline, "timed out: {:#?}", s.agents.iter().map(|a| (&a.key, a.status, &a.log)).collect::<Vec<_>>());
        if s.agents[0].status == AgentStatus::Approved {
            break s;
        }
        for a in s.pending_approvals() {
            assert_ne!(a.kind, ApprovalKind::Plan, "a one-shot has no plan gate");
            if !answered.contains(&a.id) {
                answered.push(a.id);
                h.approve(a.id);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    };

    // Exactly one ticket, from the validated spec, ready for the worker.
    assert_eq!(s.tickets.len(), 1, "{:?}", s.tickets);
    let t = &s.tickets[0];
    assert_eq!(t.key, "fix-readme");
    assert_eq!(t.state, TicketState::Done, "{:?}", t.state);
    assert_eq!(t.acceptance, vec!["README.md no longer contains `teh`"]);
    assert!(!s.approvals.iter().any(|a| a.kind == ApprovalKind::Plan));

    // The spec was handed off as a file.
    let spec = std::fs::read_to_string(ws.join(".backspace/handoff/fix-readme/spec.md")).unwrap();
    assert!(spec.contains("Correct `teh` to `the` in README.md") && spec.contains("README.md no longer contains `teh`"), "{spec}");

    // It reached execution: a worker got the spec as its brief, did the work
    // and it was merged. Main never wrote the file itself.
    let seen = m.seen.lock().unwrap();
    let worker = seen.iter().find(|r| system(r).starts_with("You are a Backspace ticket agent")).expect("a worker ran");
    assert!(first_user(worker).contains("Correct `teh` to `the` in README.md") && first_user(worker).contains("README.md no longer contains `teh`"), "{}", first_user(worker));
    assert_eq!(std::fs::read_to_string(ws.join(".backspace/worktrees/_run/README.md")).unwrap(), "Say the word.\n");

    // And the refusals along the way came from code, in the planner's results.
    let last = seen.iter().filter(|r| system(r).starts_with("You are the one-shot agent")).last().unwrap();
    let r = results(last);
    assert!(r[0].to_lowercase().contains("exactly one"), "{}", r[0]);
    assert!(r[1].to_lowercase().contains("acceptance"), "{}", r[1]);
    assert!(r[2].contains("write") && r[2].contains("one-shot"), "{}", r[2]);
    assert!(r[3].contains("fix-readme"), "{}", r[3]);
    let offered = tool_names(last);
    for t in ["write", "edit", "bash", "revise_ticket", "submit_report"] {
        assert!(!offered.iter().any(|x| x == t), "{t} was offered on one-shot: {offered:?}");
    }
    assert_eq!(s.intake.path, None);
}

#[test]
fn stopping_a_restricted_task_releases_its_path() {
    // A research agent that is slow to answer, so Stop lands mid-turn.
    let m = mock(|req| {
        if system(req).starts_with("You are the research agent") {
            std::thread::sleep(Duration::from_millis(1500));
            return say("still looking");
        }
        say("lead")
    });
    let (h, _ws) = open("stop", m.port, ON, &[]);
    h.send("research the pros and cons of sqlite and write up a report, no code");
    wait(&h, "the research turn", |s| s.intake.path == Some(TaskType::Research) && m.calls.load(Ordering::SeqCst) >= 1);
    h.stop(None);
    let s = wait(&h, "the stop", |s| s.intake.path.is_none() && s.agents[0].status == AgentStatus::Idle);
    assert_eq!(s.intake.spec_key, None);
    // The next message is a new task, classified afresh, not trapped on the research path.
    let calls = m.calls.load(Ordering::SeqCst);
    h.send("build a dashboard system platform across several services");
    wait(&h, "the next task's model call", |_| m.calls.load(Ordering::SeqCst) > calls);
    let seen = m.seen.lock().unwrap();
    assert!(system(&seen[calls]).starts_with("You are the lead agent"), "{}", system(&seen[calls]));
}
