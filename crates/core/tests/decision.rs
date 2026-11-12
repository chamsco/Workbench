//! M2 acceptance: the decision interface. Anything that answers typed
//! questions with probabilities can be swapped in; a malformed answer is
//! rejected rather than trusted; below the threshold the user is asked.

use anyhow::{anyhow, Result};
use backspace_core::decision::*;

/// Answers the task-type question with a fixed distribution, whatever the text.
struct Fixed(f64, f64, f64);

impl DecisionModel for Fixed {
    fn name(&self) -> &str {
        "fixed"
    }
    fn decide(&self, _state: &str, questions: &[Question]) -> Result<Vec<Answer>> {
        Ok(questions
            .iter()
            .map(|q| Answer {
                question: q.id.clone(),
                probs: vec![
                    ("one_shot".into(), self.0),
                    ("long".into(), self.1),
                    ("research".into(), self.2),
                ],
            })
            .collect())
    }
}

/// Returns whatever answers it was built with, valid or not.
struct Scripted(Vec<Answer>);

impl DecisionModel for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }
    fn decide(&self, _state: &str, _questions: &[Question]) -> Result<Vec<Answer>> {
        Ok(self.0.clone())
    }
}

struct Broken;

impl DecisionModel for Broken {
    fn name(&self) -> &str {
        "broken"
    }
    fn decide(&self, _state: &str, _questions: &[Question]) -> Result<Vec<Answer>> {
        Err(anyhow!("the model is down"))
    }
}

fn ans(q: &str, probs: &[(&str, f64)]) -> Answer {
    Answer { question: q.into(), probs: probs.iter().map(|(o, p)| (o.to_string(), *p)).collect() }
}

fn argmax(model: &dyn DecisionModel, text: &str) -> TaskType {
    let d = task_distribution(model, text).unwrap();
    d.iter().cloned().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0
}

#[test]
fn the_baseline_answers_each_question_with_a_distribution_that_sums_to_one() {
    let q = task_type_question();
    let a = decide_checked(&KeywordDecider, "fix a typo in the readme", &[q.clone()]).unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].question, q.id);
    let sum: f64 = a[0].probs.iter().map(|(_, p)| p).sum();
    assert!((sum - 1.0).abs() < 1e-9, "{sum}");
    assert!(a[0].probs.iter().all(|(_, p)| (0.0..=1.0).contains(p)));
    let options: Vec<&str> = a[0].probs.iter().map(|(o, _)| o.as_str()).collect();
    assert_eq!(options, q.options.iter().map(String::as_str).collect::<Vec<_>>());
}

#[test]
fn the_baseline_separates_clear_cases() {
    assert_eq!(argmax(&KeywordDecider, "fix a typo in the readme"), TaskType::OneShot);
    assert_eq!(
        argmax(
            &KeywordDecider,
            "research the options for rate limiting libraries and write up a comparison report, no code changes"
        ),
        TaskType::Research
    );
    assert_eq!(
        argmax(
            &KeywordDecider,
            "design and build a multi-tenant billing system across several services with database migrations, auth and an admin dashboard"
        ),
        TaskType::Long
    );
}

#[test]
fn nothing_to_go_on_is_never_a_confident_answer() {
    for text in ["", "   ", "ok"] {
        let t = classify(&KeywordDecider, text, DEFAULT_TRIAGE_THRESHOLD).unwrap();
        assert!(matches!(t, Triage::Ask { .. }), "{text:?} -> {t:?}");
    }
}

#[test]
fn a_different_decider_can_be_swapped_in_and_decides_instead() {
    let text = "fix a typo in the readme";
    let baseline = classify(&KeywordDecider, text, 0.5).unwrap();
    assert_eq!(baseline.best(), TaskType::OneShot);
    let swapped = classify(&Fixed(0.05, 0.05, 0.9), text, 0.5).unwrap();
    assert_eq!(swapped, Triage::Confident { task: TaskType::Research, p: 0.9 });
}

#[test]
fn below_the_threshold_the_user_is_asked_and_at_it_the_answer_stands() {
    let low = classify(&Fixed(0.5, 0.3, 0.2), "anything", 0.7).unwrap();
    assert_eq!(low, Triage::Ask { best: TaskType::OneShot, p: 0.5 });
    let at = classify(&Fixed(0.1, 0.7, 0.2), "anything", 0.7).unwrap();
    assert_eq!(at, Triage::Confident { task: TaskType::Long, p: 0.7 });
}

#[test]
fn ties_resolve_the_same_way_every_time() {
    let a = Triage::from_distribution(&[(TaskType::OneShot, 0.4), (TaskType::Long, 0.4), (TaskType::Research, 0.2)], 0.9);
    for _ in 0..10 {
        assert_eq!(
            a,
            Triage::from_distribution(&[(TaskType::OneShot, 0.4), (TaskType::Long, 0.4), (TaskType::Research, 0.2)], 0.9)
        );
    }
    assert_eq!(a.best(), TaskType::OneShot, "first listed wins a tie");
}

#[test]
fn a_decider_that_fails_is_an_error_not_a_guess() {
    assert!(classify(&Broken, "anything", 0.7).is_err());
}

#[test]
fn a_malformed_answer_is_rejected() {
    let q = task_type_question();
    let good = [("one_shot", 0.2), ("long", 0.3), ("research", 0.5)];
    assert!(decide_checked(&Scripted(vec![ans(TASK_TYPE, &good)]), "x", &[q.clone()]).is_ok());

    let cases: Vec<(&str, Vec<Answer>)> = vec![
        ("no answer", vec![]),
        ("answer to another question", vec![ans("other", &good)]),
        ("answered twice", vec![ans(TASK_TYPE, &good), ans(TASK_TYPE, &good)]),
        ("an option that was not offered", vec![ans(TASK_TYPE, &[("one_shot", 0.5), ("long", 0.3), ("rewrite", 0.2)])]),
        ("an option missing", vec![ans(TASK_TYPE, &[("one_shot", 0.5), ("long", 0.5)])]),
        ("an option twice", vec![ans(TASK_TYPE, &[("one_shot", 0.5), ("one_shot", 0.3), ("long", 0.1), ("research", 0.1)])]),
        ("does not sum to one", vec![ans(TASK_TYPE, &[("one_shot", 0.5), ("long", 0.5), ("research", 0.5)])]),
        ("a negative probability", vec![ans(TASK_TYPE, &[("one_shot", 1.2), ("long", -0.2), ("research", 0.0)])]),
        ("NaN", vec![ans(TASK_TYPE, &[("one_shot", f64::NAN), ("long", 0.5), ("research", 0.5)])]),
    ];
    for (what, answers) in cases {
        assert!(
            decide_checked(&Scripted(answers.clone()), "x", &[q.clone()]).is_err(),
            "{what} must be rejected"
        );
        // And classification never acts on it.
        assert!(classify(&Scripted(answers), "x", 0.5).is_err(), "{what} must not classify");
    }
}

#[test]
fn task_types_round_trip_through_their_names() {
    for t in TaskType::ALL {
        assert_eq!(TaskType::parse(t.as_str()), Some(t));
        assert_eq!(serde_json::to_string(&t).unwrap(), format!("\"{}\"", t.as_str()));
    }
    assert_eq!(TaskType::parse("one_shot"), Some(TaskType::OneShot));
    assert_eq!(TaskType::parse("ONE_SHOT"), None);
    assert_eq!(TaskType::parse("epic"), None);
    assert_eq!(task_type_question().options, ["one_shot", "long", "research"]);
}
