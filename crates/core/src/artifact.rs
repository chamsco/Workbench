//! Artifacts: the things an answer is *about*.
//!
//! A reply is prose, and the object it refers to — the diff it just wrote, a
//! screenshot, the options it wants a decision on, the trace of what it ran, a
//! document it produced, a table — should be an addressable thing in the
//! thread, not something the model has to transcribe.
//!
//! The shape follows the one useful lesson from T3 Code's architecture: a
//! thread is the unit, and its output is structured objects over which one
//! renderer runs per kind. We already produce all six kinds; they simply had
//! nowhere to live. An artifact is on the project state, so it reaches the
//! desktop shells in the same snapshot as the board and the tickets, and it
//! carries the id of the thing it refers to so that "show the diff for ticket
//! 3" is a lookup rather than a re-render.
//!
//! Nothing here renders. This is the record; `app.js` draws it, and a kind it
//! does not know is still listed by name rather than dropped.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::project::ProjectState;

/// Kinds one project may hold. Older ones fall off the front: the point is what
/// the thread is about now, and this array is serialized into every snapshot,
/// so it cannot grow without bound.
pub const MAX_ARTIFACTS: usize = 120;
/// Characters of a `body` (a diff, a document, a trace excerpt).
pub const MAX_BODY: usize = 200_000;
/// Rows in one table.
pub const MAX_ROWS: usize = 200;

/// What an artifact is. A string rather than an enum so a newer shell (or a
/// tool from an app) can record a kind this build has never heard of, and an
/// older shell lists it instead of failing to deserialize the whole project.
pub const KINDS: &[&str] = &["diff", "image", "options", "trace", "document", "table"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Artifact {
    /// Stable within the project ("art1"), and what a message or a tool refers to.
    pub id: String,
    /// The agent that produced it, by key.
    pub by: String,
    /// One of [`KINDS`]; a kind nobody knows is still listed.
    pub kind: String,
    pub title: String,
    /// The ticket or branch this is about, when it is about one.
    pub ticket: Option<String>,
    /// A path inside the project, for `image` and `document`.
    pub path: Option<String>,
    /// Inline text: a diff, a document, a trace excerpt.
    pub body: Option<String>,
    /// A data: URL, for `image`.
    pub data: Option<String>,
    /// For `diff`: `git diff --stat` or similar, one line per file.
    pub stat: Option<String>,
    /// For `table`: the header row.
    pub columns: Vec<String>,
    /// For `table`: the rows.
    pub rows: Vec<Vec<String>>,
    /// For `options`: the `OptionSet::id` to draw.
    pub options: Option<String>,
    /// For `trace`: the trace id in `<data>/traces`.
    pub trace: Option<String>,
    pub at: u64,
}

impl Default for Artifact {
    fn default() -> Self {
        Self {
            id: String::new(),
            by: String::new(),
            kind: String::new(),
            title: String::new(),
            ticket: None,
            path: None,
            body: None,
            data: None,
            stat: None,
            columns: vec![],
            rows: vec![],
            options: None,
            trace: None,
            at: 0,
        }
    }
}

impl Artifact {
    /// One line for a list: what it is, and how much of it.
    pub fn summary(&self) -> String {
        match self.kind.as_str() {
            "diff" => self
                .stat
                .as_deref()
                .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
                .map(|n| format!("{n} file{} changed", if n == 1 { "" } else { "s" }))
                .unwrap_or_else(|| "a diff".into()),
            "image" => "an image".into(),
            "options" => "options to choose from".into(),
            "trace" => "a trace".into(),
            "document" => self
                .body
                .as_deref()
                .map(|b| format!("{} lines", b.lines().count()))
                .unwrap_or_else(|| "a document".into()),
            "table" => format!(
                "{} row{}",
                self.rows.len(),
                if self.rows.len() == 1 { "" } else { "s" }
            ),
            other => other.to_string(),
        }
    }
}

/// Record an artifact on the project, filling in its id and time.
///
/// Validation is per kind, so a half-filled artifact cannot reach a renderer:
/// each kind's required field is checked here rather than trusted from a tool
/// call.
pub fn add(s: &mut ProjectState, mut a: Artifact) -> Result<Artifact> {
    a.kind = a.kind.trim().to_lowercase();
    if a.kind.is_empty() {
        bail!("an artifact needs a kind (one of: {})", KINDS.join(", "));
    }
    if a.title.trim().is_empty() {
        a.title = match &a.ticket {
            Some(t) => format!("{t}: {}", a.kind),
            None => a.kind.clone(),
        };
    }
    let present = |x: &Option<String>| x.as_deref().map(str::trim).is_some_and(|s| !s.is_empty());
    match a.kind.as_str() {
        "diff" => {
            if !present(&a.body) && !present(&a.path) {
                bail!("a diff needs `body`, or a `path` to read");
            }
        }
        "image" => {
            if !present(&a.data) && !present(&a.path) {
                bail!("an image needs a `path` in the project, or `data` as a data: URL");
            }
        }
        "options" => {
            let id = a.options.as_deref().map(str::trim).unwrap_or("");
            if id.is_empty() {
                bail!("options need the `options` id of the set to draw");
            }
            if crate::options::get(s, id).is_none() {
                bail!("no options set `{id}` in this project");
            }
        }
        "trace" => {
            if !present(&a.trace) {
                bail!("a trace needs the `trace` id");
            }
        }
        "document" => {
            if !present(&a.body) && !present(&a.path) {
                bail!("a document needs `body`, or a `path` to read");
            }
        }
        "table" => {
            if a.columns.is_empty() {
                bail!("a table needs `columns`");
            }
            if a.columns.len() > 24 {
                bail!("at most 24 columns");
            }
        }
        // A kind from a newer shell or an app's tool: keep it, do not guess.
        _ => {}
    }
    if let Some(b) = &a.body {
        if b.len() > MAX_BODY {
            bail!("`body` is limited to {MAX_BODY} characters ({} given)", b.len());
        }
    }
    if a.rows.len() > MAX_ROWS {
        bail!("at most {MAX_ROWS} rows");
    }
    for (i, r) in a.rows.iter().enumerate() {
        if r.len() != a.columns.len() && a.kind == "table" {
            bail!(
                "row {} has {} cells but there are {} columns",
                i + 1,
                r.len(),
                a.columns.len()
            );
        }
    }
    a.id = format!("art{}", s.artifacts.len() + 1);
    a.at = crate::chat::now_ms();
    s.artifacts.push(a.clone());
    // Oldest first out. The cap is generous for a session and small enough that
    // the snapshot stays affordable.
    while s.artifacts.len() > MAX_ARTIFACTS {
        s.artifacts.remove(0);
    }
    Ok(a)
}

pub fn get<'a>(s: &'a ProjectState, id: &str) -> Option<&'a Artifact> {
    s.artifacts.iter().find(|a| a.id == id)
}

/// Everything one agent produced, oldest first: what its session view shows.
pub fn by<'a>(s: &'a ProjectState, agent: &str) -> Vec<&'a Artifact> {
    s.artifacts.iter().filter(|a| a.by == agent).collect()
}

/// Everything about one ticket, for the Workbench view.
pub fn for_ticket<'a>(s: &'a ProjectState, ticket: &str) -> Vec<&'a Artifact> {
    s.artifacts
        .iter()
        .filter(|a| a.ticket.as_deref() == Some(ticket))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn project() -> ProjectState {
        ProjectState::empty("p")
    }

    fn art(kind: &str, extra: serde_json::Value) -> Artifact {
        let mut v = json!({"kind": kind, "by": "main", "title": "T"});
        for (k, val) in extra.as_object().unwrap() {
            v[k] = val.clone();
        }
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn each_kind_is_validated_on_its_own_field() {
        let mut s = project();
        assert!(add(&mut s, art("diff", json!({}))).is_err(), "a diff with nothing to show");
        assert!(add(&mut s, art("image", json!({}))).is_err(), "an image with no path or data");
        assert!(add(&mut s, art("trace", json!({}))).is_err(), "a trace with no id");
        assert!(add(&mut s, art("options", json!({}))).is_err(), "options with no set");
        assert!(add(&mut s, art("document", json!({}))).is_err(), "a document with no body");
        assert!(add(&mut s, art("table", json!({}))).is_err(), "a table with no columns");
        assert!(add(&mut s, art("", json!({"body": "x"}))).is_err(), "no kind at all");
        // None of the failures left anything behind.
        assert!(s.artifacts.is_empty(), "{:?}", s.artifacts);

        // The valid fillings go in.
        assert!(add(&mut s, art("diff", json!({"body": "+a\n-b"}))).is_ok());
        assert!(add(&mut s, art("image", json!({"path": "shot.png"}))).is_ok());
        assert!(add(&mut s, art("trace", json!({"trace": "tr1"}))).is_ok());
        assert!(add(&mut s, art("document", json!({"body": "# Hi"}))).is_ok());
        assert!(add(&mut s, art("table", json!({"columns": ["a"], "rows": [["1"]]}))).is_ok());
        assert_eq!(s.artifacts.len(), 5);
    }

    #[test]
    fn ids_are_unique_and_addressable() {
        let mut s = project();
        let a = add(&mut s, art("document", json!({"body": "one"}))).unwrap();
        let b = add(&mut s, art("document", json!({"body": "two"}))).unwrap();
        assert_eq!(a.id, "art1");
        assert_eq!(b.id, "art2");
        assert_eq!(get(&s, "art2").unwrap().body.as_deref(), Some("two"));
        assert!(get(&s, "nope").is_none());
        assert_eq!(by(&s, "main").len(), 2);
        assert!(by(&s, "someone").is_empty());
        // A title is filled in when the tool does not give one.
        let untitled: Artifact = serde_json::from_value(json!({"kind": "image", "by": "w1", "path": "a.png", "ticket": "t-1"})).unwrap();
        let c = add(&mut s, untitled).unwrap();
        assert_eq!(c.title, "t-1: image");
    }

    #[test]
    fn options_must_refer_to_a_set_that_exists() {
        let mut s = project();
        let bad = art("options", json!({"options": "opt9"}));
        assert!(add(&mut s, bad).is_err(), "no such set");
        // With a real set in place it goes in.
        let set = crate::options::add(
            &mut s,
            "main",
            "Which one?",
            "",
            vec![
                crate::options::Variant { id: "a".into(), label: "A".into(), note: String::new(), image: Some("a.png".into()), url: None },
                crate::options::Variant { id: "b".into(), label: "B".into(), note: String::new(), image: Some("b.png".into()), url: None },
            ],
        )
        .unwrap();
        assert!(add(&mut s, art("options", json!({"options": set.id}))).is_ok());
    }

    #[test]
    fn a_table_row_must_match_the_header() {
        let mut s = project();
        let bad = art("table", json!({"columns": ["a", "b"], "rows": [["1"]]}));
        let err = add(&mut s, bad).unwrap_err().to_string();
        assert!(err.contains("2 columns"), "{err}");
        assert!(add(&mut s, art("table", json!({"columns": ["a", "b"], "rows": [["1", "2"]]}))).is_ok());
    }

    #[test]
    fn kinds_this_build_does_not_know_are_kept_not_dropped() {
        let mut s = project();
        let a = add(&mut s, art("mindmap", json!({"body": "x"}))).unwrap();
        assert_eq!(a.kind, "mindmap");
        assert_eq!(a.summary(), "mindmap", "listed by name rather than dropped");
        // And the whole project still serializes: a newer shell's artifact does
        // not break an older one.
        let json = serde_json::to_string(&s).unwrap();
        let back: ProjectState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.artifacts[0].kind, "mindmap");
    }

    #[test]
    fn the_list_is_bounded_and_drops_the_oldest() {
        let mut s = project();
        for i in 0..(MAX_ARTIFACTS + 5) {
            add(&mut s, art("document", json!({"body": format!("doc {i}")}))).unwrap();
        }
        assert_eq!(s.artifacts.len(), MAX_ARTIFACTS);
        assert!(
            !s.artifacts.iter().any(|a| a.body.as_deref() == Some("doc 0")),
            "the oldest fell off"
        );
        let newest = format!("doc {}", MAX_ARTIFACTS + 4);
        assert!(s.artifacts.iter().any(|a| a.body.as_deref() == Some(newest.as_str())));
    }

    #[test]
    fn a_body_cannot_be_unbounded_and_a_summary_says_what_it_is() {
        let mut s = project();
        let huge = art("diff", json!({"body": "x".repeat(MAX_BODY + 1)}));
        let err = add(&mut s, huge).unwrap_err().to_string();
        assert!(err.contains("body"), "{err}");

        let d = add(&mut s, art("diff", json!({"body": "+a\n-b", "stat": " a.txt | 2 ++\n b.txt | 1 +\n"}))).unwrap();
        assert_eq!(d.summary(), "2 files changed");
        let t = add(&mut s, art("table", json!({"columns": ["a"], "rows": [["1"], ["2"]]}))).unwrap();
        assert_eq!(t.summary(), "2 rows");
        let doc = add(&mut s, art("document", json!({"body": "one\ntwo"}))).unwrap();
        assert_eq!(doc.summary(), "2 lines");
        assert_eq!(add(&mut s, art("image", json!({"path": "x.png"}))).unwrap().summary(), "an image");
    }

    #[test]
    fn an_old_state_file_without_artifacts_still_loads() {
        let s: ProjectState = serde_json::from_value(json!({
            "name": "p", "workspace": "", "agents": [], "approvals": [], "tickets": [],
            "total_cost_usd": 0.0, "router_cost_usd": 0.0
        }))
        .unwrap();
        assert!(s.artifacts.is_empty());
        let a: Artifact = serde_json::from_value(json!({
            "id": "art1", "by": "main", "kind": "diff", "title": "T", "at": 1
        }))
        .unwrap();
        assert!(a.columns.is_empty() && a.rows.is_empty() && a.ticket.is_none());
    }
}
