//! Deciding, on this machine and in microseconds, whether something you
//! said is worth remembering, and learning from your answers.
//!
//! Each message you send is scored by a small logistic model over hand-made
//! features ("I prefer…", "from now on…", "my name is…", a trailing "?",
//! a task verb up front, code…) and the words in it. Above the threshold,
//! the message becomes a note and the app says "… will remember that".
//! Your reply teaches the model: dismissing the toast keeps the note and
//! counts as a yes; "Don't remember that" deletes it and counts as a no.
//! The weights live in `<data>/memory-learn.json`, so it fits the way you
//! talk over time. No model call, nothing leaves the machine.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// What a message would become as a note.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Proposal {
    /// The note, rewritten in the third person ("The user prefers tabs").
    pub text: String,
    /// "preference", "identity", "instruction" or "fact".
    pub kind: String,
    /// 0..1, how sure the model is.
    pub score: f32,
    /// The features that fired, kept with the note so feedback can train on them.
    pub features: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Model {
    bias: f32,
    weights: BTreeMap<String, f32>,
    threshold: f32,
    seen: u32,
}

/// Cues and their starting weights. Learning moves them; this is only day one.
const CUES: &[(&str, &[&str], f32)] = &[
    ("remember", &["remember that", "remember:", "please remember", "don't forget", "do not forget", "keep in mind", "note that"], 3.2),
    ("from-now", &["from now on", "going forward", "in the future,", "every time you", "whenever you"], 3.0),
    ("prefer", &["i prefer", "i'd rather", "i would rather", "i like ", "i love ", "i hate ", "i dislike", "i don't like", "i do not like", "my favorite", "my favourite"], 2.2),
    ("always", &["always ", "never ", "please don't", "do not ever", "don't ever"], 1.1),
    ("identity", &["my name is", "call me ", "i'm a ", "i am a ", "i work at", "i work as", "i work on", "i live in", "i'm based in", "my timezone", "i'm in ", "my pronouns", "my birthday", "i speak "], 2.4),
    ("we-use", &["we use ", "we deploy", "our stack", "our team", "our company", "our repo", "we're using", "we are using", "i use ", "i'm using", "my stack", "my setup", "my editor"], 2.2),
    ("my", &["my ", "our "], 0.4),
];

/// Signs that a message is a request, not something to keep.
const ANTI: &[(&str, &[&str], f32)] = &[
    ("ask-verb", &["write ", "explain ", "translate ", "summarize ", "summarise ", "fix ", "create ", "make ", "generate ", "show ", "tell me", "give me", "find ", "list ", "draft ", "rewrite ", "debug ", "build ", "help me", "what ", "how ", "why ", "can you", "could you", "would you", "is it", "are there", "does "], -1.6),
];

impl Default for Model {
    fn default() -> Self {
        let mut weights = BTreeMap::new();
        for (name, _, w) in CUES.iter().chain(ANTI) {
            weights.insert(format!("cue:{name}"), *w);
        }
        weights.insert("shape:question".into(), -2.4);
        weights.insert("shape:code".into(), -2.0);
        weights.insert("shape:long".into(), -1.2);
        weights.insert("shape:short".into(), -0.8);
        weights.insert("shape:first-person".into(), 0.6);
        Self { bias: -2.2, weights, threshold: 0.6, seen: 0 }
    }
}

pub struct Learner {
    path: PathBuf,
    model: Mutex<Model>,
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

impl Learner {
    pub fn open(dir: PathBuf) -> Self {
        let path = dir.join("memory-learn.json");
        let model = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { path, model: Mutex::new(model) }
    }

    fn save(&self, m: &Model) {
        if let Some(d) = self.path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(b) = serde_json::to_vec_pretty(m) {
            let _ = std::fs::write(&self.path, b);
        }
    }

    /// Score a message you sent; Some when it should become a note.
    pub fn propose(&self, message: &str) -> Option<Proposal> {
        let text = message.trim();
        if text.len() < 8 || text.starts_with('/') || text.starts_with('@') {
            return None;
        }
        let (features, sentence, kind) = features(text);
        let m = self.model.lock().unwrap();
        let z = m.bias + features.iter().map(|f| m.weights.get(f).copied().unwrap_or(0.0)).sum::<f32>();
        let score = sigmoid(z);
        // Only messages that carry a cue are candidates at all; words alone
        // never make a note, however the weights drift.
        let has_cue = features.iter().any(|f| f.starts_with("cue:") && !ANTI.iter().any(|(n, _, _)| *f == format!("cue:{n}")) && f != "cue:my");
        (has_cue && score >= m.threshold).then(|| Proposal { text: third_person(&sentence), kind, score, features })
    }

    /// Learn from your answer to a proposal: kept (true) or not (false).
    pub fn feedback(&self, features: &[String], kept: bool) {
        let mut m = self.model.lock().unwrap();
        let z = m.bias + features.iter().map(|f| m.weights.get(f).copied().unwrap_or(0.0)).sum::<f32>();
        let p = sigmoid(z);
        let y = if kept { 1.0 } else { 0.0 };
        // One step of logistic regression. Rejections teach a little harder
        // than acceptances: an unwanted note costs more than a missed one.
        let lr = if kept { 0.25 } else { 0.45 };
        let g = (y - p) * lr;
        m.bias += g * 0.2;
        for f in features {
            let w = m.weights.entry(f.clone()).or_insert(0.0);
            *w = (*w + g).clamp(-6.0, 6.0);
        }
        m.seen += 1;
        // Many rejections in a row raise the bar; acceptances lower it again.
        m.threshold = (m.threshold + if kept { -0.01 } else { 0.02 }).clamp(0.5, 0.85);
        let snapshot = m.clone();
        drop(m);
        self.save(&snapshot);
    }

    pub fn threshold(&self) -> f32 {
        self.model.lock().unwrap().threshold
    }
}

/// Features of a message, the sentence that carries the cue, and its kind.
fn features(text: &str) -> (Vec<String>, String, String) {
    let low = text.to_lowercase();
    let mut f: Vec<String> = vec![];
    let mut cue_at: Option<usize> = None;
    // "remember that…" / "from now on…" start the note themselves.
    let mut lead_at: Option<usize> = None;
    let mut kind = "fact";
    for (name, pats, _) in CUES {
        for p in *pats {
            if let Some(i) = low.find(p) {
                f.push(format!("cue:{name}"));
                if *name != "my" && *name != "always" {
                    cue_at = Some(cue_at.map_or(i, |c| c.min(i)));
                }
                if *name == "remember" || *name == "from-now" {
                    lead_at = Some(lead_at.map_or(i, |c| c.min(i)));
                }
                kind = match *name {
                    "prefer" => "preference",
                    "identity" => "identity",
                    "remember" | "from-now" | "always" if kind == "fact" => "instruction",
                    _ => kind,
                };
                break;
            }
        }
    }
    let starts_ask = ANTI[0].1.iter().any(|p| low.starts_with(p));
    if starts_ask {
        f.push("cue:ask-verb".into());
    }
    if low.trim_end().ends_with('?') {
        f.push("shape:question".into());
    }
    if text.contains("```") || text.lines().filter(|l| l.starts_with("    ") || l.contains(';')).count() > 2 {
        f.push("shape:code".into());
    }
    if text.len() > 400 {
        f.push("shape:long".into());
    }
    if text.split_whitespace().count() < 4 {
        f.push("shape:short".into());
    }
    if low.starts_with("i ") || low.starts_with("i'") || low.starts_with("my ") || low.starts_with("we ") || low.starts_with("our ") {
        f.push("shape:first-person".into());
    }
    // A few words, so it learns your phrasing ("staging", "pnpm"...).
    for w in low
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| w.len() > 3 && !STOP.contains(w))
        .take(12)
    {
        f.push(format!("w:{w}"));
    }
    f.sort();
    f.dedup();
    let sentence = match lead_at {
        Some(i) => sentence_from(text, i),
        None => sentence_at(text, cue_at.unwrap_or(0)),
    };
    (f, sentence, kind.to_string())
}

const STOP: &[&str] = &["that", "this", "with", "have", "from", "they", "there", "about", "would", "could", "should", "what", "when", "where", "which", "your", "just", "like", "please", "thanks", "thank", "really", "into", "also", "then", "than", "them", "been", "were", "will"];

/// A sentence end: . ! ? or a newline, but not the dot in "2.0" or "fly.io".
fn ends_at(text: &str, from: usize) -> usize {
    let b = text.as_bytes();
    let mut i = from;
    while i < b.len() {
        match b[i] {
            b'!' | b'?' | b'\n' => return i,
            b'.' if b.get(i + 1).is_none_or(|c| c.is_ascii_whitespace()) => return i,
            _ => {}
        }
        i += 1;
    }
    b.len()
}

/// From a lead phrase ("remember that …") to the end of its sentence.
fn sentence_from(text: &str, at: usize) -> String {
    let at = at.min(text.len());
    let end = ends_at(text, at);
    clean(&text[at..end])
}

/// The sentence around byte `at`, without "remember that".
fn sentence_at(text: &str, at: usize) -> String {
    let at = at.min(text.len());
    let b = text.as_bytes();
    let mut start = 0;
    for i in (0..at).rev() {
        if matches!(b[i], b'!' | b'?' | b'\n') || (b[i] == b'.' && b.get(i + 1).is_some_and(|c| c.is_ascii_whitespace())) {
            start = i + 1;
            break;
        }
    }
    clean(&text[start..ends_at(text, at)])
}

fn clean(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    for lead in ["please remember that ", "please remember ", "remember that ", "remember: ", "remember ", "don't forget that ", "don't forget ", "keep in mind that ", "keep in mind ", "note that ", "from now on, ", "from now on "] {
        if s.to_lowercase().starts_with(lead) {
            s = s[lead.len()..].to_string();
            break;
        }
    }
    let s: String = s.chars().take(220).collect();
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => s,
    }
}

/// "I prefer tabs" -> "The user prefers tabs"; "my name is Sam" ->
/// "The user's name is Sam". Leaves anything else as it was.
pub fn third_person(s: &str) -> String {
    let words: Vec<&str> = s.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for (i, w) in words.iter().enumerate() {
        let lw = w.to_lowercase();
        let first = i == 0;
        let rep = match lw.as_str() {
            "i" => Some(if first { "The user" } else { "the user" }.to_string()),
            "i'm" => Some(if first { "The user is" } else { "the user is" }.to_string()),
            "i've" => Some(if first { "The user has" } else { "the user has" }.to_string()),
            "i'd" => Some(if first { "The user would" } else { "the user would" }.to_string()),
            "my" => Some(if first { "The user's" } else { "the user's" }.to_string()),
            "me" => Some("the user".to_string()),
            "am" if i > 0 && words[i - 1].eq_ignore_ascii_case("i") => Some("is".to_string()),
            _ => None,
        };
        match rep {
            Some(r) => out.push(r),
            None => {
                // "The user prefer" -> "prefers": the verb after a bare "I".
                if i > 0 && words[i - 1].eq_ignore_ascii_case("i") && !lw.ends_with('s') && lw.chars().all(|c| c.is_alphabetic()) && !["am", "was", "will", "can", "should", "would", "could", "must", "might", "did", "do", "don't", "never", "always", "also", "really", "just", "only"].contains(&lw.as_str()) {
                    let v = if lw.ends_with('y') && !lw.ends_with("ay") && !lw.ends_with("ey") && !lw.ends_with("oy") {
                        format!("{}ies", &w[..w.len() - 1])
                    } else if lw.ends_with("sh") || lw.ends_with("ch") || lw.ends_with('x') || lw.ends_with('o') {
                        format!("{w}es")
                    } else if lw == "have" {
                        "has".to_string()
                    } else {
                        format!("{w}s")
                    };
                    out.push(v);
                } else if i > 0 && words[i - 1].eq_ignore_ascii_case("i") && lw == "do" {
                    out.push("does".into());
                } else if i > 0 && words[i - 1].eq_ignore_ascii_case("i") && lw == "don't" {
                    out.push("doesn't".into());
                } else {
                    out.push((*w).to_string());
                }
            }
        }
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn learner() -> (Learner, PathBuf) {
        let dir = std::env::temp_dir().join(format!("bs-learn-{}", crate::chat::now_ms()));
        (Learner::open(dir.clone()), dir)
    }

    #[test]
    fn proposes_and_skips() {
        let (l, dir) = learner();
        for yes in [
            "Remember that the staging database is read-only.",
            "I prefer pnpm over npm in every project",
            "From now on, answer in French.",
            "My name is Sam and I work at Almanac.",
            "We deploy everything with fly.io",
        ] {
            assert!(l.propose(yes).is_some(), "should keep: {yes}");
        }
        for no in [
            "Write a haiku about rain",
            "What's the difference between a process and a thread?",
            "fix this:\n```rust\nfn main() { let x = 1; }\n```",
            "ok",
            "Can you explain how I prefer to deploy?",
        ] {
            assert!(l.propose(no).is_none(), "should skip: {no}");
        }
        let p = l.propose("Remember that the staging database is read-only.").unwrap();
        assert_eq!(p.text, "The staging database is read-only");
        assert_eq!(p.kind, "instruction");
        // The note starts at its lead phrase, and "2.0" is not a sentence end.
        let p = l.propose("Almanac 2.0 ships Thursday. Remember that the user list is on fly.io now.").unwrap();
        assert_eq!(p.text, "The user list is on fly.io now");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn learns_from_rejections() {
        let (l, dir) = learner();
        let msg = "I like the weather today";
        let p = l.propose(msg).expect("starts as a candidate");
        for _ in 0..6 {
            l.feedback(&p.features, false);
        }
        assert!(l.propose(msg).is_none(), "learned to skip it");
        // And it remembers across restarts.
        assert!(Learner::open(dir.clone()).propose(msg).is_none());
        // Other preferences still pass.
        assert!(l.propose("Remember that I prefer tabs over spaces").is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn third_person_rewrites() {
        assert_eq!(third_person("I prefer tabs"), "The user prefers tabs");
        assert_eq!(third_person("My name is Sam"), "The user's name is Sam");
        assert_eq!(third_person("I'm based in Lisbon"), "The user is based in Lisbon");
        assert_eq!(third_person("I don't like semicolons"), "The user doesn't like semicolons");
        assert_eq!(third_person("The CI runs on Fridays"), "The CI runs on Fridays");
    }
}
