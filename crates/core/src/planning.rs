//! M4: the long path's planning stage (PLAN §5.2, §6).
//!
//! The planner grills the user, then hands in the plan as a *typed* checklist
//! (`submit_spec`). Code, not the model, decides whether it is complete:
//!
//! 1. goal stated, 2. acceptance criteria present, 3. one test per criterion,
//! 4. non-goals stated, 5. constraints stated, 6. dependencies named,
//! 7. the "before" baseline captured.
//!
//! The baseline is captured here by deterministic tools (zero tokens), per
//! task type (§6): a bug fix needs a repro that fails today, a feature needs
//! acceptance tests that fail today, a UI change needs screenshots of the
//! current state (copied and hashed), a refactor needs the existing suite
//! passing plus performance numbers. Research has none: it is a report.
//!
//! No baseline command runs before the user has approved that exact list
//! ([`commands`], bound to [`commands_sha256`]): the planner submits, the
//! commands are put to the user, and only after their yes does a second
//! submission run them.
//!
//! When every item is met, code writes `spec.md` from the checklist and asks
//! the user for a yes. The yes is bound to the file's SHA-256: tickets on the
//! long path are refused until the spec on disk is the one the user approved
//! (harness `create_batch`). A decision model may score the checklist through
//! [`score`], but it can only refuse what the rules passed (§6: it cannot see
//! unknown unknowns, so a human yes stays required).
//!
//! Everything here is pure except [`capture`], which runs commands.

use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::decision::{decide_checked, DecisionModel, Question};
use crate::provider::ToolDef;
use crate::tools::Workspace;

pub const DIR: &str = ".backspace/planning";
/// The handoff file (§3.3): what Builder and Rampart get in M5.
pub const SPEC: &str = ".backspace/planning/spec.md";
/// The record of the last capture, kept whether it held or not.
pub const BASELINE: &str = ".backspace/planning/baseline.json";
/// Copies of baseline screenshots: the current state will not stay current.
pub const BASELINE_DIR: &str = ".backspace/planning/baseline";

pub const MET: &str = "met";
pub const MISSING: &str = "missing";

pub const PROMPT: &str = include_str!("prompts/planning.md");

const PLACEHOLDERS: [&str; 4] = ["TBD", "TODO", "???", "FIXME"];
const NONE: [&str; 5] = ["none", "nothing", "n/a", "aucune", "aucun"];
/// Kept from a command's output in the record.
const OUTPUT_TAIL: usize = 2000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Criterion {
    #[serde(default)]
    pub criterion: String,
    /// The command that checks it.
    #[serde(default)]
    pub test: String,
}

/// The baseline asked for, by task type (§6 table).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BaselineSpec {
    /// A command that reproduces the bug: it must fail today.
    BugFix { repro: String },
    /// The acceptance tests themselves: each must fail today.
    Feature,
    /// Screenshots of the current state, optionally made by `capture` first.
    UiChange {
        #[serde(default)]
        capture: Option<String>,
        #[serde(default)]
        screenshots: Vec<String>,
    },
    /// The existing suite (must pass) and, optionally, a perf command.
    Refactor {
        suite: String,
        #[serde(default)]
        perf: Option<String>,
    },
    /// No baseline: research is a report, not a PR.
    Research,
}

impl BaselineSpec {
    pub fn kind(&self) -> &'static str {
        match self {
            BaselineSpec::BugFix { .. } => "bug_fix",
            BaselineSpec::Feature => "feature",
            BaselineSpec::UiChange { .. } => "ui_change",
            BaselineSpec::Refactor { .. } => "refactor",
            BaselineSpec::Research => "research",
        }
    }
}

/// The typed checklist the planner submits.
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
    /// Read a plan. A missing field is empty (the checklist then names it); a
    /// field of the wrong type is an error naming the field.
    pub fn from_json(v: &Value) -> Result<PlanSpec> {
        fn field<T: serde::de::DeserializeOwned + Default>(v: &Value, name: &str) -> Result<T> {
            match v.get(name) {
                None | Some(Value::Null) => Ok(T::default()),
                Some(x) => serde_json::from_value(x.clone()).with_context(|| format!("`{name}` is not the right shape")),
            }
        }
        if !v.is_object() && !v.is_null() {
            bail!("the plan must be an object");
        }
        Ok(PlanSpec {
            title: field(v, "title")?,
            goal: field(v, "goal")?,
            acceptance: field(v, "acceptance")?,
            non_goals: field(v, "non_goals")?,
            constraints: field(v, "constraints")?,
            dependencies: field(v, "dependencies")?,
            baseline: match v.get("baseline") {
                None | Some(Value::Null) => None,
                Some(b) => Some(serde_json::from_value(b.clone()).context("`baseline` is not one of bug_fix, feature, ui_change, refactor")?),
            },
        })
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
    pub const ALL: [Item; 7] = [
        Item::Goal,
        Item::AcceptanceCriteria,
        Item::TestPerCriterion,
        Item::NonGoals,
        Item::Constraints,
        Item::Dependencies,
        Item::Baseline,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Item::Goal => "goal",
            Item::AcceptanceCriteria => "acceptance_criteria",
            Item::TestPerCriterion => "test_per_criterion",
            Item::NonGoals => "non_goals",
            Item::Constraints => "constraints",
            Item::Dependencies => "dependencies",
            Item::Baseline => "baseline",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Item::Goal => "Goal",
            Item::AcceptanceCriteria => "Acceptance criteria",
            Item::TestPerCriterion => "Test per criterion",
            Item::NonGoals => "Non-goals",
            Item::Constraints => "Constraints",
            Item::Dependencies => "Dependencies",
            Item::Baseline => "Baseline",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Check {
    pub item: Item,
    pub passed: bool,
    /// Why it is met, or what is missing.
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Checklist {
    pub items: Vec<Check>,
}

impl Checklist {
    pub fn passed(&self) -> bool {
        !self.items.is_empty() && self.items.iter().all(|c| c.passed)
    }

    /// One line per unmet item: "Label: what is missing".
    pub fn gaps(&self) -> Vec<String> {
        self.items.iter().filter(|c| !c.passed).map(|c| format!("{}: {}", c.item.label(), c.note)).collect()
    }

    /// Every item but the baseline met: only then are commands run to capture it.
    pub fn ready_to_capture(&self) -> bool {
        self.items.iter().all(|c| c.passed || c.item == Item::Baseline)
    }

    fn markdown(&self) -> String {
        self.items.iter().map(|c| format!("- [{}] {}: {}", if c.passed { "x" } else { " " }, c.item.label(), c.note)).collect::<Vec<_>>().join("\n")
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Run {
    pub command: String,
    pub exit: i32,
    pub ms: u64,
    /// The end of what it printed.
    pub output: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Shot {
    /// Where it was, in the workspace.
    pub path: String,
    /// The copy, relative to the project root.
    pub stored: String,
    pub sha256: String,
    pub bytes: u64,
}

/// What a capture found. `ok` is the baseline checklist item.
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

/// Where the long path's planning stands. Saved with the project.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct PlanningState {
    /// The hash of the spec.md last written (only a passing checklist writes one).
    #[serde(default)]
    pub spec_sha256: Option<String>,
    /// Whether the last submission passed every item.
    #[serde(default)]
    pub passed: bool,
    /// The user was asked for a yes on `spec_sha256` and has not answered.
    #[serde(default)]
    pub awaiting_yes: bool,
    /// The spec the user said yes to.
    #[serde(default)]
    pub approved_sha256: Option<String>,
    /// The baseline commands were put to the user and they have not answered.
    #[serde(default)]
    pub awaiting_commands: bool,
    /// The hash of the command list last put to the user ([`commands_sha256`]).
    #[serde(default)]
    pub commands_sha256: Option<String>,
    /// The command list the user said yes to: only this list may run.
    #[serde(default)]
    pub commands_approved_sha256: Option<String>,
}

/// Every command capturing this plan's baseline would run, in order. Empty
/// when it runs none (screenshots only, or no baseline). These are put to
/// the user before any of them runs.
pub fn commands(plan: &PlanSpec) -> Vec<String> {
    let some = |c: &str| (!blank(c)).then(|| c.trim().to_string());
    match &plan.baseline {
        None | Some(BaselineSpec::Research) => vec![],
        Some(BaselineSpec::BugFix { repro }) => some(repro).into_iter().collect(),
        Some(BaselineSpec::Feature) => plan.acceptance.iter().filter_map(|c| some(&c.test)).collect(),
        Some(BaselineSpec::UiChange { capture, .. }) => capture.as_deref().and_then(some).into_iter().collect(),
        Some(BaselineSpec::Refactor { suite, perf }) => some(suite).into_iter().chain(perf.as_deref().and_then(some)).collect(),
    }
}

/// The hash a user's approval of the commands is bound to: the task type and
/// the exact list, so one changed command is a new list.
pub fn commands_sha256(plan: &PlanSpec) -> String {
    let kind = plan.baseline.as_ref().map_or("", |b| b.kind());
    sha256_hex(serde_json::json!({"kind": kind, "commands": commands(plan)}).to_string().as_bytes())
}

/// The message asking the user to approve the commands before they run.
pub fn commands_ask(plan: &PlanSpec) -> String {
    let list = commands(plan).iter().map(|c| format!("```sh\n{c}\n```")).collect::<Vec<_>>().join("\n");
    format!(
        "Every other checklist item is met. To capture the {} baseline I need to run these commands in the planning worktree. Nothing has run yet:\n\n{list}\n\nReply **yes** to run exactly these, or tell me what to change.",
        plan.baseline.as_ref().map_or("", |b| b.kind())
    )
}

/// The user's reply to the commands put to them: a plain yes approves that
/// exact list. Returns the text for the planner and what to log.
pub fn answer_commands(state: &mut PlanningState, text: &str) -> (String, String) {
    state.awaiting_commands = false;
    if !parse_yes(text) {
        return (text.to_string(), "That is not a yes, so the baseline commands did not run: the planner revises them and asks again.".into());
    }
    state.commands_approved_sha256 = state.commands_sha256.clone();
    (
        format!("{text}\n\n(Backspace: the user approved the baseline commands. Call submit_spec again with the same plan to run them.)"),
        "You approved the baseline commands; they run when the planner submits the plan again.".into(),
    )
}

fn blank(s: &str) -> bool {
    s.trim().is_empty()
}

fn is_none(s: &str) -> bool {
    let t = s.trim().trim_start_matches(['-', '*']).trim().trim_end_matches('.').to_lowercase();
    NONE.contains(&t.as_str())
}

fn placeholder(s: &str) -> Option<&'static str> {
    PLACEHOLDERS.iter().copied().find(|p| s.contains(p))
}

/// A list section that must be stated: non-empty, no blank line, no
/// placeholder. "None" is a statement.
fn stated(list: &[String], what: &str) -> (bool, String) {
    if list.is_empty() {
        return (false, format!("no {what} stated (write \"None\" if there are truly none)"));
    }
    if let Some(i) = list.iter().position(|s| blank(s)) {
        return (false, format!("line {} is blank", i + 1));
    }
    if let Some(p) = list.iter().find_map(|s| placeholder(s)) {
        return (false, format!("placeholder left: {p}"));
    }
    if list.iter().all(|s| is_none(s)) {
        return (true, "none, stated".into());
    }
    (true, format!("{} stated", list.len()))
}

/// Judge a plan by rules. `baseline` is the capture, once there is one.
pub fn check(plan: &PlanSpec, baseline: Option<&Baseline>) -> Checklist {
    let mut items = Vec::new();
    let mut push = |item: Item, (passed, note): (bool, String)| items.push(Check { item, passed, note });

    push(
        Item::Goal,
        if blank(&plan.goal) || is_none(&plan.goal) {
            (false, "no goal stated: say what the task achieves and for whom".into())
        } else if let Some(p) = placeholder(&plan.goal) {
            (false, format!("placeholder left: {p}"))
        } else {
            (true, "stated".into())
        },
    );

    let n = plan.acceptance.len();
    push(
        Item::AcceptanceCriteria,
        if n == 0 {
            (false, "no acceptance criteria: list what must be true when it is done".into())
        } else if let Some(i) = plan.acceptance.iter().position(|c| blank(&c.criterion)) {
            (false, format!("A{} has no criterion", i + 1))
        } else if let Some((i, p)) = plan.acceptance.iter().enumerate().find_map(|(i, c)| placeholder(&c.criterion).map(|p| (i, p))) {
            (false, format!("A{}: placeholder left: {p}", i + 1))
        } else {
            (true, format!("{n} criteria (A1 to A{n})"))
        },
    );

    push(Item::TestPerCriterion, {
        let missing: Vec<String> = plan.acceptance.iter().enumerate().filter(|(_, c)| blank(&c.test)).map(|(i, _)| format!("A{}", i + 1)).collect();
        let mut shared = None;
        'outer: for (i, a) in plan.acceptance.iter().enumerate() {
            for (j, b) in plan.acceptance.iter().enumerate().skip(i + 1) {
                if !blank(&a.test) && a.test.trim() == b.test.trim() {
                    shared = Some((i, j));
                    break 'outer;
                }
            }
        }
        if n == 0 {
            (false, "no criteria, so no tests: each criterion needs the command that checks it".into())
        } else if !missing.is_empty() {
            (false, format!("{} has no test: give each criterion the command that checks it", missing.join(", ")))
        } else if let Some((i, j)) = shared {
            (false, format!("A{} and A{} have the same test: each criterion needs its own", i + 1, j + 1))
        } else if let Some((i, p)) = plan.acceptance.iter().enumerate().find_map(|(i, c)| placeholder(&c.test).map(|p| (i, p))) {
            (false, format!("A{}'s test: placeholder left: {p}", i + 1))
        } else {
            (true, format!("{n} tests, one per criterion"))
        }
    });

    push(Item::NonGoals, stated(&plan.non_goals, "non-goals"));
    push(Item::Constraints, stated(&plan.constraints, "constraints"));
    push(Item::Dependencies, stated(&plan.dependencies, "dependencies"));

    push(
        Item::Baseline,
        match (&plan.baseline, baseline) {
            (None, _) => (false, "no task type: name the baseline (bug_fix, feature, ui_change or refactor)".into()),
            (Some(BaselineSpec::Research), _) => (false, research_note()),
            (Some(_), None) => (false, "not captured yet (it is captured once every other item is met)".into()),
            (Some(_), Some(b)) => (b.ok, if b.ok { format!("{} captured: {}", b.kind, b.note) } else { b.note.clone() }),
        },
    );
    Checklist { items }
}

fn research_note() -> String {
    "a research task has no baseline and no spec: it is a report, not a PR (the research path)".into()
}

/// Capture the baseline the plan names, in the planner's workspace. Copies
/// of screenshots go under `root`/[`BASELINE_DIR`]. Never errors: what went
/// wrong is the baseline's note, and `ok` is false.
pub async fn capture(plan: &PlanSpec, ws: &Workspace, root: &Path) -> Baseline {
    let Some(spec) = &plan.baseline else {
        return Baseline { note: "no task type named".into(), ..Default::default() };
    };
    let mut b = Baseline { kind: spec.kind().into(), ..Default::default() };
    let (ok, note) = match spec {
        BaselineSpec::Research => (false, research_note()),
        BaselineSpec::BugFix { repro } => {
            if blank(repro) {
                (false, "bug_fix needs a repro: the command that shows the bug".into())
            } else {
                let r = run(ws, repro).await;
                let passes = r.exit == 0;
                b.runs.push(r);
                if passes {
                    (false, "the repro passes today, so it does not reproduce the bug: write one that fails on the bug".into())
                } else {
                    (true, "the repro fails today".into())
                }
            }
        }
        BaselineSpec::Feature => {
            if plan.acceptance.is_empty() || plan.acceptance.iter().any(|c| blank(&c.test)) {
                (false, "a feature's baseline is its acceptance tests failing today: every criterion needs a test".into())
            } else {
                let mut passing = vec![];
                for (i, c) in plan.acceptance.iter().enumerate() {
                    let r = run(ws, &c.test).await;
                    if r.exit == 0 {
                        passing.push(format!("A{}", i + 1));
                    }
                    b.runs.push(r);
                }
                if passing.is_empty() {
                    (true, format!("{} acceptance tests fail today", plan.acceptance.len()))
                } else {
                    let s = passing.iter().map(|a| format!("{a}'s test")).collect::<Vec<_>>().join(", ");
                    (false, format!("{s} passes today, so it proves nothing about the feature: make it fail until the feature exists"))
                }
            }
        }
        BaselineSpec::UiChange { capture, screenshots } => ui(ws, root, capture.as_deref(), screenshots, &mut b).await,
        BaselineSpec::Refactor { suite, perf } => {
            if blank(suite) {
                (false, "a refactor's baseline is the existing suite: name the command".into())
            } else {
                let r = run(ws, suite).await;
                let (exit, ms) = (r.exit, r.ms);
                b.runs.push(r);
                if exit != 0 {
                    (false, format!("the existing suite fails today (exit {exit}): a refactor starts from green"))
                } else {
                    match perf.as_deref().filter(|p| !blank(p)) {
                        None => (true, format!("suite passes in {ms} ms")),
                        Some(p) => {
                            let r = run(ws, p).await;
                            let exit = r.exit;
                            b.runs.push(r);
                            if exit != 0 {
                                (false, format!("the perf command failed (exit {exit}): no performance numbers"))
                            } else {
                                (true, format!("suite passes in {ms} ms; perf numbers recorded"))
                            }
                        }
                    }
                }
            }
        }
    };
    b.ok = ok;
    b.note = note;
    b
}

async fn ui(ws: &Workspace, root: &Path, cmd: Option<&str>, shots: &[String], b: &mut Baseline) -> (bool, String) {
    if let Some(c) = cmd.filter(|c| !blank(c)) {
        let r = run(ws, c).await;
        let exit = r.exit;
        b.runs.push(r);
        if exit != 0 {
            return (false, format!("the screenshot command failed (exit {exit})"));
        }
    }
    if shots.is_empty() {
        return (false, "a UI change's baseline is screenshots of the current state: list them".into());
    }
    // Check every file before copying any, so a refusal leaves nothing behind.
    let mut found = vec![];
    for p in shots {
        let path = match ws.resolve(p) {
            Ok(path) => path,
            Err(e) => return (false, format!("{p}: {e:#}")),
        };
        let bytes = match std::fs::read(&path) {
            Ok(x) => x,
            Err(_) => return (false, format!("{p} does not exist: take the screenshot first")),
        };
        let png = bytes.starts_with(b"\x89PNG\r\n\x1a\n");
        let jpeg = bytes.starts_with(&[0xFF, 0xD8, 0xFF]);
        if !png && !jpeg {
            return (false, format!("{p} is not a PNG or JPEG"));
        }
        found.push((p.clone(), bytes));
    }
    let dir = root.join(BASELINE_DIR);
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return (false, format!("could not keep the screenshots: {e}"));
    }
    for (i, (p, bytes)) in found.into_iter().enumerate() {
        let name: String = Path::new(&p)
            .file_name()
            .map(|n| n.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect())
            .unwrap_or_else(|| "shot".into());
        let stored = format!("{BASELINE_DIR}/{:02}-{name}", i + 1);
        if let Err(e) = std::fs::write(root.join(&stored), &bytes) {
            return (false, format!("could not keep {p}: {e}"));
        }
        b.shots.push(Shot { path: p, stored, sha256: sha256_hex(&bytes), bytes: bytes.len() as u64 });
    }
    (true, format!("{} screenshots of the current state kept", b.shots.len()))
}

async fn run(ws: &Workspace, cmd: &str) -> Run {
    let start = Instant::now();
    let (exit, output) = match ws.sh(cmd).await {
        Ok(x) => x,
        Err(e) => (-1, format!("{e:#}")),
    };
    let skip = output.chars().count().saturating_sub(OUTPUT_TAIL);
    Run { command: cmd.to_string(), exit, ms: start.elapsed().as_millis() as u64, output: output.chars().skip(skip).collect() }
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn list(items: &[String]) -> String {
    items.iter().map(|s| format!("- {}", s.trim())).collect::<Vec<_>>().join("\n")
}

/// spec.md, from the checklist alone. Its `## Acceptance` lines are numbered
/// A1, A2, … the way the coverage gate (spec.rs) reads them.
pub fn render(plan: &PlanSpec, baseline: &Baseline, checklist: &Checklist) -> String {
    let mut s = format!("# {}\n\n## Goal\n\n{}\n\n## Acceptance\n\n", plan.title.trim(), plan.goal.trim());
    s.push_str(&list(&plan.acceptance.iter().map(|c| c.criterion.clone()).collect::<Vec<_>>()));
    s.push_str("\n\n## Tests\n\n| # | Criterion | Test |\n|---|---|---|\n");
    for (i, c) in plan.acceptance.iter().enumerate() {
        s.push_str(&format!("| A{} | {} | `{}` |\n", i + 1, cell(c.criterion.trim()), cell(c.test.trim())));
    }
    s.push_str(&format!("\n## Non-goals\n\n{}\n\n## Constraints\n\n{}\n\n## Dependencies\n\n{}\n\n", list(&plan.non_goals), list(&plan.constraints), list(&plan.dependencies)));
    s.push_str(&format!("## Baseline\n\n{}: {}\n", baseline.kind, baseline.note));
    for r in &baseline.runs {
        s.push_str(&format!("\n- `{}`: exit {} in {} ms", cell(&r.command), r.exit, r.ms));
    }
    for sh in &baseline.shots {
        s.push_str(&format!("\n- {} (sha256 {}, {} bytes): {}", sh.path, sh.sha256, sh.bytes, sh.stored));
    }
    s.push_str(&format!("\n\n## Checklist\n\n{}\n", checklist.markdown()));
    s
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The user's reply, read in code. Only a plain yes is one: anything with a
/// condition or a change in it is not.
pub fn parse_yes(text: &str) -> bool {
    let t: String = text.trim().to_lowercase().trim_end_matches(['.', '!']).trim().to_string();
    ["yes", "y", "yep", "yeah", "ok", "okay", "approve", "approved", "lgtm", "go ahead", "go", "oui"].contains(&t.as_str())
}

/// One typed question per checklist item, for a decision model.
pub fn questions() -> Vec<Question> {
    Item::ALL.iter().map(|i| Question { id: format!("checklist.{}", i.key()), options: vec![MET.into(), MISSING.into()] }).collect()
}

/// Put a checklist the rules passed to a decision model. `Some(reason)`
/// means it refuses; `None` lets the rules stand. A checklist the rules
/// refused is not asked about: a model can block, never open. An unusable
/// answer is an error, which the caller must not read as a pass or a block.
pub fn score(model: &dyn DecisionModel, spec: &str, rules: &Checklist, threshold: f64) -> Result<Option<String>> {
    if !rules.passed() {
        return Ok(None);
    }
    let answers = decide_checked(model, spec, &questions())?;
    let mut doubts = vec![];
    for (item, a) in Item::ALL.iter().zip(&answers) {
        let p = a.probs.iter().find(|(o, _)| o == MET).map(|(_, p)| *p).ok_or_else(|| anyhow!("no `{MET}` for {}", item.key()))?;
        if p < threshold {
            doubts.push(format!("{} (p met {p:.2} < {threshold:.2})", item.label()));
        }
    }
    Ok((!doubts.is_empty()).then(|| format!("decision model `{}` refused the checklist: {}", model.name(), doubts.join(", "))))
}

pub fn submit_spec_tool() -> ToolDef {
    ToolDef {
        name: "submit_spec",
        description: "The long path's planning stage: hand in the plan as a typed checklist once the interview is over. Code checks every item, captures the \"before\" baseline by running commands, writes spec.md and asks the user for a yes. create_tickets is refused until the checklist passed and the user said yes. A refusal lists what is missing: ask the user, then submit again.",
        schema: serde_json::json!({"type": "object", "properties": {
            "title": {"type": "string"},
            "goal": {"type": "string", "description": "what the task achieves and for whom"},
            "acceptance": {"type": "array", "items": {"type": "object", "properties": {
                "criterion": {"type": "string"},
                "test": {"type": "string", "description": "the shell command that checks this criterion (exit 0 = met); its own, not shared"}
            }, "required": ["criterion", "test"]}},
            "non_goals": {"type": "array", "items": {"type": "string"}, "description": "what this will not do; [\"None\"] if truly none"},
            "constraints": {"type": "array", "items": {"type": "string"}, "description": "limits the work must respect; [\"None\"] if truly none"},
            "dependencies": {"type": "array", "items": {"type": "string"}, "description": "what it needs from elsewhere; [\"None\"] if truly none"},
            "baseline": {"type": "object", "description": "the task type and its \"before\" baseline. kind: bug_fix {repro: a command that fails today}, feature {} (the acceptance tests must fail today), ui_change {capture?: a command that writes the screenshots, screenshots: [paths]}, refactor {suite: a command that passes today, perf?: a command printing performance numbers}",
                "properties": {"kind": {"type": "string", "enum": ["bug_fix", "feature", "ui_change", "refactor"]}},
                "required": ["kind"]}
        }, "required": ["title", "goal", "acceptance", "non_goals", "constraints", "dependencies", "baseline"]}),
    }
}

/// Why the long path may not make tickets yet, or None when it may: the
/// checklist passed and the spec on disk is the one the user said yes to.
pub fn ticket_refusal(state: &PlanningState, root: &Path) -> Option<String> {
    let on_disk = std::fs::read(root.join(SPEC)).ok().map(|b| sha256_hex(&b));
    match (&state.approved_sha256, &on_disk) {
        (Some(a), Some(d)) if a == d && state.passed => return None,
        (Some(_), _) if state.passed => {
            return Some(format!("{SPEC} changed since the user approved it, so their yes no longer holds: call submit_spec again and ask for a new yes before create_tickets."))
        }
        _ => {}
    }
    Some(if state.spec_sha256.is_none() || !state.passed {
        "The long path plans first (the planning stage): grill the user until you can fill every checklist item, then call submit_spec. create_tickets opens once the checklist passes and the user says yes.".to_string()
    } else if state.awaiting_yes {
        format!("{SPEC} passed its checklist, but the user has not said yes yet: end your turn and wait for their answer.")
    } else {
        format!("The user did not approve {SPEC} (they did not answer yes): revise it with them, call submit_spec again and ask for a yes.")
    })
}

/// The user's next message after a spec was put to them. Returns the text to
/// pass on to the planner and what to log, and updates `state`.
pub fn answer(state: &mut PlanningState, root: &Path, text: &str) -> (String, String) {
    state.awaiting_yes = false;
    if !parse_yes(text) {
        return (text.to_string(), format!("That is not a yes, so {SPEC} stays unapproved: the planner revises it and asks again."));
    }
    let on_disk = std::fs::read(root.join(SPEC)).ok().map(|b| sha256_hex(&b));
    if on_disk.is_none() || on_disk != state.spec_sha256 {
        return (
            format!("{text}\n\n(Backspace: the user said yes, but {SPEC} changed after it was put to them, so nothing was approved. Call submit_spec again.)"),
            format!("{SPEC} changed after it was put to you, so your yes approved nothing: the planner submits it again."),
        );
    }
    state.approved_sha256 = on_disk;
    (
        format!("{text}\n\n(Backspace: the user approved {SPEC}. create_tickets is open: plan from the spec.)"),
        format!("You approved {SPEC}; the planner may make tickets from it."),
    )
}
