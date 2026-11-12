//! The person's own words, kept verbatim for every agent that works on them.
//!
//! A worker used to learn why its ticket exists from its ancestors' briefs,
//! each cut to 600 characters: a summary of a summary. This log keeps what the
//! person actually said to the project (the original request, then every
//! later message, and the feedback they gave when they turned something
//! down), and nothing else: no agent replies, tool output or messages between
//! agents. A worker gets it as a file in its handoff folder (`handoff`), next
//! to the spec and its ticket, so the request (what they want) and the spec
//! (what was agreed to build) stay two separate things.
//!
//! The source of truth is `.backspace/request.jsonl`, one entry per line, so a
//! message that itself contains markdown headings cannot break the log.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const LOG: &str = ".backspace/request.jsonl";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// Something the person typed to the project.
    Message,
    /// Feedback the person gave when they rejected a plan or a delivery.
    Rejection,
    /// A choice the person made on options an agent showed them.
    Pick,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub kind: EntryKind,
    /// Exactly what the person wrote (or picked). Never trimmed or cut.
    pub text: String,
    /// What it answered, for a rejection or a pick: "plan for a, b", an option set's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    pub at: u64,
}

/// Add one entry. An entry with nothing in it is not recorded.
pub fn append(root: &Path, kind: EntryKind, text: &str, about: Option<&str>) -> Result<()> {
    if text.trim().is_empty() {
        return Ok(());
    }
    let e = Entry { kind, text: text.to_string(), about: about.map(String::from), at: crate::chat::now_ms() };
    let path = root.join(LOG);
    std::fs::create_dir_all(path.parent().expect("has a parent"))?;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "{}", serde_json::to_string(&e)?)?;
    Ok(())
}

/// Every entry, oldest first. A line that does not parse is skipped.
pub fn load(root: &Path) -> Vec<Entry> {
    std::fs::read_to_string(root.join(LOG))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// The first thing the person said: the project's original request.
pub fn original(entries: &[Entry]) -> Option<&Entry> {
    entries.iter().find(|e| e.kind == EntryKind::Message)
}

/// `request.md` as a worker reads it.
pub fn markdown(entries: &[Entry]) -> String {
    let mut s = String::from(
        "# The request\n\nThe person's own words, verbatim, oldest first. Nothing here is a summary. The first entry is the original request; later entries are corrections, answers and decisions they made since, and where they differ from the original, the later one is what they want now. Agents' replies and messages between agents are not included.\n",
    );
    if entries.is_empty() {
        s.push_str("\n(The person has not written anything to this project yet.)\n");
    }
    let first = entries.iter().position(|e| e.kind == EntryKind::Message);
    for (i, e) in entries.iter().enumerate() {
        let head = match (e.kind, Some(i) == first) {
            (EntryKind::Message, true) => "Original request".to_string(),
            (EntryKind::Message, false) => "Later message".to_string(),
            (EntryKind::Rejection, _) => format!("Rejected {}, with this feedback", e.about.as_deref().unwrap_or("a proposal")),
            (EntryKind::Pick, _) => format!("Picked on \"{}\"", e.about.as_deref().unwrap_or("options")),
        };
        // A fence longer than any run of backticks in the text keeps it verbatim.
        let fence = "`".repeat(longest_backtick_run(&e.text).max(2) + 1);
        s.push_str(&format!("\n## {}. {head}\n\n{fence}text\n{}\n{fence}\n", i + 1, e.text));
    }
    s
}

pub(crate) fn longest_backtick_run(s: &str) -> usize {
    let (mut best, mut run) = (0, 0);
    for c in s.chars() {
        if c == '`' {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best
}

/// Where a ticket's handoff files live, relative to the worker's directory.
pub fn handoff_dir(key: &str) -> String {
    format!(".backspace/handoff/{key}")
}

/// The files a worker starts from, written into its own directory under
/// `.backspace/handoff/<key>/` (ignored by git, so they never reach a commit).
pub struct Handoff {
    pub key: String,
    pub request: Vec<Entry>,
    /// The agreed contract: SPEC.md (plan mode) or the one-shot spec.
    pub spec: Option<String>,
    /// This ticket and the tickets above it, as their own markdown.
    pub tickets: Vec<(String, String)>,
    /// `deps/<key>.md`: each blocker's delivery record.
    pub deps: Vec<(String, String)>,
}

impl Handoff {
    /// Write the folder; returns its path relative to `dir`.
    pub fn write(&self, dir: &Path) -> Result<String> {
        if !crate::intake::safe_key(&self.key) {
            bail!("ticket key `{}` cannot name a folder", self.key);
        }
        let rel = handoff_dir(&self.key);
        let base: PathBuf = dir.join(&rel);
        std::fs::create_dir_all(base.join("deps")).with_context(|| format!("creating {rel}"))?;
        // Ignore the whole folder from inside it, so a worktree that has no
        // `.backspace/.gitignore` of its own still never commits these.
        let ignore = dir.join(".backspace/.gitignore");
        if !ignore.exists() {
            std::fs::write(&ignore, "*\n")?;
        }
        std::fs::write(base.join("request.md"), markdown(&self.request))?;
        match &self.spec {
            Some(spec) => std::fs::write(base.join("spec.md"), spec)?,
            None => {
                let _ = std::fs::remove_file(base.join("spec.md"));
            }
        }
        for (k, md) in &self.tickets {
            if crate::intake::safe_key(k) {
                std::fs::write(base.join(format!("ticket-{k}.md")), md)?;
            }
        }
        for (k, md) in &self.deps {
            if crate::intake::safe_key(k) {
                std::fs::write(base.join("deps").join(format!("{k}.md")), md)?;
            }
        }
        Ok(rel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);
    impl Dir {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn keeps_every_word_and_order() {
        let dir = std::env::temp_dir().join(format!("bs-request-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dir = Dir(dir);
        let long = "x".repeat(5000);
        append(dir.path(), EntryKind::Message, &long, None).unwrap();
        append(dir.path(), EntryKind::Message, "   ", None).unwrap();
        append(dir.path(), EntryKind::Rejection, "use Postgres, not SQLite", Some("the plan for db")).unwrap();
        append(dir.path(), EntryKind::Message, "## not a heading\n```\ncode\n```", None).unwrap();
        let e = load(dir.path());
        assert_eq!(e.len(), 3);
        assert_eq!(original(&e).unwrap().text, long);
        let md = markdown(&e);
        assert!(md.contains(&long));
        assert!(md.contains("## 1. Original request"));
        assert!(md.contains("## 2. Rejected the plan for db, with this feedback"));
        assert!(md.contains("## 3. Later message"));
        assert!(md.contains("````text\n## not a heading\n```\ncode\n```\n````"));
    }
}
