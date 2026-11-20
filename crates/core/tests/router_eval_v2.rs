//! §10 historical-prompt replay, v2 corpus: dataset integrity, group
//! membership, control handling and the extended metrics. The v1 files are
//! kept as they were; v2 is a corrected candidate, not ground truth.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use backspace_core::decision::{Answer, DecisionModel, Question, TaskType, DEFAULT_TRIAGE_THRESHOLD};
use backspace_core::eval::{extended, parse_traces, Trace};
use serde_json::Value;
use sha2::{Digest, Sha256};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/cauldron/router-eval-v2/");
const FILES: [(&str, &str); 4] = [
    ("primary", "router-eval-v2.jsonl"),
    ("pool", "router-eval-v2-owner-review-pool.jsonl"),
    ("control", "router-eval-v2-excluded.jsonl"),
    ("stress", "router-eval-v2-stress.jsonl"),
];

fn load(file: &str) -> Vec<Value> {
    std::fs::read_to_string(format!("{DIR}{file}"))
        .unwrap_or_else(|e| panic!("{file}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or_else(|| panic!("{} has no string `{k}`", v["id"]))
}
fn ids(rows: &[Value]) -> BTreeSet<String> {
    rows.iter().map(|r| s(r, "id").to_string()).collect()
}
fn flags(r: &Value) -> Vec<String> {
    r["flags"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect()
}

#[test]
fn every_row_hash_and_length_match_its_text() {
    for (_, f) in FILES {
        for r in load(f) {
            let t = s(&r, "text");
            assert_eq!(r["chars"].as_u64().unwrap() as usize, t.chars().count(), "{} chars", r["id"]);
            assert_eq!(s(&r, "text_sha256"), format!("{:x}", Sha256::digest(t.as_bytes())), "{} hash", r["id"]);
            assert!(s(&r, "source_message_id").ends_with(s(&r, "id")), "{}", r["id"]);
            assert!(!s(&r, "created_at").is_empty());
        }
    }
}

#[test]
fn no_message_is_in_two_groups_and_group_fields_agree_with_files() {
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for (g, f) in FILES {
        for r in load(f) {
            assert_eq!(s(&r, "group"), g, "{} sits in {f}", r["id"]);
            assert!(seen.insert(s(&r, "id").to_string(), g).is_none(), "{} appears twice", r["id"]);
        }
    }
    assert_eq!(seen.len(), 31 + 17 + 6 + 1, "group counts changed: update the report");
}

#[test]
fn the_three_reclassified_controls_are_tasks_in_the_primary_set() {
    let primary = load(FILES[0].1);
    let find = |id: &str| primary.iter().find(|r| s(r, "id") == id).unwrap_or_else(|| panic!("{id} not primary")).clone();
    assert_eq!(s(&find("3vfSNZZAcn"), "label"), "one_shot");
    assert_eq!(s(&find("d4MEWNBX8V"), "label"), "research");
    assert_eq!(s(&find("VTsRosZxiV"), "label"), "research");
    let control = ids(&load(FILES[2].1));
    for id in ["3vfSNZZAcn", "d4MEWNBX8V", "VTsRosZxiV", "BUau1Gzo6o"] {
        assert!(!control.contains(id), "{id} is a task request and must not stay a control");
    }
}

#[test]
fn context_dependent_set_is_exactly_the_eight_and_ambiguity_is_separate() {
    let pool = load(FILES[1].1);
    let by = |reason: &str| -> BTreeSet<String> { pool.iter().filter(|r| s(r, "pool_reason") == reason).map(|r| s(r, "id").to_string()).collect() };
    let want: BTreeSet<String> =
        ["x7JWmQtsnD", "YdeMLtxPLj", "SrJFhTmoT9", "txK4nC7PZi", "GtKvSDHfQB", "at7XCYUxdG", "Ke2VfK4qip", "bf7eL9rznC"].iter().map(|x| x.to_string()).collect();
    assert_eq!(by("context_dependent"), want);
    assert!(by("ambiguous").contains("MHgLPqGHzr"));
    assert!(!by("context_dependent").contains("MHgLPqGHzr"));
    // flags follow the reason, and only the reason
    for r in &pool {
        let f = flags(r);
        let reason = s(r, "pool_reason");
        assert_eq!(f.contains(&"context-dependent".to_string()), reason == "context_dependent", "{}", r["id"]);
        assert_eq!(f.contains(&"ambiguous".to_string()), reason == "ambiguous", "{}", r["id"]);
        assert_eq!(f.contains(&"low-confidence".to_string()), reason == "low_confidence", "{}", r["id"]);
    }
    // nothing context-dependent in the primary denominator
    for r in load(FILES[0].1) {
        let f = flags(&r);
        for bad in ["context-dependent", "ambiguous", "low-confidence"] {
            assert!(!f.contains(&bad.to_string()), "{} is primary but flagged {bad}", r["id"]);
        }
    }
}

#[test]
fn restored_messages_are_complete_and_truncation_is_history_only() {
    let all: Vec<Value> = FILES.iter().flat_map(|(_, f)| load(f)).collect();
    for r in &all {
        assert!(!flags(r).contains(&"truncated-8kb".to_string()), "{} still carries the defect flag", r["id"]);
        assert!(!s(r, "text").contains("bytes truncated"));
    }
    for (id, min) in [("wYCfpHgci1", 9460), ("BjxMTw3GNE", 19567), ("ijp71Z5AZt", 9126), ("xhMcczbtWd", 8267)] {
        let r = all.iter().find(|r| s(r, "id") == id).unwrap();
        assert_eq!(r["chars"].as_u64().unwrap(), min, "{id}");
        assert_eq!(r["historically_truncated_in_draft"], true, "{id}");
    }
    let ke = all.iter().find(|r| s(r, "id") == "Ke2VfK4qip").unwrap();
    assert_eq!(ke["chars"].as_u64().unwrap(), 477);
    assert_eq!(s(ke, "pool_reason"), "context_dependent");
    assert_eq!(all.iter().filter(|r| r["historically_truncated_in_draft"] == true).count(), 4);
}

#[test]
fn nothing_is_owner_approved_and_every_row_says_so() {
    for (_, f) in FILES {
        for r in load(f) {
            assert_eq!(r["owner_approved"], false, "{}", r["id"]);
            assert_eq!(s(&r, "label_status"), "provisional-pending-owner-review");
        }
    }
}

#[test]
fn primary_and_stress_parse_as_traces_and_controls_have_no_label() {
    for f in [FILES[0].1, FILES[3].1] {
        let t = parse_traces(&std::fs::read_to_string(format!("{DIR}{f}")).unwrap()).unwrap();
        assert!(!t.is_empty());
        assert!(t.iter().all(|t| t.input_tokens.is_none() && t.output_tokens.is_none()), "no measured tokens exist");
    }
    for r in load(FILES[2].1) {
        assert!(r["label"].is_null(), "control {} carries a label", r["id"]);
        assert_eq!(s(&r, "expected"), "ask");
    }
    let classes: BTreeSet<String> = load(FILES[0].1).iter().map(|r| s(r, "label").to_string()).collect();
    assert_eq!(classes, ["long", "one_shot", "research"].iter().map(|s| s.to_string()).collect());
}

// ---- extended metrics, on a model whose answers are known -----------------

struct Fixed;
impl DecisionModel for Fixed {
    fn name(&self) -> &str {
        "fixed"
    }
    /// The text names the distribution: "<type> <p>" puts p on that type and splits the rest.
    fn decide(&self, state: &str, questions: &[Question]) -> Result<Vec<Answer>> {
        let mut it = state.split_whitespace();
        let (t, p): (&str, f64) = (it.next().unwrap(), it.next().unwrap().parse().unwrap());
        Ok(questions
            .iter()
            .map(|q| Answer {
                question: q.id.clone(),
                probs: q.options.iter().map(|o| (o.clone(), if o == t { p } else { (1.0 - p) / 2.0 })).collect(),
            })
            .collect())
    }
}
fn tr(id: &str, text: &str, label: TaskType) -> Trace {
    serde_json::from_value(serde_json::json!({"id": id, "text": text, "label": label.as_str()})).unwrap()
}

#[test]
fn extended_metrics_on_known_answers() {
    let traces = vec![
        tr("a", "long 0.9", TaskType::Long),          // confident, right
        tr("b", "long 0.8", TaskType::OneShot),       // confident, wrong
        tr("c", "one_shot 0.5", TaskType::OneShot),   // asked, right
        tr("d", "research 0.6", TaskType::Long),      // asked, wrong
        tr("e", "one_shot 0.7", TaskType::OneShot),   // exactly the threshold: confident, right
    ];
    let x = extended(&Fixed, &traces, DEFAULT_TRIAGE_THRESHOLD).unwrap();
    assert_eq!((x.confident, x.confident_correct), (3, 2));
    assert_eq!((x.asked, x.wrong_caught, x.asked_but_right), (2, 1, 1));
    // rows true, columns predicted in TaskType::ALL order: one_shot, long, research
    assert_eq!(x.confusion, [[2, 1, 0], [0, 1, 1], [0, 0, 0]]);
    assert_eq!(x.per_trace.len(), 5);
    assert!(x.per_trace.iter().find(|p| p.id == "e").unwrap().confident);
    assert_eq!(x.per_trace.iter().filter(|p| p.correct).count(), 3);
}

#[test]
fn extended_agrees_with_evaluate_on_overall_accuracy() {
    use backspace_core::config::{Config, DEFAULT_CONFIG};
    use backspace_core::eval::evaluate;
    let traces = vec![tr("a", "long 0.9", TaskType::Long), tr("b", "research 0.6", TaskType::Long), tr("c", "one_shot 0.4", TaskType::OneShot)];
    let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
    let r = evaluate(&Fixed, &traces, 0.7, &cfg).unwrap();
    let x = extended(&Fixed, &traces, 0.7).unwrap();
    let diag: usize = (0..3).map(|i| x.confusion[i][i]).sum();
    assert_eq!(diag, r.correct);
    assert_eq!(x.confident + x.asked, r.n);
    assert_eq!(x.asked, r.below_threshold);
    assert_eq!(x.wrong_caught, r.wrong_below_threshold);
    assert_eq!(x.confident - x.confident_correct, r.confident_wrong);
}

#[test]
fn the_majority_class_baseline_uses_the_actual_distribution() {
    let rows = load(FILES[0].1);
    let mut n: BTreeMap<String, usize> = BTreeMap::new();
    for r in &rows {
        *n.entry(s(r, "label").to_string()).or_default() += 1;
    }
    assert_eq!(n["long"], 17);
    assert_eq!(n["one_shot"], 8);
    assert_eq!(n["research"], 6);
    assert_eq!(n.values().max().copied().unwrap(), 17);
}
