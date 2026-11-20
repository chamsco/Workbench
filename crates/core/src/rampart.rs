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
        let tests = v["tests"].as_array().context("`tests` must be a list of {path, content, command, criteria}")?;
        let mut out = Vec::new();
        for (i, t) in tests.iter().enumerate() {
            let s = |k: &str| t[k].as_str().map(String::from).with_context(|| format!("test {} needs `{k}` (a string)", i + 1));
            let criteria = t["criteria"]
                .as_array()
                .with_context(|| format!("test {} needs `criteria`, the acceptance criteria it checks (A1, A2, …)", i + 1))?
                .iter()
                .map(|c| c.as_str().map(|c| c.trim().to_uppercase()).with_context(|| format!("test {}: every criterion is a string such as \"A1\"", i + 1)))
                .collect::<Result<Vec<_>>>()?;
            out.push(TestFile { path: s("path")?, content: s("content")?, command: s("command")?, criteria });
        }
        Ok(Suite { tests: out })
    }

    /// One hash over every path, content and command, in order.
    pub fn sha256(&self) -> String {
        let mut b = Vec::new();
        for t in &self.tests {
            for part in [&t.path, &t.content, &t.command] {
                b.extend_from_slice(&(part.len() as u64).to_le_bytes());
                b.extend_from_slice(part.as_bytes());
            }
            b.extend_from_slice(t.criteria.join(",").as_bytes());
            b.push(0);
        }
        sha256_hex(&b)
    }
}

/// A checkout-relative path, normalised: `./a/b` is `a/b`.
fn norm(p: &str) -> String {
    p.trim().trim_start_matches("./").replace('\\', "/")
}

/// The acceptance criteria spec.md numbers (A1, A2, …), from its test table.
pub fn criteria(spec_md: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in spec_md.lines() {
        let Some(rest) = line.trim().strip_prefix("| A") else { continue };
        let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !n.is_empty() && rest[n.len()..].trim_start().starts_with('|') {
            let id = format!("A{n}");
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
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
    let known = criteria(spec_md);
    let mut gaps = Vec::new();
    if suite.tests.is_empty() {
        gaps.push("the suite is empty".to_string());
    }
    let mut seen: Vec<String> = Vec::new();
    for t in &suite.tests {
        if let Some(g) = path_gap(&t.path) {
            gaps.push(g);
        }
        let p = norm(&t.path);
        if seen.contains(&p) && !p.is_empty() {
            gaps.push(format!("`{p}` is given twice"));
        }
        seen.push(p.clone());
        if t.content.trim().is_empty() {
            gaps.push(format!("`{p}` is empty"));
        }
        if t.command.trim().is_empty() {
            gaps.push(format!("`{p}` has no command to run it"));
        }
        if t.criteria.is_empty() {
            gaps.push(format!("`{p}` names no acceptance criterion"));
        }
        for c in &t.criteria {
            if !known.contains(c) {
                gaps.push(format!("`{p}` names {c}, which is not a criterion of spec.md ({})", known.join(", ")));
            }
        }
    }
    for c in &known {
        if !suite.tests.iter().any(|t| t.criteria.contains(c)) {
            gaps.push(format!("{c} has no test: every acceptance criterion needs at least one"));
        }
    }
    gaps
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
    if let Some(why) = crate::planning::ticket_refusal(state, root) {
        bail!("Rampart starts from an approved {}: {why}", crate::planning::SPEC);
    }
    let spec = std::fs::read_to_string(root.join(crate::planning::SPEC)).with_context(|| format!("reading {}", crate::planning::SPEC))?;
    let baseline = std::fs::read_to_string(root.join(crate::planning::BASELINE)).unwrap_or_else(|_| "{}".into());
    Ok(Brief { spec_sha256: sha256_hex(spec.as_bytes()), spec, baseline })
}

impl Brief {
    pub fn message(&self) -> String {
        format!(
            "# spec.md (approved, sha256 {})\n\n{}\n\n# The \"before\" baseline (baseline.json)\n\n```json\n{}\n```\n\nYour directory is the code as it was at the baseline. Write the suite and call submit_ramparts.",
            self.spec_sha256,
            self.spec.trim_end(),
            self.baseline.trim_end()
        )
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
        let fails = self.exits.iter().filter(|e| **e != 0).count();
        if fails == self.exits.len() && fails > 0 {
            Verdict::FailsOnBaseline
        } else if fails == 0 {
            Verdict::PassesOnBaseline
        } else {
            Verdict::Flaky
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Vetting {
    pub tests: Vec<TestVet>,
}

impl Vetting {
    pub fn passed(&self) -> bool {
        !self.tests.is_empty() && self.gaps().is_empty()
    }

    pub fn gaps(&self) -> Vec<String> {
        self.tests
            .iter()
            .filter_map(|t| {
                let exits = t.exits.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                match t.verdict() {
                    Verdict::FailsOnBaseline => None,
                    Verdict::PassesOnBaseline => Some(format!("`{}` passes on the baseline (exits {exits}), so it proves nothing: it must fail before the change", t.path)),
                    Verdict::Flaky => Some(format!("`{}` is flaky on the baseline (exits {exits}): it must fail every run\n{}", t.path, t.tail)),
                }
            })
            .collect()
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
    let saved = lay(&tree.root, suite)?;
    let mut out = Vec::new();
    for t in &suite.tests {
        let mut exits = Vec::new();
        let mut last = String::new();
        for _ in 0..runs.max(1) {
            let (code, o) = tree.sh(&t.command).await.unwrap_or_else(|e| (-1, format!("{e:#}")));
            exits.push(code);
            last = o;
        }
        out.push(TestVet { path: norm(&t.path), command: t.command.clone(), exits, tail: tail(&last, TAIL) });
    }
    restore(saved);
    Ok(Vetting { tests: out })
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
    if !vetting.passed() {
        bail!("the suite cannot be sealed: {}", vetting.gaps().join("; "));
    }
    if vetting.tests.len() != suite.tests.len() || vetting.tests.iter().zip(&suite.tests).any(|(v, t)| v.path != norm(&t.path) || v.command != t.command) {
        bail!("the suite cannot be sealed: it is not the suite that was vetted");
    }
    Ok(Seal {
        files: suite
            .tests
            .iter()
            .map(|t| SealedFile { path: norm(&t.path), sha256: sha256_hex(t.content.as_bytes()), bytes: t.content.len(), command: t.command.clone(), criteria: t.criteria.clone() })
            .collect(),
        suite_sha256: suite.sha256(),
        spec_sha256: spec_sha256.to_string(),
        base: base.to_string(),
        rampart_model: model.to_string(),
        vetting: vetting.clone(),
        human_checked,
        sealed_at: crate::chat::now_ms(),
    })
}

/// Write seal.json and the sealed copies under `root`.
pub fn write(root: &Path, suite: &Suite, seal: &Seal) -> Result<()> {
    let dir = root.join(SEALED_DIR);
    let _ = std::fs::remove_dir_all(&dir);
    for t in &suite.tests {
        let p = dir.join(norm(&t.path));
        std::fs::create_dir_all(p.parent().unwrap_or(&dir))?;
        std::fs::write(&p, &t.content)?;
    }
    std::fs::write(root.join(SEAL), serde_json::to_string_pretty(seal)?)?;
    Ok(())
}

/// Copy the sealed files from `root`'s sealed copies into `dir`.
pub fn place(dir: &Path, root: &Path, seal: &Seal) -> Result<()> {
    for f in &seal.files {
        let bytes = std::fs::read(root.join(SEALED_DIR).join(&f.path)).with_context(|| format!("the sealed copy of {} is missing", f.path))?;
        if sha256_hex(&bytes) != f.sha256 {
            bail!("the sealed copy of {} does not match its seal", f.path);
        }
        let p = dir.join(&f.path);
        std::fs::create_dir_all(p.parent().unwrap_or(dir))?;
        std::fs::write(&p, bytes)?;
    }
    Ok(())
}

/// Every sealed file in `dir` that is missing or differs from its hash.
pub fn verify(dir: &Path, seal: &Seal) -> Vec<String> {
    seal.files
        .iter()
        .filter_map(|f| match std::fs::read(dir.join(&f.path)) {
            Err(_) => Some(format!("{} is missing (sealed tests cannot be deleted)", f.path)),
            Ok(b) if sha256_hex(&b) != f.sha256 => Some(format!("{} was changed (sealed tests are hash-locked)", f.path)),
            Ok(_) => None,
        })
        .collect()
}

/// The seal cut down to the tests for `covers` (acceptance criteria such as
/// "A1"): what a ticket that delivers only those criteria must pass. A ticket
/// that names none is held to the whole suite.
pub fn scoped(seal: &Seal, covers: &[String]) -> Seal {
    let covers: Vec<String> = covers.iter().map(|c| c.trim().to_uppercase()).collect();
    if covers.is_empty() {
        return seal.clone();
    }
    let mut s = seal.clone();
    s.files.retain(|f| f.criteria.iter().any(|c| covers.contains(c)));
    if s.files.is_empty() {
        return seal.clone();
    }
    s
}

/// Is `path` (relative to a checkout) one of the sealed files?
pub fn is_sealed(seal: &Seal, path: &str) -> bool {
    let p = norm(path);
    seal.files.iter().any(|f| f.path == p)
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
    /// The checkout Rampart writes against: a commit, or "copy".
    pub base: Option<String>,
    pub pending: Option<Pending>,
    pub awaiting_yes: bool,
    pub seal: Option<Seal>,
    /// Suites a person has said yes to so far.
    pub suites_checked: u32,
    /// Builder's sealed-test attempts, per ticket.
    pub limits: BTreeMap<String, Limit>,
    /// The long task's own tickets (planned from spec.md): the ones held to
    /// the seal. A task started alongside (`start_task`) is not one of them.
    pub tickets: Vec<String>,
}

/// Why Builder may not start work yet, or None once the suite is sealed.
pub fn work_refusal(state: &RampartState) -> Option<String> {
    if state.seal.is_some() {
        return None;
    }
    Some(if state.awaiting_yes {
        "Rampart's tests are waiting for the user's yes before they are sealed; workers start once they are. End your turn and wait.".to_string()
    } else {
        "Workers cannot start yet: Rampart, an independent test writer, is writing the sealed tests from spec.md. You will be told when they are sealed; end your turn and wait.".to_string()
    })
}

pub fn needs_spot_check(state: &RampartState, cfg: &RampartConfig) -> bool {
    state.suites_checked < cfg.spot_checks
}

/// What the user is asked about a vetted suite.
pub fn ask(p: &Pending) -> String {
    let mut s = String::from("Rampart wrote these tests from the approved spec. Each failed on the baseline on every run:\n");
    for (t, v) in p.suite.tests.iter().zip(&p.vetting.tests) {
        s.push_str(&format!("\n- `{}` ({}): `{}`, exits {}", norm(&t.path), t.criteria.join(", "), t.command, v.exits.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ")));
    }
    s.push_str("\n\nOnce sealed they are hash-locked and Builder must pass them as they are. Reply yes to seal them; anything else sends them back to Rampart.");
    s
}

/// The user's answer to a suite put to them. Returns (text for the lead, note
/// for the log, approved).
pub fn answer(state: &mut RampartState, text: &str) -> (String, String, bool) {
    state.awaiting_yes = false;
    if !crate::planning::parse_yes(text) || state.pending.is_none() {
        state.pending = None;
        return (
            text.to_string(),
            "That is not a yes, so Rampart's tests are not sealed: Rampart writes them again.".to_string(),
            false,
        );
    }
    state.suites_checked += 1;
    (
        format!("{text}\n\n(Backspace: the user approved Rampart's tests and they are sealed. Call work_tickets.)"),
        "You approved Rampart's tests; they are sealed and Builder's workers may start.".to_string(),
        true,
    )
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
    Attempt {
        signature: signature(failed),
        failing: failed.iter().map(|(c, _, _)| c.clone()).collect(),
        tail: failed.iter().map(|(c, e, o)| format!("$ {c}  (exit {e})\n{}", tail(o, 800))).collect::<Vec<_>>().join("\n"),
        cost_usd,
    }
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
    let mut b = String::new();
    let mut sorted: Vec<&(String, i32, String)> = failed.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (cmd, exit, out) in sorted {
        b.push_str(&format!("{cmd}\u{0}{exit}\u{0}"));
        let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
        for l in &lines[lines.len().saturating_sub(40)..] {
            b.push_str(&normalise(l));
            b.push('\n');
        }
    }
    sha256_hex(b.as_bytes())[..16].to_string()
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
        match self {
            Trigger::Failures(n) => format!("the sealed tests failed {n} times on cheap rungs"),
            Trigger::Stagnation => "the same failure twice in a row (stagnation)".to_string(),
            Trigger::CheapBudget { spent, cap } => format!("the cheap-rung budget is spent (${spent:.2} of ${cap:.2})"),
            Trigger::CheaperToEscalate { cheap, frontier } => {
                format!("another cheap retry is expected to cost more than the frontier (${cheap:.4} vs ${frontier:.4} to green)")
            }
        }
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
        let repeat = self.attempts.last().is_some_and(|l| l.signature == a.signature);
        self.attempts.push(a);
        // One frontier call, then a person: never a second escalation.
        if self.escalated_to.is_some() || self.trigger.is_some() {
            self.handed_off = true;
            return Next::HandOff;
        }
        let n = self.attempts.len() as u32;
        let spent = self.cheap_spent();
        let trigger = if repeat {
            Some(Trigger::Stagnation)
        } else if n >= cfg.max_failures.max(1) {
            Some(Trigger::Failures(n))
        } else if let Some(cap) = cfg.cheap_budget_usd.filter(|c| spent >= *c) {
            Some(Trigger::CheapBudget { spent, cap })
        } else if let Some(f) = frontier_call_usd {
            // §3.4: expected spend to one success on each side, from what was
            // measured on this ticket; the frontier is unmeasured (p = 0.5).
            let cheap = RungStats { attempts: n, successes: 0, mean_cost_usd: spent / n as f64 };
            let frontier = RungStats { attempts: 0, successes: 0, mean_cost_usd: f };
            crate::ladder::escalate_sooner(&cheap, &frontier)
                .then(|| Trigger::CheaperToEscalate { cheap: cheap.expected_cost_to_green(), frontier: frontier.expected_cost_to_green() })
        } else {
            None
        };
        match trigger {
            Some(t) => {
                self.trigger = Some(t.clone());
                Next::Escalate(t)
            }
            None => Next::Retry,
        }
    }

    pub fn mark_escalated(&mut self, model: &str) {
        self.escalated_to = Some(model.to_string());
    }

    /// What the failed attempts before the escalation cost.
    pub fn cheap_spent(&self) -> f64 {
        let before = if self.escalated_to.is_some() { self.attempts.len().saturating_sub(1) } else { self.attempts.len() };
        self.attempts[..before].iter().map(|a| a.cost_usd).sum()
    }
}

/// What the one frontier call and then the human are given.
pub fn digest(goal: &str, seal: &Seal, limit: &Limit) -> String {
    let mut s = format!("Goal: {}\n", goal.trim());
    if let Some(t) = &limit.trigger {
        s.push_str(&format!("Why it stopped on cheap rungs: {}.\n", t.describe()));
    }
    s.push_str(&format!("Attempts so far: {}", limit.attempts.len()));
    if let Some(m) = &limit.escalated_to {
        s.push_str(&format!(" (the last on {m})"));
    }
    s.push_str("\n\nSealed tests (hash-locked; they cannot be changed):\n");
    for f in &seal.files {
        s.push_str(&format!("- `{}` checks {}: `{}`\n", f.path, f.criteria.join(", "), f.command));
    }
    s.push_str("\nFailures, oldest first:\n");
    for (i, a) in limit.attempts.iter().enumerate() {
        s.push_str(&format!("{}. {} (signature {})\n", i + 1, a.failing.join(", "), a.signature));
    }
    if let Some(last) = limit.attempts.last() {
        s.push_str("\nThe last failure:\n");
        s.push_str(&last.tail);
        s.push('\n');
    }
    if s.chars().count() > DIGEST_MAX {
        let keep: String = s.chars().take(DIGEST_MAX - 40).collect();
        s = format!("{keep}\n… (digest cut at {DIGEST_MAX} characters)");
    }
    s
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
    let all: Vec<&str> = content.split('\n').collect();
    let mut wanted: Vec<usize> = lines.to_vec();
    wanted.sort_unstable();
    wanted.dedup();
    let mut out = Vec::new();
    for n in wanted {
        let Some(line) = all.get(n.wrapping_sub(1)) else { continue };
        if n == 0 || is_comment(line) {
            continue;
        }
        for m in line_mutants(line) {
            if out.len() >= max {
                return out;
            }
            let mut copy: Vec<&str> = all.clone();
            copy[n - 1] = &m;
            out.push(Mutant { path: norm(path), line: n, from: line.to_string(), to: m.clone(), content: copy.join("\n") });
        }
    }
    out
}

/// (file, added or changed lines) from `git diff -U0` output.
pub fn changed_lines(diff: &str) -> Vec<(String, Vec<usize>)> {
    let mut out: Vec<(String, Vec<usize>)> = Vec::new();
    let mut file: Option<String> = None;
    for line in diff.lines() {
        if let Some(p) = line.strip_prefix("+++ ") {
            file = (p != "/dev/null").then(|| p.strip_prefix("b/").unwrap_or(p).to_string());
            continue;
        }
        let Some(h) = line.strip_prefix("@@ ") else { continue };
        let Some(f) = &file else { continue };
        let Some(plus) = h.split_whitespace().find(|w| w.starts_with('+')) else { continue };
        let mut it = plus[1..].split(',');
        let start: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let count: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or(1);
        if count == 0 {
            continue;
        }
        match out.iter_mut().find(|(p, _)| p == f) {
            Some((_, v)) => v.extend(start..start + count),
            None => out.push((f.clone(), (start..start + count).collect())),
        }
    }
    out
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct MutationScore {
    pub total: usize,
    pub killed: usize,
    pub survived: Vec<String>,
}

impl MutationScore {
    pub fn score(&self) -> Option<f64> {
        (self.total > 0).then(|| self.killed as f64 / self.total as f64)
    }
}

/// Mutate the changed area of `ws` and run the sealed tests on each mutant.
/// Every file is put back as it was.
pub async fn mutation_score(ws: &Workspace, seal: &Seal, changed: &[(String, Vec<usize>)], max: usize) -> Result<MutationScore> {
    let mut score = MutationScore::default();
    for (path, lines) in changed {
        if is_sealed(seal, path) || score.total >= max {
            continue;
        }
        let p = ws.root.join(norm(path));
        let Ok(original) = std::fs::read_to_string(&p) else { continue };
        for m in mutants(path, &original, lines, max - score.total) {
            std::fs::write(&p, &m.content)?;
            let mut killed = false;
            for f in &seal.files {
                let (code, _) = ws.sh(&f.command).await.unwrap_or((-1, String::new()));
                if code != 0 {
                    killed = true;
                    break;
                }
            }
            score.total += 1;
            if killed {
                score.killed += 1;
            } else {
                score.survived.push(format!("{}:{}: `{}` -> `{}`", m.path, m.line, m.from.trim(), m.to.trim()));
            }
        }
        std::fs::write(&p, &original)?;
    }
    Ok(score)
}

/// The lines of `new` that are added or changed against `old` (which may be
/// missing: then every line is new), by `git diff --no-index -U0`.
pub fn diff_lines(old: &Path, new: &Path) -> Vec<usize> {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let old_s = if old.is_file() { old.to_string_lossy().into_owned() } else { null.to_string() };
    let out = backspace_runner::cmd("git")
        .args(["diff", "--no-index", "--no-color", "-U0", "--", &old_s, &new.to_string_lossy()])
        .stdin(std::process::Stdio::null())
        .output();
    let Ok(out) = out else { return vec![] };
    let text = String::from_utf8_lossy(&out.stdout);
    changed_lines(&text).into_iter().flat_map(|(_, l)| l).collect()
}

/// Copy `from` into `to`, leaving out Backspace's own folder, git and build
/// output: Rampart's tree when the project is not a git checkout.
pub fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    const SKIP: [&str; 4] = [".backspace", ".git", "target", "node_modules"];
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let name = e.file_name();
        if SKIP.iter().any(|s| name == *s) {
            continue;
        }
        let ty = e.file_type()?;
        if ty.is_dir() {
            copy_tree(&e.path(), &to.join(&name))?;
        } else if ty.is_file() {
            std::fs::copy(e.path(), to.join(&name))?;
        }
    }
    Ok(())
}

/// The tools Rampart may call. A shell only inside a sandbox that isolates
/// (M5 decision 2): read-only on the project, writes only to its own baseline
/// checkout, no network, no secrets.
pub fn tools(shell: bool) -> Vec<&'static str> {
    if shell {
        vec!["read", "list", "bash", "submit_ramparts"]
    } else {
        vec!["read", "list", "submit_ramparts"]
    }
}

/// The cheap model Rampart starts on, from the allowed ones below the
/// frontier: the strongest, then the cheapest, preferring one that is not the
/// lead's model (M5 decision 1). With only the lead's, Rampart runs on it as a
/// separate agent.
pub fn pick_cheap<'a>(candidates: &[&'a crate::config::ModelSpec], lead: &str) -> Option<&'a crate::config::ModelSpec> {
    let best = |list: Vec<&'a crate::config::ModelSpec>| {
        list.into_iter().max_by(|a, b| a.tier.cmp(&b.tier).then(b.output_usd_per_mtok.total_cmp(&a.output_usd_per_mtok)))
    };
    best(candidates.iter().copied().filter(|m| m.id != lead).collect()).or_else(|| best(candidates.to_vec()))
}
