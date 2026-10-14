//! The features added on 14 Oct 2026, end to end where it costs nothing to do
//! so: board conversations between agents, the local tiny model a subagent is
//! pointed at (reached over HTTP like a real Ollama would be), and the account
//! registry's failover policy.
//!
//! Deliberately no VM and no real model: the point is the wiring, which is
//! what regressed the last time this repo moved these pieces around.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use backspace_core::accounts::{Accounts, FailoverPolicy, Usage};
use backspace_core::board::{self, BoardMsg, Cursors};
use backspace_core::config::Config;
use backspace_core::prefs::Prefs;
use backspace_core::project::{AgentRecord, ProjectState};
use backspace_core::{Effort, Harness};
use serde_json::{json, Value};

/// `BACKSPACE_PREFS`, `BACKSPACE_DATA` and `BACKSPACE_CONFIG` are process-wide,
/// and the harness reads them when a project opens. Two tests that need their
/// own decisions therefore cannot run at the same time: this lock serializes
/// them. The failover/board tests do not take it — they never open a project.
static GLOBALS: Mutex<()> = Mutex::new(());

fn agent(id: usize, key: &str, kind: &str, role: &str) -> AgentRecord {
    serde_json::from_value(json!({
        "id": id, "role": role, "kind": kind, "parent": null, "depth": 0, "key": key,
        "title": key, "brief": "", "depends_on": [], "status": "idle", "decision": null,
        "log": [], "cost_usd": 0.0, "input_tokens": 0, "output_tokens": 0, "deliverable": null,
        "budget_usd": null, "ticket": null, "branch": null, "worktree": null, "escalations": []
    }))
    .unwrap()
}

fn project() -> ProjectState {
    let mut s = ProjectState::empty("p");
    s.agents.push(agent(0, "main", "main", "main"));
    s.agents.push(agent(1, "api", "worker", "sub"));
    s.agents.push(agent(2, "web", "worker", "sub"));
    s
}

// ------------------------------------------------------------------ board

#[test]
fn a_direct_message_between_two_agents_is_its_own_conversation() {
    let mut s = project();
    let q = board::post(&mut s, "api", "web", "which port is the API on?").unwrap();
    board::reply(&mut s, "web", "api", "8080; don't touch the config", Some(q.id)).unwrap();
    board::post(&mut s, "main", "all", "freeze the API").unwrap();

    // The pair's thread has both messages, in order, and names its sides.
    let pair = board::between(&s, "web", "api");
    assert_eq!(pair.len(), 2);
    assert_eq!(pair[0].from, "api");
    assert_eq!(pair[1].reply_to, Some(q.id));
    assert_eq!(pair[1].thread.as_deref(), pair[0].thread.as_deref());
    // Whichever way round the pair is asked for.
    assert_eq!(board::between(&s, "api", "web").len(), 2);

    // The broadcast is separate, and the three conversations are two.
    let threads = board::conversations(&s);
    assert_eq!(threads.len(), 2);
    let pair_thread = threads.iter().find(|t| t.sides.len() == 2).unwrap();
    assert_eq!(pair_thread.messages.len(), 2);
    let broadcast = threads.iter().find(|t| t.sides.len() == 1).unwrap();
    assert_eq!(broadcast.messages[0].to, "all");

    // Each side of the pair got the other's message, and nobody else's: `web`
    // has the question and its own answer is not in its inbox, while `api` has
    // the answer plus the broadcast — a message you sent is never in your own
    // inbox.
    let mut c = Cursors::default();
    let web_inbox = c.take(&s, "web");
    assert!(web_inbox.contains("which port"), "web got: {web_inbox:?}");
    assert!(!web_inbox.contains("8080"), "web got: {web_inbox:?}");
    let api_inbox = c.take(&s, "api");
    assert!(api_inbox.contains("8080"), "api got: {api_inbox:?}");
    assert!(api_inbox.contains("freeze the API"), "api got: {api_inbox:?}");
    let main_inbox = c.take(&s, "main");
    assert_eq!(main_inbox, "No new messages.", "main is in neither conversation");
}

#[test]
fn read_messages_hands_back_the_id_a_reply_can_name() {
    let mut s = project();
    board::post(&mut s, "api", "main", "schema is in db.sql").unwrap();
    let mut c = Cursors::default();
    // The id is in the line, so a CLI worker can answer without another call.
    assert_eq!(c.take(&s, "main"), "[#0 api]: schema is in db.sql");
    assert_eq!(c.take(&s, "main"), "No new messages.");
}

#[test]
fn a_reply_to_a_message_that_is_not_there_is_refused() {
    let mut s = project();
    assert!(board::reply(&mut s, "api", "web", "re: nothing", Some(7)).is_err());
    assert!(s.board.is_empty(), "a refused reply must not land on the board");
}

#[test]
fn the_tick_gates_the_local_model_and_scouts_only_follow_their_own() {
    let base = Config::parse(backspace_core::config::DEFAULT_CONFIG).unwrap();
    let before = base.advisor.scout_model.clone();
    let mut prefs = Prefs::default();
    prefs.subagent_model = Some("ollama:lfm2.5:latest".into());

    // Triage ticked, scouts not: the model is in the config (so routing can
    // reach it) but scouts keep the config's own scout model.
    prefs.local_tasks = vec!["triage".into()];
    let mut cfg = base.clone();
    backspace_core::local_subagent(&mut cfg, &prefs);
    assert!(cfg.model("ollama:lfm2.5:latest").is_some(), "the model is usable for triage");
    assert_eq!(cfg.advisor.scout_model, before, "scouts were not ticked");

    // Scouts ticked: now they take it.
    prefs.local_tasks = vec!["scouts".into()];
    let mut cfg = base.clone();
    backspace_core::local_subagent(&mut cfg, &prefs);
    assert_eq!(cfg.advisor.scout_model.as_deref(), Some("ollama:lfm2.5:latest"));

    // No job ticked at all: nothing is pointed at the model, even though one
    // is chosen — the panel says "keep at least one job on", and this is why.
    prefs.local_tasks = vec!["something-else".into()];
    let mut cfg = base.clone();
    backspace_core::local_subagent(&mut cfg, &prefs);
    assert_eq!(cfg.advisor.scout_model, before);
    assert!(cfg.model("ollama:lfm2.5:latest").is_none());
}

// ------------------------------------------------------- local tiny models

#[test]
fn a_chosen_local_model_becomes_the_scout_model() {
    let mut cfg = Config::parse(backspace_core::config::DEFAULT_CONFIG).unwrap();
    let mut prefs = Prefs::default();
    prefs.subagent_model = Some("ollama:lfm2.5:latest".into());

    backspace_core::local_subagent(&mut cfg, &prefs);

    assert_eq!(cfg.advisor.scout_model.as_deref(), Some("ollama:lfm2.5:latest"));
    let m = cfg.model("ollama:lfm2.5:latest").expect("the local model is in the config");
    assert_eq!(m.provider, "ollama");
    // The wire name keeps its tag, minus the runtime prefix.
    assert_eq!(m.wire_model(), "lfm2.5:latest");
    assert_eq!(m.tier, 0, "cheapest, so nothing else outranks it for a scout");
    assert_eq!(m.input_usd_per_mtok, 0.0);
    let p = &cfg.providers["ollama"];
    assert_eq!(p.base_url, "http://localhost:11434/v1");
    // A keyless local endpoint counts as usable.
    assert!(cfg.usable_models().iter().any(|u| u.id == m.id));
}

#[test]
fn a_local_model_on_a_custom_router_is_found_by_id() {
    let mut cfg = Config::parse(backspace_core::config::DEFAULT_CONFIG).unwrap();
    let mut prefs = Prefs::default();
    prefs.routers.push(backspace_core::prefs::RouterCfg {
        id: "studio".into(),
        name: "Studio".into(),
        base_url: "http://127.0.0.1:9999/v1/".into(),
        api_key: String::new(),
        models: vec![],
    });
    prefs.subagent_model = Some("studio:qwen3:1.7b".into());
    backspace_core::local_subagent(&mut cfg, &prefs);
    assert_eq!(cfg.providers["studio"].base_url, "http://127.0.0.1:9999/v1");
    assert_eq!(cfg.model("studio:qwen3:1.7b").unwrap().wire_model(), "qwen3:1.7b");
}

#[test]
fn no_choice_or_an_unknown_runtime_leaves_the_config_alone() {
    let base = Config::parse(backspace_core::config::DEFAULT_CONFIG).unwrap();
    let before = base.advisor.scout_model.clone();

    let mut cfg = base.clone();
    backspace_core::local_subagent(&mut cfg, &Prefs::default());
    assert_eq!(cfg.advisor.scout_model, before, "no choice: nothing changes");

    let mut cfg = base.clone();
    let mut prefs = Prefs::default();
    prefs.subagent_model = Some("somewhere:tiny".into());
    backspace_core::local_subagent(&mut cfg, &prefs);
    assert_eq!(cfg.advisor.scout_model, before, "an unknown runtime is ignored, not guessed at");

    // A malformed value must not panic or half-apply either.
    for bad in ["", ":", "ollama:", ":lfm2.5"] {
        let mut cfg = base.clone();
        let mut prefs = Prefs::default();
        prefs.subagent_model = Some(bad.into());
        backspace_core::local_subagent(&mut cfg, &prefs);
        assert_eq!(cfg.advisor.scout_model, before, "bad value {bad:?}");
    }
}

/// A stand-in for a local runtime (Ollama included): an OpenAI-compatible
/// `/v1/chat/completions` that answers, and records what it was asked.
fn local_model_server() -> (u16, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let sink = sink.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; len];
                let _ = reader.read_exact(&mut body);
                let req: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                sink.lock().unwrap().push(req.clone());
                // The tiny model's own words, so a match cannot come from the
                // big model's mock by accident.
                let out = json!({
                    "choices": [{"finish_reason": "stop", "message": {"content": "seen by the small local model"}}],
                    "usage": {"prompt_tokens": 12, "completion_tokens": 4}
                })
                .to_string();
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    (port, seen)
}

/// A stand-in for the big subscription model: the lead delegates one lookup.
fn big_model_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; len];
                let _ = reader.read_exact(&mut body);
                let req: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                let msgs = req["messages"].as_array().cloned().unwrap_or_default();
                let system = msgs.first().and_then(|m| m["content"].as_str()).unwrap_or("");
                let tools_seen = msgs.iter().filter(|m| m["role"] == "tool").count();
                let out = if system.starts_with("You are the lead agent") && tools_seen == 0 {
                    json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
                        {"id": "c1", "type": "function", "function": {"name": "scout", "arguments": json!({"tasks": ["where is the token stored?"]}).to_string()}}]}}],
                        "usage": {"prompt_tokens": 900, "completion_tokens": 40}})
                } else {
                    json!({"choices": [{"finish_reason": "stop", "message": {"content": "plan written"}}],
                        "usage": {"prompt_tokens": 900, "completion_tokens": 40}})
                }
                .to_string();
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    port
}

/// The whole point of the local-model work, end to end: the user picks a tiny
/// model on this machine, the lead delegates a lookup with `scout`, and the
/// *local* endpoint is what answers — while the lead keeps talking to the
/// subscription model.
#[test]
fn a_delegated_lookup_reaches_the_chosen_local_model() {
    let _guard = GLOBALS.lock().unwrap_or_else(|e| e.into_inner());
    let (local_port, local_seen) = local_model_server();
    let big_port = big_model_server();

    // Prefs live in a temp file so the choice is what the harness loads.
    let dir = std::env::temp_dir().join(format!("bs-local-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("BACKSPACE_PREFS", dir.join("prefs.json"));
    std::env::set_var("BACKSPACE_DATA", dir.join("data"));

    let mut prefs = Prefs::default();
    // Point "ollama" at the stand-in: the same base_url shape the real one has.
    prefs.ollama_url = format!("http://127.0.0.1:{local_port}");
    prefs.subagent_model = Some("ollama:lfm2.5:latest".into());
    prefs.save();

    let ws = dir.join("project");
    std::fs::create_dir_all(&ws).unwrap();
    let cfg = format!(
        r#"
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"
pin_main = "mock-big"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = true
bash_timeout_secs = 10

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{big_port}/v1"
api_key_env = "NONE"

[[models]]
id = "mock-big"
provider = "mock"
tier = 3
description = "the subscription model"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000
"#
    );
    // The project's own backspace.toml: per-workspace, so two tests in one
    // process cannot fight over a config path.
    std::fs::write(ws.join("backspace.toml"), cfg).unwrap();

    let h = Harness::open(ws.clone()).expect("open the project");
    h.send("where is the token stored?");
    // One turn for the lead to call scout, then the scout's own turn.
    let mut waited = 0;
    while waited < 60 {
        let s = h.snapshot();
        let answered = s
            .agents
            .iter()
            .any(|a| a.log.iter().any(|l| l.text.contains("seen by the small local model")));
        if answered {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
        waited += 1;
    }

    let seen = local_seen.lock().unwrap().clone();
    let s = h.snapshot();
    let scout = s.agents.iter().find(|a| a.kind == backspace_core::AgentKind::Scout);
    let report = format!(
        "local requests: {} | scouts: {:?} | lead model: {:?}",
        seen.len(),
        scout.map(|a| a.decision.as_ref().map(|d| d.model.clone())),
        s.agents.iter().find(|a| a.id == 0).and_then(|a| a.decision.as_ref()).map(|d| d.model.clone())
    );
    assert!(!seen.is_empty(), "the local model was never asked: {report}");
    let asked = &seen[0];
    // It was asked as an Ollama/OpenAI model, with the local model's own name.
    assert_eq!(asked["model"], "lfm2.5:latest", "{report}");
    assert_eq!(scout.and_then(|a| a.decision.as_ref().map(|d| d.model.clone())).as_deref(), Some("ollama:lfm2.5:latest"), "{report}");
    // The small jobs run at low effort whatever the lead is doing.
    assert_eq!(scout.unwrap().decision.as_ref().unwrap().effort, Effort::Low, "{report}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn local_jobs_defaults_to_scouts_and_refuses_unknown_words() {
    use backspace_core::harness::local_jobs;
    // No model: nothing runs locally, whatever the task list says.
    let mut prefs = Prefs::default();
    prefs.local_tasks = vec!["scouts".into(), "triage".into()];
    assert!(local_jobs(&prefs).is_empty());

    // A model but no explicit list: scouts, the safe read-only default.
    prefs.subagent_model = Some("ollama:lfm2.5:latest".into());
    prefs.local_tasks = vec![];
    let jobs = local_jobs(&prefs);
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs["scouts"], "ollama:lfm2.5:latest");

    // The user's list wins, singulars and spacing included, and a word the
    // harness does not know is dropped rather than guessed at.
    prefs.local_tasks = vec!["Triage".into(), " worker ".into(), "planner".into()];
    let jobs = local_jobs(&prefs);
    let mut keys: Vec<&str> = jobs.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, vec!["triage", "workers"]);
}

/// The other half of the local-model wiring: the *triage* job, not just
/// scouts, when the user asks for it. Same stand-ins, no Ollama and no API
/// key at all — the whole project runs on the tiny local model.
#[test]
fn triage_can_run_on_the_local_model_with_no_api_key() {
    let _guard = GLOBALS.lock().unwrap_or_else(|e| e.into_inner());
    let (local_port, local_seen) = local_triage_server();

    let dir = std::env::temp_dir().join(format!("bs-triage-local-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("BACKSPACE_PREFS", dir.join("prefs.json"));
    std::env::set_var("BACKSPACE_DATA", dir.join("data"));

    let mut prefs = Prefs::default();
    prefs.ollama_url = format!("http://127.0.0.1:{local_port}");
    prefs.subagent_model = Some("ollama:qwen3:1.7b".into());
    prefs.local_tasks = vec!["triage".into()];
    prefs.save();

    let ws = dir.join("project");
    std::fs::create_dir_all(&ws).unwrap();
    // The project's own backspace.toml, so this test never depends on a
    // process-wide variable another test may be using.
    let cfg_path = ws.join("backspace.toml");
    std::fs::write(
        &cfg_path,
        r#"
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"

[orchestrator]
max_parallel_calls = 1
max_subagents = 2
auto_approve = true
bash_timeout_secs = 10

[providers.nowhere]
kind = "openai"
base_url = "http://127.0.0.1:1/v1"
api_key_env = "NO_SUCH_KEY"

[[models]]
id = "placeholder"
provider = "nowhere"
tier = 1
description = "unusable: its provider has no credentials"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 100
"#,
    )
    .unwrap();

    let h = Harness::open(ws.clone()).expect("open the project");
    let filed = h.file_ticket("Crash on empty input", "The parser panics on an empty file").unwrap();
    assert!(filed.contains("crash-on-empty-input"), "{filed}");

    let mut waited = 0;
    while waited < 60 {
        let s = h.snapshot();
        if s.tickets.first().map(|t| t.state) != Some(backspace_core::TicketState::NeedsTriage) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
        waited += 1;
    }

    let s = h.snapshot();
    let t = s.tickets.first().expect("the ticket");
    let seen = local_seen.lock().unwrap().clone();
    let report = format!(
        "local requests: {} (models: {:?}) | ticket state: {:?} | notes: {:?} | agents: {:?} | config: {:?}",
        seen.len(),
        seen.iter().map(|r| r["model"].clone()).collect::<Vec<_>>(),
        t.state,
        t.notes,
        s.agents
            .iter()
            .map(|a| format!("{}:{:?}{}", a.key, a.status, a.log.last().map(|l| l.text.clone()).unwrap_or_default()))
            .collect::<Vec<_>>(),
        s.config_source
    );
    assert!(!seen.is_empty(), "the local model was never asked: {report}");
    assert_eq!(seen[0]["model"], "qwen3:1.7b", "{report}");
    assert_eq!(t.state, backspace_core::TicketState::ReadyForAgent, "{report}");
    let triager = s
        .agents
        .iter()
        .find(|a| a.kind == backspace_core::AgentKind::Triage)
        .expect("the triage agent");
    assert_eq!(
        triager.decision.as_ref().map(|d| d.model.as_str()),
        Some("ollama:qwen3:1.7b"),
        "{report}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A stand-in for a local runtime asked to triage a ticket: it answers with
/// the `triage_ticket` tool call the harness expects.
fn local_triage_server() -> (u16, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let sink = sink.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; len];
                let _ = reader.read_exact(&mut body);
                let req: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                sink.lock().unwrap().push(req.clone());
                let out = json!({
                    "choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
                        {"id": "t1", "type": "function", "function": {"name": "triage_ticket", "arguments": json!({
                            "state": "ready_for_agent",
                            "category": "bug",
                            "notes": "reproduced on an empty file",
                            "acceptance": ["no crash on empty input"],
                            "check": "true"
                        }).to_string()}}]}}],
                    "usage": {"prompt_tokens": 40, "completion_tokens": 12}
                })
                .to_string();
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    (port, seen)
}

// --------------------------------------------------------------- accounts

#[test]
fn one_limited_login_does_not_stop_the_project() {
    let mut a = Accounts { policy: FailoverPolicy::Automatic, list: vec![] };
    let one = a.add("claude", "Personal");
    let two = a.add("claude", "Work");
    let now = 1_800_000_000_000u64;

    // Give the second login a head start, then a success each: the one that
    // succeeded most recently wins, because both are otherwise healthy.
    a.note_ok(&two.id, now);
    a.note_ok(&one.id, now + 1);
    assert_eq!(a.best_for("claude", now + 2).unwrap().id, one.id);

    // A confirmed limit parks it and the next turn moves to the other login.
    a.note_limit(&one.id, now + 2, 0, "5-hour limit reached".into());
    assert_eq!(a.best_for("claude", now + 2).unwrap().id, two.id);
    a.set_usage(&two.id, Usage { used: Some(3), limit: Some(100), resets_at: 0, report: "3/100".into() });
    assert_eq!(a.list[1].usage.as_ref().unwrap().used, Some(3));

    // Policy Off turns the whole registry off: the machine's own login is used.
    a.policy = FailoverPolicy::Off;
    assert!(a.best_for("claude", now + 2).is_none());

    // A plain failure must never park a login: only a limit report does. The
    // failed one still loses to the healthy one, but it stays usable.
    a.policy = FailoverPolicy::Automatic;
    a.note_ok(&two.id, now + 3);
    a.note_fail(&one.id, now + 4);
    assert_eq!(a.best_for("claude", now + 5).unwrap().id, two.id);
    assert!(a.best_for("claude", now + 5).is_some(), "a failure is not a cooldown");
}

#[test]
fn a_limit_is_recognised_from_the_clis_own_words_only() {
    use backspace_core::accounts::looks_like_usage_limit as limit;
    assert!(limit("Claude usage limit reached. Resets at 3pm."));
    assert!(limit("You've hit your weekly limit"));
    assert!(limit(r#"{"error":{"type":"insufficient_quota"}}"#));
    // 429s, crashes and network blips are not limits.
    assert!(!limit("HTTP 429 Too Many Requests"));
    assert!(!limit("connection reset by peer"));
    assert!(!limit("the model returned an empty response"));
}

#[test]
fn accounts_round_trip_and_keep_their_env() {
    let dir: PathBuf = std::env::temp_dir().join(format!("bs-accounts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("accounts.json");

    let mut a = Accounts { policy: FailoverPolicy::Manual, list: vec![] };
    let acc = a.add("claude", "Work Claude");
    a.add("codex", "Codex Work");
    a.save(&path).unwrap();

    let back = Accounts::load(&path);
    assert_eq!(back.list.len(), 2);
    assert_eq!(back.policy, FailoverPolicy::Manual);
    assert_eq!(back.list[0].id, acc.id);
    // The profile directory is per provider and per id, and the env var is the
    // CLI's own (accounts.rs::profile_env).
    let env = back.env_for(&acc.id);
    assert_eq!(env.len(), 1);
    assert_eq!(env[0].0, "CLAUDE_CONFIG_DIR");
    assert!(env[0].1.contains("accounts"));
    assert!(Accounts::load(&dir.join("nope.json")).list.is_empty(), "a missing file is empty, not a panic");
    std::fs::remove_dir_all(&dir).unwrap();
}

// ------------------------------------------------------------------ helpers

#[test]
fn the_board_message_shape_survives_a_round_trip() {
    // Threads and replies are persisted in state.json; an old file has neither.
    let old: BoardMsg = serde_json::from_value(json!({
        "id": 0, "from": "api", "to": "main", "text": "hi", "at": 1
    }))
    .unwrap();
    assert!(old.reply_to.is_none() && old.thread.is_none());
    assert_eq!(board::thread_of(&old.from, &old.to), "api|main");
}
