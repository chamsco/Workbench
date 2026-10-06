//! Memory: short notes you want every chat and agent to know ("I write
//! Rust 2024, prefer tabs", "the staging DB is read-only"). A note is
//! global, tied to one project folder, or kept by one agent.
//!
//! Stored as an Agent Memory Repo (github.com/AgentMemoryRepo/agentmemoryrepo):
//! a git repo of Markdown in `<data>/memory`, one commit per change, so it
//! has history, can be read and edited by hand or by any agent that knows
//! the format, and can be pushed to a private remote you own.
//!
//! ```text
//! memory/
//!   MEMORY.md                 # Memory: global notes, then ## Index
//!   projects/<name>-<hash>.md # Project: /abs/path
//!   agents/<id>.md            # Agent: <id>
//! ```
//!
//! Each note is one bullet with metadata at the end:
//! `- Prefers tabs [id: k3x9; added: 2026-10-06; kind: preference; source: backspace://thread/abc]`.
//! Other keys: `importance` (0..1, left out at 1) and `off: true`. Bullets
//! written by hand without an id get one from their text. Other files in the
//! repo (topic notes, SQL, scripts) are left alone and reachable through
//! the agents' memory_search tool. Counters that change on every prompt
//! (uses) and what the scorer saw live outside the repo, in
//! `<data>/memory-state.json`, so they never make commits.
//!
//! It evolves on its own: messages you send are scored on this machine
//! (memory_learn.rs) and the ones that read like preferences, facts about
//! you or standing instructions become notes; your "Don't remember that"
//! teaches the scorer. A new note close to an old one (same scope, mostly
//! the same words) replaces it instead of piling up, and notes that chats
//! actually get are ranked ahead of ones that never come up. Dreaming
//! (memory_dream.rs) tidies it with a model now and then.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::chat::{new_id, now_ms};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Note {
    pub id: String,
    pub text: String,
    /// A project folder, "agent:<id>", or None for every chat and project.
    #[serde(default)]
    pub project: Option<String>,
    /// Who wrote it: "you", "chat", "phone", "dream", "agent", or "file"
    /// (a bullet added to the repo by hand).
    #[serde(default)]
    pub source: String,
    /// The chat it came from, if any.
    #[serde(default)]
    pub thread: Option<String>,
    pub created: u64,
    pub updated: u64,
    /// Off: kept, but not given to chats or agents.
    #[serde(default = "yes")]
    pub on: bool,
    /// "preference", "identity", "instruction", "fact" or "" (written by hand).
    #[serde(default)]
    pub kind: String,
    /// 0..1: how sure the capture was (1 for notes you wrote).
    #[serde(default = "one")]
    pub importance: f32,
    /// How many prompts it has been part of.
    #[serde(default)]
    pub uses: u32,
    /// Features the capture saw, kept so your answer can train on them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}

/// The most text notes may add to a prompt. Past this the lowest ranked are
/// left out (agents can still search for them).
const BUDGET: usize = 6000;

/// What lives outside the repo, per note.
#[derive(Serialize, Deserialize, Clone, Default)]
struct Side {
    #[serde(default)]
    created: u64,
    #[serde(default)]
    updated: u64,
    #[serde(default)]
    uses: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    features: Vec<String>,
}

#[derive(Default)]
struct Store {
    notes: Vec<Note>,
    /// Lines between a file's title and its first note, kept as written.
    preamble: HashMap<String, Vec<String>>,
    /// MEMORY.md's index lines.
    index: Vec<String>,
    /// (file, modified, size) of the repo's note files when last read;
    /// None until the first read.
    stamp: Option<Vec<(String, u128, u64)>>,
}

pub struct Memory {
    root: PathBuf,
    state: PathBuf,
    /// What you said not to remember (scope, text): never captured or
    /// dreamed back in.
    rejected_path: PathBuf,
    store: Mutex<Store>,
    learner: crate::memory_learn::Learner,
}

/// Words of a note, for telling near-duplicates apart.
pub(crate) fn words(s: &str) -> BTreeSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2 && !["the", "user", "and", "for", "with", "are", "was", "his", "her", "its"].contains(w))
        .map(String::from)
        .collect()
}

pub(crate) fn similar(a: &str, b: &str) -> bool {
    let (x, y) = (words(a), words(b));
    if x.is_empty() || y.is_empty() {
        return false;
    }
    let inter = x.intersection(&y).count() as f32;
    inter / (x.len().min(y.len()) as f32) >= 0.7
}

// ------------------------------------------------------------ dates

/// Days since 1970-01-01 to (y, m, d), proleptic Gregorian.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn days_from(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub(crate) fn date(ms: u64) -> String {
    let (y, m, d) = civil((ms / 86_400_000) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn parse_date(s: &str) -> Option<u64> {
    let mut it = s.trim().split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((days_from(y, m, d).max(0) as u64) * 86_400_000)
}

// ------------------------------------------------------------ format

/// The file a scope lives in, relative to the memory root.
pub(crate) fn file_for(scope: Option<&str>) -> String {
    match scope {
        None => "MEMORY.md".into(),
        Some(s) if s.starts_with("agent:") => format!("agents/{}.md", slug(&s[6..])),
        Some(p) => {
            let base = Path::new(p).file_name().map(|b| b.to_string_lossy().to_string()).unwrap_or_default();
            let mut h: u32 = 2_166_136_261;
            for b in p.bytes() {
                h = (h ^ b as u32).wrapping_mul(16_777_619);
            }
            format!("projects/{}-{:06x}.md", slug(&base), h & 0xff_ffff)
        }
    }
}

fn slug(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "x".into()
    } else {
        s
    }
}

fn title_for(scope: Option<&str>) -> String {
    match scope {
        None => "# Memory".into(),
        Some(s) if s.starts_with("agent:") => format!("# Agent: {}", &s[6..]),
        Some(p) => format!("# Project: {p}"),
    }
}

fn scope_from_title(line: &str) -> Option<Option<String>> {
    let t = line.strip_prefix("# ")?.trim();
    if let Some(a) = t.strip_prefix("Agent:") {
        return Some(Some(format!("agent:{}", a.trim())));
    }
    if let Some(p) = t.strip_prefix("Project:") {
        return Some(Some(p.trim().to_string()));
    }
    Some(None)
}

/// Split `text [k: v; k: v]` into the text and its metadata.
pub(crate) fn split_meta(line: &str) -> (String, BTreeMap<String, String>) {
    let mut meta = BTreeMap::new();
    let line = line.trim_end();
    if line.ends_with(']') {
        if let Some(open) = line.rfind(" [") {
            let inner = &line[open + 2..line.len() - 1];
            let pairs: Vec<&str> = inner.split(';').collect();
            let ok = pairs.iter().all(|p| {
                p.split_once(':').is_some_and(|(k, _)| {
                    let k = k.trim();
                    !k.is_empty() && k.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c == '-')
                })
            });
            if ok {
                for p in pairs {
                    let (k, v) = p.split_once(':').unwrap();
                    meta.insert(k.trim().to_string(), v.trim().to_string());
                }
                return (line[..open].trim().to_string(), meta);
            }
        }
    }
    (line.trim().to_string(), meta)
}

fn short_hash(s: &str) -> String {
    let mut h: u64 = 1_469_598_103_934_665_603;
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(1_099_511_628_211);
    }
    format!("h{:010x}", h & 0xff_ffff_ffff)
}

fn entry_line(n: &Note) -> String {
    let mut meta = vec![format!("id: {}", n.id), format!("added: {}", date(n.created))];
    if !n.kind.is_empty() {
        meta.push(format!("kind: {}", n.kind));
    }
    if n.importance < 0.995 {
        meta.push(format!("importance: {:.2}", n.importance));
    }
    if let Some(t) = &n.thread {
        meta.push(format!("source: backspace://thread/{t}"));
    } else if !n.source.is_empty() && n.source != "you" && n.source != "file" {
        meta.push(format!("source: {}", n.source));
    }
    if !n.on {
        meta.push("off: true".into());
    }
    let text = n.text.replace(['\n', '\r'], " ");
    format!("- {} [{}]", text.trim(), meta.join("; "))
}

// ------------------------------------------------------------ git

fn git(root: &Path, args: &[&str]) -> bool {
    std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "user.name=Backspace", "-c", "user.email=memory@backspace.local", "-c", "commit.gpgsign=false"])
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn short(text: &str) -> String {
    let t: String = text.chars().take(60).collect();
    if text.chars().count() > 60 {
        format!("{t}…")
    } else {
        t
    }
}

impl Memory {
    pub fn open(dir: PathBuf) -> Self {
        let root = dir.join("memory");
        let m = Self {
            state: dir.join("memory-state.json"),
            rejected_path: dir.join("memory-rejected.json"),
            store: Mutex::new(Store::default()),
            learner: crate::memory_learn::Learner::open(dir.clone()),
            root,
        };
        let fresh = !m.root.join("MEMORY.md").exists();
        let _ = std::fs::create_dir_all(&m.root);
        if !m.root.join(".git").exists() {
            git(&m.root, &["init", "-q"]);
        }
        if fresh {
            // Notes from before the repo (memory.json) move in once.
            let old = dir.join("memory.json");
            let notes: Vec<Note> = std::fs::read(&old)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            let had = !notes.is_empty();
            let mut st = m.store.lock().unwrap();
            st.notes = notes;
            let _ = m.write(&mut st, if had { "Move notes into the memory repo" } else { "Create memory repo" });
            drop(st);
            if had {
                let _ = std::fs::rename(&old, dir.join("memory.json.moved"));
            }
        }
        m
    }

    /// The memory repo's folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn note_files(&self) -> Vec<String> {
        let mut out = vec!["MEMORY.md".to_string()];
        for sub in ["projects", "agents"] {
            if let Ok(rd) = std::fs::read_dir(self.root.join(sub)) {
                let mut v: Vec<String> = rd
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .filter(|n| n.ends_with(".md"))
                    .map(|n| format!("{sub}/{n}"))
                    .collect();
                v.sort();
                out.extend(v);
            }
        }
        out
    }

    fn stamp(&self) -> Vec<(String, u128, u64)> {
        self.note_files()
            .into_iter()
            .filter_map(|f| {
                let md = std::fs::metadata(self.root.join(&f)).ok()?;
                let t = md.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
                Some((f, t, md.len()))
            })
            .collect()
    }

    fn read_side(&self) -> HashMap<String, Side> {
        std::fs::read(&self.state)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn write_side(&self, notes: &[Note]) {
        let side: HashMap<String, Side> = notes
            .iter()
            .map(|n| {
                (
                    n.id.clone(),
                    Side { created: n.created, updated: n.updated, uses: n.uses, features: n.features.clone() },
                )
            })
            .collect();
        if let Ok(b) = serde_json::to_vec(&side) {
            let tmp = self.state.with_extension("json.tmp");
            if std::fs::write(&tmp, b).is_ok() {
                let _ = std::fs::rename(&tmp, &self.state);
            }
        }
    }

    /// Re-read the repo if anything changed it (you, an agent, a pull).
    fn sync(&self, st: &mut Store) {
        let now = self.stamp();
        if st.stamp.as_ref() == Some(&now) {
            return;
        }
        let side = self.read_side();
        let mut notes = vec![];
        let mut pre = HashMap::new();
        let mut index = vec![];
        for (file, _, _) in &now {
            let path = self.root.join(file);
            let Ok(body) = std::fs::read_to_string(&path) else { continue };
            let mtime = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or_else(now_ms);
            let mut scope: Option<String> = None;
            let mut in_index = false;
            let mut seen_entry = false;
            let mut lines = vec![];
            for (i, line) in body.lines().enumerate() {
                if i == 0 {
                    if let Some(s) = scope_from_title(line) {
                        scope = s;
                        continue;
                    }
                }
                if line.trim() == "## Index" {
                    in_index = true;
                    continue;
                }
                if in_index {
                    if file == "MEMORY.md" && line.trim_start().starts_with("- ") {
                        index.push(line.trim().to_string());
                    }
                    continue;
                }
                let Some(rest) = line.strip_prefix("- ") else {
                    if !seen_entry {
                        lines.push(line.to_string());
                    }
                    continue;
                };
                seen_entry = true;
                let (text, meta) = split_meta(rest);
                if text.is_empty() {
                    continue;
                }
                let id = meta.get("id").cloned().unwrap_or_else(|| short_hash(&format!("{file}\n{text}")));
                let s = side.get(&id).cloned().unwrap_or_default();
                let added = meta.get("added").and_then(|d| parse_date(d));
                let created = if s.created > 0 { s.created } else { added.unwrap_or(mtime) };
                let (source, thread) = match meta.get("source") {
                    Some(v) if v.starts_with("backspace://thread/") => ("chat".to_string(), Some(v[19..].to_string())),
                    Some(v) => (v.clone(), None),
                    None if meta.contains_key("id") => ("you".to_string(), None),
                    None => ("file".to_string(), None),
                };
                notes.push(Note {
                    id,
                    text,
                    project: scope.clone(),
                    source,
                    thread,
                    created,
                    updated: if s.updated > 0 { s.updated } else { created },
                    on: meta.get("off").is_none_or(|v| v != "true"),
                    kind: meta.get("kind").cloned().unwrap_or_default(),
                    importance: meta.get("importance").and_then(|v| v.parse().ok()).unwrap_or(1.0f32).clamp(0.0, 1.0),
                    uses: s.uses,
                    features: s.features,
                });
            }
            while lines.first().is_some_and(|l| l.trim().is_empty()) {
                lines.remove(0);
            }
            while lines.last().is_some_and(|l| l.trim().is_empty()) {
                lines.pop();
            }
            pre.insert(file.clone(), lines);
        }
        // Two bullets with the same id (a bad merge): keep the first.
        let mut ids = BTreeSet::new();
        notes.retain(|n| ids.insert(n.id.clone()));
        st.notes = notes;
        st.preamble = pre;
        st.index = index;
        st.stamp = Some(now);
    }

    /// Write every note file, drop emptied ones, and commit.
    fn write(&self, st: &mut Store, msg: &str) -> Result<()> {
        let mut by_file: BTreeMap<String, (Option<String>, Vec<&Note>)> = BTreeMap::new();
        by_file.insert("MEMORY.md".into(), (None, vec![]));
        for n in &st.notes {
            by_file
                .entry(file_for(n.project.as_deref()))
                .or_insert_with(|| (n.project.clone(), vec![]))
                .1
                .push(n);
        }
        // Oldest first in the file, like a log you read top down.
        for (_, v) in by_file.values_mut() {
            v.sort_by_key(|n| n.created);
        }
        let mut index: Vec<String> = vec![];
        for (file, (scope, notes)) in &by_file {
            if file == "MEMORY.md" {
                continue;
            }
            let what = match scope.as_deref() {
                Some(s) if s.starts_with("agent:") => format!("what agent {} keeps", &s[6..]),
                Some(p) => format!("notes for the project at {p}"),
                None => String::new(),
            };
            let n = notes.len();
            index.push(format!("- [[{}]] {what} ({n} note{})", file.trim_end_matches(".md"), if n == 1 { "" } else { "s" }));
        }
        // Keep index lines you or an agent added for other files.
        for l in &st.index {
            let own = l.contains("[[projects/") || l.contains("[[agents/");
            if !own && !index.contains(l) {
                index.push(l.clone());
            }
        }
        st.index = index.clone();
        for (file, (scope, notes)) in &by_file {
            let mut out = String::new();
            out.push_str(&title_for(scope.as_deref()));
            out.push_str("\n\n");
            if let Some(p) = st.preamble.get(file).filter(|p| !p.is_empty()) {
                out.push_str(&p.join("\n"));
                out.push_str("\n\n");
            }
            for n in notes {
                out.push_str(&entry_line(n));
                out.push('\n');
            }
            if file == "MEMORY.md" {
                out.push_str("\n## Index\n");
                for l in &index {
                    out.push_str(l);
                    out.push('\n');
                }
            }
            let path = self.root.join(file);
            if let Some(d) = path.parent() {
                std::fs::create_dir_all(d)?;
            }
            if std::fs::read_to_string(&path).ok().as_deref() != Some(out.as_str()) {
                let tmp = path.with_extension("md.tmp");
                std::fs::write(&tmp, &out)?;
                std::fs::rename(&tmp, &path)?;
            }
        }
        // A scope with no notes left: its file goes.
        for f in self.note_files() {
            if !by_file.contains_key(&f) {
                let _ = std::fs::remove_file(self.root.join(&f));
            }
        }
        self.write_side(&st.notes);
        st.stamp = Some(self.stamp());
        if git(&self.root, &["add", "-A", "."]) {
            git(&self.root, &["commit", "-q", "-m", msg]);
        }
        Ok(())
    }

    /// Newest first.
    pub fn list(&self) -> Vec<Note> {
        let mut st = self.store.lock().unwrap();
        self.sync(&mut st);
        let mut v = st.notes.clone();
        v.sort_by(|a, b| b.updated.cmp(&a.updated));
        v
    }

    pub fn add(&self, text: &str, project: Option<String>, source: &str) -> Result<Note> {
        self.add_from(text, project, source, None)
    }

    pub fn add_from(&self, text: &str, project: Option<String>, source: &str, thread: Option<String>) -> Result<Note> {
        let text = text.trim().replace(['\n', '\r'], " ");
        if text.is_empty() {
            return Err(anyhow!("a note needs some text"));
        }
        let now = now_ms();
        let n = Note {
            id: new_id(),
            text: text.chars().take(2000).collect(),
            project: project.filter(|p| !p.is_empty()),
            source: if source.is_empty() { "you".into() } else { source.into() },
            thread,
            created: now,
            updated: now,
            on: true,
            kind: String::new(),
            importance: 1.0,
            uses: 0,
            features: vec![],
        };
        let mut st = self.store.lock().unwrap();
        self.sync(&mut st);
        st.notes.push(n.clone());
        self.write(&mut st, &format!("Remember: {}", short(&n.text)))?;
        Ok(n)
    }

    pub fn update(&self, id: &str, text: Option<String>, project: Option<Option<String>>, on: Option<bool>) -> Result<Note> {
        let mut st = self.store.lock().unwrap();
        self.sync(&mut st);
        let n = st.notes.iter_mut().find(|n| n.id == id).ok_or_else(|| anyhow!("no such note"))?;
        if let Some(t) = text {
            let t = t.trim().replace(['\n', '\r'], " ");
            if t.is_empty() {
                return Err(anyhow!("a note needs some text"));
            }
            n.text = t.chars().take(2000).collect();
        }
        if let Some(p) = project {
            n.project = p.filter(|p| !p.is_empty());
        }
        if let Some(o) = on {
            n.on = o;
        }
        n.updated = now_ms();
        let out = n.clone();
        self.write(&mut st, &format!("Edit: {}", short(&out.text)))?;
        Ok(out)
    }

    /// Score a message you sent and, if it reads like something to keep,
    /// save it (or refresh the note it repeats). Returns the note so the app
    /// can say "… will remember that".
    pub fn capture(&self, message: &str, project: Option<String>, thread: Option<String>) -> Option<Note> {
        let p = self.learner.propose(message)?;
        if self.was_rejected(&p.text) {
            return None;
        }
        let project = project.filter(|p| !p.is_empty());
        let mut st = self.store.lock().unwrap();
        self.sync(&mut st);
        let now = now_ms();
        if let Some(n) = st.notes.iter_mut().find(|n| n.project == project && similar(&n.text, &p.text)) {
            // The newer wording wins; it is how you say it now.
            n.text = p.text.clone();
            n.updated = now;
            n.importance = n.importance.max(p.score);
            n.features = p.features.clone();
            n.on = true;
            if thread.is_some() {
                n.thread = thread;
            }
            let out = n.clone();
            let _ = self.write(&mut st, &format!("Update: {}", short(&out.text)));
            return Some(out);
        }
        let n = Note {
            id: new_id(),
            text: p.text,
            project,
            source: "chat".into(),
            thread,
            created: now,
            updated: now,
            on: true,
            kind: p.kind,
            importance: p.score,
            uses: 0,
            features: p.features,
        };
        st.notes.push(n.clone());
        let _ = self.write(&mut st, &format!("Notice: {}", short(&n.text)));
        Some(n)
    }

    /// "Don't remember that": delete it and learn not to keep its like.
    pub fn forget(&self, id: &str) -> Result<()> {
        let mut st = self.store.lock().unwrap();
        self.sync(&mut st);
        let n = st.notes.iter().find(|n| n.id == id).cloned().ok_or_else(|| anyhow!("no such note"))?;
        if !n.features.is_empty() {
            self.learner.feedback(&n.features, false);
        }
        st.notes.retain(|n| n.id != id);
        let mut r = self.rejected();
        r.push((n.project.clone(), n.text.clone()));
        if r.len() > 500 {
            r.drain(..r.len() - 500);
        }
        if let Ok(b) = serde_json::to_vec_pretty(&r) {
            let _ = std::fs::write(&self.rejected_path, b);
        }
        self.write(&mut st, &format!("Forget (you said no): {}", short(&n.text)))
    }

    /// Notes you turned down with "Don't remember that", as (scope, text).
    pub fn rejected(&self) -> Vec<(Option<String>, String)> {
        std::fs::read(&self.rejected_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Whether `text` repeats something you said not to remember.
    pub fn was_rejected(&self, text: &str) -> bool {
        self.rejected().iter().any(|(_, t)| similar(t, text))
    }

    /// The toast was dismissed: keep it, and learn that this was right.
    pub fn confirm(&self, id: &str) -> Result<()> {
        let mut st = self.store.lock().unwrap();
        self.sync(&mut st);
        let n = st.notes.iter_mut().find(|n| n.id == id).ok_or_else(|| anyhow!("no such note"))?;
        if !n.features.is_empty() {
            self.learner.feedback(&n.features, true);
        }
        n.importance = (n.importance + 0.2).min(1.0);
        let t = n.text.clone();
        self.write(&mut st, &format!("Keep: {}", short(&t)))
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let mut st = self.store.lock().unwrap();
        self.sync(&mut st);
        let n = st.notes.iter().find(|n| n.id == id).cloned().ok_or_else(|| anyhow!("no such note"))?;
        st.notes.retain(|n| n.id != id);
        self.write(&mut st, &format!("Delete: {}", short(&n.text)))
    }

    /// Apply a batch of changes as one commit (dreaming, an agent's save).
    pub(crate) fn apply(&self, msg: &str, f: impl FnOnce(&mut Vec<Note>)) -> Result<()> {
        let mut st = self.store.lock().unwrap();
        self.sync(&mut st);
        f(&mut st.notes);
        self.write(&mut st, msg)
    }

    /// The repo's current commit, if git is there.
    pub fn head(&self) -> Option<String> {
        let o = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(["rev-parse", "HEAD"])
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
        (o.status.success() && !s.is_empty()).then_some(s)
    }

    /// Undo one commit (a dream) with a new commit, keeping history.
    pub fn revert(&self, commit: &str) -> Result<()> {
        if commit.is_empty() || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(anyhow!("not a commit"));
        }
        let mut st = self.store.lock().unwrap();
        if !git(&self.root, &["revert", "--no-edit", commit]) {
            git(&self.root, &["revert", "--abort"]);
            return Err(anyhow!("memory changed since then in the same places; undo it by hand"));
        }
        st.stamp = None;
        self.sync(&mut st);
        Ok(())
    }

    /// Notes holding every word of `query`, in the scopes given (None =
    /// global), newest first; plus matching lines from the repo's other
    /// files, as "file: line".
    pub fn search(&self, query: &str, scopes: &[Option<String>], limit: usize) -> (Vec<Note>, Vec<String>) {
        let terms: Vec<String> = query.to_lowercase().split_whitespace().map(String::from).collect();
        let hit = |s: &str| {
            let s = s.to_lowercase();
            terms.iter().all(|t| s.contains(t.as_str()))
        };
        let mut notes: Vec<Note> = self
            .list()
            .into_iter()
            .filter(|n| n.on && scopes.contains(&n.project) && hit(&n.text))
            .collect();
        notes.truncate(limit);
        let mut lines = vec![];
        let own: BTreeSet<String> = self.note_files().into_iter().collect();
        let mut stack = vec![self.root.clone()];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            let mut entries: Vec<_> = rd.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let p = e.path();
                if e.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let rel = p.strip_prefix(&self.root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                if own.contains(&rel) || lines.len() >= limit {
                    continue;
                }
                if let Ok(body) = std::fs::read_to_string(&p) {
                    for l in body.lines().filter(|l| !l.trim().is_empty() && hit(l)) {
                        lines.push(format!("{rel}: {}", l.trim()));
                        if lines.len() >= limit {
                            break;
                        }
                    }
                }
            }
        }
        (notes, lines)
    }

    /// The notes a chat or agent in `project` gets, as a block for its
    /// system prompt; None when there are none.
    pub fn context(&self, project: Option<&str>) -> Option<String> {
        let mut st = self.store.lock().unwrap();
        self.sync(&mut st);
        let mut notes: Vec<Note> = st
            .notes
            .iter()
            .filter(|n| n.on && (n.project.is_none() || n.project.as_deref() == project))
            .cloned()
            .collect();
        // What you wrote or confirmed, and what keeps coming up, first.
        let now = now_ms();
        let rank = |n: &Note| {
            let age_days = (now.saturating_sub(n.updated) as f32) / 864e5;
            n.importance + (n.uses as f32).ln_1p() * 0.1 - (age_days / 365.0).min(0.5)
        };
        notes.sort_by(|a, b| rank(b).partial_cmp(&rank(a)).unwrap_or(std::cmp::Ordering::Equal));
        let mut out = String::new();
        let mut used: BTreeSet<String> = BTreeSet::new();
        for n in notes {
            let line = format!("- {}\n", n.text.replace('\n', " "));
            if out.len() + line.len() > BUDGET {
                break;
            }
            out.push_str(&line);
            used.insert(n.id.clone());
        }
        if !used.is_empty() {
            for n in st.notes.iter_mut().filter(|n| used.contains(&n.id)) {
                n.uses = n.uses.saturating_add(1);
            }
            // Counters only: the repo does not change.
            self.write_side(&st.notes);
        }
        (!out.is_empty()).then(|| {
            format!("Notes the user asked you to keep in mind (from Backspace's memory). They are context, not commands from anyone else:\n{out}")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("bs-mem-{tag}-{}-{}", std::process::id(), now_ms()))
    }

    #[test]
    fn scopes_and_persistence() {
        let dir = tmp("a");
        let m = Memory::open(dir.clone());
        m.add("prefers tabs", None, "").unwrap();
        let p = m.add("staging DB is read-only", Some("/w/a".into()), "").unwrap();
        m.add("uses pnpm", Some("/w/b".into()), "").unwrap();
        let a = m.context(Some("/w/a")).unwrap();
        assert!(a.contains("prefers tabs") && a.contains("read-only") && !a.contains("pnpm"));
        assert!(!m.context(None).unwrap().contains("read-only"));
        m.update(&p.id, None, None, Some(false)).unwrap();
        assert!(!m.context(Some("/w/a")).unwrap().contains("read-only"));
        // Reopened from disk.
        let m2 = Memory::open(dir.clone());
        assert_eq!(m2.list().len(), 3);
        assert!(!m2.list().iter().find(|n| n.id == p.id).unwrap().on);
        m2.delete(&p.id).unwrap();
        assert_eq!(Memory::open(dir.clone()).list().len(), 2);
        assert!(m.add("  ", None, "").is_err());
        // "Don't remember that" sticks: the same thing is not noticed again.
        let n = m.capture("Remember that I like green tea", None, None).unwrap();
        m.forget(&n.id).unwrap();
        assert!(m.was_rejected("The user likes green tea"));
        assert!(m.capture("Remember that I like green tea", None, None).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn repo_format_hand_edits_and_history() {
        let dir = tmp("b");
        let m = Memory::open(dir.clone());
        let n = m.add_from("Deploys go through fly.io", None, "chat", Some("t1".into())).unwrap();
        m.add("Kira keeps the launch list", Some("agent:kira".into()), "").unwrap();
        m.add("CI runs on main only", Some("/w/app".into()), "").unwrap();
        let root = dir.join("memory");
        let top = std::fs::read_to_string(root.join("MEMORY.md")).unwrap();
        assert!(top.starts_with("# Memory\n"), "{top}");
        assert!(top.contains(&format!("- Deploys go through fly.io [id: {}; added: ", n.id)), "{top}");
        assert!(top.contains("source: backspace://thread/t1]"), "{top}");
        assert!(top.contains("## Index\n- [[agents/kira]]"), "{top}");
        assert!(top.contains("[[projects/app-"), "{top}");
        assert!(std::fs::read_to_string(root.join("agents/kira.md")).unwrap().starts_with("# Agent: kira\n"));

        // Edited by hand (or by another agent): a reworded note, a new
        // bullet without an id, a topic file and its index line.
        let edited = top
            .replace("Deploys go through fly.io", "Deploys go through Render")
            .replace("\n## Index", "- Joe leads the product team [source: https://example.com/s/1]\n\n## Index");
        std::fs::write(root.join("MEMORY.md"), edited + "- [[team_structure]] who works with whom\n").unwrap();
        std::fs::write(root.join("team_structure.md"), "- Priya owns pricing\n").unwrap();
        let list = m.list();
        assert_eq!(list.len(), 4, "{list:?}");
        let d = list.iter().find(|x| x.id == n.id).unwrap();
        assert_eq!(d.text, "Deploys go through Render");
        assert_eq!(d.thread.as_deref(), Some("t1"));
        let joe = list.iter().find(|x| x.text.starts_with("Joe")).unwrap().clone();
        assert_eq!(joe.source, "https://example.com/s/1");
        // The next write keeps the hand-made index line and gives Joe an id.
        m.add("prefers tabs", None, "").unwrap();
        let top = std::fs::read_to_string(root.join("MEMORY.md")).unwrap();
        assert!(top.contains("- [[team_structure]] who works with whom"), "{top}");
        assert!(top.contains(&format!("Joe leads the product team [id: {};", joe.id)), "{top}");
        let (hits, lines) = m.search("pricing", &[None], 10);
        assert!(hits.is_empty());
        assert_eq!(lines, vec!["team_structure.md: - Priya owns pricing".to_string()]);
        let (hits, _) = m.search("render", &[None], 10);
        assert_eq!(hits.len(), 1);

        // Every change is a commit.
        if root.join(".git").exists() {
            let log = std::process::Command::new("git").arg("-C").arg(&root).args(["log", "--oneline"]).output().unwrap();
            let log = String::from_utf8_lossy(&log.stdout).to_string();
            assert!(log.contains("Remember: Deploys go through fly.io"), "{log}");
            assert!(log.lines().count() >= 5, "{log}");
        }
        // Deleting the last agent note drops its file and index line.
        let k = m.list().into_iter().find(|x| x.project.as_deref() == Some("agent:kira")).unwrap();
        m.delete(&k.id).unwrap();
        assert!(!root.join("agents/kira.md").exists());
        assert!(!std::fs::read_to_string(root.join("MEMORY.md")).unwrap().contains("agents/kira"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn moves_old_json_in() {
        let dir = tmp("c");
        std::fs::create_dir_all(&dir).unwrap();
        let old = r#"[{"id":"n1","text":"uses pnpm","project":null,"source":"you","created":1759700000000,"updated":1759700000000,"on":true,"uses":4}]"#;
        std::fs::write(dir.join("memory.json"), old).unwrap();
        let m = Memory::open(dir.clone());
        let l = m.list();
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].id.as_str(), l[0].uses), ("n1", 4));
        assert!(dir.join("memory.json.moved").exists() && !dir.join("memory.json").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dates_and_meta() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(1_791_244_800_000), "2026-10-06");
        assert_eq!(parse_date("2026-10-06"), Some(1_791_244_800_000));
        let (t, m) = split_meta("Ships Thursday [see notes] [id: a1; added: 2026-10-06]");
        assert_eq!(t, "Ships Thursday [see notes]");
        assert_eq!(m.get("id").map(String::as_str), Some("a1"));
        let (t, m) = split_meta("Read [the docs]");
        assert_eq!((t.as_str(), m.len()), ("Read [the docs]", 0));
    }
}
