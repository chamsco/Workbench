//! M1 final hardening, section 2: a Jev request that was sent and then failed
//! has a cost nobody was told. Its reservation is not released as if nothing
//! had happened: the amount held for it is counted as spent, marked as an
//! unknown cost, in the same locked step that gives the hold back.
//!
//! Cases: a request that never left (refused connection, an endpoint that is
//! not a URL) releases its hold; one that was sent and then timed out, was cut
//! off, got an error status, a body that is not JSON or an answer with no usage
//! is counted at its hold; one that fails but says what it cost is counted at
//! that cost. A later request cannot use the room, a retry neither frees nor
//! double-counts the failed attempt, and a mixture run concurrently adds up.
//!
//! Real `Router` and `Reservations` against a mock Jev on a local port. The
//! "books" stand in for the project's recorded spend, read the way the harness
//! reads its own: inside the table's lock, from the `lines` of a reservation.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
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
        let home = std::env::temp_dir().join(format!("bs-jevunk-home-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("BACKSPACE_DATA", home.join("data"));
        std::env::remove_var("BACKSPACE_CONFIG");
        std::env::set_var("BS_JEV_TEST_KEY", "test-key");
    });
}

// ------------------------------------------------------------------- mock Jev

/// What the mock does with one request.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    /// A good answer that reports 120 tokens.
    Ok,
    /// Takes far longer than the router waits, then answers.
    Hang,
    /// Reads the request and closes the connection without a word.
    Cut,
    /// HTTP 500, no body that says what it cost.
    Error500,
    /// HTTP 500 whose body reports 120 tokens of usage.
    Error500Billed,
    /// HTTP 200 with a body that is not JSON.
    Garbage,
    /// HTTP 200, a good answer, no usage in it.
    NoUsage,
    /// HTTP 200 that reports 120 tokens but names a model that was not offered.
    BilledButUnusable,
}

struct Jev {
    port: u16,
    queue: Arc<Mutex<VecDeque<Mode>>>,
    /// What the mock did, in the order the requests arrived.
    served: Arc<Mutex<Vec<Mode>>>,
    calls: Arc<AtomicUsize>,
}

impl Jev {
    fn next(&self, modes: &[Mode]) {
        self.queue.lock().unwrap().extend(modes.iter().copied());
    }
}

fn jev() -> Jev {
    env();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let j = Jev { port, queue: Arc::default(), served: Arc::default(), calls: Arc::default() };
    let (queue, served, calls) = (j.queue.clone(), j.served.clone(), j.calls.clone());
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let mut stream = stream.unwrap();
            let (queue, served, calls) = (queue.clone(), served.clone(), calls.clone());
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
                if !first.contains("/route") {
                    return;
                }
                calls.fetch_add(1, Ordering::SeqCst);
                let mode = queue.lock().unwrap().pop_front().unwrap_or(Mode::Ok);
                served.lock().unwrap().push(mode);
                let good = |choice: &str, usage: Option<Value>| {
                    let mut v = json!({"answers": {"model": {"choice": choice, "confidence": 0.9}, "effort": {"score": 1.0}}});
                    if let Some(u) = usage {
                        v["usage"] = u;
                    }
                    v.to_string()
                };
                let usage = json!({"input_tokens": 100, "output_tokens": 20});
                let (status, out): (&str, String) = match mode {
                    Mode::Cut => return,
                    Mode::Hang => {
                        std::thread::sleep(Duration::from_millis(900));
                        ("200 OK", good("mock-small", Some(usage)))
                    }
                    Mode::Ok => ("200 OK", good("mock-small", Some(usage))),
                    Mode::Error500 => ("500 Internal Server Error", json!({"error": "boom"}).to_string()),
                    Mode::Error500Billed => ("500 Internal Server Error", json!({"error": "boom", "usage": usage}).to_string()),
                    Mode::Garbage => ("200 OK", "this is not json".into()),
                    Mode::NoUsage => ("200 OK", good("mock-small", None)),
                    Mode::BilledButUnusable => ("200 OK", good("no-such-model", Some(usage))),
                };
                let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    j
}

// -------------------------------------------------------------------- the rig

const PRICE: f64 = 2000.0; // dollars per million tokens, the router's own rate
const KNOWN: f64 = 120.0 * PRICE / 1e6; // what the mock's usage costs
const TIMEOUT: Duration = Duration::from_millis(300);

/// `provider_port` is where a loopback API model "listens": it has to be a live port or the router
/// drops the model as a candidate.
fn config(endpoint: &str, provider_port: u16) -> Config {
    Config::parse(&format!(
        r#"
[router]
backend = "jev"
jev_endpoint = "{endpoint}"
jev_model = "jev-test"
jev_api_key_env = "BS_JEV_TEST_KEY"
jev_usd_per_mtok = {PRICE}
main_min_effort = "high"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:{provider_port}/v1"
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
"#
    ))
    .unwrap()
}

fn cfg(j: &Jev) -> Config {
    config(&format!("http://127.0.0.1:{}/route", j.port), j.port)
}

#[derive(Default)]
struct Books {
    /// Everything recorded as spent, unknown costs included.
    spent: Mutex<f64>,
    /// The part of it that was counted at a hold because the cost was never reported.
    unknown: Mutex<f64>,
    /// How many times each kind of recording ran.
    known_calls: AtomicUsize,
    unknown_calls: AtomicUsize,
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

/// A router wired like the harness's. `slow_unknown` sleeps while an unknown cost is recorded;
/// `started` hears about it the moment that recording begins (the hold is still held then).
fn rig(cfg: Config, table: &Arc<Reservations>, books: &Arc<Books>, limit: f64, slow_unknown: Duration, started: Option<mpsc::Sender<()>>) -> Router {
    let (t, lines) = (table.clone(), project(books, limit));
    let (b1, b2) = (books.clone(), books.clone());
    let started = started.map(Mutex::new);
    Router::new(reqwest::Client::new(), cfg)
        .with_jev_timeout(TIMEOUT)
        .with_meter(move |usd, tokens| t.reserve_fixed("the router", usd, tokens, &lines))
        .with_recorder(move |usd| {
            b1.known_calls.fetch_add(1, Ordering::SeqCst);
            *b1.spent.lock().unwrap() += usd;
        })
        .with_unknown_cost_recorder(move |usd| {
            if let Some(tx) = &started {
                let _ = tx.lock().unwrap().send(());
            }
            std::thread::sleep(slow_unknown);
            b2.unknown_calls.fetch_add(1, Ordering::SeqCst);
            *b2.unknown.lock().unwrap() += usd;
            *b2.spent.lock().unwrap() += usd;
        })
}

fn simple(j: &Jev, limit: f64) -> (Router, Arc<Reservations>, Arc<Books>) {
    let (table, books) = (Arc::new(Reservations::default()), Arc::new(Books::default()));
    (rig(cfg(j), &table, &books, limit, Duration::ZERO, None), table, books)
}

fn rt() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap())
}

fn route(rt: &tokio::runtime::Runtime, r: &Router) -> Decision {
    rt.block_on(r.route(AgentRole::Main, "fix a typo in the readme", "")).unwrap()
}

/// What one routing call holds for this task, learned from a run against a roomy budget.
fn one_reservation(j: &Jev) -> f64 {
    let asked = Arc::new(Mutex::new(0.0));
    let (table, a2) = (Arc::new(Reservations::default()), asked.clone());
    let books = Arc::new(Books::default());
    let lines = project(&books, 1e9);
    let r = Router::new(reqwest::Client::new(), cfg(j)).with_meter(move |usd, tokens| {
        *a2.lock().unwrap() = usd;
        table.reserve_fixed("the router", usd, tokens, &lines)
    });
    route(&rt(), &r);
    let usd = *asked.lock().unwrap();
    assert!(usd > 1.0, "at $2,000 a million, a call holds dollars: {usd}");
    usd
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn heuristic_with_note(d: &Decision, what: &str) {
    assert!(d.source.starts_with("heuristic"), "the heuristic decides when Jev fails: {} / {:?}", d.source, d.note);
    let note = d.note.clone().unwrap_or_default();
    assert!(note.contains(what), "the note says `{what}`: {note}");
}

// ------------------------------------------------- 1. it never left: released

#[test]
fn a_request_that_never_left_releases_its_hold_and_records_nothing() {
    env();
    // Nothing is listening: the connection is refused before anything is sent.
    let closed = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let live = jev();
    for (what, endpoint) in [("a refused connection", format!("http://127.0.0.1:{closed}/route")), ("an endpoint that is not a URL", "not a url".to_string())] {
        let (table, books) = (Arc::new(Reservations::default()), Arc::new(Books::default()));
        let r = rig(config(&endpoint, live.port), &table, &books, 1e9, Duration::ZERO, None);
        let d = route(&rt(), &r);
        heuristic_with_note(&d, "jev");
        assert_eq!(*books.spent.lock().unwrap(), 0.0, "{what}: nothing reached Jev, so nothing was spent");
        assert_eq!(books.unknown_calls.load(Ordering::SeqCst) + books.known_calls.load(Ordering::SeqCst), 0, "{what}: nothing was recorded");
        assert_eq!(table.held("project"), (0.0, 0), "{what}: and the hold is gone");
        assert!(!d.note.clone().unwrap_or_default().contains("cost is unknown"), "{what}: no cost is unknown: {:?}", d.note);
    }
}

// ------------------------------------- 2. sent and failed, cost unknown: counted

fn unknown_case(mode: Mode, what: &str) {
    let j = jev();
    let r_one = one_reservation(&j);
    j.next(&[mode]);
    let (router, table, books) = simple(&j, 1e9);
    let d = route(&rt(), &router);
    heuristic_with_note(&d, "cost is unknown");
    assert!(approx(*books.unknown.lock().unwrap(), r_one), "{what}: the {r_one} held for it is counted as an unknown cost, not freed as zero: {}", books.unknown.lock().unwrap());
    assert!(approx(*books.spent.lock().unwrap(), r_one), "{what}: and it is part of the project's spend: {}", books.spent.lock().unwrap());
    assert_eq!(books.unknown_calls.load(Ordering::SeqCst), 1, "{what}: recorded once");
    assert_eq!(d.router_cost_usd, 0.0, "{what}: the heuristic decision carries no router cost, so the harness adds nothing");
    assert!(!d.cost_settled, "{what}: and there is nothing for it to skip");
    assert_eq!(table.held("project"), (0.0, 0), "{what}: the hold was settled, not leaked");
}

#[test]
fn a_timeout_after_dispatch_is_counted_at_the_reserved_amount() {
    unknown_case(Mode::Hang, "a timeout");
}

#[test]
fn a_connection_cut_after_dispatch_is_counted_at_the_reserved_amount() {
    unknown_case(Mode::Cut, "a cut connection");
}

#[test]
fn an_error_status_without_usage_is_counted_at_the_reserved_amount() {
    unknown_case(Mode::Error500, "an HTTP 500");
}

#[test]
fn a_body_that_is_not_json_is_counted_at_the_reserved_amount() {
    unknown_case(Mode::Garbage, "a non-JSON body");
}

#[test]
fn a_usable_answer_with_no_usage_is_used_and_counted_at_the_reserved_amount() {
    // Nothing failed, so Jev's pick stands; what it cost was never said, so the hold is its cost.
    let j = jev();
    let r_one = one_reservation(&j);
    j.next(&[Mode::NoUsage]);
    let (router, table, books) = simple(&j, 1e9);
    let d = route(&rt(), &router);
    assert_eq!(d.source, "jev", "{:?}", d.note);
    assert!(d.cost_settled && approx(d.router_cost_usd, r_one), "the decision shows the cost that was counted: {} settled={}", d.router_cost_usd, d.cost_settled);
    assert!(d.note.clone().unwrap_or_default().contains("cost is unknown"), "{:?}", d.note);
    assert!(approx(*books.unknown.lock().unwrap(), r_one) && approx(*books.spent.lock().unwrap(), r_one));
    assert_eq!(books.unknown_calls.load(Ordering::SeqCst), 1);
    assert_eq!(table.held("project"), (0.0, 0));
}

// ---------------------------------------- 3. failed but it said what it cost

#[test]
fn a_failure_that_reports_its_cost_is_recorded_at_that_cost() {
    for (mode, what) in [(Mode::Error500Billed, "an error status that reports usage"), (Mode::BilledButUnusable, "an answer that cannot be used")] {
        let j = jev();
        j.next(&[mode]);
        let (router, table, books) = simple(&j, 1e9);
        let d = route(&rt(), &router);
        assert!(d.source.starts_with("heuristic"), "{what}: {}", d.source);
        assert!(approx(*books.spent.lock().unwrap(), KNOWN), "{what}: recorded at what Jev said it cost, {KNOWN}: {}", books.spent.lock().unwrap());
        assert_eq!(*books.unknown.lock().unwrap(), 0.0, "{what}: it is not an unknown cost");
        assert_eq!(books.known_calls.load(Ordering::SeqCst), 1);
        assert_eq!(table.held("project"), (0.0, 0));
    }
}

// ---------------------------------------- 4. nobody else gets the retained room

#[test]
fn a_later_request_cannot_use_the_room_an_unknown_cost_call_held() {
    let j = jev();
    let r_one = one_reservation(&j);
    j.next(&[Mode::Cut]);
    // Room for one call's worst case and a half: a second one fits only if the first was freed.
    let (router, table, books) = simple(&j, r_one * 1.5);
    route(&rt(), &router);
    let lines = project(&books, r_one * 1.5);
    let second = table.reserve_fixed("a second routing call", r_one, 1000, &lines);
    assert!(second.is_err(), "the first call's {r_one} is still counted: a second one must not fit in what is left");
    // The room that really is left (half a call) is still usable.
    assert!(table.reserve_fixed("something small", r_one * 0.4, 100, &lines).is_ok());
}

#[test]
fn a_retry_neither_frees_nor_double_counts_the_failed_attempt() {
    let j = jev();
    let r_one = one_reservation(&j);
    j.next(&[Mode::Hang, Mode::Ok]);
    let (router, table, books) = simple(&j, 1e9);
    let rt = rt();
    let first = route(&rt, &router);
    assert!(first.source.starts_with("heuristic"));
    let after_first = *books.spent.lock().unwrap();
    assert!(approx(after_first, r_one), "{after_first}");
    let second = route(&rt, &router);
    assert_eq!(second.source, "jev", "the retry went through: {:?}", second.note);
    assert!(second.cost_settled);
    let spent = *books.spent.lock().unwrap();
    assert!(approx(spent, r_one + KNOWN), "the failed attempt once at its hold, the retry once at its cost: {spent} vs {}", r_one + KNOWN);
    assert!(approx(*books.unknown.lock().unwrap(), r_one), "only the failed attempt is an unknown cost");
    assert_eq!(table.held("project"), (0.0, 0));
}

// -------------------------------- 5. while the unknown cost is being recorded

#[test]
fn nobody_sees_an_unknown_cost_call_with_no_hold_and_no_spend() {
    let j = jev();
    let r_one = one_reservation(&j);
    j.next(&[Mode::Cut]);
    let limit = r_one * 1.5;
    let (table, books) = (Arc::new(Reservations::default()), Arc::new(Books::default()));
    let (tx, rx) = mpsc::channel();
    // Recording takes a while, as it can when the state is busy.
    let router = rig(cfg(&j), &table, &books, limit, Duration::from_millis(300), Some(tx));
    let rt = rt();
    let call = std::thread::spawn(move || route(&rt, &router));

    // The moment recording starts, an API call tries to take a call's worth of the same budget.
    let seen = Arc::new(Mutex::new(None::<f64>));
    let racer = {
        let (table, books, seen) = (table.clone(), books.clone(), seen.clone());
        std::thread::spawn(move || {
            rx.recv_timeout(Duration::from_secs(10)).expect("the router never recorded the unknown cost while it still held the reservation");
            let lines = project(&books, limit);
            let (seen2, books2) = (seen.clone(), books.clone());
            table.reserve_fixed("an api call", r_one, 1000, move || {
                // Read inside the table's lock, as the harness reads its state.
                *seen2.lock().unwrap() = Some(*books2.spent.lock().unwrap());
                lines()
            })
        })
    };
    call.join().unwrap();
    let got = racer.join().unwrap();
    let at_check = seen.lock().unwrap().expect("the racer checked the budget");
    assert!(approx(at_check, r_one), "the racer checked while the spend was {at_check}: it must see the recorded {r_one}, never the gap (hold gone, spend 0)");
    assert!(got.is_err(), "with the unknown cost on the books only half a call is left, so a whole one must be refused");
    assert_eq!(table.held("project"), (0.0, 0));
}

// -------------------------------------------------------- 6. a mixture, at once

#[test]
fn a_mixture_of_outcomes_run_concurrently_adds_up_and_never_passes_the_limit() {
    let j = jev();
    let r_one = one_reservation(&j);
    let learned = j.calls.load(Ordering::SeqCst);
    j.served.lock().unwrap().clear();
    // The first eight requests the mock sees, whichever router thread they come from.
    j.next(&[Mode::Ok, Mode::Hang, Mode::Cut, Mode::Ok, Mode::Error500, Mode::Garbage, Mode::NoUsage, Mode::BilledButUnusable]);
    let limit = r_one * 6.2;
    let (table, books) = (Arc::new(Reservations::default()), Arc::new(Books::default()));
    let router = Arc::new(rig(cfg(&j), &table, &books, limit, Duration::from_millis(10), None));
    let rt = rt();
    let admitted_api = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];
    for i in 0..8u64 {
        let (router, rt) = (router.clone(), rt.clone());
        handles.push(std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(i * 40));
            route(&rt, &router);
        }));
        let (table, books, admitted) = (table.clone(), books.clone(), admitted_api.clone());
        handles.push(std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(i * 40 + 20));
            if let Ok(res) = table.reserve_fixed("an api call", r_one, 1000, project(&books, limit)) {
                admitted.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(50));
                let books = books.clone();
                // An API call's real cost is what it reserved.
                res.settle(move || {
                    std::thread::sleep(Duration::from_millis(10));
                    *books.spent.lock().unwrap() += r_one;
                });
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let spent = *books.spent.lock().unwrap();
    assert!(spent <= limit + 1e-9, "spent {spent} of a {limit} limit: a call was let in while another's cost was not yet counted");
    // What each served request was counted as, from what the mock actually did.
    let served = j.served.lock().unwrap().clone();
    let known = served.iter().filter(|m| matches!(m, Mode::Ok | Mode::Error500Billed | Mode::BilledButUnusable)).count();
    let unknown = served.len() - known;
    assert!(known >= 1 && unknown >= 1, "the mixture included both: {served:?}");
    assert!(admitted_api.load(Ordering::SeqCst) >= 1, "API calls took part");
    let expected = known as f64 * KNOWN + unknown as f64 * r_one + admitted_api.load(Ordering::SeqCst) as f64 * r_one;
    assert!(approx(spent, expected), "recorded {spent}; {known} known and {unknown} unknown Jev calls and {} API calls cost {expected} ({served:?}, {learned} sizing)", admitted_api.load(Ordering::SeqCst));
    assert!(approx(*books.unknown.lock().unwrap(), unknown as f64 * r_one), "{} unknown", books.unknown.lock().unwrap());
    assert_eq!(table.held("project"), (0.0, 0), "nothing is left held");
}

// ---------------------------------------------- 7. known cost above the hold

#[test]
fn a_known_cost_above_the_hold_is_recorded_in_full_when_the_call_failed_too() {
    let j = jev();
    let r_one = one_reservation(&j);
    // The server billed ten times what was held and then failed: the whole of it is recorded.
    let big = (r_one / PRICE * 1e6 * 10.0) as u64;
    let rate_cost = big as f64 * PRICE / 1e6;
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let mut stream = stream.unwrap();
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
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let out = json!({"usage": {"input_tokens": big, "output_tokens": 0}}).to_string();
                let _ = write!(stream, "HTTP/1.1 503 Service Unavailable\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    let (table, books) = (Arc::new(Reservations::default()), Arc::new(Books::default()));
    let r = rig(config(&format!("http://127.0.0.1:{port}/route"), port), &table, &books, 1e9, Duration::ZERO, None);
    let d = route(&rt(), &r);
    assert!(d.source.starts_with("heuristic"));
    assert!(approx(*books.spent.lock().unwrap(), rate_cost), "{} vs {rate_cost}", books.spent.lock().unwrap());
    assert!(rate_cost > r_one * 9.0);
    assert_eq!(*books.unknown.lock().unwrap(), 0.0);
    assert_eq!(table.held("project"), (0.0, 0));
}

// -------------------------------------------------- 8. the heuristic still works

#[test]
fn the_heuristic_still_decides_whatever_happens_to_the_jev_call() {
    for mode in [Mode::Hang, Mode::Cut, Mode::Error500, Mode::Garbage, Mode::BilledButUnusable, Mode::Error500Billed] {
        let j = jev();
        j.next(&[mode]);
        let (router, _, _) = simple(&j, 1e9);
        let d = route(&rt(), &router);
        assert!(d.source.starts_with("heuristic"), "{mode:?}: {} / {:?}", d.source, d.note);
        assert!(["mock-small", "mock-big"].contains(&d.model.as_str()), "{mode:?}: it picked one of the offered models: {}", d.model);
    }
    // A call refused by the budget never reaches Jev at all, and the heuristic decides too.
    let j = jev();
    let (table, books) = (Arc::new(Reservations::default()), Arc::new(Books::default()));
    let r = rig(cfg(&j), &table, &books, 0.0001, Duration::ZERO, None);
    let d = route(&rt(), &r);
    assert!(d.source.starts_with("heuristic") && d.note.clone().unwrap_or_default().contains("refused by project policy"), "{:?}", d.note);
    assert_eq!(j.calls.load(Ordering::SeqCst), 0, "not one request was sent");
    assert_eq!(*books.spent.lock().unwrap(), 0.0);
}

// ------------------------------------------------ 9. in a real project's books

fn project_text(port: u16) -> String {
    format!(
        r#"
[router]
backend = "jev"
jev_endpoint = "http://127.0.0.1:{port}/route"
jev_model = "jev-test"
jev_api_key_env = "BS_JEV_TEST_KEY"
jev_usd_per_mtok = {PRICE}
main_min_effort = "high"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10
max_project_usd = 500.0

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
"#
    )
}

fn routed_text(h: &Harness) -> String {
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

fn open_project(tag: &str, j: &Jev) -> Harness {
    let ws = std::env::temp_dir().join(format!("bs-jevunk-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("backspace.toml"), project_text(j.port)).unwrap();
    Harness::open(ws).unwrap()
}

#[test]
fn a_project_counts_an_unknown_cost_in_its_spend_once_and_marks_it() {
    let j = jev();
    let r_one = one_reservation(&j);
    j.next(&[Mode::Cut]);
    let h = open_project("unknown", &j);
    h.send("fix a typo in the readme");
    let text = routed_text(&h);
    assert!(text.contains("routed by heuristic") && text.contains("cost is unknown"), "the person is told: {text}");
    let s = h.snapshot();
    assert!(approx(s.router_unknown_cost_usd, r_one), "unknown routing cost {} vs the {r_one} held", s.router_unknown_cost_usd);
    assert!(approx(s.router_cost_usd, r_one), "it is part of the router's spend: {}", s.router_cost_usd);
    assert!(approx(s.total_cost_usd, r_one), "and of the project's, once: {}", s.total_cost_usd);
    std::thread::sleep(Duration::from_millis(300));
    let again = h.snapshot();
    assert_eq!((again.router_cost_usd, again.total_cost_usd, again.router_unknown_cost_usd), (s.router_cost_usd, s.total_cost_usd, s.router_unknown_cost_usd), "and nothing adds it a second time");
}

#[test]
fn a_project_that_got_a_failure_before_dispatch_has_nothing_to_count() {
    let j = jev();
    let closed = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let ws = std::env::temp_dir().join(format!("bs-jevunk-never-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("backspace.toml"), project_text(j.port).replace(&format!("127.0.0.1:{}/route", j.port), &format!("127.0.0.1:{closed}/route"))).unwrap();
    let h = Harness::open(ws).unwrap();
    h.send("fix a typo in the readme");
    let text = routed_text(&h);
    assert!(text.contains("routed by heuristic") && !text.contains("cost is unknown"), "{text}");
    let s = h.snapshot();
    assert_eq!((s.router_cost_usd, s.router_unknown_cost_usd, s.total_cost_usd), (0.0, 0.0, 0.0));
}
