//! The terminal card (v3 frame 5): Warp-style blocks of the commands the thread's agents ran, read
//! from core's traces (each `Bash` tool span is one block), or from the reply's steps when it has
//! no trace, and an input line. A typed command runs in the thread's folder (sh, or PowerShell on
//! Windows) and lands as a block of yours. A line starting with `#` goes to the thread's agent.

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

/// The blocks of a thread, oldest first: every `Bash` span in its replies' traces. A trace is
/// written once, when its reply ends, so each is read from disk once and kept in `cache` by trace
/// id: core pokes the view many times a second while a reply streams, and re-reading every trace
/// of a long stream each time cost milliseconds that grew with its length.
pub(crate) fn blocks(chats: &Chats, t: Option<&Thread>, cache: &mut std::collections::HashMap<String, Vec<Block>>) -> Vec<Block> {
    let mut out = Vec::new();
    for m in t.into_iter().flat_map(|t| t.messages.iter()).filter(|m| m.role != Role::User) {
        if let Some(id) = m.trace.as_ref() {
            if !cache.contains_key(id) {
                if let Some(tr) = chats.trace(id) {
                    cache.insert(id.clone(), trace_blocks(&tr, m.model.clone()));
                }
            }
            if let Some(b) = cache.get(id) {
                out.extend(b.iter().cloned());
                continue;
            }
        }
        // No trace (a runner that writes none, or one not written yet): the reply's own steps.
        out.extend(act_blocks(m));
    }
    out
}

fn act_blocks(m: &backspace_core::chat::Msg) -> Vec<Block> {
    let who = m.model.clone().unwrap_or_else(|| "agent".into());
    m.acts
        .iter()
        .filter(|a| a.name.eq_ignore_ascii_case("bash") || a.name.eq_ignore_ascii_case("powershell"))
        .map(|a| Block {
            command: super::ledger::command_of(&a.input).unwrap_or_else(|| a.input.clone()),
            output: a.output.trim_end().to_string(),
            who: who.clone(),
            start: a.start,
            end: a.end,
            failed: a.error,
        })
        .collect()
}

/// Runs `line` in `dir`: sh -c, or PowerShell on Windows. Gives what it printed (both streams) and
/// whether it failed.
pub(crate) fn run_shell(line: &str, dir: Option<&std::path::Path>) -> (String, bool) {
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("powershell");
        c.args(["-NoProfile", "-NonInteractive", "-Command", line]);
        c
    } else {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", line]);
        c
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    if let Some(d) = dir.filter(|d| d.is_dir()) {
        cmd.current_dir(d);
    }
    match cmd.stdin(std::process::Stdio::null()).output() {
        Ok(o) => {
            let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.trim().is_empty() {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&err);
            }
            if !o.status.success() {
                if let Some(code) = o.status.code() {
                    text.push_str(&format!("\nexit code {code}"));
                }
            }
            (text.trim_end().to_string(), !o.status.success())
        }
        Err(e) => (format!("Could not start the shell: {e}"), true),
    }
}

fn trace_blocks(tr: &backspace_core::trace::Trace, model: Option<String>) -> Vec<Block> {
    let who = model.unwrap_or_else(|| "agent".into());
    tr.spans
        .iter()
        .filter(|s| s.kind == "tool" && s.name == "Bash")
        .map(|s| Block {
            command: serde_json::from_str::<serde_json::Value>(&s.input)
                .ok()
                .and_then(|v| v.get("command").and_then(|c| c.as_str()).map(str::to_string))
                .unwrap_or_else(|| s.input.clone()),
            output: s.output.trim_end().to_string(),
            who: who.clone(),
            start: s.start,
            end: s.end,
            failed: s.error.is_some(),
        })
        .collect()
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
    parts.push(if b.failed { "failed".into() } else if b.end.is_some() { "ok".into() } else if b.who == YOU { "running".into() } else { "no exit recorded".into() });
    parts.join(" · ")
}

/// Who ran a command typed in the terminal.
pub(crate) const YOU: &str = "you";
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
                this.run_term_line(&line, cx);
                st.update(cx, |s, cx| s.set_value("", window, cx));
                cx.notify();
            }
        });
        self.term_input = Some((st, sub));
    }

    /// `# question` goes to the thread's agent; anything else runs in the thread's folder.
    pub(super) fn run_term_line(&mut self, line: &str, cx: &mut Context<Self>) {
        if line.is_empty() {
            return;
        }
        if let Some(ask) = line.strip_prefix('#') {
            let ask = ask.trim();
            let (Some(id), Some((_, chats))) = (self.open.clone(), self.core.as_ref()) else {
                self.term_note = Some("Open a thread first: # asks that thread's agent.".into());
                return;
            };
            self.term_note = match chats.send(&id, ask, Vec::new(), None, self.mode_ctx()) {
                Ok(()) => None,
                Err(e) => Some(e.to_string()),
            };
            self.refresh();
        } else {
            self.term_note = None;
            let key = self.open.clone().unwrap_or_default();
            let dir = self.thread.as_ref().and_then(|t| t.project.clone()).map(std::path::PathBuf::from);
            let start = backspace_core::chat::now_ms();
            let runs = self.term_runs.entry(key.clone()).or_default();
            runs.push(Block { command: line.to_string(), output: String::new(), who: YOU.into(), start, end: None, failed: false });
            let at = runs.len() - 1;
            self.term_scroll.scroll_to_bottom();
            let line = line.to_string();
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let (out, failed) = cx.background_executor().spawn(async move { run_shell(&line, dir.as_deref()) }).await;
                let _ = this.update(cx, |c, cx| {
                    if let Some(b) = c.term_runs.get_mut(&key).and_then(|r| r.get_mut(at)) {
                        b.output = out;
                        b.failed = failed;
                        b.end = Some(backspace_core::chat::now_ms());
                    }
                    c.term_scroll.scroll_to_bottom();
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// The blocks and the input line: the terminal card's body, and a Terminal pane's.
    pub(super) fn term_body(&self, pal: &'static Palette, id: &'static str) -> Stateful<Div> {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let line = c(pal.line);
        let mut list = div().id(id).flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.term_scroll).flex().flex_col();
        let mut shown: Vec<&Block> = self.term_blocks.iter().chain(self.term_runs.get(self.open.as_deref().unwrap_or_default()).into_iter().flatten()).collect();
        shown.sort_by_key(|b| b.start);
        if shown.is_empty() {
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
        for (i, b) in shown.into_iter().enumerate() {
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
            .h(px(metric::HEAD_H))
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
                    .rounded(px(radius::CONTROL))
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
        pane("terminal", r)
            .opacity(alpha)
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(radius::PANEL))
            .bg(super::backdrop::surface(self.painting, pal.panel, super::backdrop::GLASS_TERM))
            .border_1()
            .border_color(c(pal.line))
            .child(head.test_support())
            .child(div().flex_1().min_h_0().child(self.term_body(pal, "term-blocks")))
    }
}

#[cfg(test)]
mod tests {
    use super::{meta, Block};
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

    /// The Terminal button docks the card under the stream (Tight tiles rule 5) with one block per
    /// command the stream's agent ran (from its trace), the failed one marked; a typed command is
    /// not run. The support card is not touched.
    #[gpui_kit::test]
    fn the_terminal_shows_the_threads_commands(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        // The shell moves on the wall clock, so each check starts from rest.
        let rest = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| c.motion = Default::default()).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
            assert!(window.try_find("terminal").is_none());
            window.click("top-term", cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("terminal").is_some(), "the card opens");
            let (term, main) = (super::super::tests::rect(window, "terminal"), super::super::tests::rect(window, "main"));
            assert_eq!((term.x, term.w, term.y), (main.x, main.w, main.bottom() + 4.0), "docked under the stream, 4px below it");
            assert_eq!(window.find(("block", 0usize)).label(), Some("cargo test"), "the agent's command, from the trace");
            assert!(window.try_find(("block", 1usize)).is_none(), "one Bash span, one block");
            window.click("term-close", cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("terminal").is_none(), "closing undocks it");
        })
        .unwrap();
    }

    /// A typed command runs, in the given folder, and its output and failure come back.
    #[::core::prelude::v1::test]
    fn a_typed_command_runs_in_the_streams_folder() {
        let dir = std::env::temp_dir().join(format!("cauldron-term-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("marker.txt"), "x").unwrap();
        let (out, failed) = super::run_shell(if cfg!(windows) { "Get-ChildItem -Name" } else { "ls" }, Some(&dir));
        assert!(out.contains("marker.txt") && !failed, "{out}");
        let (_, failed) = super::run_shell("exit 3", Some(&dir));
        assert!(failed);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
