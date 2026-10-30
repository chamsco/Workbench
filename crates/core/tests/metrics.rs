//! M2 acceptance: the PLAN §3.4 metrics, computed from the ledger.

use backspace_core::ladder::ModelClass;
use backspace_core::metrics::*;

fn call(class: ModelClass, input: u64, output: u64, usd: f64) -> CallRecord {
    CallRecord { model: format!("{class:?}"), class, input_tokens: input, output_tokens: output, usd }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

#[test]
fn an_empty_ledger_reports_zeros_and_no_ratios() {
    let m = Ledger::default().metrics();
    assert_eq!(m.unit, "ticket");
    assert_eq!((m.calls, m.frontier_calls, m.total_tokens, m.merged, m.tasks), (0, 0, 0, 0, 0));
    assert_eq!(m.total_usd, 0.0);
    for r in [
        m.cost_to_green_usd,
        m.escalation_rate,
        m.escalation_success_rate,
        m.frontier_calls_per_merged,
        m.tokens_per_merged,
        m.usd_per_merged,
        m.human_tokens_per_merged,
    ] {
        assert_eq!(r, None);
    }
}

#[test]
fn frontier_calls_are_counted_by_class_not_by_price() {
    let mut l = Ledger::default();
    l.record_call(call(ModelClass::Frontier, 100, 50, 0.0)); // a subscription CLI: free, still frontier
    l.record_call(call(ModelClass::Mid, 100, 50, 0.01));
    l.record_call(call(ModelClass::Local, 100, 50, 0.0));
    l.record_call(call(ModelClass::Frontier, 100, 50, 0.20));
    let m = l.metrics();
    assert_eq!((m.calls, m.frontier_calls), (4, 2));
    assert_eq!(m.total_tokens, 600);
    assert!(close(m.total_usd, 0.21));
}

#[test]
fn cost_to_green_includes_every_retry_up_to_the_green_result() {
    let mut l = Ledger::default();
    l.task_started("a");
    l.record_call(call(ModelClass::Mid, 0, 0, 0.10)); // first try, fails
    l.record_call(call(ModelClass::Mid, 0, 0, 0.20)); // retry, fails
    l.task_escalated("a");
    assert_eq!(l.metrics().cost_to_green_usd, None, "nothing is green yet");
    l.record_call(call(ModelClass::Frontier, 0, 0, 0.50)); // escalation, passes
    l.task_finished("a", true);
    assert!(close(l.metrics().cost_to_green_usd.unwrap(), 0.80));
    // Work after the green result is not part of getting to green.
    l.record_call(call(ModelClass::Frontier, 0, 0, 1.00));
    let m = l.metrics();
    assert!(close(m.cost_to_green_usd.unwrap(), 0.80));
    assert!(close(m.total_usd, 1.80));
}

#[test]
fn a_task_that_never_goes_green_does_not_make_cost_to_green() {
    let mut l = Ledger::default();
    l.task_started("a");
    l.record_call(call(ModelClass::Mid, 0, 0, 0.30));
    l.task_finished("a", false);
    assert_eq!(l.metrics().cost_to_green_usd, None);
}

#[test]
fn escalation_rate_and_success_rate_count_tasks_not_events() {
    let mut l = Ledger::default();
    for k in ["a", "b", "c"] {
        l.task_started(k);
    }
    l.task_escalated("a");
    l.task_escalated("a"); // climbing twice is still one escalated task
    l.task_escalated("b");
    l.task_finished("a", true);
    l.task_finished("b", false);
    l.task_finished("c", true);
    let m = l.metrics();
    assert_eq!((m.tasks, m.escalated_tasks, m.escalated_green), (3, 2, 1));
    assert!(close(m.escalation_rate.unwrap(), 2.0 / 3.0));
    assert!(close(m.escalation_success_rate.unwrap(), 0.5));
}

#[test]
fn no_escalations_means_no_success_rate_rather_than_zero() {
    let mut l = Ledger::default();
    l.task_started("a");
    l.task_finished("a", true);
    let m = l.metrics();
    assert_eq!(m.escalation_rate, Some(0.0));
    assert_eq!(m.escalation_success_rate, None);
}

#[test]
fn finishing_a_task_that_was_never_started_still_counts_it() {
    let mut l = Ledger::default();
    l.task_finished("x", true);
    assert_eq!(l.metrics().tasks, 1);
}

#[test]
fn human_tokens_are_estimated_from_characters_and_split_into_typed_and_read() {
    let mut l = Ledger::default();
    l.human_typed(10); // ceil(10 / 3) = 4
    l.human_typed(1); // 1
    l.human_read(9); // 3
    let m = l.metrics();
    assert_eq!((m.human_tokens_typed, m.human_tokens_read), (5, 3));
}

#[test]
fn per_merged_figures_divide_by_merged_tickets() {
    let mut l = Ledger::default();
    l.human_typed(30); // 10
    l.human_read(30); // 10
    for k in ["a", "b"] {
        l.task_started(k);
    }
    l.record_call(call(ModelClass::Frontier, 300, 100, 0.40));
    l.record_call(call(ModelClass::Mid, 300, 100, 0.20));
    l.record_call(call(ModelClass::Mid, 300, 100, 0.20));
    l.task_finished("a", true);
    l.task_finished("b", false);
    let m = l.metrics();
    assert_eq!(m.merged, 1, "only the green one merged");
    assert!(close(m.frontier_calls_per_merged.unwrap(), 1.0));
    assert!(close(m.tokens_per_merged.unwrap(), 1200.0));
    assert!(close(m.usd_per_merged.unwrap(), 0.80));
    assert!(close(m.human_tokens_per_merged.unwrap(), 20.0));
}

#[test]
fn the_json_form_carries_every_metric_and_round_trips() {
    let mut l = Ledger::default();
    l.task_started("a");
    l.record_call(call(ModelClass::Frontier, 10, 5, 0.01));
    l.task_finished("a", true);
    let m = l.metrics();
    let v = serde_json::to_value(&m).unwrap();
    for key in [
        "unit",
        "frontier_calls",
        "total_tokens",
        "total_usd",
        "cost_to_green_usd",
        "escalation_rate",
        "escalation_success_rate",
        "human_tokens_typed",
        "human_tokens_read",
        "merged",
        "frontier_calls_per_merged",
        "tokens_per_merged",
        "usd_per_merged",
        "human_tokens_per_merged",
    ] {
        assert!(v.get(key).is_some(), "missing {key}: {v}");
    }
    let back: RunMetrics = serde_json::from_value(v).unwrap();
    assert_eq!(back, m);
}

#[test]
fn decision_model_spend_counts_toward_cost_but_is_not_a_model_call() {
    let mut l = Ledger::default();
    l.record_decision(0.002);
    l.record_call(call(ModelClass::Mid, 10, 5, 0.01));
    let m = l.metrics();
    assert_eq!((m.calls, m.frontier_calls, m.total_tokens), (1, 0, 15));
    assert!(close(m.total_usd, 0.012));
}
