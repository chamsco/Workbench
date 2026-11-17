//! Verify and Ship (PRD §4.4-4.5): the two stages between Execute and Learn.
//!
//! Verify asks whether the run delivers the plan, not whether some tests
//! pass. It reads the plan's acceptance criteria (`plan.json`, written when
//! the spec passed), runs each criterion's command in the run's worktree,
//! checks every criterion has a ticket, that every ticket finished, that no
//! sealed test changed (rampart.rs) and that no test was deleted, skipped or
//! lost its asserts since the run started. The answer is kept as
//! `verify.json` plus a readable `VERIFY.md`, and pinned to the commit it
//! judged.
//!
//! Ship delivers exactly that commit: locally (the run branch, for you to
//! merge), as a pushed branch, or as a pull request through `gh`. It never
//! merges. Anything that changed after Verify sends the work back to Verify;
//! a missing `gh` or remote is reported, not worked around. The delivery
//! record (`delivery.json`, `DELIVERY.md`) says what went where and what is
//! still open.
//!
//! The commands Verify runs are the plan's own acceptance commands, the same
//! list the user approved before the baseline ran. They run in the run's
//! worktree, not sandboxed until M6.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::phase::{GateBy, Phase, PhaseState};
use crate::planning::{Criterion, PlanSpec};
use crate::ticket::TicketState;

/// The typed plan, as the planner submitted it (written with spec.md).
pub const PLAN_JSON: &str = ".backspace/planning/plan.json";
pub const VERIFY_MD: &str = ".backspace/VERIFY.md";
pub const VERIFY_JSON: &str = ".backspace/verify.json";
pub const DELIVERY_MD: &str = ".backspace/DELIVERY.md";
pub const DELIVERY_JSON: &str = ".backspace/delivery.json";
/// Kept from a command's output.
const TAIL: usize = 1500;

/// What Verify needs to know about a ticket.
#[derive(Clone, Debug)]
pub struct TicketFacts {
    pub key: String,
    pub state: TicketState,
    /// Acceptance criteria it delivers ("A1", …).
    pub covers: Vec<String>,
}

impl TicketFacts {
    pub fn of(t: &crate::ticket::Ticket) -> TicketFacts {
        TicketFacts { key: t.key.clone(), state: t.state, covers: t.covers.clone() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct CriterionResult {
    /// "A1", "A2", … in plan order.
    pub key: String,
    pub criterion: String,
    pub command: String,
    pub passed: bool,
    pub exit: Option<i32>,
    pub ms: u64,
    /// The end of what it printed.
    pub output: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct VerifyReport {
    pub passed: bool,
    /// The commit judged; Ship delivers only this one.
    pub head: String,
    /// What the run is compared against (where it started).
    pub base: String,
    pub criteria: Vec<CriterionResult>,
    /// Requirements not met, unfinished tickets, uncovered criteria, sealed
    /// tests changed.
    pub issues: Vec<String>,
    /// Tests deleted, skipped or with fewer asserts than at the base.
    pub weakened: Vec<String>,
    pub at: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// The run branch stays where it is; you merge it.
    #[default]
    Local,
    /// Pushed to `origin` as `cauldron/<slug>`.
    Branch,
    /// Pushed, then a pull request against your branch through `gh`.
    PullRequest,
}

impl Target {
    pub fn parse(s: &str) -> Result<Target> {
        match s {
            "local" => Ok(Target::Local),
            "branch" => Ok(Target::Branch),
            "pr" | "pull-request" | "pull_request" => Ok(Target::PullRequest),
            other => bail!("unknown ship target `{other}`: local, branch or pr"),
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct ShipOpts {
    pub target: Target,
    /// The GitHub CLI to use; `gh` on PATH when unset.
    pub gh: Option<PathBuf>,
}

/// What was delivered, where, on what evidence, and what is still open.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct DeliveryRecord {
    pub title: String,
    pub target: Target,
    pub commit: String,
    /// Where the commit is: the run branch (local) or the pushed branch.
    pub branch: String,
    /// "delivered"
    pub status: String,
    pub pr_url: Option<String>,
    /// "A1 passed: `cmd`", one per criterion.
    pub evidence: Vec<String>,
    pub limitations: Vec<String>,
    pub at: u64,
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(["-c", "user.name=Cauldron", "-c", "user.email=cauldron@localhost"])
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .output()
        .context("running git")?;
    if !out.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The run's checkout: the harness's run worktree when there is one, else
/// the project itself (shared isolation).
pub fn run_dir(root: &Path) -> PathBuf {
    let w = crate::git::worktree_path(root, "_run");
    if w.join(".git").exists() {
        w
    } else {
        root.to_path_buf()
    }
}

/// The plan Verify judges against: `plan.json`, or the Tests table of spec.md
/// for a plan written before plan.json existed.
pub fn load_plan(root: &Path) -> Option<PlanSpec> {
    if let Ok(t) = std::fs::read_to_string(root.join(PLAN_JSON)) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
            if let Ok(p) = PlanSpec::from_json(&v) {
                return Some(p);
            }
        }
    }
    let md = std::fs::read_to_string(root.join(crate::planning::SPEC)).ok()?;
    let title = md.lines().next().unwrap_or("").trim_start_matches("# ").to_string();
    let acceptance: Vec<Criterion> = md
        .lines()
        .filter(|l| l.starts_with("| A"))
        .filter_map(|l| {
            let cells: Vec<&str> = l.trim_matches('|').split('|').map(str::trim).collect();
            (cells.len() >= 3).then(|| Criterion { criterion: cells[1].to_string(), test: cells[2].trim_matches('`').to_string() })
        })
        .collect();
    Some(PlanSpec { title, acceptance, ..Default::default() })
}

fn shell(dir: &Path, cmd: &str) -> (Option<i32>, u64, String) {
    let t = std::time::Instant::now();
    let mut c = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", cmd]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    let out = c.current_dir(dir).stdin(std::process::Stdio::null()).output();
    let ms = t.elapsed().as_millis() as u64;
    match out {
        Err(e) => (None, ms, format!("could not start: {e}")),
        Ok(o) => {
            let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            let tail: String = text.chars().rev().take(TAIL).collect::<Vec<_>>().into_iter().rev().collect();
            (o.status.code(), ms, tail)
        }
    }
}

fn is_test_path(p: &str) -> bool {
    let l = p.to_ascii_lowercase();
    l.starts_with("tests/")
        || l.starts_with("test/")
        || l.contains("/tests/")
        || l.contains("/test/")
        || l.contains("__tests__")
        || l.contains("_test.")
        || l.contains(".test.")
        || l.contains(".spec.")
        || l.rsplit('/').next().is_some_and(|f| f.starts_with("test_"))
}

const SKIPS: [&str; 8] = ["#[ignore", ".skip(", "it.skip", "describe.skip", "xit(", "@pytest.mark.skip", "t.Skip(", "@Disabled"];

/// Tests deleted, skipped, or left with fewer asserts between `base` and `head`.
pub fn weakened_tests(dir: &Path, base: &str, head: &str) -> Vec<String> {
    let mut out = vec![];
    let Ok(changed) = git(dir, &["diff", "--name-status", "--no-renames", base, head]) else {
        return vec![format!("could not compare the run with where it started ({base})")];
    };
    for line in changed.lines() {
        let mut parts = line.split('\t');
        let (Some(status), Some(path)) = (parts.next(), parts.next()) else { continue };
        if !is_test_path(path) {
            continue;
        }
        if status.starts_with('D') {
            out.push(format!("{path} was removed"));
            continue;
        }
        let Ok(diff) = git(dir, &["diff", "-U0", base, head, "--", path]) else { continue };
        let added: Vec<&str> = diff.lines().filter(|l| l.starts_with('+') && !l.starts_with("+++")).collect();
        let removed: Vec<&str> = diff.lines().filter(|l| l.starts_with('-') && !l.starts_with("---")).collect();
        if let Some(s) = SKIPS.iter().find(|s| added.iter().any(|l| l.contains(*s))) {
            out.push(format!("{path} adds a skip ({}): an ignored test", s.trim_start_matches(['#', '[', '.', '@'])));
        }
        let asserts = |ls: &[&str]| ls.iter().filter(|l| l.contains("assert") || l.contains("expect(")).count();
        let (lost, gained) = (asserts(&removed), asserts(&added));
        if lost > gained {
            out.push(format!("{path} has {} fewer assert(s) than when the run started", lost - gained));
        }
    }
    out
}

fn now() -> u64 {
    crate::chat::now_ms()
}

/// Judge the run against the plan, record the answer on the Verify gate, and
/// keep the report. Errors only when the project can't be read at all.
pub fn verify(root: &Path, tickets: &[TicketFacts]) -> Result<VerifyReport> {
    let dir = run_dir(root);
    let head = git(&dir, &["rev-parse", "HEAD"]).context("the run has no commit to verify")?;
    let user_head = git(root, &["rev-parse", "HEAD"]).unwrap_or_default();
    let base = if dir != root && !user_head.is_empty() { git(&dir, &["merge-base", &user_head, &head]).unwrap_or(user_head) } else { git(&dir, &["rev-parse", "HEAD~1"]).unwrap_or_default() };
    let mut issues = vec![];

    let plan = load_plan(root);
    let mut criteria = vec![];
    match &plan {
        None => issues.push("there is no plan to verify against (no plan.json or spec.md)".to_string()),
        Some(p) if p.acceptance.is_empty() => issues.push("the plan has no acceptance criteria".to_string()),
        Some(p) => {
            for (i, c) in p.acceptance.iter().enumerate() {
                let key = format!("A{}", i + 1);
                let cmd = c.test.trim();
                if cmd.is_empty() {
                    issues.push(format!("{key} has no test command, so nothing shows it is met"));
                    criteria.push(CriterionResult { key, criterion: c.criterion.clone(), ..Default::default() });
                    continue;
                }
                let (exit, ms, output) = shell(&dir, cmd);
                let passed = exit == Some(0);
                if !passed {
                    issues.push(format!("{key} is not met: `{cmd}` exited {}", exit.map_or("without a code".into(), |e| e.to_string())));
                }
                criteria.push(CriterionResult { key, criterion: c.criterion.clone(), command: cmd.to_string(), passed, exit, ms, output });
            }
        }
    }

    let states: Vec<(String, TicketState)> = tickets.iter().map(|t| (t.key.clone(), t.state)).collect();
    issues.extend(crate::stages::execute_gaps(&states));
    let covered: std::collections::BTreeSet<String> = tickets.iter().flat_map(|t| t.covers.iter().map(|c| c.trim().to_uppercase())).collect();
    if !covered.is_empty() {
        for c in &criteria {
            if !covered.contains(&c.key) {
                issues.push(format!("{} ({}) is covered by no ticket", c.key, c.criterion));
            }
        }
    }

    #[derive(Deserialize)]
    struct SealFiles {
        files: Vec<crate::rampart::SealedFile>,
    }
    if let Ok(t) = std::fs::read_to_string(root.join(crate::rampart::SEAL)) {
        match serde_json::from_str::<SealFiles>(&t) {
            Err(e) => issues.push(format!("the seal can't be read: {e}")),
            Ok(s) => {
                for f in &s.files {
                    match std::fs::read(dir.join(&f.path)) {
                        Err(_) => issues.push(format!("{} is missing (sealed tests cannot be deleted)", f.path)),
                        Ok(b) if crate::planning::sha256_hex(&b) != f.sha256 => issues.push(format!("{} was changed (sealed tests are hash-locked)", f.path)),
                        Ok(_) => {}
                    }
                }
            }
        }
    }

    let weakened = if base.is_empty() { vec![] } else { weakened_tests(&dir, &base, &head) };
    let report = VerifyReport { passed: issues.is_empty() && weakened.is_empty(), head, base, criteria, issues, weakened, at: now() };
    save(root, VERIFY_JSON, &serde_json::to_string_pretty(&report)?)?;
    save(root, VERIFY_MD, &verify_markdown(&report, plan.as_ref()))?;

    let mut p = PhaseState::load(root);
    if p.phase == Phase::Test {
        let note = if report.passed {
            format!("{} criteria met on {}", report.criteria.len(), short(&report.head))
        } else {
            report.issues.iter().chain(&report.weakened).cloned().collect::<Vec<_>>().join("; ")
        };
        p.record(report.passed, GateBy::Rules, report.at, VERIFY_MD, &note);
        p.save(root)?;
    }
    Ok(report)
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(10)]
}

fn save(root: &Path, rel: &str, text: &str) -> Result<()> {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap())?;
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, p)?;
    Ok(())
}

pub fn verify_markdown(r: &VerifyReport, plan: Option<&PlanSpec>) -> String {
    let mut s = format!(
        "# Verify: {}\n\n**{}** on `{}` (from `{}`)\n\n## Criteria\n\n| # | Criterion | Command | Result |\n|---|---|---|---|\n",
        plan.map_or("no plan", |p| p.title.as_str()),
        if r.passed { "Passed" } else { "Not passed" },
        short(&r.head),
        short(&r.base)
    );
    for c in &r.criteria {
        s.push_str(&format!(
            "| {} | {} | `{}` | {} |\n",
            c.key,
            c.criterion.replace('|', "/"),
            c.command.replace('|', "/"),
            if c.passed { "pass".to_string() } else { format!("FAIL ({})", c.exit.map_or("no exit".into(), |e| e.to_string())) }
        ));
    }
    let list = |xs: &[String]| if xs.is_empty() { "None.".to_string() } else { xs.iter().map(|x| format!("- {x}")).collect::<Vec<_>>().join("\n") };
    s.push_str(&format!("\n## Unresolved issues\n\n{}\n\n## Weakened tests\n\n{}\n", list(&r.issues), list(&r.weakened)));
    s
}

fn slug(title: &str) -> String {
    let s: String = title.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).take(6).collect::<Vec<_>>().join("-");
    if s.is_empty() { "delivery".into() } else { s }
}

/// Deliver the verified run. Refused unless the project is at Ship and the
/// last Verify passed on the commit the run is at now.
pub fn ship(root: &Path, opts: &ShipOpts) -> Result<DeliveryRecord> {
    let mut p = PhaseState::load(root);
    let report: VerifyReport = std::fs::read_to_string(root.join(VERIFY_JSON))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or_else(|| anyhow!("nothing has been verified yet: run Verify first"))?;
    if !report.passed {
        bail!("the last verification did not pass: {}", report.issues.iter().chain(&report.weakened).cloned().collect::<Vec<_>>().join("; "));
    }
    if p.phase != Phase::Ship {
        bail!("Ship needs a verified run; the project is at {:?}", p.phase);
    }
    let dir = run_dir(root);
    let head = git(&dir, &["rev-parse", "HEAD"])?;
    if head != report.head {
        let at = now();
        p.gates.push(crate::phase::Gate { phase: Phase::Ship, passed: false, by: GateBy::Rules, at, artifact: VERIFY_MD.into(), note: format!("the run changed since it was verified ({} -> {})", short(&report.head), short(&head)) });
        p.phase = Phase::Test;
        p.save(root)?;
        bail!("the run changed since it was verified ({} -> {}): back to Verify", short(&report.head), short(&head));
    }
    let title = load_plan(root).map(|p| p.title).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| "Cauldron delivery".into());
    let run_branch = git(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|_| crate::git::RUN_BRANCH.into());
    let user_branch = git(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|_| "main".into());
    let pushed = format!("cauldron/{}-{}", slug(&title), &head[..head.len().min(7)]);
    let evidence: Vec<String> = report.criteria.iter().map(|c| format!("{} passed: `{}`", c.key, c.command)).collect();
    let mut limitations = vec![];
    if !root.join(crate::rampart::SEAL).exists() {
        limitations.push("no sealed suite: verified by the plan's acceptance commands only".to_string());
    }

    let gh = opts.gh.clone().unwrap_or_else(|| PathBuf::from("gh"));
    if opts.target == Target::PullRequest {
        let ok = Command::new(&gh).arg("--version").stdin(std::process::Stdio::null()).output().map(|o| o.status.success()).unwrap_or(false);
        if !ok {
            bail!("a pull request needs the GitHub CLI: `{}` is missing or failed (install gh and run `gh auth login`, or ship with --ship branch)", gh.display());
        }
    }
    let (branch, pr_url) = match opts.target {
        Target::Local => {
            limitations.push(format!("not merged: take it with `git merge {run_branch}` on {user_branch}"));
            (run_branch.clone(), None)
        }
        Target::Branch | Target::PullRequest => {
            git(root, &["remote", "get-url", "origin"]).context("there is no `origin` remote to push to")?;
            git(root, &["push", "-q", "origin", &format!("{head}:refs/heads/{pushed}")]).context("pushing the delivery branch")?;
            if opts.target == Target::Branch {
                limitations.push(format!("no pull request opened; not merged into {user_branch}"));
                (pushed.clone(), None)
            } else {
                let body = format!("{}\n\n_Opened by Cauldron after Verify passed on `{}`._\n", verify_markdown(&report, load_plan(root).as_ref()), short(&head));
                let body_file = root.join(".backspace/pr-body.md");
                std::fs::write(&body_file, body)?;
                let out = Command::new(&gh)
                    .args(["pr", "create", "--head", &pushed, "--base", &user_branch, "--title", &title, "--body-file"])
                    .arg(&body_file)
                    .current_dir(root)
                    .stdin(std::process::Stdio::null())
                    .output()
                    .context("running gh")?;
                if !out.status.success() {
                    bail!("gh could not open the pull request (the branch {pushed} was pushed): {}", String::from_utf8_lossy(&out.stderr).trim());
                }
                let url = String::from_utf8_lossy(&out.stdout).lines().rev().find(|l| l.trim().starts_with("http")).map(|l| l.trim().to_string());
                limitations.push("awaiting review and merge by a person".to_string());
                (pushed.clone(), url)
            }
        }
    };

    let record = DeliveryRecord { title, target: opts.target, commit: head, branch, status: "delivered".into(), pr_url, evidence, limitations, at: now() };
    save(root, DELIVERY_JSON, &serde_json::to_string_pretty(&record)?)?;
    save(root, DELIVERY_MD, &delivery_markdown(&record))?;
    let note = match &record.pr_url {
        Some(u) => format!("pull request {u}"),
        None => format!("{:?} delivery on {}", record.target, record.branch),
    };
    p.record(true, GateBy::Rules, record.at, DELIVERY_MD, &note);
    p.save(root)?;
    Ok(record)
}

pub fn delivery_markdown(d: &DeliveryRecord) -> String {
    let list = |xs: &[String]| if xs.is_empty() { "None.".to_string() } else { xs.iter().map(|x| format!("- {x}")).collect::<Vec<_>>().join("\n") };
    format!(
        "# Delivered: {}\n\n- Target: {:?}\n- Commit: `{}`\n- Branch: `{}`\n- Pull request: {}\n- Status: {}\n\n## Evidence\n\n{}\n\n## Known limitations\n\n{}\n",
        d.title,
        d.target,
        d.commit,
        d.branch,
        d.pr_url.as_deref().unwrap_or("none"),
        d.status,
        list(&d.evidence),
        list(&d.limitations)
    )
}

/// The last delivery, if any.
pub fn last_delivery(root: &Path) -> Option<DeliveryRecord> {
    serde_json::from_str(&std::fs::read_to_string(root.join(DELIVERY_JSON)).ok()?).ok()
}

/// The last verification, if any.
pub fn last_verify(root: &Path) -> Option<VerifyReport> {
    serde_json::from_str(&std::fs::read_to_string(root.join(VERIFY_JSON)).ok()?).ok()
}

/// One step of the rules-only loop after Execute, for the CLI and tests.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Executed,
    Verified(VerifyReport),
    Shipped(DeliveryRecord),
    /// What Learn filed; an error here never undoes the delivery.
    Learned(Result<String, String>),
}

/// Take the project from wherever it stands after planning as far as the
/// rules allow: Execute's gate (every ticket finished), Verify, Ship, then
/// Learn without holding the delivery back. Stops at the first gate that
/// refuses and returns the steps taken; a refusing Verify is a step, not an
/// error. `learn` files the run's notes and says what it did.
pub fn advance_rules(root: &Path, tickets: &[TicketFacts], opts: &ShipOpts, learn: &mut dyn FnMut() -> Result<String>) -> Result<Vec<Step>> {
    let mut steps = vec![];
    loop {
        let mut p = PhaseState::load(root);
        match p.phase {
            Phase::Specify | Phase::Plan => bail!("the plan isn't approved yet ({:?}): Execute starts after it", p.phase),
            Phase::Done => return Ok(steps),
            Phase::Execute => {
                let states: Vec<(String, TicketState)> = tickets.iter().map(|t| (t.key.clone(), t.state)).collect();
                let gaps = crate::stages::execute_gaps(&states);
                if !gaps.is_empty() {
                    bail!("Execute isn't finished: {}", gaps.join("; "));
                }
                p.record(true, GateBy::Rules, now(), ".backspace/tickets", "every ticket finished");
                p.save(root)?;
                steps.push(Step::Executed);
            }
            Phase::Test => {
                let r = verify(root, tickets)?;
                let passed = r.passed;
                steps.push(Step::Verified(r));
                if !passed {
                    return Ok(steps);
                }
            }
            Phase::Ship => steps.push(Step::Shipped(ship(root, opts)?)),
            Phase::Learn => {
                let out = learn().map_err(|e| format!("{e:#}"));
                let note = match &out {
                    Ok(n) => n.clone(),
                    Err(e) => format!("Learn failed, the delivery stands: {e}"),
                };
                p.record(true, GateBy::Rules, now(), "memory", &note);
                p.save(root)?;
                steps.push(Step::Learned(out));
            }
        }
    }
}
