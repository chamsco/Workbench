//! The Library graph: what Cauldron remembers, as hubs and notes.
//!
//! A pure function of the notes, the folders and the agents that exist, so the
//! page can show it without any other state. Core returns no coordinates: the
//! UI lays the graph out itself.
//!
//! Nodes: one `global` hub, a hub for each folder and agent that exists, and a
//! node for each note. Edges are of three kinds and nothing else:
//!
//! - `Hub`: Global to each folder and agent hub.
//! - `Place`: a note to the hub it belongs to (its `project`: none or
//!   "global", a folder, or "agent:<id>"). A note whose folder or agent no
//!   longer exists attaches to Global.
//! - `Link`: a note to another note whose title its text names as `[[title]]`.
//!   The title is the note's first non-blank line, trimmed and cut to 60
//!   characters; the match ignores case and the spaces inside the brackets and
//!   is otherwise exact. Unknown titles, self references and repeats make no
//!   edge; when two notes share a title the link reaches both. No edge is ever
//!   drawn from similar wording.
//!
//! Folder ids are the strings notes carry in `Note::project` (the folder's
//! path); agent ids are the part after `agent:`. The output is sorted, so the
//! same notes give the same graph in any input order.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::memory::Note;

/// The longest a note's label gets, in characters.
const TITLE_CHARS: usize = 60;
const GLOBAL: &str = "global";

/// The order of the variants is the order nodes are listed in.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Global,
    Folder,
    Agent,
    Note,
}

/// The order of the variants is the order edges are listed in.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    Hub,
    Place,
    Link,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GraphNode {
    /// Stable and unique: `global`, `folder:<id>`, `agent:<id>` or `note:<id>`.
    pub id: String,
    pub kind: NodeKind,
    /// The label: a hub's name, or a note's short title.
    pub title: String,
    /// The folder, agent or note id behind the node (empty for Global).
    pub ref_id: String,
    /// Who wrote the note ("you", "chat", "dream", ...); empty for hubs.
    pub source: String,
}

/// Fields in sort order: kind, then the two ends.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphEdge {
    pub kind: EdgeKind,
    /// Node id. For `Link` the note that names the other; for `Place` the note.
    pub from: String,
    /// Node id.
    pub to: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl Graph {
    /// (nodes, edges).
    pub fn stats(&self) -> (usize, usize) {
        (self.nodes.len(), self.edges.len())
    }
}

/// A note's label: its first non-blank line, trimmed, cut to 60 characters.
fn title_of(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    line.chars().take(TITLE_CHARS).collect::<String>().trim_end().to_string()
}

/// The `[[...]]` references in `text`, lowercased and trimmed. Only a closed
/// pair on one line counts; in `[[a [[b]]` the reference is `b`.
fn references(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("[[") {
        let after = &rest[open + 2..];
        let Some(close) = after.find("]]") else { break };
        let inner = after[..close].rsplit("[[").next().unwrap_or("");
        let inner = inner.trim();
        if !inner.is_empty() && !inner.contains(['\n', '\r']) {
            out.push(inner.to_lowercase());
        }
        rest = &after[close + 2..];
    }
    out
}

/// The graph of `notes`, with a hub for each of `folders` and `agents`
/// (`(id, name)` pairs) that exists.
pub fn build_graph(notes: &[Note], folders: &[(String, String)], agents: &[(String, String)]) -> Graph {
    // If an id repeats, the smaller name wins, so the input order never decides.
    let by_name = |pairs: &[(String, String)]| -> BTreeMap<String, String> {
        let mut m: BTreeMap<String, String> = BTreeMap::new();
        for (id, name) in pairs {
            let slot = m.entry(id.clone()).or_insert_with(|| name.clone());
            if name < slot {
                *slot = name.clone();
            }
        }
        m
    };
    let folders = by_name(folders);
    let agents = by_name(agents);

    let mut nodes = vec![GraphNode { id: GLOBAL.into(), kind: NodeKind::Global, title: "Global".into(), ref_id: String::new(), source: String::new() }];
    let mut edges = Vec::new();
    let hub = |nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>, kind: NodeKind, prefix: &str, id: &str, name: &str| {
        let node_id = format!("{prefix}:{id}");
        edges.push(GraphEdge { kind: EdgeKind::Hub, from: GLOBAL.into(), to: node_id.clone() });
        nodes.push(GraphNode { id: node_id, kind, title: name.to_string(), ref_id: id.to_string(), source: String::new() });
    };
    for (id, name) in &folders {
        hub(&mut nodes, &mut edges, NodeKind::Folder, "folder", id, name);
    }
    for (id, name) in &agents {
        hub(&mut nodes, &mut edges, NodeKind::Agent, "agent", id, name);
    }

    // One note per id; if an id repeats, the most recently updated wins (then the greater text).
    let mut by_id: BTreeMap<&str, &Note> = BTreeMap::new();
    for n in notes {
        match by_id.get(n.id.as_str()) {
            Some(old) if (old.updated, &old.text) >= (n.updated, &n.text) => {}
            _ => {
                by_id.insert(n.id.as_str(), n);
            }
        }
    }

    let mut by_title: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for (&id, &n) in &by_id {
        let title = title_of(&n.text);
        by_title.entry(title.to_lowercase()).or_default().push(id);
        let place = match n.project.as_deref().filter(|p| !p.is_empty() && *p != GLOBAL) {
            Some(p) if p.strip_prefix("agent:").is_some_and(|a| agents.contains_key(a)) => format!("agent:{}", &p[6..]),
            Some(p) if !p.starts_with("agent:") && folders.contains_key(p) => format!("folder:{p}"),
            _ => GLOBAL.to_string(),
        };
        let node_id = format!("note:{id}");
        edges.push(GraphEdge { kind: EdgeKind::Place, from: node_id.clone(), to: place });
        nodes.push(GraphNode { id: node_id, kind: NodeKind::Note, title, ref_id: id.to_string(), source: n.source.clone() });
    }

    let mut links: BTreeSet<(&str, &str)> = BTreeSet::new();
    for (&id, &n) in &by_id {
        for r in references(&n.text) {
            for &target in by_title.get(&r).into_iter().flatten().filter(|t| **t != id) {
                links.insert((id, target));
            }
        }
    }
    edges.extend(links.into_iter().map(|(a, b)| GraphEdge { kind: EdgeKind::Link, from: format!("note:{a}"), to: format!("note:{b}") }));

    nodes.sort_by(|a, b| (a.kind, &a.id).cmp(&(b.kind, &b.id)));
    edges.sort();
    edges.dedup();
    Graph { nodes, edges }
}

/// The ids of the nodes joined to `node_id` by an edge in either direction,
/// sorted. Empty for an id the graph does not have.
pub fn neighbours(graph: &Graph, node_id: &str) -> Vec<String> {
    let mut out = BTreeSet::new();
    for e in &graph.edges {
        if e.from == node_id {
            out.insert(e.to.clone());
        } else if e.to == node_id {
            out.insert(e.from.clone());
        }
    }
    out.into_iter().collect()
}
