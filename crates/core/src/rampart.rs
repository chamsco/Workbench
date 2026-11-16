//! Builder and Rampart (PLAN §7, milestone M5).
//!
//! At planning time two orchestrators are kept apart: Builder (the lead and
//! its workers) builds; Rampart writes the hard tests (the ramparts) Builder
//! must pass. This module holds what is enforced in code:
//!
//! * Rampart's view is the approved spec.md and the baseline, and nothing of
//!   Builder's ([`brief`]).
//! * A suite is vetted before it is sealed: every acceptance criterion has a
//!   test, every test fails on the baseline, and every test is run
//!   [`RampartConfig::runs`] times to catch flakiness ([`vet`]).
//! * A sealed suite is hash-locked ([`Seal`], [`verify`]). Builder cannot
//!   change, skip or delete a sealed test without its delivery being refused.
//! * Mutation testing on the changed area scores how strong the ramparts
//!   are once Builder passes them ([`mutation_score`]).
//! * When to stop retrying on cheap rungs (§7.4): N failures, the same
//!   failure twice, the cheap budget spent, or a frontier call expected to be
//!   cheaper than another retry. Then ONE frontier call with a failure digest,
//!   then a human ([`Limit`]).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ladder::RungStats;
use crate::planning::{sha256_hex, PlanningState};
use crate::tools::Workspace;

/// Everything Rampart owns, under the project's `.backspace`.
pub const DIR: &str = ".backspace/rampart";
/// The seal: what was sealed and its hashes.
pub const SEAL: &str = ".backspace/rampart/seal.json";
/// Copies of the sealed files as they were sealed.
pub const SEALED_DIR: &str = ".backspace/rampart/sealed";
/// Rampart's own checkout of the baseline: the code before Builder.
pub const TREE: &str = ".backspace/rampart/tree";
/// Rampart's brief.
pub const PROMPT: &str = include_str!("prompts/rampart.md");
/// The failure reason a delivery carries when the sealed tests did not pass.
pub const SEALED_FAILED: &str = "sealed tests failed";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct RampartConfig {
    /// Runs per test, on the baseline and on Builder's code (flake check).
    pub runs: u32,
    /// The first this-many suites wait for a person's yes before they are
    /// sealed (§7.2 human spot-check).
    pub spot_checks: u32,
    /// Sealed-test failures on cheap rungs before the one frontier call.
    pub max_failures: u32,
    /// Spend on cheap rungs, per ticket, before the one frontier call.
    pub cheap_budget_usd: Option<f64>,
    /// Mutants tried on the changed area, at most.
    pub max_mutants: usize,
}

impl Default for RampartConfig {
    fn default() -> Self {
        RampartConfig { runs: 3, spot_checks: 3, max_failures: 3, cheap_budget_usd: None, max_mutants: 12 }
    }
}

/// One sealed test: a file Rampart wrote, the command that runs it, and the
/// acceptance criteria (A1…) it checks.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TestFile {
    pub path: String,
    pub content: String,
    pub command: String,
    pub criteria: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Suite {
    pub tests: Vec<TestFile>,
}

impl Suite {
    pub fn from_json(v: &Value) -> Result<Suite> {
        todo!()
    }

    /// One hash over every path, content and command, in order.
    pub fn sha256(&self) -> String {
        todo!()
    }
}

/// A checkout-relative path, normalised: `./a/b` is `a/b`.
fn norm(p: &str) -> String {
    p.trim().trim_start_matches("./").replace('\\', "/")
}

/// The acceptance criteria spec.md numbers (A1, A2, …), from its test table.
pub fn criteria(spec_md: &str) -> Vec<String> {
    todo!()
}

/// Why `path` may not be a sealed test, if it may not.
fn path_gap(path: &str) -> Option<String> {
    let p = norm(path);
    if p.is_empty() {
        return Some("a test has an empty path".into());
    }
    let bad = Path::new(&p).is_absolute()
        || p.starts_with('/')
        || p.contains(':')
        || p.split('/').any(|c| c == ".." || c.is_empty())
        || p.starts_with(".backspace")
        || p.starts_with(".git/")
        || p == ".git"
        || p.eq_ignore_ascii_case("backspace.toml");
    bad.then(|| format!("`{path}` is not a usable path: give one relative to the project root, outside .backspace and .git"))
}

/// What is wrong with a suite before anything runs: paths, duplicates, empty
/// tests, and criteria with no test (traceability).
pub fn shape_gaps(suite: &Suite, spec_md: &str) -> Vec<String> {
    todo!()
}

/// Everything Rampart is given: the approved spec and the baseline.
#[derive(Clone, Debug, PartialEq)]
pub struct Brief {
    pub spec: String,
    pub spec_sha256: String,
    pub baseline: String,
}

/// Rampart's view, only while spec.md is the one the user said yes to.
pub fn brief(state: &PlanningState, root: &Path) -> Result<Brief> {
    todo!()
}

impl Brief {
    pub fn message(&self) -> String {
        todo!()
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    FailsOnBaseline,
    PassesOnBaseline,
    Flaky,
}

/// One test's runs on the baseline.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TestVet {
    pub path: String,
    pub command: String,
    pub exits: Vec<i32>,
    pub tail: String,
}

impl TestVet {
    pub fn verdict(&self) -> Verdict {
        todo!()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Vetting {
    pub tests: Vec<TestVet>,
}

impl Vetting {
    pub fn passed(&self) -> bool {
        todo!()
    }

    pub fn gaps(&self) -> Vec<String> {
        todo!()
    }
}

/// The end of a command's output kept with a run.
const TAIL: usize = 1200;

fn tail(s: &str, n: usize) -> String {
    let skip = s.chars().count().saturating_sub(n);
    s.chars().skip(skip).collect()
}

/// Write the suite's files into `dir`, returning what each path held before
/// (None: it did not exist), so they can be put back.
fn lay(dir: &Path, suite: &Suite) -> Result<Vec<(std::path::PathBuf, Option<Vec<u8>>)>> {
    let mut saved = Vec::new();
    for t in &suite.tests {
        let p = dir.join(norm(&t.path));
        saved.push((p.clone(), std::fs::read(&p).ok()));
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, &t.content).with_context(|| format!("writing {}", t.path))?;
    }
    Ok(saved)
}

fn restore(saved: Vec<(std::path::PathBuf, Option<Vec<u8>>)>) {
    for (p, before) in saved.into_iter().rev() {
        match before {
            Some(b) => {
                let _ = std::fs::write(&p, b);
            }
            None => {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

/// Run every test `runs` times on the baseline checkout `tree`. The test files
/// are written for the runs and removed after, so the tree stays the baseline.
pub async fn vet(suite: &Suite, tree: &Workspace, runs: u32) -> Result<Vetting> {
    todo!()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SealedFile {
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
    pub command: String,
    pub criteria: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Seal {
    pub files: Vec<SealedFile>,
    pub suite_sha256: String,
    pub spec_sha256: String,
    /// The checkout Rampart wrote against (a commit, or "copy").
    pub base: String,
    pub rampart_model: String,
    pub vetting: Vetting,
    /// A person said yes to this suite before it was sealed.
    pub human_checked: bool,
    pub sealed_at: u64,
}

/// Seal a vetted suite. Refused unless the vetting passed.
pub fn seal(suite: &Suite, vetting: &Vetting, spec_sha256: &str, base: &str, model: &str, human_checked: bool) -> Result<Seal> {
    todo!()
}

/// Write seal.json and the sealed copies under `root`.
pub fn write(root: &Path, suite: &Suite, seal: &Seal) -> Result<()> {
    todo!()
}

/// Copy the sealed files from `root`'s sealed copies into `dir`.
pub fn place(dir: &Path, root: &Path, seal: &Seal) -> Result<()> {
    todo!()
}

/// Every sealed file in `dir` that is missing or differs from its hash.
pub fn verify(dir: &Path, seal: &Seal) -> Vec<String> {
    todo!()
}

/// Is `path` (relative to a checkout) one of the sealed files?
pub fn is_sealed(seal: &Seal, path: &str) -> bool {
    todo!()
}

/// A suite vetted and waiting for a person's yes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pending {
    pub suite: Suite,
    pub vetting: Vetting,
    pub model: String,
    pub base: String,
}

/// Where Builder and Rampart stand. Saved with the project.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct RampartState {
    /// The Rampart agent, once started.
    pub agent: Option<usize>,
    pub pending: Option<Pending>,
    pub awaiting_yes: bool,
    pub seal: Option<Seal>,
    /// Suites a person has said yes to so far.
    pub suites_checked: u32,
    /// Builder's sealed-test attempts, per ticket.
    pub limits: BTreeMap<String, Limit>,
}

/// Why Builder may not start work yet, or None once the suite is sealed.
pub fn work_refusal(state: &RampartState) -> Option<String> {
    todo!()
}

pub fn needs_spot_check(state: &RampartState, cfg: &RampartConfig) -> bool {
    todo!()
}

/// What the user is asked about a vetted suite.
pub fn ask(p: &Pending) -> String {
    todo!()
}

/// The user's answer to a suite put to them. Returns (text for the lead, note
/// for the log, approved).
pub fn answer(state: &mut RampartState, text: &str) -> (String, String, bool) {
    todo!()
}

// ------------------------------------------------------------ §7.4 limits

/// One failed run of the sealed tests.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Attempt {
    pub signature: String,
    pub failing: Vec<String>,
    pub tail: String,
    pub cost_usd: f64,
}

/// (command, exit, output) of each sealed test that failed.
pub fn attempt(failed: &[(String, i32, String)], cost_usd: f64) -> Attempt {
    todo!()
}

/// One line with what changes between identical runs taken out: hex ids,
/// numbers (timings, ports, temp names).
fn normalise(line: &str) -> String {
    let mut out = String::new();
    for word in line.split_whitespace() {
        let mut w = String::new();
        let chars: Vec<char> = word.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            // A run of 7+ hex characters with a digit in it is an id.
            let j = (i..chars.len()).find(|&k| !chars[k].is_ascii_hexdigit()).unwrap_or(chars.len());
            if j - i >= 7 && chars[i..j].iter().any(|c| c.is_ascii_digit()) {
                w.push_str("<hex>");
                i = j;
                continue;
            }
            if chars[i].is_ascii_digit() {
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                w.push('#');
                continue;
            }
            w.push(chars[i]);
            i += 1;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&w);
    }
    out
}

/// The failure, with what changes between identical runs (numbers, timings,
/// hashes, temp paths) taken out.
pub fn signature(failed: &[(String, i32, String)]) -> String {
    todo!()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    Failures(u32),
    Stagnation,
    CheapBudget { spent: f64, cap: f64 },
    CheaperToEscalate { cheap: f64, frontier: f64 },
}

impl Trigger {
    pub fn describe(&self) -> String {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Next {
    Retry,
    Escalate(Trigger),
    HandOff,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Limit {
    pub attempts: Vec<Attempt>,
    pub trigger: Option<Trigger>,
    /// The one frontier model it was escalated to.
    pub escalated_to: Option<String>,
    pub handed_off: bool,
}

impl Limit {
    /// Record a failed attempt and say what happens next.
    /// `frontier_call_usd` is the estimated cost of one frontier call.
    pub fn record(&mut self, a: Attempt, cfg: &RampartConfig, frontier_call_usd: Option<f64>) -> Next {
        todo!()
    }

    pub fn mark_escalated(&mut self, model: &str) {
        todo!()
    }

    /// What the failed attempts before the escalation cost.
    pub fn cheap_spent(&self) -> f64 {
        todo!()
    }
}

/// What the one frontier call and then the human are given.
pub fn digest(goal: &str, seal: &Seal, limit: &Limit) -> String {
    todo!()
}

/// The longest digest, in characters.
pub const DIGEST_MAX: usize = 6000;

// --------------------------------------------------------------- mutation

#[derive(Clone, Debug, PartialEq)]
pub struct Mutant {
    pub path: String,
    pub line: usize,
    pub from: String,
    pub to: String,
    pub content: String,
}

/// Operators swapped by a mutant, tried in this order on each line.
const SWAPS: [(&str, &str); 12] = [
    ("==", "!="),
    ("!=", "=="),
    ("<=", ">"),
    (">=", "<"),
    ("&&", "||"),
    ("||", "&&"),
    (" < ", " >= "),
    (" > ", " <= "),
    (" + ", " - "),
    (" - ", " + "),
    ("true", "false"),
    ("false", "true"),
];

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with('#') || t.starts_with("/*") || t.starts_with('*') || t.starts_with("--")
}

fn word_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Every single-site mutation of one line.
fn line_mutants(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (from, to) in SWAPS {
        let words = from.chars().all(|c| c.is_alphabetic());
        let mut start = 0;
        while let Some(i) = line[start..].find(from) {
            let at = start + i;
            let end = at + from.len();
            let bounded = !words || (!word_char(line[..at].chars().last()) && !word_char(line[end..].chars().next()));
            // "==" inside "===" or "!==" is left alone; so is "<=" inside "<=>".
            let glued = !words && (line[end..].starts_with('=') || line[..at].ends_with(['=', '!', '<', '>']));
            if bounded && !glued {
                out.push(format!("{}{to}{}", &line[..at], &line[end..]));
            }
            start = end;
        }
    }
    // Integer literals: n -> n + 1.
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'.')) {
            let mut j = i;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let tail_ok = j == b.len() || !(b[j].is_ascii_alphabetic() || b[j] == b'_' || b[j] == b'.');
            if tail_ok {
                if let Ok(n) = line[i..j].parse::<u64>() {
                    out.push(format!("{}{}{}", &line[..i], n.saturating_add(1), &line[j..]));
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Mutants of `content` on the given 1-based lines, at most `max`.
pub fn mutants(path: &str, content: &str, lines: &[usize], max: usize) -> Vec<Mutant> {
    todo!()
}

/// (file, added or changed lines) from `git diff -U0` output.
pub fn changed_lines(diff: &str) -> Vec<(String, Vec<usize>)> {
    todo!()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct MutationScore {
    pub total: usize,
    pub killed: usize,
    pub survived: Vec<String>,
}

impl MutationScore {
    pub fn score(&self) -> Option<f64> {
        todo!()
    }
}

/// Mutate the changed area of `ws` and run the sealed tests on each mutant.
/// Every file is put back as it was.
pub async fn mutation_score(ws: &Workspace, seal: &Seal, changed: &[(String, Vec<usize>)], max: usize) -> Result<MutationScore> {
    todo!()
}
