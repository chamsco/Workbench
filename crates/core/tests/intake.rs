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
            5 => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "typo fixed", "files": ["README.md"]})),
            _ => say("done"),
        };
    }
    if sys.starts_with("You are a Backspace ticket agent") {
        return match results(req).last() {
            None => call("write", json!({"path": "README.md", "content": "Say the word.\n"})),
            Some(t) if t.starts_with("wrote") => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "fixed the typo", "files": ["README.md"]})),
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

#[test]
#[cfg(unix)]
fn research_cannot_read_through_a_symlink_that_leaves_the_project() {
    // The research path has `read` and `list` and nothing else, so the file
    // boundary is its boundary. A link inside the project to a secret outside
    // it must not reach the model, the report or the log.
    let outside = scratch("symlink-outside");
    std::fs::write(outside.join("secret.txt"), "TOP-SECRET-VALUE").unwrap();
    let m = mock(|req| {
        if !system(req).starts_with("You are the research agent") {
            return say("lead");
        }
        let last = results(req).last().copied().unwrap_or("");
        match results(req).len() {
            0 => call("read", json!({"path": "leak.txt"})),
            1 => call("list", json!({"path": "leakdir"})),
            2 => call("read", json!({"path": "leakdir/secret.txt"})),
            3 => call("submit_report", json!({"title": "Links", "body": format!("Last result: {last}")})),
            _ => say("filed"),
        }
    });
    let ws = std::env::temp_dir().join(format!("bs-intake-h-symlink-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::os::unix::fs::symlink(outside.join("secret.txt"), ws.join("leak.txt")).unwrap();
    std::os::unix::fs::symlink(&outside, ws.join("leakdir")).unwrap();
    write_config(&ws, m.port, ON);
    let h = Harness::open(ws.clone()).unwrap();
    h.send("research the pros and cons of sqlite and write up a report, no code");
    let s = wait(&h, "the report", |s| s.agents[0].status == AgentStatus::Approved);

    let seen = m.seen.lock().unwrap();
    assert!(!seen.iter().any(|r| r.to_string().contains("TOP-SECRET-VALUE")), "the secret reached the model");
    let last = seen.iter().filter(|r| system(r).starts_with("You are the research agent")).last().unwrap();
    for (i, r) in results(last).iter().take(3).enumerate() {
        assert!(r.to_lowercase().contains("symlink"), "attempt {i} was not refused for the symlink: {r}");
    }
    let d = s.agents[0].deliverable.as_ref().unwrap();
    let report = std::fs::read_to_string(ws.join(&d.files[0])).unwrap();
    assert!(!report.contains("TOP-SECRET-VALUE"), "{report}");
    let log: String = s.agents[0].log.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
    assert!(!log.contains("TOP-SECRET-VALUE"));
}

#[test]
fn a_one_shot_files_one_ticket_and_a_second_is_refused_while_the_first_still_runs() {
    // The same task a coding-CLI planner now refuses, on the native/API route.
    let m = mock(|req| {
        let (sys, n) = (system(req), results(req).len());
        if sys.starts_with("You are the one-shot agent") {
            return match n {
                0 => call("create_tickets", json!({"tickets": [ticket(json!({}))]})),
                1 => call("create_tickets", json!({"tickets": [ticket(json!({"key": "second"}))]})),
                2 => call("work_tickets", json!({})),
                3 => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "typo fixed", "files": ["README.md"]})),
                _ => say("done"),
            };
        }
        if sys.starts_with("You are a Backspace ticket agent") {
            return match results(req).last() {
                None => call("write", json!({"path": "README.md", "content": "Say the word.\n"})),
                Some(t) if t.starts_with("wrote") => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "fixed the typo", "files": ["README.md"]})),
                Some(_) => say("worker done"),
            };
        }
        say("lead")
    });
    let (h, ws) = open("second", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send("fix a typo in the readme, quick small change");
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
    assert_eq!(s.tickets.len(), 1, "{:?}", s.tickets);
    assert_eq!(s.tickets[0].state, TicketState::Done);
    let seen = m.seen.lock().unwrap();
    let last = seen.iter().filter(|r| system(r).starts_with("You are the one-shot agent")).last().unwrap();
    assert!(results(last)[1].contains("already has its ticket"), "{}", results(last)[1]);
    assert_eq!(std::fs::read_to_string(ws.join(".backspace/worktrees/_run/README.md")).unwrap(), "Say the word.\n");
    assert_eq!((s.intake.path, s.intake.spec_key.clone()), (None, None), "released once delivered");
}

// ------------------------------------------- the question names what can run
//
// With a coding-CLI planner only the long path runs (research and one-shot are
// refused before the CLI starts), so the question must not offer the others as
// if they were open. The refusal on an answer stays as defence in depth.

#[test]
fn the_question_offers_only_the_paths_that_can_run() {
    use intake::Paths;
    // Everything available: the question it always was.
    for best in [None, Some((TaskType::Research, 0.52))] {
        assert_eq!(intake::question_in(best, Paths::ALL), intake::question(best));
    }
    // A coding-CLI planner: long is the one path offered; the others are named as unavailable, with why.
    let q = intake::question_in(Some((TaskType::OneShot, 0.55)), Paths::LONG_ONLY);
    let lower = q.to_lowercase();
    assert!(q.contains("55%") && lower.contains("one-shot"), "the guess is still said: {q}");
    for needed in ["not available", "coding cli", "m6", "api model"] {
        assert!(lower.contains(needed), "`{needed}` missing from:\n{q}");
    }
    assert!(q.lines().any(|l| l.trim_start().starts_with("- long")), "long is offered as the choice:\n{q}");
    assert!(
        !q.lines().any(|l| l.trim_start().starts_with(|c: char| c.is_ascii_digit())),
        "no numbered menu of kinds that cannot run:\n{q}"
    );
    for gone in ["a small, contained change", "a written report only"] {
        assert!(!q.contains(gone), "`{gone}` is described as if it were open:\n{q}");
    }
}

#[test]
fn intake_asks_and_asks_again_with_the_paths_that_can_run() {
    use intake::{Next, Paths};
    let unsure = || Ok(Triage::Ask { best: TaskType::Research, p: 0.5 });
    let Next::Ask(pending, q) = intake::first_in("hmm", unsure(), Paths::LONG_ONLY) else { panic!("an unsure task is asked about") };
    assert_eq!(q, intake::question_in(Some((TaskType::Research, 0.5)), Paths::LONG_ONLY));
    assert!(q.to_lowercase().contains("not available") && !q.contains("3. research"), "{q}");
    // A classifier that cannot decide is asked about the same way.
    let Next::Ask(_, q) = intake::first_in("hmm", Err(anyhow::anyhow!("boom")), Paths::LONG_ONLY) else { panic!() };
    assert_eq!(q, intake::question_in(None, Paths::LONG_ONLY));
    // An unclear reply gets the same options again, and the task text is kept.
    let Next::Ask(again, q) = intake::answer_in(&pending, "dunno", 2, Paths::LONG_ONLY) else { panic!() };
    assert_eq!(q, format!("I didn't get a clear answer.\n\n{}", intake::question_in(None, Paths::LONG_ONLY)));
    assert!(q.to_lowercase().contains("not available") && !q.contains("1. one-shot"), "{q}");
    assert!(again.text.contains("hmm") && again.text.contains("dunno") && again.asks == 2);
    // "long" goes.
    match intake::answer_in(&pending, "long", 2, Paths::LONG_ONLY) {
        Next::Go { path: TaskType::Long, text, .. } => assert_eq!(text, "hmm"),
        other => panic!("{other:?}"),
    }
    // Everything-available is what first/answer/question always did.
    assert_eq!(intake::first("hmm", unsure()), intake::first_in("hmm", unsure(), Paths::ALL));
    assert_eq!(intake::answer(&pending, "dunno", 2), intake::answer_in(&pending, "dunno", 2, Paths::ALL));
}

#[test]
fn with_an_api_planner_the_question_still_offers_all_three_paths() {
    let m = mock(model);
    let (h, _ws) = open("api-question", m.port, ON, &[]);
    h.send("hmm");
    let s = wait(&h, "the question", |s| asked(s) == 1 && s.agents[0].status == AgentStatus::Idle);
    let q = s.agents[0].log.iter().rev().find(|l| l.kind == LogKind::Assistant).unwrap().text.clone();
    for (n, kind) in [("1.", "one-shot"), ("2.", "long"), ("3.", "research")] {
        assert!(q.lines().any(|l| l.trim_start().starts_with(n) && l.contains(kind)), "{n} {kind} missing from:\n{q}");
    }
    assert!(!q.to_lowercase().contains("not available"), "{q}");
}

// -------------------------------------- the API one-shot's failure lifecycle
//
// A one-shot ends when it is delivered or when the user presses Stop. A worker
// that fails, or is stopped, does not end it: the one ticket is still the
// task's, and can be dispatched again (by the same agent in the same turn, or
// on the user's say-so next turn). Stop must release it even though the main
// agent is idle by then, and a ticket that no longer exists, or was dropped
// (wontfix), must not keep holding the task's one ticket slot.

const TYPO: &str = "fix a typo in the readme, quick small change";

/// What each worker the mock starts does, in the order they start; any later
/// worker delivers.
#[derive(Clone, Copy)]
enum W {
    Deliver,
    GiveUp,
    Hang,
}

/// `lead` plays the one-shot agent. Returns the mock and how many workers have started.
fn lifecycle_mock(workers: Vec<W>, lead: impl Fn(&Value) -> Value + Send + Sync + 'static) -> (Mock, Arc<AtomicUsize>) {
    let started = Arc::new(AtomicUsize::new(0));
    let s2 = started.clone();
    let m = mock(move |req| {
        let sys = system(req);
        if sys.starts_with("You are the one-shot agent") {
            return lead(req);
        }
        if sys.starts_with("You are a Backspace ticket agent") {
            // A worker's first request has nothing from the model in it yet.
            if req["messages"].as_array().is_some_and(|m| m.iter().all(|m| m["role"] != "assistant")) {
                s2.fetch_add(1, Ordering::SeqCst);
            }
            let attempt = s2.load(Ordering::SeqCst).max(1);
            return match workers.get(attempt - 1).copied().unwrap_or(W::Deliver) {
                W::GiveUp => say("I can't do this."),
                W::Hang => {
                    std::thread::sleep(Duration::from_secs(20));
                    say("too late")
                }
                W::Deliver => match results(req).last() {
                    None => call("write", json!({"path": "README.md", "content": "Say the word.\n"})),
                    Some(t) if t.contains("REJECTED") => call("write", json!({"path": "README.md", "content": "Say the word, please.\n"})),
                    Some(t) if t.starts_with("wrote") => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "fixed the typo", "files": ["README.md"]})),
                    Some(_) => say("worker done"),
                },
            };
        }
        say("lead")
    });
    (m, started)
}

fn last_role(req: &Value) -> &str {
    req["messages"].as_array().and_then(|m| m.last()).and_then(|m| m["role"].as_str()).unwrap_or("")
}

/// Poll until `done`, answering each deliverable that waits: approve it, or
/// reject the first with feedback. A plan is never asked for on a one-shot.
fn approving(h: &Harness, what: &str, reject_first: bool, done: impl Fn(&ProjectState) -> bool) -> ProjectState {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut answered: Vec<usize> = vec![];
    loop {
        let s = h.snapshot();
        if done(&s) {
            return s;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:#?} {:?}",
            s.agents.iter().map(|a| (&a.key, a.status, &a.log)).collect::<Vec<_>>(),
            s.tickets.iter().map(|t| (&t.key, t.state)).collect::<Vec<_>>()
        );
        for a in s.pending_approvals() {
            assert_ne!(a.kind, ApprovalKind::Plan, "a one-shot has no plan gate");
            if !answered.contains(&a.id) {
                if reject_first && answered.is_empty() {
                    h.reject(a.id, "write it as: Say the word, please.");
                } else {
                    h.approve(a.id);
                }
                answered.push(a.id);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn workers(s: &ProjectState) -> Vec<&backspace_core::AgentRecord> {
    s.agents.iter().filter(|a| a.kind == backspace_core::AgentKind::Worker).collect()
}

fn lead_results(m: &Mock) -> Vec<String> {
    let seen = m.seen.lock().unwrap();
    let last = seen.iter().filter(|r| system(r).starts_with("You are the one-shot agent")).last().expect("the one-shot agent ran");
    results(last).iter().map(|s| s.to_string()).collect()
}

#[test]
fn a_failed_one_shot_is_retried_in_the_same_turn_on_its_one_ticket() {
    let (m, started) = lifecycle_mock(vec![W::GiveUp], |req| match results(req).len() {
        0 => call("create_tickets", json!({"tickets": [ticket(json!({}))]})),
        1 | 2 => call("work_tickets", json!({})),
        3 => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "typo fixed", "files": ["README.md"]})),
        _ => say("done"),
    });
    let (h, ws) = open("life-retry", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send(TYPO);
    let s = approving(&h, "the delivery", false, |s| s.agents[0].status == AgentStatus::Approved);
    assert_eq!(started.load(Ordering::SeqCst), 2, "the failed worker, then its retry: {:#?}", lead_results(&m));
    assert_eq!(s.tickets.len(), 1, "still one ticket: {:?}", s.tickets);
    assert_eq!(s.tickets[0].state, TicketState::Done);
    let w = workers(&s);
    assert_eq!(w.len(), 2);
    assert_eq!((w[0].status, w[1].status), (AgentStatus::Failed, AgentStatus::Approved));
    let r = lead_results(&m);
    assert!(r[1].contains("FAILED"), "the agent was told the worker failed: {}", r[1]);
    assert_eq!(std::fs::read_to_string(ws.join(".backspace/worktrees/_run/README.md")).unwrap(), "Say the word.\n");
    assert_eq!((s.intake.path, s.intake.spec_key.clone()), (None, None), "released once delivered");
}

/// The first turn of a one-shot whose worker fails and whose agent just says so.
fn failing_lead(req: &Value) -> Value {
    match (results(req).len(), last_role(req)) {
        (0, _) => call("create_tickets", json!({"tickets": [ticket(json!({}))]})),
        (1, _) => call("work_tickets", json!({})),
        (2, "tool") => say("The worker failed. Say retry to try again, or press Stop."),
        // The user's next message: first try a second ticket, then the one it has.
        (2, _) => call("create_tickets", json!({"tickets": [ticket(json!({"key": "second"}))]})),
        // Naming the key works too (a failed ticket is dispatched by name).
        (3, _) => call("work_tickets", json!({"keys": ["fix-readme"]})),
        (4, _) => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "typo fixed", "files": ["README.md"]})),
        _ => say("done"),
    }
}

fn failed_and_idle(s: &ProjectState) -> bool {
    s.tickets.first().is_some_and(|t| t.state == TicketState::Failed) && s.agents[0].status == AgentStatus::Idle
}

#[test]
fn a_failed_one_shot_stays_open_for_a_retry_and_keeps_to_its_one_ticket() {
    let (m, started) = lifecycle_mock(vec![W::GiveUp], failing_lead);
    let (h, _ws) = open("life-open", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send(TYPO);
    let s = wait(&h, "the failure", failed_and_idle);
    assert_eq!(s.intake.path, Some(TaskType::OneShot), "a failure does not end the task");
    assert_eq!(s.intake.spec_key.as_deref(), Some("fix-readme"));
    assert_eq!(asked(&s), 0);

    // "retry" goes to the same agent (it is not a new task to classify or ask about).
    h.send("retry");
    let s = approving(&h, "the retry's delivery", false, |s| s.agents[0].status == AgentStatus::Approved);
    assert_eq!(started.load(Ordering::SeqCst), 2);
    assert_eq!(s.tickets.len(), 1, "a second spec was not accepted: {:?}", s.tickets);
    assert_eq!(s.tickets[0].state, TicketState::Done);
    let r = lead_results(&m);
    assert!(r[2].contains("already has its ticket"), "{}", r[2]);
    assert_eq!(asked(&s), 0, "the retry was not asked about");
    assert_eq!((s.intake.path, s.intake.spec_key.clone()), (None, None));
}

#[test]
fn stop_releases_a_failed_one_shot_even_though_the_main_agent_is_idle() {
    let (m, _started) = lifecycle_mock(vec![W::GiveUp], failing_lead);
    let (h, ws) = open("life-stop", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send(TYPO);
    wait(&h, "the failure", failed_and_idle);
    h.stop(None);
    let s = wait(&h, "the stop to release the task", |s| s.intake.path.is_none());
    assert_eq!((s.intake.spec_key.clone(), s.agents[0].status), (None, AgentStatus::Idle));
    assert!(s.agents[0].log.iter().any(|l| l.text.contains("stopped by you")), "{:?}", s.agents[0].log);
    assert!(!ws.join(".backspace/main.json").exists(), "its saved conversation went with it");

    // The next message is a new task: classified afresh and given to the right agent.
    let calls = m.calls.load(Ordering::SeqCst);
    h.send("build a dashboard system platform across several services");
    wait(&h, "the next task's model call", |_| m.calls.load(Ordering::SeqCst) > calls);
    let seen = m.seen.lock().unwrap();
    assert!(system(&seen[calls]).starts_with("You are the lead agent"), "{}", system(&seen[calls]));
    assert!(tool_names(&seen[calls]).iter().any(|t| t == "revise_ticket"), "the full planner's tools, not the one-shot's: {:?}", tool_names(&seen[calls]));
    assert_eq!(first_user(&seen[calls]), "build a dashboard system platform across several services");
}

#[test]
fn a_one_shot_worker_the_user_stops_fails_its_ticket_and_can_be_dispatched_again() {
    let (m, started) = lifecycle_mock(vec![W::Hang], |req| match results(req).len() {
        0 => call("create_tickets", json!({"tickets": [ticket(json!({}))]})),
        1 | 2 => call("work_tickets", json!({})),
        3 => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "typo fixed", "files": ["README.md"]})),
        _ => say("done"),
    });
    let (h, _ws) = open("life-drop", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send(TYPO);
    let s = wait(&h, "the first worker to be at work", |s| started.load(Ordering::SeqCst) == 1 && workers(s).len() == 1);
    h.stop(Some(workers(&s)[0].id));
    let s = approving(&h, "the retry's delivery", false, |s| s.agents[0].status == AgentStatus::Approved);
    let w = workers(&s);
    assert_eq!((w.len(), w[0].status, w[1].status), (2, AgentStatus::Failed, AgentStatus::Approved));
    assert_eq!(s.tickets.len(), 1);
    assert_eq!(s.tickets[0].state, TicketState::Done);
    assert!(s.tickets[0].notes.iter().any(|n| n.contains("stopped by the user")), "{:?}", s.tickets[0].notes);
    assert!(lead_results(&m)[1].contains("stopped by the user"), "{}", lead_results(&m)[1]);
}

#[test]
fn a_rejected_one_shot_deliverable_goes_back_to_its_worker_and_the_path_ends_with_the_delivery() {
    let (m, started) = lifecycle_mock(vec![], |req| match results(req).len() {
        0 => call("create_tickets", json!({"tickets": [ticket(json!({}))]})),
        1 => call("work_tickets", json!({})),
        2 => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "typo fixed", "files": ["README.md"]})),
        _ => say("done"),
    });
    let (h, ws) = open("life-reject", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send(TYPO);
    let s = approving(&h, "the delivery", true, |s| s.agents[0].status == AgentStatus::Approved);
    assert_eq!(started.load(Ordering::SeqCst), 1, "the same worker took the feedback; no second worker");
    assert_eq!(s.tickets.len(), 1);
    assert_eq!(s.tickets[0].state, TicketState::Done);
    assert!(s.tickets[0].notes.iter().any(|n| n.contains("rejected by the user")), "{:?}", s.tickets[0].notes);
    assert!(s.approvals.iter().any(|a| matches!(a.state, backspace_core::ApprovalState::Rejected { .. })));
    assert_eq!(std::fs::read_to_string(ws.join(".backspace/worktrees/_run/README.md")).unwrap(), "Say the word, please.\n");
    assert_eq!((s.intake.path, s.intake.spec_key.clone()), (None, None));
}

/// A task whose one ticket was dropped (no longer there, or wontfix) before the
/// session was reopened: the slot is free, so the agent can file its spec.
fn dropped_ticket_task(tag: &str, drop_as: &str) {
    let (m, _started) = lifecycle_mock(vec![], |req| match results(req).len() {
        0 => call("create_tickets", json!({"tickets": [ticket(json!({}))]})),
        1 => call("work_tickets", json!({})),
        2 => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "typo fixed", "files": ["README.md"]})),
        _ => say("done"),
    });
    let (h, ws) = open(tag, m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send("hmm");
    wait(&h, "the question", |s| asked(s) == 1 && s.agents[0].status == AgentStatus::Idle);
    drop(h);

    let file = ws.join(".backspace/state.json");
    let mut state: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    state["intake"] = json!({"path": "one_shot", "pending": null, "spec_key": "dropped-one"});
    state["tickets"] = match drop_as {
        "removed" => json!([]),
        _ => {
            let old = backspace_core::Ticket::from_json(&ticket(json!({"key": "dropped-one"})), 1, 0, TicketState::Wontfix).unwrap();
            json!([old])
        }
    };
    std::fs::write(&file, state.to_string()).unwrap();

    let h = Harness::open(ws.clone()).unwrap();
    assert_eq!(h.snapshot().intake.spec_key.as_deref(), Some("dropped-one"), "the saved task was restored");
    h.send("do it");
    let s = approving(&h, "the delivery", false, |s| s.agents[0].status == AgentStatus::Approved);
    let r = lead_results(&m);
    assert!(!r[0].contains("already has its ticket"), "a dropped ticket must not hold the slot: {}", r[0]);
    let fresh = s.tickets.iter().find(|t| t.key == "fix-readme").unwrap_or_else(|| panic!("the new spec was not filed: {:?}", s.tickets));
    assert_eq!(fresh.state, TicketState::Done);
    assert_eq!(s.tickets.iter().filter(|t| t.key == "dropped-one").count(), usize::from(drop_as != "removed"));
    assert_eq!((s.intake.path, s.intake.spec_key.clone()), (None, None));
}

#[test]
fn a_one_shot_whose_ticket_is_gone_can_file_its_spec_again() {
    dropped_ticket_task("life-gone", "removed");
}

#[test]
fn a_one_shot_whose_ticket_was_dropped_as_wontfix_can_file_its_spec_again() {
    dropped_ticket_task("life-wontfix", "wontfix");
}

// --------------------------------------------- the start_task authorization
//
// `start_task` (the shell's "start another task", and POST /v1/task) is a
// person's request. What it authorizes: a ticket of its own, built in parallel
// with no plan approval and no intake question, because they asked for it. What
// it does not: merging without their review, anything without the API token,
// or a way round the research and one-shot paths while one is open (research
// makes no tickets or branches; a one-shot has exactly one ticket, its own).
// No agent can call it.

const SIDE_TASK: &str = "add a short CONTRIBUTING.md that says to run the tests first";

fn side_task_mock() -> Mock {
    mock(|req| {
        let sys = system(req);
        if sys.starts_with("You are the research agent") {
            return say("still looking"); // reply, so the research task stays open
        }
        if sys.starts_with("You are the one-shot agent") {
            let users = req["messages"].as_array().unwrap().iter().filter(|m| m["role"] == "user").count();
            return match (last_role(req), results(req).len()) {
                ("user", _) if users == 1 => say("What should the spec say?"),
                ("user", _) => call("create_tickets", json!({"tickets": [ticket(json!({}))]})),
                (_, 1) => call("work_tickets", json!({})),
                (_, 2) => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "typo fixed", "files": ["README.md"]})),
                _ => say("done"),
            };
        }
        if sys.starts_with("You are a Backspace ticket agent") {
            return match results(req).last() {
                None => call("write", json!({"path": "README.md", "content": "Say the word.\n"})),
                Some(t) if t.starts_with("wrote") => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "did the side task", "files": ["README.md"]})),
                Some(_) => say("worker done"),
            };
        }
        say("lead")
    })
}

fn nothing_was_started(s: &ProjectState, ws: &Path) {
    assert!(s.tickets.is_empty() && s.approvals.is_empty() && s.agents.len() == 1, "{:?}", (&s.tickets, &s.approvals, s.agents.len()));
    assert!(!ws.join(".backspace/tickets").exists() || std::fs::read_dir(ws.join(".backspace/tickets")).unwrap().next().is_none(), "no ticket file");
    assert!(!ws.join(".backspace/handoff").exists(), "no spec");
}

fn http_post(port: u16, path: &str, auth: Option<&str>, body: &str) -> (u16, String) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut s = loop {
        match std::net::TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => break s,
            Err(e) => {
                assert!(Instant::now() < deadline, "no server: {e}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };
    let auth = auth.map(|a| format!("Authorization: {a}\r\n")).unwrap_or_default();
    write!(s, "POST {path} HTTP/1.1\r\nHost: x\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let code = out.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    (code, out.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
fn the_task_endpoint_wants_the_token_and_starts_nothing_without_it() {
    let m = side_task_mock();
    let (h, ws) = open("st-token", m.port, ON, &[("README.md", "Say teh word.\n")]);
    let port = free_port();
    let _serving = h.serve(&format!("127.0.0.1:{port}"), "s3cret").unwrap();
    let body = json!({"text": SIDE_TASK}).to_string();
    for (what, auth) in [
        ("no header", None),
        ("a wrong token", Some("Bearer wrong")),
        ("the right token with no scheme", Some("s3cret")),
        ("another scheme", Some("Basic czNjcmV0")),
        ("an empty token", Some("Bearer ")),
    ] {
        let (code, text) = http_post(port, "/v1/task", auth, &body);
        assert_eq!(code, 401, "{what}: {text}");
    }
    std::thread::sleep(Duration::from_millis(300));
    nothing_was_started(&h.snapshot(), &ws);
    assert_eq!(m.calls.load(Ordering::SeqCst), 0, "no model was called for an unauthorized request");

    let (code, text) = http_post(port, "/v1/task", Some("Bearer s3cret"), &body);
    assert_eq!(code, 200, "{text}");
    let key = serde_json::from_str::<Value>(&text).unwrap()["key"].as_str().unwrap().to_string();
    let s = wait(&h, "the ticket", |s| s.tickets.iter().any(|t| t.key == key));
    assert_eq!(s.tickets.len(), 1);
}

#[test]
fn a_started_task_skips_the_plan_gate_but_its_delivery_still_waits_for_the_user() {
    let m = side_task_mock();
    let (h, ws) = open("st-review", m.port, ON, &[("README.md", "Say teh word.\n")]);
    let key = h.start_task(SIDE_TASK).unwrap();
    let s = wait(&h, "the delivery to wait for review", |s| s.pending_approvals().any(|a| a.kind == ApprovalKind::Deliverable));
    assert!(!s.approvals.iter().any(|a| a.kind == ApprovalKind::Plan), "the person asked for it: no plan approval");
    let t = s.tickets.iter().find(|t| t.key == key).unwrap();
    assert_eq!(t.state, TicketState::InReview, "{:?}", t.state);
    // Nothing is merged until they say so.
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(std::fs::read_to_string(ws.join(".backspace/worktrees/_run/README.md")).unwrap(), "Say teh word.\n", "merged without approval");
    let id = s.pending_approvals().next().unwrap().id;
    h.approve(id);
    wait(&h, "the merge", |s| s.tickets.iter().any(|t| t.key == key && t.state == TicketState::Done));
    assert_eq!(std::fs::read_to_string(ws.join(".backspace/worktrees/_run/README.md")).unwrap(), "Say the word.\n");
}

#[test]
fn a_started_task_is_not_a_task_for_the_planner_or_the_question() {
    // A question is waiting on the user. Starting a side task neither answers it,
    // classifies the side task, nor asks about it.
    let m = side_task_mock();
    let (h, _ws) = open("st-intake", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send("hmm");
    let before = wait(&h, "the question", |s| asked(s) == 1 && s.agents[0].status == AgentStatus::Idle);
    let key = h.start_task("hmm").unwrap(); // text the classifier would ask about
    let s = wait(&h, "the worker", |s| s.tickets.iter().any(|t| t.key == key) && s.agents.len() > 1);
    assert_eq!(asked(&s), 1, "the side task was not asked about");
    assert_eq!(s.intake.pending, before.intake.pending, "the waiting question is untouched");
    assert_eq!(s.intake.path, None);
    let seen = m.seen.lock().unwrap();
    assert!(
        seen.iter().all(|r| system(r).starts_with("You are a Backspace ticket agent")),
        "only the worker was called: {:?}",
        seen.iter().map(|r| system(r).chars().take(40).collect::<String>()).collect::<Vec<_>>()
    );
}

#[test]
fn nothing_can_be_started_from_nothing() {
    let m = side_task_mock();
    let (h, ws) = open("st-empty", m.port, ON, &[]);
    for text in ["", "   ", "\n\t "] {
        let e = h.start_task(text).expect_err("an empty task").to_string();
        assert!(e.contains("say what the task is"), "{e}");
    }
    nothing_was_started(&h.snapshot(), &ws);
}

#[test]
fn a_task_cannot_be_started_while_a_research_task_is_open() {
    let m = side_task_mock();
    let (h, ws) = open("st-research", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send("research the pros and cons of sqlite and write up a report, no code");
    wait(&h, "the research task", |s| s.intake.path == Some(TaskType::Research) && s.agents[0].status == AgentStatus::Idle);
    let e = h.start_task(SIDE_TASK).expect_err("research makes no tickets or branches").to_string().to_lowercase();
    for needed in ["research task is open", "stop"] {
        assert!(e.contains(needed), "`{needed}` missing from the refusal: {e}");
    }
    assert!(!e.contains("create_tickets"), "the refusal speaks to the person, not to a model: {e}");
    std::thread::sleep(Duration::from_millis(300));
    let s = h.snapshot();
    nothing_was_started(&s, &ws);
    assert_eq!(s.intake.path, Some(TaskType::Research), "the research task is undisturbed");
    // Over the API it is a refusal too, and the same one.
    let port = free_port();
    let _serving = h.serve(&format!("127.0.0.1:{port}"), "s3cret").unwrap();
    let (code, text) = http_post(port, "/v1/task", Some("Bearer s3cret"), &json!({"text": SIDE_TASK}).to_string());
    assert_eq!(code, 400, "{text}");
    assert!(text.to_lowercase().contains("research task is open"), "{text}");
    // Ending the research task (Stop) opens the way again.
    h.stop(None);
    wait(&h, "the stop", |s| s.intake.path.is_none());
    h.start_task(SIDE_TASK).expect("with no restricted task open");
}

#[test]
fn a_task_cannot_be_started_while_a_one_shot_is_open_and_does_not_take_its_ticket() {
    let m = side_task_mock();
    let (h, _ws) = open("st-oneshot", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send(TYPO);
    wait(&h, "the one-shot's question", |s| s.intake.path == Some(TaskType::OneShot) && s.agents[0].status == AgentStatus::Idle);
    let e = h.start_task(SIDE_TASK).expect_err("a one-shot has exactly one ticket, its own").to_string().to_lowercase();
    for needed in ["one-shot task is open", "stop"] {
        assert!(e.contains(needed), "`{needed}` missing from the refusal: {e}");
    }
    std::thread::sleep(Duration::from_millis(300));
    let s = h.snapshot();
    assert!(s.tickets.is_empty() && s.agents.len() == 1, "{:?}", s.tickets);
    assert_eq!(s.intake.spec_key, None, "the one-shot's ticket slot was not used up");
    // The one-shot can still file and run its own ticket.
    h.send("the spec: fix the typo in the readme");
    let s = approving(&h, "its delivery", false, |s| s.agents[0].status == AgentStatus::Approved);
    assert_eq!(s.tickets.len(), 1);
    assert_eq!((s.tickets[0].key.as_str(), s.tickets[0].state), ("fix-readme", TicketState::Done));
}

#[test]
fn a_long_task_does_not_stop_a_side_task_from_starting() {
    let m = side_task_mock();
    let (h, _ws) = open("st-long", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send("build a dashboard system platform across several services");
    wait(&h, "the long path", |s| s.intake.path == Some(TaskType::Long) && s.agents[0].status == AgentStatus::Idle);
    let key = h.start_task(SIDE_TASK).expect("only the research and one-shot paths hold a task back");
    wait(&h, "the side task's ticket", |s| s.tickets.iter().any(|t| t.key == key));
}

#[test]
fn no_agent_is_offered_a_way_to_start_a_task() {
    let seen_tools = Arc::new(Mutex::new(Vec::<(String, Vec<String>)>::new()));
    let log = seen_tools.clone();
    let m = mock(move |req| {
        log.lock().unwrap().push((system(req).chars().take(30).collect(), tool_names(req)));
        let sys = system(req);
        if sys.starts_with("You are a Backspace ticket agent") {
            return match results(req).last() {
                None => call("write", json!({"path": "README.md", "content": "Say the word.\n"})),
                Some(t) if t.starts_with("wrote") => call("submit_deliverable", json!({"checks_run": [], "assumptions": [], "open_questions": [], "summary": "done", "files": ["README.md"]})),
                Some(_) => say("worker done"),
            };
        }
        say("lead")
    });
    let (h, _ws) = open("st-tools", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send("build a dashboard system platform across several services");
    wait(&h, "the lead agent", |_| m.calls.load(Ordering::SeqCst) >= 1);
    h.start_task(SIDE_TASK).unwrap();
    wait(&h, "a worker", |_| seen_tools.lock().unwrap().iter().any(|(s, _)| s.starts_with("You are a Backspace ticket")));
    let seen = seen_tools.lock().unwrap();
    assert!(seen.iter().any(|(s, _)| s.starts_with("You are the lead agent")));
    for (who, tools) in seen.iter() {
        assert!(!tools.iter().any(|t| t.contains("start_task")), "{who} was offered a way to start a task: {tools:?}");
    }
    for path in [TaskType::Research, TaskType::OneShot] {
        assert!(!intake::allows(path, "start_task"), "{path:?}");
    }
}

// ------------------------------- start_task: project access and execution
//
// Authentication is not authorization. The token is issued for one project's
// server and `start_task` has no way to name another project, a path, a key or
// a state; a task it starts is an ordinary ticket of this project, so the
// project's own limits (policy, budget, agent cap) hold, and it can neither
// start nor approve a ticket that already exists.

fn serve_on(h: &Harness, token: &str) -> (u16, tokio_abort::Handle) {
    let port = free_port();
    let handle = h.serve(&format!("127.0.0.1:{port}"), token).unwrap();
    (port, tokio_abort::Handle(handle))
}

/// Keeps a server up for the length of a test.
mod tokio_abort {
    pub struct Handle(#[allow(dead_code)] pub tokio::task::AbortHandle);
}

fn worktrees(ws: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(ws.join(".backspace/worktrees"))
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

#[test]
fn a_valid_token_for_one_project_starts_nothing_in_another() {
    let m = side_task_mock();
    let (a, wa) = open("auth-a", m.port, ON, &[("README.md", "Say teh word.\n")]);
    let (b, wb) = open("auth-b", m.port, ON, &[("README.md", "Say teh word.\n")]);
    let (pa, _sa) = serve_on(&a, "token-a");
    let (pb, _sb) = serve_on(&b, "token-b");
    let body = json!({"text": SIDE_TASK}).to_string();
    // Each token is good on its own project's server and nowhere else.
    for (port, token) in [(pb, "Bearer token-a"), (pa, "Bearer token-b")] {
        let (code, text) = http_post(port, "/v1/task", Some(token), &body);
        assert_eq!(code, 401, "{token} on port {port}: {text}");
    }
    std::thread::sleep(Duration::from_millis(300));
    nothing_was_started(&a.snapshot(), &wa);
    nothing_was_started(&b.snapshot(), &wb);
    let b_trees = worktrees(&wb);

    // Project A's token, asking for more than it may name: another project's
    // folder, a key, a state, an agent. The request cannot say any of it.
    let greedy = json!({
        "text": SIDE_TASK, "workspace": wb, "project": wb, "path": wb, "cwd": wb,
        "key": "hijack", "ticket": "hijack", "state": "done", "agent": 0, "depth": 0
    })
    .to_string();
    let (code, text) = http_post(pa, "/v1/task", Some("Bearer token-a"), &greedy);
    assert_eq!(code, 200, "{text}");
    let key = serde_json::from_str::<Value>(&text).unwrap()["key"].as_str().unwrap().to_string();
    assert_ne!(key, "hijack", "the key is the harness's to choose");
    let s = wait(&a, "the delivery in project A", |s| s.pending_approvals().next().is_some());
    let t = s.tickets.iter().find(|t| t.key == key).unwrap();
    assert_eq!(t.state, TicketState::InReview, "it ran as an ordinary ticket, waiting for review; the requested state `done` is not honoured");
    assert_eq!(workers(&s).len(), 1);

    // Nothing at all happened in project B.
    std::thread::sleep(Duration::from_millis(500));
    nothing_was_started(&b.snapshot(), &wb);
    assert_eq!(worktrees(&wb), b_trees, "no worktree appeared in project B");
    assert_eq!(std::fs::read_to_string(wb.join("README.md")).unwrap(), "Say teh word.\n");
    assert!(!wb.join(".backspace/handoff").exists());
}

#[test]
fn a_started_task_never_starts_or_approves_a_ticket_that_already_exists() {
    // Tickets in states that do not permit execution: a proposed plan nobody
    // approved, one that needs information, one dropped, one done, one in review.
    let m = side_task_mock();
    let (h, ws) = open("st-states", m.port, ON, &[("README.md", "Say teh word.\n")]);
    h.send("hmm");
    wait(&h, "the question", |s| asked(s) == 1 && s.agents[0].status == AgentStatus::Idle);
    drop(h);
    let existing = [
        ("add-a-short-contributing", TicketState::Proposed),
        ("add-a-short-contributing-2", TicketState::Wontfix),
        ("add-a-short-contributing-3", TicketState::NeedsInfo),
        ("other-done", TicketState::Done),
        ("other-review", TicketState::InReview),
    ];
    let file = ws.join(".backspace/state.json");
    let mut state: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let seeded: Vec<backspace_core::Ticket> = existing
        .iter()
        .enumerate()
        .map(|(i, (key, st))| backspace_core::Ticket::from_json(&ticket(json!({"key": key})), i + 1, 0, *st).unwrap())
        .collect();
    state["tickets"] = json!(seeded);
    std::fs::write(&file, state.to_string()).unwrap();

    let h = Harness::open(ws.clone()).unwrap();
    // What reopening the project left them as is not start_task's doing.
    let before: Vec<(String, TicketState)> = h.snapshot().tickets.iter().map(|t| (t.key.clone(), t.state)).collect();
    assert_eq!(before.len(), existing.len(), "the seeded tickets were restored");
    // The new task's slug collides with three of them; it is a new ticket, not one of those.
    let key = h.start_task(SIDE_TASK).unwrap();
    assert_eq!(key, "add-a-short-contributing-4");
    let s = wait(&h, "the new task's worker", |s| s.agents.len() > 1 && s.pending_approvals().next().is_some());
    for (k, st) in &before {
        assert_eq!(s.tickets.iter().find(|t| &t.key == k).unwrap().state, *st, "`{k}` was touched by start_task");
    }
    for (k, st) in existing.iter().take(4) {
        assert_eq!(before.iter().find(|(b, _)| b == k).unwrap().1, *st, "`{k}` changed on reopening, before start_task");
    }
    let w = workers(&s);
    assert_eq!(w.len(), 1, "one worker, for the new ticket only: {:?}", w.iter().map(|a| &a.key).collect::<Vec<_>>());
    assert_eq!(w[0].ticket.as_deref(), Some(key.as_str()));
    assert!(!s.approvals.iter().any(|a| a.kind == ApprovalKind::Plan), "no plan was approved");
}

/// A project with extra config after the model list, and its tweaks.
fn open_tweaked(tag: &str, port: u16, extra: &str, tweak: impl Fn(String) -> String) -> (Harness, PathBuf) {
    let ws = std::env::temp_dir().join(format!("bs-intake-h-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("README.md"), "Say teh word.\n").unwrap();
    std::fs::write(ws.join("backspace.toml"), tweak(config_text(port, extra))).unwrap();
    (Harness::open(ws.clone()).unwrap(), ws)
}

fn worker_calls(m: &Mock) -> usize {
    m.seen.lock().unwrap().iter().filter(|r| system(r).starts_with("You are a Backspace ticket agent")).count()
}

#[test]
fn a_started_task_runs_under_the_project_policy_and_budget() {
    for (tag, extra, why) in [
        // A started task's worker may plan sub-tickets, so the policy sees it as an orchestration agent.
        ("st-policy", "[policy]\norchestration_models = [\"mock-front\"]\n", "refused by project policy"),
        ("st-budget", "[policy.budget]\nmax_usd = 0.0001\n", "budget"),
    ] {
        let m = side_task_mock();
        let (h, _ws) = open_tweaked(tag, m.port, extra, |c| c);
        let key = h.start_task(SIDE_TASK).unwrap();
        // The worker's status and its ticket's state are set one after the other: wait for both.
        let s = wait(&h, "the worker to be refused", |s| {
            s.agents.iter().any(|a| a.kind == backspace_core::AgentKind::Worker && a.status == AgentStatus::Failed)
                && s.tickets.iter().any(|t| t.key == key && t.state == TicketState::Failed)
        });
        let w = workers(&s);
        let errs: String = w[0].log.iter().filter(|l| l.kind == LogKind::Error).map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
        assert!(errs.contains(why), "{tag}: {errs}");
        assert_eq!(s.tickets.iter().find(|t| t.key == key).unwrap().state, TicketState::Failed, "{tag}");
        assert_eq!(worker_calls(&m), 0, "{tag}: the refused worker's request must not be made");
        assert!(s.pending_approvals().next().is_none(), "{tag}");
    }
}

#[test]
fn a_started_task_is_held_to_the_agent_cap() {
    let (m, started) = lifecycle_mock(vec![W::Hang], |_| say("lead"));
    let (h, _ws) = open_tweaked("st-cap", m.port, "", |c| c.replace("max_subagents = 4", "max_subagents = 1"));
    let first = h.start_task(SIDE_TASK).unwrap();
    wait(&h, "the first worker", |_| started.load(Ordering::SeqCst) == 1);
    let second = h.start_task("write a short SECURITY.md that says where to report bugs").unwrap();
    let s = wait(&h, "the cap to be reported", |s| {
        s.agents[0].log.iter().any(|l| l.text.contains(&second) && l.text.contains("could not start") && l.text.contains("agent cap"))
    });
    assert_eq!(workers(&s).len(), 1, "no second worker past the cap");
    assert_eq!(s.tickets.iter().find(|t| t.key == second).unwrap().state, TicketState::ReadyForAgent, "it waits; it is not run");
    assert_ne!(first, second);
}

#[test]
fn a_task_is_not_started_in_a_project_folder_that_has_gone() {
    let m = side_task_mock();
    let (h, ws) = open("st-gone", m.port, ON, &[("README.md", "Say teh word.\n")]);
    std::fs::remove_dir_all(&ws).unwrap();
    let e = h.start_task(SIDE_TASK).expect_err("the project folder is gone").to_string().to_lowercase();
    assert!(e.contains("project folder"), "{e}");
    assert!(!ws.exists(), "start_task must not recreate a folder the project no longer has");
    assert!(h.snapshot().tickets.is_empty());
}
