//! Side streams (board "Side streams A", 21 Nov): the tab row's + opens a small window that asks
//! a side question off the open stream. The side stream is its own stream with its own CLI
//! session (core `Chats::side`), so the main never waits; its tab sits in the main's group on the
//! same line, and the sidebar keeps one row for the main with a count of its side streams.
//! Borrowed from Orca's `/btw`: it starts from the main's last reply unless asked otherwise.

use super::*;
use backspace_core::chat::SideCtx;
use gpui_kit::prelude::FluentBuilder;

pub(super) const SIDE_W: f32 = 340.0;

impl Cauldron {
    /// Re-reads which main the open stream belongs to and its side streams, oldest first.
    pub(super) fn sync_sides(&mut self) {
        let root = self.thread.as_ref().map(|t| t.side_of.clone().unwrap_or_else(|| t.id.clone()));
        self.sides = match (&root, &self.threads) {
            (Some(r), Threads::Ready(v)) => {
                let mut s: Vec<_> = v.iter().filter(|t| t.side_of.as_deref() == Some(r.as_str())).map(|t| (t.created, t.id.clone(), t.title.clone())).collect();
                s.sort();
                s.into_iter().map(|(_, id, t)| (id, t)).collect()
            }
            _ => Vec::new(),
        };
        // The anchor tab names the main, wherever in the group you are.
        if let (Some(r), Some((_, chats))) = (&root, &self.core) {
            if self.thread.as_ref().is_some_and(|t| t.side_of.is_some()) {
                if let Some(m) = chats.peek(r) {
                    self.coordinator = Some(m.title.into());
                }
            }
        }
        self.side_root = root;
    }

    /// Opens the + window with an empty question, focused.
    pub(super) fn open_side_win(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx).placeholder("Ask a side question…"));
        let sub = cx.subscribe_in(&st, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.start_side(window, cx);
            }
        });
        st.read(cx).focus_handle(cx).focus(window, cx);
        self.side_input = Some((st, sub));
        self.side_win = true;
    }

    /// Starts the side stream, sends the question, and opens it.
    pub(super) fn start_side(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(id), Some((_, chats)), Some((st, _))) = (self.open.clone(), self.core.as_ref(), self.side_input.as_ref()) else { return };
        let q = st.read(cx).value().trim().to_string();
        if q.is_empty() {
            return;
        }
        let chats = chats.clone();
        match chats.side(&id, &q, self.side_ctx) {
            Ok(t) => {
                if let Err(e) = chats.send(&t.id, &q, Vec::new(), None, super::thread::ctx(self.plan)) {
                    self.send_error = Some(e.to_string());
                }
                st.update(cx, |s, cx| s.set_value("", window, cx));
                self.side_win = false;
                self.open_thread(t.id);
            }
            Err(e) => self.send_error = Some(e.to_string()),
        }
        cx.notify();
    }

    /// The + window.
    pub(super) fn side_window(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let main = self.coordinator.clone().unwrap_or_else(|| "this stream".into());
        let input = self.side_input.as_ref().map(|(s, _)| s.clone());
        let mut seg = div().id("side-ctx").flex_1().flex().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL)).bg(c(pal.inset));
        for (k, (label, v)) in [("None", SideCtx::None), ("Last reply", SideCtx::LastReply), ("Whole stream", SideCtx::Whole)].into_iter().enumerate() {
            let on = self.side_ctx == v;
            seg = seg.child(
                div()
                    .id(("side-ctx-opt", k))
                    .flex_1()
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .whitespace_nowrap()
                    .rounded(px(radius::TAG))
                    .cursor_pointer()
                    .text_size(px(11.5))
                    .text_color(if on { fg } else { fg3 })
                    .when(on, |d| d.bg(c(pal.raised)).shadow(vec![drop(tokens::shadow::TIP)]))
                    .when(!on, |d| d.hover(|d| d.text_color(fg2)))
                    .aria_selected(on)
                    .aria_label(label)
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.side_ctx = v;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        let route = self.thread.as_ref().map(|t| super::subs::route_label(&self.start.providers, Some(&t.route))).unwrap_or_default();
        div()
            .id("side-win")
            .w(px(SIDE_W))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .rounded(px(radius::PANEL))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .shadow(vec![drop(tokens::shadow::MENU)])
            .font_family(SANS)
            .font_features(sans_features())
            .text_size(px(12.5))
            .text_color(fg2)
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.side_win = false;
                cx.notify();
            }))
            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("New side stream"))
            .child(div().text_size(px(12.)).text_color(fg3).child(format!("Beside {main}. The main stream keeps going.")))
            .child(
                div()
                    .id("side-q")
                    .h(px(34.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .rounded(px(radius::CONTROL))
                    .bg(c(pal.panel))
                    .border_1()
                    .border_color(c(pal.line_strong))
                    .aria_label("Side question")
                    .when_some(input, |d, s| d.child(div().flex_1().min_w_0().child(Input::new(&s).appearance(false).text_size(px(13.)).p_0())))
                    .test_support(),
            )
            .child(div().flex().items_center().gap(px(8.)).child(div().w(px(56.)).flex_none().text_size(px(12.)).text_color(fg3).child("Context")).child(seg.test_support()))
            .child(div().flex().items_center().gap(px(8.)).child(div().w(px(56.)).flex_none().text_size(px(12.)).text_color(fg3).child("Runs on")).child(div().min_w_0().truncate().text_size(px(12.)).text_color(fg2).child(format!("{route}, as the main"))))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mt(px(2.))
                    .child(
                        div()
                            .id("side-spawn")
                            .cursor_pointer()
                            .text_size(px(11.5))
                            .text_color(fg3)
                            .hover(|d| d.text_color(fg2))
                            .aria_label("Spawn a subagent instead")
                            .child("Spawn a subagent instead…")
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.side_win = false;
                                if let Some((st, _)) = &this.composer {
                                    st.update(cx, |s, cx| s.set_value("Use your delegate tool to hand this to a sub-agent on another CLI: ", window, cx));
                                }
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .child(
                        div()
                            .id("side-start")
                            .h(px(26.))
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .rounded(px(radius::CONTROL))
                            .bg(c(pal.accent))
                            .text_color(c(pal.on_accent))
                            .font_weight(FontWeight::MEDIUM)
                            .cursor_pointer()
                            .aria_label("Start side stream")
                            .child("Start  ↵")
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.start_side(window, cx);
                            }))
                            .test_support(),
                    ),
            )
            .test_support()
            .into_any_element()
    }
}
