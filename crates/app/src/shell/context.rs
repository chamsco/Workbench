//! The sidebar follows the destination (the context-sidebar board, direction A; Charles, 13 Nov
//! 18:57Z). The top row, the six buttons and the footer stay; the zone between them is the open
//! destination's own: Streams as before; Agents with New chat, New group, its agents and its crews;
//! Folders, Automations, Usage and Library with their views and lists. Outside Streams a one-line
//! pill says how many streams need you.
//!
//! What core backs shows plainly: the streams and their folders, core's agents, the agents'
//! routines, the coding tools found on this machine, the notes in memory. What it does not (crews,
//! the Librarian's proposals, templates, alerts) is marked Example, and actions not built yet are
//! disabled with "Demo: not built yet" (UI-PLAN decision 5).

use super::pages::{tag, AppPage};
use super::*;

/// What the destinations' lists read from core, loaded when the destination opens.
#[derive(Default)]
pub(crate) struct SideData {
    /// Every agent's routines: (agent, name, when, last run ms, runs).
    pub routines: Vec<(String, String, String, u64, u32)>,
    /// The coding tools core reads usage from, found on this machine.
    pub tools: Vec<&'static str>,
    /// The notes in memory: (text, place).
    pub notes: Vec<(String, String)>,
}

/// A note's place, as the Library groups them: Global, a folder, or the agent's own.
fn place(project: &Option<String>) -> String {
    match project {
        None => "Global".into(),
        Some(p) if p.starts_with("agent:") => "Agents".into(),
        Some(p) => p.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(p).to_string(),
    }
}

impl SideData {
    /// Reads core, without creating anything: no memory folder, no notes.
    pub(crate) fn load(page: AppPage) -> SideData {
        let dir = Prefs::data_dir();
        let mut d = SideData::default();
        match page {
            AppPage::Automations => {
                for a in backspace_core::agents::Agents::open(dir.join("agents")).list() {
                    for r in a.routines {
                        d.routines.push((a.name.clone(), r.name.clone(), r.summary(), r.last_run, r.runs));
                    }
                }
            }
            AppPage::Usage => {
                let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
                d.tools = backspace_core::usage::sources(std::path::Path::new(&home)).into_iter().filter(|s| s.found && s.supported).map(|s| s.name).collect();
            }
            AppPage::Library | AppPage::Folders => d.notes = notes(),
            _ => {}
        }
        d
    }
}

/// The notes in memory, if memory exists (opening it would create it).
pub(crate) fn notes() -> Vec<(String, String)> {
    let dir = Prefs::data_dir();
    if !dir.join("memory").exists() {
        return Vec::new();
    }
    backspace_core::memory::Memory::open(dir).list().into_iter().map(|n| (n.text, place(&n.project))).collect()
}

/// How many streams need you, for the pill.
pub(crate) fn needing(list: &[backspace_core::chat::ThreadInfo]) -> usize {
    list.iter().filter(|t| threads::needs_you(t)).count()
}

impl Cauldron {
    /// "2 need you in Streams": outside Streams, when any does; a click goes to Streams.
    pub(super) fn needs_pill(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        if self.page == AppPage::Streams || self.settings.is_some() {
            return None;
        }
        let n = match &self.threads {
            Threads::Ready(v) => needing(v),
            _ => 0,
        };
        (n > 0).then(|| {
            div()
                .id("needs-pill")
                .h(px(30.))
                .mb(px(6.))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .rounded(px(radius::TAG))
                .bg(c(pal.accent_soft))
                .cursor_pointer()
                .hover(|d| d.opacity(0.85))
                .text_size(px(12.5))
                .text_color(c(pal.fg2))
                .aria_label(format!("{n} need you in Streams"))
                .child(div().size(px(7.)).flex_none().rounded_full().bg(c(pal.accent)))
                .child(div().flex_1().min_w_0().truncate().child(format!("{n} need{} you in Streams", if n == 1 { "s" } else { "" })))
                .child(glyph("chevron-right", 12., c(pal.fg3)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.open_page(AppPage::Streams);
                    cx.notify();
                }))
        })
    }

    /// The zone under the buttons for a destination other than Streams.
    pub(super) fn side_zone(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let zone = div().id("side-zone").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap(px(1.));
        match self.page {
            AppPage::Agents => self.agents_side(zone, pal, cx),
            AppPage::Folders => self.folders_side(zone, pal, cx),
            AppPage::Automations => self.automations_side(zone, pal, cx),
            AppPage::Usage => self.usage_side(zone, pal, cx),
            _ => self.library_side(zone, pal, cx),
        }
    }

    /// A create action: New chat, New folder... None of these are built yet, so each is disabled
    /// and says so.
    fn side_action(&self, id: &'static str, icon: &'static str, label: &'static str, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let el = div()
            .id(id)
            .h(px(30.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(9.))
            .px(px(8.))
            .rounded(px(radius::TAG))
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(pal.fg4))
            .aria_label(format!("{label} (demo: not built yet)"))
            .child(glyph(icon, 15., c(pal.fg4)))
            .child(label);
        self.tipped((id, 1usize), el, "Demo: not built yet", Side::Right, pal, cx)
    }

    /// A row in a destination's list, selected like a stream's; `n` is the list's own index.
    #[allow(clippy::too_many_arguments)]
    fn side_row(&self, id: (&'static str, usize), lead: AnyElement, title: String, sub: Option<String>, end: Option<AnyElement>, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let page = self.page as usize;
        // Until a row is picked, the destination's main view is (the board: All folders,
        // Templates, Overview, Notes).
        let on = match self.side_sel[page] {
            Some(k) => k == id.1 + slot(id.0),
            None => id.1 == 0 && matches!(id.0, "all-folders" | "templates" | "usage-overview" | "lib-notes"),
        };
        let tall = sub.is_some();
        div()
            .id(id)
            .h(px(if tall { 40. } else { 30. }))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(9.))
            .px(px(8.))
            .rounded(px(radius::TAG))
            .cursor_pointer()
            .when_live(on, |d| d.bg(c(pal.selected)))
            .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
            .aria_label(title.clone())
            .aria_selected(on)
            .child(lead)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().truncate().text_size(px(13.)).text_color(c(pal.fg)).child(title))
                    .when_opt(sub, |d, s| d.child(div().truncate().text_size(px(11.)).text_color(c(pal.fg3)).child(s))),
            )
            .when_opt(end, |d, e| d.child(e))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.side_sel[page] = Some(id.1 + slot(id.0));
                cx.notify();
            }))
    }

    fn side_head(&self, t: &'static str, n: Option<usize>, pal: &'static Palette) -> Div {
        div()
            .h(px(28.))
            .mt(px(6.))
            .flex_none()
            .flex()
            .items_end()
            .justify_between()
            .px(px(8.))
            .pb(px(5.))
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(pal.fg3))
            .child(t)
            .when_opt(n, |d, n| d.child(n.to_string()))
    }

    fn count(&self, n: String, pal: &'static Palette) -> AnyElement {
        div().flex_none().text_size(px(11.5)).text_color(c(pal.fg3)).child(n).into_any_element()
    }

    /// Agents: New chat, New group, the agents (core's, then the examples), the crews (examples).
    fn agents_side(&self, mut z: Stateful<Div>, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        z = z.child(self.side_action("new-chat", "square-pen", "New chat", pal, cx).test_support()).child(self.side_action("new-group", "plus", "New group", pal, cx).test_support());
        let roster = self.roster();
        let real = roster.iter().filter(|a| !a.example).count();
        z = z.child(self.side_head("Agents", Some(roster.len()), pal));
        for (i, a) in roster.iter().enumerate() {
            let on = self.agent_sel == Some(i);
            z = z.child(
                div()
                    .id(("agent-row", i))
                    .h(px(40.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .px(px(8.))
                    .rounded(px(radius::TAG))
                    .cursor_pointer()
                    .when_live(on, |d| d.bg(c(pal.selected)))
                    .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                    .aria_label(a.name.clone())
                    .aria_selected(on)
                    .child(pages::figure(a.dot, a.mood, 22., false))
                    .child(div().flex_1().min_w_0().flex().flex_col().child(div().truncate().text_size(px(13.)).text_color(c(pal.fg)).child(a.name.clone())).child(div().truncate().text_size(px(11.)).text_color(c(pal.fg3)).child(a.status.clone())))
                    .when_live(a.example, |d| d.child(tag(("agent-example", i), "Example", pal).test_support()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.agent_sel = Some(i);
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        z = z.child(self.side_head("Crew", Some(pages::ROOMS.len()), pal));
        for (r, (name, members, _)) in pages::ROOMS.iter().enumerate() {
            let on = self.agent_sel.is_none() && self.room_sel == r;
            let mut faces = div().flex().w(px(38.)).flex_none();
            for m in members {
                let a = &roster[real + m];
                faces = faces.child(div().ml(px(-4.)).child(pages::figure(a.dot, a.mood, 16., false)));
            }
            z = z.child(
                div()
                    .id(("room-row", r))
                    .h(px(40.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(8.))
                    .pl(px(12.))
                    .rounded(px(radius::TAG))
                    .cursor_pointer()
                    .when_live(on, |d| d.bg(c(pal.selected)))
                    .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                    .aria_label(*name)
                    .aria_selected(on)
                    .child(faces)
                    .child(div().flex_1().min_w_0().flex().flex_col().child(div().truncate().text_size(px(13.)).text_color(c(pal.fg)).child(*name)).child(div().truncate().text_size(px(11.)).text_color(c(pal.fg3)).child(format!("{} agents", members.len()))))
                    .child(tag(("room-example", r), "Example", pal).test_support())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.agent_sel = None;
                        this.room_sel = r;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        z
    }

    /// Folders: New folder, All folders, the folders the streams are in, the Librarian's proposal.
    fn folders_side(&self, mut z: Stateful<Div>, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let list = match &self.threads {
            Threads::Ready(v) => threads::by_folder(v, "", &self.filing).into_iter().map(|(n, ts)| (n, ts.len())).collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        // New folder works now (core's folders): you name it.
        z = z.child(
            div()
                .id("new-folder")
                .h(px(30.))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(9.))
                .px(px(8.))
                .rounded(px(radius::TAG))
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(pal.fg))
                .aria_label("New folder")
                .child(glyph("plus", 15., c(pal.fg)))
                .child("New folder")
                .on_click(cx.listener(|this, _, window, cx| this.ask_folder_name(None, false, window, cx)))
                .test_support(),
        );
        z = z.child(self.side_row(("all-folders", 0), glyph("layers", 15., c(pal.fg2)).into_any_element(), "All folders".into(), None, Some(self.count(list.len().to_string(), pal)), pal, cx).test_support());
        z = z.child(self.side_head("Folders", None, pal));
        for (i, (name, n)) in list.iter().enumerate() {
            let col: Hsla = c(Rgb::hex(self.filing.colour(name)));
            z = z.child(self.side_row(("folder-row", i), glyph("folder", 15., col).into_any_element(), name.clone(), None, Some(self.count(n.to_string(), pal)), pal, cx).test_support());
        }
        if list.is_empty() {
            z = z.child(div().px(px(8.)).text_size(px(12.)).text_color(c(pal.fg3)).child("No folders yet."));
        }
        z = z.child(self.side_head("Proposed by the Librarian", None, pal));
        z.child(self.side_row(("proposed-folder", 0), glyph("folder-plus", 15., c(pal.fg3)).into_any_element(), "Site copy".into(), None, Some(tag("proposed-example", "Example", pal).test_support().into_any_element()), pal, cx).test_support())
    }

    /// Automations: New automation, Templates (examples), the agents' routines, their last runs.
    fn automations_side(&self, mut z: Stateful<Div>, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        z = z.child(self.side_action("new-automation", "plus", "New automation", pal, cx).test_support());
        z = z.child(self.side_row(("templates", 0), glyph("layers", 15., c(pal.fg2)).into_any_element(), "Templates".into(), None, Some(tag("templates-example", "Example", pal).test_support().into_any_element()), pal, cx).test_support());
        let rs = &self.side_data.routines;
        z = z.child(self.side_head("Yours", Some(rs.len()), pal));
        for (i, (agent, name, when, _, _)) in rs.iter().enumerate() {
            z = z.child(self.side_row(("routine-row", i), glyph("zap", 15., c(pal.fg2)).into_any_element(), name.clone(), Some(format!("{agent} · {when}")), None, pal, cx).test_support());
        }
        if rs.is_empty() {
            z = z.child(div().id("routines-empty").px(px(8.)).text_size(px(12.)).text_color(c(pal.fg3)).child("No routines yet. An agent's routines show here.").test_support());
        }
        z = z.child(self.side_head("Recent runs", None, pal));
        let now = now_ms();
        let mut ran: Vec<_> = rs.iter().filter(|r| r.3 > 0).collect();
        ran.sort_by_key(|r| std::cmp::Reverse(r.3));
        for (i, (_, name, _, last, runs)) in ran.iter().take(5).enumerate() {
            z = z.child(self.side_row(("run-row", i), glyph("clock", 15., c(pal.fg3)).into_any_element(), format!("{name} · {} ago", threads::age(*last, now)), None, Some(self.count(format!("{runs} runs"), pal)), pal, cx).test_support());
        }
        if ran.is_empty() {
            z = z.child(div().px(px(8.)).text_size(px(12.)).text_color(c(pal.fg3)).child("Nothing has run yet."));
        }
        z
    }

    /// Usage: Overview, the tools found on this machine (no numbers until core reports them),
    /// alerts (example).
    fn usage_side(&self, mut z: Stateful<Div>, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        z = z.child(self.side_row(("usage-overview", 0), glyph("layers", 15., c(pal.fg2)).into_any_element(), "Overview".into(), None, None, pal, cx).test_support());
        let tools = &self.side_data.tools;
        z = z.child(self.side_head("Tools", Some(tools.len()), pal));
        for (i, name) in tools.iter().enumerate() {
            let ring = div().size(px(14.)).flex_none().rounded_full().border_2().border_color(c(pal.line_strong)).into_any_element();
            z = z.child(self.side_row(("tool-row", i), ring, (*name).into(), None, Some(self.count("No data yet".into(), pal)), pal, cx).test_support());
        }
        if tools.is_empty() {
            z = z.child(div().id("tools-empty").px(px(8.)).text_size(px(12.)).text_color(c(pal.fg3)).child("No coding tools found on this machine.").test_support());
        }
        z = z.child(self.side_head("Alerts", None, pal));
        z.child(self.side_row(("alert-row", 0), glyph("bell", 15., c(pal.fg3)).into_any_element(), "Limit alerts".into(), None, Some(tag("alerts-example", "Example", pal).test_support().into_any_element()), pal, cx).test_support())
    }

    /// Library: New note, its four views, then the notes by place (from memory).
    fn library_side(&self, mut z: Stateful<Div>, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let notes = &self.side_data.notes;
        z = z.child(self.side_action("new-note", "plus", "New note", pal, cx).test_support());
        z = z.child(self.side_row(("lib-proposals", 0), glyph("check", 15., c(pal.fg2)).into_any_element(), "Proposals".into(), None, Some(tag("proposals-example", "Example", pal).test_support().into_any_element()), pal, cx).test_support());
        z = z.child(self.side_row(("lib-notes", 0), glyph("database", 15., c(pal.fg2)).into_any_element(), "Notes".into(), None, Some(self.count(notes.len().to_string(), pal)), pal, cx).test_support());
        z = z.child(self.side_row(("lib-graph", 0), glyph("git-branch", 15., c(pal.fg2)).into_any_element(), "Graph".into(), None, None, pal, cx).test_support());
        z = z.child(self.side_row(("lib-librarian", 0), glyph("refresh-cw", 15., c(pal.fg2)).into_any_element(), "Librarian".into(), None, None, pal, cx).test_support());
        z = z.child(self.side_head("Notes by folder", None, pal));
        let mut places: Vec<(String, usize)> = Vec::new();
        for (_, p) in notes {
            match places.iter_mut().find(|(q, _)| q == p) {
                Some((_, n)) => *n += 1,
                None => places.push((p.clone(), 1)),
            }
        }
        for (i, (p, n)) in places.iter().enumerate() {
            let col: Hsla = c(Rgb::hex(if p == "Global" { folder_colour(p) } else { self.filing.colour(p) }));
            z = z.child(self.side_row(("note-folder", i), glyph("folder", 15., col).into_any_element(), p.clone(), None, Some(self.count(n.to_string(), pal)), pal, cx).test_support());
        }
        if places.is_empty() {
            z = z.child(div().id("notes-empty").px(px(8.)).text_size(px(12.)).text_color(c(pal.fg3)).child("No notes yet. What Cauldron remembers shows here.").test_support());
        }
        z
    }
}

/// Rows of different lists share one selection per destination: each list its own range.
fn slot(list: &str) -> usize {
    match list {
        "folder-row" | "routine-row" | "tool-row" | "note-folder" => 1000,
        "run-row" | "proposed-folder" | "alert-row" => 2000,
        "templates" | "lib-notes" => 3000,
        "lib-graph" => 3001,
        "lib-librarian" => 3002,
        _ => 0,
    }
}

/// A folder's colour, the destinations board's palette for the glass folders (violet, green,
/// amber, blue, orange), the same each time for the same name; Unfiled and Global grey.
pub(crate) fn folder_colour(name: &str) -> u32 {
    const COLOURS: [u32; 5] = [0x8b5cf6, 0x3fb27f, 0xe0a32a, 0x5b8cf0, 0xe8743b];
    if name == threads::NO_FOLDER || name == "Global" {
        return 0x8a8a93;
    }
    let h = name.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    COLOURS[h as usize % COLOURS.len()]
}

#[cfg(test)]
mod tests {
    use super::{needing, place};
    use crate::threads;

    /// The pill counts the streams that need you (an unopened reply), nothing else.
    #[test]
    fn the_pill_counts_streams_that_need_you() {
        let now = 1_700_000_000_000;
        let list = threads::fixture(now);
        let expect = list.iter().filter(|t| t.unread && !t.busy && t.failed.is_none()).count();
        assert_eq!(needing(&list), expect);
        assert!(expect > 0, "the fixture has some");
        let none: Vec<_> = list.into_iter().map(|mut t| {
            t.unread = false;
            t
        }).collect();
        assert_eq!(needing(&none), 0);
    }

    #[test]
    fn notes_group_by_place() {
        assert_eq!(place(&None), "Global");
        assert_eq!(place(&Some("/Users/x/code/workbench/".into())), "workbench");
        assert_eq!(place(&Some("agent:abc".into())), "Agents");
    }
}
