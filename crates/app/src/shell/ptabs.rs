//! Tabs in the pane header (the coordinator's board 88454ee, Charles 13 Nov: "I'd like to have the
//! tabs in the header of the main content element, in each pane, instead of floating on top"). The
//! thread's tabs (the thread, then its agents' 1:1s) sit in the header of the pane that shows the
//! thread, and the active tab is the title, so the header does not say it twice. Under 320px a pane
//! keeps only the active tab and a "+N" that steps to the next. Settings > Appearance > Tabs puts the
//! floating strip back, to compare.

use super::*;

/// A pane narrower than this keeps only its active tab and "+N".
pub(super) const NARROW: f32 = 320.0;

impl Cauldron {
    /// The thread's tabs for a pane header `width` wide.
    pub(super) fn tab_strip(&self, pal: &'static Palette, width: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let mut names: Vec<SharedString> = vec![self.coordinator.clone().unwrap_or_else(|| NEW_TAB.into())];
        names.extend(self.agents.iter().map(|a| a.name.clone()));
        let tab = |i: usize, name: SharedString, on: bool| {
            div()
                .id(("ptab", i))
                .h(px(metric::TAB_H))
                .min_w_0()
                .flex()
                .items_center()
                .px(px(8.))
                .rounded(px(radius::INNER))
                .cursor_pointer()
                .when_live(on, |d| d.flex_shrink_0().max_w(px(220.)).bg(c(pal.selected)))
                .when_live(!on, |d| d.max_w(px(160.)).hover(|d| d.bg(c(pal.hover))))
                .text_size(px(12.))
                .line_height(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if on { fg } else { fg3 })
                .aria_label(name.clone())
                .aria_selected(on)
                .child(div().min_w_0().truncate().child(name))
        };
        let active = self.active.min(names.len() - 1);
        let mut strip = div().id("pane-tabs").flex_1().min_w_0().flex().items_center().gap(px(4.)).overflow_hidden();
        if width < NARROW && names.len() > 1 {
            strip = strip.child(tab(active, names[active].clone(), true).test_support());
            let more = names.len() - 1;
            strip = strip.child(
                div()
                    .id("ptab-more")
                    .h(px(metric::TAB_H))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px(px(6.))
                    .rounded(px(radius::INNER))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .text_size(px(11.))
                    .text_color(fg3)
                    .aria_label(format!("{more} more tabs"))
                    .child(format!("+{more}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.active = (this.active + 1) % (more + 1);
                        cx.notify();
                    }))
                    .test_support(),
            );
            return strip;
        }
        for (i, name) in names.into_iter().enumerate() {
            strip = strip.child(
                tab(i, name, i == active)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.active = i;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        strip
    }

    /// The window bar's breadcrumb when the tabs are in the panes: the project, then the thread.
    pub(super) fn bar_crumb(&self, pal: &'static Palette, r: Rect) -> Option<Stateful<Div>> {
        let t = self.thread.as_ref()?;
        let folder = t.project.as_ref().map(|p| p.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(p).to_string());
        let label = match &folder {
            Some(f) => format!("{f} / {}", t.title),
            None => t.title.clone(),
        };
        Some(
            pane("bar-crumb", r)
                .flex()
                .items_center()
                .gap(px(4.))
                .text_size(px(12.))
                .text_color(c(pal.fg3))
                .aria_label(label)
                .children(folder.map(|f| div().flex_none().child(format!("{f} /"))))
                .child(div().min_w_0().truncate().text_color(c(pal.fg2)).child(t.title.clone())),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::rect;
    use super::super::thread::tests::open;
    use cauldron_chrome::panes::{Line, Shape};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};

    /// With the tabs in the pane header, a thread's tabs sit in its card head, the active one is the
    /// title (no separate title), and the window bar keeps only the breadcrumb.
    #[gpui_kit::test]
    fn the_tabs_sit_in_the_pane_header(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        handle.update(cx, |c, _, _| c.tabs_floating = false).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
            window.render_frame(cx);
            assert!(window.try_find(("tab", 0usize)).is_none() && window.try_find("tabs").is_none(), "no floating strip");
            assert_eq!(window.find(("ptab", 0usize)).label(), Some("Get M3 through review"));
            assert_eq!(window.find(("ptab", 0usize)).selected(), Some(true));
            assert!(window.try_find("thread-title").is_none(), "the active tab is the title");
            let (head, t) = (rect(window, "card-head"), rect(window, ("ptab", 0usize)));
            assert!(t.y >= head.y && t.bottom() <= head.bottom(), "in the card head");
            assert_eq!(window.find("bar-crumb").label(), Some("Cauldron / Get M3 through review"), "the bar keeps the breadcrumb");
        })
        .unwrap();
    }

    /// Split, each pane has its own 40px header: the thread's pane carries the thread's tabs and
    /// the layout button, the others their own name; the card head is gone. Under 320px the thread's
    /// pane keeps the active tab and "+N".
    #[gpui_kit::test]
    fn split_panes_carry_their_own_tabs(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        handle
            .update(cx, |c, _, _| {
                c.tabs_floating = false;
                c.panes.shape = Shape::Cols;
                c.coordinator = Some("Coordinator".into());
                c.agents = vec![super::super::AgentTab { name: "Rampart".into(), dot: 0 }, super::super::AgentTab { name: "Builder".into(), dot: 1 }];
            })
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("card-head").is_none(), "no card head over split panes");
            let head = rect(window, ("pane-head", 0usize));
            assert_eq!(head.h, 40.0);
            for i in 0..3usize {
                assert!(window.try_find(("ptab", i)).is_some(), "tab {i} in the thread's pane");
            }
            let b = rect(window, "layout");
            assert!(b.y >= head.y && b.bottom() <= head.bottom(), "the layout button moves into the first pane's header");
        })
        .unwrap();
        handle.update(cx, |c, _, _| c.panes.drag(Line::V, cauldron_chrome::layout::Rect::new(0.0, 0.0, 10_000.0, 10_000.0), 0.0)).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(rect(window, ("pane", 0usize)).w < 320.0);
            assert_eq!(window.find("ptab-more").label(), Some("2 more tabs"));
            assert!(window.try_find(("ptab", 1usize)).is_none(), "only the active tab");
            window.click("ptab-more", cx);
            window.render_frame(cx);
            assert_eq!(window.find(("ptab", 1usize)).label(), Some("Rampart"), "+N steps to the next");
        })
        .unwrap();
    }

    /// Settings > Appearance > Tabs switches between the two, so they can be compared.
    #[gpui_kit::test]
    fn settings_switches_where_tabs_sit(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("settings", cx);
            window.click(("settings-page", 2usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find(("look", 1usize)).label(), Some("Where tabs sit"));
            window.click("set-tabs", cx);
        })
        .unwrap();
        assert!(!handle.update(cx, |c, _, _| c.tabs_floating).unwrap(), "the switch moves them into the panes");
    }
}
