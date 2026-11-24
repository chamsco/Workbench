//! Search streams as a Spotlight-style overlay (Charles, 23 Nov: "the search on the left sidebar is
//! not really working ... the search streams should pop up something like a macOS highlight").
//! The sidebar's search button, the folded band's search and Ctrl+K / Cmd+K open it; it searches
//! every stream (archived and snoozed too) by title, last reply, folder and branch. Up and Down move
//! the highlight, Enter opens, Escape or a click outside closes.

use super::*;
use gpui_kit::prelude::FluentBuilder;

/// Rows the overlay shows at most.
const MAX_ROWS: usize = 9;
const SPOT_W: f32 = 560.0;

pub(crate) struct Spot {
    pub input: Entity<InputState>,
    pub _sub: Subscription,
    pub q: String,
    pub sel: usize,
}

/// The streams matching `q`, best first: title hits before other hits, then the most recently
/// updated. Side streams count; an empty query lists the most recent streams.
pub(crate) fn matches<'a>(list: &'a [ThreadInfo], q: &str) -> Vec<&'a ThreadInfo> {
    let q = q.trim().to_lowercase();
    let mut v: Vec<(u8, &ThreadInfo)> = list
        .iter()
        .filter(|t| !t.ghost)
        .filter_map(|t| {
            if q.is_empty() {
                return Some((0, t));
            }
            if t.title.to_lowercase().contains(&q) {
                Some((0, t))
            } else if (t.project.is_some() && threads::meta(t).to_lowercase().contains(&q)) || t.branch.as_deref().is_some_and(|b| b.to_lowercase().contains(&q)) {
                Some((1, t))
            } else if t.preview.to_lowercase().contains(&q) {
                Some((2, t))
            } else {
                None
            }
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.updated.cmp(&a.1.updated)));
    v.into_iter().map(|(_, t)| t).take(MAX_ROWS).collect()
}

impl Cauldron {
    /// Opens the overlay with an empty query and the caret in it.
    pub(super) fn open_spotlight(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spot.is_some() {
            if let Some(s) = &self.spot {
                s.input.update(cx, |st, cx| st.focus(window, cx));
            }
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search streams"));
        let sub = cx.subscribe_in(&input, window, |this, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                if let Some(s) = this.spot.as_mut() {
                    s.q = st.read(cx).value().to_string();
                    s.sel = 0;
                }
                cx.notify();
            }
            InputEvent::PressEnter { .. } => this.spot_open_selected(window, cx),
            _ => {}
        });
        input.update(cx, |st, cx| st.focus(window, cx));
        self.spot = Some(Spot { input, _sub: sub, q: String::new(), sel: 0 });
        self.search_open = false;
        cx.notify();
    }

    pub(super) fn close_spotlight(&mut self, cx: &mut Context<Self>) {
        self.spot = None;
        cx.notify();
    }

    fn spot_list(&self) -> Vec<ThreadInfo> {
        let (Some(s), Threads::Ready(v)) = (&self.spot, &self.threads) else { return Vec::new() };
        matches(v, &s.q).into_iter().cloned().collect()
    }

    fn spot_open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sel) = self.spot.as_ref().map(|s| s.sel) else { return };
        if let Some(t) = self.spot_list().get(sel) {
            self.spot_open(t.id.clone(), window, cx);
        }
    }

    fn spot_open(&mut self, id: String, _window: &mut Window, cx: &mut Context<Self>) {
        self.spot = None;
        self.settings = None;
        if self.section != 0 {
            self.select_section(0, cx);
        }
        self.selected = Some(id.clone());
        self.open_thread(id);
        cx.notify();
    }

    /// The overlay, over the whole window; None while it is closed.
    pub(super) fn spotlight_el(&self, pal: &'static Palette, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let s = self.spot.as_ref()?;
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let dark = pal.panel.0 < 0x80;
        let rows = self.spot_list();
        let sel = s.sel.min(rows.len().saturating_sub(1));
        let vp = window.viewport_size();
        let w = SPOT_W.min(f32::from(vp.width) - 32.0);
        let top = (f32::from(vp.height) * 0.18).max(48.0);
        let mut list = div().id("spot-list").flex().flex_col().gap(px(2.)).p(px(6.));
        if rows.is_empty() {
            list = list.child(div().id("spot-empty").px(px(10.)).py(px(14.)).text_size(px(13.)).text_color(fg3).child(format!("No streams match \u{201c}{}\u{201d}", s.q.trim())).test_support());
        }
        for (i, t) in rows.iter().enumerate() {
            let on = i == sel;
            let id = t.id.clone();
            let status = threads::Status::of(t).map(|st| st.label()).or(t.archived.then_some("Archived")).or(threads::snoozed(t, backspace_core::chat::now_ms()).then_some("Snoozed"));
            let folder = threads::meta(t);
            list = list.child(
                div()
                    .id(("spot-row", i))
                    .h(px(44.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .when(on, |d| d.bg(c(pal.selected)))
                    .when(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                    .aria_label(t.title.clone())
                    .aria_selected(on)
                    .child(glyph(if t.side_of.is_some() { "corner-down-right" } else { "message-square" }, 14., if on { fg2 } else { fg3 }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().truncate().text_size(px(13.5)).text_color(fg).child(t.title.clone()))
                            .child(div().truncate().text_size(px(11.5)).text_color(fg3).child(match &t.branch {
                                Some(b) => format!("{folder} \u{b7} {b}"),
                                None => folder,
                            })),
                    )
                    .when_some(status, |d, st| d.child(div().flex_none().text_size(px(11.)).text_color(fg3).child(st)))
                    .on_mouse_move(cx.listener(move |this, _: &MouseMoveEvent, _, cx| {
                        if let Some(s) = this.spot.as_mut() {
                            if s.sel != i {
                                s.sel = i;
                                cx.notify();
                            }
                        }
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.spot_open(id.clone(), window, cx);
                    }))
                    .test_support(),
            );
        }
        let n = rows.len();
        let panel = div()
            .id("spotlight")
            .absolute()
            .top(px(top))
            .left(px((f32::from(vp.width) - w) / 2.0))
            .w(px(w))
            .flex()
            .flex_col()
            .rounded(px(14.))
            .bg(c(pal.panel))
            .border_1()
            .border_color(c(pal.line_strong))
            .shadow(lift(Lift::Frame, pal))
            .overflow_hidden()
            .occlude()
            .aria_label("Search streams")
            .on_click(|_, _, cx| cx.stop_propagation())
            .capture_key_down(cx.listener(move |this, ev: &KeyDownEvent, _, cx| {
                let Some(s) = this.spot.as_mut() else { return };
                match ev.keystroke.key.as_str() {
                    "down" => s.sel = (s.sel + 1).min(n.saturating_sub(1)),
                    "up" => s.sel = s.sel.saturating_sub(1),
                    "escape" => this.spot = None,
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                div()
                    .h(px(50.))
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .border_b_1()
                    .border_color(c(pal.line))
                    .child(glyph("search", 17., fg2))
                    .child(div().id("spot-input").flex_1().min_w_0().child(Input::new(&s.input).appearance(false).text_size(px(16.)).p_0()).test_support())
                    .child(div().flex_none().px(px(6.)).h(px(20.)).flex().items_center().rounded(px(radius::TAG)).border_1().border_color(c(pal.line)).text_size(px(10.5)).text_color(fg3).child("esc")),
            )
            .child(list.test_support())
            .test_support();
        Some(
            div()
                .id("spot-scrim")
                .absolute()
                .inset_0()
                .bg(gpui_kit::black().opacity(if dark { 0.35 } else { 0.18 }))
                .occlude()
                .on_click(cx.listener(|this, _, _, cx| this.close_spotlight(cx)))
                .child(panel)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: &str, title: &str, updated: u64) -> ThreadInfo {
        let mut v = threads::sample(id, title);
        v.updated = updated;
        v
    }

    /// The old sidebar field only ever showed the open stream's name (Charles, 23 Nov). The overlay
    /// must find a stream by a word in its title even when it is not the most recent, and put title
    /// hits ahead of hits in the last reply.
    #[::core::prelude::v1::test]
    fn finds_by_title_first_then_by_reply() {
        let mut by_reply = t("a", "Tray icon", 30);
        by_reply.preview = "the router weights changed".into();
        let v = vec![by_reply, t("b", "Router weights", 10), t("c", "Something else", 20)];
        let ids: Vec<&str> = matches(&v, "router").iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a"]);
    }
}
