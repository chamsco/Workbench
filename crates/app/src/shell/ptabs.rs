//! Tabs in the pane header (the coordinator's board 88454ee, Charles 13 Nov: "I'd like to have the
//! tabs in the header of the main content element, in each pane, instead of floating on top"). The
//! thread's tabs (the thread, then its agents' 1:1s) sit in the header of the pane that shows the
//! thread, and the active tab is the title, so the header does not say it twice. Under 320px a pane
//! keeps only the active tab and a "+N" that steps to the next. The floating strip is gone: Charles
//! chose this variant (13 Nov 06:52Z), with no breadcrumb.

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
                .when_live(on, |d| d.max_w(px(220.)).bg(c(pal.selected)))
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
            let mut t = tab(i, name, i == active).on_click(cx.listener(move |this, _, _, cx| {
                this.active = i;
                cx.notify();
            }));
            if i > 0 {
                // An agent's 1:1 closes; the agent goes back to the + menu.
                t = t.gap(px(4.)).child(
                    div()
                        .id(("ptab-close", i))
                        .flex_none()
                        .size(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(radius::TAG))
                        .hover(|d| d.bg(c(pal.hover)))
                        .aria_label("Close tab")
                        .child(glyph("x", 11., fg3))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.close_agent(i - 1);
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            strip = strip.child(t.test_support());
        }
        // + spawns a subagent or opens a 1:1 with an agent not open yet (the floating strip's menu).
        let mut add = div()
            .id("tab-add")
            .relative()
            .size(px(metric::TAB_H))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::INNER))
            .cursor_pointer()
            .hover(|d| d.bg(c(pal.hover)))
            .aria_label(ADD_LABEL)
            .child(glyph("plus", 13., fg3))
            .on_click(cx.listener(|this, _, _, cx| {
                this.tab_menu = !this.tab_menu;
                this.tab_slide.reset();
                cx.notify();
            }));
        if self.tab_menu {
            add = add.child(deferred(div().absolute().top(px(metric::TAB_H + 4.)).left_0().child(self.tab_menu_el(pal, cx))).with_priority(2));
        }
        strip.child(add.test_support())
    }

    /// Windows' card head (the board e0fd268, B): the project, then the thread, at the left; the
    /// tabs are in the strip above the pane.
    pub(super) fn crumb(&self, pal: &'static Palette) -> Stateful<Div> {
        let title = self.thread.as_ref().map_or_else(|| self.coordinator.clone().unwrap_or_else(|| NEW_TAB.into()).to_string(), |t| t.title.clone());
        let folder = self.thread.as_ref().and_then(|t| t.project.as_ref()).map(|p| p.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(p).to_string());
        let label = match &folder {
            Some(f) => format!("{f} / {title}"),
            None => title.clone(),
        };
        div()
            .id("crumb")
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(4.))
            .px(px(4.))
            .text_size(px(12.))
            .text_color(c(pal.fg3))
            .aria_label(label)
            .children(folder.map(|f| div().flex_none().child(format!("{f} /"))))
            .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).text_color(c(pal.fg2)).child(title))
    }

    /// The right end of a pane's header: split (the layout button), the terminal and the Code pill.
    /// `slot` is the pane, or None for the card head. `narrow` drops the terminal but keeps split, so
    /// a pane of three at 320px can still step the layout. On the new thread's painting the group
    /// is a glass pill.
    pub(super) fn head_controls(&self, slot: Option<usize>, pal: &'static Palette, now: std::time::Instant, reduce: bool, narrow: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = |n: &'static str| -> ElementId {
            match slot {
                None => n.into(),
                Some(i) => (n, i).into(),
            }
        };
        let mut g = div().id(id("head-ctl")).relative().flex_none().flex().items_center().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL));
        if slot.is_none() {
            g = g.children(self.glass("head-ctl", radius::CONTROL));
        }
        g = g.child(self.layout_button(id("layout"), pal, now, reduce, cx).test_support());
        if !narrow {
            let term = self
                .tipped(id("term-btn"), icon_button(id("term-btn"), "terminal", "Terminal", pal, self.terminal), "Terminal", Side::Below, pal, cx)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.terminal = !this.terminal;
                    this.term_scroll.scroll_to_bottom();
                    cx.notify();
                }));
            g = g.child(term.test_support());
        }
        // ponytail: the thread's mode. Only coding threads exist yet, so it is a label (Charles asked
        // for the pill by name); it becomes a switch when core has other modes.
        g.child(
            div()
                .id(id("code-pill"))
                .relative()
                .h(px(24.))
                .flex()
                .items_center()
                .px(px(8.))
                .ml(px(2.))
                .rounded(px(radius::CONTROL))
                .border_1()
                .border_color(c(pal.line_strong))
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(pal.fg2))
                .aria_label("Code")
                .child("Code")
                .test_support(),
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

    /// A thread's tabs sit in its card head, the active one is the title (no separate title), the
    /// head ends with split, the terminal and the Code pill, and there is no breadcrumb.
    #[gpui_kit::test]
    fn the_tabs_sit_in_the_pane_header(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find(("ptab", 0usize)).label(), Some("Get M3 through review"));
            assert_eq!(window.find(("ptab", 0usize)).selected(), Some(true));
            assert!(window.try_find("thread-title").is_none() && window.try_find("bar-crumb").is_none(), "no title twice, no breadcrumb");
            let head = rect(window, "card-head");
            assert_eq!(head.h, 40.0);
            let mut last = rect(window, ("ptab", 0usize)).right();
            for id in ["layout", "term-btn", "code-pill"] {
                let r = rect(window, id);
                assert!(r.x >= last && r.y >= head.y && r.bottom() <= head.bottom(), "{id} at the head's right, in order");
                last = r.right();
            }
            assert_eq!(window.find("code-pill").label(), Some("Code"));
        })
        .unwrap();
    }

    /// Split, each pane has its own 40px header: the thread's pane carries the thread's tabs, every
    /// pane split, the terminal and Code; the card head is gone. Under 320px the thread's pane keeps
    /// the active tab, "+N" and Code.
    #[gpui_kit::test]
    fn split_panes_carry_their_own_tabs(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        handle
            .update(cx, |c, _, _| {
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
            for slot in 0..2usize {
                let h = rect(window, ("pane-head", slot));
                for id in ["layout", "term-btn", "code-pill"] {
                    let b = rect(window, (id, slot));
                    assert!(b.y >= h.y && b.bottom() <= h.bottom(), "{id} in pane {slot}'s header");
                }
            }
        })
        .unwrap();
        handle.update(cx, |c, _, _| c.panes.drag(Line::V, cauldron_chrome::layout::Rect::new(0.0, 0.0, 10_000.0, 10_000.0), 0.0)).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(rect(window, ("pane", 0usize)).w < 320.0);
            assert_eq!(window.find("ptab-more").label(), Some("2 more tabs"));
            assert!(window.try_find(("ptab", 1usize)).is_none(), "only the active tab");
            assert!(window.try_find(("code-pill", 0usize)).is_some() && window.try_find(("layout", 0usize)).is_some(), "Code and split stay");
            assert!(window.try_find(("term-btn", 0usize)).is_none(), "the terminal makes way");
            window.click("ptab-more", cx);
            window.render_frame(cx);
            assert_eq!(window.find(("ptab", 1usize)).label(), Some("Rampart"), "+N steps to the next");
        })
        .unwrap();
    }

    /// Windows (the board e0fd268, B): the thread's tabs float in a strip above its pane, in the
    /// Shell's top band, clear of the caption buttons; the card head keeps the breadcrumb and the
    /// controls; the card starts 38px down.
    #[gpui_kit::test]
    fn windows_puts_the_tabs_above_the_pane(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        handle.update(cx, |c, _, _| c.plat = cauldron_chrome::layout::Platform::Windows).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
            window.render_frame(cx);
            let (card, strip, caps) = (rect(window, "card"), rect(window, ("pane-strip", 0usize)), rect(window, "caps"));
            assert_eq!(card.y, 38.0);
            assert!(strip.bottom() <= card.y && strip.h == 26.0, "the strip above the card");
            assert!(strip.right() <= caps.x - 8.0 + 0.5, "clear of the caption buttons");
            let t = rect(window, ("ptab", 0usize));
            assert!(t.y >= strip.y && t.bottom() <= strip.bottom(), "the tab in the strip");
            assert_eq!(window.find("crumb").label(), Some("Cauldron / Get M3 through review"), "the head keeps the breadcrumb");
            assert!(window.try_find("code-pill").is_some());
        })
        .unwrap();
    }
}
