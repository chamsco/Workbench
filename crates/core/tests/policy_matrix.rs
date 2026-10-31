//! M1 hardening, third review, items 2 and 4: for every path a model call can
//! take, the boundaries document answers two separate questions, *does the
//! budget cover it* and *does the project's model policy cover it*, and does
//! not infer either from the other.
//!
//! This checks the document against the source: the table has a distinct
//! `Project policy check` column, every path has a row, each row uses one of
//! the agreed phrases and names the role it is checked at, and every place in
//! the sources that sends an HTTP request is either a model call with a row or
//! on a short list of requests that are not model calls. The behaviour behind
//! the rows is in `cli_policy.rs`, `jev_settlement.rs` and the earlier gateway
//! and policy tests; this file only keeps the words honest.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn docs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/cauldron")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(docs().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn has(text: &str, needle: &str) -> bool {
    text.to_ascii_lowercase().contains(&needle.to_ascii_lowercase())
}

const ALLOWLIST: &str = "Checked against the relevant project orchestration/execution allowlist";
const CLI_OWN: &str = "Checked by the CLI's own configuration, not independently by the gateway";
const NOT_COVERED: &str = "Not covered by the project's policy";
const REMOTE: &str = "Runs under the remote machine's own policy";
const GATEWAY_ONLY: &str = "Gateway-level check only; no end-to-end loader exists yet";

/// The agreed phrases. Every row's policy cell contains at least one.
const PHRASES: [&str; 5] = [ALLOWLIST, CLI_OWN, NOT_COVERED, REMOTE, GATEWAY_ONLY];

struct Row {
    path: String,
    budget_class: String,
    policy: String,
    line: String,
}

/// The paths table: the one whose header has a `Project policy check` column.
fn rows(doc: &str) -> Vec<Row> {
    let lines: Vec<&str> = doc.lines().collect();
    let header = lines
        .iter()
        .position(|l| l.starts_with('|') && has(l, "project policy check"))
        .expect("the paths table has no `Project policy check` column");
    let cells = |l: &str| -> Vec<String> { l.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect() };
    let head = cells(lines[header]);
    let col = |name: &str| head.iter().position(|h| has(h, name)).unwrap_or_else(|| panic!("no `{name}` column in {head:?}"));
    let (path, class, policy) = (col("path"), col("budget class"), col("project policy check"));
    lines[header + 2..]
        .iter()
        .take_while(|l| l.starts_with('|'))
        .map(|l| {
            let c = cells(l);
            assert_eq!(c.len(), head.len(), "a row with {} cells under a header of {}:\n{l}", c.len(), head.len());
            Row { path: c[path].clone(), budget_class: c[class].clone(), policy: c[policy].clone(), line: l.to_string() }
        })
        .collect()
}

fn row<'a>(rows: &'a [Row], key: &str) -> &'a Row {
    let hits: Vec<_> = rows.iter().filter(|r| has(&r.path, key)).collect();
    assert_eq!(hits.len(), 1, "expected one row for `{key}`, found {}: {:?}", hits.len(), rows.iter().map(|r| &r.path).collect::<Vec<_>>());
    hits[0]
}

#[test]
fn the_paths_table_keeps_budget_and_policy_in_separate_columns() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    let rows = rows(&doc);
    assert!(rows.len() >= 11, "{} rows", rows.len());
    for r in &rows {
        assert!(!r.budget_class.is_empty() && !r.policy.is_empty(), "both columns are filled for `{}`", r.path);
        assert!(
            r.budget_class.len() < 60 && !has(&r.budget_class, "allowlist"),
            "the budget class is a class (reserved, admitted, unmetered...), not a policy statement: `{}`",
            r.budget_class
        );
    }
}

#[test]
fn every_path_has_a_row() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    let rows = rows(&doc);
    for key in [
        "API agent turn",
        "advisor on an API turn",
        "CLI worker turn",
        "CLI planner",
        "Claude Code's own advisor",
        "Claude Code's scout",
        "JevRouter",
        "CloudChatServer",
        "AgentChat",
        "remote machine",
        "Plugins",
    ] {
        row(&rows, key);
    }
}

#[test]
fn every_policy_cell_uses_an_agreed_phrase() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    for r in rows(&doc) {
        assert!(PHRASES.iter().any(|p| r.policy.contains(p)), "`{}` has no agreed policy phrase:\n{}", r.path, r.policy);
    }
}

#[test]
fn each_path_gets_the_phrase_it_has_earned_and_names_the_role_it_is_checked_at() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    let rows = rows(&doc);
    let p = |k: &str| row(&rows, k).policy.clone();

    // API: the model is in the list for the agent's role.
    for k in ["API agent turn", "advisor on an API turn"] {
        assert!(p(k).contains(ALLOWLIST), "{k}: {}", p(k));
    }
    assert!(has(&p("API agent turn"), "orchestration") && has(&p("API agent turn"), "execution"), "which role is which: {}", p("API agent turn"));

    // A CLI: its config entry is checked (execution for a worker, orchestration for a planner);
    // the model it then runs is its own configuration's choice.
    let w = p("CLI worker turn");
    assert!(w.contains(CLI_OWN) && has(&w, "execution") && has(&w, "config"), "{w}");
    let pl = p("CLI planner");
    assert!(pl.contains(CLI_OWN) && has(&pl, "orchestration") && has(&pl, "config"), "{pl}");

    // Claude Code's advisor and scouts are passed by Backspace, so Backspace checks them.
    let a = p("Claude Code's own advisor");
    assert!(a.contains(ALLOWLIST) && has(&a, "left off"), "{a}");
    let s = p("Claude Code's scout");
    assert!(s.contains(ALLOWLIST) && has(&s, "execution") && has(&s, "left off"), "{s}");

    // Jev: the models it may name are filtered; its own model is not in the project's policy.
    let j = p("JevRouter");
    assert!(j.contains(NOT_COVERED) && has(&j, "candidates") && has(&j, "heuristic"), "{j}");

    // Unmetered chat and the cloud server have no policy of their own.
    assert!(p("CloudChatServer").contains(NOT_COVERED));
    let chat = p("AgentChat");
    assert!(chat.contains(NOT_COVERED) && has(&chat, "Backspace Cloud") && has(&chat, "local model"), "{chat}");

    assert!(p("remote machine").contains(REMOTE));
    assert!(p("Plugins").contains(GATEWAY_ONLY));
}

#[test]
fn the_page_says_it_answers_two_questions_and_that_neither_is_read_from_the_other() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    assert!(
        has(&doc, "does the budget cover it") && has(&doc, "does the model policy cover it"),
        "the page says it answers two separate questions"
    );
    assert!(has(&doc, "neither is inferred from the other"), "and that neither answer is read from the other");
    // An unmetered path can be policy-checked and a reserved one can sit outside the policy; the rows show both.
    let rows = rows(&doc);
    assert!(rows.iter().any(|r| has(&r.budget_class, "unmetered") && r.policy.contains(NOT_COVERED)));
    assert!(rows.iter().any(|r| has(&r.budget_class, "reserved") && r.policy.contains(ALLOWLIST)));
    assert!(rows.iter().any(|r| has(&r.budget_class, "admitted") && r.policy.contains(CLI_OWN)));
}

// ------------------------------------------------------------- the sources

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Every `.post(` in the non-test source of the library crates, by `crate/file`.
fn posts() -> BTreeMap<String, usize> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let mut found = BTreeMap::new();
    for c in std::fs::read_dir(&crates).unwrap().flatten() {
        let name = c.file_name().to_string_lossy().into_owned();
        if name == "tauri-app" || name == "app" {
            continue;
        }
        let mut files = vec![];
        rust_files(&c.path().join("src"), &mut files);
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            // Production code only: what follows `#[cfg(test)]` is tests.
            let prod = text.split("#[cfg(test)]").next().unwrap();
            let n = prod.matches(".post(").count();
            if n > 0 {
                found.insert(format!("{name}/{}", f.file_name().unwrap().to_string_lossy()), n);
            }
        }
    }
    found
}

/// Every file that sends an HTTP request, and what it sends. A new one has to be added here, in
/// review, with a row in the boundaries document if it is a model call.
const POSTS: [(&str, &str); 9] = [
    ("core/provider.rs", "a model call: API agent turns, reserved by the gateway (row: API agent turn)"),
    ("core/router.rs", "a model call: the Jev router (row: JevRouter)"),
    ("core/chat.rs", "a model call: AgentChat's Backspace Cloud route (row: AgentChat)"),
    ("core/cloud.rs", "the Backspace Cloud account API (sign-up, plan); the chat server is row: CloudChatServer"),
    ("core/trace.rs", "trace export; not a model call"),
    ("core/remote.rs", "the protocol to a remote machine's harness; not a model call here (row: remote machine)"),
    ("core/board.rs", "the local bridge a CLI agent's `backspace msg` posts to; not a model call"),
    ("runner/lib.rs", "Ollama and OpenAI-compatible targets, reached only through provider::run_target and backspace_runner::run, which unmetered.rs scans"),
    ("runner/a2a.rs", "an A2A agent target, the same way"),
];

#[test]
fn every_place_that_posts_over_http_is_a_model_call_with_a_row_or_is_listed_as_not_one() {
    let found = posts();
    let want: Vec<&str> = POSTS.iter().map(|(f, _)| *f).collect();
    let got: Vec<&str> = found.keys().map(String::as_str).collect();
    assert_eq!(got, {
        let mut w = want.clone();
        w.sort();
        w
    }, "a file sends HTTP requests that this list does not know about, or one on the list no longer does: add it with what it sends (and a row in M1-BUDGET-BOUNDARIES.md if it calls a model)");

    // The rows the model-call entries point at exist.
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    let rows = rows(&doc);
    for (_, what) in POSTS {
        if let Some(key) = what.split("(row: ").nth(1).and_then(|r| r.split(')').next()) {
            row(&rows, key);
        }
    }
}

#[test]
fn chats_direct_post_to_backspace_cloud_is_the_one_the_unmetered_scan_cannot_see() {
    // `unmetered.rs` finds calls by name (`provider::complete(`, `run_cli_turn(`...). chat.rs's cloud
    // route sends its own request, so the declaration lives in the document, and this keeps it there.
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/chat.rs")).unwrap();
    assert!(src.contains("/v1/chat"), "chat.rs no longer posts to Backspace Cloud: update the AgentChat row");
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    let chat = rows(&doc).into_iter().find(|r| has(&r.path, "AgentChat")).unwrap();
    assert!(has(&chat.line, "Backspace Cloud") && has(&chat.line, "not enforced"), "{}", chat.line);
}

// --------------------------------------------- strict budgets: what kind of statement

#[test]
fn the_strict_budget_section_separates_a_recommendation_from_an_enforced_setting_from_a_limit() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    let start = doc.to_ascii_lowercase().find("## strict budgets").expect("a section headed `Strict budgets`");
    let section = &doc[start..];
    let section = &section[..section[3..].find("\n## ").map_or(section.len(), |n| n + 3)];
    for label in ["Operational recommendation", "Enforced by configuration", "Limitation the software cannot enforce"] {
        assert!(section.contains(label), "the strict-budget section does not use the label `{label}`");
    }
    assert!(has(section, "no strict-budget mode"), "it must say plainly that there is no strict-budget mode");
    assert!(!has(section, "guaranteed strict"), "{section}");
    assert!(has(section, "backend = \"heuristic\"") && has(section, "not enforced"), "the Jev bypass is named as a setting, and its non-enforcement is");
}

#[test]
fn the_document_states_what_max_project_tokens_costs() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    for needle in [
        "one at a time",
        "refused, not queued",
        "router tokens",
        "max_project_tokens",
        "a project that does not set it",
    ] {
        assert!(has(&doc, needle), "the boundaries document does not say `{needle}` about max_project_tokens");
    }
}
