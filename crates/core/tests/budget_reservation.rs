//! M1 hardening, F2: the budget is held for a call's worst case from the
//! moment it is allowed until it finishes, so concurrent agents cannot all
//! spend the same dollar, and a call that is dropped gives its hold back.

use std::sync::{Arc, Barrier};

use backspace_core::config::ModelSpec;
use backspace_core::gateway::{BudgetLine, Gateway, PolicyConfig, Reservations, Role, ScopedLine};

fn model(input: f64, output: f64, max_out: u32) -> ModelSpec {
    ModelSpec {
        id: "m".into(),
        provider: "p".into(),
        model: None,
        tier: 1,
        description: String::new(),
        input_usd_per_mtok: input,
        output_usd_per_mtok: output,
        max_output_tokens: max_out,
        efforts: vec![],
        class: None,
    }
}

fn scope(key: &str, usd: Option<f64>, tokens: Option<u64>) -> ScopedLine {
    ScopedLine {
        key: key.into(),
        line: BudgetLine { label: key.into(), limit_usd: usd, limit_tokens: tokens, spent_usd: 0.0, spent_tokens: 0 },
    }
}

fn gw() -> Gateway {
    Gateway::new(PolicyConfig::default())
}

fn table() -> Arc<Reservations> {
    Arc::new(Reservations::default())
}

/// `n` agents try to start a call at the same instant and keep it in flight.
fn stampede(n: usize, m: ModelSpec, est: u64, lines: impl Fn() -> Vec<ScopedLine> + Send + Sync + 'static) -> (Arc<Reservations>, Vec<bool>) {
    let (t, m, lines) = (table(), Arc::new(m), Arc::new(lines));
    let gate = Arc::new(Barrier::new(n));
    let handles: Vec<_> = (0..n)
        .map(|_| {
            let (t, m, lines, gate) = (t.clone(), m.clone(), lines.clone(), gate.clone());
            std::thread::spawn(move || {
                gate.wait();
                t.reserve(&gw(), Role::Execution, &m, est, 1.0, || lines()).map(|r| {
                    // Held until every thread has tried: the calls are all in flight at once.
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    r
                })
            })
        })
        .collect();
    let ok: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap().is_ok()).collect();
    (t, ok)
}

#[test]
fn parallel_calls_cannot_jointly_exceed_a_dollar_budget() {
    // Each call: 1,000 prompt tokens ($0.01) + 10,000 output tokens ($0.20) = $0.21 worst case.
    let m = model(10.0, 20.0, 10_000);
    let (t, ok) = stampede(16, m, 1_000, || vec![scope("project", Some(1.0), None)]);
    let admitted = ok.iter().filter(|b| **b).count();
    // Four full calls ($0.84), a fifth clamped to the last $0.16, and nobody after that.
    assert_eq!(admitted, 5, "{ok:?}");
    // Every guard is dropped by now, so nothing is left held.
    assert_eq!(t.held("project"), (0.0, 0));
}

#[test]
fn parallel_calls_cannot_jointly_exceed_a_token_budget() {
    // A free model: only the token limit applies. 10,000 + 20,000 = 30,000 per call, 100,000 in all.
    let m = model(0.0, 0.0, 20_000);
    let (_, ok) = stampede(16, m, 10_000, || vec![scope("task:1", None, Some(100_000))]);
    // Three full calls (90,000); the fourth would have only 10,000 left, which is the prompt alone.
    assert_eq!(ok.iter().filter(|b| **b).count(), 3, "{ok:?}");
}

#[test]
fn what_is_held_never_passes_the_limit_while_calls_are_in_flight() {
    let m = model(10.0, 20.0, 10_000);
    let t = table();
    let mut guards = vec![];
    for _ in 0..8 {
        if let Ok(r) = t.reserve(&gw(), Role::Execution, &m, 1_000, 1.0, || vec![scope("project", Some(1.0), None)]) {
            guards.push(r);
        }
        assert!(t.held("project").0 <= 1.0 + 1e-9, "held {:?}", t.held("project"));
    }
    assert_eq!(guards.len(), 5);
    let held: f64 = guards.iter().map(|g| g.reserved_usd()).sum();
    assert!((held - t.held("project").0).abs() < 1e-9);
}

#[test]
fn a_stopped_call_releases_its_reservation() {
    let m = model(10.0, 20.0, 10_000);
    let t = table();
    let lines = || vec![scope("project", Some(0.25), None)];
    let first = t.reserve(&gw(), Role::Execution, &m, 1_000, 1.0, lines).unwrap();
    assert!(first.reserved_usd() > 0.2 && t.held("project").0 > 0.2);
    assert!(t.reserve(&gw(), Role::Execution, &m, 1_000, 1.0, lines).is_err(), "no room while the first is in flight");
    drop(first); // the call failed, or the user stopped the agent
    assert_eq!(t.held("project"), (0.0, 0));
    assert!(t.reserve(&gw(), Role::Execution, &m, 1_000, 1.0, lines).is_ok(), "the room is back");
}

#[test]
fn every_scope_a_call_belongs_to_is_held_and_others_are_not() {
    let m = model(10.0, 20.0, 10_000);
    let t = table();
    let lines = |task: &'static str| move || vec![scope(task, Some(0.3), None), scope("project", Some(10.0), None)];
    let a1 = t.reserve(&gw(), Role::Execution, &m, 1_000, 1.0, lines("task:1")).unwrap();
    assert_eq!(a1.grant.max_output_tokens, 10_000, "$0.30 covers a $0.21 call in full");
    // The same task again: only $0.09 is unheld, $0.01 of it the prompt, so 4,000 output tokens.
    let a2 = t.reserve(&gw(), Role::Execution, &m, 1_000, 1.0, lines("task:1")).unwrap();
    assert!((3_990..=4_000).contains(&a2.grant.max_output_tokens), "{}", a2.grant.max_output_tokens);
    // A different task has its own $0.30.
    let b1 = t.reserve(&gw(), Role::Execution, &m, 1_000, 1.0, lines("task:2")).unwrap();
    assert_eq!(b1.grant.max_output_tokens, 10_000);
    assert!((t.held("task:1").0 - 0.30).abs() < 1e-3, "{:?}", t.held("task:1"));
    assert!((t.held("task:2").0 - 0.21).abs() < 1e-9);
    assert!((t.held("project").0 - 0.51).abs() < 1e-3);
    drop(a2);
    assert!((t.held("task:1").0 - 0.21).abs() < 1e-9);
    assert!((t.held("project").0 - 0.42).abs() < 1e-9);
}

#[test]
fn a_reservation_is_the_worst_case_with_input_priced_at_the_factor() {
    let m = model(10.0, 20.0, 10_000);
    let t = table();
    let r = t.reserve(&gw(), Role::Execution, &m, 100_000, 1.25, || vec![scope("project", Some(100.0), None)]).unwrap();
    // 100,000 prompt tokens at $10 x 1.25 = $1.25, plus 10,000 output tokens at $20 = $0.20.
    assert!((r.reserved_usd() - 1.45).abs() < 1e-9, "{}", r.reserved_usd());
    assert_eq!(r.reserved_tokens(), 110_000, "token budgets count tokens, not their price");
}

#[test]
fn a_call_that_cost_more_than_its_reservation_is_reported_with_by_how_much() {
    let m = model(10.0, 20.0, 10_000);
    let t = table();
    let r = t.reserve(&gw(), Role::Execution, &m, 1_000, 1.0, || vec![scope("project", Some(100.0), None)]).unwrap();
    assert_eq!(r.overshoot(0.2, 5_000), None, "under the reservation");
    assert_eq!(r.overshoot(0.21, 11_000), None, "exactly the reservation");
    let o = r.overshoot(0.71, 15_000).expect("over");
    assert!((o.usd - 0.5).abs() < 1e-9, "{o:?}");
    assert_eq!(o.tokens, 4_000);
}

#[test]
fn a_fixed_reservation_for_the_advisor_fits_or_is_refused_and_is_released() {
    let m = model(10.0, 20.0, 10_000);
    let t = table();
    let lines = || vec![scope("project", Some(0.5), None)];
    let main = t.reserve(&gw(), Role::Execution, &m, 1_000, 1.0, lines).unwrap(); // $0.21
    let adv = t.reserve_fixed("advisor", 0.25, 4_096, lines).expect("$0.46 of $0.50");
    assert!(t.reserve_fixed("advisor", 0.10, 100, lines).is_err(), "$0.56 of $0.50");
    assert!((t.held("project").0 - 0.46).abs() < 1e-9);
    drop(adv);
    assert!((t.held("project").0 - 0.21).abs() < 1e-9);
    drop(main);
    assert_eq!(t.held("project"), (0.0, 0));
}
