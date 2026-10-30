//! The decision interface: a state plus typed questions in, one calibrated
//! probability distribution per question out. No text is generated. Anything
//! that can answer questions this way (a keyword baseline, a local model, a
//! hosted decision model) sits behind [`DecisionModel`] and can be swapped
//! without touching the router.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A closed question: the answer is one of `options`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Question {
    pub id: String,
    pub options: Vec<String>,
}

/// A distribution over a question's options. Probabilities are in [0, 1] and
/// sum to 1.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Answer {
    pub question: String,
    pub probs: Vec<(String, f64)>,
}

pub trait DecisionModel: Send + Sync {
    fn name(&self) -> &str;
    fn decide(&self, state: &str, questions: &[Question]) -> Result<Vec<Answer>>;
}

/// Reject an answer set that does not match the questions asked.
pub fn validate(_questions: &[Question], _answers: &[Answer]) -> Result<()> {
    unimplemented!("M2")
}

/// Ask, then check the reply before anyone relies on it.
pub fn decide_checked(_model: &dyn DecisionModel, _state: &str, _questions: &[Question]) -> Result<Vec<Answer>> {
    unimplemented!("M2")
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    OneShot,
    Long,
    Research,
}

impl TaskType {
    pub const ALL: [TaskType; 3] = [TaskType::OneShot, TaskType::Long, TaskType::Research];

    pub fn as_str(self) -> &'static str {
        unimplemented!("M2")
    }

    pub fn parse(_s: &str) -> Option<TaskType> {
        unimplemented!("M2")
    }
}

pub const TASK_TYPE: &str = "task_type";

pub fn task_type_question() -> Question {
    unimplemented!("M2")
}

/// Confident enough to act on, or the user should be asked.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Triage {
    Confident { task: TaskType, p: f64 },
    Ask { best: TaskType, p: f64 },
}

impl Triage {
    pub fn best(&self) -> TaskType {
        unimplemented!("M2")
    }

    pub fn p(&self) -> f64 {
        unimplemented!("M2")
    }

    pub fn from_distribution(_dist: &[(TaskType, f64)], _threshold: f64) -> Triage {
        unimplemented!("M2")
    }
}

/// Provisional until the §10 eval has run on labeled traces.
pub const DEFAULT_TRIAGE_THRESHOLD: f64 = 0.7;

pub fn task_distribution(_model: &dyn DecisionModel, _text: &str) -> Result<Vec<(TaskType, f64)>> {
    unimplemented!("M2")
}

pub fn classify(_model: &dyn DecisionModel, _text: &str, _threshold: f64) -> Result<Triage> {
    unimplemented!("M2")
}

/// The no-network baseline: keyword evidence turned into a distribution. It
/// is not calibrated, and the eval is what says how good it is.
pub struct KeywordDecider;

impl DecisionModel for KeywordDecider {
    fn name(&self) -> &str {
        "keyword"
    }

    fn decide(&self, _state: &str, _questions: &[Question]) -> Result<Vec<Answer>> {
        unimplemented!("M2")
    }
}
