//! M4 planning stage (PLAN §5.2, §6). Red stub: types only, bodies to come.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::decision::{DecisionModel, Question};
use crate::provider::ToolDef;
use crate::tools::Workspace;

pub const SPEC: &str = ".backspace/planning/spec.md";
pub const BASELINE: &str = ".backspace/planning/baseline.json";
pub const MET: &str = "met";
pub const MISSING: &str = "missing";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Criterion {
    pub criterion: String,
    pub test: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BaselineSpec {
    BugFix { repro: String },
    Feature,
    UiChange { capture: Option<String>, screenshots: Vec<String> },
    Refactor { suite: String, perf: Option<String> },
    Research,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct PlanSpec {
    pub title: String,
    pub goal: String,
    pub acceptance: Vec<Criterion>,
    pub non_goals: Vec<String>,
    pub constraints: Vec<String>,
    pub dependencies: Vec<String>,
    pub baseline: Option<BaselineSpec>,
}

impl PlanSpec {
    pub fn from_json(_v: &Value) -> Result<PlanSpec> {
        todo!("M4")
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Item {
    Goal,
    AcceptanceCriteria,
    TestPerCriterion,
    NonGoals,
    Constraints,
    Dependencies,
    Baseline,
}

impl Item {
    pub const ALL: [Item; 7] = [Item::Goal, Item::Goal, Item::Goal, Item::Goal, Item::Goal, Item::Goal, Item::Goal];
    pub fn key(self) -> &'static str {
        todo!("M4")
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Check {
    pub item: Item,
    pub passed: bool,
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Checklist {
    pub items: Vec<Check>,
}

impl Checklist {
    pub fn passed(&self) -> bool {
        todo!("M4")
    }
    pub fn gaps(&self) -> Vec<String> {
        todo!("M4")
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Run {
    pub command: String,
    pub exit: i32,
    pub ms: u64,
    pub output: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Shot {
    pub path: String,
    pub stored: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Baseline {
    pub kind: String,
    pub ok: bool,
    pub note: String,
    #[serde(default)]
    pub runs: Vec<Run>,
    #[serde(default)]
    pub shots: Vec<Shot>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct PlanningState {
    #[serde(default)]
    pub spec_sha256: Option<String>,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub awaiting_yes: bool,
    #[serde(default)]
    pub approved_sha256: Option<String>,
}

pub fn check(_plan: &PlanSpec, _baseline: Option<&Baseline>) -> Checklist {
    todo!("M4")
}

pub async fn capture(_plan: &PlanSpec, _ws: &Workspace, _root: &Path) -> Baseline {
    todo!("M4")
}

pub fn render(_plan: &PlanSpec, _baseline: &Baseline, _checklist: &Checklist) -> String {
    todo!("M4")
}

pub fn sha256_hex(_bytes: &[u8]) -> String {
    todo!("M4")
}

pub fn parse_yes(_text: &str) -> bool {
    todo!("M4")
}

pub fn questions() -> Vec<Question> {
    todo!("M4")
}

pub fn score(_model: &dyn DecisionModel, _spec: &str, _rules: &Checklist, _threshold: f64) -> Result<Option<String>> {
    todo!("M4")
}

pub fn submit_spec_tool() -> ToolDef {
    todo!("M4")
}
