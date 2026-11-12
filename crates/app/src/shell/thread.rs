//! The thread view (UI4): the open thread's transcript, the composer that sends to core, and the
//! stage bar's usage. Rows open here from the sidebar; core's change channel keeps it current.
//!
//! Left out until core has them per thread (docs/cauldron/UI3-CORE-MAP.md): ask cards, the stage
//! stepper, teammates' messages and avatars.

use super::*;
use backspace_core::chat::{Ctx, Msg, Role, Status, Thread};
use gpui_kit::component::input::{Textarea, TextareaState};
use cauldron_chrome::avatar::{self, bust_frame, Look, Mood};
use gpui_kit::prelude::FluentBuilder;

/// One transcript entry as drawn.
#[derive(Debug, PartialEq)]
pub(super) struct Line {
    pub id: String,
    pub mine: bool,
    pub text: String,
    pub status: Status,
    pub error: Option<String>,
    /// Who answered, with which model and at what cost, under a reply.
    pub meta: Option<String>,
}

pub(super) fn lines(t: &Thread) -> Vec<Line> {
    t.messages.iter().map(|m| Line { id: m.id.clone(), mine: m.role == Role::User, text: m.text.clone(), status: m.status, error: m.error.clone(), meta: meta(m) }).collect()
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

/// How a reply was made, from its trace: one plain line per kind of tool call, in the order each
/// kind first happened ("Read 12 files", "Made 3 edits"). Core does not time the thinking, so
/// "Thought" carries no duration.
pub(super) fn steps(t: &backspace_core::trace::Trace, thought: bool) -> Vec<String> {
    #[derive(PartialEq)]
    enum K {
        Read,
        Edit,
        Run,
        Web,
        Code,
        Agent,
        Plan,
        Other(String),
    }
    // (kind, calls, failed, distinct files)
    let mut ks: Vec<(K, usize, usize, std::collections::BTreeSet<String>)> = Vec::new();
    for sp in t.spans.iter().filter(|s| s.kind == "tool") {
        let k = match sp.name.as_str() {
            "Read" => K::Read,
            "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => K::Edit,
            "Bash" | "PowerShell" => K::Run,
            "WebSearch" | "WebFetch" => K::Web,
            "Grep" | "Glob" | "LS" => K::Code,
            "Task" | "Agent" => K::Agent,
            "TodoWrite" => K::Plan,
            n => K::Other(n.to_string()),
        };
        let i = match ks.iter().position(|e| e.0 == k) {
            Some(i) => i,
            None => {
                ks.push((k, 0, 0, Default::default()));
                ks.len() - 1
            }
        };
        let e = &mut ks[i];
        e.1 += 1;
        e.2 += usize::from(sp.error.is_some());
        let file = serde_json::from_str::<serde_json::Value>(&sp.input).ok().and_then(|v| v.get("file_path").or_else(|| v.get("path")).and_then(|p| p.as_str()).map(str::to_string));
        e.3.insert(file.unwrap_or_else(|| format!("#{}", e.1)));
    }
    let n = |k: usize, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
    let mut out: Vec<String> = thought.then(|| "Thought".to_string()).into_iter().collect();
    for (k, calls, failed, files) in ks {
        let line = match k {
            K::Read => format!("Read {}", n(files.len(), "file", "files")),
            K::Edit => format!("Made {}", n(calls, "edit", "edits")),
            K::Run => format!("Ran {}", n(calls, "command", "commands")),
            K::Web => "Searched the web".into(),
            K::Code => "Searched the code".into(),
            K::Agent => format!("Ran {}", n(calls, "subagent", "subagents")),
            K::Plan => "Updated the plan".into(),
            // MCP tools arrive as mcp__server__tool: say "Used linear: get_issue".
            K::Other(name) => match name.strip_prefix("mcp__").and_then(|r| r.split_once("__")) {
                Some((server, tool)) => format!("Used {server}: {tool}"),
                None => format!("Used {name}"),
            },
        };
        out.push(if failed > 0 { format!("{line}, {failed} failed") } else { line });
    }
    out
}

/// What a reply that is not finished says in place of its text.
fn pending(s: Status) -> Option<&'static str> {
    match s {
        Status::Stopped => Some("Stopped"),
        s => live(s).map(|(_, label)| label),
    }
}

/// A reply on its way: the agent's mood and what its status line says.
fn live(s: Status) -> Option<(Mood, &'static str)> {
    match s {
        Status::Sending => Some((Mood::Waiting, "Waiting for the reply")),
        Status::Streaming => Some((Mood::Typing, "Writing")),
        _ => None,
    }
}

/// The agent's bust in a 16px slot, then its label: the same slot and 8px gap as the step
/// lines, so the label starts where their text does. It stays under the text while the reply
/// streams. The head and marks overhang the slot, so the image sits absolutely above it.
fn status_line(i: usize, mood: Mood, label: &'static str, grey: Rgb) -> impl IntoElement {
    // ponytail: a still frame (ms None); the cog spin and typing hop come with the motion clock.
    let look = Look { px: 16., outline: true, ms: None, bust: true, ink: Some(grey) };
    let (w, h, top) = bust_frame(16.);
    let pic = img(Arc::new(Image::from_bytes(ImageFormat::Svg, avatar::svg(0, mood, look).into_bytes())));
    div()
        .id(("live", i))
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(20.))
        .text_size(px(12.5))
        .text_color(c(grey))
        .aria_label(label)
        .child(div().id(("bust", i)).relative().size(px(16.)).flex_none().child(pic.absolute().left_0().top(px(-top)).w(px(w)).h(px(h))).test_support())
        .child(div().id(("live-text", i)).child(label).test_support())
        .test_support()
}

/// The settings a send carries: the saved prefs, nothing else wired yet.
fn ctx() -> Ctx {
    Ctx { prefs: Prefs::load(), memory: None, agents: None, plan: false }
}

/// Efforts the chip cycles through; None is Auto, the CLI's own default.
pub(super) const EFFORTS: [Option<&str>; 5] = [None, Some("low"), Some("medium"), Some("high"), Some("max")];

/// `a` mixed toward `b` by `t`, as CSS `color-mix(in srgb, b t, a)`.
/// An overlay `a` mixes its alpha toward `b`'s full opacity too.
pub(super) fn mix(a: impl Into<Rgba>, b: Rgb, t: f32) -> Hsla {
    let Rgba(a, al) = a.into();
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    c(Rgba(Rgb(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2)), al + (1.0 - al) * t))
}

/// Each finished reply's steps, read from its trace once (render never touches the disk).
/// Adds step lines for replies not yet in `map`. A saved trace never changes, so each one is
/// read once per thread load, not on every core poke.
fn fill_steps(chats: &Chats, t: Option<&Thread>, map: &mut std::collections::HashMap<String, Vec<String>>) {
    for m in t.into_iter().flat_map(|t| t.messages.iter()).filter(|m| m.role != Role::User) {
        if map.contains_key(&m.id) {
            continue;
        }
        if let Some(tr) = m.trace.as_ref().and_then(|id| chats.trace(id)) {
            map.insert(m.id.clone(), steps(&tr, !m.thinking.is_empty()));
        }
    }
}

impl Cauldron {
    /// Opens a thread in the main card. Reading it through core marks it read.
    pub(super) fn open_thread(&mut self, id: String) {
        let Some((_, chats)) = &self.core else { return };
        self.thread = chats.thread(&id);
        self.steps.clear();
        fill_steps(chats, self.thread.as_ref(), &mut self.steps);
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
                fill_steps(chats, self.thread.as_ref(), &mut self.steps);
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
    pub(super) fn card_body(&self, pal: &'static Palette, window: &Window, cx: &mut Context<Self>) -> AnyElement {
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
            (None, _) => self.new_thread_view(pal, window, cx),
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
                .rounded(px(radius::CONTROL))
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
            .child(chip("Code".into(), pal.fg2));
        if let Some(p) = &t.project {
            let name = p.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(p).to_string();
            head = head.child(chip(name, pal.fg2));
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
                        .rounded(px(radius::PANEL))
                        .bg(c(pal.raised))
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
                .when_some(self.steps.get(&l.id).filter(|s| !s.is_empty()), |d, st| {
                    let mut col = div().id(("steps", i)).flex().flex_col().gap(px(2.)).aria_label(st.join(", "));
                    for (j, s) in st.iter().enumerate() {
                        col = col.child(div().flex().items_center().gap(px(8.)).h(px(20.)).text_size(px(12.5)).text_color(fg3).child(div().size(px(16.)).flex_none().flex().items_center().justify_center().child(glyph("check", 13., fg3))).child(div().id(format!("step-text-{i}-{j}")).child(s.clone()).test_support()));
                    }
                    d.child(col.test_support())
                })
                .when_live(!l.text.is_empty() || live(l.status).is_none(), |d| d.child(div().text_size(px(13.)).line_height(px(20.)).text_color(if l.text.is_empty() { fg3 } else { fg }).child(shown.clone())))
                .when_some(live(l.status), |d, (mood, label)| d.child(status_line(i, mood, label, pal.fg3)));
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
                            .rounded(px(radius::INNER))
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
            .rounded(px(radius::INNER))
            .bg(c(pal.inset))
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
                .rounded(px(radius::CONTROL))
                .border_1()
                .border_color(c(pal.line))
                .bg(c(pal.inset))
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
            .rounded(px(radius::PANEL))
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
pub(super) mod tests {
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
                msg("m2", "assistant", "All 591 pass.", serde_json::json!({ "model": "opus", "cost_usd": 0.42, "thinking": "Check the suite.", "trace": "00ff00ff" })),
            ]
        });
        let t2 = serde_json::json!({ "id": "t2", "title": "Empty one", "created": 1, "updated": 2, "route": route, "messages": [] });
        std::fs::create_dir_all(d.join("traces")).unwrap();
        let tool = |name: &str, input: &str| serde_json::json!({ "span_id": "s", "name": name, "kind": "tool", "start": 1, "input": input });
        let trace = serde_json::json!({ "trace_id": "00ff00ff", "thread": "t1", "message": "m2", "spans": [
            { "span_id": "r", "name": "reply", "kind": "reply", "start": 1 },
            tool("Read", r#"{"file_path":"a.rs"}"#), tool("Bash", r#"{"command":"cargo test"}"#),
        ]});
        std::fs::write(d.join("traces").join("00ff00ff.json"), trace.to_string()).unwrap();
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

    pub(in crate::shell) fn open(cx: &mut TestAppContext) -> (gpui_kit::WindowHandle<Cauldron>, Arc<Chats>) {
        cx.update(gpui_kit::init);
        prefs();
        let d = data();
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        // No change channel: a wake from core's tokio thread into the test scheduler makes the run
        // non-deterministic (and panics). Tests call `refresh` where the app would be poked.
        let (tx, _) = async_channel::bounded(1);
        let chats = Chats::open(d, rt.handle().clone(), tx);
        let ch = chats.clone();
        cx.update(|cx| cx.set_reduce_motion(true));
        let handle = cx.add_window(move |_, _| Cauldron { threads: Threads::Ready(ch.list()), core: Some((rt, ch)), ..Cauldron::new() });
        cx.simulate_window_resize(handle.into(), size(px(1240.), px(702.)));
        (handle, chats)
    }

    fn span(name: &str, input: &str, error: bool) -> backspace_core::trace::Span {
        backspace_core::trace::Span { name: name.into(), kind: "tool".into(), input: input.into(), error: error.then(|| "failed".into()), ..Default::default() }
    }

    /// A reply's tool calls read as a few plain lines, in the order they first happened: files
    /// counted once, failures said, unknown tools named.
    #[test]
    fn steps_group_a_replys_tool_calls() {
        let root = backspace_core::trace::Span { name: "reply".into(), kind: "reply".into(), ..Default::default() };
        let t = backspace_core::trace::Trace {
            spans: vec![
                root,
                span("Read", r#"{"file_path":"a.rs"}"#, false),
                span("Bash", r#"{"command":"cargo test"}"#, false),
                span("Read", r#"{"file_path":"b.rs"}"#, false),
                span("Read", r#"{"file_path":"a.rs"}"#, false),
                span("Edit", r#"{"file_path":"a.rs"}"#, false),
                span("Write", r#"{"file_path":"c.rs"}"#, false),
                span("Edit", r#"{"file_path":"a.rs"}"#, false),
                span("Bash", r#"{"command":"cargo build"}"#, true),
                span("WebSearch", r#"{"query":"gpui"}"#, false),
                span("Grep", r#"{"pattern":"x"}"#, false),
                span("mcp__linear__get_issue", "{}", false),
            ],
            ..Default::default()
        };
        assert_eq!(super::steps(&t, true), ["Thought", "Read 2 files", "Ran 2 commands, 1 failed", "Made 3 edits", "Searched the web", "Searched the code", "Used linear: get_issue"]);
        let one = backspace_core::trace::Trace { spans: vec![t.spans[0].clone(), span("Read", r#"{"file_path":"a.rs"}"#, false), span("Bash", "{}", false)], ..Default::default() };
        assert_eq!(super::steps(&one, false), ["Read 1 file", "Ran 1 command"]);
        assert!(super::steps(&backspace_core::trace::Trace::default(), false).is_empty());
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
            assert!(window.try_find("nt-box").is_some(), "no thread open: the New thread screen");
            window.click(("thread", 0usize), cx);
            assert_eq!(window.find("thread-title").label(), Some("Get M3 through review"));
            assert_eq!(window.find(("tab", 0usize)).label(), Some("Get M3 through review"));
            assert_eq!(window.find(("msg", 0usize)).label(), Some("Run the tests"));
            assert_eq!(window.find(("msg", 1usize)).label(), Some("All 591 pass."));
            assert_eq!(window.find(("steps", 1usize)).label(), Some("Thought, Read 1 file, Ran 1 command"), "how the reply was made, from its trace");
            assert!(window.find(("msg", 0usize)).bounds().origin.x > window.find(("msg", 1usize)).bounds().origin.x, "your message sits right");
            assert_eq!(window.find("usage").label(), Some("This thread has cost $0.42"));
            assert_eq!(window.find("composer").label(), Some("Message the coordinator"));
            assert_eq!(window.find("send").label(), Some("Send"));
            window.click(("thread", 1usize), cx);
            assert_eq!(window.find("thread-empty").label(), Some("No messages yet"));
        })
        .unwrap();
    }

    /// A reply still on its way shows the agent's bust and what it is doing, in the step lines'
    /// grey with its label lined up on the step text, under any text so far; a finished reply has none.
    #[gpui_kit::test]
    fn a_running_reply_shows_the_bust_on_its_status_line(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
        })
        .unwrap();
        for (status, label, text) in [(Status::Streaming, "Writing", ""), (Status::Sending, "Waiting for the reply", ""), (Status::Streaming, "Writing", "Half a sen")] {
            handle
                .update(cx, |c, _, _| {
                    let t = c.thread.as_mut().unwrap();
                    t.messages.truncate(2);
                    let mut m = t.messages[1].clone();
                    m.id = "m3".into();
                    m.text = text.into();
                    m.status = status;
                    t.messages.push(m);
                })
                .unwrap();
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert_eq!(window.find(("live", 2usize)).label(), Some(label));
                let bust = window.find(("bust", 2usize)).bounds();
                assert_eq!((bust.size.width, bust.size.height), (px(16.), px(16.)), "a 16px square slot");
                let tx = window.find(("live-text", 2usize)).bounds().origin.x;
                let step = window.find("step-text-1-0").bounds().origin.x;
                assert_eq!(tx, step, "the label lines up with the step text");
                assert!(window.try_find(("bust", 1usize)).is_none(), "a finished reply has no status line");
                if !text.is_empty() {
                    assert_eq!(window.find(("msg", 2usize)).label(), Some(text), "the text shows above the status line while it streams");
                }
            })
            .unwrap();
        }
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
                    handle.update(cx, |c, _, _| c.refresh()).unwrap();
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
