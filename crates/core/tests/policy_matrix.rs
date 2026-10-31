//! M1 final hardening, sections 4, 5 and 10: for every path a model call can
//! take, the boundaries document answers two separate questions, *does the
//! budget cover it* and *does the project's model policy cover it*, and does
//! not infer either from the other.
//!
//! This checks the document against the source: the table has the columns
//! `Path | Budget classification and enforcement | Project policy check |
//! Remaining limitation`, every path has a row (the router's own model and the
//! models it may pick are two rows), each row uses one of the agreed phrases
//! and names the policy it is checked at, and every place in the sources that
//! sends an HTTP request is either a model call with a row or on a short list
//! of requests that are not model calls. The behaviour behind the rows is in
//! `cli_policy.rs`, `cli_model_policy.rs`, `routing_policy.rs`,
//! `jev_settlement.rs`, `jev_unknown_cost.rs` and the earlier gateway and
//! policy tests; this file only keeps the words honest.

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
const ROUTING: &str = "Checked against the project's routing-model policy";
const CLI_PINNED: &str = "Pinned model checked against the relevant project orchestration/execution allowlist";
const CLI_OWN: &str = "Checked by the CLI's own configuration, not independently by the gateway";
const NOT_COVERED: &str = "Not covered by the project's policy";
const REMOTE: &str = "Runs under the remote machine's own policy";
const GATEWAY_ONLY: &str = "Gateway-level check only; no end-to-end loader exists yet";

/// The agreed phrases. Every row's policy cell contains at least one.
const PHRASES: [&str; 7] = [ALLOWLIST, ROUTING, CLI_PINNED, CLI_OWN, NOT_COVERED, REMOTE, GATEWAY_ONLY];

/// The words a budget cell opens with: the classes the code has.
const CLASSES: [&str; 5] = ["reserved", "admitted", "unmetered", "out of this process", "no call site yet"];

struct Row {
    path: String,
    budget: String,
    policy: String,
    limitation: String,
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
    let (path, budget, policy, limitation) =
        (col("path"), col("budget classification and enforcement"), col("project policy check"), col("remaining limitation"));
    lines[header + 2..]
        .iter()
        .take_while(|l| l.starts_with('|'))
        .map(|l| {
            let c = cells(l);
            assert_eq!(c.len(), head.len(), "a row with {} cells under a header of {}:\n{l}", c.len(), head.len());
            Row { path: c[path].clone(), budget: c[budget].clone(), policy: c[policy].clone(), limitation: c[limitation].clone(), line: l.to_string() }
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
    assert!(rows.len() >= 12, "{} rows", rows.len());
    for r in &rows {
        assert!(!r.budget.is_empty() && !r.policy.is_empty() && !r.limitation.is_empty(), "all three columns are filled for `{}`", r.path);
        assert!(
            CLASSES.iter().any(|c| r.budget.to_ascii_lowercase().starts_with(c)),
            "the budget cell opens with the class (reserved, admitted, unmetered, out of this process): `{}`",
            r.budget
        );
        assert!(
            !has(&r.budget, "allowlist") && !has(&r.budget, "routing_models") && !has(&r.budget, "unpinned_cli_models"),
            "policy talk belongs in the policy column, not the budget one: `{}`",
            r.budget
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
        "JevRouter's own model",
        "JevRouter's candidate models",
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
fn each_path_gets_the_phrase_it_has_earned_and_names_the_policy_it_is_checked_at() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    let rows = rows(&doc);
    let p = |k: &str| row(&rows, k).policy.clone();

    // API: the model is in the list for the agent's role.
    for k in ["API agent turn", "advisor on an API turn"] {
        assert!(p(k).contains(ALLOWLIST), "{k}: {}", p(k));
    }
    assert!(has(&p("API agent turn"), "orchestration") && has(&p("API agent turn"), "execution"), "which role is which: {}", p("API agent turn"));

    // A CLI: its config entry is checked (execution for a worker, orchestration for a planner), and under an
    // enforced policy the model it runs is either pinned and checked, or named in unpinned_cli_models.
    // Under legacy_open the model stays the CLI's own configuration's choice, and the row says so.
    for (k, role) in [("CLI worker turn", "execution"), ("CLI planner", "orchestration")] {
        let c = p(k);
        assert!(c.contains(CLI_PINNED) && c.contains(CLI_OWN), "{k}: {c}");
        assert!(has(&c, role) && has(&c, "unpinned_cli_models") && has(&c, "legacy_open") && has(&c, "Claude Code") && has(&c, "Codex"), "{k}: {c}");
    }

    // Claude Code's advisor and scouts are passed by Backspace, so Backspace checks them.
    let a = p("Claude Code's own advisor");
    assert!(a.contains(ALLOWLIST) && has(&a, "left off"), "{a}");
    let s = p("Claude Code's scout");
    assert!(s.contains(ALLOWLIST) && has(&s, "execution") && has(&s, "left off"), "{s}");

    // Jev's own model is governed by its own list, separate from the two roles; what it may pick by the role lists.
    let own = p("JevRouter's own model");
    assert!(own.contains(ROUTING) && has(&own, "routing_models") && has(&own, "heuristic") && has(&own, "orchestration or execution"), "{own}");
    let cand = p("JevRouter's candidate models");
    assert!(cand.contains(ALLOWLIST) && has(&cand, "candidates") && has(&cand, "never widened"), "{cand}");

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
    assert!(rows.iter().any(|r| has(&r.budget, "unmetered") && r.policy.contains(NOT_COVERED)));
    assert!(rows.iter().any(|r| has(&r.budget, "reserved") && r.policy.contains(ALLOWLIST)));
    assert!(rows.iter().any(|r| has(&r.budget, "reserved") && r.policy.contains(ROUTING)));
    assert!(rows.iter().any(|r| has(&r.budget, "admitted") && r.policy.contains(CLI_OWN)));
}

#[test]
fn the_remaining_limitation_column_names_what_each_path_still_cannot_do() {
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    let rows = rows(&doc);
    let l = |k: &str| row(&rows, k).limitation.clone();
    // Jev's unknown cost is a stated limitation of the router's own row, with the way it is accounted.
    let own = l("JevRouter's own model");
    assert!(has(&own, "unknown cost") && has(&own, "reservation") && has(&own, "not counted toward `max_project_tokens`"), "{own}");
    assert!(has(&own, "no budget") && has(&own, "unmetered"), "a router with no budget around it is unmetered: {own}");
    // CLI: what Backspace passes is not what the CLI does with it.
    for k in ["CLI worker turn", "CLI planner"] {
        let c = l(k);
        assert!(has(&c, "fake") && has(&c, "managed") && has(&c, "alias"), "{k}: {c}");
    }
    // Unmetered paths say it is not enforced.
    for k in ["CloudChatServer", "AgentChat"] {
        assert!(has(&l(k), "not enforced"), "{k}: {}", l(k));
    }
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
    ("core/router.rs", "a model call: the Jev router (row: JevRouter's own model)"),
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

fn strict_section(doc: &str) -> &str {
    let start = doc.to_ascii_lowercase().find("## strict budgets").expect("a section headed `Strict budgets`");
    let section = &doc[start..];
    &section[..section[3..].find("\n## ").map_or(section.len(), |n| n + 3)]
}

#[test]
fn the_strict_budget_section_and_the_jev_rows_say_the_same_things() {
    // Final hardening, section 4: the budget table and the strict section must not disagree about Jev.
    let doc = read("M1-BUDGET-BOUNDARIES.md");
    let strict = strict_section(&doc);
    let rows = rows(&doc);
    let own = row(&rows, "JevRouter's own model");
    let both = |needle: &str| {
        assert!(has(strict, needle), "the strict-budget section does not say `{needle}`");
        assert!(has(&own.line, needle), "the router's row does not say `{needle}`");
    };
    both("counts against the project dollar budget");
    both("does not count toward `max_project_tokens`");
    both("a router with no budget is unmetered");
    // Only the strict section can say what a pin and a strict task are.
    assert!(has(strict, "a pin is not a bypass"), "a pinned model still asks Jev first");
    assert!(has(strict, "does not enforce it for an individual strict task"), "backend = \"heuristic\" is project-wide; there is no per-task strict mode");
    assert!(has(strict, "output size is not provider-capped") || has(strict, "output is not provider-capped"), "Jev's answer size is an assumption");
    assert!(has(strict, "routing_models"), "and the model Jev itself runs is governed by the routing-model policy");
}

// ------------------------------------------- what is claimed, what is assumed, what is not done

fn notes() -> String {
    read("M1-HARDENING-NOTES.md")
}

#[test]
fn the_notes_keep_controls_recommendations_assumptions_limitations_and_future_work_apart() {
    let n = notes();
    let heads: Vec<String> = n.lines().filter(|l| l.starts_with("## ")).map(|l| l.to_ascii_lowercase()).collect();
    for h in [
        "implemented and tested controls",
        "operational recommendations",
        "assumptions",
        "known limitations",
        "future work",
    ] {
        assert!(heads.iter().any(|x| x.contains(h)), "no `## …{h}…` section; headings: {heads:?}");
    }
    // In that order, so a reader meets what is proven before what is hoped.
    let at = |h: &str| heads.iter().position(|x| x.contains(h)).unwrap();
    assert!(at("implemented and tested") < at("operational") && at("operational") < at("assumptions") && at("assumptions") < at("known limitations") && at("known limitations") < at("future work"));
}

#[test]
fn every_assumption_and_limitation_the_review_named_is_written_down() {
    let all = format!("{}\n{}", notes(), read("M1-BUDGET-BOUNDARIES.md"));
    for (what, needle) in [
        ("bytes-per-token bound", "bytes per token"),
        ("provider request overhead", "provider overhead"),
        ("pricing accuracy", "pricing accuracy"),
        ("max_tokens enforcement by the provider", "enforcement of `max_tokens`"),
        ("advisor output is not capped", "not provider-capped"),
        ("Jev's answer size", "512-token"),
        ("Jev's unknown cost", "unknown cost"),
        ("Claude Code reports usage at the end", "once, at the end"),
        ("Codex cumulative vs per-turn", "cumulative"),
        ("CLI policy vs the CLI's model selection", "model selection"),
        ("CloudChatServer unmetered", "CloudChatServer"),
        ("AgentChat unmetered", "AgentChat"),
        ("remote harnesses have their own policy and budgets", "remote machine's own"),
        ("max_project_tokens runs CLI turns one at a time", "one at a time"),
        ("Jev tokens are not counted", "not counted toward `max_project_tokens`"),
        ("the restore guard is not OS-level", "not an os-level"),
        ("no plugin loader until M11", "no loader"),
        ("fake CLIs and mocks are not the real thing", "fake"),
    ] {
        assert!(has(&all, needle), "the M1 documents do not mention {what} (`{needle}`)");
    }
}

#[test]
fn the_documents_never_promise_a_hard_budget_guarantee() {
    let all = format!("{}\n{}", notes(), read("M1-BUDGET-BOUNDARIES.md")).to_ascii_lowercase();
    for line in all.lines() {
        for bad in ["hard guarantee", "guaranteed not to exceed", "cannot exceed", "never exceeds", "fully bounded"] {
            if line.contains(bad) {
                // Allowed only when the line is denying it.
                assert!(
                    line.contains("not ") || line.contains("no ") || line.contains("never claim") || line.contains("without"),
                    "a budget claim stronger than the code backs: `{line}`"
                );
            }
        }
    }
}
