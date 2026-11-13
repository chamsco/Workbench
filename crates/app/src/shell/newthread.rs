//! The New thread screen, as the new-look board's frame 1: the machine above a box centred in
//! the card, and in the box the editor, files, effort and Start. Start creates the thread in
//! core, attaches the files, sends the text and opens it. On a connected machine Start sends the
//! text as a task to that machine's harness (`/v1/task`); it does not open a local thread.
//!
//! Sizes are measured from design/cauldron-new-look.html (2096c8f) at 1200x720.

use super::thread::{mix, EFFORTS};
use super::*;
use backspace_core::chat::{Route, RouteKind, Scope};
use backspace_core::prefs::MachineCfg;
use backspace_core::remote::{Link, Remote};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder;
use std::path::PathBuf;

/// The board's column (`.w`): the ctx row and the box.
const WRAP_W: f32 = 780.0;
/// The box with one line of text (`.box`).
const BOX_H: f32 = 84.0;
/// The ctx row above the box (`.ctx`): the machine on the left.
const CTX_H: f32 = 34.0;

#[derive(Default)]
pub(crate) struct Start {
    pub(crate) input: Option<(Entity<TextareaState>, Subscription)>,
    /// Who answers a new thread here: prefs' default route, else Claude Code if installed.
    /// Resolved when core opens; tests set it so nothing reaches a real model.
    pub(crate) route: Option<Route>,
    pub(crate) machines: Vec<MachineCfg>,
    /// 0 is this machine; i is `machines[i - 1]`.
    pub(crate) machine: usize,
    pub(crate) remote: Option<Arc<Remote>>,
    remote_watch: Option<Task<()>>,
    pub(crate) menu: bool,
    slide: super::motion::Slide,
    pub(crate) files: Vec<PathBuf>,
    pub(crate) error: Option<String>,
    /// What a connected machine answered, e.g. "Sent to Studio as T-7".
    pub(crate) note: Option<String>,
    pub(crate) sending: bool,
}

/// The route a new thread gets: the default chat model, else Claude Code if it is installed.
pub(super) fn default_route(p: &Prefs) -> Option<Route> {
    p.default_route.clone().or_else(|| backspace_core::harnesses::which("claude").map(|_| Route { kind: RouteKind::Cli, provider: "claude".into(), model: None }))
}

/// The body's inner line: the accent at 70% over the line while the editor has focus.
pub(super) fn body_line(pal: &Palette, focused: bool) -> Hsla {
    if focused {
        mix(pal.line, pal.accent, 0.7)
    } else {
        c(pal.line)
    }
}

const PLACEHOLDER: &str = "Describe a task, a bug or an idea…";

fn mime(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("txt" | "md" | "rs" | "ts" | "js" | "py" | "json" | "toml" | "csv") => "text/plain",
        _ => "application/octet-stream",
    }
}

impl Cauldron {
    /// Back to the New thread screen with an empty box.
    pub(super) fn new_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = None;
        self.thread = None;
        self.selected = None;
        self.coordinator = None;
        self.agents.clear();
        self.idle.clear();
        self.active = 0;
        self.start.error = None;
        self.start.note = None;
        if let Some((st, _)) = &self.start.input {
            st.update(cx, |s, cx| s.set_value("", window, cx));
            let f = st.read(cx).focus_handle(cx);
            window.focus(&f, cx);
        }
        cx.notify();
    }

    pub(super) fn ensure_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.start.input.is_some() {
            return;
        }
        let st = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 10).submit_on_enter(true).placeholder(PLACEHOLDER));
        let sub = cx.subscribe_in(&st, window, |this, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { shift: false, .. } => this.start_thread(window, cx),
            InputEvent::Focus | InputEvent::Blur => cx.notify(),
            _ => {}
        });
        self.start.input = Some((st, sub));
    }

    /// Why Start cannot run now, if it cannot.
    fn start_blocked(&self) -> Option<String> {
        if self.start.sending {
            return Some("Sending".into());
        }
        if self.start.machine > 0 {
            let name = self.start.machines.get(self.start.machine - 1).map_or("", |m| m.name.as_str());
            return match self.start.remote.as_ref().map(|r| r.link()) {
                Some(Link::Online) => None,
                Some(Link::Offline { error }) => Some(format!("Can't reach {name}: {error}")),
                _ => Some(format!("Connecting to {name}")),
            };
        }
        match (&self.threads, &self.core) {
            (Threads::Error(e), _) => Some(format!("Can't open your threads: {e}")),
            (_, None) => Some("Opening your threads".into()),
            _ if self.start.route.is_none() => Some("Connect a subscription to start a thread.".into()),
            _ => None,
        }
    }

    /// Start: a new thread here (create, attach, send, open), or a task on the chosen machine.
    pub(super) fn start_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.start_blocked().is_some() {
            return;
        }
        let Some((st, _)) = &self.start.input else { return };
        let st = st.clone();
        let text = st.read(cx).value().trim().to_string();
        if text.is_empty() && self.start.files.is_empty() {
            return;
        }
        self.start.error = None;
        self.start.note = None;
        if self.start.machine > 0 {
            let (Some(r), Some(m)) = (self.start.remote.clone(), self.start.machines.get(self.start.machine - 1)) else { return };
            let name = m.name.clone();
            self.start.sending = true;
            cx.spawn_in(window, async move |this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                // post_wait blocks its thread; keep it off the window's.
                let res = cx.background_executor().spawn(async move { r.post_wait("/v1/task", serde_json::json!({ "text": text })) }).await;
                let _ = this.update_in(cx, |c, window, cx| {
                    c.start.sending = false;
                    match res {
                        Ok(v) => {
                            let key = v["key"].as_str().unwrap_or("a task").to_string();
                            c.start.note = Some(format!("Sent to {name} as {key}"));
                            st.update(cx, |s, cx| s.set_value("", window, cx));
                        }
                        Err(e) => c.start.error = Some(e.to_string()),
                    }
                    cx.notify();
                });
            })
            .detach();
            cx.notify();
            return;
        }
        let (Some((_, chats)), Some(route)) = (self.core.as_ref(), self.start.route.clone()) else { return };
        let chats = chats.clone();
        // Read every file first, so a bad one leaves no empty thread behind.
        let mut files = Vec::new();
        for p in &self.start.files {
            let name = p.file_name().map_or_else(|| "file".into(), |n| n.to_string_lossy().to_string());
            match std::fs::read(p) {
                Ok(b) => files.push((name, b)),
                Err(e) => {
                    self.start.error = Some(format!("Couldn't read {name}: {e}"));
                    cx.notify();
                    return;
                }
            }
        }
        let t = chats.create_in(route, Scope::default());
        let sent = (|| {
            use base64::Engine;
            let atts = files.iter().map(|(name, b)| chats.attach(&t.id, name, mime(name), &base64::engine::general_purpose::STANDARD.encode(b))).collect::<anyhow::Result<Vec<_>>>()?;
            chats.send(&t.id, &text, atts, None, ctx_now())
        })();
        if let Err(e) = sent {
            let _ = chats.delete(&t.id);
            self.start.error = Some(e.to_string());
            self.refresh();
            cx.notify();
            return;
        }
        self.start.files.clear();
        st.update(cx, |s, cx| s.set_value("", window, cx));
        self.open_thread(t.id);
        cx.notify();
    }

    /// Picks the machine Start goes to; a connected one is reached over its harness API.
    pub(super) fn pick_machine(&mut self, i: usize, cx: &mut Context<Self>) {
        self.start.menu = false;
        self.start.machine = i;
        self.start.error = None;
        self.start.note = None;
        self.start.remote = None;
        self.start.remote_watch = None;
        if i == 0 {
            cx.notify();
            return;
        }
        let (Some(m), Some((rt, _))) = (self.start.machines.get(i - 1).cloned(), self.core.as_ref()) else {
            cx.notify();
            return;
        };
        let (tx, rx) = async_channel::bounded(1);
        self.start.remote = Some(Remote::connect(m.name, m.url, m.token, rt.handle().clone(), tx));
        // Tests render explicitly: a wake from the tokio thread would trip the test scheduler.
        if cfg!(test) {
            return;
        }
        self.start.remote_watch = Some(cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while rx.recv().await.is_ok() {
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn pick_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some("Attach".into()) });
        cx.spawn_in(window, async move |this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
            if let Ok(Ok(Some(ps))) = paths.await {
                let _ = this.update(cx, |c, cx| {
                    c.start.files.extend(ps);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(super) fn new_thread_view(&self, pal: &'static Palette, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let blocked = self.start_blocked();
        let input = self.start.input.as_ref().expect("made in render").0.clone();
        let focused = input.read(cx).focus_handle(cx).is_focused(window);

        // A dropdown-looking control as on the board (`.dd`): 26px, radius 6, the label, then a chevron.
        let dd = |id: &'static str| {
            div()
                .id(id)
                .h(px(26.))
                .flex()
                .items_center()
                .gap(px(4.))
                .px(px(8.))
                .rounded(px(radius::TAG))
                .text_size(px(12.))
                .text_color(fg2)
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)).text_color(fg))
        };
        let effort = Prefs::load().cli_effort;
        let effort_label = match effort.as_deref() {
            None => "Auto".to_string(),
            Some(e) => {
                let mut ch = e.chars();
                ch.next().map_or_else(|| "Auto".into(), |f| format!("Auto · {}{}", f.to_uppercase(), ch.as_str()))
            }
        };
        let machine_name = if self.start.machine == 0 { "This machine".to_string() } else { self.start.machines.get(self.start.machine - 1).map_or_else(|| "This machine".into(), |m| m.name.clone()) };
        let ready = blocked.is_none() && !input.read(cx).value().trim().is_empty();
        let start_btn = div()
            .id("start")
            .size(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(if ready { c(pal.accent) } else { c(pal.raised) })
            .aria_label("Start")
            .when_some(blocked.clone(), |d, why| d.aria_description(why).opacity(0.45))
            .when(blocked.is_none(), |d| d.cursor_pointer())
            .child(glyph("arrow-up", 14., if ready { c(pal.on_accent) } else { fg2 }))
            .on_click(cx.listener(|this, _, window, cx| this.start_thread(window, cx)))
            .test_support();
        let tools = div()
            .flex()
            .items_center()
            .gap(px(4.))
            .child(
                dd("add-file")
                    .w(px(29.))
                    .justify_center()
                    .px_0()
                    .aria_label("Add a file")
                    .child(glyph("plus", 15., fg2))
                    .on_click(cx.listener(|this, _, window, cx| this.pick_files(window, cx)))
                    .test_support(),
            )
            .child(div().flex_1())
            .child(
                dd("nt-effort")
                    .aria_label(effort_label.clone())
                    .child(effort_label)
                    .child(glyph("chevron-down", 13., fg3))
                    .on_click(cx.listener(|_, _, _, cx| {
                        let mut p = Prefs::load();
                        let at = EFFORTS.iter().position(|e| *e == p.cli_effort.as_deref()).unwrap_or(0);
                        p.cli_effort = EFFORTS[(at + 1) % EFFORTS.len()].map(str::to_string);
                        p.save();
                        cx.notify();
                    }))
                    .test_support(),
            )
            .child(start_btn);

        let mut nt_box = div()
            .id("nt-box")
            .flex()
            .flex_col()
            .min_h(px(BOX_H))
            .pt(px(4.))
            .px(px(15.))
            .pb(px(10.))
            .bg(c(pal.panel))
            .rounded(px(radius::CONTROL))
            .border_1()
            .border_color(if focused { body_line(pal, true) } else { c(pal.line_strong) })
            .shadow(vec![drop(tokens::shadow::BOX)])
            .child(div().id("nt-input").mx(px(-10.)).text_size(px(13.)).child(Textarea::new(&input).appearance(false)).test_support());
        if !self.start.files.is_empty() {
            let mut row = div().flex().flex_wrap().gap(px(6.)).pb(px(6.));
            for (i, p) in self.start.files.iter().enumerate() {
                let name = p.file_name().map_or_else(String::new, |n| n.to_string_lossy().to_string());
                row = row.child(
                    div()
                        .id(("file", i))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .px(px(8.))
                        .py(px(3.))
                        .rounded(px(radius::TAG))
                        .bg(c(pal.inset))
                        .text_size(px(12.))
                        .text_color(fg2)
                        .aria_label(name.clone())
                        .child(name)
                        .child(
                            div()
                                .id(("file-x", i))
                                .cursor_pointer()
                                .aria_label("Remove")
                                .child(glyph("x", 12., fg3))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.start.files.remove(i);
                                    cx.notify();
                                }))
                                .test_support(),
                        )
                        .test_support(),
                );
            }
            nt_box = nt_box.child(row);
        }
        let nt_box = nt_box.child(div().mt_auto().child(tools)).test_support();

        let ctx = div()
            .id("nt-ctx")
            .relative()
            .h(px(CTX_H))
            .flex()
            .items_center()
            .child(
                dd("machine")
                    .px(px(6.))
                    .text_color(fg3)
                    .aria_label(machine_name.clone())
                    .aria_expanded(self.start.menu)
                    .child(glyph("laptop", 13., fg3))
                    .child(machine_name)
                    .child(glyph("chevron-down", 12., fg3))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.start.menu = !this.start.menu;
                        this.start.slide.reset();
                        cx.notify();
                    }))
                    .test_support(),
            )
            // Deferred: the box comes after this row and would paint over the menu.
            .when(self.start.menu, |d| d.child(deferred(div().absolute().top(px(CTX_H - 4.)).left_0().child(self.machine_menu(pal, cx))).with_priority(2)))
            .test_support();

        // What the box says under it: why Start waits, a refusal, or a machine's answer.
        let line = |id: &'static str, text: String, col: Hsla| div().id(id).mt(px(10.)).px(px(6.)).text_size(px(12.)).line_height(px(17.)).text_color(col).aria_label(text.clone()).child(text).test_support();
        let mut wrap = div().w_full().max_w(px(WRAP_W)).flex().flex_col().child(ctx).child(nt_box);
        if let Some(e) = &self.start.error {
            wrap = wrap.child(line("start-error", e.clone(), c(pal.bad)));
        } else if let Some(n) = &self.start.note {
            wrap = wrap.child(line("start-note", n.clone(), c(pal.ok)));
        } else if let Some(why) = blocked.filter(|w| w != "Sending") {
            let bad = why.starts_with("Can't");
            wrap = wrap.child(line("start-why", why, if bad { c(pal.bad) } else { fg3 }));
        }

        // ponytail: the board's trust, Coordinator, mic, Worktree/Isolated, usage and stage
        // controls need core (UI-PLAN decision 5); they come when core has them, not as fakes.
        let screen = div()
            .id("new-thread-screen")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px(px(24.))
            .pb(px(40.))
            .child(wrap)
            .test_support();
        screen.into_any_element()
    }

    fn machine_menu(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mut menu = div()
            .id("machine-menu")
            .w(px(220.))
            .p(px(4.))
            .rounded(px(radius::CONTROL))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .shadow(vec![drop(tokens::shadow::MENU)])
            .relative()
            .text_size(px(12.))
            .aria_label("Machine")
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.start.menu = false;
                cx.notify();
            }))
            .test_support()
            .child(self.start.slide.measure(None))
            .children(self.start.slide.highlight(c(pal.inset), radius::INNER));
        let names = std::iter::once("This machine".to_string()).chain(self.start.machines.iter().map(|m| m.name.clone()));
        for (i, n) in names.enumerate() {
            let on = i == self.start.machine;
            menu = menu.child(
                div()
                    .id(("machine-item", i))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(8.))
                    .py(px(6.))
                    .relative()
                    .rounded(px(radius::INNER))
                    .cursor_pointer()
                    .text_color(if on { fg } else { fg2 })
                    .when(on, |d| d.bg(c(pal.inset)))
                    .child(self.start.slide.measure(Some(i)))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if this.start.slide.hover(i, *h) {
                            cx.notify();
                        }
                    }))
                    .aria_label(n.clone())
                    .aria_selected(on)
                    .child(glyph(if i == 0 { "terminal" } else { "monitor" }, 13., fg3))
                    .child(n)
                    .on_click(cx.listener(move |this, _, _, cx| this.pick_machine(i, cx)))
                    .test_support()
                    .with_animation(("in", i), super::motion::staggered(i), |d, t| d.opacity(t)),
            );
        }
        let n = self.start.machines.len() + 1;
        if self.start.machines.is_empty() {
            menu = menu.child(div().px(px(8.)).py(px(6.)).text_color(fg3).child("No other machines yet. Add one in Settings.").with_animation("in-hint", super::motion::staggered(n), |d, t| d.opacity(t)));
        } else {
            menu = menu.child(div().px(px(8.)).pt(px(6.)).pb(px(4.)).text_size(px(11.5)).text_color(fg3).child("Add machines in Settings.").with_animation("in-hint", super::motion::staggered(n), |d, t| d.opacity(t)));
        }
        super::motion::pop_in(div(), menu, "menu-in", 0.).into_any_element()
    }
}

fn ctx_now() -> backspace_core::chat::Ctx {
    backspace_core::chat::Ctx { prefs: Prefs::load(), memory: None, agents: None, plan: false }
}

#[cfg(test)]
mod tests {
    use super::super::thread::tests::open;
    use super::super::Cauldron;
    use super::body_line;
    use backspace_core::chat::{Route, RouteKind};
    use backspace_core::prefs::MachineCfg;
    use cauldron_chrome::tokens::DARK;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, Window};

    fn missing() -> Option<Route> {
        Some(Route { kind: RouteKind::Cli, provider: "cauldron-test-missing-cli".into(), model: None })
    }

    fn rect(window: &Window, id: impl Into<gpui_kit::ElementId>) -> (f32, f32, f32, f32) {
        let b = window.find(id).bounds();
        (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height))
    }

    /// With no thread open the card is the New thread screen, as the new-look board's frame 1:
    /// the machine above a 780px box centred in the card, the box's controls inside it, and
    /// nothing else (no brand, no Code/Design tabs, no cards).
    #[gpui_kit::test]
    fn the_new_thread_screen_is_the_boards_centred_box(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        handle.update(cx, |c, _, _| c.start.route = missing()).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            for gone in ["nt-brand", "kinds", "nt-cards", "add-machine"] {
                assert!(window.try_find(gone).is_none(), "{gone} is not on the board");
            }
            let (bx, by, bw, bh) = rect(window, "nt-box");
            let (cx0, cy0, cw, ch) = rect(window, "new-thread-screen");
            assert_eq!(bw, 780f32.min(cw - 48.0), "the board's 780, or the card's inside less its padding");
            assert_eq!(bh, 84.0, "the board's box with one line");
            assert!(((bx + bw / 2.0) - (cx0 + cw / 2.0)).abs() <= 1.0, "centred across the card");
            assert!(((by + bh / 2.0) - (cy0 + ch / 2.0)).abs() <= 48.0, "about the middle of the card");
            let (mx, my, _, mh) = rect(window, "machine");
            assert_eq!(mx, bx, "the machine sits on the box's left edge");
            assert!(my + mh <= by && by - (my + mh) <= 6.0, "just above the box");
            for (id, label) in [("add-file", "Add a file"), ("nt-effort", "Auto"), ("machine", "This machine"), ("start", "Start")] {
                assert_eq!(window.find(id).label(), Some(label), "{id}");
            }
            let (sx, sy, sw, sh) = rect(window, "start");
            assert_eq!((sw, sh), (28.0, 28.0));
            assert!(sx + sw <= bx + bw - 12.0 && sy + sh <= by + bh - 8.0, "Start sits in the box's bottom right");
            assert!(window.try_find("start-why").is_none(), "nothing in the way");
        })
        .unwrap();
    }

    /// Start makes the thread in core, attaches the file, sends the text and opens it; New thread
    /// brings the empty box back with the editor focused.
    #[gpui_kit::test]
    fn start_creates_sends_and_opens_the_thread(cx: &mut TestAppContext) {
        let (handle, chats) = open(cx);
        let f = std::env::temp_dir().join(format!("cauldron-ui5-{}.txt", std::process::id()));
        std::fs::write(&f, "notes").unwrap();
        handle
            .update(cx, |c, _, _| {
                c.start.route = missing();
                c.start.files.push(f.clone());
            })
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find(("file", 0usize)).label().unwrap().starts_with("cauldron-ui5-"));
            window.click("nt-input", cx);
            window.input("Fix the build", cx);
            window.click("start", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let list = chats.list();
        assert_eq!(list.len(), 3, "one thread more");
        let id = handle.update(cx, |c, _, _| c.open.clone()).unwrap().expect("opened");
        let t = chats.peek(&id).unwrap();
        assert_eq!(t.messages[0].text, "Fix the build");
        assert_eq!(t.messages[0].attachments.len(), 1);
        assert_eq!(t.route.provider, "cauldron-test-missing-cli");
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("nt-box").is_none(), "the thread replaced the screen");
            assert!(window.try_find("thread-title").is_some(), "the thread view replaced it");
            window.click("new-thread", cx);
            window.render_frame(cx);
            assert!(window.try_find("nt-box").is_some(), "New thread brings it back");
            assert!(window.try_find(("file", 0usize)).is_none(), "files went with the thread");
        })
        .unwrap();
    }

    /// A file that cannot be read stops Start before core makes anything: no empty thread is left.
    #[gpui_kit::test]
    fn an_unreadable_file_leaves_no_thread_behind(cx: &mut TestAppContext) {
        let (handle, chats) = open(cx);
        handle
            .update(cx, |c, _, _| {
                c.start.route = missing();
                c.start.files.push(std::env::temp_dir().join("cauldron-ui5-not-there.txt"));
            })
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("nt-input", cx);
            window.input("Read this", cx);
            window.click("start", cx);
            window.render_frame(cx);
            assert!(window.find("start-error").label().unwrap().starts_with("Couldn't read cauldron-ui5-not-there.txt"));
        })
        .unwrap();
        assert_eq!(chats.list().len(), 2);
    }

    /// Without a subscription, Start says why and does nothing.
    #[gpui_kit::test]
    fn start_waits_for_a_subscription(cx: &mut TestAppContext) {
        let (handle, chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("start-why").label(), Some("Connect a subscription to start a thread."));
            window.click("nt-input", cx);
            window.input("Anything", cx);
            window.click("start", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(chats.list().len(), 2, "nothing started");
    }

    /// Loading and error: Start waits and says why.
    #[gpui_kit::test]
    fn start_waits_while_core_opens_and_says_when_it_failed(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        for (threads, why) in [(super::super::Threads::Loading, "Opening your threads"), (super::super::Threads::Error("disk full".into()), "Can't open your threads: disk full")] {
            cx.update(|cx| cx.set_reduce_motion(true));
            let handle = cx.add_window(|_, _| Cauldron { threads, ..Cauldron::new() });
            cx.simulate_window_resize(handle.into(), gpui_kit::size(gpui_kit::px(1240.), gpui_kit::px(702.)));
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert_eq!(window.find("start-why").label(), Some(why));
            })
            .unwrap();
        }
    }

    /// The editor's focus shows on the body's line, as the wireframe's focus-within ring.
    #[test]
    fn the_body_line_turns_accent_with_focus() {
        assert_ne!(body_line(&DARK, true), body_line(&DARK, false));
        assert_eq!(body_line(&DARK, true), super::super::thread::mix(DARK.line, DARK.accent, 0.7));
    }

    /// A harness on another machine, answering like `backspace-cli serve`: state at once the
    /// first time and then holding the long poll, and `/v1/task` with a key.
    fn fake_machine() -> String {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                std::thread::spawn(move || {
                    let mut s = s;
                    let mut buf = [0u8; 8192];
                    let n = s.read(&mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let body = if req.starts_with("GET /v1/state?since=0") {
                        serde_json::json!({ "version": 1, "state": backspace_core::project::ProjectState::empty("demo") }).to_string()
                    } else if req.starts_with("GET /v1/state") {
                        std::thread::sleep(std::time::Duration::from_secs(30));
                        return;
                    } else if req.starts_with("POST /v1/task") && req.contains("Run the evals") {
                        r#"{"key":"T-7"}"#.to_string()
                    } else {
                        r#"{"error":"unexpected"}"#.to_string()
                    };
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                });
            }
        });
        url
    }

    fn wait(cx: &mut TestAppContext, mut done: impl FnMut(&mut TestAppContext) -> bool) {
        for _ in 0..500 {
            if done(cx) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            cx.run_until_parked();
        }
        panic!("timed out");
    }

    /// The machine button lists this machine and the connected ones. A reachable machine takes the
    /// text as a task and says so; an unreachable one says why and Start waits.
    #[gpui_kit::test]
    fn start_on_a_connected_machine_sends_a_task(cx: &mut TestAppContext) {
        let (handle, chats) = open(cx);
        let url = fake_machine();
        handle
            .update(cx, |c, _, _| {
                c.start.route = missing();
                c.start.machines = vec![MachineCfg { name: "Studio".into(), url, token: "t".into() }, MachineCfg { name: "Attic".into(), url: "http://127.0.0.1:1".into(), token: "t".into() }];
            })
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("machine", cx);
            window.render_frame(cx);
            assert_eq!(window.find(("machine-item", 0usize)).label(), Some("This machine"));
            assert_eq!(window.find(("machine-item", 2usize)).label(), Some("Attic"));
            let (mx, _, mw, _) = rect(window, "machine-menu");
            let (bx, _, bw, _) = rect(window, "nt-box");
            assert!(mx >= bx && mx + mw <= bx + bw, "the menu sits inside the box's width, under its button");
            window.click(("machine-item", 1usize), cx);
        })
        .unwrap();
        let online = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| { c.start.remote.as_ref().is_some_and(|r| matches!(r.link(), backspace_core::remote::Link::Online)) }).unwrap();
        wait(cx, online);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("machine").label(), Some("Studio"));
            window.click("nt-input", cx);
            window.input("Run the evals", cx);
            window.click("start", cx);
        })
        .unwrap();
        wait(cx, |cx| handle.update(cx, |c, _, _| c.start.note.is_some() || c.start.error.is_some()).unwrap());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("start-note").label(), Some("Sent to Studio as T-7"));
        })
        .unwrap();
        assert_eq!(chats.list().len(), 2, "no local thread for a task on another machine");

        cx.update_window(handle.into(), |_, window, cx| {
            window.click("machine", cx);
            window.render_frame(cx);
            window.click(("machine-item", 2usize), cx);
        })
        .unwrap();
        wait(cx, |cx| handle.update(cx, |c, _, _| c.start.remote.as_ref().is_some_and(|r| matches!(r.link(), backspace_core::remote::Link::Offline { .. }))).unwrap());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("start-why").label().unwrap().starts_with("Can't reach Attic"), "{:?}", window.find("start-why").label());
        })
        .unwrap();
    }
}
