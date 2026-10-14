//! The features added on 14 Oct 2026, end to end where it costs nothing to do
//! so: board conversations between agents, the local tiny model a subagent is
//! pointed at, and the account registry's failover policy.
//!
//! Deliberately no network and no VM: the point is the wiring, which is what
//! regressed the last time this repo moved these pieces around.

use backspace_core::accounts::{Accounts, FailoverPolicy, Usage};
use backspace_core::board::{self, BoardMsg, Cursors};
use backspace_core::config::Config;
use backspace_core::prefs::Prefs;
use backspace_core::project::{AgentRecord, ProjectState};
use serde_json::json;

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
    let dir = std::env::temp_dir().join(format!("bs-accounts-{}", std::process::id()));
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
