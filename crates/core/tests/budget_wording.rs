//! M1 hardening, second review, items 3 and 4: the documents say exactly what
//! is and is not guaranteed. These check that each limit Charles named is
//! written down, in the place a reader of the budget boundaries or the
//! hardening notes will look, and that nothing claims more than the code does.

use std::path::{Path, PathBuf};

fn docs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/cauldron")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(docs().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn has(doc: &str, needle: &str) -> bool {
    doc.to_ascii_lowercase().contains(&needle.to_ascii_lowercase())
}

#[test]
fn the_boundaries_state_every_assumption_a_reservation_rests_on() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    for needle in [
        "byte-per-token",
        "provider overhead",
        "accurate configured pricing",
        "not provider-capped",
        "overshoot within one reported turn",
        "cumulative",
        "per-turn",
        "not verified",
        "outside task-budget enforcement",
    ] {
        assert!(has(&doc, needle), "M1-BUDGET-BOUNDARIES.md does not say `{needle}`");
    }
}

#[test]
fn the_three_classes_are_kept_and_unmetered_paths_do_not_claim_a_guarantee() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    for class in ["reserved", "admitted", "unmetered"] {
        assert!(has(&doc, class), "{class}");
    }
    for line in doc.lines().filter(|l| l.starts_with('|')) {
        for path in ["CloudChatServer", "AgentChat"] {
            if line.contains(path) {
                assert!(has(line, "not enforced") && !has(line, "hard guarantee"), "{path} must say it is not enforced:\n{line}");
            }
        }
    }
    // Nothing is called a hard guarantee without a qualifier on the same line.
    for line in doc.lines().filter(|l| has(l, "hard") && (has(l, "guarantee") || has(l, "budget"))) {
        assert!(
            has(line, "not") || has(line, "only") || has(line, "no ") || has(line, "never") || has(line, "under"),
            "an unqualified claim of a hard budget:\n{line}"
        );
    }
}

#[test]
fn a_strict_budget_section_names_the_paths_to_switch_off() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    let start = doc.to_ascii_lowercase().find("## strict budgets").expect("a section headed `Strict budgets`");
    let section = &doc[start..];
    let section = &section[..section[3..].find("\n## ").map_or(section.len(), |n| n + 3)];
    for path in ["advisor", "Claude Code", "CloudChatServer", "AgentChat", "remote", "plan-billed", "max_project_tokens"] {
        assert!(has(section, path), "the strict-budget section does not mention `{path}`:\n{section}");
    }
    assert!(has(section, "disable") || has(section, "switch off") || has(section, "turn off"), "{section}");
}

#[test]
fn the_notes_do_not_oversell_policy_integrity() {
    for name in ["M1-HARDENING-NOTES.md", "M1-BUDGET-BOUNDARIES.md"] {
        let doc = read(name);
        for line in doc.lines().filter(|l| has(l, "cannot weaken") || has(l, "can not weaken")) {
            assert!(
                has(line, "background") || has(line, "os-level") || has(line, "m6"),
                "{name} states the policy guarantee without its limit:\n{line}"
            );
        }
    }
    let notes = read("M1-HARDENING-NOTES.md");
    for needle in ["not an OS-level security boundary", "background process", "between monitored actions", "M6"] {
        assert!(has(&notes, needle), "the notes do not say `{needle}`");
    }
}

#[test]
fn the_router_is_described_as_budgeted_where_it_is_and_unmetered_where_it_is_not() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    let jev: Vec<&str> = doc.lines().filter(|l| l.contains("JevRouter")).collect();
    assert!(!jev.is_empty());
    let text = jev.join("\n");
    assert!(has(&text, "reserved") && has(&text, "project") && has(&text, "no budget"), "{text}");
}
