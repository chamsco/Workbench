//! The three apps (Charles, 13 Nov 13:40Z): Streams, Agents and Memory, switched at the top of the
//! sidebar; each replaces the main content area (not tabs). Streams is the built one. Agents and
//! Memory are mockups for now, so the switch leads somewhere: the coordinator's pages board will
//! design them. A mockup says it is one and shows no sample data as if it were real; Agents lists
//! core's agents by name, read only.

use super::*;

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
    fn icon(self) -> &'static str {
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
            self.page_agents = backspace_core::agents::Agents::open(Prefs::data_dir().join("agents")).list().into_iter().map(|a| (a.name, a.job)).collect();
        }
    }

    /// The sidebar under the switch while Agents or Memory is open.
    pub(super) fn page_nav(&self, pal: &'static Palette) -> Stateful<Div> {
        div()
            .id("page-nav")
            .flex_1()
            .min_h_0()
            .px(px(8.))
            .pt(px(8.))
            .text_size(px(12.))
            .text_color(c(pal.fg3))
            .child(format!("{} has its own list here once the page is designed.", self.page.label()))
    }

    /// Agents' or Memory's page in the card: a labelled mockup of what it will hold.
    pub(super) fn page_body(&self, pal: &'static Palette) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let line = c(pal.line);
        let (lead, sections): (&str, [(&str, &str); 3]) = match self.page {
            AppPage::Agents => (
                "Your agents: who they are, what they run on, what they may touch, and their routines.",
                [("Agents", "Each with its job, model and fallbacks."), ("Routines", "What each runs on a schedule, and when it last ran."), ("Rules", "What an agent may not do, and the apps it may not use.")],
            ),
            _ => (
                "What Cauldron remembers across streams, and why.",
                [("Notes", "What was kept, from which stream and project."), ("Proposed", "Notes waiting for the Librarian at Commit."), ("Receipts", "What each Commit kept and dropped, with the reason.")],
            ),
        };
        let mut col = div().w_full().max_w(px(660.)).flex().flex_col().px(px(24.)).py(px(8.));
        col = col
            .child(
                div()
                    .mt(px(12.))
                    .mb(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().text_size(px(20.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(self.page.label()))
                    .child(div().id("mockup-tag").px(px(6.)).h(px(18.)).flex().items_center().rounded(px(radius::TAG)).bg(c(pal.accent_soft)).text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(c(pal.accent)).aria_label("Mockup").child("Mockup").test_support()),
            )
            .child(div().text_size(px(13.)).text_color(fg3).mb(px(6.)).child(lead));
        for (i, (title, help)) in sections.into_iter().enumerate() {
            let mut sec = div().id(("mock-section", i)).mt(px(20.)).flex().flex_col().gap(px(6.)).aria_label(title);
            sec = sec.child(div().text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child(title)).child(div().text_size(px(12.)).text_color(fg2).child(help));
            if self.page == AppPage::Agents && i == 0 && !self.page_agents.is_empty() {
                for (j, (name, job)) in self.page_agents.iter().enumerate() {
                    sec = sec.child(
                        div()
                            .id(("agent-row", j))
                            .flex()
                            .gap(px(12.))
                            .py(px(6.))
                            .border_t_1()
                            .border_color(line)
                            .aria_label(name.clone())
                            .child(div().text_size(px(13.)).text_color(fg).child(name.clone()))
                            .child(div().min_w_0().truncate().text_size(px(12.)).text_color(fg3).child(job.clone()))
                            .test_support(),
                    );
                }
            } else {
                // The shape of the section, no content: grey bars, so nothing reads as real data.
                for w in [0.9f32, 0.7, 0.8] {
                    sec = sec.child(div().h(px(28.)).border_t_1().border_color(line).flex().items_center().child(div().h(px(8.)).w(relative(w * 0.6)).rounded(px(4.)).bg(c(pal.line_strong))));
                }
            }
            col = col.child(sec.test_support());
        }
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
            assert!(window.try_find("page-body").is_some() && window.try_find("mockup-tag").is_some(), "Memory's mockup, labelled");
            assert!(window.try_find(("thread", 0usize)).is_none() && window.try_find("drawer").is_none() && window.try_find("rrail").is_none(), "no streams, no drawer");
            window.click(("app-page", 0usize), cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("page-body").is_none() && window.try_find(("thread", 0usize)).is_some(), "Streams again");
        })
        .unwrap();
    }
}
