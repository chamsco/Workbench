//! M2 acceptance: the router eval harness (PLAN §10). These tests check the
//! arithmetic on small traces whose answers are worked out by hand; they say
//! nothing about how good any decision model is. That needs labeled traces
//! from real Cauldron runs.

use std::collections::HashMap;

use anyhow::Result;
use backspace_core::config::Config;
use backspace_core::decision::{Answer, DecisionModel, Question, TaskType};
use backspace_core::eval::*;

/// Looks the distribution up by the trace text.
struct Table(HashMap<&'static str, [f64; 3]>);

impl DecisionModel for Table {
    fn name(&self) -> &str {
        "table"
    }
    fn decide(&self, state: &str, questions: &[Question]) -> Result<Vec<Answer>> {
        let p = self.0.get(state).copied().unwrap_or([1.0 / 3.0; 3]);
        Ok(questions
            .iter()
            .map(|q| Answer {
                question: q.id.clone(),
                probs: vec![("one_shot".into(), p[0]), ("long".into(), p[1]), ("research".into(), p[2])],
            })
            .collect())
    }
}

fn table() -> Table {
    Table(HashMap::from([
        ("t1", [0.95, 0.03, 0.02]),
        ("t2", [0.10, 0.85, 0.05]),
        ("t3", [0.10, 0.15, 0.75]),
        ("t4", [0.75, 0.10, 0.15]),
        ("t5", [0.30, 0.55, 0.15]),
        ("t6", [0.45, 0.35, 0.20]),
    ]))
}

fn trace(id: &str, label: TaskType, tokens: Option<(u64, u64)>) -> Trace {
    Trace {
        id: id.into(),
        text: id.into(),
        label,
        input_tokens: tokens.map(|t| t.0),
        output_tokens: tokens.map(|t| t.1),
    }
}

fn six(tokens: Option<(u64, u64)>) -> Vec<Trace> {
    use TaskType::*;
    vec![
        trace("t1", OneShot, tokens),
        trace("t2", Long, tokens),
        trace("t3", Research, tokens),
        trace("t4", Research, tokens), // predicted one_shot, wrong, at 0.75
        trace("t5", Long, tokens),     // right, but only 0.55 sure
        trace("t6", Research, tokens), // predicted one_shot, wrong, at 0.45
    ]
}

fn cfg() -> Config {
    Config::parse(&format!(
        r#"
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"

[orchestrator]
max_parallel_calls = 2
max_subagents = 4
auto_approve = false
bash_timeout_secs = 10

[providers.p]
kind = "openai"
base_url = "http://127.0.0.1:1/v1"
api_key_env = "NONE"

[[models]]
id = "mid-cheap"
provider = "p"
tier = 1
description = "x"
input_usd_per_mtok = 0.4
output_usd_per_mtok = 2.0
max_output_tokens = 1000

[[models]]
id = "mid-strong"
provider = "p"
tier = 2
description = "x"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 5.0
max_output_tokens = 1000

[[models]]
id = "front"
provider = "p"
tier = 4
description = "x"
input_usd_per_mtok = 4.0
output_usd_per_mtok = 20.0
max_output_tokens = 1000
"#
    ))
    .unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn traces_load_from_json_lines_and_blank_lines_are_skipped() {
    let src = r#"
{"id":"a","text":"fix a typo","label":"one_shot"}

{"id":"b","text":"build it","label":"long","input_tokens":900,"output_tokens":400}
"#;
    let t = parse_traces(src).unwrap();
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].label, TaskType::OneShot);
    assert_eq!(t[0].input_tokens, None);
    assert_eq!((t[1].input_tokens, t[1].output_tokens), (Some(900), Some(400)));
}

#[test]
fn an_unlabeled_or_mislabeled_trace_is_an_error_naming_its_line() {
    let missing = parse_traces("{\"id\":\"a\",\"text\":\"x\",\"label\":\"long\"}\n{\"id\":\"b\",\"text\":\"y\"}\n").unwrap_err();
    assert!(format!("{missing:#}").contains("line 2"), "{missing:#}");
    let unknown = parse_traces("{\"id\":\"a\",\"text\":\"x\",\"label\":\"epic\"}\n").unwrap_err();
    let s = format!("{unknown:#}");
    assert!(s.contains("line 1") && s.contains("epic"), "{s}");
    let not_json = parse_traces("hello\n").unwrap_err();
    assert!(format!("{not_json:#}").contains("line 1"));
}

#[test]
fn an_empty_trace_set_cannot_be_evaluated() {
    assert!(evaluate(&table(), &[], 0.7, &cfg()).is_err());
}

#[test]
fn accuracy_is_reported_per_task_type() {
    let r = evaluate(&table(), &six(None), 0.7, &cfg()).unwrap();
    assert_eq!((r.n, r.correct), (6, 4));
    let s = |t| r.per_type[&t].clone();
    assert_eq!(s(TaskType::OneShot), TypeStats { n: 1, correct: 1 });
    assert_eq!(s(TaskType::Long), TypeStats { n: 2, correct: 2 });
    assert_eq!(s(TaskType::Research), TypeStats { n: 3, correct: 1 });
}

#[test]
fn calibration_bins_ece_and_brier_match_the_hand_calculation() {
    let r = evaluate(&table(), &six(None), 0.7, &cfg()).unwrap();
    let bin = |i: usize| r.bins[i].clone();
    assert_eq!(r.bins.len(), 10);
    assert_eq!((bin(9).n, bin(8).n, bin(7).n, bin(5).n, bin(4).n), (1, 1, 2, 1, 1));
    assert!(close(bin(7).mean_confidence, 0.75) && close(bin(7).accuracy, 0.5));
    assert!(close(bin(9).accuracy, 1.0) && close(bin(4).accuracy, 0.0));
    // (0.05 + 0.15 + 2 * 0.25 + 0.45 + 0.45) / 6
    assert!(close(r.ece, 1.6 / 6.0), "{}", r.ece);
    // (0.0038 + 0.035 + 0.095 + 1.295 + 0.315 + 0.965) / 6
    assert!(close(r.brier, 2.7088 / 6.0), "{}", r.brier);
}

#[test]
fn below_the_threshold_is_counted_as_asking_and_confident_mistakes_are_counted_apart() {
    let r = evaluate(&table(), &six(None), 0.7, &cfg()).unwrap();
    assert_eq!(r.below_threshold, 2, "t5 and t6 would be asked about");
    assert_eq!(r.wrong_below_threshold, 1, "asking catches t6");
    assert_eq!(r.confident_wrong, 1, "t4 is wrong and sure of itself");
    // Raising the bar to 0.9 asks about everything but t1.
    let strict = evaluate(&table(), &six(None), 0.9, &cfg()).unwrap();
    assert_eq!(strict.below_threshold, 5);
    assert_eq!(strict.confident_wrong, 0);
}

#[test]
fn cost_is_compared_with_always_frontier_when_traces_carry_token_counts() {
    let r = evaluate(&table(), &six(Some((1000, 500))), 0.7, &cfg()).unwrap();
    let c = r.cost.expect("tokens were given");
    assert_eq!(c.priced, 6);
    // One confident long task on the frontier ($0.014), five on the cheapest
    // mid model ($0.0014): the unsure ones start mid and are asked about.
    assert!(close(c.routed_usd, 5.0 * 0.0014 + 0.014), "{}", c.routed_usd);
    assert!(close(c.always_frontier_usd, 6.0 * 0.014), "{}", c.always_frontier_usd);
}

#[test]
fn without_token_counts_cost_is_not_computed_rather_than_guessed() {
    let r = evaluate(&table(), &six(None), 0.7, &cfg()).unwrap();
    assert!(r.cost.is_none());
    assert!(r.to_markdown().contains("not computed"));
}

#[test]
fn the_report_renders_every_section_the_plan_asks_for() {
    let md = evaluate(&table(), &six(Some((1000, 500))), 0.7, &cfg()).unwrap().to_markdown();
    for needle in [
        "## Accuracy per type",
        "## Calibration",
        "## Cost versus always-frontier",
        "## Escalation below the threshold",
        "table",
        "6 traces",
        "one_shot",
        "research",
        "1/3",
        "Expected calibration error",
    ] {
        assert!(md.contains(needle), "missing {needle:?} in:\n{md}");
    }
}

#[test]
fn a_decider_that_fails_on_a_trace_fails_the_eval() {
    struct Down;
    impl DecisionModel for Down {
        fn name(&self) -> &str {
            "down"
        }
        fn decide(&self, _s: &str, _q: &[Question]) -> Result<Vec<Answer>> {
            Err(anyhow::anyhow!("down"))
        }
    }
    assert!(evaluate(&Down, &six(None), 0.7, &cfg()).is_err());
}
