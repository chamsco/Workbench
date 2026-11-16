//! §10 follow-up: the model-class policy. Only tier 4 is frontier by default;
//! tiers 1–3 are mid and tier 0 is local. A per-model `model_class` (or the
//! older `class` key) overrides the tier, and an unknown class is a config
//! error rather than a silent fall back to the tier. Effort and tier stay
//! separate from class: a tier-3 model that takes high effort is still mid.

use backspace_core::config::{Config, ModelSpec, DEFAULT_CONFIG};
use backspace_core::ladder::{cheapest_on, class_of, within, ModelClass};
use backspace_core::metrics::{CallRecord, Ledger};

fn model(id: &str, tier: u8, class_line: &str) -> String {
    format!(
        "\n[[models]]\nid = \"{id}\"\nprovider = \"anthropic\"\ntier = {tier}\n{class_line}\ndescription = \"x\"\ninput_usd_per_mtok = 1.0\noutput_usd_per_mtok = {tier}.0\nmax_output_tokens = 1000\n"
    )
}

fn cfg_with(extra: &str) -> Result<Config, anyhow::Error> {
    Config::parse(&format!("{DEFAULT_CONFIG}{extra}"))
}

fn spec(cfg: &Config, id: &str) -> ModelSpec {
    cfg.model(id).unwrap_or_else(|| panic!("no model {id}")).clone()
}

#[test]
fn the_default_class_follows_the_tier_and_only_tier_4_is_frontier() {
    let table = [
        (0u8, ModelClass::Local),
        (1, ModelClass::Mid),
        (2, ModelClass::Mid),
        (3, ModelClass::Mid),
        (4, ModelClass::Frontier),
    ];
    let extra: String = table.iter().map(|(t, _)| model(&format!("t{t}"), *t, "")).collect();
    let cfg = cfg_with(&extra).unwrap();
    for (tier, want) in table {
        assert_eq!(class_of(&spec(&cfg, &format!("t{tier}"))), want, "tier {tier}");
    }
}

#[test]
fn model_class_overrides_the_tier() {
    let table = [
        ("two-as-frontier", 2u8, "frontier", ModelClass::Frontier),
        ("four-as-mid", 4, "mid", ModelClass::Mid),
        ("zero-as-mid", 0, "mid", ModelClass::Mid),
        ("three-as-frontier", 3, "frontier", ModelClass::Frontier),
        ("four-as-local", 4, "local", ModelClass::Local),
    ];
    let extra: String =
        table.iter().map(|(id, t, c, _)| model(id, *t, &format!("model_class = \"{c}\""))).collect();
    let cfg = cfg_with(&extra).unwrap();
    for (id, _, _, want) in table {
        assert_eq!(class_of(&spec(&cfg, id)), want, "{id}");
    }
}

#[test]
fn the_older_class_key_still_overrides_the_tier() {
    let cfg = cfg_with(&model("legacy", 1, "class = \"frontier\"")).unwrap();
    assert_eq!(class_of(&spec(&cfg, "legacy")), ModelClass::Frontier);
}

#[test]
fn an_invalid_model_class_is_a_config_error_not_a_tier_fallback() {
    for bad in ["huge", "Frontier", "", "tier4"] {
        let r = cfg_with(&model("odd", 4, &format!("model_class = \"{bad}\"")));
        assert!(r.is_err(), "model_class = {bad:?} was accepted");
    }
    assert!(cfg_with(&model("odd", 4, "model_class = 4")).is_err(), "a number is not a class");
}

#[test]
fn setting_both_class_keys_is_a_config_error() {
    let r = cfg_with(&model("both", 2, "class = \"mid\"\nmodel_class = \"frontier\""));
    assert!(r.is_err(), "two class keys on one model must not silently pick one");
}

#[test]
fn shipped_default_models_follow_the_policy() {
    let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
    for m in &cfg.models {
        if m.class.is_none() {
            let want = match m.tier {
                0 => ModelClass::Local,
                1..=3 => ModelClass::Mid,
                _ => ModelClass::Frontier,
            };
            assert_eq!(class_of(m), want, "{} (tier {})", m.id, m.tier);
        }
    }
    // A tier-3 model that accepts high effort is not frontier on that account.
    assert_eq!(class_of(&spec(&cfg, "claude-opus-5-5")), ModelClass::Mid);
    assert_eq!(class_of(&spec(&cfg, "claude-fable-5-1")), ModelClass::Frontier);
}

#[test]
fn selection_and_metrics_use_the_resolved_class() {
    let extra = format!(
        "{}{}{}",
        model("mid3", 3, ""),
        model("front2", 2, "model_class = \"frontier\""),
        model("front4", 4, "")
    );
    let cfg = cfg_with(&extra).unwrap();
    let (mid3, front2, front4) = (spec(&cfg, "mid3"), spec(&cfg, "front2"), spec(&cfg, "front4"));
    let all = [&mid3, &front2, &front4];

    // A mid ceiling keeps the tier-3 model and drops both frontier ones.
    let kept: Vec<_> = within(&all, ModelClass::Mid).iter().map(|m| m.id.clone()).collect();
    assert_eq!(kept, vec!["mid3".to_string()]);

    // The cheapest frontier is the overridden tier-2 model, never the tier-3 one.
    assert_eq!(cheapest_on(&all, ModelClass::Frontier).map(|m| m.id.as_str()), Some("front2"));

    // The frontier-call metric counts by resolved class.
    let mut ledger = Ledger::default();
    for m in all {
        ledger.record_call(CallRecord { model: m.id.clone(), class: class_of(m), input_tokens: 1, output_tokens: 1, usd: 0.0 });
    }
    let r = ledger.metrics();
    assert_eq!((r.calls, r.frontier_calls), (3, 2));
}
