//! The New thread screen (UI5): the Code and Design tabs joined to the box, the box (files,
//! effort, machine, Start) and the cards under it. Start creates the thread in core, attaches the
//! files, sends the text and opens it. On a connected machine Start sends the text as a task to
//! that machine's harness (`/v1/task`), as the Tauri shell did; it does not open a local thread.
//!
//! Sizes are measured from the rendered wireframe at 1240x702 (`.nt-wrap` and children).

use super::thread::{mix, EFFORTS};
use super::*;
use backspace_core::chat::{Route, RouteKind, Scope};
use backspace_core::prefs::MachineCfg;
use backspace_core::remote::{Link, Remote};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder;
use std::path::PathBuf;

/// The rendered `.nt-wrap` at 1240x702: an auto-sized grid track, not the CSS's 680.
const WRAP_W: f32 = 608.0;
/// The cards under the box.
const CARDS_W: f32 = 600.0;
/// The machine menu's top, under the box's foot: brand 28 + 20, tab 35, box padding 4, body 104, 4.
const MENU_TOP: f32 = 72. + 28. + 20. + 35. + 4. + 104. + 4.;

#[derive(Default)]
pub(crate) struct Start {
    pub(crate) design: bool,
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

fn placeholder(design: bool) -> &'static str {
    if design {
        "Describe a screen or a flow to design…"
    } else {
        "Describe a task, a bug or an idea…"
    }
}

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
        let st = cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 10).submit_on_enter(true).placeholder(placeholder(false)));
        let sub = cx.subscribe_in(&st, window, |this, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { shift: false, .. } => this.start_thread(window, cx),
            InputEvent::Focus | InputEvent::Blur => cx.notify(),
            _ => {}
        });
        self.start.input = Some((st, sub));
    }

    /// Why Start cannot run now, if it cannot.
    fn start_blocked(&self) -> Option<String> {
        if self.start.design {
            return Some("Design is a planned app".into());
        }
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
        let design = self.start.design;
        let blocked = self.start_blocked();
        let input = self.start.input.as_ref().expect("made in render").0.clone();
        let focused = input.read(cx).focus_handle(cx).is_focused(window);

        let brand = div()
            .id("nt-brand")
            .flex()
            .items_center()
            .gap(px(10.))
            .mb(px(20.))
            .aria_label("Cauldron")
            .child(div().size(px(28.)).flex().items_center().justify_center().rounded(px(radius::R2)).bg(c(pal.raised)).border_1().border_color(c(pal.line)).child(glyph("flask-conical", 17., c(pal.accent))))
            .child(div().text_size(px(19.)).line_height(px(26.)).font_weight(FontWeight(650.)).text_color(fg).child("Cauldron"))
            .test_support();

        // The tab and the box are one shape: the selected tab has the box's colour, covers its top
        // line and curves into it with an inverse corner of radius R on each free side. GPUI draws a
        // border over its children, so the corners sit beside the tab, one pixel over its side line.
        let corner = |left: bool| {
            div()
                .absolute()
                .bottom_0()
                .when(left, |d| d.left(px(-radius::R)))
                .when(!left, |d| d.right(px(-radius::R)))
                .w(px(radius::R + 1.))
                .h(px(radius::R))
                .overflow_hidden()
                .bg(c(pal.strip))
                .child(
                    div()
                        .absolute()
                        .top(px(-radius::R))
                        .when(left, |d| d.left(px(-radius::R)))
                        .when(!left, |d| d.left(px(1.)))
                        .size(px(radius::R * 2.))
                        .rounded_full()
                        .bg(c(pal.card))
                        .border_1()
                        .border_color(c(pal.line)),
                )
        };
        let kind = |i: usize, label: &'static str, icon: &'static str| {
            let on = design == (i == 1);
            let tab = div()
                .id(("kind", i))
                .h(px(if on { 35. } else { 34. }))
                .flex()
                .items_center()
                .gap(px(7.))
                .px(px(15.))
                .rounded_t(px(radius::R))
                .border_1()
                .border_b_0()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .aria_label(label)
                .aria_selected(on)
                .when(on, |d| d.bg(c(pal.strip)).border_color(c(pal.line)).text_color(fg))
                .when(!on, |d| d.border_color(transparent_black()).text_color(fg3).hover(|d| d.text_color(fg).bg(c(pal.raised))))
                .child(glyph(icon, 14., if on { c(if i == 0 { pal.info } else { pal.design }) } else { fg3 }))
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.start.design = i == 1;
                    if let Some((st, _)) = &this.start.input {
                        st.update(cx, |s, cx| s.set_placeholder(placeholder(i == 1), window, cx));
                    }
                    cx.notify();
                }))
                .test_support();
            div().relative().mb(px(-1.)).child(tab).when(on, |d| d.child(corner(false)).when(i > 0, |d| d.child(corner(true))))
        };
        let tabs = div().id("kinds").flex().items_end().gap(px(2.)).aria_label("Kind of work").child(kind(0, "Code", "code")).child(kind(1, "Design", "palette"));

        let foot_btn = |id: &'static str| {
            div()
                .id(id)
                .h(px(30.))
                .flex()
                .items_center()
                .gap(px(6.))
                .bg(c(pal.well))
                .border_1()
                .border_color(c(pal.line))
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(fg2)
                .cursor_pointer()
                .hover(|d| d.text_color(fg))
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
        let start_btn = div()
            .id("start")
            .size(px(30.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::R2))
            .bg(c(pal.well))
            .border_1()
            .border_color(c(pal.line))
            .aria_label("Start")
            .when_some(blocked.clone(), |d, why| d.aria_description(why).opacity(0.45))
            .when(blocked.is_none(), |d| d.cursor_pointer())
            .child(glyph("arrow-up", 16., fg))
            .on_click(cx.listener(|this, _, window, cx| this.start_thread(window, cx)))
            .test_support();
        let foot = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .id("add-file")
                    .size(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::R1))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.well)))
                    .aria_label("Add a file")
                    .child(glyph("plus", 15., fg2))
                    .on_click(cx.listener(|this, _, window, cx| this.pick_files(window, cx)))
                    .test_support(),
            )
            .child(
                div()
                    .id("nt-effort")
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(fg2)
                    .cursor_pointer()
                    .hover(|d| d.text_color(fg))
                    .aria_label(effort_label.clone())
                    .child(effort_label)
                    .on_click(cx.listener(|_, _, _, cx| {
                        let mut p = Prefs::load();
                        let at = EFFORTS.iter().position(|e| *e == p.cli_effort.as_deref()).unwrap_or(0);
                        p.cli_effort = EFFORTS[(at + 1) % EFFORTS.len()].map(str::to_string);
                        p.save();
                        cx.notify();
                    }))
                    .test_support(),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .gap(px(1.))
                    .child(
                        foot_btn("machine")
                            .px(px(10.))
                            .rounded_l(px(radius::R1))
                            .aria_label(machine_name.clone())
                            .aria_expanded(self.start.menu)
                            .child(glyph("terminal", 13., fg2))
                            .child(machine_name)
                            .child(glyph("chevron-down", 12., fg3))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.start.menu = !this.start.menu;
                                this.start.slide.reset();
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .child(
                        foot_btn("add-machine")
                            .w(px(31.))
                            .justify_center()
                            .rounded_r(px(radius::R1))
                            .aria_label("Add another machine")
                            .aria_description("Machines are added in Settings")
                            .child(glyph("plus", 13., fg2))
                            // Opens the machine list, which says where machines are added.
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.start.menu = true;
                                this.start.slide.reset();
                                cx.notify();
                            }))
                            .test_support(),
                    ),
            )
            .child(start_btn);

        let mut body = div()
            .id("nt-body")
            .flex()
            .flex_col()
            .gap(px(10.))
            .min_h(px(104.))
            .pt(px(12.))
            .px(px(12.))
            .pb(px(10.))
            .bg(c(pal.raised))
            .rounded(px(radius::R))
            .when(!design, |d| d.rounded_tl(px(radius::R2)))
            .border_1()
            .border_color(body_line(pal, focused))
            .child(
                div()
                    .id("nt-input")
                    .mx(px(-10.))
                    .mt(px(-8.))
                    .text_size(px(14.))
                    .child(Textarea::new(&input).appearance(false))
                    .test_support(),
            );
        if !self.start.files.is_empty() {
            let mut row = div().flex().flex_wrap().gap(px(6.));
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
                        .rounded(px(radius::R1))
                        .bg(c(pal.well))
                        .border_1()
                        .border_color(c(pal.line))
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
            body = body.child(row);
        }
        let body = body.child(div().mt_auto().child(foot)).test_support();

        let nt_box = div()
            .id("nt-box")
            .p(px(4.))
            .bg(c(pal.strip))
            .border_1()
            .border_color(c(pal.line))
            .rounded(px(radius::BOX))
            .when(!design, |d| d.rounded_tl(px(0.)))
            .shadow(vec![
                // The same ring the cards wear, then the wireframe's drop.
                BoxShadow { color: ca(pal.fg, if std::ptr::eq(pal, &tokens::DARK) { 0.08 } else { 0.09 }), offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(1.), inset: false },
                drop(tokens::shadow::BOX),
            ])
            .child(body)
            .test_support();

        // What the box says under it: why Start waits, a refusal, a machine's answer, or Design's note.
        let line = |id: &'static str, text: String, col: Hsla| div().id(id).mt(px(10.)).text_size(px(12.)).line_height(px(17.)).text_color(col).aria_label(text.clone()).child(text).test_support();
        let mut wrap = div().w_full().max_w(px(WRAP_W)).flex().flex_col().child(brand).child(tabs).child(nt_box);
        if design {
            wrap = wrap.child(line("design-note", "Design is a planned app. Until it ships, describe the screen in a Code thread.".into(), fg3));
        } else if let Some(e) = &self.start.error {
            wrap = wrap.child(line("start-error", e.clone(), c(pal.bad)));
        } else if let Some(n) = &self.start.note {
            wrap = wrap.child(line("start-note", n.clone(), c(pal.ok)));
        } else if let Some(why) = blocked.filter(|w| w != "Sending") {
            let bad = why.starts_with("Can't");
            wrap = wrap.child(line("start-why", why, if bad { c(pal.bad) } else { fg3 }));
        }

        let mut screen = div()
            .id("new-thread-screen")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(72.))
            .px(px(24.))
            .child(wrap);
        if let Some(cards) = self.start_cards(pal) {
            screen = screen.child(cards);
        }
        if self.start.menu {
            screen = screen.child(self.machine_menu(pal, cx));
        }
        screen.into_any_element()
    }

    /// The cards under the box, from what core has: the thread counts and the recent projects.
    /// Pull requests wait for a GitHub connection in core.
    fn start_cards(&self, pal: &'static Palette) -> Option<Stateful<Div>> {
        let Threads::Ready(list) = &self.threads else { return None };
        if list.is_empty() {
            return None;
        }
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let card = |id: &'static str, title: &'static str| {
            div()
                .id(id)
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(5.))
                .p(px(10.))
                .rounded(px(radius::R1))
                .bg(c(pal.raised))
                .border_1()
                .border_color(c(pal.line))
                .child(div().text_size(px(12.)).line_height(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(fg2).child(title))
        };
        let running = list.iter().filter(|t| t.busy).count();
        let failed = list.iter().filter(|t| t.failed.is_some()).count();
        let counts = card("nt-threads", "Threads")
            .aria_label(format!("{} threads, {running} running, {failed} failed", list.len()))
            .child(div().text_size(px(22.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).font_features(tnum()).child(list.len().to_string()))
            .child(div().text_size(px(11.5)).text_color(fg3).font_features(tnum()).child(format!("{running} running · {failed} failed")))
            .test_support();
        let mut projects: Vec<(String, usize)> = Vec::new();
        for t in list.iter().filter(|t| t.project.is_some()) {
            let name = threads::subtitle(t);
            match projects.iter_mut().find(|(n, _)| *n == name) {
                Some(p) => p.1 += 1,
                None => projects.push((name, 1)),
            }
        }
        let mut proj = card("nt-projects", "Projects").aria_label(if projects.is_empty() { "No projects yet".to_string() } else { projects.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ") });
        if projects.is_empty() {
            proj = proj.child(div().text_size(px(11.5)).text_color(fg3).child("Threads in a project folder show here."));
        }
        for (n, k) in projects.iter().take(2) {
            proj = proj.child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(6.))
                    .min_w_0()
                    .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).truncate().child(n.clone()))
                    .child(div().text_size(px(11.5)).text_color(fg3).font_features(tnum()).child(format!("{k} thread{}", if *k == 1 { "" } else { "s" }))),
            );
        }
        Some(div().id("nt-cards").w_full().max_w(px(CARDS_W)).mt(px(16.)).flex().gap(px(8.)).child(counts).child(proj.test_support()))
    }

    fn machine_menu(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mut menu = div()
            .id("machine-menu")
            .w(px(220.))
            .p(px(4.))
            .rounded(px(radius::R1))
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
            .children(self.start.slide.highlight(c(pal.well), radius::R2));
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
                    .rounded(px(radius::R2))
                    .cursor_pointer()
                    .text_color(if on { fg } else { fg2 })
                    .when(on, |d| d.bg(c(pal.well)))
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
        // Right-aligned under the machine button: the box's right edge less Start and the gap.
        div().absolute().top_0().left_0().size_full().flex().justify_center().child(div().relative().w_full().max_w(px(WRAP_W)).child(div().absolute().top(px(MENU_TOP)).right(px(4. + 12. + 30. + 10.)).child(super::motion::pop_in(div(), menu, "menu-in", 0.)))).into_any_element()
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

    /// With no thread open the card is the New thread screen: brand, Code and Design tabs joined
    /// to the box, the box's controls, and cards from core's threads.
    #[gpui_kit::test]
    fn the_new_thread_screen_draws_the_box_joined_to_its_tab(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        handle.update(cx, |c, _, _| c.start.route = missing()).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("nt-brand").label(), Some("Cauldron"));
            assert_eq!(window.find(("kind", 0usize)).selected(), Some(true));
            assert_eq!(window.find(("kind", 1usize)).selected(), Some(false));
            let (tx, ty, _, th) = rect(window, ("kind", 0usize));
            let (bx, by, bw, _) = rect(window, "nt-box");
            assert_eq!(tx, bx, "Code sits on the box's left edge");
            assert_eq!(ty + th, by + 1.0, "the open tab covers the box's top line");
            assert_eq!(th, 35.0);
            let (cx0, _, cw, _) = rect(window, "card");
            assert_eq!(bw, 608f32.min(cw - 48.0), "the wireframe's width, or the card less its padding");
            assert!(((bx + bw / 2.0) - (cx0 + cw / 2.0)).abs() <= 1.0, "centred in the card");
            for (id, label) in [("add-file", "Add a file"), ("nt-effort", "Auto"), ("machine", "This machine"), ("add-machine", "Add another machine"), ("start", "Start")] {
                assert_eq!(window.find(id).label(), Some(label), "{id}");
            }
            assert_eq!(window.find("nt-threads").label(), Some("2 threads, 0 running, 0 failed"));
            assert_eq!(window.find("nt-projects").label(), Some("Cauldron"));
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

    /// Without a subscription, Start says why and does nothing. Design shows its note, moves the
    /// join (the box's top-left rounds again) and does not start either.
    #[gpui_kit::test]
    fn start_waits_for_a_subscription_and_design_only_notes(cx: &mut TestAppContext) {
        let (handle, chats) = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("start-why").label(), Some("Connect a subscription to start a thread."));
            window.click("nt-input", cx);
            window.input("Anything", cx);
            window.click("start", cx);
            window.click(("kind", 1usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find(("kind", 1usize)).selected(), Some(true));
            assert!(window.find("design-note").label().unwrap().starts_with("Design is a planned app"));
            let (tx, _, _, _) = rect(window, ("kind", 1usize));
            let (bx, _, _, _) = rect(window, "nt-box");
            assert!(tx > bx, "Design's tab is not on the corner");
        })
        .unwrap();
        handle.update(cx, |c, _, _| c.start.route = missing()).unwrap();
        cx.update_window(handle.into(), |_, window, cx| window.click("start", cx)).unwrap();
        cx.run_until_parked();
        assert_eq!(chats.list().len(), 2, "neither started a thread");
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
                assert!(window.try_find("nt-cards").is_none());
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
