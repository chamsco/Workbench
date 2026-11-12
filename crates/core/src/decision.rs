//! The decision interface: a state plus typed questions in, one calibrated
//! probability distribution per question out. No text is generated. Anything
//! that can answer questions this way (a keyword baseline, a local model, a
//! hosted decision model) sits behind [`DecisionModel`] and can be swapped
//! without touching the router.

use anyhow::{anyhow, bail, Context, Result};
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

/// Reject an answer set that does not match the questions asked: one answer
/// per question, over exactly that question's options, each probability in
/// [0, 1], summing to 1. A model's output is not trusted until it passes.
pub fn validate(questions: &[Question], answers: &[Answer]) -> Result<()> {
    if answers.len() != questions.len() {
        bail!("{} questions asked, {} answers returned", questions.len(), answers.len());
    }
    for q in questions {
        let mut mine = answers.iter().filter(|a| a.question == q.id);
        let a = mine.next().ok_or_else(|| anyhow!("no answer to `{}`", q.id))?;
        if mine.next().is_some() {
            bail!("`{}` was answered twice", q.id);
        }
        if a.probs.len() != q.options.len() {
            bail!("`{}` has {} options, the answer has {}", q.id, q.options.len(), a.probs.len());
        }
        for o in &q.options {
            if a.probs.iter().filter(|(name, _)| name == o).count() != 1 {
                bail!("`{}`: option `{o}` must be answered exactly once", q.id);
            }
        }
        if let Some((o, _)) = a.probs.iter().find(|(name, _)| !q.options.contains(name)) {
            bail!("`{}`: `{o}` is not one of the options", q.id);
        }
        let mut sum = 0.0;
        for (o, p) in &a.probs {
            if !p.is_finite() || !(0.0..=1.0).contains(p) {
                bail!("`{}`: probability {p} for `{o}` is not in [0, 1]", q.id);
            }
            sum += p;
        }
        if (sum - 1.0).abs() > 1e-6 {
            bail!("`{}`: probabilities sum to {sum}, not 1", q.id);
        }
    }
    Ok(())
}

/// Ask, then check the reply before anyone relies on it.
pub fn decide_checked(model: &dyn DecisionModel, state: &str, questions: &[Question]) -> Result<Vec<Answer>> {
    let answers = model
        .decide(state, questions)
        .with_context(|| format!("decision model `{}` failed", model.name()))?;
    validate(questions, &answers).with_context(|| format!("decision model `{}` gave an unusable answer", model.name()))?;
    Ok(answers)
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
        match self {
            TaskType::OneShot => "one_shot",
            TaskType::Long => "long",
            TaskType::Research => "research",
        }
    }

    pub fn parse(s: &str) -> Option<TaskType> {
        TaskType::ALL.into_iter().find(|t| t.as_str() == s)
    }
}

pub const TASK_TYPE: &str = "task_type";

pub fn task_type_question() -> Question {
    Question { id: TASK_TYPE.into(), options: TaskType::ALL.iter().map(|t| t.as_str().to_string()).collect() }
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
        match *self {
            Triage::Confident { task, .. } => task,
            Triage::Ask { best, .. } => best,
        }
    }

    pub fn p(&self) -> f64 {
        match *self {
            Triage::Confident { p, .. } | Triage::Ask { p, .. } => p,
        }
    }

    /// The most probable type (the first listed wins a tie), acted on only if
    /// its probability reaches `threshold`. An empty distribution asks.
    pub fn from_distribution(dist: &[(TaskType, f64)], threshold: f64) -> Triage {
        let mut best = (TaskType::OneShot, f64::NEG_INFINITY);
        for &(t, p) in dist {
            if p > best.1 {
                best = (t, p);
            }
        }
        let (task, p) = if best.1.is_finite() { best } else { (TaskType::OneShot, 0.0) };
        if p >= threshold {
            Triage::Confident { task, p }
        } else {
            Triage::Ask { best: task, p }
        }
    }
}

/// Provisional until the §10 eval has run on labeled traces.
pub const DEFAULT_TRIAGE_THRESHOLD: f64 = 0.7;

/// The model's checked distribution over task types, in `TaskType::ALL` order.
pub fn task_distribution(model: &dyn DecisionModel, text: &str) -> Result<Vec<(TaskType, f64)>> {
    let answers = decide_checked(model, text, &[task_type_question()])?;
    let a = &answers[0];
    Ok(TaskType::ALL
        .into_iter()
        .map(|t| (t, a.probs.iter().find(|(o, _)| o == t.as_str()).map_or(0.0, |(_, p)| *p)))
        .collect())
}

pub fn classify(model: &dyn DecisionModel, text: &str, threshold: f64) -> Result<Triage> {
    Ok(Triage::from_distribution(&task_distribution(model, text)?, threshold))
}

/// The no-network baseline: keyword evidence turned into a distribution. It
/// is not calibrated, and the eval is what says how good it is. With no
/// evidence every type gets one pseudo-count, so the answer is uniform and
/// falls under any sensible threshold.
pub struct KeywordDecider;

const ONE_SHOT_WORDS: &[&str] = &[
    "typo", "rename", "readme", "fix a ", "bump", "tweak", "one line", "single line", "small change", "quick",
    "add a button", "change the colo", "update the version",
];
const LONG_WORDS: &[&str] = &[
    "build", "design", "architect", "across", "several", "multiple", "multi-", "system", "platform", "migrat",
    "end-to-end", "integrat", "rewrite", "from scratch", "dashboard", "services", "pipeline", "framework", "roadmap",
];
const RESEARCH_WORDS: &[&str] = &[
    "research", "investigate", "compare", "comparison", "survey", "literature", "find out", "pros and cons",
    "options for", "write up", "report", "summarize", "summarise", "explain", "what is", "why does", "analys",
    "evaluate",
];
const NO_CODE: &[&str] = &["no code", "without changing code", "without code"];

fn keyword_scores(text: &str) -> [f64; 3] {
    let t = text.to_lowercase();
    let hits = |words: &[&str]| words.iter().filter(|w| t.contains(**w)).count() as f64;
    let mut s = [1.0; 3];
    s[0] += 2.0 * hits(ONE_SHOT_WORDS);
    s[1] += 1.5 * hits(LONG_WORDS);
    s[2] += 2.0 * hits(RESEARCH_WORDS);
    if hits(NO_CODE) > 0.0 {
        s[2] += 3.0;
    }
    let chars = t.chars().count();
    if chars > 400 {
        s[1] += 2.0;
    }
    if chars > 1500 {
        s[1] += 3.0;
    }
    s
}

impl DecisionModel for KeywordDecider {
    fn name(&self) -> &str {
        "keyword"
    }

    fn decide(&self, state: &str, questions: &[Question]) -> Result<Vec<Answer>> {
        let scores = keyword_scores(state);
        let total: f64 = scores.iter().sum();
        questions
            .iter()
            .map(|q| {
                if q.id != TASK_TYPE {
                    bail!("the keyword decider only answers `{TASK_TYPE}`, not `{}`", q.id);
                }
                let probs = q
                    .options
                    .iter()
                    .map(|o| {
                        let i = TaskType::parse(o).ok_or_else(|| anyhow!("`{o}` is not a task type"))? as usize;
                        Ok((o.clone(), scores[i] / total))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(Answer { question: q.id.clone(), probs })
            })
            .collect()
    }
}
