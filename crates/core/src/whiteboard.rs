//! A coordinator stream's Board (Charles, 22 Nov): every generation as a node on a canvas, joined
//! by the thread it came along, with drawings and notes on top. A node is a finished reply that
//! shipped something (a commit, a picture, an HTML page) or one pinned by hand. A side stream has
//! no board of its own: what its replies ship lands on its coordinator's, marked with where it
//! came from. One file per board, under the data folder's `boards/`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::chat::{Msg, Role, Status, Thread};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Ref {
    /// "Commit", "File", "Link", "Side stream".
    pub kind: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Node {
    /// The reply it was made from (its message id), which is also its id.
    pub id: String,
    /// The thread that reply is in: the coordinator, or one of its side streams.
    pub thread: String,
    pub title: String,
    /// What was asked: the message the reply answered.
    pub ask: String,
    /// What changed, a few lines from the reply.
    pub changes: Vec<String>,
    pub refs: Vec<Ref>,
    /// Pictures it made (paths), shown as its examples.
    pub images: Vec<String>,
    /// The side stream it came from, by title; None for the coordinator's own replies.
    pub via: Option<String>,
    pub pinned: bool,
    pub at: u64,
    /// The side thread where you talk about this node, once you have.
    #[serde(default)]
    pub chat: Option<String>,
    /// Its place on the canvas, in board units; set when first laid out, then wherever it is dragged.
    pub x: f32,
    pub y: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Link {
    pub from: String,
    pub to: String,
    /// A branch off the main line (drawn dashed); false for the next step on the same line.
    #[serde(default)]
    pub fork: bool,
    /// Drawn by hand with the link tool.
    #[serde(default)]
    pub manual: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Note {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub text: String,
    /// A sticky note; false for a plain label written on the board.
    pub sticky: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Board {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    /// Pen strokes, each a list of points in board units.
    pub strokes: Vec<Vec<(f32, f32)>>,
    pub notes: Vec<Note>,
    /// Nodes taken off by hand, so the next sync does not bring them back.
    pub removed: Vec<String>,
}

pub const NODE_W: f32 = 300.;
const GAP_X: f32 = 110.;
const LANE_H: f32 = 430.;

/// What a reply shipped, as references; empty when it shipped nothing a board keeps.
pub fn shipped(m: &Msg) -> (Vec<Ref>, Vec<String>) {
    let mut refs = Vec::new();
    let mut images = Vec::new();
    for a in &m.acts {
        if a.name == "Bash" && a.input.contains("git commit") {
            let hash = a.output.split(|c: char| c == ' ' || c == ']').find(|w| w.len() >= 7 && w.len() <= 12 && w.chars().all(|c| c.is_ascii_hexdigit())).map(str::to_string);
            refs.push(Ref { kind: "Commit".into(), text: hash.unwrap_or_else(|| "a commit".into()) });
        }
        if let Some(p) = &a.path {
            let low = p.to_lowercase();
            if matches!(a.name.as_str(), "Write" | "Edit" | "MultiEdit") && [".html", ".htm"].iter().any(|e| low.ends_with(e)) {
                refs.push(Ref { kind: "Page".into(), text: p.clone() });
            }
            if [".png", ".jpg", ".jpeg", ".gif", ".webp"].iter().any(|e| low.ends_with(e)) && !images.contains(p) {
                images.push(p.clone());
            }
        }
    }
    for w in m.text.split_whitespace() {
        let w = w.trim_matches(|c: char| matches!(c, '(' | ')' | '<' | '>' | '[' | ']' | '.' | ',' | '`'));
        if w.starts_with("https://claude.ai/artifact/") && !refs.iter().any(|r| r.text == w) {
            refs.push(Ref { kind: "Artifact".into(), text: w.to_string() });
        }
    }
    for p in &images {
        refs.push(Ref { kind: "Picture".into(), text: p.clone() });
    }
    (refs, images)
}

/// A few lines saying what changed: the reply's bullet points, else its first sentences.
fn changes(text: &str) -> Vec<String> {
    let clean = |s: &str| s.trim().trim_start_matches(['-', '*', '•']).trim().replace("**", "").replace('`', "");
    let bullets: Vec<String> = text.lines().map(str::trim).filter(|l| l.starts_with("- ") || l.starts_with("* ") || l.starts_with("• ")).map(clean).filter(|l| !l.is_empty()).take(4).collect();
    if !bullets.is_empty() {
        return bullets.into_iter().map(|l| cut(&l, 90)).collect();
    }
    let body = text.split("```").step_by(2).collect::<Vec<_>>().join(" ");
    body.split_inclusive(['.', '!', '?']).map(str::trim).filter(|s| s.len() > 3).take(2).map(|s| cut(&clean(s), 110)).collect()
}

fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    format!("{}\u{2026}", s.chars().take(n - 1).collect::<String>().trim_end())
}

/// The node a finished reply makes, when it shipped something (or `pin` is set).
pub fn node_of(t: &Thread, i: usize, via: Option<String>, pin: bool) -> Option<Node> {
    let m = t.messages.get(i)?;
    if m.role == Role::User || m.status != Status::Done || m.error.is_some() {
        return None;
    }
    let (refs, images) = shipped(m);
    if refs.is_empty() && !pin {
        return None;
    }
    let ask = t.messages[..i].iter().rev().find(|x| x.role == Role::User).map(|x| x.text.trim().to_string()).unwrap_or_default();
    let title = crate::chat::title_from(ask.lines().next().unwrap_or(&t.title));
    Some(Node { id: m.id.clone(), thread: t.id.clone(), title, ask: cut(&ask, 600), changes: changes(&m.text), refs, images, via, pinned: pin, at: m.done_at.max(m.at), chat: None, x: 0., y: 0. })
}

impl Board {
    /// Adds a node for each shipping reply of `main` and of its side streams `sides` that the
    /// board does not have yet, lays the new ones out, and links them along their lines. Returns
    /// whether anything changed.
    pub fn sync(&mut self, main: &Thread, sides: &[Thread]) -> bool {
        let mut new: Vec<(usize, Node)> = Vec::new();
        for (lane, t) in std::iter::once(main).chain(sides.iter()).enumerate() {
            let via = (lane > 0).then(|| t.title.clone());
            for i in 0..t.messages.len() {
                let id = &t.messages[i].id;
                if self.nodes.iter().any(|n| &n.id == id) || self.removed.contains(id) {
                    continue;
                }
                if let Some(n) = node_of(t, i, via.clone(), false) {
                    new.push((lane, n));
                }
            }
        }
        if new.is_empty() {
            return false;
        }
        for (lane, n) in new {
            self.place(n, lane);
        }
        true
    }

    /// Pins reply `i` of `t` (a node even if it shipped nothing).
    pub fn pin(&mut self, t: &Thread, i: usize, via: Option<String>, lane: usize) -> bool {
        let Some(id) = t.messages.get(i).map(|m| m.id.clone()) else { return false };
        if let Some(n) = self.nodes.iter_mut().find(|n| n.id == id) {
            n.pinned = true;
            return true;
        }
        self.removed.retain(|r| r != &id);
        match node_of(t, i, via, true) {
            Some(n) => {
                self.place(n, lane);
                true
            }
            None => false,
        }
    }

    /// Puts a new node in time order: after the newest node before it, on its lane's row; linked
    /// from the previous node on the same thread, or, as a branch, from the main line's node
    /// just before it.
    fn place(&mut self, mut n: Node, lane: usize) {
        let col = self.nodes.iter().filter(|o| o.at <= n.at).count() as f32;
        n.x = col * (NODE_W + GAP_X);
        n.y = lane as f32 * LANE_H;
        // Later nodes move one step right to make room.
        let shift: Vec<String> = self.nodes.iter().filter(|o| o.at > n.at).map(|o| o.id.clone()).collect();
        for o in self.nodes.iter_mut().filter(|o| shift.contains(&o.id)) {
            o.x += NODE_W + GAP_X;
        }
        let prev_same = self.nodes.iter().filter(|o| o.thread == n.thread && o.at <= n.at).max_by_key(|o| o.at).map(|o| o.id.clone());
        let prev_main = self.nodes.iter().filter(|o| o.via.is_none() && o.at <= n.at).max_by_key(|o| o.at).map(|o| o.id.clone());
        match (prev_same, prev_main) {
            (Some(p), _) => self.links.push(Link { from: p, to: n.id.clone(), fork: false, manual: false }),
            (None, Some(p)) if n.via.is_some() => self.links.push(Link { from: p, to: n.id.clone(), fork: true, manual: false }),
            _ => {}
        }
        self.nodes.push(n);
    }

    /// Takes a node off for good (until pinned again).
    pub fn remove(&mut self, id: &str) {
        self.nodes.retain(|n| n.id != id);
        self.links.retain(|l| l.from != id && l.to != id);
        if !self.removed.iter().any(|r| r == id) {
            self.removed.push(id.to_string());
        }
    }

    /// The nodes that led to `id`, oldest first, ending with it.
    pub fn came_from(&self, id: &str) -> Vec<String> {
        let mut chain = vec![id.to_string()];
        let mut cur = id.to_string();
        for _ in 0..32 {
            match self.links.iter().find(|l| l.to == cur && !l.manual) {
                Some(l) if !chain.contains(&l.from) => {
                    chain.insert(0, l.from.clone());
                    cur = l.from.clone();
                }
                _ => break,
            }
        }
        chain
    }
}

/// Where boards are kept: one JSON file per coordinator stream.
pub struct Boards {
    dir: PathBuf,
}

impl Boards {
    pub fn open(data: &Path) -> Boards {
        Boards { dir: data.join("boards") }
    }

    fn path(&self, thread: &str) -> PathBuf {
        let safe: String = thread.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
        self.dir.join(format!("{safe}.json"))
    }

    pub fn load(&self, thread: &str) -> Board {
        std::fs::read_to_string(self.path(thread)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self, thread: &str, b: &Board) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let p = self.path(thread);
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(b)?)?;
        std::fs::rename(tmp, p)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread(id: &str, title: &str, msgs: serde_json::Value) -> Thread {
        serde_json::from_value(serde_json::json!({ "id": id, "title": title, "created": 0, "updated": 0, "route": { "kind": "cli", "provider": "claude", "model": null }, "messages": msgs })).unwrap()
    }

    fn user(id: &str, text: &str, at: u64) -> serde_json::Value {
        serde_json::json!({ "id": id, "role": "user", "text": text, "at": at, "status": "done" })
    }

    fn reply(id: &str, text: &str, at: u64, acts: serde_json::Value) -> serde_json::Value {
        serde_json::json!({ "id": id, "role": "assistant", "text": text, "at": at, "status": "done", "acts": acts })
    }

    fn commit(hash: &str) -> serde_json::Value {
        serde_json::json!([{ "name": "Bash", "input": "{\"command\":\"git commit -m x\"}", "output": format!("[main {hash}] x"), "start": 1 }])
    }

    #[test]
    fn replies_that_ship_become_nodes_and_side_streams_branch_off() {
        let main = thread(
            "m",
            "Redesign",
            serde_json::json!([
                user("u1", "Add the tray", 1),
                reply("r1", "Done.\n- Close hides to the tray\n- Quit from the menu", 2, commit("e7abb55")),
                user("u2", "What does it cost?", 3),
                reply("r2", "About a dollar.", 4, serde_json::json!([])),
                user("u3", "Now the Done card", 7),
                reply("r3", "Shipped it. See https://claude.ai/artifact/abc.", 8, serde_json::json!([{ "name": "Write", "path": "/w/shot.png", "start": 1 }])),
            ]),
        );
        let side = thread("s", "Tester", serde_json::json!([user("t1", "Check crew talk", 5), reply("t2", "Works.", 6, commit("12c5944"))]));
        let mut b = Board::default();
        assert!(b.sync(&main, std::slice::from_ref(&side)));
        let ids: Vec<&str> = b.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids.len(), 3, "a plain answer is no node: {ids:?}");
        let r1 = b.nodes.iter().find(|n| n.id == "r1").unwrap();
        assert_eq!((r1.title.as_str(), r1.changes.clone()), ("Add the tray", vec!["Close hides to the tray".to_string(), "Quit from the menu".to_string()]));
        assert_eq!(r1.refs, vec![Ref { kind: "Commit".into(), text: "e7abb55".into() }]);
        let t2 = b.nodes.iter().find(|n| n.id == "t2").unwrap();
        assert_eq!(t2.via.as_deref(), Some("Tester"), "a side stream's node says where it came from");
        assert!(t2.y > r1.y, "on its own lane");
        let r3 = b.nodes.iter().find(|n| n.id == "r3").unwrap();
        assert_eq!(r3.images, vec!["/w/shot.png".to_string()]);
        assert!(r3.refs.iter().any(|r| r.kind == "Artifact" && r.text == "https://claude.ai/artifact/abc"));
        assert!(r1.x < t2.x && t2.x < r3.x, "in time order");
        assert!(b.links.contains(&Link { from: "r1".into(), to: "r3".into(), fork: false, manual: false }));
        assert!(b.links.contains(&Link { from: "r1".into(), to: "t2".into(), fork: true, manual: false }));
        assert_eq!(b.came_from("r3"), vec!["r1".to_string(), "r3".to_string()]);
        assert!(!b.sync(&main, &[side.clone()]), "nothing new the second time");
        b.remove("r3");
        assert!(!b.sync(&main, &[side]), "a removed node stays off");
        assert!(b.pin(&main, 3, None, 0), "pinning a plain answer makes a node");
        assert!(b.nodes.iter().any(|n| n.id == "r2" && n.pinned));
    }

    #[test]
    fn a_board_saves_and_loads() {
        let dir = std::env::temp_dir().join(format!("wb-{}", std::process::id()));
        let boards = Boards::open(&dir);
        assert_eq!(boards.load("t1"), Board::default());
        let mut b = Board::default();
        b.notes.push(Note { id: "n".into(), x: 1., y: 2., text: "hi".into(), sticky: true });
        b.strokes.push(vec![(0., 0.), (3., 4.)]);
        boards.save("t1", &b).unwrap();
        assert_eq!(boards.load("t1"), b);
        let _ = std::fs::remove_dir_all(dir);
    }
}
