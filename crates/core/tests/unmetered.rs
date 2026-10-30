//! M1 hardening, F4: no model call leaves this process without saying how it
//! is metered. A call is either `Granted` (a reservation from the gateway) or
//! `Unmetered` for one of a short, closed list of reasons, each written down in
//! docs/cauldron/M1-BUDGET-BOUNDARIES.md. This scans the sources, so a new
//! unmetered call has to be added to the list below on purpose, in review.

use std::path::{Path, PathBuf};

/// Every `Unmetered::<Reason>` use outside gateway.rs: the file it is in and why.
const DECLARED: [(&str, &str); 3] = [
    ("chat.rs", "AgentChat"),
    ("cloud.rs", "CloudChatServer"),
    ("router.rs", "JevRouter"),
];

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

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

fn sources() -> Vec<(PathBuf, String)> {
    let mut files = vec![];
    for c in std::fs::read_dir(crates_dir()).unwrap().flatten() {
        rust_files(&c.path().join("src"), &mut files);
    }
    files.sort();
    files.into_iter().map(|p| { let t = std::fs::read_to_string(&p).unwrap(); (p, t) }).collect()
}

fn name(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

#[test]
fn unmetered_is_only_used_where_declared() {
    let mut found: Vec<(String, String)> = vec![];
    for (p, text) in sources() {
        if name(&p) == "gateway.rs" {
            continue;
        }
        let mut rest = text.as_str();
        while let Some(i) = rest.find("Unmetered::") {
            rest = &rest[i + "Unmetered::".len()..];
            let reason: String = rest.chars().take_while(|c| c.is_alphanumeric()).collect();
            found.push((name(&p), reason));
        }
    }
    found.sort();
    found.dedup();
    let mut want: Vec<(String, String)> = DECLARED.iter().map(|(f, r)| (f.to_string(), r.to_string())).collect();
    want.sort();
    assert_eq!(found, want, "an unmetered call was added or removed without updating the declared list");
}

#[test]
fn the_list_of_reasons_is_closed() {
    let (_, gateway) = sources().into_iter().find(|(p, _)| name(p) == "gateway.rs").unwrap();
    let start = gateway.find("pub enum Unmetered").expect("gateway.rs defines the reasons");
    let body = &gateway[start..];
    let body = &body[..body.find('}').unwrap()];
    let mut variants: Vec<String> = body
        .lines()
        .skip(1)
        .map(|l| l.trim().trim_end_matches(',').to_string())
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
        .collect();
    variants.sort();
    let mut want: Vec<String> = DECLARED.iter().map(|(_, r)| r.to_string()).collect();
    want.sort();
    assert_eq!(variants, want);
}

#[test]
fn every_place_that_sends_a_model_call_carries_an_authorization() {
    let mut seen = 0;
    for (p, text) in sources() {
        for needle in ["provider::complete(", "backspace_runner::run(", "run_cli_turn("] {
            for (i, _) in text.match_indices(needle) {
                let line_start = text[..i].rfind('\n').map_or(0, |n| n + 1);
                let line = &text[line_start..text[i..].find('\n').map_or(text.len(), |n| i + n)];
                if line.contains("fn run_cli_turn(") || line.trim_start().starts_with("//") {
                    continue; // the definition, or a mention in a comment
                }
                seen += 1;
                let window = &text[i..(i + 700).min(text.len())];
                assert!(
                    window.to_ascii_lowercase().contains("authorization"),
                    "{}: `{needle}` at byte {i} does not pass an Authorization:\n{window}",
                    p.display()
                );
            }
        }
    }
    assert!(seen >= 5, "expected the five known call sites, found {seen}");
}

#[test]
fn every_declared_boundary_is_written_down() {
    let doc = std::fs::read_to_string(crates_dir().join("../docs/cauldron/M1-BUDGET-BOUNDARIES.md")).expect("the boundaries document exists");
    for (_, reason) in DECLARED {
        assert!(doc.contains(reason), "{reason} is not described in the boundaries document");
    }
    for needle in ["Claude Code", "advisor", "remote", "not enforced"] {
        assert!(doc.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()), "the boundaries document does not mention {needle}");
    }
}
