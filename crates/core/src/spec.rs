//! Plan mode's spec: a `SPEC.md` at the project root with a fixed skeleton,
//! and the gate that judges it by rules alone.
//!
//! The model grills the user (the vendored `grilling` skill) and writes the
//! spec in a fenced ```spec block; the app, not the model, writes the file and
//! runs the gate. Nothing about whether a spec is done is left to a model's
//! reading of it: a section is there or it isn't, a question is open or it
//! isn't, an acceptance line names a command or a click or it doesn't.

use std::path::Path;

use serde::{Deserialize, Serialize};

pub const FILE: &str = "SPEC.md";

/// The skeleton's sections, in order.
pub const SECTIONS: [&str; 7] =
    ["Problem", "Solution", "User stories", "Decisions", "Acceptance", "Out of scope", "Open questions"];

/// Words that mean "nothing here" in a section that may be empty.
const NONE: [&str; 6] = ["none", "nothing", "n/a", "-", "aucune", "aucun"];

const PLACEHOLDERS: [&str; 5] = ["TBD", "TODO", "???", "<actor>", "<feature>"];

/// The gate's answer, kept on the reply that wrote the spec.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Verdict {
    pub passed: bool,
    /// What is missing, one fixable line each. Empty when it passed.
    pub gaps: Vec<String>,
    pub at: u64,
}

pub fn skeleton(title: &str) -> String {
    let mut s = format!("# {title}\n");
    for h in SECTIONS {
        s.push_str(&format!("\n## {h}\n\n"));
    }
    s
}

/// Everything the rules find missing; empty means the gate opens.
pub fn check(spec: &str) -> Vec<String> {
    let mut gaps = Vec::new();
    for p in PLACEHOLDERS {
        if spec.contains(p) {
            gaps.push(format!("Placeholder left in the spec: {p}"));
        }
    }
    let sections = sections(spec);
    let body = |name: &str| sections.iter().find(|(h, _)| h.eq_ignore_ascii_case(name)).map(|(_, b)| b.as_str());
    for name in SECTIONS {
        if body(name).is_none() {
            gaps.push(format!("Missing section: ## {name}"));
        }
    }
    for name in ["Problem", "Solution", "Decisions", "Out of scope"] {
        if body(name).is_some_and(|b| b.trim().is_empty()) {
            gaps.push(format!("{name} is empty (write \"None\" if there is truly nothing)"));
        }
    }
    if body("Problem").is_some_and(is_none) {
        gaps.push("Problem says there is none: name the problem this solves".into());
    }
    if let Some(b) = body("User stories") {
        let stories = items(b);
        if stories.is_empty() {
            gaps.push("User stories has none: add \"As a …, I want …, so that …\"".into());
        }
        for (i, s) in stories.iter().enumerate() {
            if !s.to_lowercase().contains("so that") {
                gaps.push(format!("User story {} names no problem (no \"so that …\"): {}", i + 1, short(s)));
            }
        }
    }
    if let Some(b) = body("Acceptance") {
        let lines = items(b);
        if lines.is_empty() {
            gaps.push("Acceptance has no lines: each needs a command (`…`) or a click to check it".into());
        }
        for (i, a) in lines.iter().enumerate() {
            if !checkable(a) {
                gaps.push(format!("Acceptance {} has no command (`…`) or click to check it: {}", i + 1, short(a)));
            }
        }
    }
    if let Some(b) = body("Open questions") {
        let open: Vec<String> = items(b).into_iter().filter(|q| !is_none(q)).collect();
        let loose = !b.trim().is_empty() && items(b).is_empty() && !is_none(b);
        if !open.is_empty() || loose {
            let n = open.len().max(1);
            gaps.push(format!("Open questions: {n} still open; answer them or move them to Out of scope"));
        }
    }
    gaps
}

/// The spec's acceptance lines, numbered from 1 as `A1`, `A2`, … by the
/// coverage gate.
pub fn acceptance(spec: &str) -> Vec<String> {
    sections(spec).into_iter().find(|(h, _)| h.eq_ignore_ascii_case("Acceptance")).map(|(_, b)| items(&b)).unwrap_or_default()
}

/// Gate 2: does the plan cover the spec? Every acceptance line is delivered
/// by at least one ticket, every ticket delivers at least one, and nothing
/// names a line that isn't there. `tickets` is (key, covers).
pub fn coverage(spec: &str, tickets: &[(&str, &[String])]) -> Vec<String> {
    let lines = acceptance(spec);
    let id = |i: usize| format!("A{}", i + 1);
    let mut gaps = Vec::new();
    for (key, covers) in tickets {
        if covers.is_empty() {
            gaps.push(format!("Ticket {key} covers no acceptance line: name the A# it delivers, or drop it"));
        }
        for c in covers.iter() {
            if !(0..lines.len()).any(|i| id(i) == *c) {
                gaps.push(format!("Ticket {key} covers {c}, which {FILE} doesn't have (it has A1 to A{})", lines.len()));
            }
        }
    }
    for (i, line) in lines.iter().enumerate() {
        if !tickets.iter().any(|(_, covers)| covers.contains(&id(i))) {
            gaps.push(format!("No ticket covers {}: {}", id(i), short(line)));
        }
    }
    gaps
}

/// The last ```spec block in a reply, without its fences.
pub fn extract(reply: &str) -> Option<String> {
    let mut found = None;
    let mut open: Option<(String, Vec<&str>)> = None;
    for line in reply.lines() {
        let t = line.trim_start();
        match &mut open {
            None => {
                let fence: String = t.chars().take_while(|c| *c == '`' || *c == '~').collect();
                if fence.len() >= 3 && t[fence.len()..].trim().eq_ignore_ascii_case("spec") {
                    open = Some((fence, vec![]));
                }
            }
            Some((fence, body)) => {
                if t.trim_end().starts_with(fence.as_str()) && t.trim_end().chars().all(|c| c == fence.chars().next().unwrap()) {
                    found = Some(body.join("\n"));
                    open = None;
                } else {
                    body.push(line);
                }
            }
        }
    }
    found.map(|s| s.trim().to_string() + "\n")
}

/// A Plan reply in a project: write the spec it carries to `SPEC.md`, judge
/// it, and record the answer on the project's phase. None when the reply
/// carries no spec (a round of questions, say).
pub fn file(project: &Path, reply: &str, at: u64) -> anyhow::Result<Option<Verdict>> {
    let Some(spec) = extract(reply) else { return Ok(None) };
    let path = project.join(FILE);
    let tmp = project.join(".SPEC.md.tmp");
    std::fs::write(&tmp, &spec)?;
    std::fs::rename(&tmp, &path)?;
    let gaps = check(&spec);
    let passed = gaps.is_empty();
    let note = if passed { "every rule passed".to_string() } else { gaps.join("; ") };
    let mut phase = crate::phase::PhaseState::load(project);
    phase.record_spec(passed, at, &note);
    phase.save(project)?;
    Ok(Some(Verdict { passed, gaps, at }))
}

/// Plan mode's instructions for a project thread: grill, then write the spec
/// in the skeleton, knowing exactly which rules will judge it.
pub fn prompt(project: &Path, last_gaps: &[String]) -> String {
    let grilling = crate::skills::Skills::load(project).read("grilling", None).unwrap_or_default();
    let grilling = grilling.split("\n---\n").nth(1).unwrap_or(&grilling).trim();
    let mut s = format!(
        "# Plan mode\n\n\
        The user wants a spec, not changes. Read whatever you need and run read-only commands; \
        don't edit, create or delete any file. The app writes `{FILE}` at the project root from your reply.\n\n\
        ## 1. Grill\n\n{grilling}\n\n\
        Flag any feature that isn't needed for the problem: propose it for Out of scope instead of building it.\n\n\
        ## 2. Write the spec\n\n\
        Once the user confirms the frontier is empty, reply with the whole spec in one fenced block opened with ```spec, \
        in exactly this skeleton (keep every heading, in this order):\n\n```\n{}```\n\n\
        A gate checks it by rules, not judgment. It refuses the spec when:\n\
        - a heading is missing, or Problem, Solution, Decisions or Out of scope is empty;\n\
        - a user story has no \"so that …\" (each one names the problem it solves);\n\
        - an Acceptance line has neither a command in backticks nor a click to check it \
        (e.g. \"`cargo test -p core spec` passes\", \"Click Plan, then …\");\n\
        - Open questions holds anything but \"None\";\n\
        - TBD, TODO or ??? is left anywhere.\n\
        Write every decision down: nothing is left for whoever builds it to interpret.",
        skeleton("Title")
    );
    if let Ok(current) = std::fs::read_to_string(project.join(FILE)) {
        s.push_str(&format!("\n\n## The current {FILE}\n\nRevise it rather than starting over:\n\n```markdown\n{}\n```", current.trim()));
    }
    if !last_gaps.is_empty() {
        s.push_str(&format!(
            "\n\n## The gate refused the last spec\n\n{}\n\nAsk the user what only they can answer; fix the rest yourself and write the spec again.",
            last_gaps.iter().map(|g| format!("- {g}")).collect::<Vec<_>>().join("\n")
        ));
    }
    s
}

/// `## Heading` sections and their bodies.
fn sections(spec: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in spec.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            out.push((h.trim().to_string(), String::new()));
        } else if let Some((_, b)) = out.last_mut() {
            b.push_str(line);
            b.push('\n');
        }
    }
    out
}

/// A section's list items (`- `, `* `, `1. `, `1) `), indented lines joined on.
fn items(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
        let item = t
            .strip_prefix("- ")
            .or_else(|| t.strip_prefix("* "))
            .or_else(|| (digits > 0).then(|| t[digits..].strip_prefix(". ").or_else(|| t[digits..].strip_prefix(") "))).flatten());
        match item {
            Some(i) => out.push(i.trim().to_string()),
            None if line.starts_with([' ', '\t']) && !out.is_empty() => {
                let last = out.last_mut().unwrap();
                last.push(' ');
                last.push_str(t);
            }
            None => {}
        }
    }
    out
}

fn is_none(text: &str) -> bool {
    let t = text.trim().trim_start_matches(['-', '*']).trim().trim_end_matches('.').to_lowercase();
    NONE.contains(&t.as_str())
}

fn checkable(line: &str) -> bool {
    let l = line.to_lowercase();
    let code = line.matches('`').count() >= 2;
    code || ["click", "clic", "tap", "press"].iter().any(|w| l.contains(w))
}

fn short(s: &str) -> String {
    s.chars().take(60).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "# Dark mode\n\n## Problem\n\nThe app is too bright at night.\n\n## Solution\n\nA theme switch.\n\n\
        ## User stories\n\n1. As a night owl, I want a dark theme, so that my eyes don't hurt.\n\n\
        ## Decisions\n\n- The theme follows the OS until the user picks one.\n\n\
        ## Acceptance\n\n- `cargo test -p backspace-core theme` passes\n- Click Settings → Theme → Dark; the window turns dark.\n\n\
        ## Out of scope\n\n- Per-thread themes.\n\n## Open questions\n\nNone\n";

    #[test]
    fn a_complete_spec_passes() {
        assert_eq!(check(GOOD), Vec::<String>::new());
    }

    #[test]
    fn the_skeleton_alone_is_refused_for_every_empty_part() {
        let gaps = check(&skeleton("X"));
        assert!(gaps.iter().any(|g| g.starts_with("Problem is empty")), "{gaps:?}");
        assert!(gaps.iter().any(|g| g.starts_with("User stories has none")));
        assert!(gaps.iter().any(|g| g.starts_with("Acceptance has no lines")));
        assert!(!gaps.iter().any(|g| g.starts_with("Open questions")), "an empty Open questions is fine");
    }

    #[test]
    fn each_rule_names_what_is_missing() {
        let open = GOOD.replace("None\n", "- Which shade of grey?\n- Animate the switch?\n");
        assert_eq!(check(&open), vec!["Open questions: 2 still open; answer them or move them to Out of scope"]);

        let vague = GOOD.replace("- Click Settings → Theme → Dark; the window turns dark.", "- It looks nice.");
        assert_eq!(check(&vague), vec!["Acceptance 2 has no command (`…`) or click to check it: It looks nice."]);

        let why = GOOD.replace(", so that my eyes don't hurt", "");
        assert!(check(&why)[0].starts_with("User story 1 names no problem"));

        let tbd = GOOD.replace("A theme switch.", "TBD");
        assert_eq!(check(&tbd), vec!["Placeholder left in the spec: TBD"]);

        let gone = GOOD.replace("## Out of scope\n\n- Per-thread themes.\n\n", "");
        assert_eq!(check(&gone), vec!["Missing section: ## Out of scope"]);
    }

    #[test]
    fn coverage_names_every_hole() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (both, one, stray, none) = (a(&["A1", "A2"]), a(&["A1"]), a(&["A1", "A9"]), a(&[]));
        assert_eq!(acceptance(GOOD).len(), 2);
        assert!(coverage(GOOD, &[("theme", &both)]).is_empty());
        assert!(coverage(GOOD, &[("api", &one), ("ui", &a(&["A2"]))]).is_empty());
        assert_eq!(coverage(GOOD, &[("theme", &one)]), vec!["No ticket covers A2: Click Settings → Theme → Dark; the window turns dark."]);
        assert_eq!(coverage(GOOD, &[("theme", &stray)])[0], "Ticket theme covers A9, which SPEC.md doesn't have (it has A1 to A2)");
        let gaps = coverage(GOOD, &[("theme", &both), ("polish", &none)]);
        assert_eq!(gaps, vec!["Ticket polish covers no acceptance line: name the A# it delivers, or drop it"]);
    }

    #[test]
    fn extract_takes_the_last_spec_block_only() {
        let reply = format!("Here it is.\n\n```spec\nold\n```\n\nRevised:\n\n````spec\n{GOOD}````\n\n```rust\nfn x() {{}}\n```\n");
        assert_eq!(extract(&reply).unwrap(), GOOD);
        assert_eq!(extract("❓ **Q1** - which?\n\n```\nnot a spec\n```"), None);
    }

    #[test]
    fn the_prompt_carries_the_grilling_skill_the_rules_and_the_last_refusal() {
        let p = prompt(Path::new("/nonexistent"), &["Open questions: 1 still open".into()]);
        assert!(p.contains("design tree"), "grilling skill missing");
        assert!(p.contains("## Open questions"));
        assert!(p.contains("- Open questions: 1 still open"));
    }
}
