//! Backspace workbench, GPUI edition: a pixel-for-pixel port of
//! design/workbench.html (the same layout the Tauri shell renders), drawn
//! natively. Frosted sidebar and main column; environment tabs in the title
//! bar switch between Terminals (1, 2 or a resizable 2x2 of agent sessions
//! and worktrees), Browser (dev-server previews), Diagram (review canvas)
//! and PLAN.md.
//!
//!   backspace [workspace]     (defaults to the current directory)
//!
//! GPUI has no web engine, so the Browser environment shows a text snapshot
//! of the page and opens the real thing in the system browser.

mod icons;
mod pal;
mod stage;

use std::cell::Cell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use backspace_core::diagram::{self, Diagram};
use backspace_core::{
    AgentKind, AgentRecord, AgentStatus, Approval, ApprovalKind, ApprovalState, Harness, LogKind,
    ProjectState, TicketState, MAIN,
};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Root, Theme, ThemeMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use pal::Pal;
use stage::Cam;

/// gpui takes one family name, not a CSS stack: the system faces on macOS
/// (SF Pro, Menlo) and Windows (Segoe UI); elsewhere the bundled Inter (the
/// closest open match to SF Pro) and JetBrains Mono, which the Tauri shell
/// bundles too.
const MONO: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "JetBrains Mono"
};
const UI: &str = if cfg!(target_os = "macos") {
    ".SystemUIFont"
} else if cfg!(target_os = "windows") {
    "Segoe UI"
} else {
    "Inter"
};

/// The faces registered at startup (shared with crates/tauri-app/ui/fonts).
fn bundled_fonts() -> Vec<std::borrow::Cow<'static, [u8]>> {
    macro_rules! font {
        ($f:literal) => {
            std::borrow::Cow::Borrowed(
                &include_bytes!(concat!("../../tauri-app/ui/fonts/", $f, ".ttf"))[..],
            )
        };
    }
    let mut v = vec![
        font!("JetBrainsMono-Regular"),
        font!("JetBrainsMono-Medium"),
        font!("JetBrainsMono-Bold"),
        font!("JetBrainsMono-Italic"),
    ];
    if !cfg!(any(target_os = "macos", target_os = "windows")) {
        v.extend([
            font!("Inter-Regular"),
            font!("Inter-Medium"),
            font!("Inter-SemiBold"),
            font!("Inter-Bold"),
            font!("Inter-Italic"),
        ]);
    }
    v
}

/// Width of two monospace cells at 12px, for hanging indents.
const CH2: f32 = 14.4;

/// macOS blurs what is behind a translucent window; elsewhere the replica's
/// desk (base colour plus soft blobs) is painted behind the glass instead.
const GLASS: bool = cfg!(target_os = "macos");

const SPIN: [&str; 10] = ["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];

// ------------------------------------------------------------------ prefs

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ThemePref {
    Auto,
    Light,
    Dark,
}

impl ThemePref {
    fn path() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/backspace/ui.toml"))
    }

    fn load() -> Self {
        let value = Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| {
                s.lines().find_map(|l| {
                    let (k, v) = l.split_once('=')?;
                    (k.trim() == "theme").then(|| v.trim().trim_matches('"').to_string())
                })
            });
        match value.as_deref() {
            Some("light") => Self::Light,
            Some("dark") => Self::Dark,
            _ => Self::Auto,
        }
    }

    /// ui.toml only holds UI preferences, so rewriting it whole is fine.
    fn save(self) {
        if let Some(p) = Self::path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let name = match self {
                Self::Auto => "system",
                Self::Light => "light",
                Self::Dark => "dark",
            };
            let _ = std::fs::write(p, format!("theme = \"{name}\"\n"));
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Auto => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::Auto,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Light => "sun",
            Self::Dark => "moon",
        }
    }

    fn dark(self, window: &Window) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::Auto => matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        }
    }
}

// ------------------------------------------------------------------ model

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Env {
    Terminals,
    Browser,
    Diagram,
    Docs,
    New,
}

const ENVS: [(Env, &str); 4] = [
    (Env::Terminals, "Terminals"),
    (Env::Browser, "Browser"),
    (Env::Diagram, "Diagram"),
    (Env::Docs, "PLAN.md"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Agent(usize),
    Files(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SideView {
    Projects,
    Agents,
    Tickets,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Viewport {
    Desktop,
    Tablet,
    Phone,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Drag {
    Side,
    Cols,
    Rows,
}

struct Entry {
    depth: usize,
    name: String,
    path: PathBuf,
    dir: bool,
}

/// What a dev-server URL answered, for the Browser environment.
struct Page {
    status: String,
    title: String,
    text: Vec<String>,
    ms: u128,
}

type FitKey = (usize, usize, f32, f32, f32);

struct Workbench {
    harness: Rc<Harness>,
    snap: ProjectState,
    pal: Pal,
    theme_pref: ThemePref,

    env: Env,
    layout: usize,
    slots: [Slot; 4],
    focus: usize,
    max: Option<usize>,
    cols: f32,
    rows: f32,
    side: SideView,
    side_open: bool,
    side_w: f32,
    drag: Option<Drag>,
    grid_bounds: Rc<Cell<Bounds<Pixels>>>,
    win_bounds: Rc<Cell<Bounds<Pixels>>>,

    composer: Entity<InputState>,
    filters: [Entity<InputState>; 4],
    url: Entity<InputState>,
    feedback: Entity<InputState>,
    ticket: Entity<InputState>,
    scrolls: [ScrollHandle; 4],
    seen_len: [usize; 4],
    trees: HashMap<usize, (Instant, Vec<Entry>)>,
    file_preview: [Option<(PathBuf, String)>; 4],

    previews: Vec<String>,
    btab: usize,
    viewport: Viewport,
    pages: HashMap<String, Result<Page, String>>,

    review: Option<usize>,
    cam: Option<Cam>,
    /// Inputs of the last automatic fit; a change refits until the user pans.
    fitted: Option<FitKey>,
    user_cam: bool,
    pan: Option<Point<Pixels>>,
    stage_bounds: Rc<Cell<Bounds<Pixels>>>,
    card_bounds: Rc<Cell<Bounds<Pixels>>>,
    fb_err: bool,
    ticket_form: bool,
    plan: Option<(Instant, Option<String>)>,
    last_pending: usize,

    desk: Option<((u32, u32, Hsla), Arc<Image>)>,
    spin: usize,
    started: Instant,
    focus_handle: FocusHandle,
    _subs: Vec<Subscription>,
    _tasks: Vec<Task<()>>,
}

fn on_enter(
    input: &Entity<InputState>,
    window: &mut Window,
    cx: &mut Context<Workbench>,
    f: impl Fn(&mut Workbench, String, &mut Context<Workbench>) -> bool + 'static,
) -> Subscription {
    cx.subscribe_in(
        input,
        window,
        move |this, input, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = input.read(cx).value().trim().to_string();
                if !text.is_empty() && f(this, text, cx) {
                    input.update(cx, |s, cx| s.set_value("", window, cx));
                }
                cx.notify();
            }
        },
    )
}

impl Workbench {
    fn new(harness: Harness, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let harness = Rc::new(harness);
        let input = |p: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(p))
        };
        let composer = input("Describe the goal, or answer the main agent", window, cx);
        let filters = [
            input("Search", window, cx),
            input("Search", window, cx),
            input("Search", window, cx),
            input("Search", window, cx),
        ];
        let url = input("http://localhost:5173", window, cx);
        let feedback = input("Feedback for the agent (needed to reject)", window, cx);
        let ticket = input("Title: what is wrong or wanted", window, cx);

        let theme_pref = ThemePref::load();
        let mut subs = vec![
            on_enter(&composer, window, cx, |this, text, _| {
                this.harness.send(text);
                true
            }),
            on_enter(&url, window, cx, |this, text, cx| {
                this.go(text, cx);
                false
            }),
            on_enter(&ticket, window, cx, |this, text, _| {
                let (title, body) = text.split_once(':').unwrap_or((&text, &text));
                let ok = this.harness.file_ticket(title.trim(), body.trim()).is_ok();
                if ok {
                    this.ticket_form = false;
                    this.side = SideView::Tickets;
                    this.env = Env::Terminals;
                }
                ok
            }),
            cx.observe(&feedback, |this, _, cx| {
                this.fb_err = false;
                cx.notify()
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                this.apply_theme(window, cx);
            }),
        ];
        for f in &filters {
            subs.push(cx.observe(f, |_, _, cx| cx.notify()));
        }

        let changes = harness.changes();
        let refresh = cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while changes.recv().await.is_ok() {
                let alive = this.update(cx, |this, cx| this.refresh(cx));
                if alive.is_err() {
                    break;
                }
            }
        });
        // Spinner and pulsing dots, only while something is working.
        let tick = cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut AsyncApp| loop {
                cx.background_executor()
                    .timer(Duration::from_millis(130))
                    .await;
                let alive = this.update(cx, |this, cx| {
                    let busy = this.snap.agents.iter().any(|a| {
                        matches!(
                            a.status,
                            AgentStatus::Running | AgentStatus::AwaitingApproval
                        )
                    });
                    if busy {
                        this.spin = (this.spin + 1) % SPIN.len();
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            },
        );

        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let mut this = Self {
            snap: harness.snapshot(),
            harness,
            pal: Pal::light(),
            theme_pref,
            env: Env::Terminals,
            layout: 2,
            slots: [
                Slot::Agent(MAIN),
                Slot::Files(MAIN),
                Slot::Agent(MAIN),
                Slot::Files(MAIN),
            ],
            focus: 0,
            max: None,
            cols: 0.5,
            rows: 0.56,
            side: SideView::Projects,
            side_open: true,
            side_w: 264.,
            drag: None,
            grid_bounds: Rc::default(),
            win_bounds: Rc::default(),
            composer,
            filters,
            url,
            feedback,
            ticket,
            scrolls: Default::default(),
            seen_len: [usize::MAX; 4],
            trees: HashMap::new(),
            file_preview: Default::default(),
            previews: vec![],
            btab: 0,
            viewport: Viewport::Desktop,
            pages: HashMap::new(),
            review: None,
            cam: None,
            fitted: None,
            user_cam: false,
            pan: None,
            stage_bounds: Rc::default(),
            card_bounds: Rc::default(),
            fb_err: false,
            ticket_form: false,
            plan: None,
            last_pending: 0,
            desk: None,
            spin: 0,
            started: Instant::now(),
            focus_handle,
            _subs: subs,
            _tasks: vec![refresh, tick],
        };
        if let Some(n) = std::env::var("BACKSPACE_LAYOUT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| [1, 2, 4].contains(n))
        {
            this.layout = n;
        }
        this.apply_theme(window, cx);
        // bench/ times launch to the first frame with data on screen.
        if let Ok(path) = std::env::var("BACKSPACE_READY_FILE") {
            cx.on_next_frame(window, move |_, _, _| {
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis());
                let _ = std::fs::write(&path, ms.to_string());
            });
        }
        this
    }

    /// Pick the palette, and keep gpui-kit's inputs in the same colours.
    fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dark = self.theme_pref.dark(window);
        self.pal = if dark { Pal::dark() } else { Pal::light() };
        Theme::change(
            if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            Some(window),
            cx,
        );
        let p = self.pal;
        let t = Theme::global_mut(cx);
        t.foreground = p.fg;
        t.muted_foreground = p.fg4;
        t.caret = p.accent;
        t.selection = p.blue.opacity(0.25);
        t.font_family = UI.into();
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.snap = self.harness.snapshot();
        // A new review is queued up front, the way a notification would be.
        let pending: Vec<usize> = self.snap.pending_approvals().map(|a| a.id).collect();
        if pending.len() > self.last_pending && self.env != Env::Diagram {
            self.review = pending.last().copied();
            self.cam = None;
            self.user_cam = false;
        }
        self.last_pending = pending.len();
        cx.notify();
    }

    fn pending(&self) -> usize {
        self.snap.pending_approvals().count()
    }

    fn set_env(&mut self, e: Env, cx: &mut Context<Self>) {
        self.env = e;
        cx.notify();
    }

    fn open(&mut self, v: Slot, cx: &mut Context<Self>) {
        match self.slots[..self.layout].iter().position(|s| *s == v) {
            Some(i) => self.focus = i,
            None => self.slots[self.focus] = v,
        }
        self.max = None;
        self.env = Env::Terminals;
        cx.notify();
    }

    /// A slot that repeats an earlier one takes the busiest agent not on
    /// screen yet (the replica's fillSlots).
    fn fill_slots(&mut self, n: usize) {
        let rank = |a: &AgentRecord| match a.status {
            AgentStatus::Running => 0,
            AgentStatus::AwaitingApproval => 1,
            AgentStatus::Failed => 2,
            _ => 3,
        };
        let mut seen: Vec<Slot> = vec![];
        for i in 0..n {
            if seen.contains(&self.slots[i]) {
                let mut cands: Vec<&AgentRecord> = self
                    .snap
                    .agents
                    .iter()
                    .filter(|a| a.id != MAIN && !seen.contains(&Slot::Agent(a.id)))
                    .collect();
                cands.sort_by_key(|a| rank(a));
                self.slots[i] = match cands.first() {
                    Some(a) => Slot::Agent(a.id),
                    None => Slot::Files(
                        self.snap
                            .agents
                            .iter()
                            .find(|a| a.branch.is_some() && !seen.contains(&Slot::Files(a.id)))
                            .map_or(MAIN, |a| a.id),
                    ),
                };
                if seen.contains(&self.slots[i]) {
                    self.slots[i] = Slot::Agent(MAIN);
                }
            }
            seen.push(self.slots[i]);
        }
    }

    fn tree(&mut self, id: usize) -> &Vec<Entry> {
        let root = self
            .snap
            .agents
            .get(id)
            .and_then(|a| a.worktree.clone())
            .unwrap_or_else(|| self.snap.workspace.clone());
        let stale = self
            .trees
            .get(&id)
            .is_none_or(|(at, _)| at.elapsed() > Duration::from_secs(2));
        if stale {
            let mut out = Vec::new();
            scan(&root, 0, &mut out);
            self.trees.insert(id, (Instant::now(), out));
        }
        &self.trees[&id].1
    }

    fn go(&mut self, text: String, cx: &mut Context<Self>) {
        let u = if text.starts_with("http://") || text.starts_with("https://") {
            text
        } else {
            format!("http://{text}")
        };
        if self.btab < self.previews.len() {
            self.previews[self.btab] = u.clone();
        } else {
            self.previews.push(u.clone());
            self.btab = self.previews.len() - 1;
        }
        self.fetch(u, cx);
    }

    fn fetch(&mut self, url: String, cx: &mut Context<Self>) {
        self.pages.remove(&url);
        let task = cx.background_spawn({
            let url = url.clone();
            async move { probe(&url) }
        });
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let page = task.await;
            let _ = this.update(cx, |this, cx| {
                this.pages.insert(url, page);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn plan_text(&mut self) -> Option<String> {
        let stale = self
            .plan
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed() > Duration::from_secs(1));
        if stale {
            let text = std::fs::read_to_string(self.snap.workspace.join("PLAN.md")).ok();
            self.plan = Some((Instant::now(), text));
        }
        self.plan.as_ref().and_then(|(_, t)| t.clone())
    }

    fn zoom_center(&mut self, f: f32, cx: &mut Context<Self>) {
        let b = self.stage_bounds.get();
        let (sw, sh) = (f32::from(b.size.width), f32::from(b.size.height));
        if let Some(cam) = self.cam.as_mut() {
            cam.zoom(f, sw / 2., sh / 2., sw);
            self.user_cam = true;
            cx.notify();
        }
    }
}

// ------------------------------------------------------------------ primitives

/// The replica's 0.5px hairline: one device pixel.
fn hair(window: &Window) -> Pixels {
    px(1. / window.scale_factor().max(1.))
}

trait Hair: Styled + Sized {
    fn hair_all(mut self, w: Pixels, c: Hsla) -> Self {
        let s = self.style();
        s.border_widths.top = Some(w.into());
        s.border_widths.right = Some(w.into());
        s.border_widths.bottom = Some(w.into());
        s.border_widths.left = Some(w.into());
        self.border_color(c)
    }
    fn hair_b(mut self, w: Pixels, c: Hsla) -> Self {
        self.style().border_widths.bottom = Some(w.into());
        self.border_color(c)
    }
    fn hair_t(mut self, w: Pixels, c: Hsla) -> Self {
        self.style().border_widths.top = Some(w.into());
        self.border_color(c)
    }
    fn hair_r(mut self, w: Pixels, c: Hsla) -> Self {
        self.style().border_widths.right = Some(w.into());
        self.border_color(c)
    }
}
impl<T: Styled> Hair for T {}

fn icon(name: &str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(icons::path(name))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// `.ib`: a 24px square button around `content`.
fn ib_with(
    id: impl Into<ElementId>,
    content: impl IntoElement,
    p: &Pal,
    on: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(24.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .cursor_pointer()
        .text_color(p.fg3)
        .when(on, |d| d.bg(p.sel))
        .hover(|d| d.bg(p.hover))
        .child(content)
}

/// `.ib` with one of the line icons.
fn ib(id: impl Into<ElementId>, name: &str, p: &Pal, on: bool) -> Stateful<Div> {
    ib_with(id, icon(name, 14., if on { p.fg } else { p.fg3 }), p, on)
}

/// `.dot`, with the CSS pulse (opacity 1 → 0.35 → 1 over 1.4s) for running.
fn dot(c: Hsla, pulse: Option<f32>) -> Div {
    let a = pulse.map_or(1., |t| {
        let ph = (t % 1.4) / 1.4;
        1. - 0.65 * (1. - (2. * ph - 1.).abs())
    });
    div()
        .size(px(6.))
        .flex_none()
        .rounded_full()
        .bg(c.opacity(c.a * a))
}

fn status_dot(p: &Pal, s: AgentStatus, t: f32) -> Div {
    match s {
        AgentStatus::Running => dot(p.accent, Some(t)),
        AgentStatus::AwaitingApproval => dot(p.amber, None),
        AgentStatus::Approved => dot(p.green, None),
        AgentStatus::Failed => dot(p.red, None),
        _ => dot(p.fg4, None),
    }
}

/// Text with per-span colours (StyledText highlights).
fn rich(parts: &[(&str, Option<Hsla>)]) -> StyledText {
    let mut s = String::new();
    let mut hl = vec![];
    for (t, c) in parts {
        let start = s.len();
        s.push_str(t);
        if let Some(c) = c {
            hl.push((
                start..s.len(),
                HighlightStyle {
                    color: Some(*c),
                    ..Default::default()
                },
            ));
        }
    }
    StyledText::new(s).with_highlights(hl)
}

fn route(a: &AgentRecord) -> String {
    let now = a.decision.as_ref().map_or("not routed".to_string(), |d| {
        format!("{} @ {}", d.model, d.effort)
    });
    match a.escalations.first().and_then(|e| e.split(" → ").next()) {
        Some(start) => format!("{start} → {now} ↑{}", a.escalations.len()),
        None => now,
    }
}

fn btn(id: impl Into<ElementId>, label: &str, p: &Pal, w: Pixels) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.))
        .px(px(14.))
        .flex()
        .items_center()
        .rounded(px(7.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .bg(p.pill_on)
        .hair_all(w, p.pane_edge)
        .child(label.to_string())
}

/// The pixel mascot: 12x7 cells of 4px, accent coloured.
fn mascot(c: Hsla) -> impl IntoElement {
    const ROWS: [&str; 7] = [
        "...XXXXXXXXX",
        "..XXXXXXXXXX",
        ".XXXX.XX.XXX",
        "XXXXXXXXXXXX",
        ".XXXXXXXXXXX",
        "..XX.XXXX.XX",
        "...XXXXXXXXX",
    ];
    canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            for (y, row) in ROWS.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    if ch == 'X' {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(
                                    b.origin.x + px(x as f32 * 4.),
                                    b.origin.y + px(y as f32 * 4.),
                                ),
                                size(px(4.), px(4.)),
                            ),
                            c,
                        ));
                    }
                }
            }
        },
    )
    .w(px(48.))
    .h(px(28.))
    .flex_none()
}

/// The desk behind the glass: the replica's three `blur(70px)` blobs over
/// the base colour, rendered once per window size at quarter resolution
/// (blurred content loses nothing) and drawn as a single image, so frames
/// do not re-rasterise it.
fn desk_image(p: &Pal, w: f32, h: f32) -> Arc<Image> {
    const SCALE: f32 = 4.;
    let (iw, ih) = (
        (w / SCALE).ceil().max(1.) as usize,
        (h / SCALE).ceil().max(1.) as usize,
    );
    let vw = w / 100.;
    let rgb = |c: Hsla| {
        let c = c.to_rgb();
        [c.r, c.g, c.b]
    };
    // (diameter, left, top) in vw, from .blob.a/.b/.c; c hangs off the bottom.
    let blobs = [
        (52., -10., Some(-14.), rgb(p.blobs[0])),
        (46., 100. - 12. - 46., Some(-6.), rgb(p.blobs[1])),
        (60., 26., None, rgb(p.blobs[2])),
    ];
    let base = rgb(p.desk_base);
    let sigma = 70.;
    // 24-bit BMP, bottom-up rows padded to 4 bytes.
    let row = (iw * 3 + 3) & !3;
    let mut bmp = Vec::with_capacity(54 + row * ih);
    let size = (54 + row * ih) as u32;
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&size.to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0]);
    bmp.extend_from_slice(&(iw as i32).to_le_bytes());
    bmp.extend_from_slice(&(ih as i32).to_le_bytes());
    bmp.extend_from_slice(&[1, 0, 24, 0]);
    bmp.extend_from_slice(&[0; 24]);
    for y in (0..ih).rev() {
        let py = (y as f32 + 0.5) * SCALE;
        let mut line = Vec::with_capacity(row);
        for x in 0..iw {
            let px_ = (x as f32 + 0.5) * SCALE;
            let mut c = base;
            for (d, l, t, bc) in &blobs {
                let r = d * vw / 2.;
                let cx0 = l * vw + r;
                let cy0 = match t {
                    Some(t) => t * vw + r,
                    None => h + 34. * vw - r,
                };
                let dist = ((px_ - cx0).powi(2) + (py - cy0).powi(2)).sqrt();
                // A Gaussian-blurred disc's edge: ~ 0.5 * erfc((d - r) / (σ√2)).
                let a = 0.85 * 0.5 * (1. - ((dist - r) / sigma * 1.13).tanh());
                for i in 0..3 {
                    c[i] = c[i] * (1. - a) + bc[i] * a;
                }
            }
            for i in [2, 1, 0] {
                line.push((c[i].clamp(0., 1.) * 255.).round() as u8);
            }
        }
        line.resize(row, 0);
        bmp.extend_from_slice(&line);
    }
    Arc::new(Image::from_bytes(ImageFormat::Bmp, bmp))
}

// ------------------------------------------------------------------ render

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let w = hair(window);
        let t = self.started.elapsed().as_secs_f32();
        let win_bounds = self.win_bounds.clone();
        let vs = window.viewport_size();
        let key = (
            f32::from(vs.width) as u32,
            f32::from(vs.height) as u32,
            self.pal.desk_base,
        );
        if self.desk.as_ref().is_none_or(|(k, _)| *k != key) {
            let img = desk_image(&self.pal, key.0 as f32, key.1 as f32);
            self.desk = Some((key, img));
        }
        let desk = self.desk.as_ref().unwrap().1.clone();
        let env = match self.env {
            Env::Terminals => self.terminals(w, cx),
            Env::Browser => self.browser(w, cx),
            Env::Diagram => self.diagram_env(w, window, cx),
            Env::Docs => self.docs(w),
            Env::New => self.new_env(w, cx),
        };

        let main = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(p.glass_main)
            .child(self.tbar(w, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .px(px(6.))
                    .pb(px(6.))
                    .child(env),
            );

        let side = self.side_open.then(|| self.sidebar(w, t, cx));
        let win = div()
            .size_full()
            .relative()
            .flex()
            .bg(p.glass)
            .children(side)
            .child(main)
            .when(self.side_open, |d| {
                d.child(
                    div()
                        .id("side-gut")
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(self.side_w - 3.))
                        .w(px(7.))
                        .cursor_col_resize()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, _| this.drag = Some(Drag::Side)),
                        ),
                )
            });

        div()
            .id("root")
            .track_focus(&self.focus_handle)
            .size_full()
            .relative()
            .font_family(UI)
            .text_size(px(13.))
            .line_height(relative(1.4))
            .text_color(p.fg)
            .when(!GLASS, |d| {
                d.bg(p.desk_base)
                    .child(img(desk).absolute().size_full().object_fit(ObjectFit::Fill))
            })
            .child(
                canvas(move |b, _, _| win_bounds.set(b), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(win)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                let m = e.keystroke.modifiers;
                if !(m.platform || m.control) {
                    return;
                }
                match e.keystroke.key.as_str() {
                    k @ ("1" | "2" | "3" | "4") => {
                        let i: usize = k.parse().unwrap();
                        this.set_env(ENVS[i - 1].0, cx);
                        cx.stop_propagation();
                    }
                    "\\" => {
                        this.side_open = !this.side_open;
                        cx.notify();
                    }
                    _ => {}
                }
            }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if let Some(d) = this.drag {
                    match d {
                        Drag::Side => {
                            let left = this.win_bounds.get().origin.x;
                            this.side_w = f32::from(e.position.x - left).clamp(220., 380.);
                        }
                        Drag::Cols => {
                            let b = this.grid_bounds.get();
                            this.cols = (f32::from(e.position.x - b.origin.x)
                                / f32::from(b.size.width))
                            .clamp(0.2, 0.8);
                        }
                        Drag::Rows => {
                            let b = this.grid_bounds.get();
                            this.rows = (f32::from(e.position.y - b.origin.y)
                                / f32::from(b.size.height))
                            .clamp(0.2, 0.8);
                        }
                    }
                    cx.notify();
                } else if let (Some(last), Some(cam)) = (this.pan, this.cam.as_mut()) {
                    let d = e.position - last;
                    cam.x -= f32::from(d.x) / cam.k;
                    cam.y -= f32::from(d.y) / cam.k;
                    this.pan = Some(e.position);
                    this.user_cam = true;
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.drag.take().is_some() || this.pan.take().is_some() {
                        cx.notify();
                    }
                }),
            )
    }
}

impl Workbench {
    // -------------------------------------------------------------- sidebar

    fn sidebar(&mut self, w: Pixels, t: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let seg = [
            (SideView::Projects, "folder", "Projects"),
            (SideView::Agents, "sparkle", "Agents"),
            (SideView::Tickets, "ticket", "Tickets"),
        ];
        let rows = self.side_rows(t, cx);
        div()
            .w(px(self.side_w))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(p.glass_side)
            .hair_r(w, p.pane_edge)
            .child(
                div()
                    .h(px(38.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_end()
                    .px(px(10.))
                    .window_control_area(WindowControlArea::Drag)
                    .child(ib("side-close", "sidebar", &p, false).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.side_open = false;
                            cx.notify()
                        },
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .pt(px(4.))
                    .px(px(12.))
                    .pb(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(16.))
                            .child(
                                div()
                                    .size(px(18.))
                                    .rounded(px(5.))
                                    .bg(p.fg)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon("bksp", 12., p.desk_base)),
                            )
                            .child("Local"),
                    )
                    .child(icon("chev", 14., p.fg3))
                    .child(icon("home", 14., p.fg3))
                    .child(div().w_full().flex().gap(px(2.)).mt(px(6.)).children(
                        seg.into_iter().map(|(k, ic, l)| {
                            let on = self.side == k;
                            div()
                                .id(l)
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .h(px(22.))
                                .px(px(7.))
                                .rounded(px(6.))
                                .text_size(px(11.5))
                                .cursor_pointer()
                                .text_color(if on { p.fg } else { p.fg3 })
                                .when(on, |d| d.bg(p.sel))
                                .hover(|d| d.text_color(p.fg))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.side = k;
                                    cx.notify()
                                }))
                                .child(icon(ic, 14., if on { p.fg } else { p.fg3 }))
                                .child(l)
                        }),
                    )),
            )
            .child(
                div()
                    .id("side-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pt(px(2.))
                    .px(px(6.))
                    .pb(px(8.))
                    .children(rows),
            )
            .child(self.run_card(w))
            .child(
                div()
                    .h(px(34.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .px(px(10.))
                    .child(ib("home", "home", &p, true))
                    .child(ib("windows", "window", &p, false))
                    .child(div().flex_1())
                    .child(ib("gear", "gear", &p, false)),
            )
    }

    fn row(&self, id: impl Into<ElementId>, indent: f32, sel: bool) -> Stateful<Div> {
        let p = self.pal;
        div()
            .id(id)
            .w_full()
            .flex()
            .items_center()
            .gap(px(7.))
            .h(px(27.))
            .pl(px(indent))
            .pr(px(8.))
            .rounded(px(6.))
            .whitespace_nowrap()
            .cursor_pointer()
            .when(sel, |d| d.bg(p.sel))
            .hover(|d| d.bg(if sel { p.sel } else { p.hover }))
    }

    fn agent_row(
        &self,
        a: &AgentRecord,
        indent: f32,
        t: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.pal;
        let id = a.id;
        let sel = self.env == Env::Terminals && self.slots[self.focus] == Slot::Agent(id);
        self.row(("agent", id), indent, sel)
            .on_click(cx.listener(move |this, _, _, cx| this.open(Slot::Agent(id), cx)))
            .child(icon("sparkle", 14., p.accent))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .when(a.kind == AgentKind::Triage, |d| d.text_color(p.fg3))
                    .child(a.title.clone()),
            )
            .child(
                div()
                    .ml_auto()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(11.))
                    .text_color(p.fg3)
                    .when(!a.escalations.is_empty(), |d| {
                        d.child(
                            div()
                                .text_color(p.amber)
                                .child(format!("↑{}", a.escalations.len())),
                        )
                    })
                    .child(status_dot(&p, a.status, t)),
            )
    }

    fn side_rows(&mut self, t: f32, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = self.pal;
        let s = &self.snap;
        let mut out: Vec<AnyElement> = vec![];
        let label = |text: String| {
            div()
                .pt(px(10.))
                .px(px(10.))
                .pb(px(4.))
                .text_size(px(11.))
                .text_color(p.fg3)
                .child(text)
                .into_any_element()
        };
        match self.side {
            SideView::Projects => {
                out.push(
                    self.row("proj", 8., true)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(icon("folder", 14., p.fg))
                        .child(div().min_w_0().truncate().child(s.name.clone()))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(p.fg3)
                                .child(s.agents.len().to_string()),
                        )
                        .into_any_element(),
                );
                for id in tree_order(s) {
                    let a = &s.agents[id];
                    out.push(
                        self.agent_row(a, if a.depth > 0 { 36. } else { 22. }, t, cx)
                            .into_any_element(),
                    );
                }
                for (i, u) in self.previews.iter().enumerate() {
                    out.push(
                        self.row(("pv", i), 22., false)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.btab = i;
                                this.set_env(Env::Browser, cx)
                            }))
                            .child(icon("globe", 14., p.blue))
                            .child(div().min_w_0().truncate().child(format!(
                                "Preview · {}",
                                u.trim_start_matches("http://")
                                    .trim_start_matches("https://")
                            )))
                            .into_any_element(),
                    );
                }
                let wts: Vec<&AgentRecord> = s
                    .agents
                    .iter()
                    .filter(|a| a.branch.is_some() && a.id != MAIN)
                    .collect();
                if !wts.is_empty() {
                    out.push(
                        self.row("wts", 8., false)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(icon("folder", 14., p.fg))
                            .child(div().child("worktrees"))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(p.fg3)
                                    .child(wts.len().to_string()),
                            )
                            .into_any_element(),
                    );
                    for a in wts {
                        let id = a.id;
                        out.push(
                            self.row(("wt", id), 22., false)
                                .on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.open(Slot::Files(id), cx)
                                    }),
                                )
                                .child(icon("branch", 14., p.fg3))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .child(a.branch.clone().unwrap_or_default()),
                                )
                                .child(
                                    div()
                                        .ml_auto()
                                        .text_size(px(11.))
                                        .text_color(p.fg3)
                                        .child(a.ticket.clone().unwrap_or_default()),
                                )
                                .into_any_element(),
                        );
                    }
                }
            }
            SideView::Agents => {
                out.push(label(format!("{} · {} agents", s.name, s.agents.len())));
                for id in tree_order(s) {
                    let a = &s.agents[id];
                    out.push(self.agent_row(a, 8., t, cx).into_any_element());
                    out.push(
                        div()
                            .ml(px(37.))
                            .mt(px(-3.))
                            .mb(px(3.))
                            .font_family(MONO)
                            .text_size(px(11.))
                            .text_color(p.fg3)
                            .truncate()
                            .child(format!("{} · ${:.3}", route(a), s.subtree_cost(id)))
                            .into_any_element(),
                    );
                }
            }
            SideView::Tickets => {
                use TicketState::*;
                let groups: [(&str, &[TicketState]); 5] = [
                    ("Needs you", &[Proposed, InReview, ReadyForHuman, NeedsInfo]),
                    ("Running", &[Queued, InProgress]),
                    ("Ready", &[ReadyForAgent, NeedsTriage]),
                    ("Done", &[Done]),
                    ("Closed", &[Failed, Wontfix]),
                ];
                for (name, states) in groups {
                    let rows: Vec<_> = s
                        .tickets
                        .iter()
                        .filter(|x| states.contains(&x.state))
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    out.push(label(format!("{name} · {}", rows.len())));
                    for tk in rows {
                        let assignee = tk.assignee;
                        out.push(
                            self.row(("tk", tk.num), 8., false)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(a) = assignee {
                                        this.open(Slot::Agent(a), cx)
                                    }
                                }))
                                .child(
                                    div()
                                        .font_family(MONO)
                                        .text_size(px(11.5))
                                        .text_color(p.fg3)
                                        .child(format!("{:02}", tk.num)),
                                )
                                .child(div().min_w_0().truncate().child(tk.title.clone()))
                                .child(
                                    div()
                                        .ml_auto()
                                        .text_size(px(11.))
                                        .text_color(p.fg3)
                                        .child(tk.state.label().replace('_', "-")),
                                )
                                .into_any_element(),
                        );
                    }
                }
                if s.tickets.is_empty() {
                    out.push(nothing(
                        &p,
                        "No tickets yet. The main agent creates them from your goal.",
                    ));
                }
            }
        }
        out
    }

    fn run_card(&self, w: Pixels) -> impl IntoElement {
        let p = self.pal;
        let s = &self.snap;
        let running = s
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Running)
            .count();
        let wts = s
            .agents
            .iter()
            .filter(|a| a.branch.is_some() && a.id != MAIN)
            .count();
        let pending = self.pending();
        let row = |d: Div, l: &str, v: String| {
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(24.))
                .px(px(3.))
                .text_size(px(12.))
                .child(d)
                .child(div().truncate().child(l.to_string()))
                .child(
                    div()
                        .ml_auto()
                        .font_family(MONO)
                        .text_size(px(11.))
                        .text_color(p.fg3)
                        .whitespace_nowrap()
                        .child(v),
                )
        };
        div()
            .mx(px(8.))
            .my(px(6.))
            .py(px(8.))
            .px(px(9.))
            .rounded(px(10.))
            .bg(p.pane)
            .hair_all(w, p.pane_edge)
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.fg3)
                    .mb(px(4.))
                    .child(format!("This run · {}", s.name)),
            )
            .child(row(
                dot(
                    p.accent,
                    (running > 0).then(|| self.started.elapsed().as_secs_f32()),
                ),
                "Agents running",
                running.to_string(),
            ))
            .child(row(
                dot(if pending > 0 { p.amber } else { p.fg4 }, None),
                "Waiting on you",
                pending.to_string(),
            ))
            .child(row(dot(p.green, None), "Worktrees", wts.to_string()))
            .child(row(
                dot(p.fg4, None),
                "Spent",
                format!(
                    "${:.3} · router ${:.3}",
                    s.total_cost_usd, s.router_cost_usd
                ),
            ))
    }

    // -------------------------------------------------------------- title bar

    fn tbar(&mut self, w: Pixels, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.pal;
        let pending = self.pending();
        let tab = |id: &'static str, sel: bool| {
            div()
                .id(id)
                .relative()
                .h(px(22.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(5.))
                .rounded(px(6.))
                .text_size(px(11.5))
                .whitespace_nowrap()
                .cursor_pointer()
                .bg(if sel { p.pill_on } else { p.pill })
                .hair_all(w, p.pill_edge)
                .text_color(if sel { p.fg } else { p.fg2 })
                .when(sel, |d| {
                    d.shadow(vec![BoxShadow {
                        color: hsla(0., 0., 0., 0.08),
                        offset: point(px(0.), px(1.)),
                        blur_radius: px(2.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                })
                .hover(|d| d.text_color(p.fg))
        };
        let lays = [(2, "lay2"), (1, "lay1"), (4, "lay4")];
        div()
            .h(px(38.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .when(!self.side_open && GLASS, |d| d.pl(px(84.)))
            .window_control_area(WindowControlArea::Drag)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .gap(px(2.))
                    .when(!self.side_open, |d| {
                        d.child(ib("side-open", "sidebar", &p, false).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.side_open = true;
                                cx.notify()
                            },
                        )))
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .items_center()
                            .children(ENVS.iter().map(|&(e, l)| {
                                tab(l, self.env == e)
                                    .on_click(
                                        cx.listener(move |this, _, _, cx| this.set_env(e, cx)),
                                    )
                                    .child(l)
                                    .when(e == Env::Diagram && pending > 0, |d| {
                                        d.child(
                                            div()
                                                .absolute()
                                                .top(px(-3.))
                                                .right(px(-3.))
                                                .size(px(7.))
                                                .rounded_full()
                                                .bg(p.blue)
                                                .border(px(1.5))
                                                .border_color(p.glass),
                                        )
                                    })
                            }))
                            .child(
                                tab("plus", self.env == Env::New)
                                    .px(px(6.))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.set_env(Env::New, cx)),
                                    )
                                    .child(icon("plus", 14., p.fg2)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(3.))
                            .ml(px(6.))
                            .children(lays.map(|(n, ic)| {
                                let on = self.env == Env::Terminals
                                    && self.layout == n
                                    && self.max.is_none();
                                div()
                                    .id(ic)
                                    .w(px(24.))
                                    .h(px(20.))
                                    .rounded(px(5.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hair_all(w, if on { p.pill_edge } else { transparent_black() })
                                    .when(on, |d| d.bg(p.pill_on))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.layout = n;
                                        this.max = None;
                                        this.focus = this.focus.min(n - 1);
                                        this.set_env(Env::Terminals, cx)
                                    }))
                                    .child(
                                        svg()
                                            .path(icons::path(ic))
                                            .w(px(16.))
                                            .h(px(12.))
                                            .text_color(
                                                (if on { p.fg } else { p.fg3 }).opacity(0.85),
                                            ),
                                    )
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_end()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        ib("theme", self.theme_pref.icon(), &p, false).on_click(cx.listener(
                            |this, _, window, cx| {
                                this.theme_pref = this.theme_pref.next();
                                this.theme_pref.save();
                                this.apply_theme(window, cx);
                            },
                        )),
                    )
                    .child(
                        div()
                            .id("cta")
                            .h(px(22.))
                            .px(px(10.))
                            .rounded_full()
                            .text_size(px(11.5))
                            .border_1()
                            .border_color(p.fg2)
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .whitespace_nowrap()
                            .cursor_pointer()
                            .hover(|d| d.bg(p.hover))
                            .on_click(cx.listener(|this, _, _, cx| this.set_env(Env::Diagram, cx)))
                            .map(|d| {
                                if pending > 0 {
                                    d.child(dot(p.amber, None))
                                        .child(format!("{pending} to review"))
                                } else {
                                    d.child(icon("check", 14., p.fg)).child("Nothing to review")
                                }
                            }),
                    ),
            )
    }

    // -------------------------------------------------------------- terminals

    fn gutter(&self, id: &'static str, d: Drag, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = self.pal;
        let vertical = d == Drag::Cols;
        let line = if self.drag == Some(d) {
            p.pane_edge_on
        } else {
            transparent_black()
        };
        div()
            .id(id)
            .flex_none()
            .relative()
            .group(id)
            .map(|el| {
                if vertical {
                    el.w(px(6.)).h_full().cursor_col_resize()
                } else {
                    el.h(px(6.)).w_full().cursor_row_resize()
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, _| this.drag = Some(d)),
            )
            .on_click(cx.listener(|this, e: &ClickEvent, _, cx| {
                if e.click_count() == 2 {
                    this.cols = 0.5;
                    this.rows = 0.56;
                    cx.notify();
                }
            }))
            .child(
                div()
                    .absolute()
                    .rounded(px(2.))
                    .bg(line)
                    .group_hover(id, |s| s.bg(p.pane_edge_on))
                    .map(|el| {
                        if vertical {
                            el.top(relative(0.3))
                                .bottom(relative(0.3))
                                .left(px(2.))
                                .w(px(2.))
                        } else {
                            el.left(relative(0.3))
                                .right(relative(0.3))
                                .top(px(2.))
                                .h(px(2.))
                        }
                    }),
            )
    }

    fn terminals(&mut self, w: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let n = if self.max.is_some() { 1 } else { self.layout };
        if self.max.is_none() {
            self.fill_slots(n);
        }
        let shown: Vec<usize> = match self.max {
            Some(m) => vec![m],
            None => (0..n).collect(),
        };
        let mut panes: Vec<AnyElement> = shown.iter().map(|&i| self.pane(i, w, cx)).collect();
        let gb = self.grid_bounds.clone();
        let measure = canvas(move |b, _, _| gb.set(b), |_, _, _, _| {})
            .absolute()
            .size_full();
        let cell = |el: AnyElement| div().size_full().flex().child(el);
        // The stylesheet's `calc((100% - 6px) * ratio)`, from last frame's size.
        let gb = self.grid_bounds.get().size;
        let split = |total: Pixels, ratio: f32| -> DefiniteLength {
            if total > px(0.) {
                px(f32::from(total - px(6.)) * ratio).into()
            } else {
                relative(ratio)
            }
        };
        let (cw, rh) = (split(gb.width, self.cols), split(gb.height, self.rows));
        let grid = match n {
            1 => div().size_full().flex().child(panes.remove(0)),
            2 => {
                let (a, b) = (panes.remove(0), panes.remove(0));
                div()
                    .size_full()
                    .flex()
                    .child(div().w(cw).h_full().flex_none().flex().child(a))
                    .child(self.gutter("gut-v", Drag::Cols, cx))
                    .child(div().flex_1().min_w_0().h_full().flex().child(b))
            }
            _ => {
                let mut it = panes.into_iter();
                let mut next = || cell(it.next().unwrap());
                let (a, b, c, d) = (next(), next(), next(), next());
                let row = |l: Div, r: Div, g: Stateful<Div>| {
                    div()
                        .w_full()
                        .flex()
                        .child(div().w(cw).h_full().flex_none().flex().child(l))
                        .child(g)
                        .child(div().flex_1().min_w_0().h_full().flex().child(r))
                };
                let (g1, g2, gh) = (
                    self.gutter("gut-v1", Drag::Cols, cx),
                    self.gutter("gut-v2", Drag::Cols, cx),
                    self.gutter("gut-h", Drag::Rows, cx),
                );
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(row(a, b, g1).h(rh).flex_none())
                    .child(gh)
                    .child(row(c, d, g2).flex_1().min_h_0())
            }
        };
        div()
            .size_full()
            .relative()
            .child(measure)
            .child(grid)
            .into_any_element()
    }

    fn pane(&mut self, slot: usize, w: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let v = self.slots[slot];
        let on = slot == self.focus;
        let (id, files) = match v {
            Slot::Agent(id) => (id, false),
            Slot::Files(id) => (id, true),
        };
        let Some(a) = self.snap.agents.get(id).cloned() else {
            return div().into_any_element();
        };
        let title = if files {
            a.ticket
                .as_ref()
                .map_or("Primary worktree".into(), |k| format!("{k} worktree"))
        } else if id == MAIN {
            format!("Main agent · {}", self.snap.name)
        } else {
            a.title.clone()
        };
        let head = div()
            .h(px(30.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(7.))
            .pl(px(10.))
            .pr(px(6.))
            .text_size(px(12.))
            .hair_b(w, p.pane_edge)
            .child(if files {
                icon("folder", 14., p.fg3)
            } else {
                icon("sparkle", 14., p.accent)
            })
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .min_w_0()
                    .truncate()
                    .child(title),
            )
            .child(div().flex_1())
            .when(!files, |d| {
                d.child(
                    ib(("pf", slot), "folder", &p, false)
                        .size(px(22.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.slots[slot] = Slot::Files(id);
                            cx.notify()
                        })),
                )
            })
            .child(
                ib(("pm", slot), "expand", &p, false)
                    .size(px(22.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.max = if this.max.is_none() { Some(slot) } else { None };
                        cx.notify()
                    })),
            )
            .child(
                ib(("pc", slot), "close", &p, false)
                    .size(px(22.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.max.is_some() {
                            this.max = None;
                        } else if this.layout > 1 {
                            let x = this.slots[slot];
                            for i in slot..3 {
                                this.slots[i] = this.slots[i + 1];
                            }
                            this.slots[3] = x;
                            this.layout = if this.layout == 4 { 2 } else { 1 };
                        }
                        this.focus = 0;
                        cx.notify()
                    })),
            );
        let body = if files {
            self.files_body(slot, id, w, cx).into_any_element()
        } else {
            self.agent_body(slot, &a, w).into_any_element()
        };
        div()
            .id(("pane", slot))
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .rounded(px(9.))
            .bg(p.pane)
            .hair_all(w, if on { p.pane_edge_on } else { p.pane_edge })
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if this.focus != slot {
                        this.focus = slot;
                        cx.notify();
                    }
                }),
            )
            .child(head)
            .child(body)
            .into_any_element()
    }

    fn agent_body(&mut self, slot: usize, a: &AgentRecord, w: Pixels) -> impl IntoElement {
        let p = self.pal;
        // Stay pinned to the tail unless the reader scrolled up; a pane that
        // just opened (or changed agent) starts at the tail.
        let sh = &self.scrolls[slot];
        let near_bottom = -sh.offset().y >= sh.max_offset().y - px(40.);
        if near_bottom || self.seen_len[slot] != a.id {
            sh.scroll_to_bottom();
        }
        self.seen_len[slot] = a.id;
        let s = &self.snap;

        let path = a
            .worktree
            .clone()
            .unwrap_or_else(|| s.workspace.clone())
            .display()
            .to_string();
        let routed = match &a.decision {
            Some(d) if d.confidence > 0. => format!(
                "{} · routed by {} ({}% confident)",
                route(a),
                d.source,
                (d.confidence * 100.).round()
            ),
            Some(d) => format!("{} · routed by {}", route(a), d.source),
            None => route(a),
        };
        let banner = div()
            .flex()
            .gap(px(14.))
            .items_center()
            .mt(px(4.))
            .child(mascot(p.accent))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .line_height(relative(1.45))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child("Backspace v0.1.0"),
                    )
                    .child(div().text_color(p.term_dim).child(routed))
                    .child(div().text_color(p.term_dim).child(path)),
            );
        let spin = SPIN[self.spin];
        let mut lines: Vec<AnyElement> = vec![];
        // CSS collapses adjacent margins: each gap is the larger of the two.
        let mut prev_mb = 14.; // the banner's margin-bottom
        for e in &a.log {
            let (mt, mb) = log_margins(e.kind, &e.text);
            lines.push(
                div()
                    .mt(px(mt.max(prev_mb)))
                    .child(log_el(e.kind, &e.text, &p))
                    .into_any_element(),
            );
            prev_mb = mb;
        }
        let tail_gap = |mt: f32| px(mt.max(prev_mb));
        match a.status {
            AgentStatus::Running => lines.push(
                div()
                    .mt(tail_gap(10.))
                    .text_color(p.accent)
                    .child(format!(
                        "{spin} Working… ({} · ↓ {} tokens)",
                        a.decision
                            .as_ref()
                            .map_or("routing".into(), |d| d.model.clone()),
                        thousands(a.input_tokens)
                    ))
                    .into_any_element(),
            ),
            AgentStatus::AwaitingApproval => lines.push(
                div()
                    .mt(tail_gap(10.))
                    .text_color(p.accent)
                    .child(format!("{spin} Waiting for your review…"))
                    .into_any_element(),
            ),
            _ => {}
        }
        if a.id == MAIN && !a.log.iter().any(|e| e.kind == LogKind::User) {
            lines.push(
                div()
                    .mt(tail_gap(8.))
                    .child(note(
                        &p,
                        "Type a goal below. The main agent interviews you, then proposes tickets for you to approve.",
                    ))
                    .into_any_element(),
            );
        }

        let total = s.total_cost_usd.max(0.000001);
        let sub = s.subtree_cost(a.id);
        let share = (sub / total).min(1.);
        let filled = (share * 10.).round() as usize;
        let model = a.decision.as_ref().map_or("not routed".into(), |d| {
            format!("{} @ {}", d.model, d.effort)
        });
        let branch = a.branch.clone().unwrap_or_else(|| "-".into());
        let hint: StyledText = if a.id == MAIN {
            rich(&[
                ("▸▸ plan approval on ", None),
                ("(review in Diagram)", Some(p.blue)),
                (" · ⌘1–4 switch tabs", None),
            ])
        } else {
            let text = match a.ticket.as_ref().and_then(|k| s.ticket(k)) {
                Some(tk) => format!(
                    "▸▸ check: {} · {}",
                    tk.check.clone().unwrap_or_else(|| "none".into()),
                    tk.state.label().replace('_', "-")
                ),
                None => format!("▸▸ {:?}", a.status).to_lowercase(),
            };
            StyledText::new(text)
        };
        let name = s.name.clone();
        let prompt = if a.id == MAIN {
            div()
                .flex_1()
                .min_w_0()
                .child(
                    Input::new(&self.composer)
                        .appearance(false)
                        .font_family(MONO)
                        .text_size(px(12.))
                        .h(px(18.))
                        .p_0(),
                )
                .into_any_element()
        } else {
            div()
                .text_color(p.fg4)
                .child("Only the main agent takes messages")
                .into_any_element()
        };
        let footer = div()
            .flex_none()
            .px(px(12.))
            .pb(px(7.))
            .font_family(MONO)
            .text_size(px(12.))
            .line_height(relative(1.5))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .py(px(5.))
                    .hair_t(w, p.pane_edge)
                    .hair_b(w, p.pane_edge)
                    .text_color(p.term_fg)
                    .child(div().text_color(p.term_dim).child("❯"))
                    .child(prompt),
            )
            .child(
                div()
                    .mt(px(4.))
                    .text_color(p.term_dim)
                    .truncate()
                    .child(rich(&[
                        (&format!("[{model}] "), None),
                        (&format!("▣ {name}"), Some(p.blue)),
                        (" | ", None),
                        (&format!("⎇ {branch}"), Some(p.green)),
                    ])),
            )
            .child(div().text_color(p.term_dim).truncate().child(rich(&[
                (&"█".repeat(filled), Some(p.green)),
                (&"░".repeat(10 - filled), Some(p.green)),
                (&format!(" {}% of spend | ", (share * 100.).round()), None),
                (&format!("${sub:.3}"), Some(p.amber)),
                (
                    &format!(
                        " | {}↓ {}↑",
                        thousands(a.input_tokens),
                        thousands(a.output_tokens)
                    ),
                    None,
                ),
            ])))
            .child(div().text_color(p.accent).truncate().child(hint));
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .id(("tb", slot))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scrolls[slot])
                    .pt(px(10.))
                    .px(px(12.))
                    .pb(px(6.))
                    .font_family(MONO)
                    .text_size(px(12.))
                    .line_height(relative(1.55))
                    .text_color(p.term_fg)
                    .child(banner)
                    .children(lines),
            )
            .child(footer)
    }

    fn files_body(
        &mut self,
        slot: usize,
        id: usize,
        w: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = self.pal;
        let filter = self.filters[slot].read(cx).value().to_lowercase();
        let label = match self.snap.agents.get(id).and_then(|a| a.ticket.clone()) {
            Some(k) => format!("{k} worktree"),
            None => "Primary worktree".into(),
        };
        let rows: Vec<(usize, String, PathBuf, bool)> = self
            .tree(id)
            .iter()
            .filter(|e| filter.is_empty() || e.name.to_lowercase().contains(&filter))
            .take(400)
            .map(|e| (e.depth, e.name.clone(), e.path.clone(), e.dir))
            .collect();
        let preview = self.file_preview[slot].clone();
        let ext_color = |name: &str, dir: bool| {
            if dir {
                p.fg3
            } else if name.ends_with(".html") || name.ends_with(".htm") {
                p.red
            } else if name.ends_with(".py") {
                p.blue
            } else if name.ends_with(".sql") {
                p.amber
            } else if name.ends_with(".md") {
                p.violet
            } else {
                p.fg
            }
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .p(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h(px(26.))
                            .rounded(px(6.))
                            .bg(p.hover)
                            .hair_all(w, p.pane_edge)
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .child(
                                Input::new(&self.filters[slot])
                                    .appearance(false)
                                    .text_size(px(12.))
                                    .h(px(24.))
                                    .p_0(),
                            ),
                    )
                    .child(
                        div()
                            .w(px(30.))
                            .h(px(24.))
                            .rounded(px(6.))
                            .hair_all(w, p.pane_edge)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.))
                            .text_color(p.fg3)
                            .child("Aa"),
                    ),
            )
            .child(
                div()
                    .id(("ftree", slot))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(6.))
                    .pb(px(8.))
                    .text_size(px(12.5))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .h(px(24.))
                            .px(px(6.))
                            .text_color(p.fg3)
                            .child(icon("chevd", 14., p.fg3))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(p.fg)
                                    .child(label),
                            ),
                    )
                    .children(
                        rows.into_iter()
                            .enumerate()
                            .map(|(n, (depth, name, path, dir))| {
                                let sel = preview.as_ref().is_some_and(|(pp, _)| *pp == path);
                                let c = ext_color(&name, dir);
                                div()
                                    .id(("ft", n))
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .h(px(24.))
                                    .pl(px(18. + depth as f32 * 14.))
                                    .pr(px(6.))
                                    .rounded(px(5.))
                                    .whitespace_nowrap()
                                    .cursor_pointer()
                                    .when(sel, |d| d.bg(p.sel))
                                    .hover(|d| d.bg(p.hover))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !dir {
                                            let text = std::fs::read(&path)
                                                .map(|b| {
                                                    String::from_utf8_lossy(
                                                        &b[..b.len().min(256 * 1024)],
                                                    )
                                                    .into_owned()
                                                })
                                                .unwrap_or_else(|e| e.to_string());
                                            this.file_preview[slot] = Some((path.clone(), text));
                                            cx.notify();
                                        }
                                    }))
                                    .child(icon(if dir { "chev" } else { "doc" }, 14., c))
                                    .child(name)
                            }),
                    ),
            )
            .when_some(preview, |d, (_, text)| {
                d.child(
                    div()
                        .id(("fprev", slot))
                        .max_h(relative(0.45))
                        .flex_none()
                        .overflow_y_scroll()
                        .hair_t(w, p.pane_edge)
                        .py(px(8.))
                        .px(px(12.))
                        .font_family(MONO)
                        .text_size(px(11.5))
                        .line_height(relative(1.5))
                        .text_color(p.term_fg)
                        .child(text),
                )
            })
    }

    // -------------------------------------------------------------- browser

    fn browser(&mut self, w: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let cur = self.previews.get(self.btab).cloned();
        let vps = [
            (Viewport::Desktop, "window"),
            (Viewport::Tablet, "doc"),
            (Viewport::Phone, "term"),
        ];
        let short = |u: &str| {
            u.trim_start_matches("http://")
                .trim_start_matches("https://")
                .to_string()
        };
        let tabs = div()
            .h(px(32.))
            .flex_none()
            .flex()
            .items_end()
            .gap(px(2.))
            .px(px(8.))
            .hair_b(w, p.pane_edge)
            .children(self.previews.iter().enumerate().map(|(i, u)| {
                let sel = Some(u) == cur.as_ref();
                div()
                    .id(("btab", i))
                    .h(px(26.))
                    .max_w(px(220.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(10.))
                    .rounded_t(px(7.))
                    .text_size(px(11.5))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .text_color(if sel { p.fg } else { p.fg3 })
                    .when(sel, |d| d.bg(p.hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.btab = i;
                        cx.notify()
                    }))
                    .child(
                        div()
                            .size(px(8.))
                            .rounded(px(2.))
                            .flex_none()
                            .bg(p.page_accent),
                    )
                    .child(div().truncate().child(short(u)))
            }))
            .child(
                ib("newtab", "plus", &p, false)
                    .mb(px(2.))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.btab = this.previews.len();
                        this.url.update(cx, |s, cx| {
                            s.set_value("", window, cx);
                            s.focus(window, cx)
                        });
                        cx.notify()
                    })),
            );
        let bar = div()
            .h(px(38.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .px(px(8.))
            .child(ib("bback", "back", &p, false))
            .child(ib("bfwd", "fwd", &p, false))
            .child(
                ib("breload", "reload", &p, false).on_click(cx.listener(|this, _, _, cx| {
                    if let Some(u) = this.previews.get(this.btab).cloned() {
                        this.fetch(u, cx)
                    }
                })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .px(px(10.))
                    .rounded(px(7.))
                    .bg(p.hover)
                    .hair_all(w, p.pane_edge)
                    .text_size(px(12.))
                    .child(icon("globe", 14., p.fg2))
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.url)
                                .appearance(false)
                                .text_size(px(12.))
                                .h(px(24.))
                                .p_0(),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .text_size(px(10.5))
                            .text_color(p.green)
                            .child(dot(p.green, None))
                            .child("live"),
                    ),
            )
            .child(div().flex().gap(px(2.)).children(vps.map(|(v, ic)| {
                ib(ic, ic, &p, self.viewport == v).on_click(cx.listener(move |this, _, _, cx| {
                    this.viewport = v;
                    cx.notify()
                }))
            })))
            .when(cur.is_some(), |d| {
                d.child(
                    ib("bclose", "close", &p, false).on_click(cx.listener(|this, _, _, cx| {
                        if this.btab < this.previews.len() {
                            this.previews.remove(this.btab);
                        }
                        this.btab = 0;
                        cx.notify()
                    })),
                )
            });
        let page = match &cur {
            None => div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .p(px(24.))
                .text_color(p.page_dim)
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.page_fg)
                        .child("No previews yet"),
                )
                .child(div().max_w(px(520.)).text_center().child(
                    "Type a dev server address above, for example the URL an agent printed after npm run dev, and press Enter.",
                ))
                .into_any_element(),
            Some(u) => match self.pages.get(u) {
                None => div()
                    .p(px(24.))
                    .text_color(p.page_dim)
                    .child(format!("Loading {u}…"))
                    .into_any_element(),
                Some(Err(e)) => div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .text_color(p.page_dim)
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.page_fg)
                            .child("Can't reach the page"),
                    )
                    .child(e.clone())
                    .into_any_element(),
                Some(Ok(pg)) => {
                    let url = u.clone();
                    div()
                        .id("page")
                        .size_full()
                        .overflow_y_scroll()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .py(px(8.))
                                .px(px(12.))
                                .text_size(px(12.))
                                .text_color(p.page_dim)
                                .border_b_1()
                                .border_color(p.page_line)
                                .child(format!(
                                    "Text snapshot · {} · {} ms · GPUI has no embedded web engine",
                                    pg.status, pg.ms
                                ))
                                .child(div().flex_1())
                                .child(
                                    div()
                                        .id("open-ext")
                                        .flex()
                                        .items_center()
                                        .gap(px(4.))
                                        .text_color(p.page_accent)
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            cx.open_url(&url)
                                        }))
                                        .child("Open in browser")
                                        .child(icon("ext", 12., p.page_accent)),
                                ),
                        )
                        .child(
                            div()
                                .max_w(px(560.))
                                .mx_auto()
                                .py(px(48.))
                                .px(px(24.))
                                .text_size(px(15.))
                                .line_height(relative(1.5))
                                .text_color(p.page_fg)
                                .child(
                                    div()
                                        .text_size(px(30.))
                                        .line_height(relative(1.2))
                                        .font_weight(FontWeight::BOLD)
                                        .mb(px(14.))
                                        .child(pg.title.clone()),
                                )
                                .children(
                                    pg.text.iter().map(|l| div().mb(px(6.)).child(l.clone())),
                                ),
                        )
                        .into_any_element()
                }
            },
        };
        let frame = div().h_full().bg(p.page_bg).child(page);
        let frame = match self.viewport {
            Viewport::Desktop => frame.w_full(),
            Viewport::Tablet => frame.w(px(768.)),
            Viewport::Phone => frame.w(px(390.)),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .rounded(px(9.))
            .bg(p.pane)
            .hair_all(w, p.pane_edge)
            .overflow_hidden()
            .child(tabs)
            .child(bar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .justify_center()
                    .bg(p.page_desk)
                    .child(frame),
            )
            .into_any_element()
    }

    // -------------------------------------------------------------- diagram

    fn diagram_env(
        &mut self,
        w: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.pal;
        let s = self.snap.clone();
        if self.review.is_none_or(|r| r >= s.approvals.len()) {
            self.review = s
                .pending_approvals()
                .next()
                .map(|a| a.id)
                .or(s.approvals.last().map(|a| a.id));
        }
        let ap = self.review.and_then(|r| s.approvals.get(r)).cloned();
        let label = |a: &Approval| match a.kind {
            ApprovalKind::Plan => format!("Plan · {} tickets", a.tickets.len()),
            ApprovalKind::Deliverable if a.agent == MAIN => "Final deliverable".into(),
            ApprovalKind::Deliverable => format!("Ticket · {}", a.tickets.join(", ")),
        };
        let list = div()
            .id("dg-list")
            .w(px(250.))
            .flex_none()
            .h_full()
            .rounded(px(9.))
            .bg(p.pane)
            .hair_all(w, p.pane_edge)
            .p(px(8.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .mt(px(4.))
                    .mx(px(6.))
                    .mb(px(6.))
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.fg3)
                    .child("Reviews · diagram first"),
            )
            .when(s.approvals.is_empty(), |d| {
                d.child(nothing(&p, "Nothing has needed your review yet."))
            })
            .children(s.approvals.iter().rev().map(|a| {
                let sel = Some(a.id) == self.review;
                let id = a.id;
                let (chip, c, edge) = match a.state {
                    ApprovalState::Pending => ("pending", p.amber, p.amber),
                    ApprovalState::Approved => ("approved", p.green, p.green),
                    ApprovalState::Rejected { .. } => ("rejected", p.fg4, p.pane_edge),
                };
                let kind = match a.kind {
                    ApprovalKind::Plan => "PLAN",
                    _ if a.agent == MAIN => "FINAL",
                    _ => "TICKET",
                };
                div()
                    .id(("rv", id))
                    .py(px(8.))
                    .px(px(9.))
                    .rounded(px(7.))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .cursor_pointer()
                    .hair_all(
                        w,
                        if sel {
                            p.pane_edge
                        } else {
                            transparent_black()
                        },
                    )
                    .when(sel, |d| d.bg(p.sel))
                    .hover(|d| d.bg(if sel { p.sel } else { p.hover }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.review = Some(id);
                        this.cam = None;
                        this.user_cam = false;
                        cx.notify()
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(10.5))
                            .text_color(p.fg3)
                            .child(icon("diagram", 14., p.fg3))
                            .child(kind),
                    )
                    .child(div().font_weight(FontWeight::MEDIUM).child(
                        if a.kind == ApprovalKind::Plan {
                            label(a)
                        } else {
                            s.agents[a.agent].title.clone()
                        },
                    ))
                    .child(
                        div().flex().child(
                            div()
                                .text_size(px(10.5))
                                .line_height(px(14.))
                                .px(px(7.))
                                .py(px(1.))
                                .rounded_full()
                                .hair_all(w, edge)
                                .text_color(c)
                                .child(chip),
                        ),
                    )
            }));

        let Some(ap) = ap else {
            return div()
                .size_full()
                .flex()
                .gap(px(6.))
                .child(list)
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .rounded(px(9.))
                        .bg(p.pane)
                        .hair_all(w, p.pane_edge)
                        .child(empty_mark(
                            &p,
                            "diagram",
                            "Plans and deliverables appear here, drawn by the harness before you read them.",
                        )),
                )
                .into_any_element();
        };
        let d: Diagram = diagram::for_approval(&s, &ap);

        // Fit to the stage (and the card over it) until the user pans or zooms.
        let sb = self.stage_bounds.get();
        let (sw, sh) = (f32::from(sb.size.width), f32::from(sb.size.height));
        let card_h = f32::from(self.card_bounds.get().size.height);
        let key = (ap.id, d.nodes.len(), sw, sh, card_h);
        if sw > 0. && (self.cam.is_none() || (!self.user_cam && self.fitted != Some(key))) {
            self.cam = Some(stage::fit(&d, sw, sh, card_h));
            self.fitted = Some(key);
        }
        if sw == 0. || card_h == 0. {
            // Sizes arrive with the first paint; draw once more with them.
            cx.on_next_frame(window, |_, _, cx| cx.notify());
        }
        let cam = self.cam.unwrap_or(Cam {
            x: 0.,
            y: 0.,
            k: 1.,
        });

        let facts = div()
            .absolute()
            .top(px(10.))
            .left(px(12.))
            .right(px(120.))
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .child(chip_f(&p, w, true).child(d.title.clone()))
            .children(d.facts.iter().map(|(k, v)| {
                chip_f(&p, w, false).child(
                    div()
                        .flex()
                        .child(
                            div()
                                .font_family(MONO)
                                .font_weight(FontWeight::MEDIUM)
                                .child(v.clone()),
                        )
                        .child(format!(" {k}")),
                )
            }));
        let zoomc = div()
            .absolute()
            .top(px(10.))
            .right(px(10.))
            .flex()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(8.))
            .bg(p.glass)
            .hair_all(w, p.pane_edge)
            .child(
                ib_with("zout", "−", &p, false)
                    .on_click(cx.listener(|this, _, _, cx| this.zoom_center(1.25, cx))),
            )
            .child(
                ib_with("zin", "+", &p, false)
                    .on_click(cx.listener(|this, _, _, cx| this.zoom_center(0.8, cx))),
            )
            .child(
                ib("zfit", "expand", &p, false).on_click(cx.listener(|this, _, _, cx| {
                    this.cam = None;
                    this.user_cam = false;
                    cx.notify()
                })),
            );

        let st = ap.state.clone();
        let id = ap.id;
        let agent = ap.agent;
        let cb = self.card_bounds.clone();
        let actions = if st == ApprovalState::Pending {
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(
                    div()
                        .min_h(px(52.))
                        .rounded(px(8.))
                        .hair_all(w, p.pane_edge)
                        .bg(p.hover)
                        .py(px(4.))
                        .px(px(9.))
                        .child(
                            Input::new(&self.feedback)
                                .appearance(false)
                                .text_size(px(12.5))
                                .p_0(),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(8.))
                        .items_center()
                        .child(
                            btn("appr", "Approve", &p, w)
                                .bg(p.fg)
                                .text_color(p.desk_base)
                                .border_color(transparent_black())
                                .on_click(
                                    cx.listener(move |this, _, _, _| this.harness.approve(id)),
                                ),
                        )
                        .child(
                            btn("rej", "Reject", &p, w)
                                .text_color(p.red)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let fb = this.feedback.read(cx).value().trim().to_string();
                                    if fb.is_empty() {
                                        this.fb_err = true;
                                    } else {
                                        this.harness.reject(id, fb);
                                        this.feedback
                                            .update(cx, |s, cx| s.set_value("", window, cx));
                                    }
                                    cx.notify()
                                })),
                        )
                        .child(btn("sess", "Session", &p, w).on_click(
                            cx.listener(move |this, _, _, cx| this.open(Slot::Agent(agent), cx)),
                        ))
                        .when(self.fb_err, |d| {
                            d.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.red)
                                    .child("Write what should change first."),
                            )
                        }),
                )
        } else {
            let (txt, c) = match &st {
                ApprovalState::Rejected { feedback } => (format!("rejected: {feedback}"), p.fg4),
                _ => ("approved".to_string(), p.green),
            };
            div()
                .flex()
                .gap(px(8.))
                .items_center()
                .child(
                    div()
                        .text_size(px(10.5))
                        .px(px(7.))
                        .py(px(1.))
                        .rounded_full()
                        .hair_all(w, c)
                        .text_color(c)
                        .child(txt),
                )
                .child(
                    btn("sess", "Session", &p, w).on_click(
                        cx.listener(move |this, _, _, cx| this.open(Slot::Agent(agent), cx)),
                    ),
                )
        };
        let card = div()
            .id("rcard")
            .absolute()
            .right(px(12.))
            .bottom(px(12.))
            // CSS width is content-box: 360 + 2x14 padding + hairlines.
            .w(px(360. + 28. + 2. * f32::from(w)))
            .max_h(relative(0.9))
            .overflow_y_scroll()
            .p(px(14.))
            .rounded(px(12.))
            .bg(p.glass)
            .hair_all(w, p.pane_edge)
            .shadow(vec![BoxShadow {
                color: hsla(0., 0., 0., 0.14),
                offset: point(px(0.), px(12.)),
                blur_radius: px(30.),
                spread_radius: px(0.),
                inset: false,
            }])
            .flex()
            .flex_col()
            .gap(px(10.))
            .cursor_default()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                canvas(move |b, _, _| cb.set(b), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .child(label(&ap)),
            )
            .child(
                div()
                    .text_color(p.fg2)
                    .line_height(relative(1.5))
                    .child(pre_line(&ap.deliverable.summary)),
            )
            .when(!d.warnings.is_empty(), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .children(d.warnings.iter().map(|wn| {
                            div()
                                .flex()
                                .text_size(px(12.))
                                .text_color(p.amber)
                                .child(
                                    div()
                                        .w(px(14.))
                                        .flex_none()
                                        .font_weight(FontWeight::BOLD)
                                        .child("!"),
                                )
                                .child(div().flex_1().child(wn.clone()))
                        })),
                )
            })
            .when(!d.files.is_empty(), |el| {
                el.child(stage::file_bars(&d.files, &p, MONO))
            })
            .child(actions);

        let stage_el = div()
            .id("stage")
            .flex_1()
            .min_w_0()
            .h_full()
            .relative()
            .rounded(px(9.))
            .bg(p.pane)
            .hair_all(w, p.pane_edge)
            .overflow_hidden()
            .cursor(if self.pan.is_some() {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            })
            .child(stage::layer(
                &d,
                cam,
                &p,
                UI.into(),
                MONO,
                self.stage_bounds.clone(),
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, _| this.pan = Some(e.position)),
            )
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                let dy = f32::from(e.delta.pixel_delta(px(16.)).y);
                let b = this.stage_bounds.get();
                let at = e.position - b.origin;
                if let Some(cam) = this.cam.as_mut() {
                    cam.zoom(
                        (-dy * 0.0015).exp(),
                        f32::from(at.x),
                        f32::from(at.y),
                        f32::from(b.size.width),
                    );
                    this.user_cam = true;
                    cx.notify();
                }
            }))
            .child(facts)
            .child(zoomc)
            .child(card);
        div()
            .size_full()
            .flex()
            .gap(px(6.))
            .child(list)
            .child(stage_el)
            .into_any_element()
    }

    // -------------------------------------------------------------- docs + new

    fn docs(&mut self, w: Pixels) -> AnyElement {
        let p = self.pal;
        let text = self.plan_text();
        let mut body: Vec<AnyElement> = vec![];
        match text {
            None => {
                body.push(h1(&p, "PLAN.md"));
                body.push(
                    div()
                        .text_size(px(12.5))
                        .text_color(p.fg3)
                        .mb(px(26.))
                        .child(self.snap.name.clone())
                        .into_any_element(),
                );
                body.push(para(
                    &p,
                    "The main agent writes PLAN.md before it proposes tickets. It will show here once it exists.",
                ));
            }
            Some(src) => {
                let mut code: Option<Vec<String>> = None;
                for raw in src.lines() {
                    let l = raw.trim_end();
                    if l.starts_with("```") {
                        match code.take() {
                            Some(lines) => body.push(
                                div()
                                    .my(px(8.))
                                    .py(px(10.))
                                    .px(px(12.))
                                    .rounded(px(8.))
                                    .bg(p.hover)
                                    .font_family(MONO)
                                    .text_size(px(12.5))
                                    .line_height(relative(1.5))
                                    .child(lines.join("\n"))
                                    .into_any_element(),
                            ),
                            None => code = Some(vec![]),
                        }
                        continue;
                    }
                    if let Some(c) = code.as_mut() {
                        c.push(l.to_string());
                        continue;
                    }
                    let plain = l.replace("**", "").replace('`', "");
                    if let Some(h) = plain.strip_prefix("# ") {
                        body.push(h1(&p, h));
                    } else if let Some(h) = plain.strip_prefix("## ").or(plain.strip_prefix("### "))
                    {
                        body.push(
                            div()
                                .mt(px(26.))
                                .mb(px(8.))
                                .text_size(px(15.))
                                .font_weight(FontWeight::BOLD)
                                .child(h.to_string())
                                .into_any_element(),
                        );
                    } else if let Some(li) = plain.strip_prefix("- ").or(plain.strip_prefix("* ")) {
                        body.push(
                            div()
                                .flex()
                                .pl(px(6.))
                                .child(div().w(px(14.)).flex_none().child("•"))
                                .child(div().flex_1().child(li.to_string()))
                                .into_any_element(),
                        );
                    } else if !plain.trim().is_empty() {
                        body.push(para(&p, &plain));
                    }
                }
            }
        }
        div()
            .id("docs")
            .size_full()
            .overflow_y_scroll()
            .rounded(px(9.))
            .bg(p.pane)
            .hair_all(w, p.pane_edge)
            .child(
                div()
                    .max_w(px(680.))
                    .mx_auto()
                    .pt(px(40.))
                    .px(px(28.))
                    .pb(px(60.))
                    .text_size(px(14.))
                    .line_height(relative(1.6))
                    .children(body),
            )
            .into_any_element()
    }

    fn new_env(&mut self, w: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let card = |id: &'static str, ic: &'static str, t: &'static str, s: &'static str| {
            div()
                .id(id)
                .w(px(168. + 24. + 2. * f32::from(w)))
                .p(px(12.))
                .rounded(px(9.))
                .bg(p.pane)
                .hair_all(w, p.pane_edge)
                .flex()
                .flex_col()
                .gap(px(6.))
                .cursor_pointer()
                .hover(|d| d.border_color(p.pane_edge_on))
                .child(icon(ic, 14., p.fg))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_size(px(12.5))
                        .child(t),
                )
                .child(div().text_size(px(11.5)).text_color(p.fg3).child(s))
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(26.))
            .p(px(20.))
            .child(
                div()
                    .size(px(54.))
                    .rounded(px(14.))
                    .bg(p.hover)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("bksp", 26., p.fg3)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(10.))
                    .child(
                        card(
                            "n-goal",
                            "sparkle",
                            "New goal",
                            "Tell the main agent what to build",
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open(Slot::Agent(MAIN), cx);
                            this.composer.update(cx, |s, cx| s.focus(window, cx));
                        })),
                    )
                    .child(
                        card(
                            "n-ticket",
                            "ticket",
                            "File a ticket",
                            "Triaged, then scheduled by the main agent",
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.ticket_form = true;
                            this.ticket.update(cx, |s, cx| s.focus(window, cx));
                            cx.notify()
                        })),
                    )
                    .child(
                        card(
                            "n-files",
                            "folder",
                            "Open worktree",
                            "Browse the project's files",
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.open(Slot::Files(MAIN), cx))),
                    ),
            )
            .when(self.ticket_form, |d| {
                d.child(
                    div()
                        .w(px(520.))
                        .h(px(36.))
                        .px(px(12.))
                        .rounded(px(9.))
                        .bg(p.pane)
                        .hair_all(w, p.pane_edge)
                        .flex()
                        .items_center()
                        .child(
                            Input::new(&self.ticket)
                                .appearance(false)
                                .text_size(px(13.))
                                .p_0(),
                        ),
                )
            })
            .into_any_element()
    }
}

fn chip_f(p: &Pal, w: Pixels, title: bool) -> Div {
    div()
        .text_size(px(11.5))
        .px(px(9.))
        .py(px(3.))
        .rounded_full()
        .bg(p.glass)
        .hair_all(w, p.pane_edge)
        .when(title, |d| d.font_weight(FontWeight::SEMIBOLD))
}

fn nothing(p: &Pal, s: &str) -> AnyElement {
    div()
        .p(px(14.))
        .text_size(px(12.5))
        .text_color(p.fg3)
        .child(s.to_string())
        .into_any_element()
}

fn empty_mark(p: &Pal, ic: &str, s: &str) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(26.))
        .child(
            div()
                .size(px(54.))
                .rounded(px(14.))
                .bg(p.hover)
                .flex()
                .items_center()
                .justify_center()
                .child(icon(ic, 26., p.fg3)),
        )
        .child(nothing(p, s))
}

fn h1(p: &Pal, s: &str) -> AnyElement {
    div()
        .text_size(px(26.))
        .line_height(relative(1.2))
        .font_weight(FontWeight::BOLD)
        .mb(px(4.))
        .text_color(p.fg)
        .child(s.to_string())
        .into_any_element()
}

fn para(p: &Pal, s: &str) -> AnyElement {
    div()
        .my(px(10.))
        .text_color(p.fg)
        .child(s.to_string())
        .into_any_element()
}

fn note(p: &Pal, s: &str) -> AnyElement {
    hang("※ ", div().italic().child(s.to_string()), p.term_dim).into_any_element()
}

/// A line with a two-cell glyph gutter (CSS `padding-left: 2ch; text-indent: -2ch`).
fn hang(glyph: &str, body: impl IntoElement, c: Hsla) -> Div {
    div()
        .flex()
        .items_start()
        .text_color(c)
        .child(
            div()
                .w(px(CH2))
                .flex_none()
                .whitespace_nowrap()
                .child(glyph.trim_end().to_string()),
        )
        .child(div().flex_1().min_w_0().child(body))
}

fn log_el(kind: LogKind, text: &str, p: &Pal) -> AnyElement {
    // `white-space: pre-wrap` drops a block's final newline; gpui would draw it.
    let text = text.strip_suffix('\n').unwrap_or(text);
    match kind {
        LogKind::User => div()
            .px(px(6.))
            .rounded(px(2.))
            .bg(p.prompt_bg)
            .text_color(p.prompt_fg)
            .font_weight(FontWeight::MEDIUM)
            .child(format!("❯ {text}"))
            .into_any_element(),
        LogKind::Assistant => hang("● ", text.to_string(), p.term_fg).into_any_element(),
        LogKind::ToolCall => {
            let (name, rest) = text.split_once(' ').unwrap_or((text, ""));
            hang(
                "⎿ ",
                rich(&[(name, Some(p.term_fg)), (&format!(" {rest}"), None)]),
                p.term_dim,
            )
            .into_any_element()
        }
        LogKind::ToolResult => {
            let lines: Vec<&str> = text.lines().collect();
            let body = if lines.len() > 6 {
                format!(
                    "{}\n… {} more lines",
                    lines[..6].join("\n"),
                    lines.len() - 6
                )
            } else {
                text.to_string()
            };
            if text.starts_with("CHECK FAILED") || text.starts_with("REJECTED") {
                hang("✗ ", body, p.red).into_any_element()
            } else if text.starts_with("Approved")
                || text.starts_with("Plan APPROVED")
                || text.starts_with("Merged")
            {
                hang("✓ ", body, p.green).into_any_element()
            } else {
                div()
                    .pl(px(CH2))
                    .text_color(p.term_dim)
                    .child(body)
                    .into_any_element()
            }
        }
        LogKind::System if text.starts_with("escalated") => {
            hang("↑ ", text.to_string(), p.amber).into_any_element()
        }
        LogKind::System => note(p, text),
        LogKind::Error => hang("✗ ", text.to_string(), p.red).into_any_element(),
    }
}

/// (margin-top, margin-bottom) of each transcript line, from the stylesheet.
fn log_margins(kind: LogKind, text: &str) -> (f32, f32) {
    match kind {
        LogKind::User => (8., 8.),
        LogKind::Assistant => (8., 0.),
        LogKind::ToolCall => (6., 0.),
        LogKind::ToolResult
            if text.starts_with("Approved")
                || text.starts_with("Plan APPROVED")
                || text.starts_with("Merged") =>
        {
            (6., 0.)
        }
        LogKind::System if text.starts_with("escalated") => (6., 0.),
        LogKind::System => (8., 0.),
        _ => (0., 0.),
    }
}

/// CSS `white-space: pre-line`: keep line breaks, collapse runs of spaces.
fn pre_line(s: &str) -> String {
    s.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Depth-first walk, skipping build output and VCS internals.
fn scan(dir: &Path, depth: usize, out: &mut Vec<Entry>) {
    if depth > 4 || out.len() > 2000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<_> = rd.flatten().collect();
    items.sort_by_key(|e| (!e.path().is_dir(), e.file_name()));
    for e in items {
        let name = e.file_name().to_string_lossy().into_owned();
        if matches!(
            name.as_str(),
            ".git" | "target" | "node_modules" | ".backspace" | "dist" | ".astro"
        ) {
            continue;
        }
        let path = e.path();
        let dir = path.is_dir();
        out.push(Entry {
            depth,
            name,
            path: path.clone(),
            dir,
        });
        if dir {
            scan(&path, depth + 1, out);
        }
    }
}

/// Depth-first, so each agent sits directly under its manager.
fn tree_order(s: &ProjectState) -> Vec<usize> {
    fn walk(s: &ProjectState, id: usize, out: &mut Vec<usize>) {
        out.push(id);
        for child in s.agents.iter().filter(|a| a.parent == Some(id)) {
            walk(s, child.id, out);
        }
    }
    let mut out = Vec::new();
    if !s.agents.is_empty() {
        walk(s, MAIN, &mut out);
    }
    out
}

/// A plain-HTTP GET of a dev server, reduced to its title and visible text.
fn probe(url: &str) -> Result<Page, String> {
    let start = Instant::now();
    let rest = url.strip_prefix("http://").ok_or(
        "Only http:// dev servers can be previewed here; open https pages in your browser.",
    )?;
    let (host, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_string()), |(h, p)| (h, format!("/{p}")));
    let addr = if host.contains(':') {
        host.to_string()
    } else {
        format!("{host}:80")
    };
    let mut s = std::net::TcpStream::connect(&addr).map_err(|e| format!("{addr}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok();
    write!(
        s,
        "GET {path} HTTP/1.0\r\nHost: {host}\r\nUser-Agent: backspace\r\n\r\n"
    )
    .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    s.take(2 << 20)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    let raw = String::from_utf8_lossy(&buf).into_owned();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_once(' '))
        .map_or(String::new(), |(_, s)| format!("HTTP {s}"));
    // Lowercasing ASCII keeps byte offsets aligned with `body`.
    let lower = body.to_ascii_lowercase();
    let title = lower
        .find("<title>")
        .and_then(|i| {
            lower[i..]
                .find("</title>")
                .map(|j| body[i + 7..i + j].trim().to_string())
        })
        .or_else(|| {
            let i = lower.find("<h1")?;
            let gt = lower[i..].find('>')? + i + 1;
            let end = lower[gt..].find("</h1>")? + gt;
            Some(strip_tags(&body[gt..end]))
        })
        .unwrap_or_else(|| url.to_string());
    // Visible text: drop scripts, styles and the title; break on block tags.
    let mut text = String::new();
    let mut i = 0;
    while i < body.len() {
        if body.as_bytes()[i] == b'<' {
            let end = lower[i..].find('>').map_or(body.len(), |j| i + j + 1);
            let tag = &lower[i..end];
            let mut next = end;
            for skip in ["script", "style", "title"] {
                if tag.starts_with(&format!("<{skip}")) {
                    let close = format!("</{skip}>");
                    next = lower[end..]
                        .find(&close)
                        .map_or(body.len(), |j| end + j + close.len());
                }
            }
            let block = [
                "<p", "<li", "<div", "<h", "<br", "<tr", "</p", "</li", "</div", "</h", "<ul",
                "</ul",
            ];
            if block.iter().any(|t| tag.starts_with(t)) {
                text.push('\n');
            }
            i = next;
        } else {
            let next = lower[i..].find('<').map_or(body.len(), |j| i + j);
            text.push_str(&body[i..next]);
            i = next;
        }
    }
    let lines: Vec<String> = text
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|l| {
            l.replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&nbsp;", " ")
        })
        .filter(|l| !l.is_empty() && *l != title)
        .take(200)
        .collect();
    Ok(Page {
        status,
        title,
        text: lines,
        ms: start.elapsed().as_millis(),
    })
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            c if !tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn main() -> anyhow::Result<()> {
    let ws = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    let harness = Harness::open(ws)?;
    // Scripted runs (bench/) hand the goal over without typing it.
    if let Ok(goal) = std::env::var("BACKSPACE_GOAL") {
        harness.send(goal);
    }

    gpui_kit::application()
        .with_assets(icons::Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            if let Err(e) = cx.text_system().add_fonts(bundled_fonts()) {
                eprintln!("bundled fonts: {e}");
            }
            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(720.), px(480.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Backspace".into()),
                        appears_transparent: GLASS,
                        traffic_light_position: GLASS.then(|| point(px(14.), px(13.))),
                    }),
                    window_background: if GLASS {
                        WindowBackgroundAppearance::Blurred
                    } else {
                        WindowBackgroundAppearance::Opaque
                    },
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| Workbench::new(harness, window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("failed to open window");
            cx.activate(true);
        });
    Ok(())
}
