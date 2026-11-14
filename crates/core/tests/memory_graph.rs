//! The Library graph (memory_graph.rs): what Cauldron remembers as hubs and
//! notes. Edges are only places a note belongs to and `[[title]]` references
//! a note's text makes; nothing is guessed from similar wording.

use std::time::Instant;

use backspace_core::memory::Note;
use backspace_core::memory_graph::{build_graph, neighbours, EdgeKind, Graph, GraphNode, NodeKind};

fn note(id: &str, text: &str, project: Option<&str>) -> Note {
    Note {
        id: id.into(),
        text: text.into(),
        project: project.map(String::from),
        source: "you".into(),
        thread: None,
        created: 1,
        updated: 1,
        on: true,
        kind: String::new(),
        importance: 1.0,
        uses: 0,
        features: vec![],
    }
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.to_string(), b.to_string())
}

fn ids(g: &Graph, kind: NodeKind) -> Vec<String> {
    g.nodes.iter().filter(|n| n.kind == kind).map(|n| n.id.clone()).collect()
}

fn edges(g: &Graph, kind: EdgeKind) -> Vec<(String, String)> {
    g.edges.iter().filter(|e| e.kind == kind).map(|e| (e.from.clone(), e.to.clone())).collect()
}

fn node<'a>(g: &'a Graph, id: &str) -> &'a GraphNode {
    g.nodes.iter().find(|n| n.id == id).unwrap_or_else(|| panic!("no node {id} in {:?}", g.nodes))
}

#[test]
fn empty_input_has_only_the_global_hub() {
    let g = build_graph(&[], &[], &[]);
    assert_eq!(ids(&g, NodeKind::Global), vec!["global"]);
    assert_eq!(g.nodes.len(), 1);
    assert!(g.edges.is_empty());
    assert_eq!(g.stats(), (1, 0));
    assert!(neighbours(&g, "global").is_empty());
}

#[test]
fn hubs_exist_only_for_folders_and_agents_that_exist() {
    let notes = vec![
        note("n1", "in a", Some("/w/a")),
        note("n2", "by kira", Some("agent:kira")),
        note("n3", "in a folder that is gone", Some("/w/c")),
        note("n4", "by an agent that is gone", Some("agent:ghost")),
    ];
    let folders = vec![pair("/w/a", "a"), pair("/w/b", "b")];
    let agents = vec![pair("kira", "Kira")];
    let g = build_graph(&notes, &folders, &agents);

    assert_eq!(ids(&g, NodeKind::Folder), vec!["folder:/w/a", "folder:/w/b"]);
    assert_eq!(ids(&g, NodeKind::Agent), vec!["agent:kira"]);
    assert_eq!(ids(&g, NodeKind::Global), vec!["global"]);
    assert_eq!(ids(&g, NodeKind::Note), vec!["note:n1", "note:n2", "note:n3", "note:n4"]);
    // A hub with no notes is still a hub; the names are the places' names.
    assert_eq!(node(&g, "folder:/w/b").title, "b");
    assert_eq!(node(&g, "agent:kira").title, "Kira");
    // Global to each hub, and nothing to a place that does not exist.
    assert_eq!(
        edges(&g, EdgeKind::Hub),
        vec![pair("global", "agent:kira"), pair("global", "folder:/w/a"), pair("global", "folder:/w/b")]
    );
    assert_eq!(g.stats(), (4 + 4, 3 + 4));
}

#[test]
fn every_note_attaches_to_its_place() {
    let notes = vec![
        note("g", "everywhere", None),
        note("f", "in a folder", Some("/w/a")),
        note("a", "kept by kira", Some("agent:kira")),
    ];
    let g = build_graph(&notes, &[pair("/w/a", "a")], &[pair("kira", "Kira")]);
    assert_eq!(
        edges(&g, EdgeKind::Place),
        vec![pair("note:a", "agent:kira"), pair("note:f", "folder:/w/a"), pair("note:g", "global")]
    );
}

#[test]
fn a_note_whose_place_is_gone_attaches_to_global() {
    let notes = vec![
        note("o1", "folder was removed", Some("/w/gone")),
        note("o2", "agent was removed", Some("agent:ghost")),
        note("o3", "no place at all", None),
        note("o4", "the word global", Some("global")),
    ];
    let g = build_graph(&notes, &[], &[]);
    assert_eq!(
        edges(&g, EdgeKind::Place),
        vec![pair("note:o1", "global"), pair("note:o2", "global"), pair("note:o3", "global"), pair("note:o4", "global")]
    );
    assert!(ids(&g, NodeKind::Folder).is_empty());
    assert!(ids(&g, NodeKind::Agent).is_empty());
}

#[test]
fn note_nodes_carry_id_short_title_and_source() {
    let mut n = note("k3x9", "  Prefers tabs  \nand a second line that is not the title", None);
    n.source = "chat".into();
    let g = build_graph(&[n], &[], &[]);
    let node = node(&g, "note:k3x9");
    assert_eq!(node.kind, NodeKind::Note);
    assert_eq!(node.ref_id, "k3x9");
    assert_eq!(node.title, "Prefers tabs");
    assert_eq!(node.source, "chat");
}

#[test]
fn titles_are_cut_to_sixty_characters_without_splitting_one() {
    let long = "a".repeat(80);
    let wide = "é".repeat(70);
    let g = build_graph(&[note("l", &long, None), note("w", &wide, None)], &[], &[]);
    assert_eq!(node(&g, "note:l").title, "a".repeat(60));
    assert_eq!(node(&g, "note:w").title.chars().count(), 60);
    assert_eq!(node(&g, "note:w").title, "é".repeat(60));
    // Leading blank lines do not make an empty label.
    let g = build_graph(&[note("b", "\n\n  Second line wins  \nthird", None)], &[], &[]);
    assert_eq!(node(&g, "note:b").title, "Second line wins");
}

#[test]
fn links_resolve_case_insensitively() {
    let notes = vec![
        note("a", "Prefers tabs", None),
        note("b", "Style: see [[PREFERS TABS]] first", None),
        note("c", "Also [[ prefers tabs ]] with stray spaces", None),
    ];
    let g = build_graph(&notes, &[], &[]);
    assert_eq!(edges(&g, EdgeKind::Link), vec![pair("note:b", "note:a"), pair("note:c", "note:a")]);
}

#[test]
fn unknown_links_make_no_edge_and_no_node() {
    let notes = vec![note("a", "Prefers tabs", None), note("b", "see [[Nothing like this]]", None)];
    let g = build_graph(&notes, &[], &[]);
    assert!(edges(&g, EdgeKind::Link).is_empty());
    assert_eq!(ids(&g, NodeKind::Note), vec!["note:a", "note:b"]);
}

#[test]
fn a_note_that_links_itself_gets_no_self_edge() {
    let g = build_graph(&[note("s", "Self [[self]]", None)], &[], &[]);
    assert!(edges(&g, EdgeKind::Link).is_empty());

    // The title is the first line: a note can link its own title from line two.
    let g = build_graph(&[note("s", "Self\nsee [[SELF]]", None), note("t", "Other\nsee [[self]]", None)], &[], &[]);
    assert_eq!(edges(&g, EdgeKind::Link), vec![pair("note:t", "note:s")]);
}

#[test]
fn repeated_links_collapse_to_one_edge() {
    let notes = vec![note("a", "Prefers tabs", None), note("b", "[[Prefers tabs]] and again [[prefers TABS]] and [[Prefers tabs]]", None)];
    let g = build_graph(&notes, &[], &[]);
    assert_eq!(edges(&g, EdgeKind::Link), vec![pair("note:b", "note:a")]);
}

#[test]
fn two_notes_with_one_title_are_both_linked() {
    let notes = vec![note("a1", "Deploy", None), note("a2", "deploy\nsecond line", None), note("b", "run [[Deploy]]", None)];
    let g = build_graph(&notes, &[], &[]);
    assert_eq!(edges(&g, EdgeKind::Link), vec![pair("note:b", "note:a1"), pair("note:b", "note:a2")]);
}

#[test]
fn links_both_ways_stay_two_edges() {
    let notes = vec![note("a", "Alpha\n[[Beta]]", None), note("b", "Beta\n[[Alpha]]", None)];
    let g = build_graph(&notes, &[], &[]);
    assert_eq!(edges(&g, EdgeKind::Link), vec![pair("note:a", "note:b"), pair("note:b", "note:a")]);
}

#[test]
fn a_title_is_the_whole_first_line() {
    // "Alpha [[Beta]]" is titled with its whole first line, so a link to
    // "Alpha" does not reach it.
    let notes = vec![note("a", "Alpha [[Beta]]", None), note("b", "Beta [[Alpha]]", None), note("c", "see [[Alpha]]", None)];
    let g = build_graph(&notes, &[], &[]);
    assert!(edges(&g, EdgeKind::Link).is_empty(), "{:?}", edges(&g, EdgeKind::Link));
}

#[test]
fn no_edge_comes_from_similar_wording_or_partial_titles() {
    let notes = vec![
        note("a", "Prefers tabs over spaces", None),
        note("b", "Prefers tabs over spaces in Rust files", None),
        note("c", "a bare mention of prefers tabs over spaces without brackets", None),
        note("d", "partial [[tabs]] and [[Prefers tabs]] and [[spaces]]", None),
    ];
    let g = build_graph(&notes, &[], &[]);
    assert!(edges(&g, EdgeKind::Link).is_empty(), "{:?}", edges(&g, EdgeKind::Link));
}

#[test]
fn malformed_brackets_are_ignored() {
    let notes = vec![
        note("a", "Target", None),
        note("b", "unclosed [[Target", None),
        note("c", "across lines [[Tar\nget]]", None),
        note("d", "empty [[]] and blank [[   ]]", None),
        note("e", "single [Target] and (Target)", None),
        note("f", "nested [[ignore [[Target]]", None),
    ];
    let g = build_graph(&notes, &[], &[]);
    // Only the innermost, closed pair in f is a reference.
    assert_eq!(edges(&g, EdgeKind::Link), vec![pair("note:f", "note:a")]);
}

#[test]
fn a_duplicated_note_id_makes_one_node_whatever_the_order() {
    // Ids are unique in a real store. If two ever share one, the most
    // recently updated wins, and the input order never decides.
    let mut newer = note("a", "newer", None);
    newer.updated = 9;
    let older = note("a", "older", None);
    for notes in [vec![older.clone(), newer.clone()], vec![newer.clone(), older.clone()]] {
        let g = build_graph(&notes, &[], &[]);
        assert_eq!(ids(&g, NodeKind::Note), vec!["note:a"]);
        assert_eq!(node(&g, "note:a").title, "newer");
    }
}

#[test]
fn neighbours_are_hub_and_linked_notes_both_directions() {
    let notes = vec![
        note("a", "Alpha", Some("/w/p")),
        note("b", "Beta\nsee [[Alpha]]", Some("/w/p")),
        note("c", "Gamma\nsee [[Alpha]] and [[Beta]]", None),
    ];
    let g = build_graph(&notes, &[pair("/w/p", "p")], &[]);
    assert_eq!(neighbours(&g, "note:a"), vec!["folder:/w/p", "note:b", "note:c"]);
    assert_eq!(neighbours(&g, "note:c"), vec!["global", "note:a", "note:b"]);
    assert_eq!(neighbours(&g, "folder:/w/p"), vec!["global", "note:a", "note:b"]);
    assert_eq!(neighbours(&g, "global"), vec!["folder:/w/p", "note:c"]);
    assert!(neighbours(&g, "note:missing").is_empty());
}

#[test]
fn stats_count_nodes_and_edges() {
    let notes = vec![note("a", "Alpha", None), note("b", "Beta\n[[Alpha]]", Some("/w/p"))];
    let g = build_graph(&notes, &[pair("/w/p", "p")], &[]);
    // global, folder, two notes; one hub edge, two place edges, one link.
    assert_eq!(g.stats(), (4, 4));
    assert_eq!((g.nodes.len(), g.edges.len()), (4, 4));
}

#[test]
fn output_is_deterministic_whatever_the_input_order() {
    let notes: Vec<Note> = (0..40)
        .map(|i| {
            let place = match i % 4 {
                0 => None,
                1 => Some("/w/a"),
                2 => Some("agent:kira"),
                _ => Some("/w/gone"),
            };
            note(&format!("n{i:02}"), &format!("Note {i}\nsee [[Note {}]] and [[note {}]]", (i + 1) % 40, (i + 7) % 40), place)
        })
        .collect();
    let folders = vec![pair("/w/b", "b"), pair("/w/a", "a")];
    let agents = vec![pair("zed", "Zed"), pair("kira", "Kira")];

    let first = build_graph(&notes, &folders, &agents);
    let second = build_graph(&notes, &folders, &agents);
    assert_eq!(first, second);

    let mut rev_notes = notes.clone();
    rev_notes.reverse();
    let mut rev_folders = folders.clone();
    rev_folders.reverse();
    let mut rev_agents = agents.clone();
    rev_agents.reverse();
    let shuffled = build_graph(&rev_notes, &rev_folders, &rev_agents);
    assert_eq!(first, shuffled);

    // Sorted: Global, folders, agents, notes; each by id. Edges by kind, then ends.
    let kinds: Vec<NodeKind> = first.nodes.iter().map(|n| n.kind).collect();
    let mut sorted = kinds.clone();
    sorted.sort();
    assert_eq!(kinds, sorted);
    for k in [NodeKind::Folder, NodeKind::Agent, NodeKind::Note] {
        let v = ids(&first, k);
        let mut s = v.clone();
        s.sort();
        assert_eq!(v, s);
    }
    let keys: Vec<(EdgeKind, String, String)> = first.edges.iter().map(|e| (e.kind, e.from.clone(), e.to.clone())).collect();
    let mut sorted_keys = keys.clone();
    sorted_keys.sort();
    assert_eq!(keys, sorted_keys);
    assert!(sorted_keys.windows(2).all(|w| w[0] != w[1]), "no duplicate edges");
}

#[test]
fn three_hundred_notes_build_well_under_a_second() {
    let notes: Vec<Note> = (0..300)
        .map(|i| {
            let place = match i % 5 {
                0 => None,
                1 => Some("/w/a"),
                2 => Some("/w/b"),
                3 => Some("agent:kira"),
                _ => Some("agent:ghost"),
            };
            note(&format!("n{i:03}"), &format!("Note {i} about something\nsee [[Note {} about something]] and [[note {} about something]]", (i * 7) % 300, (i + 1) % 300), place)
        })
        .collect();
    let folders = vec![pair("/w/a", "a"), pair("/w/b", "b")];
    let agents = vec![pair("kira", "Kira")];
    let t = Instant::now();
    let g = build_graph(&notes, &folders, &agents);
    let took = t.elapsed();
    assert_eq!(ids(&g, NodeKind::Note).len(), 300);
    assert!(edges(&g, EdgeKind::Link).len() > 300);
    assert!(took.as_millis() < 250, "took {took:?}");
}
