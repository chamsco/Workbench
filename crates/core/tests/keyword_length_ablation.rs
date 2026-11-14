//! §10 follow-up: the keyword baseline adds to `long` for texts over 400 and
//! 1500 characters. `KeywordDeciderNoLength` is that same baseline with only
//! the length bonus removed, so an eval can show how much of a result is the
//! length rule. It is an evaluation tool: the router keeps using
//! `KeywordDecider`, whose behaviour must not change.

use backspace_core::decision::{task_distribution, KeywordDecider, KeywordDeciderNoLength};

fn p(m: &dyn backspace_core::decision::DecisionModel, text: &str) -> Vec<(String, f64)> {
    task_distribution(m, text).unwrap().into_iter().map(|(t, p)| (t.as_str().to_string(), p)).collect()
}

#[test]
fn up_to_400_characters_the_two_are_identical() {
    for text in ["fix a typo in the readme", "build a dashboard across several services", "research the options and write up a report", ""] {
        assert!(text.chars().count() <= 400);
        assert_eq!(p(&KeywordDecider, text), p(&KeywordDeciderNoLength, text), "{text:?}");
    }
    let at_400 = "x".repeat(400);
    assert_eq!(p(&KeywordDecider, &at_400), p(&KeywordDeciderNoLength, &at_400));
}

#[test]
fn past_400_characters_only_long_moves_and_only_the_ablation_is_neutral() {
    let text = "x".repeat(401);
    let (with, without) = (p(&KeywordDecider, &text), p(&KeywordDeciderNoLength, &text));
    // No keyword evidence at all: without the length rule the answer is uniform.
    for (_, pr) in &without {
        assert!((pr - 1.0 / 3.0).abs() < 1e-12);
    }
    let long = |v: &[(String, f64)]| v.iter().find(|(k, _)| k == "long").unwrap().1;
    assert!(long(&with) > long(&without));
}

#[test]
fn keyword_evidence_is_unchanged_by_the_ablation() {
    let body = "fix a typo ".repeat(60); // > 400 chars, one_shot keyword evidence
    assert!(body.chars().count() > 400);
    let v = p(&KeywordDeciderNoLength, &body);
    let best = v.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
    assert_eq!(best.0, "one_shot");
}

#[test]
fn the_shipped_baseline_still_has_its_length_rule() {
    // Guard: over 1500 chars with no keywords, long = 1+2+3 = 6 of 8 pseudo-counts = 0.75.
    let text = "x".repeat(1501);
    let v = p(&KeywordDecider, &text);
    let long = v.iter().find(|(k, _)| k == "long").unwrap().1;
    assert!((long - 6.0 / 8.0).abs() < 1e-12, "long = {long}");
}
