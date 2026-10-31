//! M1 hardening, third review, item 1: a Jev routing call's hold and its real
//! cost change hands in one step. The router records what the call cost while
//! its reservation is still held, under the reservation table's own lock
//! (`Reservation::settle`), so no other call can see the budget with the hold
//! gone and the spend not yet recorded, and a cost above the estimate is on
//! the books before the room is given back.
//!
//! The tests drive a real `Router` and a real `Reservations` table against a
//! mock Jev on a local port. The "books" stand in for the project's recorded
//! spend and are read the way the harness reads its own: inside the table's
//! lock, from the `lines` closure of a reservation.
//!
//! Also here: what the strict-budget status of Jev actually rests on (the
//! heuristic backend, and the fact that a pin does not stop a Jev call), and
//! that Jev is only offered models the project's policy allows.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex, Once};
use std::time::{Duration, Instant};

use backspace_core::config::Config;
use backspace_core::gateway::{BudgetLine, Reservations, ScopedLine};
use backspace_core::router::{AgentRole, Decision, Router};
use backspace_core::{Harness, LogKind};
use serde_json::{json, Value};

static ENV: Once = Once::new();

fn env() {
    ENV.call_once(|| {
        let home = std::env::temp_dir().join(format!("bs-jevsettle-home-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("BACKSPACE_DATA", home.join("data"));
        std::env::remove_var("BACKSPACE_CONFIG");
        std::env::set_var("BS_JEV_TEST_KEY", "test-key");
    });
}

// ------------------------------------------------------------------- mock Jev

struct Jev {
    port: u16,
    calls: Arc<AtomicUsize>,
    delay_ms: Arc<AtomicU64>,
    /// (input, output) tokens the next answers report.
    usage: Arc<Mutex<(u64, u64)>>,
    choice: Arc<Mutex<String>>,
    /// The model ids each request offered.
    offered: Arc<Mutex<Vec<Vec<String>>>>,
    body_len: Arc<Mutex<Vec<usize>>>,
}

fn jev() -> Jev {
    env();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let j = Jev {
        port,
        calls: Arc::default(),
        delay_ms: Arc::default(),
        usage: Arc::new(Mutex::new((100, 20))),
        choice: Arc::new(Mutex::new("mock-small".into())),
        offered: Arc::default(),
        body_len: Arc::default(),
    };
    let (calls, delay, usage, choice, offered, lens) = (j.calls.clone(), j.delay_ms.clone(), j.usage.clone(), j.choice.clone(), j.offered.clone(), j.body_len.clone());
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let mut stream = stream.unwrap();
            let (calls, delay, usage, choice, offered, lens) = (calls.clone(), delay.clone(), usage.clone(), choice.clone(), offered.clone(), lens.clone());
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
                    lens.lock().unwrap().push(len);
                    if let Ok(v) = serde_json::from_slice::<Value>(&body) {
                        let ids = v["questions"]["model"]["criteria"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
                        offered.lock().unwrap().push(ids);
                    }
                    std::thread::sleep(Duration::from_millis(delay.load(Ordering::SeqCst)));
                    let (i, o) = *usage.lock().unwrap();
                    json!({"answers": {"model": {"choice": choice.lock().unwrap().clone(), "confidence": 0.9}, "effort": {"score": 1.0}},
                        "usage": {"input_tokens": i, "output_tokens": o}})
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

const PRICE: f64 = 2000.0; // dollars per million tokens, the router's own rate

fn config(port: u16, router_extra: &str, orchestrator_extra: &str, policy: &str) -> String {
    format!(
        r#"
[router]
backend = "jev"
jev_endpoint = "http://127.0.0.1:{port}/route"
jev_model = "jev-test"
jev_api_key_env = "BS_JEV_TEST_KEY"
jev_usd_per_mtok = {PRICE}
main_min_effort = "high"
{router_extra}

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10
{orchestrator_extra}

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

fn parse(port: u16) -> Config {
    Config::parse(&config(port, "", "", "")).unwrap()
}

// ------------------------------------------------------------- the "books"

/// The project's recorded spend, standing in for the harness's state.
#[derive(Default)]
struct Books {
    spent: Mutex<f64>,
}

fn project(books: &Arc<Books>, limit: f64) -> impl Fn() -> Vec<ScopedLine> + Send + Sync + 'static {
    let books = books.clone();
    move || {
        let spent = *books.spent.lock().unwrap();
        vec![ScopedLine {
            key: "project".into(),
            line: BudgetLine { label: "project".into(), limit_usd: Some(limit), limit_tokens: None, spent_usd: spent, spent_tokens: 0 },
        }]
    }
}

/// A router wired like the harness's: it reserves in `table` against `books`, and records the
/// real cost into `books` in the step that releases the hold. `slow_record` sleeps while doing so.
fn router(port: u16, table: &Arc<Reservations>, books: &Arc<Books>, limit: f64, slow_record: Duration) -> Router {
    router_with(parse(port), table, books, limit, slow_record, None)
}

fn router_with(cfg: Config, table: &Arc<Reservations>, books: &Arc<Books>, limit: f64, slow_record: Duration, started: Option<mpsc::Sender<()>>) -> Router {
    let (t, b) = (table.clone(), books.clone());
    let lines = project(books, limit);
    let started = started.map(Mutex::new);
    Router::new(reqwest::Client::new(), cfg)
        .with_meter(move |usd, tokens| t.reserve_fixed("the router", usd, tokens, &lines))
        .with_recorder(move |usd| {
            if let Some(tx) = &started {
                let _ = tx.lock().unwrap().send(());
            }
            std::thread::sleep(slow_record);
            *b.spent.lock().unwrap() += usd;
        })
}

fn rt() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap())
}

fn route(rt: &tokio::runtime::Runtime, r: &Router) -> Decision {
    rt.block_on(r.route(AgentRole::Main, "fix a typo in the readme", "")).unwrap()
}

/// What one routing call reserves for this task, learned from a run against a roomy budget.
fn one_reservation(j: &Jev) -> f64 {
    let asked = Arc::new(Mutex::new(0.0));
    let (table, a2) = (Arc::new(Reservations::default()), asked.clone());
    let books = Arc::new(Books::default());
    let lines = project(&books, 1e9);
    let r = Router::new(reqwest::Client::new(), parse(j.port)).with_meter(move |usd, tokens| {
        *a2.lock().unwrap() = usd;
        table.reserve_fixed("the router", usd, tokens, &lines)
    });
    route(&rt(), &r);
    let usd = *asked.lock().unwrap();
    assert!(usd > 1.0, "at $2,000 a million, a call reserves dollars: {usd}");
    usd
}

fn tokens_for_cost(usd: f64) -> u64 {
    (usd / PRICE * 1e6) as u64
}

// ------------------------------------------------------------------- the tests

#[test]
fn simultaneous_jev_calls_cannot_jointly_pass_the_project_budget() {
    let j = jev();
    let r_one = one_reservation(&j);
    j.calls.store(0, Ordering::SeqCst);
    j.delay_ms.store(500, Ordering::SeqCst);
    *j.usage.lock().unwrap() = (100, 20);

    // Room for two calls' worst case, not three.
    let limit = r_one * 2.5;
    let (table, books) = (Arc::new(Reservations::default()), Arc::new(Books::default()));
    let router = Arc::new(router(j.port, &table, &books, limit, Duration::ZERO));
    let rt = rt();
    let gate = Arc::new(std::sync::Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let (router, rt, gate) = (router.clone(), rt.clone(), gate.clone());
            std::thread::spawn(move || {
                gate.wait();
                route(&rt, &router)
            })
        })
        .collect();
    let decisions: Vec<Decision> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert_eq!(j.calls.load(Ordering::SeqCst), 2, "only two worst cases fit at once; the other six were never sent");
    let by_jev: Vec<_> = decisions.iter().filter(|d| d.source == "jev").collect();
    assert_eq!(by_jev.len(), 2);
    for d in decisions.iter().filter(|d| d.source != "jev") {
        let note = d.note.clone().unwrap_or_default();
        assert!(d.source.starts_with("heuristic") && note.contains("refused by project policy"), "{} / {note}", d.source);
    }
    // What was spent is what the two calls really cost, recorded once each, and nothing is still held.
    let real = 120.0 * PRICE / 1e6;
    assert!((*books.spent.lock().unwrap() - 2.0 * real).abs() < 1e-9, "{} vs {}", books.spent.lock().unwrap(), 2.0 * real);
    assert!(by_jev.iter().all(|d| d.cost_settled), "the router says it already recorded the cost");
    assert_eq!(table.held("project"), (0.0, 0));
}

#[test]
fn nobody_sees_the_hold_gone_and_the_spend_not_yet_recorded() {
    let j = jev();
    let r_one = one_reservation(&j);
    // The call really costs most of what was held for it, so a gap would let a second call in.
    let t = tokens_for_cost(r_one * 0.9);
    *j.usage.lock().unwrap() = (t, 0);
    let limit = r_one * 1.5;

    let (table, books) = (Arc::new(Reservations::default()), Arc::new(Books::default()));
    let (tx, rx) = mpsc::channel();
    // Recording takes a while, as it can when the state is busy.
    let router = router_with(parse(j.port), &table, &books, limit, Duration::from_millis(300), Some(tx));
    let rt = rt();
    let call = std::thread::spawn(move || route(&rt, &router));

    // The moment the router starts recording, an API call tries to reserve the same size.
    let seen = Arc::new(Mutex::new(None::<f64>));
    let racer = {
        let (table, books, seen) = (table.clone(), books.clone(), seen.clone());
        std::thread::spawn(move || {
            rx.recv_timeout(Duration::from_secs(10)).expect("the router never recorded its cost while it still held the reservation");
            let lines = project(&books, limit);
            let seen2 = seen.clone();
            let books2 = books.clone();
            table.reserve_fixed("an api call", r_one * 0.2, 1000, move || {
                // Read inside the table's lock, as the harness reads its state.
                *seen2.lock().unwrap() = Some(*books2.spent.lock().unwrap());
                lines()
            })
        })
    };
    let d = call.join().unwrap();
    let got = racer.join().unwrap();

    let real = d.router_cost_usd;
    assert!(real > r_one * 0.85, "the call cost nearly what was held: {real} of {r_one}");
    let at_check = seen.lock().unwrap().expect("the racer checked the budget");
    assert!(
        (at_check - real).abs() < 1e-9,
        "the racer checked while the spend was {at_check}: it must see the recorded {real}, never the gap (hold gone, spend 0)"
    );
    // With the spend on the books ($0.9 of the $1.5 it may use), a $0.2 call (of one reservation) is told what is left.
    assert!(got.is_ok(), "{:?}", got.as_ref().err());
    drop(got); // the racer's own reservation is held until here
    assert_eq!(table.held("project"), (0.0, 0), "the router's hold is gone once the spend is recorded");
}

#[test]
fn a_cost_above_the_estimate_is_on_the_books_before_the_room_is_given_back() {
    let j = jev();
    let r_one = one_reservation(&j);
    // The answer is billed at three times what was held: Jev's answer size is an assumption, not a cap.
    let t = tokens_for_cost(r_one * 3.0);
    *j.usage.lock().unwrap() = (t, 0);
    let limit = r_one * 1.5;

    let (table, books) = (Arc::new(Reservations::default()), Arc::new(Books::default()));
    let (tx, rx) = mpsc::channel();
    let router = router_with(parse(j.port), &table, &books, limit, Duration::from_millis(200), Some(tx));
    let rt = rt();
    let call = std::thread::spawn(move || route(&rt, &router));

    let racer = {
        let (table, books) = (table.clone(), books.clone());
        std::thread::spawn(move || {
            rx.recv_timeout(Duration::from_secs(10)).expect("the router never recorded its cost while it still held the reservation");
            // A tiny call: it fits only if the budget looks unspent.
            table.reserve_fixed("an api call", 0.01, 10, project(&books, limit))
        })
    };
    let d = call.join().unwrap();
    let got = racer.join().unwrap();

    assert!(d.router_cost_usd > r_one * 2.9, "{}", d.router_cost_usd);
    assert!((*books.spent.lock().unwrap() - d.router_cost_usd).abs() < 1e-9, "the whole real cost is recorded, not capped at the reservation");
    assert!(got.is_err(), "the budget is overspent, so the next call is refused; admitting it would mean the overshoot was not recorded first");
    let note = d.note.clone().unwrap_or_default();
    assert!(note.contains("overshoot"), "and the decision says the call went past what was held: {note}");
    assert_eq!(table.held("project"), (0.0, 0));
}

#[test]
fn jev_calls_completing_among_api_reservations_never_leave_more_spent_than_the_limit() {
    let j = jev();
    let r_one = one_reservation(&j);
    // Each Jev call costs nearly its whole reservation; each API call costs exactly its own.
    *j.usage.lock().unwrap() = (tokens_for_cost(r_one * 0.95), 0);
    j.delay_ms.store(120, Ordering::SeqCst);
    let limit = r_one * 3.2;
    let learned = j.calls.load(Ordering::SeqCst); // the sizing call above is not on these books

    let (table, books) = (Arc::new(Reservations::default()), Arc::new(Books::default()));
    let router = Arc::new(router_with(parse(j.port), &table, &books, limit, Duration::from_millis(15), None));
    let rt = rt();
    let admitted_api = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];
    for i in 0..8u64 {
        let (router, rt) = (router.clone(), rt.clone());
        handles.push(std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(i * 35));
            route(&rt, &router);
        }));
        let (table, books, admitted) = (table.clone(), books.clone(), admitted_api.clone());
        handles.push(std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(i * 35 + 17));
            if let Ok(res) = table.reserve_fixed("an api call", r_one, 1000, project(&books, limit)) {
                admitted.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(60));
                // An API call's actual cost is its reservation.
                let books = books.clone();
                res.settle(move || {
                    std::thread::sleep(Duration::from_millis(15));
                    *books.spent.lock().unwrap() += r_one;
                });
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let spent = *books.spent.lock().unwrap();
    assert!(spent <= limit + 1e-9, "spent {spent} of a {limit} limit: a call was let in while another's spend was not yet on the books");
    // Every call that ran is on the books at what it really cost: nothing lost, nothing counted twice.
    let jev_each = tokens_for_cost(r_one * 0.95) as f64 * PRICE / 1e6;
    let jev_calls = j.calls.load(Ordering::SeqCst) - learned;
    let expected = jev_calls as f64 * jev_each + admitted_api.load(Ordering::SeqCst) as f64 * r_one;
    assert!((spent - expected).abs() < 1e-6, "recorded {spent}, but {jev_calls} Jev calls and {} API calls cost {expected}", admitted_api.load(Ordering::SeqCst));
    assert!(jev_calls >= 1 && admitted_api.load(Ordering::SeqCst) >= 1, "both kinds of call took part");
    assert_eq!(table.held("project"), (0.0, 0));
}

#[test]
fn a_billed_answer_the_router_cannot_use_is_recorded_once_and_the_fallback_is_not_charged_again() {
    let j = jev();
    *j.choice.lock().unwrap() = "no-such-model".into();
    let (table, books) = (Arc::new(Reservations::default()), Arc::new(Books::default()));
    let r = router(j.port, &table, &books, 1e9, Duration::ZERO);
    let d = route(&rt(), &r);
    assert!(d.source.starts_with("heuristic"), "the answer named a model that was not offered: {}", d.source);
    assert_eq!(d.router_cost_usd, 0.0, "the heuristic decision carries no router cost");
    assert!(!d.cost_settled, "so there is nothing for the harness to add or to skip");
    let real = 120.0 * PRICE / 1e6;
    assert!((*books.spent.lock().unwrap() - real).abs() < 1e-9, "Jev billed for the call whether or not the answer was usable: {}", books.spent.lock().unwrap());
    assert_eq!(table.held("project"), (0.0, 0));
}

// ------------------------------------------------------------------ the harness

fn open(tag: &str, cfg: String) -> Harness {
    let ws = std::env::temp_dir().join(format!("bs-jevsettle-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
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
        assert!(Instant::now() < deadline, "the project never routed: {:?}", s.agents[0].log.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_jev_decision_reaches_the_projects_spend_exactly_once() {
    let j = jev();
    let h = open("once", config(j.port, "", "max_project_usd = 50.0", ""));
    h.send("fix a typo in the readme");
    routed(&h);
    let s = h.snapshot();
    let one_call = 120.0 * PRICE / 1e6;
    assert!((s.router_cost_usd - one_call).abs() < 1e-9, "router cost {} for one call of {one_call}", s.router_cost_usd);
    assert!((s.total_cost_usd - one_call).abs() < 1e-9, "the project total is that one call, not counted twice: {}", s.total_cost_usd);
    let (router, total) = (s.router_cost_usd, s.total_cost_usd);
    std::thread::sleep(Duration::from_millis(300));
    let s = h.snapshot();
    assert_eq!((s.router_cost_usd, s.total_cost_usd), (router, total), "and nothing adds it a second time later");
}

// ---------------------------------------------- strict budgets: what Jev rests on

#[test]
fn the_heuristic_backend_is_the_supported_way_to_keep_jev_off_a_project() {
    let j = jev();
    let h = open("heuristic", config(j.port, "", "max_project_usd = 50.0", "").replace("backend = \"jev\"", "backend = \"heuristic\""));
    h.send("fix a typo in the readme");
    let text = routed(&h);
    assert_eq!(j.calls.load(Ordering::SeqCst), 0, "not one request went to Jev: {text}");
    assert!(text.contains("routed by heuristic"), "{text}");
    assert_eq!(h.snapshot().router_cost_usd, 0.0);
}

#[test]
fn a_pinned_model_does_not_stop_the_jev_call() {
    // Recorded because the documents must not say otherwise: Jev is asked first and the pin overrides its answer.
    let j = jev();
    let cfg = Config::parse(&config(j.port, "pin_main = \"mock-big\"", "", "")).unwrap();
    let d = route(&rt(), &Router::new(reqwest::Client::new(), cfg));
    assert_eq!(j.calls.load(Ordering::SeqCst), 1);
    assert_eq!(d.model, "mock-big");
    assert!(d.source.contains("pinned"), "{}", d.source);
}

// ------------------------------------------- project policy and what Jev is offered

const ONLY_SMALL: &str = "[policy]\nmode = \"enforced\"\norchestration_models = [\"mock-small\"]\nexecution_models = [\"mock-small\"]\nskills = []\nplugins = []\n";

#[test]
fn jev_is_only_offered_models_the_project_policy_allows_for_the_role() {
    let j = jev();
    let cfg = Config::parse(&config(j.port, "", "", ONLY_SMALL)).unwrap();
    let d = route(&rt(), &Router::new(reqwest::Client::new(), cfg));
    assert_eq!(d.model, "mock-small");
    let offered = j.offered.lock().unwrap().clone();
    assert_eq!(offered, vec![vec!["mock-small".to_string()]], "mock-big is not in the lists, so Jev never hears of it");
}

#[test]
fn a_model_jev_names_that_it_was_not_offered_is_not_used() {
    let j = jev();
    *j.choice.lock().unwrap() = "mock-big".into();
    let cfg = Config::parse(&config(j.port, "", "", ONLY_SMALL)).unwrap();
    let d = route(&rt(), &Router::new(reqwest::Client::new(), cfg));
    assert_eq!(j.calls.load(Ordering::SeqCst), 1);
    assert_eq!(d.model, "mock-small", "the heuristic decided instead");
    assert!(d.note.clone().unwrap_or_default().contains("jev chose unknown model"), "{:?}", d.note);
}

// ------------------------------------------------------ Jev and `max_project_tokens`

#[test]
fn a_jev_call_must_fit_the_project_token_limit_and_its_tokens_are_not_counted_afterwards() {
    // The hold is in tokens as well as dollars, so a project token limit the call cannot fit refuses it.
    let j = jev();
    let h = open("tok-tight", config(j.port, "", "max_project_tokens = 600", ""));
    h.send("fix a typo in the readme");
    let text = routed(&h);
    assert_eq!(j.calls.load(Ordering::SeqCst), 0, "Jev was not called: {text}");
    assert!(text.contains("routed by heuristic"), "{text}");
    drop(h);

    // With room it is called, and the 120 tokens it reported are on the project's dollars only:
    // the router's tokens are not added to the agents' token count that `max_project_tokens` is
    // measured against. (The mock answers the agent's own API call with an error, so no agent
    // reports a token either.)
    let j = jev();
    let h = open("tok-roomy", config(j.port, "", "max_project_tokens = 10000000", ""));
    h.send("fix a typo in the readme");
    routed(&h);
    std::thread::sleep(Duration::from_millis(300));
    let s = h.snapshot();
    assert_eq!(j.calls.load(Ordering::SeqCst), 1);
    assert!(s.router_cost_usd > 0.0, "{}", s.router_cost_usd);
    assert_eq!(s.agents.iter().map(|a| a.input_tokens + a.output_tokens).sum::<u64>(), 0, "router tokens are not agent tokens");
}
