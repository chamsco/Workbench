//! The terminal card (v3 frame 5): Warp-style blocks of the commands the thread's agents ran, read
//! from core's traces (each `Bash` tool span is one block), and an input line. Core has no shell
//! of its own for you yet, so a typed command is not run: the line says so instead of pretending.
//! A line starting with `#` goes to the thread's agent, as the composer would send it.

use super::*;
use backspace_core::chat::Role;

/// One command block: what ran, who ran it, when, for how long, how it ended and what it printed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Block {
    pub command: String,
    pub output: String,
    /// The agent that ran it: the model that answered, or "agent".
    pub who: String,
    /// Unix ms.
    pub start: u64,
    pub end: Option<u64>,
    /// Core records a failure, not an exit code.
    pub failed: bool,
}

/// The blocks of a thread, oldest first: every `Bash` span in its replies' traces.
pub(crate) fn blocks(chats: &Chats, t: Option<&Thread>) -> Vec<Block> {
    let mut out = Vec::new();
    for m in t.into_iter().flat_map(|t| t.messages.iter()).filter(|m| m.role != Role::User) {
        let Some(tr) = m.trace.as_ref().and_then(|id| chats.trace(id)) else { continue };
        for s in tr.spans.iter().filter(|s| s.kind == "tool" && s.name == "Bash") {
            let command = serde_json::from_str::<serde_json::Value>(&s.input)
                .ok()
                .and_then(|v| v.get("command").and_then(|c| c.as_str()).map(str::to_string))
                .unwrap_or_else(|| s.input.clone());
            out.push(Block {
                command,
                output: s.output.trim_end().to_string(),
                who: m.model.clone().unwrap_or_else(|| "agent".into()),
                start: s.start,
                end: s.end,
                failed: s.error.is_some(),
            });
        }
    }
    out
}

/// "agent · 17:27 · 1.2s · failed": the meta line under a block's command.
pub(crate) fn meta(b: &Block) -> String {
    let mut parts = vec![b.who.clone()];
    if b.start > 0 {
        if let Some(t) = chrono::DateTime::from_timestamp_millis(b.start as i64) {
            parts.push(t.with_timezone(&chrono::Local).format("%H:%M").to_string());
        }
    }
    if let Some(end) = b.end.filter(|e| *e >= b.start && b.start > 0) {
        let ms = end - b.start;
        parts.push(if ms < 1000 { format!("{ms}ms") } else { format!("{:.1}s", ms as f32 / 1000.0) });
    }
    parts.push(if b.failed { "failed".into() } else if b.end.is_some() { "ok".into() } else { "no exit recorded".into() });
    parts.join(" · ")
}

/// What the input line answers when a command is typed: core has no shell for you yet.
pub(crate) const NO_SHELL: &str = "This terminal cannot run commands yet: core has no shell for it. Start a line with # to ask the agent.";
pub(crate) const PLACEHOLDER: &str = "Type a command, or # to ask the agent";

impl Cauldron {
    /// The terminal's input line, made once with the window it lives in.
    pub(super) fn ensure_term_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.term_input.is_some() {
            return;
        }
        let st = cx.new(|cx| InputState::new(window, cx).placeholder(PLACEHOLDER));
        let sub = cx.subscribe_in(&st, window, |this, st, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let line = st.read(cx).value().trim().to_string();
                this.run_term_line(&line);
                if this.term_note.as_deref() != Some(NO_SHELL) {
                    st.update(cx, |s, cx| s.set_value("", window, cx));
                }
                cx.notify();
            }
        });
        self.term_input = Some((st, sub));
    }

    /// `# question` goes to the thread's agent; anything else cannot run, and the line says why.
    pub(super) fn run_term_line(&mut self, line: &str) {
        if line.is_empty() {
            return;
        }
        if let Some(ask) = line.strip_prefix('#') {
            let ask = ask.trim();
            let (Some(id), Some((_, chats))) = (self.open.clone(), self.core.as_ref()) else {
                self.term_note = Some("Open a thread first: # asks that thread's agent.".into());
                return;
            };
            self.term_note = match chats.send(&id, ask, Vec::new(), None, thread::ctx()) {
                Ok(()) => None,
                Err(e) => Some(e.to_string()),
            };
            self.refresh();
        } else {
            self.term_note = Some(NO_SHELL.into());
        }
    }

    /// The blocks and the input line: the terminal card's body, and a Terminal pane's.
    pub(super) fn term_body(&self, pal: &'static Palette, id: &'static str) -> Stateful<Div> {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let line = c(pal.line);
        let mut list = div().id(id).flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.term_scroll).flex().flex_col();
        if self.term_blocks.is_empty() {
            let (title, body) = if self.open.is_some() {
                ("No commands yet", "Commands this thread's agents run show up here, one block each.")
            } else {
                ("No thread open", "Open a thread to see the commands its agents ran.")
            };
            list = list.child(
                div()
                    .id("term-empty")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .gap(px(4.))
                    .p(px(16.))
                    .aria_label(title)
                    .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(c(pal.fg2)).child(title))
                    .child(div().max_w_full().text_center().text_size(px(12.)).text_color(fg3).child(body))
                    .test_support(),
            );
        }
        for (i, b) in self.term_blocks.iter().enumerate() {
            let mut blk = div()
                .id(("block", i))
                .relative()
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(4.))
                .px(px(16.))
                .py(px(8.))
                .border_t_1()
                .border_color(line)
                .aria_label(b.command.clone())
                .child(div().text_size(px(11.)).line_height(px(14.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child(meta(b)))
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .font_family("Geist Mono")
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .child(div().flex_none().text_color(fg3).child("$"))
                        .child(div().flex_1().min_w_0().text_color(fg).child(b.command.clone())),
                );
            if !b.output.is_empty() {
                blk = blk.child(div().font_family("Geist Mono").text_size(px(12.)).line_height(px(18.)).text_color(fg3).child(b.output.clone()));
            }
            if b.failed {
                // A failed block takes the bad colour on its left edge.
                blk = blk.child(div().absolute().left_0().top_0().bottom_0().w(px(2.)).bg(c(pal.bad)));
            }
            list = list.child(blk.test_support());
        }
        let input = self.term_input.as_ref().map(|(st, _)| st.clone());
        let mut tail = div().id(format!("{id}-in")).flex_none().flex().flex_col().gap(px(4.)).px(px(16.)).py(px(12.)).border_t_1().border_color(line);
        if let Some(st) = input {
            tail = tail.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .font_family("Geist Mono")
                    .text_size(px(12.))
                    .child(div().flex_none().text_color(fg3).child("$"))
                    .child(div().flex_1().min_w_0().child(Input::new(&st).appearance(false).text_size(px(12.)).p_0())),
            );
        }
        if let Some(n) = &self.term_note {
            tail = tail.child(div().id(format!("{id}-note")).text_size(px(11.)).line_height(px(14.)).text_color(fg3).aria_label(n.clone()).child(n.clone()).test_support());
        }
        div().id(format!("{id}-body")).size_full().flex().flex_col().child(list.test_support()).child(tail.test_support())
    }

    /// The terminal card: its head, then the blocks and the input line.
    pub(super) fn terminal_card(&self, pal: &'static Palette, r: Rect, alpha: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg3 = c(pal.fg3);
        let title = self.thread.as_ref().map(|t| t.title.clone());
        let head = div()
            .id("term-head")
            .h(px(44.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(16.))
            .pr(px(8.))
            .border_b_1()
            .border_color(c(pal.line))
            .text_size(px(13.))
            .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(pal.fg)).child("Terminal"))
            .children(title.map(|t| div().min_w_0().truncate().text_color(fg3).child(t)))
            .child(div().flex_1())
            .child(
                div()
                    .id("term-close")
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::INNER))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label("Close the terminal")
                    .child(glyph("x", 13., fg3))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.terminal = false;
                        cx.notify();
                    }))
                    .test_support(),
            );
        // Over the painting the terminal is frosted glass; without it, the panel.
        let frost = self.glass_card.get().and_then(|(_, _, dark)| {
            let mut b = self.backdrop.borrow_mut();
            let cut = (r.x as i32, r.y as i32, r.w as u32, r.h as u32);
            let tint = b.hsla_for(super::backdrop::Surface::Card, dark, cut, pal);
            b.glass(cut.0, cut.1, cut.2, cut.3).map(|f| (f, tint))
        });
        pane("terminal", r)
            .opacity(alpha)
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(radius::PANEL))
            .when_live(frost.is_none(), |d| d.bg(c(pal.panel)))
            .children(frost.map(|(f, tint)| {
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .child(img(f).absolute().top_0().left_0().size_full().rounded(px(radius::PANEL)))
                    .child(div().absolute().top_0().left_0().size_full().rounded(px(radius::PANEL)).bg(tint))
            }))
            .border_1()
            .border_color(c(pal.line))
            .child(head.test_support())
            .child(div().flex_1().min_h_0().child(self.term_body(pal, "term-blocks")))
    }
}

#[cfg(test)]
mod tests {
    use super::{meta, Block, NO_SHELL};
    use super::super::thread::tests::open;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};

    fn block(failed: bool, end: Option<u64>) -> Block {
        Block { command: "cargo test".into(), output: String::new(), who: "opus".into(), start: 0, end, failed }
    }

    /// The meta line says who ran it and how it ended; with no exit recorded it says so.
    #[test]
    fn the_meta_line_says_who_and_how_it_ended() {
        assert_eq!(meta(&block(false, Some(5))), "opus · ok");
        assert_eq!(meta(&block(true, Some(5))), "opus · failed");
        assert_eq!(meta(&block(false, None)), "opus · no exit recorded");
        let b = Block { start: 1_000, end: Some(2_500), ..block(false, None) };
        assert!(meta(&b).ends_with("· 1.5s · ok"), "{}", meta(&b));
    }

    /// The Terminal button opens the card in the drawer's place with one block per command the
    /// thread's agent ran (from its trace), the failed one marked; a typed command is not run.
    #[gpui_kit::test]
    fn the_terminal_shows_the_threads_commands(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        // The shell moves on the wall clock, so each check starts from rest.
        let rest = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| c.motion = Default::default()).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
            assert!(window.try_find("terminal").is_none());
            window.click("term-btn", cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("terminal").is_some(), "the card opens");
            assert!(window.try_find("drawer").is_none() && window.try_find("rrail").is_some(), "the drawer folds to its rail");
            assert_eq!(window.find(("block", 0usize)).label(), Some("cargo test"), "the agent's command, from the trace");
            assert!(window.try_find(("block", 1usize)).is_none(), "one Bash span, one block");
            window.click("term-close", cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("terminal").is_none() && window.try_find("drawer").is_some(), "closing brings the drawer back");
        })
        .unwrap();
        handle
            .update(cx, |c, _, _| {
                c.run_term_line("ls -la");
                assert_eq!(c.term_note.as_deref(), Some(NO_SHELL), "no shell: the command is not run, and the line says so");
            })
            .unwrap();
    }
}
