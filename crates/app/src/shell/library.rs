//! Library (the destinations board, section 5): what Cauldron remembers. The sidebar holds its
//! views (Proposals, Notes, Graph, Librarian, then notes by folder); the page shows the one picked.
//!
//! Real: Notes from core's memory (read only if memory exists, opening it would create it) and the
//! Graph from core's `memory_graph::build_graph` (hubs Global, folders, agents; notes; their
//! `[[title]]` links), laid out here (`graph_layout`), hubs bigger, the picked note in a panel on
//! the right. Example until the Librarian is built (milestone 10): Proposals and its receipts.

use super::context::place;
use super::pages::{tag, AppPage};
use super::*;
use backspace_core::memory_graph::{build_graph, neighbours, EdgeKind, Graph, NodeKind};

/// The Library's views, by the sidebar's row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum View {
    Proposals,
    Notes(Option<usize>),
    Graph,
    Librarian,
}

/// A note's title: its first non-blank line.
fn title_of(text: &str) -> String {
    text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(80).collect()
}

impl Cauldron {
    pub(super) fn library_view(&self) -> View {
        match self.side_sel[AppPage::Library as usize] {
            Some(0) => View::Proposals,
            Some(3001) => View::Graph,
            Some(3002) => View::Librarian,
            Some(k) if (1000..2000).contains(&k) => View::Notes(Some(k - 1000)),
            _ => View::Notes(None),
        }
    }

    /// The places notes are in, in the order the sidebar lists them.
    fn note_places(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (_, p) in &self.side_data.notes {
            if !out.contains(p) {
                out.push(p.clone());
            }
        }
        out
    }

    /// The graph of the notes now: folders are the notes' own places, agents core's.
    pub(super) fn library_graph(&self) -> Graph {
        let mut folders: Vec<(String, String)> = Vec::new();
        for n in &self.side_data.lib {
            if let Some(p) = &n.project {
                if !p.starts_with("agent:") && !folders.iter().any(|(id, _)| id == p) {
                    folders.push((p.clone(), place(&n.project)));
                }
            }
        }
        build_graph(&self.side_data.lib, &folders, &self.side_data.agents)
    }

    pub(super) fn library_page(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let view = self.library_view();
        let body = match view {
            View::Notes(at) => self.notes_view(at, pal),
            View::Graph => self.graph_view(pal, cx),
            View::Proposals => self.proposals_view(pal, cx),
            View::Librarian => self.librarian_view(pal),
        };
        div().id("page-body").size_full().overflow_y_scroll().child(body).test_support().into_any_element()
    }

    fn lib_head(&self, title: String, sub: String, demo: bool, pal: &'static Palette) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().flex().items_center().gap(px(8.)).child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(pal.fg)).child(title)).when_live(demo, |d| d.child(tag("library-demo", "Example", pal).test_support())))
            .child(div().text_size(px(13.)).text_color(c(pal.fg3)).child(sub))
    }

    /// Notes: every note from memory, or one place's.
    fn notes_view(&self, at: Option<usize>, pal: &'static Palette) -> Div {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let places = self.note_places();
        let only = at.and_then(|i| places.get(i).cloned());
        let notes: Vec<&backspace_core::memory::Note> = self.side_data.lib.iter().filter(|n| only.as_ref().is_none_or(|p| &place(&n.project) == p)).collect();
        let mut list = div().id("note-list").flex().flex_col().gap(px(6.));
        for (k, n) in notes.iter().enumerate() {
            // Memory keeps a note on one line: its title is the whole note, wrapped.
            let title = n.text.trim().to_string();
            let rest: String = n.text.lines().skip_while(|l| l.trim().is_empty()).skip(1).collect::<Vec<_>>().join(" ");
            let chip = |t: String| div().px(px(6.)).h(px(18.)).flex().items_center().rounded(px(radius::TAG)).bg(c(pal.inset)).text_size(px(11.)).text_color(fg3).child(t);
            list = list.child(
                div()
                    .id(("note", k))
                    .p(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .rounded(px(14.))
                    .border_1()
                    .border_color(c(pal.line))
                    .aria_label(title.clone())
                    .child(div().text_size(px(13.5)).font_weight(FontWeight::MEDIUM).text_color(fg).child(title))
                    .when_live(!rest.trim().is_empty(), |d| d.child(div().text_size(px(12.5)).text_color(fg2).line_clamp(2).child(rest)))
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .mt(px(2.))
                            .child(chip(place(&n.project)))
                            .when_live(!n.source.is_empty(), |d| d.child(chip(format!("from {}", n.source))))
                            .when_live(!n.on, |d| d.child(chip("Off: not given to chats or agents".into()))),
                    )
                    .test_support(),
            );
        }
        if notes.is_empty() {
            list = list.child(div().id("notes-none").p(px(14.)).rounded(px(14.)).border_1().border_color(c(pal.line)).text_size(px(13.)).text_color(fg2).child("No notes yet. What Cauldron remembers from your streams, and what you tell it to remember, shows here.").test_support());
        }
        let title = only.clone().map_or_else(|| "Notes".to_string(), |p| format!("Notes · {p}"));
        div()
            .w_full()
            .max_w(px(760.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap(px(14.))
            .px(px(24.))
            .py(px(18.))
            .child(self.lib_head(title, format!("{} from memory", if notes.len() == 1 { "1 note".to_string() } else { format!("{} notes", notes.len()) }), false, pal))
            .child(list.test_support())
    }

    /// Graph: the hubs and notes, laid out here; the picked node's details on the right.
    fn graph_view(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Div {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let g = self.library_graph();
        let (bw, bh) = self.thread_box.get();
        let (gw, gh) = ((bw - 48.0 - 260.0).max(320.0), (bh - 150.0).max(300.0));
        let key = (g.nodes.len(), g.edges.len(), gw as u32, gh as u32);
        let pos = {
            let mut cache = self.graph_cache.borrow_mut();
            match cache.as_ref() {
                Some((k, p)) if *k == key => p.clone(),
                _ => {
                    let bodies: Vec<crate::graph_layout::Body> = g.nodes.iter().map(|n| crate::graph_layout::Body { hub: n.kind != NodeKind::Note }).collect();
                    let at = |id: &str| g.nodes.iter().position(|n| n.id == id);
                    let edges: Vec<(usize, usize)> = g.edges.iter().filter_map(|e| Some((at(&e.from)?, at(&e.to)?))).collect();
                    let p = crate::graph_layout::layout(&bodies, &edges, gw, gh);
                    *cache = Some((key, p.clone()));
                    p
                }
            }
        };
        let sel = self.graph_sel.clone();
        let near: Vec<String> = sel.as_ref().map(|s| neighbours(&g, s)).unwrap_or_default();
        let colour = |k: NodeKind| match k {
            NodeKind::Global => 0xa08cff,
            NodeKind::Folder => 0x5b8cf0,
            NodeKind::Agent => 0x8b5cf6,
            NodeKind::Note => 0x8a8a93,
        };
        // Edges in one picture; the picked node's in the accent.
        let idx = |id: &str| g.nodes.iter().position(|n| n.id == id);
        let line = if pal.panel.0 < 128 { "#ffffff" } else { "#000000" };
        let accent = format!("#{:02x}{:02x}{:02x}", pal.accent.0, pal.accent.1, pal.accent.2);
        let mut svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{gw}" height="{gh}" viewBox="0 0 {gw} {gh}">"#);
        for e in &g.edges {
            let (Some(a), Some(b)) = (idx(&e.from), idx(&e.to)) else { continue };
            let hot = sel.as_deref().is_some_and(|s| s == e.from || s == e.to);
            let (stroke, op, wdt) = if hot { (accent.as_str(), 0.9, 1.6) } else if e.kind == EdgeKind::Link { (accent.as_str(), 0.45, 1.2) } else { (line, 0.16, 1.0) };
            svg += &format!(r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{stroke}" stroke-opacity="{op}" stroke-width="{wdt}"/>"#, pos[a].0, pos[a].1, pos[b].0, pos[b].1);
        }
        svg += "</svg>";
        let mut area = div()
            .id("graph-area")
            .relative()
            .w(px(gw))
            .h(px(gh))
            .flex_none()
            .rounded(px(14.))
            .border_1()
            .border_color(c(pal.line))
            .child(img(Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))).absolute().top_0().left_0().w(px(gw)).h(px(gh)));
        for (i, n) in g.nodes.iter().enumerate() {
            let size = match n.kind {
                NodeKind::Global => 18.0,
                NodeKind::Folder | NodeKind::Agent => 13.0,
                NodeKind::Note => 7.0,
            };
            let on = sel.as_deref() == Some(n.id.as_str());
            let lit = on || near.contains(&n.id);
            let label = n.kind != NodeKind::Note || on;
            let id = n.id.clone();
            let mut col: Hsla = c(Rgb::hex(colour(n.kind)));
            if sel.is_some() && !lit && n.kind == NodeKind::Note {
                col.a = 0.5;
            }
            area = area.child(
                div()
                    .id(("graph-node", i))
                    .absolute()
                    .left(px(pos[i].0 - size / 2.0 - 4.0))
                    .top(px(pos[i].1 - size / 2.0 - 4.0))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .p(px(4.))
                    .cursor_pointer()
                    .aria_label(n.title.clone())
                    .aria_selected(on)
                    .child(div().size(px(size)).flex_none().rounded_full().bg(col).when_live(on, |d| d.border_2().border_color(c(pal.fg))))
                    .when_live(label, |d| d.child(div().max_w(px(220.)).truncate().text_size(px(if n.kind == NodeKind::Note { 11.5 } else { 12. })).font_weight(if n.kind == NodeKind::Note { FontWeight::NORMAL } else { FontWeight::MEDIUM }).text_color(if lit || n.kind != NodeKind::Note { fg } else { fg2 }).whitespace_nowrap().child(n.title.clone())))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.graph_sel = Some(id.clone());
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        // The panel: the picked note (or hub), what it links to.
        let panel = {
            let mut p = div().id("graph-panel").w(px(240.)).flex_none().flex().flex_col().gap(px(8.)).pl(px(16.));
            match sel.as_ref().and_then(|s| g.nodes.iter().find(|n| &n.id == s)) {
                Some(n) => {
                    let note = self.side_data.lib.iter().find(|x| n.kind == NodeKind::Note && x.id == n.ref_id);
                    let full = note.map_or_else(|| n.title.clone(), |x| x.text.trim().to_string());
                    p = p.child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(full));
                    if let Some(x) = note {
                        p = p
                            .child(div().flex().gap(px(6.)).child(div().px(px(6.)).rounded(px(radius::TAG)).bg(c(pal.inset)).text_size(px(11.)).text_color(fg3).child(place(&x.project))).when_live(!x.source.is_empty(), |d| d.child(div().px(px(6.)).rounded(px(radius::TAG)).bg(c(pal.inset)).text_size(px(11.)).text_color(fg3).child(format!("from {}", x.source)))))
                            .when_live(!x.on, |d| d.child(div().text_size(px(12.)).text_color(fg2).child("Off: not given to chats or agents")));
                    }
                    p = p.child(div().pt(px(6.)).text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child(if n.kind == NodeKind::Note { "Linked with" } else { "Holds" }));
                    for nb in neighbours(&g, &n.id).iter().take(12) {
                        if let Some(m) = g.nodes.iter().find(|m| &m.id == nb) {
                            p = p.child(div().flex().items_center().gap(px(6.)).text_size(px(12.5)).text_color(fg2).child(div().size(px(7.)).rounded_full().bg(c(Rgb::hex(colour(m.kind))))).child(div().truncate().child(m.title.clone())));
                        }
                    }
                }
                None => p = p.child(div().text_size(px(12.5)).text_color(fg3).child("Pick a note or a hub to see what it holds and what it links to.")),
            }
            p.test_support()
        };
        let legend = |t: &'static str, k: NodeKind| div().flex().items_center().gap(px(5.)).child(div().size(px(7.)).rounded_full().bg(c(Rgb::hex(colour(k))))).child(t);
        let notes = g.nodes.iter().filter(|n| n.kind == NodeKind::Note).count();
        let links = g.edges.iter().filter(|e| e.kind == EdgeKind::Link).count();
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .px(px(24.))
            .py(px(18.))
            .child(self.lib_head("Graph".into(), format!("{notes} notes, {links} {}. A note joins its place, and another note when it names it as [[its title]].", if links == 1 { "link" } else { "links" }), false, pal))
            .child(div().flex().gap(px(12.)).text_size(px(11.5)).text_color(fg3).child(legend("Global", NodeKind::Global)).child(legend("Folder", NodeKind::Folder)).child(legend("Agent", NodeKind::Agent)).child(legend("Note", NodeKind::Note)))
            .child(div().flex().items_start().child(area.test_support()).child(panel))
    }

    /// Proposals: what the Librarian will ask to keep. Not built (milestone 10): an example.
    fn proposals_view(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Div {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let off = |id: (&'static str, usize), t: &'static str| div().id(id).h(px(26.)).px(px(10.)).flex().items_center().rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line)).text_size(px(12.)).text_color(c(pal.fg4)).aria_label(format!("{t} (demo: not built yet)")).child(t);
        let mut list = div().flex().flex_col().gap(px(8.));
        for (k, (text, place, from)) in [("Every UI pull request carries before and after screenshots", "Global", "Fix policy tests"), ("workbench: the Windows build is published as windows-preview", "workbench", "PR 43 CI")].into_iter().enumerate() {
            list = list.child(
                div()
                    .id(("proposal", k))
                    .p(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .rounded(px(14.))
                    .border_1()
                    .border_color(c(pal.line))
                    .child(div().text_size(px(13.5)).font_weight(FontWeight::MEDIUM).text_color(fg).child(text))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(11.5))
                            .text_color(fg3)
                            .child(div().px(px(6.)).rounded(px(radius::TAG)).bg(c(pal.inset)).child(place))
                            .child(format!("from \u{201c}{from}\u{201d}"))
                            .child(tag(("proposal-ex", k), "Example", pal).test_support())
                            .child(div().flex_1())
                            .child(self.tipped(("proposal-reject", k), off(("proposal-reject", k), "Reject"), "Demo: not built yet", Side::Below, pal, cx).test_support())
                            .child(self.tipped(("proposal-accept", k), off(("proposal-accept", k), "Accept"), "Demo: not built yet", Side::Below, pal, cx).test_support()),
                    )
                    .test_support(),
            );
        }
        div()
            .w_full()
            .max_w(px(760.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap(px(14.))
            .px(px(24.))
            .py(px(18.))
            .child(self.lib_head("Proposals".into(), "What the Librarian asks to keep, merge or drop, for you to accept or reject. It arrives in milestone 10; these are examples.".into(), true, pal))
            .child(list)
    }

    /// The Librarian's receipts: every change it makes, as a commit you can undo. An example.
    fn librarian_view(&self, pal: &'static Palette) -> Div {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mut list = div().flex().flex_col().gap(px(8.));
        for (k, (what, receipt)) in [("At the Commit of Round 3 build: kept 3 notes, merged 1, dropped 2", "c4f1e2"), ("At the Commit of PR 43 CI: kept 1 note", "9a07bd")].into_iter().enumerate() {
            list = list.child(
                div()
                    .id(("receipt", k))
                    .p(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .rounded(px(14.))
                    .border_1()
                    .border_color(c(pal.line))
                    .child(pages::figure(3, cauldron_chrome::avatar::Mood::Reading, 22., false))
                    .child(div().flex_1().min_w_0().flex().flex_col().child(div().text_size(px(13.)).text_color(fg).child(what)).child(div().text_size(px(11.5)).text_color(fg3).child(format!("Receipt {receipt}. Every change is a commit you can revert."))))
                    .child(tag(("receipt-ex", k), "Example", pal).test_support())
                    .test_support(),
            );
        }
        div()
            .w_full()
            .max_w(px(760.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap(px(14.))
            .px(px(24.))
            .py(px(18.))
            .child(self.lib_head("Librarian".into(), "What the Librarian did and why. It arrives in milestone 10; these are examples.".into(), true, pal))
            .child(list)
            .child(div().text_size(px(12.)).text_color(fg2).child("Memory itself is real today: Notes and Graph read it."))
    }
}

#[cfg(test)]
mod tests {
    use super::title_of;

    /// Notes and the Graph read core's memory (here the test's own): every note listed; the graph
    /// has Global, a hub per folder, a node per note, and a [[link]] between two notes; picking a
    /// note shows it in the panel. Proposals and the Librarian's receipts are marked Example.
    #[gpui_kit::test]
    fn the_library_reads_memory(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        use gpui_kit::AppContext;
        let (handle, _chats) = super::super::thread::tests::open(cx);
        let dir = handle.update(cx, |c, _, _| c.data_dir()).unwrap();
        let m = backspace_core::memory::Memory::open(dir);
        // Memory keeps a note on one line; its title is its start.
        m.add("Screenshots first", None, "you").unwrap();
        m.add("Windows preview is published per build, see [[Screenshots first]]", Some("/code/workbench".into()), "chat").unwrap();
        handle
            .update(cx, |c, _, _| {
                c.open_page(super::AppPage::Library);
                c.motion = Default::default();
            })
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let titles: Vec<String> = (0..2usize).map(|i| window.find(("note", i)).label().unwrap().to_string()).collect();
            assert!(titles.contains(&"Screenshots first".to_string()) && titles.iter().any(|t| t.starts_with("Windows preview")), "{titles:?}");
            assert_eq!(window.find(("lib-notes", 0usize)).selected(), Some(true), "Notes first");
            window.click(("lib-graph", 0usize), cx);
            window.render_frame(cx);
            let names: Vec<String> = (0usize..).map_while(|i| window.try_find(("graph-node", i)).map(|e| e.label().unwrap_or_default().to_string())).collect();
            assert!(names.contains(&"Global".to_string()) && names.contains(&"workbench".to_string()), "hubs: {names:?}");
            assert_eq!(names.len(), 4, "Global, workbench, two notes: {names:?}");
            let at = names.iter().position(|n| n.starts_with("Windows preview")).unwrap();
            window.click(("graph-node", at), cx);
            window.render_frame(cx);
            assert_eq!(window.find(("graph-node", at)).selected(), Some(true));
            window.click(("lib-proposals", 0usize), cx);
            window.render_frame(cx);
            assert!(window.try_find("library-demo").is_some() && window.try_find(("proposal-ex", 0usize)).is_some(), "Proposals are examples");
        })
        .unwrap();
        let g = handle.update(cx, |c, _, _| c.library_graph()).unwrap();
        assert!(g.edges.iter().any(|e| e.kind == backspace_core::memory_graph::EdgeKind::Link), "the [[link]] is an edge");
    }

    #[test]
    fn a_notes_title_is_its_first_line() {
        assert_eq!(title_of("\n  Screenshots first \nEvery UI PR"), "Screenshots first");
        assert_eq!(title_of(""), "");
    }
}
