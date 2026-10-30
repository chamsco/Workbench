//! M2 acceptance: the cost ladder. What counts as a frontier model, where the
//! planner starts, that moving up takes measured evidence, and that a cheap
//! rung that costs more to get green than the frontier is skipped.

use backspace_core::config::{Config, ModelSpec, DEFAULT_CONFIG};
use backspace_core::decision::{TaskType, Triage};
use backspace_core::ladder::*;

fn default_cfg() -> Config {
    Config::parse(DEFAULT_CONFIG).unwrap()
}

fn class_in_default(id: &str) -> ModelClass {
    let cfg = default_cfg();
    class_of(cfg.model(id).unwrap_or_else(|| panic!("no model {id}")))
}

#[test]
fn by_default_the_class_follows_the_tier() {
    assert_eq!(class_in_default("claude-haiku-4-5"), ModelClass::Mid);
    assert_eq!(class_in_default("claude-sonnet-5-5"), ModelClass::Mid);
    assert_eq!(class_in_default("claude-opus-5-5"), ModelClass::Frontier);
    assert_eq!(class_in_default("claude-fable-5-1"), ModelClass::Frontier);
}

#[test]
fn a_free_model_can_still_be_a_frontier_call() {
    // The coding CLIs are billed to a subscription (price 0) but are tier 3.
    assert_eq!(class_in_default("claude-code"), ModelClass::Frontier);
    assert_eq!(class_in_default("codex"), ModelClass::Frontier);
}

#[test]
fn tier_zero_is_local() {
    let cfg = default_cfg();
    let mut m: ModelSpec = cfg.model("claude-haiku-4-5").unwrap().clone();
    m.tier = 0;
    m.class = None;
    assert_eq!(class_of(&m), ModelClass::Local);
}

#[test]
fn an_explicit_class_beats_the_tier() {
    let cfg = Config::parse(&format!(
        "{DEFAULT_CONFIG}\n[[models]]\nid = \"big-local\"\nprovider = \"anthropic\"\ntier = 4\nclass = \"local\"\ndescription = \"x\"\ninput_usd_per_mtok = 0.0\noutput_usd_per_mtok = 0.0\nmax_output_tokens = 1000\n\n[[models]]\nid = \"small-cloud\"\nprovider = \"anthropic\"\ntier = 1\nclass = \"frontier\"\ndescription = \"x\"\ninput_usd_per_mtok = 0.0\noutput_usd_per_mtok = 0.0\nmax_output_tokens = 1000\n"
    ))
    .unwrap();
    assert_eq!(class_of(cfg.model("big-local").unwrap()), ModelClass::Local);
    assert_eq!(class_of(cfg.model("small-cloud").unwrap()), ModelClass::Frontier);
}

#[test]
fn an_unknown_class_is_a_config_error() {
    let src = format!(
        "{DEFAULT_CONFIG}\n[[models]]\nid = \"odd\"\nprovider = \"anthropic\"\ntier = 1\nclass = \"huge\"\ndescription = \"x\"\ninput_usd_per_mtok = 0.0\noutput_usd_per_mtok = 0.0\nmax_output_tokens = 1000\n"
    );
    assert!(Config::parse(&src).is_err());
}

#[test]
fn classes_are_ordered_from_local_to_frontier() {
    assert!(ModelClass::Local < ModelClass::Mid && ModelClass::Mid < ModelClass::Frontier);
    assert_eq!(ModelClass::Frontier.as_str(), "frontier");
}

#[test]
fn the_planner_starts_on_the_frontier_only_for_a_long_task() {
    let sure = |task| Triage::Confident { task, p: 0.95 };
    assert_eq!(planner_start(Some(&sure(TaskType::Long))), ModelClass::Frontier);
    assert_eq!(planner_start(Some(&sure(TaskType::OneShot))), ModelClass::Mid);
    assert_eq!(planner_start(Some(&sure(TaskType::Research))), ModelClass::Mid);
}

#[test]
fn an_unsure_or_missing_classification_does_not_start_on_the_frontier() {
    // Unsure means ask the user; it does not mean spend on the dearest model.
    assert_eq!(planner_start(Some(&Triage::Ask { best: TaskType::Long, p: 0.4 })), ModelClass::Mid);
    assert_eq!(planner_start(None), ModelClass::Mid);
}

#[test]
fn moving_up_needs_a_measured_failure() {
    assert!(Failure::new(FailureKind::TestFailed, "").is_err());
    assert!(Failure::new(FailureKind::TestFailed, "   ").is_err());
    let f = Failure::new(FailureKind::TestFailed, "parser::empty_input panicked").unwrap();
    assert_eq!(escalate(ModelClass::Local, &f), Some(ModelClass::Mid));
    assert_eq!(escalate(ModelClass::Mid, &f), Some(ModelClass::Frontier));
}

#[test]
fn nothing_is_above_the_frontier() {
    let f = Failure::new(FailureKind::TurnBudget, "ran out of turns after 40 steps").unwrap();
    assert_eq!(escalate(ModelClass::Frontier, &f), None);
}

#[test]
fn an_unmeasured_rung_is_a_coin_flip_and_no_rung_is_certain() {
    let none = RungStats { attempts: 0, successes: 0, mean_cost_usd: 0.1 };
    assert_eq!(none.p_success(), 0.5);
    let always = RungStats { attempts: 10, successes: 10, mean_cost_usd: 0.1 };
    assert!(always.p_success() < 1.0 && always.p_success() > 0.9);
    let never = RungStats { attempts: 10, successes: 0, mean_cost_usd: 0.1 };
    assert!(never.p_success() > 0.0 && never.p_success() < 0.1);
    // 11/12 successes, $0.12 a try.
    let s = RungStats { attempts: 10, successes: 10, mean_cost_usd: 0.11 };
    assert!((s.expected_cost_to_green() - 0.11 / (11.0 / 12.0)).abs() < 1e-12);
}

#[test]
fn a_cheap_rung_that_keeps_failing_is_skipped_for_the_frontier() {
    // $0.02 a try but 1 success in 10: 2/12 -> $0.12 to get one success.
    let cheap = RungStats { attempts: 10, successes: 1, mean_cost_usd: 0.02 };
    // $0.10 a try, 5 of 5: 6/7 -> about $0.117.
    let frontier = RungStats { attempts: 5, successes: 5, mean_cost_usd: 0.10 };
    assert!(cheap.expected_cost_to_green() > frontier.expected_cost_to_green());
    assert!(escalate_sooner(&cheap, &frontier));
}

#[test]
fn a_cheap_rung_that_mostly_works_is_kept() {
    let cheap = RungStats { attempts: 10, successes: 8, mean_cost_usd: 0.02 };
    let frontier = RungStats { attempts: 5, successes: 5, mean_cost_usd: 0.10 };
    assert!(!escalate_sooner(&cheap, &frontier));
}

#[test]
fn without_measurements_price_decides_and_the_cheap_rung_goes_first() {
    let cheap = RungStats { attempts: 0, successes: 0, mean_cost_usd: 0.02 };
    let frontier = RungStats { attempts: 0, successes: 0, mean_cost_usd: 0.10 };
    assert!(!escalate_sooner(&cheap, &frontier));
}

fn spec(id: &str, tier: u8, out: f64) -> ModelSpec {
    let mut m: ModelSpec = default_cfg().model("claude-haiku-4-5").unwrap().clone();
    m.id = id.into();
    m.tier = tier;
    m.class = None;
    m.input_usd_per_mtok = out / 5.0;
    m.output_usd_per_mtok = out;
    m
}

#[test]
fn within_caps_the_candidates_and_never_empties_them() {
    let (l, m1, m2, f) = (spec("l", 0, 0.0), spec("m1", 1, 5.0), spec("m2", 2, 10.0), spec("f", 4, 50.0));
    let all = [&l, &m1, &m2, &f];
    let ids = |v: Vec<&ModelSpec>| v.iter().map(|m| m.id.as_str()).collect::<Vec<_>>().join(",");
    assert_eq!(ids(within(&all, ModelClass::Mid)), "l,m1,m2");
    assert_eq!(ids(within(&all, ModelClass::Frontier)), "l,m1,m2,f");
    assert_eq!(ids(within(&all, ModelClass::Local)), "l");
    // Only strong models configured: a Mid ceiling still routes.
    let only = [&f];
    assert_eq!(ids(within(&only, ModelClass::Mid)), "f");
}

#[test]
fn cheapest_on_prefers_the_class_then_the_nearest_below_then_above() {
    let (m1, m2, f1, f2) = (spec("m1", 1, 5.0), spec("m2", 2, 10.0), spec("f1", 3, 20.0), spec("f2", 4, 50.0));
    let all = [&m1, &m2, &f1, &f2];
    assert_eq!(cheapest_on(&all, ModelClass::Mid).unwrap().id, "m1");
    assert_eq!(cheapest_on(&all, ModelClass::Frontier).unwrap().id, "f1");
    // No local model: the nearest class above is the answer when nothing is below.
    assert_eq!(cheapest_on(&all, ModelClass::Local).unwrap().id, "m1");
    // No frontier model: the nearest class below.
    let mid_only = [&m1, &m2];
    assert_eq!(cheapest_on(&mid_only, ModelClass::Frontier).unwrap().id, "m1");
    assert!(cheapest_on(&[], ModelClass::Mid).is_none());
}
