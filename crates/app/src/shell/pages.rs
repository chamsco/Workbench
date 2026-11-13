//! The three apps (Charles, 13 Nov 13:40Z): Streams, Agents and Memory, switched at the top of the
//! sidebar; each replaces the main content area (not tabs), and the sidebar's streams stay. Streams is the built one. Agents and
//! Memory are mockups for now, so the switch leads somewhere: the coordinator's pages board will
//! design them. A mockup says it is one and shows no sample data as if it were real; Agents lists
//! core's agents by name, read only.

use super::*;
use cauldron_chrome::avatar::{self, Look, Mood};

/// What the Agents mockup shows of one agent.
#[derive(Clone, Debug, Default)]
pub(crate) struct AgentCard {
    pub name: String,
    pub job: String,
    pub route: String,
    pub computer: String,
    pub memory: bool,
    pub off: Vec<String>,
    pub routines: Vec<String>,
}

/// Example agents for the mockup when core has none, marked as example content on the page.
fn example_agents() -> Vec<AgentCard> {
    let a = |name: &str, job: &str, route: &str, routines: &[&str]| AgentCard {
        name: name.into(),
        job: job.into(),
        route: route.into(),
        computer: "folder".into(),
        memory: true,
        off: vec!["push to main".into()],
        routines: routines.iter().map(|r| r.to_string()).collect(),
    };
    vec![a("Rampart", "Tests every change red first", "claude · opus", &["Nightly mutation run"]), a("Builder", "Writes the code for a ticket", "codex", &[]), a("Reviewer", "Reads each patch before it lands", "claude · sonnet", &["Weekly review digest"])]
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AppPage {
    #[default]
    Streams,
    Agents,
    Memory,
}

impl AppPage {
    pub(crate) const ALL: [AppPage; 3] = [AppPage::Streams, AppPage::Agents, AppPage::Memory];
    pub(crate) fn label(self) -> &'static str {
        match self {
            AppPage::Streams => "Streams",
            AppPage::Agents => "Agents",
            AppPage::Memory => "Memory",
        }
    }
    pub(super) fn icon(self) -> &'static str {
        match self {
            AppPage::Streams => "message-square",
            AppPage::Agents => "bot",
            AppPage::Memory => "database",
        }
    }
}

impl Cauldron {
    /// The switch at the top of the sidebar: three equal buttons, the open app selected.
    pub(super) fn page_switch(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut row = div().id("page-switch").flex().gap(px(2.)).p(px(2.)).mb(px(8.)).rounded(px(radius::CONTROL)).bg(c(pal.inset));
        for (i, p) in AppPage::ALL.into_iter().enumerate() {
            let on = self.page == p && self.settings.is_none();
            row = row.child(
                div()
                    .id(("app-page", i))
                    .flex_1()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .rounded(px(radius::INNER))
                    .cursor_pointer()
                    .when_live(on, |d| d.bg(c(pal.selected)))
                    .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if on { c(pal.fg) } else { c(pal.fg3) })
                    .aria_label(p.label())
                    .aria_selected(on)
                    .child(glyph(p.icon(), 14., if on { c(pal.fg) } else { c(pal.fg3) }))
                    .child(p.label())
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
        if p == AppPage::Agents {
            // Core's agents, by name and job; read only.
            self.page_agents = backspace_core::agents::Agents::open(Prefs::data_dir().join("agents"))
                .list()
                .into_iter()
                .map(|a| AgentCard {
                    route: [Some(a.route.provider.clone()), a.route.model.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · "),
                    computer: a.computer.kind.clone(),
                    memory: a.memory,
                    off: a.off.clone(),
                    routines: a.routines.iter().map(|r| r.name.clone()).collect(),
                    name: a.name,
                    job: a.job,
                })
                .collect();
        }
    }

    /// Agents' own drawer (the pages board): Context, Traces, Routines and Settings. A mockup.
    pub(super) fn agents_drawer(&self, pal: &'static Palette) -> Stateful<Div> {
        let mut d = div().id("agents-drawer").absolute().left(px(10.)).right(px(10.)).top(px(8.)).flex().flex_col().gap(px(10.));
        d = d.child(div().flex().items_center().gap(px(6.)).child(div().font_weight(FontWeight::SEMIBOLD).child("Agent")).child(div().px(px(5.)).rounded(px(radius::TAG)).bg(c(pal.accent_soft)).text_size(px(10.)).text_color(c(pal.accent)).child("Mockup")));
        for (title, help) in [("Context", "What the agent knows about this work."), ("Traces", "Its recent runs, step by step."), ("Routines", "What it runs on a schedule."), ("Settings", "Its model, computer and rules.")] {
            d = d.child(div().flex().flex_col().gap(px(2.)).child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(c(pal.fg2)).child(title)).child(div().text_size(px(11.)).text_color(c(pal.fg3)).child(help)));
        }
        d
    }

    /// Agents' or Memory's page in the card: a labelled mockup of what it will hold.
    pub(super) fn page_body(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        match self.page {
            AppPage::Agents => self.agents_page(pal, cx),
            _ => self.memory_page(pal, cx),
        }
    }

    fn page_head(&self, pal: &'static Palette, lead: &str, example: bool) -> Div {
        let tag = |id: &'static str, t: &'static str, col: Hsla, bg: Hsla| div().id(id).px(px(6.)).h(px(18.)).flex().items_center().rounded(px(radius::TAG)).bg(bg).text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(col).aria_label(t).child(t).test_support();
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .mt(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().text_size(px(20.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).text_color(c(pal.fg)).child(self.page.label()))
                    .child(tag("mockup-tag", "Mockup", c(pal.accent), c(pal.accent_soft)))
                    .when_live(example, |d| d.child(tag("example-tag", "Example content", c(pal.fg3), c(pal.inset)))),
            )
            .child(div().text_size(px(13.)).text_color(c(pal.fg3)).child(lead.to_string()))
    }

    /// Agents: the roster on the left with each agent's figure (figures belong here), the first
    /// one's detail on the right. Core's agents when there are any, else marked examples.
    fn agents_page(&self, pal: &'static Palette, _cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let line = c(pal.line);
        let example = self.page_agents.is_empty();
        let agents = if example { example_agents() } else { self.page_agents.clone() };
        let figure = |i: usize, px_: f32| {
            let (fw, fh) = avatar::size(px_);
            img(Arc::new(Image::from_bytes(ImageFormat::Svg, avatar::svg(i, Mood::Idle, Look { px: px_, outline: false, ms: None, bust: false, ink: None }).into_bytes()))).w(px(fw)).h(px(fh)).flex_none()
        };
        let mut roster = div().id("agent-roster").w(px(200.)).flex_none().flex().flex_col().gap(px(2.));
        for (i, a) in agents.iter().enumerate() {
            roster = roster.child(
                div()
                    .id(("agent-row", i))
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(8.))
                    .rounded(px(radius::INNER))
                    .when_live(i == 0, |d| d.bg(c(pal.selected)))
                    .aria_label(a.name.clone())
                    .child(figure(i, 22.))
                    .child(div().min_w_0().flex().flex_col().child(div().truncate().text_size(px(13.)).text_color(fg).child(a.name.clone())).child(div().truncate().text_size(px(11.)).text_color(fg3).child(a.job.clone())))
                    .test_support(),
            );
        }
        let a = agents.first().cloned().unwrap_or_default();
        let field = |k: &str, v: String| div().flex().gap(px(12.)).py(px(8.)).border_t_1().border_color(line).child(div().w(px(110.)).flex_none().text_size(px(12.)).text_color(fg3).child(k.to_string())).child(div().flex_1().min_w_0().text_size(px(13.)).text_color(fg2).child(v));
        let detail = div()
            .id("agent-detail")
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(div().flex().items_center().gap(px(10.)).pb(px(10.)).child(figure(0, 40.)).child(div().flex().flex_col().child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(a.name.clone())).child(div().text_size(px(12.)).text_color(fg3).child(a.job.clone()))))
            .child(field("Route", a.route.clone()))
            .child(field("Computer", a.computer.clone()))
            .child(field("Memory", if a.memory { "On".into() } else { "Off".into() }))
            .child(field("Switched off", if a.off.is_empty() { "Nothing".into() } else { a.off.join(", ") }))
            .child(field("Routines", if a.routines.is_empty() { "None".into() } else { a.routines.join(", ") }))
            .test_support();
        let col = div()
            .w_full()
            .max_w(px(820.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .px(px(24.))
            .py(px(8.))
            .child(self.page_head(pal, "Your agents: who they are, what they run on, what they may touch, and their routines.", example))
            .child(div().flex().gap(px(20.)).child(roster.test_support()).child(detail));
        div().id("page-body").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element()
    }

    /// Memory: Proposals, Notes, Dreams and Skills; scope chips; proposals as cards to accept or
    /// reject, the global ones needing your approval. All example content for now, marked so.
    fn memory_page(&self, pal: &'static Palette, _cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mut tabs = div().id("memory-tabs").flex().gap(px(4.));
        for (i, t) in ["Proposals", "Notes", "Dreams", "Skills"].into_iter().enumerate() {
            tabs = tabs.child(div().id(("memory-tab", i)).h(px(24.)).px(px(8.)).flex().items_center().rounded(px(radius::INNER)).when_live(i == 0, |d| d.bg(c(pal.selected))).text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(if i == 0 { fg } else { fg3 }).aria_label(t).aria_selected(i == 0).child(t).test_support());
        }
        let mut scopes = div().flex().gap(px(6.));
        for (i, sc) in ["All", "This project", "Global"].into_iter().enumerate() {
            scopes = scopes.child(div().id(("memory-scope", i)).h(px(22.)).px(px(8.)).flex().items_center().rounded(px(radius::CONTROL)).border_1().border_color(if i == 0 { c(pal.line_strong) } else { c(pal.line) }).text_size(px(11.)).text_color(if i == 0 { fg2 } else { fg3 }).child(sc).test_support());
        }
        let btn = |id: ElementId, t: &'static str, pri: bool| {
            div().id(id).h(px(26.)).px(px(10.)).flex().items_center().rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line_strong)).when_live(pri, |d| d.bg(c(pal.accent)).border_color(c(pal.accent))).text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(if pri { c(pal.on_accent) } else { fg }).aria_label(t).child(t).test_support()
        };
        let mut cards = div().flex().flex_col().gap(px(10.));
        for (i, (text, scope, from, global)) in [
            ("Run the suite with --release on the Mac runner; debug builds time out the UI tests.", "Cauldron", "from Get M3 through review", false),
            ("Charles prefers plain words in reports and no em dashes.", "Global", "from three streams", true),
            ("The Windows build signs with the team certificate, not the personal one.", "Cauldron", "from Ship stage notes", false),
        ]
        .into_iter()
        .enumerate()
        {
            cards = cards.child(
                div()
                    .id(("proposal", i))
                    .p(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .rounded(px(radius::INSET))
                    .border_1()
                    .border_color(c(pal.line_strong))
                    .aria_label(text)
                    .child(div().flex().items_center().gap(px(6.)).child(div().text_size(px(11.)).text_color(fg3).child(format!("{scope} · {from}"))).when_live(global, |d| d.child(div().id(("needs-approval", i)).ml_auto().px(px(6.)).h(px(18.)).flex().items_center().rounded(px(radius::TAG)).bg(c(pal.accent_soft)).text_size(px(11.)).text_color(c(pal.accent)).child("Needs your approval").test_support())))
                    .child(div().text_size(px(13.)).text_color(fg2).child(text))
                    .child(div().flex().gap(px(8.)).justify_end().child(btn(("reject", i).into(), "Reject", false)).child(btn(("accept", i).into(), "Accept", true)))
                    .test_support(),
            );
        }
        let col = div()
            .w_full()
            .max_w(px(660.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .px(px(24.))
            .py(px(8.))
            .child(self.page_head(pal, "What Cauldron remembers across streams, and why. Proposals wait for you or the Librarian at Commit.", true))
            .child(tabs.test_support())
            .child(scopes)
            .child(cards);
        div().id("page-body").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::super::thread::tests::open;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};

    /// The switch replaces the main content: Agents and Memory open their mockup in the card (no
    /// drawer, no stream list), Streams brings the streams back.
    #[gpui_kit::test]
    fn the_switch_replaces_the_main_content(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        let rest = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| c.motion = Default::default()).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("app-page", 0usize)).selected(), Some(true), "Streams first");
            window.click(("app-page", 2usize), cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("card-title").label(), Some("Memory"));
            assert!(window.try_find("page-body").is_some() && window.try_find("mockup-tag").is_some() && window.try_find("example-tag").is_some(), "Memory's mockup, labelled, its content marked as examples");
            let tabs: Vec<_> = (0..4usize).map(|i| window.find(("memory-tab", i)).label().unwrap().to_string()).collect();
            assert_eq!(tabs, ["Proposals", "Notes", "Dreams", "Skills"]);
            assert!(window.try_find(("needs-approval", 1usize)).is_some() && window.try_find(("needs-approval", 0usize)).is_none(), "only the global proposal needs your approval");
            assert!(window.try_find(("thread", 0usize)).is_some(), "the sidebar's streams stay");
            assert!(window.try_find("drawer").is_none() && window.try_find("rrail").is_none(), "Memory has no drawer");
            window.click(("thread", 0usize), cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("page-body").is_none() && window.find(("app-page", 0usize)).selected() == Some(true), "a stream brings Streams back");
        })
        .unwrap();
    }
}
