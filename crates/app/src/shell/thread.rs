//! The thread view (UI4): the open thread's transcript, the composer that sends to core, and the
//! stage bar's usage. Rows open here from the sidebar; core's change channel keeps it current.
//!
//! Left out until core has them per thread (docs/cauldron/UI3-CORE-MAP.md): ask cards, the stage
//! stepper, teammates' messages and avatars.

use super::*;
use backspace_core::chat::{Ctx, Msg, Role, Status, Thread};
use gpui_kit::component::input::{Textarea, TextareaState};

/// One transcript entry as drawn.
#[derive(Debug, PartialEq)]
pub(super) struct Line {
    pub mine: bool,
    pub text: String,
    pub status: Status,
    pub error: Option<String>,
    /// Who answered, with which model and at what cost, under a reply.
    pub meta: Option<String>,
}

pub(super) fn lines(t: &Thread) -> Vec<Line> {
    t.messages.iter().map(|m| Line { mine: m.role == Role::User, text: m.text.clone(), status: m.status, error: m.error.clone(), meta: meta(m) }).collect()
}

fn meta(m: &Msg) -> Option<String> {
    if m.role == Role::User {
        return None;
    }
    let mut p = Vec::new();
    if let Some(n) = &m.author_name {
        p.push(n.clone());
    }
    if let Some(md) = &m.model {
        p.push(md.clone());
    }
    if let Some(c) = m.cost_usd {
        p.push(format!("${c:.2}"));
    }
    (!p.is_empty()).then(|| p.join(" · "))
}

/// What the thread has cost so far, from the replies that reported a cost.
pub(super) fn cost(t: &Thread) -> f64 {
    // fold from +0.0: an empty f64 sum is -0.0, which prints as "$-0.00".
    t.messages.iter().filter_map(|m| m.cost_usd).fold(0.0, |a, b| a + b)
}

/// What a reply that is not finished says in place of its text.
fn pending(s: Status) -> Option<&'static str> {
    match s {
        Status::Sending => Some("Waiting for the reply"),
        Status::Streaming => Some("Writing"),
        Status::Stopped => Some("Stopped"),
        _ => None,
    }
}

/// The settings a send carries: the saved prefs, nothing else wired yet.
fn ctx() -> Ctx {
    Ctx { prefs: Prefs::load(), memory: None, agents: None, plan: false }
}

/// Efforts the chip cycles through; None is Auto, the CLI's own default.
const EFFORTS: [Option<&str>; 5] = [None, Some("low"), Some("medium"), Some("high"), Some("max")];

/// `a` mixed toward `b` by `t`, as CSS `color-mix(in srgb, b t, a)`.
fn mix(a: Rgb, b: Rgb, t: f32) -> Hsla {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    c(Rgb(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2)))
}

impl Cauldron {
    /// Opens a thread in the main card. Reading it through core marks it read.
    pub(super) fn open_thread(&mut self, id: String) {
        let Some((_, chats)) = &self.core else { return };
        self.thread = chats.thread(&id);
        self.coordinator = Some(self.thread.as_ref().map_or_else(|| "Thread".into(), |t| t.title.clone().into()));
        self.agents.clear();
        self.idle.clear();
        self.active = 0;
        self.selected = Some(id.clone());
        self.open = Some(id);
        self.open_since = Some(std::time::Instant::now());
        self.send_error = None;
        self.threads = Threads::Ready(chats.list());
    }

    /// Re-reads the list and the open thread from core.
    pub(super) fn refresh(&mut self) {
        if let Some((_, chats)) = &self.core {
            self.threads = Threads::Ready(chats.list());
            if let Some(id) = &self.open {
                self.thread = chats.peek(id);
            }
        }
    }

    /// Keeps the view current: core pokes `changes` whenever a thread changes.
    pub(super) fn watch(&mut self, changes: async_channel::Receiver<()>, cx: &mut Context<Self>) {
        self.watch = Some(cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while changes.recv().await.is_ok() {
                if this
                    .update(cx, |c, cx| {
                        c.refresh();
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    pub(super) fn busy(&self) -> bool {
        match (&self.core, &self.open) {
            (Some((_, chats)), Some(id)) => chats.is_busy(id),
            _ => false,
        }
    }

    /// Sends the composer's text to the open thread; a refusal shows under the composer.
    fn send_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(id), Some((_, chats)), Some((st, _))) = (self.open.clone(), self.core.as_ref(), self.composer.as_ref()) else { return };
        let text = st.read(cx).value().trim().to_string();
        if text.is_empty() {
            return;
        }
        match chats.send(&id, &text, Vec::new(), None, ctx()) {
            Ok(()) => {
                self.send_error = None;
                st.update(cx, |s, cx| s.set_value("", window, cx));
            }
            Err(e) => self.send_error = Some(e.to_string()),
        }
        self.refresh();
        cx.notify();
    }

    /// The composer's input, made once with the window it lives in.
    pub(super) fn ensure_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer.is_some() {
            return;
        }
        let st = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 8).submit_on_enter(true).placeholder("Message the coordinator…"));
        let sub = cx.subscribe_in(&st, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { shift: false, .. } = ev {
                this.send_composer(window, cx);
            }
        });
        self.composer = Some((st, sub));
    }

    /// The main card's content: the open thread, a 1:1, or what stands in for them.
    pub(super) fn card_body(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let fg3 = c(pal.fg3);
        let centred = |id: &'static str, title: String, body: &'static str| {
            div()
                .id(id)
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.))
                .aria_label(title.clone())
                .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(c(pal.fg2)).child(title))
                .child(div().text_size(px(12.)).text_color(fg3).child(body))
                .test_support()
                .into_any_element()
        };
        if self.active > 0 {
            let name = self.agents.get(self.active - 1).map_or_else(String::new, |a| a.name.to_string());
            // ponytail: agent tabs are the debug fixture until core lists a thread's agents (THREAD-STATE.md).
            return centred("one-to-one", format!("1:1 with {name}"), "Agent 1:1 transcripts come with core's per-thread agents.");
        }
        match (&self.open, &self.thread) {
            (Some(_), Some(t)) => self.thread_view(t, pal, cx),
            (Some(_), None) => centred("thread-gone", "This thread is no longer here".into(), "It was deleted, or its file could not be read."),
            (None, _) => centred("no-thread", "No thread open".into(), "Pick one in the sidebar, or start one with New thread."),
        }
    }

    fn thread_view(&self, t: &Thread, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let chip = |label: String, col: Rgb| {
            div()
                .flex()
                .items_center()
                .px(px(8.))
                .py(px(3.))
                .rounded(px(radius::R1))
                .border_1()
                .border_color(mix(pal.line, col, 0.45))
                .bg(ca(col, 0.12))
                .text_size(px(11.5))
                .line_height(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(col))
                .child(label)
        };
        let mut head = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .flex_none()
            .child(div().id("thread-title").min_w_0().truncate().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).aria_label(t.title.clone()).child(t.title.clone()).test_support())
            .child(chip("Code".into(), pal.info));
        if let Some(p) = &t.project {
            let name = p.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(p).to_string();
            head = head.child(chip(name, pal.mem));
        }

        // The transcript fades in over the second half of the carry after a thread opens.
        let now = std::time::Instant::now();
        let open_k = self.open_since.map_or(1.0, |t0| cauldron_chrome::motion::fade_in(crate::motion::run(now, t0, cauldron_chrome::motion::time::CARRY)));
        let bubble_k = self.bubble_since.map_or(1.0, |t0| crate::motion::run(now, t0, cauldron_chrome::motion::time::CARRY));
        let mut msgs = div().id("transcript").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap(px(12.)).opacity(open_k);
        let ls = lines(t);
        // The newest of the user's bubbles rises into place (a rise and fade, not a carry from
        // the composer box).
        let fresh_mine = ls.iter().rposition(|l| l.mine);
        if ls.is_empty() {
            msgs = msgs.child(
                div()
                    .id("thread-empty")
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .pt(px(8.))
                    .aria_label("No messages yet")
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(fg2).child("No messages yet"))
                    .child(div().text_size(px(12.)).text_color(fg3).child("Say what you want done below."))
                    .test_support(),
            );
        }
        let last = ls.len().saturating_sub(1);
        for (i, l) in ls.into_iter().enumerate() {
            let shown = if l.text.is_empty() { pending(l.status).unwrap_or("").to_string() } else { l.text.clone() };
            if l.mine {
                let k = if fresh_mine == Some(i) { bubble_k } else { 1.0 };
                msgs = msgs.child(
                    div()
                        .id(("msg", i))
                        .self_end()
                        .relative()
                        .top(px(8. * (1. - k)))
                        .opacity(k)
                        .max_w(relative(0.72))
                        .px(px(12.))
                        .py(px(9.))
                        .rounded(px(radius::R))
                        .bg(mix(pal.raised, pal.info, 0.22))
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .text_color(fg)
                        .aria_label(l.text.clone())
                        .child(shown)
                        .test_support(),
                );
                continue;
            }
            // ponytail: replies draw as plain text; markdown rendering is a later pass.
            let mut say = div()
                .id(("msg", i))
                .max_w(px(560.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .aria_label(if l.text.is_empty() { shown.clone() } else { l.text.clone() })
                .when_live(!shown.is_empty(), |d| d.child(div().text_size(px(13.)).line_height(px(20.)).text_color(if l.text.is_empty() { fg3 } else { fg }).child(shown.clone())));
            if let Some(e) = &l.error {
                let mut err = div()
                    .id(("msg-error", i))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .text_color(c(pal.bad))
                    .aria_label(e.clone())
                    .child(glyph("circle-alert", 13., c(pal.bad)))
                    .child(div().min_w_0().child(e.clone()));
                if i == last {
                    err = err.child(
                        div()
                            .id(("retry", i))
                            .ml(px(4.))
                            .px(px(9.))
                            .py(px(3.))
                            .rounded(px(radius::R2))
                            .border_1()
                            .border_color(c(pal.line))
                            .bg(c(pal.raised))
                            .text_color(fg)
                            .text_size(px(11.5))
                            .cursor_pointer()
                            .aria_label("Retry")
                            .child("Retry")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let (Some(id), Some((_, chats))) = (this.open.clone(), this.core.as_ref()) {
                                    if let Err(e) = chats.retry(&id, ctx()) {
                                        this.send_error = Some(e.to_string());
                                    }
                                }
                                this.refresh();
                                cx.notify();
                            }))
                            .test_support(),
                    );
                }
                say = say.child(err.test_support());
            }
            if let Some(m) = &l.meta {
                say = say.child(div().flex().items_center().gap(px(6.)).text_size(px(12.)).text_color(fg3).font_features(tnum()).child(glyph("terminal", 13., fg3)).child(m.clone()));
            }
            msgs = msgs.child(say.test_support());
        }

        let busy = self.busy();
        // ponytail: reads the prefs file on each render so a change in Settings shows; cache it in
        // the shell if renders get frequent.
        let effort = Prefs::load().cli_effort;
        let effort_label = effort.clone().map_or_else(
            || "Auto".to_string(),
            |e| {
                let mut c = e.chars();
                c.next().map_or_else(|| "Auto".into(), |f| format!("Auto · {}{}", f.to_uppercase(), c.as_str()))
            },
        );
        // The Send and Stop glyphs cross-fade when a reply starts or stops. Only the current one
        // takes a click, so the one fading out cannot send or stop anything.
        let k = self.action_since.map_or(1.0, |t0| crate::motion::run(std::time::Instant::now(), t0, cauldron_chrome::motion::time::SWAP));
        let layer = |name: &'static str, size: f32, a: f32| {
            div().absolute().inset_0().flex().items_center().justify_center().opacity(a).child(glyph(name, size, fg))
        };
        let ((now_name, now_size), (was_name, was_size)) = if busy { (("square", 13.), ("arrow-up", 16.)) } else { (("arrow-up", 16.), ("square", 13.)) };
        let mut action = div()
            .id(if busy { "stop" } else { "send" })
            .relative()
            .aria_label(if busy { "Stop" } else { "Send" })
            .child(layer(now_name, now_size, k));
        if k < 1.0 {
            action = action.child(layer(was_name, was_size, 1.0 - k));
        }
        let action = if busy {
            action.on_click(cx.listener(|this, _, _, cx| {
                if let (Some(id), Some((_, chats))) = (this.open.clone(), this.core.as_ref()) {
                    chats.stop(&id);
                }
                this.refresh();
                cx.notify();
            }))
        } else {
            action.on_click(cx.listener(|this, _, window, cx| this.send_composer(window, cx)))
        };
        let action = action
            .ml_auto()
            .size(px(30.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::R2))
            .bg(c(pal.well))
            .border_1()
            .border_color(c(pal.line))
            .cursor_pointer()
            .test_support();
        let input = self.composer.as_ref().expect("made in render").0.clone();
        let chip_btn = |id: &'static str, label: String| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(5.))
                .px(px(8.))
                .py(px(3.))
                .rounded(px(radius::R1))
                .border_1()
                .border_color(c(pal.line))
                .bg(c(pal.well))
                .text_size(px(11.5))
                .line_height(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(fg2)
                .aria_label(label.clone())
                .child(label)
        };
        let effort_chip = chip_btn("effort", effort_label).cursor_pointer().hover(|d| d.text_color(fg)).on_click(cx.listener(|_, _, _, cx| {
            // Cycles Auto, low, medium, high, max and saves it, as Settings would.
            let mut p = Prefs::load();
            let at = EFFORTS.iter().position(|e| *e == p.cli_effort.as_deref()).unwrap_or(0);
            p.cli_effort = EFFORTS[(at + 1) % EFFORTS.len()].map(str::to_string);
            p.save();
            cx.notify();
        }));
        let mut composer = div()
            .id("composer")
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(12.))
            .py(px(10.))
            .rounded(px(radius::R))
            .border_1()
            .border_color(c(pal.line))
            .bg(c(pal.raised))
            .aria_label("Message the coordinator")
            .child(div()
                    .id("composer-input")
                    // The multi-line editor pads itself by Size::Medium's 10/8px and ignores style
                    // padding; pull it back so the text lines up with the chips under it.
                    .mx(px(-10.))
                    .mt(px(-8.))
                    .text_size(px(13.))
                    .child(Textarea::new(&input).appearance(false)).test_support())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(chip_btn("route", format!("Answered by {}", t.route.provider)).test_support())
                    .child(effort_chip.test_support())
                    .child(action),
            );
        if let Some(e) = &self.send_error {
            composer = composer.child(div().id("send-error").text_size(px(12.)).text_color(c(pal.bad)).aria_label(e.clone()).child(e.clone()).test_support());
        }

        // The stage bar: usage on the left. Tokens per thread are not in core, so cost only.
        let spent = cost(t);
        let usage = div()
            .id("usage")
            .flex()
            .items_center()
            .gap(px(6.))
            .pt(px(8.))
            .px(px(2.))
            .text_size(px(11.))
            .text_color(fg3)
            .font_features(tnum())
            .aria_label(format!("This thread has cost ${spent:.2}"))
            .child("This thread")
            .child(div().font_weight(FontWeight::SEMIBOLD).text_color(fg2).child(format!("${spent:.2}")))
            .test_support();

        div()
            .id("thread")
            .size_full()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(18.))
            .py(px(14.))
            .child(head)
            .child(msgs)
            .child(composer.test_support())
            .child(usage)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Cauldron, Threads};
    use super::lines;
    use backspace_core::chat::{Chats, Status};
    use backspace_core::prefs::Prefs;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{px, size, AppContext, TestAppContext};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn msg(id: &str, role: &str, text: &str, extra: serde_json::Value) -> serde_json::Value {
        let mut m = serde_json::json!({ "id": id, "role": role, "text": text, "at": 1, "status": "done" });
        m.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        m
    }

    /// A data folder with two threads: t1 answered once, t2 empty. Neither route can reach a
    /// model: the CLI they name is not installed, so a send fails fast and touches no network.
    fn data() -> PathBuf {
        let d = std::env::temp_dir().join(format!("cauldron-ui4-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("threads")).unwrap();
        let route = serde_json::json!({ "kind": "cli", "provider": "cauldron-test-missing-cli", "model": null });
        let t1 = serde_json::json!({
            "id": "t1", "title": "Get M3 through review", "created": 1, "updated": 3, "route": route,
            "project": "C:\\code\\Cauldron",
            "messages": [
                msg("m1", "user", "Run the tests", serde_json::json!({})),
                msg("m2", "assistant", "All 591 pass.", serde_json::json!({ "model": "opus", "cost_usd": 0.42 })),
            ]
        });
        let t2 = serde_json::json!({ "id": "t2", "title": "Empty one", "created": 1, "updated": 2, "route": route, "messages": [] });
        for t in [t1, t2] {
            std::fs::write(d.join("threads").join(format!("{}.json", t["id"].as_str().unwrap())), t.to_string()).unwrap();
        }
        d
    }

    /// One temp prefs file for the whole test process, set once and never unset: env vars are
    /// process-wide and tests run in parallel, so no test can render or save against the user's
    /// real prefs.
    fn prefs() -> &'static PathBuf {
        static P: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        P.get_or_init(|| {
            let p = std::env::temp_dir().join(format!("cauldron-ui4-prefs-{}.json", std::process::id()));
            let _ = std::fs::remove_file(&p);
            std::env::set_var("BACKSPACE_PREFS", &p);
            p
        })
    }

    fn open(cx: &mut TestAppContext) -> (gpui_kit::WindowHandle<Cauldron>, Arc<Chats>) {
        cx.update(gpui_kit::init);
        prefs();
        let d = data();
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        let (tx, rx) = async_channel::bounded(1);
        let chats = Chats::open(d, rt.handle().clone(), tx);
        let ch = chats.clone();
        let handle = cx.add_window(move |_, cx| {
            let mut c = Cauldron { threads: Threads::Ready(ch.list()), core: Some((rt, ch)), ..Cauldron::new() };
            c.watch(rx, cx);
            c
        });
        cx.simulate_window_resize(handle.into(), size(px(1240.), px(702.)));
        (handle, chats)
    }

    #[test]
    fn lines_carry_who_answered_and_the_cost() {
        let t: backspace_core::chat::Thread = serde_json::from_value(serde_json::json!({
            "id": "x", "title": "x", "created": 1, "updated": 1,
            "route": { "kind": "cli", "provider": "claude", "model": null },
            "messages": [
                msg("a", "user", "hi", serde_json::json!({})),
                msg("b", "assistant", "", serde_json::json!({ "status": "error", "error": "usage limit", "author_name": "Builder", "model": "opus", "cost_usd": 0.4 })),
            ]
        }))
        .unwrap();
        let l = lines(&t);
        assert!(l[0].mine && l[0].meta.is_none());
        assert_eq!(l[1].status, Status::Error);
        assert_eq!(l[1].meta.as_deref(), Some("Builder · opus · $0.40"));
        assert_eq!(super::cost(&t), 0.4);
        let empty = backspace_core::chat::Thread { messages: Vec::new(), ..t };
        assert_eq!(format!("{:.2}", super::cost(&empty)), "0.00", "no minus zero");
    }

    /// Clicking a row opens its thread: the title on the pinned tab and above the transcript,
    /// each message, who answered at what cost, and the composer. An empty thread says so.
    #[gpui_kit::test]
    fn a_row_opens_its_thread(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("no-thread").label(), Some("No thread open"));
            window.click(("thread", 0usize), cx);
            assert_eq!(window.find("thread-title").label(), Some("Get M3 through review"));
            assert_eq!(window.find(("tab", 0usize)).label(), Some("Get M3 through review"));
            assert_eq!(window.find(("msg", 0usize)).label(), Some("Run the tests"));
            assert_eq!(window.find(("msg", 1usize)).label(), Some("All 591 pass."));
            assert!(window.find(("msg", 0usize)).bounds().origin.x > window.find(("msg", 1usize)).bounds().origin.x, "your message sits right");
            assert_eq!(window.find("usage").label(), Some("This thread has cost $0.42"));
            assert_eq!(window.find("composer").label(), Some("Message the coordinator"));
            assert_eq!(window.find("send").label(), Some("Send"));
            window.click(("thread", 1usize), cx);
            assert_eq!(window.find("thread-empty").label(), Some("No messages yet"));
        })
        .unwrap();
    }

    /// Typing and Enter send to core; the message appears at once, and the reply's failure
    /// (the CLI is not installed) shows inline with Retry.
    #[gpui_kit::test]
    fn the_composer_sends_through_core(cx: &mut TestAppContext) {
        let (handle, chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 1usize), cx);
            window.click("composer-input", cx);
            window.input("Ship", cx);
            window.press("shift-enter", cx);
            window.input("it", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(chats.peek("t2").unwrap().messages.is_empty(), "Shift+Enter adds a line, it does not send");
        cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx)).unwrap();
        cx.run_until_parked();
        assert_eq!(chats.peek("t2").unwrap().messages[0].text, "Ship
it");
        let failed = |cx: &mut TestAppContext| {
            for _ in 0..200 {
                let t = chats.peek("t2").unwrap();
                if t.messages.last().is_some_and(|m| m.status == Status::Error) && !chats.is_busy("t2") {
                    return t.messages.last().unwrap().id.clone();
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
                cx.run_until_parked();
            }
            panic!("the reply never failed");
        };
        let first = failed(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("msg", 0usize)).label(), Some("Ship
it"));
            let err = window.find(("msg-error", 1usize)).label().unwrap_or_default().to_string();
            assert!(err.contains("not installed"), "{err}");
            window.click(("retry", 1usize), cx);
        })
        .unwrap();
        let second = failed(cx);
        assert_ne!(first, second, "Retry replaced the failed reply with a new attempt");
        assert_eq!(chats.peek("t2").unwrap().messages.len(), 2);
    }

    /// The effort chip cycles Auto, low, ... and saves it to prefs (the temp file in tests).
    #[gpui_kit::test]
    fn the_effort_chip_cycles_and_saves(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
            assert_eq!(window.find("effort").label(), Some("Auto"));
            window.click("effort", cx);
            window.render_frame(cx);
            assert_eq!(window.find("effort").label(), Some("Auto · Low"));
        })
        .unwrap();
        assert_eq!(Prefs::load().cli_effort.as_deref(), Some("low"));
        let saved = std::fs::read_to_string(prefs()).unwrap();
        assert!(saved.contains("low"), "{saved}");
    }
}
