//! The destinations (the coordinator's board ac5ef7a, Charles 13 Nov 16:05Z): six buttons under
//! the sidebar's top row (Streams, Folders, Automations, Agents, Usage, Library), each replacing
//! the card; the sidebar's streams stay. Streams is built. Agents is a demo of the screens to come,
//! on the approved Cauldron figure. The other four are placeholders.
//!
//! The demo rule (UI-PLAN decision 5, Charles repeated it): nothing demo may look like working
//! behaviour. The page header carries "Demo"; example agents, messages and numbers carry "Example";
//! controls that do nothing look disabled and say "Demo: not built yet"; the composer is disabled and
//! says "Not connected". Real core data (core's agents) shows unmarked.

use super::*;
use cauldron_chrome::avatar::{self, Look, Mood};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AppPage {
    #[default]
    Streams,
    Folders,
    Automations,
    Agents,
    Usage,
    Library,
}

impl AppPage {
    pub(crate) const ALL: [AppPage; 6] = [AppPage::Streams, AppPage::Folders, AppPage::Automations, AppPage::Agents, AppPage::Usage, AppPage::Library];
    pub(crate) fn label(self) -> &'static str {
        match self {
            AppPage::Streams => "Streams",
            AppPage::Folders => "Folders",
            AppPage::Automations => "Automations",
            AppPage::Agents => "Agents",
            AppPage::Usage => "Usage",
            AppPage::Library => "Library",
        }
    }
    pub(super) fn icon(self) -> &'static str {
        match self {
            AppPage::Streams => "message-square",
            AppPage::Folders => "folder",
            AppPage::Automations => "calendar",
            AppPage::Agents => "bot",
            AppPage::Usage => "gauge",
            AppPage::Library => "database",
        }
    }
    /// Whether the right drawer belongs to this page: Streams' plugins, or Agents' own panel.
    pub(super) fn has_drawer(self) -> bool {
        matches!(self, AppPage::Streams | AppPage::Agents)
    }
}

/// One agent in the roster: a real one from core (unmarked) or an example (marked).
#[derive(Clone, Debug)]
pub(crate) struct AgentCard {
    pub name: String,
    pub job: String,
    pub route: String,
    /// Index into the agent colours.
    pub dot: usize,
    pub mood: Mood,
    pub status: String,
    pub example: bool,
}

/// The demo's example agents. The Librarian is one: amber, reading, filing what it keeps.
fn example_agents() -> Vec<AgentCard> {
    let a = |name: &str, job: &str, route: &str, dot: usize, mood: Mood, status: &str| AgentCard { name: name.into(), job: job.into(), route: route.into(), dot, mood, status: status.into(), example: true };
    vec![
        a("Claude Code", "Leads: plans and checks", "claude · opus", 0, Mood::Asking, "Needs you"),
        a("Codex", "Builds each ticket", "codex", 2, Mood::Working, "Working"),
        a("Cursor", "Previews and copy", "cursor", 6, Mood::Watching, "Watching"),
        a("Librarian", "Files what is worth keeping", "claude · haiku", 3, Mood::Reading, "Filing 2 notes"),
    ]
}

/// The demo's group rooms: name, members (roster indexes), last line.
pub(super) const ROOMS: [(&str, [usize; 3], &str); 2] = [("Launch crew", [0, 1, 2], "Cursor: preview is up"), ("Design room", [0, 2, 3], "Librarian filed the palette")];

/// The demo room's example messages: who (roster index; None is you), what.
const MESSAGES: [(Option<usize>, &str); 6] = [
    (None, "Launch crew: preview the receipt flow and tell me what breaks."),
    (Some(0), "I'll lead. Codex, take the total and the boundary test; Cursor, run the preview and check the copy."),
    (Some(1), "Total is in and the tests are green. I'll post the diff when Cursor's preview settles."),
    (Some(2), "Preview is up. One label still says \u{201c}subtotal\u{201d} on the receipt; I'm fixing that now."),
    (Some(3), "Filing \u{201c}Receipt totals round half up\u{201d} to the Friday demo folder, from this room."),
    (Some(0), "Noted. I'll read the diff once Cursor is done."),
];

pub(super) fn figure(dot: usize, mood: Mood, px_: f32, bust: bool) -> Img {
    let (fw, fh) = avatar::size(px_);
    img(Arc::new(Image::from_bytes(ImageFormat::Svg, avatar::svg(dot, mood, Look { px: px_, outline: false, ms: None, bust, ink: None }).into_bytes()))).w(px(fw)).h(px(fh)).flex_none()
}

/// A small marker: "Demo" in accent, "Example" in grey.
pub(super) fn tag(id: impl Into<ElementId>, text: &'static str, pal: &'static Palette) -> Stateful<Div> {
    let demo = text == "Demo";
    div()
        .id(id)
        .flex_none()
        .px(px(6.))
        .h(px(18.))
        .flex()
        .items_center()
        .rounded(px(radius::TAG))
        .border_1()
        .border_color(if demo { c(pal.accent) } else { c(pal.line_strong) })
        .bg(if demo { c(pal.accent_soft) } else { c(pal.inset) })
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(if demo { c(pal.accent) } else { c(pal.fg3) })
        .aria_label(text)
        .child(text)
}

impl Cauldron {
    /// The six buttons under the sidebar's top row, the open one selected.
    pub(super) fn page_switch(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut row = div().id("page-switch").flex().justify_between().pt(px(2.)).pb(px(8.)).mb(px(8.)).border_b_1().border_color(c(pal.line));
        for (i, p) in AppPage::ALL.into_iter().enumerate() {
            let on = self.page == p && self.settings.is_none();
            row = row.child(
                self.tipped(("app-page", i), icon_button(("app-page", i), p.icon(), p.label(), pal, on), p.label(), Side::Below, pal, cx)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_page(p);
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        row
    }

    pub(super) fn open_page(&mut self, p: AppPage) {
        self.page = p;
        self.settings = None;
        self.side_data = super::context::SideData::load(p, &self.data_dir());
        if p == AppPage::Usage {
            // Read the logs again on each visit (the render starts it).
            self.usage = None;
            self.usage_task = None;
        }
        if p == AppPage::Agents {
            // Agents keeps its own drawer open: the agent's panel is part of the page.
            self.drawer.get_or_insert(super::Panel::Highlights);
            // Core's agents, read only; they show unmarked, the examples after them.
            self.page_agents = backspace_core::agents::Agents::open(Prefs::data_dir().join("agents"))
                .list()
                .into_iter()
                .enumerate()
                .map(|(i, a)| AgentCard {
                    route: [Some(a.route.provider.clone()), a.route.model.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · "),
                    name: a.name,
                    job: a.job,
                    dot: i,
                    mood: Mood::Idle,
                    status: "Idle".into(),
                    example: false,
                })
                .collect();
        }
    }

    /// The roster: core's agents, then the examples.
    pub(super) fn roster(&self) -> Vec<AgentCard> {
        let mut v = self.page_agents.clone();
        v.extend(example_agents());
        v
    }

    /// A destination's page in the card.
    pub(super) fn page_body(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        match self.page {
            AppPage::Agents => self.agents_page(pal, cx),
            AppPage::Folders => self.folders_page(pal, cx),
            AppPage::Library => self.library_page(pal, cx),
            AppPage::Usage => self.usage_page(pal),
            p => self.placeholder_page(p, pal),
        }
    }

    /// Folders, Automations, Usage and Library: what each will be, marked Demo, nothing working.
    fn placeholder_page(&self, p: AppPage, pal: &'static Palette) -> AnyElement {
        let lead = match p {
            AppPage::Folders => "Your folders and the streams in them, with what Cauldron proposes to file where. Next after Agents.",
            AppPage::Automations => "What runs on its own: agents' routines and their schedules, and what they did last.",
            AppPage::Usage => "What each subscription has left this week and in the last five hours, and what each stream spent.",
            _ => "What Cauldron remembers across streams, how notes link, and what the Librarian proposes to keep. After Folders.",
        };
        let col = div()
            .w_full()
            .max_w(px(660.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(24.))
            .py(px(20.))
            .child(div().flex().items_center().gap(px(8.)).child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(pal.fg)).child(p.label())).child(tag("demo-tag", "Demo", pal).test_support()))
            .child(div().text_size(px(13.)).text_color(c(pal.fg3)).child(lead))
            .child(div().mt(px(12.)).text_size(px(12.)).text_color(c(pal.fg3)).child("Not built yet: this page arrives in the order Agents, Folders, Library."));
        div().id("page-body").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element()
    }

    /// Agents: the open room (the old Crew screens, on the approved figure: flat colour, moods, a
    /// bust in the room). The roster and the crews are in the sidebar now (the context-sidebar board).
    fn agents_page(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let roster = self.roster();
        let real = roster.iter().filter(|a| !a.example).count();
        // The open room (or an agent's 1:1): header, the day, the messages, the disabled composer.
        let (title, sub, faces): (String, String, Vec<usize>) = match self.agent_sel {
            Some(i) if i < roster.len() => (roster[i].name.clone(), format!("{} · {}", roster[i].status, roster[i].route), vec![i]),
            _ => {
                let room = &ROOMS[self.room_sel.min(ROOMS.len() - 1)];
                (room.0.into(), format!("{} agents · {}", room.1.len(), room.2), room.1.iter().map(|m| real + m).collect())
            }
        };
        let mut head_faces = div().flex();
        for i in &faces {
            head_faces = head_faces.child(div().ml(px(-6.)).child(figure(roster[*i].dot, roster[*i].mood, 30., false)));
        }
        let mut msgs = div().id("room-messages").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap(px(14.)).px(px(4.));
        msgs = msgs.child(div().flex().justify_center().child(div().px(px(8.)).h(px(20.)).flex().items_center().rounded(px(radius::CONTROL)).bg(c(pal.inset)).text_size(px(11.)).text_color(fg3).child("Today · example conversation")));
        let shown: Vec<(Option<usize>, &str)> = match self.agent_sel {
            Some(i) if i >= real => MESSAGES.iter().copied().filter(|(w, _)| w.map_or(true, |w| real + w == i)).collect(),
            Some(_) => Vec::new(),
            // The second room shows its members' lines from the same example conversation.
            None => MESSAGES.iter().copied().filter(|(w, _)| self.room_sel == 0 || w.map_or(true, |w| ROOMS[self.room_sel.min(ROOMS.len() - 1)].1.contains(&w))).collect(),
        };
        if shown.is_empty() {
            msgs = msgs.child(div().id("room-empty").text_size(px(12.)).text_color(fg3).child("No messages here yet: chatting with an agent is not built yet.").test_support());
        }
        for (k, (who, text)) in shown.into_iter().enumerate() {
            msgs = msgs.child(match who {
                None => div()
                    .id(("room-msg", k))
                    .self_end()
                    .max_w(relative(0.75))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(radius::PANEL))
                    .bg(c(pal.raised))
                    .text_size(px(13.))
                    .text_color(fg)
                    .aria_label(text)
                    .child(text)
                    .test_support(),
                Some(w) => {
                    let a = &roster[real + w];
                    div()
                        .id(("room-msg", k))
                        .flex()
                        .gap(px(10.))
                        .aria_label(text)
                        .child(figure(a.dot, a.mood, 26., true))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().flex().items_center().gap(px(6.)).child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(fg).child(a.name.clone())).child(tag(("msg-example", k), "Example", pal).test_support()))
                                .child(div().text_size(px(13.)).text_color(fg2).child(text)),
                        )
                        .test_support()
                }
            });
        }
        let composer = self.tipped(
            "agent-composer",
            div()
                .id("agent-composer")
                .flex_none()
                .h(px(56.))
                .px(px(14.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(radius::PANEL))
                .border_1()
                .border_color(c(pal.line))
                .bg(c(pal.inset))
                .text_size(px(13.))
                .text_color(c(pal.fg4))
                .aria_label("Not connected")
                .child(div().flex_1().min_w_0().truncate().child(format!("Not connected: messages to {title} are not sent yet")))
                .child(div().flex_none().size(px(28.)).rounded_full().flex().items_center().justify_center().bg(c(pal.hover)).child(glyph("arrow-up", 14., c(pal.fg4)))),
            "Demo: not built yet",
            Side::Right,
            pal,
            cx,
        );
        let room = div()
            .id("agent-room")
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .pl(px(6.))
                    .child(head_faces)
                    .child(div().flex_1().min_w_0().flex().flex_col().child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(title.clone())).child(div().truncate().text_size(px(12.)).text_color(fg3).child(sub)))
                    .child(self.tipped("room-info", div().id("room-info").h(px(26.)).px(px(10.)).flex().items_center().rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line)).text_size(px(12.)).text_color(c(pal.fg4)).aria_label("Room info (demo: not built yet)").child("Room info"), "Demo: not built yet", Side::Below, pal, cx).test_support()),
            )
            .child(msgs.test_support())
            .child(composer.test_support());
        div()
            .id("page-body")
            .size_full()
            .flex()
            .flex_col()
            .px(px(16.))
            .pb(px(12.))
            .child(div().flex().items_center().gap(px(8.)).py(px(8.)).child(div().text_size(px(13.)).text_color(fg3).child("Agents work as a team in rooms; this is how it will look.")).child(tag("demo-tag", "Demo", pal).test_support()))
            .child(div().flex_1().min_h_0().flex().child(room.test_support()))
            .test_support()
            .into_any_element()
    }

    /// Agents' own drawer: the selected agent (or the room's lead) with its rings, current task,
    /// Context, Traces, Routines and Settings. All example content, marked.
    pub(super) fn agents_drawer(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let roster = self.roster();
        let a = self.agent_sel.and_then(|i| roster.get(i).cloned()).unwrap_or_else(|| example_agents()[0].clone());
        let head = |t: &'static str| div().pt(px(6.)).text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child(t);
        // A ring as a circle: its edge in accent past 80% (warn), its share inside, its caption
        // under it so two fit the drawer's 200.
        let ring = |id: &'static str, pct: u32, label: &'static str, sub: &'static str| {
            div()
                .id(id)
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.))
                .flex_1()
                .min_w_0()
                .aria_label(format!("{label} {pct}% (example)"))
                .child(div().size(px(36.)).flex_none().rounded_full().border_2().border_color(if pct >= 80 { c(pal.warn) } else { c(pal.line_strong) }).flex().items_center().justify_center().text_size(px(10.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(format!("{pct}%")))
                .child(div().max_w_full().flex().flex_col().items_center().child(div().max_w_full().truncate().text_size(px(11.)).text_color(fg2).child(label)).child(div().max_w_full().truncate().text_size(px(10.)).text_color(fg3).child(sub)))
                .test_support()
        };
        let mut tree = div().id("context-treemap").h(px(64.)).flex().gap(px(2.));
        for (k, (name, share, dot)) in [("Files", 0.45f32, 0usize), ("Chat", 0.25, 1), ("Tools", 0.2, 2), ("Memory", 0.1, 3)].into_iter().enumerate() {
            let mut col: Hsla = c(tokens::AGENT_DOTS[dot]);
            col.a = 0.35;
            tree = tree.child(div().id(("ctx", k)).h_full().w(relative(share)).rounded(px(4.)).bg(col).p(px(4.)).overflow_hidden().text_size(px(10.)).text_color(fg).aria_label(name).child(div().truncate().child(name)).test_support());
        }
        let line = |t: &'static str| div().flex().gap(px(6.)).child(div().mt(px(6.)).size(px(4.)).flex_none().rounded_full().bg(c(pal.fg4))).child(div().flex_1().min_w_0().text_size(px(12.)).text_color(fg2).child(t));
        let setting = |t: &'static str| div().id(t).h(px(28.)).flex().items_center().justify_between().text_size(px(12.)).text_color(c(pal.fg4)).aria_label(format!("{t} (demo: not built yet)")).child(t).child(glyph("chevron-right", 12., c(pal.fg4)));
        // The board's tabs (destinations board, Agents): tight padding, Settings as a cog with its
        // tip, so the row fits the 188 inside the drawer's 200.
        let mut tabs = div().id("agent-tabs").flex().py(px(4.));
        for (i, t) in ["Context", "Traces", "Routines", "Settings"].into_iter().enumerate() {
            let on = self.agent_tab == i;
            let el = div()
                .id(("agent-tab", i))
                .h(px(24.))
                .px(px(5.))
                .when_live(i == 3, |d| d.flex_none())
                .when_live(i != 3, |d| d.flex_shrink_1().min_w_0())
                .flex()
                .items_center()
                .rounded(px(8.))
                .text_size(px(12.))
                .cursor_pointer()
                .text_color(if on { fg } else { fg3 })
                .when_live(on, |d| d.bg(c(pal.selected)))
                .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                .aria_label(t)
                .aria_selected(on)
                .when_live(i == 3, |d| d.child(glyph("settings", 14., if on { fg } else { fg3 })))
                .when_live(i != 3, |d| d.child(div().truncate().child(t)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.agent_tab = i;
                    cx.notify();
                }));
            tabs = tabs.child(if i == 3 { self.tipped(("agent-tab-tip", i), el, "Settings", Side::Below, pal, cx).test_support() } else { el.test_support() });
        }
        let body = match self.agent_tab {
            0 => div().flex().flex_col().gap(px(6.)).child(tree.test_support()).child(div().text_size(px(11.)).text_color(fg3).child("34k of 200k tokens")),
            1 => div().flex().flex_col().gap(px(6.)).child(line("Split the work into three tasks")).child(line("Posted tasks to the shared board")).child(line("Waiting on Codex and Cursor")),
            2 => div().flex().flex_col().gap(px(6.)).child(line("Nightly: check the open streams")),
            _ => div().flex().flex_col().child(setting("Model and fallbacks")).child(setting("Computer")).child(setting("Switched off")),
        };
        div()
            .id("agents-drawer")
            .absolute()
            .left(px(6.))
            .right(px(6.))
            .top(px(8.))
            .bottom(px(8.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().flex().items_center().gap(px(8.)).child(figure(a.dot, a.mood, 30., false)).child(div().flex_1().min_w_0().flex().flex_col().child(div().truncate().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(a.name.clone())).child(div().truncate().text_size(px(11.)).text_color(fg3).child(a.job.clone()))).child(tag("drawer-example", "Example", pal).test_support()))
            .child(div().flex().gap(px(10.)).child(ring("ring-week", 91, "Weekly", "Resets Mon 09:00")).child(ring("ring-5h", 12, "5-hour", "12% used")))
            .child(head("Current task"))
            .child(div().p(px(8.)).rounded(px(radius::INNER)).bg(c(pal.inset)).text_size(px(12.)).text_color(fg2).child(if a.name == "Librarian" { "Reading the room and filing two notes to the Friday demo folder." } else { "Reading the receipt flow and splitting the work." }))
            .child(tabs)
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::super::thread::tests::open;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};

    /// Six buttons replace the card; the sidebar's streams stay; a stream brings Streams back.
    #[gpui_kit::test]
    fn the_buttons_replace_the_main_content(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        let rest = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| c.motion = Default::default()).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let names: Vec<String> = (0..6usize).map(|i| window.find(("app-page", i)).label().unwrap().to_string()).collect();
            assert_eq!(names, ["Streams", "Folders", "Automations", "Agents", "Usage", "Library"]);
            assert_eq!(window.find(("app-page", 0usize)).selected(), Some(true), "Streams first");
            window.click(("app-page", 5usize), cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("card-title").label(), Some("Library"));
            assert!(window.try_find("page-body").is_some(), "Library's page fills the card");
            // The sidebar follows the destination (the context-sidebar board): Library's own list.
            assert!(window.try_find(("thread", 0usize)).is_none() && window.try_find("side-zone").is_some(), "the sidebar shows Library's list");
            assert_eq!(window.find(("lib-notes", 0usize)).label(), Some("Notes"));
            assert!(window.try_find("drawer").is_none() && window.try_find("rrail").is_none(), "Library has no drawer");
            window.click(("lib-notes", 0usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find(("lib-notes", 0usize)).selected(), Some(true), "selecting a row highlights it");
            window.click(("app-page", 0usize), cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("page-body").is_none() && window.find(("app-page", 0usize)).selected() == Some(true), "Streams comes back");
            assert!(window.try_find(("thread", 0usize)).is_some(), "with its streams");
        })
        .unwrap();
    }

    /// The Agents demo marks everything it shows: Demo in the header, Example on every example
    /// agent, room and message, a disabled "Not connected" composer, and Agents' own drawer with
    /// the agent's rings and context, marked Example. The Librarian is in the roster, reading.
    #[gpui_kit::test]
    fn the_agents_demo_is_marked_as_a_demo(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        handle
            .update(cx, |c, _, _| {
                c.open_page(super::AppPage::Agents);
                c.motion = Default::default();
            })
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("demo-tag").is_some(), "Demo in the header");
            let names: Vec<String> = (0..4usize).map(|i| window.find(("agent-row", i)).label().unwrap().to_string()).collect();
            assert_eq!(names, ["Claude Code", "Codex", "Cursor", "Librarian"]);
            for i in 0..4usize {
                assert!(window.try_find(("agent-example", i)).is_some(), "example agent {i} is marked");
            }
            for k in 1..6usize {
                assert!(window.try_find(("msg-example", k)).is_some(), "message {k} is marked");
            }
            assert_eq!(window.find("agent-composer").label(), Some("Not connected"));
            assert!(window.try_find("agents-drawer").is_some() && window.try_find("drawer-example").is_some(), "Agents' own drawer, marked");
            // The drawer's tabs fit its 200 (Charles, 13 Nov 18:49Z), Settings as a cog with its name.
            let (d, last) = (super::super::tests::rect(window, "agents-drawer"), super::super::tests::rect(window, ("agent-tab", 3usize)));
            assert!(last.right() <= d.right(), "the tab row ends inside the drawer: {last:?} in {d:?}");
            assert_eq!(window.find(("agent-tab", 3usize)).label(), Some("Settings"));
            assert!(window.try_find("context-treemap").is_some(), "Context first");
            window.click(("agent-tab", 1usize), cx);
            window.render_frame(cx);
            assert!(window.try_find("context-treemap").is_none() && window.find(("agent-tab", 1usize)).selected() == Some(true), "Traces replaces it");
            window.click(("agent-row", 3usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find(("agent-row", 3usize)).selected(), Some(true));
            assert!(window.try_find(("room-msg", 1usize)).is_some(), "the Librarian's own lines");
        })
        .unwrap();
    }
}
