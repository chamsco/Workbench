//! The Cauldron shell (UI1): the frame, top bar, sidebar, main card with tabs,
//! right rail, drawer and split. Every rectangle comes from `cauldron_chrome::layout::solve`,
//! which is tested against numbers measured from the wireframe; nothing here picks a position.
//!
//! Content inside the panes is a labelled placeholder until the milestone that owns it
//! (docs/cauldron/UI-PLAN.md): the thread list is UI3, the thread view UI4, the drawer UI6.

use cauldron_chrome::layout::{self, metric, Platform, Rect, ShellState};
use cauldron_chrome::motion::Frame;
use cauldron_chrome::panes::{Shape, Split};
use cauldron_chrome::tokens::{self, motion as mt, radius, Palette, Rgb, Rgba};
use gpui_kit::*;

use crate::motion::ShellMotion;
use crate::threads::{self, Group};
use crate::ui::{sans_features, SANS};
use backspace_core::chat::{Chats, Thread, ThreadInfo};
use backspace_core::prefs::Prefs;
use gpui_kit::component::input::{Input, InputEvent, InputState, TextareaState};
use gpui_kit::component::Theme;
use std::sync::Arc;

mod motion;
mod newthread;
mod appearance;
mod backdrop;
mod pages;
mod panes;
mod ptabs;
mod settings;
mod terminal;
mod thread;
#[cfg(all(feature = "shots", target_os = "macos"))]
pub(crate) mod shots;

/// Where the shell draws window controls: lights on macOS, caption buttons elsewhere.
pub(crate) fn platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Mac
    } else {
        Platform::Windows
    }
}

/// Top-left of the traffic lights, from the wireframe (`layout::layout` Reference lights rect).
pub(crate) use motion::os_reduces_motion;

pub(crate) fn lights_origin() -> (f32, f32) {
    let s = layout::layout(1240.0, 702.0, ShellState { sidebar_open: true, drawer_open: false, split_open: false, terminal_open: false, plugins: true }, Platform::Mac);
    let l = s.lights.expect("mac layout has lights");
    (l.x, l.y)
}

/// A token colour for GPUI; overlays keep their alpha and composite over what is beneath.
fn c(v: impl Into<Rgba>) -> Hsla {
    let Rgba(v, a) = v.into();
    let mut h: Hsla = gpui_kit::rgb(((v.0 as u32) << 16) | ((v.1 as u32) << 8) | v.2 as u32).into();
    h.a = a;
    h
}

fn ca(v: Rgb, a: f32) -> Hsla {
    let mut h = c(v);
    h.a = a;
    h
}

fn abs(r: Rect) -> Div {
    div().absolute().left(px(r.x)).top(px(r.y)).w(px(r.w)).h(px(r.h))
}

/// Windows' tab strip over pane `slot`.
fn pane_strip(slot: usize, r: Rect) -> Stateful<Div> {
    abs(r).id(("pane-strip", slot)).flex().items_center().overflow_hidden()
}

/// A pane the layout tests find by `id` and compare with `layout::solve`.
fn pane(id: &'static str, r: Rect) -> Stateful<Div> {
    abs(r).id(id)
}

fn glyph(name: &str, size: f32, color: Hsla) -> Svg {
    svg().path(format!("icons/{name}.svg")).size(px(size)).flex_none().text_color(color)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Panel {
    Highlights,
    Usage,
    Koi,
    Review,
    Patches,
}

impl Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Highlights => "Highlights",
            Panel::Usage => "Usage",
            Panel::Koi => "Koi preview",
            Panel::Review => "Code review",
            Panel::Patches => "Patches",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Panel::Highlights => "pin",
            Panel::Usage => "gauge",
            Panel::Koi => "palette",
            Panel::Review => "diff",
            Panel::Patches => "layout-grid",
        }
    }
    /// Every drawer panel is UI6.
    fn milestone(self) -> &'static str {
        let _ = self;
        "UI6"
    }
}

/// The apps, listed in Settings > Apps (Charles, 13 Nov 07:34Z: out of the sidebar, into Settings).
pub(super) const FUNCTIONS: [(&str, &str); 7] = [
    ("message-square", "Streams"),
    ("folder", "Projects"),
    ("calendar", "Schedule"),
    ("bot", "Agents"),
    ("gauge", "Usage"),
    ("database", "Memory"),
    ("layout-grid", "Apps"),
];

const NEW_TAB: &str = "New stream";
/// A pane's header, and the card's head: tabs on the left, the controls on the right.
pub(super) const HEAD_H: f32 = 40.0;

const TOP_PANELS: [Panel; 4] = [Panel::Highlights, Panel::Usage, Panel::Koi, Panel::Review];

/// The sidebar's threads from core: still opening, opened, or core failed to open.
pub(crate) enum Threads {
    Loading,
    Ready(Vec<ThreadInfo>),
    Error(SharedString),
}

pub(crate) struct Cauldron {
    sidebar_open: bool,
    drawer: Option<Panel>,
    /// Background A: the new thread card's painting and the glass cut from it, cached by size and rect.
    backdrop: std::cell::RefCell<backdrop::Backdrop>,
    /// Where the glass elements were laid out last frame (window coordinates), and the painted card's
    /// top-left; the glass is cut at those rects.
    glass_at: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<&'static str, Bounds<Pixels>>>>,
    glass_card: std::cell::Cell<Option<(f32, f32, bool)>>,
    /// Re-renders when the OS switches light and dark, so System follows it live.
    appearance_sub: Option<Subscription>,
    /// Where the theme and the painting are read from; None is core's prefs file (tests use their own).
    prefs_file: Option<std::path::PathBuf>,
    /// Which window controls the shell draws: the OS's, or Windows' drawn on a Mac for a preview
    /// (debug `CAULDRON_PLATFORM=windows`).
    plat: Platform,
    /// Folders the user opened or closed by hand, by name (the session's choice wins over the
    /// default: open when the folder holds running or waiting work).
    folder_open: std::collections::HashMap<String, bool>,
    /// Which app fills the window: Streams, Agents or Memory (Charles, 13 Nov 13:40Z).
    page: pages::AppPage,
    /// Core's agents, read when Agents opens.
    page_agents: Vec<pages::AgentCard>,
    /// The Agents demo's selection: an agent in the roster, or None for the first room.
    agent_sel: Option<usize>,
    /// Settings is open on this page (v3 frame 7): its pages take the sidebar's list, the page the card.
    settings: Option<usize>,
    /// Where Settings reads the subscriptions; None is core's `accounts.json` (tests use their own).
    accounts_file: Option<std::path::PathBuf>,
    /// The terminal card is open, in the drawer's place (v3 frame 5).
    terminal: bool,
    /// Its blocks: the open thread's commands, from core's traces.
    term_blocks: Vec<terminal::Block>,
    term_input: Option<(Entity<InputState>, Subscription)>,
    /// What the input line answered last (no shell yet, or a send error).
    term_note: Option<String>,
    term_scroll: ScrollHandle,
    /// The thread card's panes (direction B): the shape and where its two lines sit.
    panes: Split,
    /// The layout button's morph: the shape it drew before, and since when.
    layout_since: Option<(Shape, std::time::Instant)>,
    /// A line held down, and the panes' area when it was grabbed.
    pane_drag: Option<(cauldron_chrome::panes::Line, Rect)>,
    /// What each pane shows, in pane order; the thread is always one of them.
    pane_content: [panes::Content; cauldron_chrome::panes::MAX],
    /// Each pane's focus, so a new pane's keys (B, T, F, ...) open its surfaces.
    pane_focus: Vec<FocusHandle>,
    /// The thread's room (width, height), so the thread view can drop what does not fit.
    thread_box: std::cell::Cell<(f32, f32)>,
    /// The transcript's scroll, kept at the newest message when a thread opens or grows.
    transcript_scroll: ScrollHandle,
    transcript_seen: std::cell::RefCell<Option<(String, usize)>>,
    /// The thread's coordinator, pinned as the first tab; None on the New stream screen.
    coordinator: Option<SharedString>,
    /// Agents with an open 1:1 tab, in tab order, and the thread's other agents (the + menu).
    agents: Vec<AgentTab>,
    idle: Vec<AgentTab>,
    /// The open tab: 0 is the pinned one, i is `agents[i - 1]`.
    active: usize,
    /// Scroll the open tab into view at the next render (set when it changes).
    reveal: bool,
    tab_menu: bool,
    tab_slide: motion::Slide,
    /// The shell's motion clock, the drawer's panel crossfade (from, when), the drawer's last
    /// panel (kept while it closes).
    motion: ShellMotion,
    swap: Option<(Panel, std::time::Instant)>,
    /// When the reply last started or stopped (the Send and Stop glyphs cross-fade from then), and
    /// whether the last render saw one running.
    action_since: Option<std::time::Instant>,
    busy_shown: bool,
    /// When the open thread was opened (its transcript fades in), when a reply bubble of the
    /// user's last appeared, and how many of the user's bubbles the open thread had at the last render.
    open_since: Option<std::time::Instant>,
    bubble_since: Option<std::time::Instant>,
    mine_for: Option<String>,
    mine_n_shown: usize,
    drawer_last: Option<Panel>,
    /// The icon button under the pointer, whether its tip is showing yet, and until when a
    /// neighbour's tip may show at once (the wireframe's `.rail.warm`).
    tip: Option<ElementId>,
    tip_shown: bool,
    tip_warm_until: Option<std::time::Instant>,
    tip_wait: Option<Task<()>>,
    /// The sidebar: core's threads, the selected row (selecting does not open or mark read), the
    /// search text and box, and the list's keyboard focus.
    threads: Threads,
    selected: Option<String>,
    query: String,
    search: Option<(Entity<InputState>, Subscription)>,
    list_focus: Option<FocusHandle>,
    /// Core's thread store and the runtime its replies run on. Only the store: the scheduled
    /// routines, dreams and relay of `Fleet` are not started by this window.
    core: Option<(tokio::runtime::Runtime, Arc<Chats>)>,
    watch: Option<Task<()>>,
    /// The thread open in the main card (UI4), as core last had it, the composer, and why the
    /// last send was refused.
    open: Option<String>,
    thread: Option<Thread>,
    /// How each finished reply of the open thread was made (thread.rs `steps`).
    steps: std::collections::HashMap<String, Vec<String>>,
    composer: Option<(Entity<TextareaState>, Subscription)>,
    send_error: Option<String>,
    /// The New stream screen (UI5).
    start: newthread::Start,
}

impl Cauldron {
    /// The window's first state. Debug builds only: `CAULDRON_FIXTURE_AGENTS=n` opens a thread with
    /// n agent tabs, for screenshots until UI4 brings real agents from core.
    pub(crate) fn launch(cx: &mut Context<Self>) -> Self {
        let mut c = Self::new();
        // Debug builds only: `CAULDRON_FIXTURE_THREADS=list|loading|empty|error` draws the sidebar
        // from made-up threads instead of core, for screenshots that hold no real thread titles.
        #[cfg(debug_assertions)]
        let fixture = std::env::var("CAULDRON_FIXTURE_THREADS").ok();
        #[cfg(not(debug_assertions))]
        let fixture: Option<String> = None;
        match fixture.as_deref() {
            #[cfg(debug_assertions)]
            Some(f) => {
                c.threads = match f {
                    "loading" => Threads::Loading,
                    "empty" => Threads::Ready(Vec::new()),
                    "error" => Threads::Error("the data folder is not readable".into()),
                    _ => Threads::Ready(threads::fixture(now_ms())),
                };
                if f == "list" {
                    c.selected = Some("f2".into());
                }
            }
            _ => c.connect(cx),
        }
        // Debug builds only: `CAULDRON_DRAWER=folded` starts with the drawer folded to its rail and
        // `CAULDRON_PANES=n` with the card split into `Shape::ALL[n]`, for screenshots of the live window.
        #[cfg(debug_assertions)]
        {
            match std::env::var("CAULDRON_PAGE").as_deref() {
                Ok("agents") => c.open_page(pages::AppPage::Agents),
                Ok("library") => c.open_page(pages::AppPage::Library),
                Ok("usage") => c.open_page(pages::AppPage::Usage),
                _ => {}
            }
            if std::env::var("CAULDRON_PLATFORM").as_deref() == Ok("windows") {
                c.plat = Platform::Windows;
            }
            if std::env::var("CAULDRON_SIDEBAR").as_deref() == Ok("folded") {
                c.sidebar_open = false;
            }
            if std::env::var("CAULDRON_DRAWER").as_deref() == Ok("folded") {
                c.drawer_last = c.drawer.take();
            }
            if let Ok(v) = std::env::var("CAULDRON_SETTINGS") {
                c.settings = Some(v.parse::<usize>().unwrap_or(0).min(settings::PAGES.len() - 1));
            }
            if std::env::var_os("CAULDRON_TERMINAL").is_some() {
                c.terminal = true;
            }
            if let Some(i) = std::env::var("CAULDRON_PANES").ok().and_then(|v| v.parse::<usize>().ok()) {
                c.panes.shape = Shape::ALL[i % Shape::ALL.len()];
            }
        }
        #[cfg(debug_assertions)]
        if let Some(n) = std::env::var("CAULDRON_FIXTURE_AGENTS").ok().and_then(|v| v.parse::<usize>().ok()) {
            let names = ["Rampart", "Builder", "Reviewer", "Tester", "Docs", "Planner", "Linter", "Router", "Fixer", "Scout", "Indexer", "Mapper", "Pusher", "Watcher", "Herald", "Keeper"];
            c.coordinator = Some("Cauldron · coordinator".into());
            c.agents = (0..n).map(|i| AgentTab { name: names[i % names.len()].into(), dot: i }).collect();
            c.idle = vec![AgentTab { name: "Herald".into(), dot: 3 }, AgentTab { name: "Keeper".into(), dot: 2 }];
            c.active = n;
            c.reveal = true;
        }
        c
    }

    pub(crate) fn new() -> Self {
        Cauldron { motion: ShellMotion::default(), swap: None, action_since: None, busy_shown: false, open_since: None, bubble_since: None, mine_for: None, mine_n_shown: 0, drawer_last: None, sidebar_open: true, drawer: Some(Panel::Highlights), backdrop: Default::default(), glass_at: Default::default(), glass_card: std::cell::Cell::new(None), appearance_sub: None, prefs_file: None, plat: platform(), folder_open: Default::default(), page: Default::default(), page_agents: Vec::new(), agent_sel: None, settings: None, accounts_file: None, terminal: false, term_blocks: Vec::new(), term_input: None, term_note: None, term_scroll: ScrollHandle::new(), panes: Split::default(), layout_since: None, pane_drag: None, pane_content: [panes::Content::Thread, panes::Content::Empty, panes::Content::Empty], pane_focus: Vec::new(), thread_box: std::cell::Cell::new((0.0, 0.0)), transcript_scroll: ScrollHandle::new(), transcript_seen: Default::default(), coordinator: None, agents: Vec::new(), idle: Vec::new(), active: 0, reveal: false, tab_menu: false, tab_slide: Default::default(), tip: None, tip_shown: false, tip_warm_until: None, tip_wait: None, threads: Threads::Ready(Vec::new()), selected: None, query: String::new(), search: None, list_focus: None, core: None, watch: None, open: None, thread: None, steps: Default::default(), composer: None, send_error: None, start: Default::default() }
    }

    /// Opens core off the main thread (the sidebar shows Loading meanwhile), then keeps the thread
    /// list current: core pokes a channel whenever a thread changes.
    fn connect(&mut self, cx: &mut Context<Self>) {
        self.threads = Threads::Loading;
        self.watch = Some(cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let opened = cx
                .background_executor()
                .spawn(async {
                    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).thread_name("chats").enable_all().build()?;
                    let (tx, rx) = async_channel::bounded(1);
                    let chats = Chats::open(Prefs::data_dir(), rt.handle().clone(), tx);
                    let p = Prefs::load();
                    anyhow::Ok((rt, chats, rx, newthread::default_route(&p), p.machines))
                })
                .await;
            let (rt, chats, changes, route, machines) = match opened {
                Ok(o) => o,
                Err(e) => {
                    let _ = this.update(cx, |c, cx| {
                        c.threads = Threads::Error(e.to_string().into());
                        cx.notify();
                    });
                    return;
                }
            };
            let _ = this.update(cx, |c, cx| {
                c.core = Some((rt, chats));
                c.start.route = route;
                c.start.machines = machines;
                c.refresh();
                // Debug builds only: `CAULDRON_OPEN=<thread id>` opens that thread once core is up, for
                // screenshots of the live window.
                #[cfg(debug_assertions)]
                if let Ok(id) = std::env::var("CAULDRON_OPEN") {
                    c.open_thread(id);
                }
                c.watch(changes, cx);
                cx.notify();
            });
        }));
    }

    /// Thread ids in the order the sidebar draws them.
    fn visible_ids(&self) -> Vec<String> {
        match &self.threads {
            Threads::Ready(v) => threads::sidebar(v, &self.query, &self.folder_open, self.selected.as_deref())
                .into_iter()
                .filter_map(|i| match i {
                    threads::Item::Row { t, .. } => Some(t.id.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Moves the selection `d` rows, stopping at the ends; with none selected, starts at an end.
    fn step_selection(&mut self, d: isize) {
        let ids = self.visible_ids();
        let Some(last) = ids.len().checked_sub(1) else { return };
        let at = self.selected.as_ref().and_then(|s| ids.iter().position(|x| x == s));
        let j = match at {
            Some(i) => (i as isize + d).clamp(0, last as isize) as usize,
            None if d > 0 => 0,
            None => last,
        };
        self.selected = Some(ids[j].clone());
    }

    /// The sidebar's list: rows by group, or the loading, empty, no-match or error state.
    fn thread_list(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let focus = self.list_focus.clone().expect("created in render");
        let mut list = div()
            .id("thread-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .pb(px(8.))
            .track_focus(&focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                let d = match ev.keystroke.key.as_str() {
                    "up" => -1,
                    "down" => 1,
                    "enter" => {
                        if let Some(id) = this.selected.clone() {
                            this.open_thread(id);
                        }
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    _ => return,
                };
                this.step_selection(d);
                cx.stop_propagation();
                cx.notify();
            }));
        // A state in place of rows: a title, a line under it, and the whole as one accessible name.
        let note = |id: &'static str, label: String, icon: Option<(&'static str, Hsla)>, title: String, body: Option<String>| {
            let mut head = div().flex().items_center().gap(px(6.)).text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg2);
            if let Some((name, col)) = icon {
                head = head.child(glyph(name, 14., col));
            }
            let mut d = div().id(id).flex().flex_col().gap(px(4.)).px(px(8.)).pt(px(8.)).aria_label(label).child(head.child(title));
            if let Some(b) = body {
                d = d.child(div().text_size(px(11.5)).line_height(px(16.)).text_color(fg3).child(b));
            }
            d.test_support()
        };
        match &self.threads {
            Threads::Loading => {
                // Skeleton rows shaped like the real ones, static (no shimmer to turn off for reduced motion).
                let bar = |w: f32| div().h(px(8.)).w(relative(w)).rounded(px(radius::INNER)).bg(c(pal.inset));
                let mut sk = div().id("threads-loading").flex().flex_col().pt(px(14.)).aria_label("Loading streams");
                for w in [0.72, 0.56, 0.8, 0.48, 0.64] {
                    sk = sk.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .h(px(30.))
                            .pl(px(11.))
                            .pr(px(8.))
                            .child(div().w(px(18.)).h(px(22.)).flex_none().rounded(px(radius::TAG)).bg(c(pal.inset)))
                            .child(div().flex_1().child(bar(w))),
                    );
                }
                list.child(sk.test_support())
            }
            Threads::Error(e) => list.child(note(
                "threads-error",
                format!("Streams could not load: {e}"),
                Some(("circle-alert", c(pal.bad))),
                "Streams could not load".into(),
                Some(e.to_string()),
            )),
            Threads::Ready(v) if v.is_empty() => {
                list.child(note("threads-empty", "No streams yet".into(), None, "No streams yet".into(), Some("Start one with New stream.".into())))
            }
            Threads::Ready(v) => {
                let items = threads::sidebar(v, &self.query, &self.folder_open, self.selected.as_deref());
                if items.is_empty() {
                    let q = self.query.trim().to_string();
                    return list.child(note("threads-no-match", format!("No streams match {q}"), None, format!("No streams match \u{201c}{q}\u{201d}"), None));
                }
                // The pages board (5b7662c), folders direction B: Needs you, then Folders, each
                // folder folding, Unfiled last; a row is its state dot, its title and its age.
                let now = now_ms();
                let head = |id: ElementId| div().id(id).flex().items_center().gap(px(6.)).h(px(36.)).flex_none().px(px(8.)).pt(px(14.)).pb(px(4.)).text_size(px(12.)).line_height(px(18.)).text_color(fg3);
                let mut i = 0usize;
                for (gi, item) in items.into_iter().enumerate() {
                    match item {
                        threads::Item::Needs(n) => {
                            let label = if n == 0 { "Needs you".to_string() } else { format!("Needs you, {n}") };
                            list = list.child(head("group-needs".into()).aria_label(label).child("Needs you").when_live(n > 0, |d| d.child(div().ml_auto().font_features(tnum()).child(n.to_string()))).test_support());
                        }
                        threads::Item::Folders => {
                            list = list.child(
                                head("group-folders".into())
                                    .aria_label("Folders")
                                    .child("Folders")
                                    .child(
                                        div()
                                            .id("folder-add")
                                            .ml_auto()
                                            .size(px(20.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded(px(radius::TAG))
                                            .aria_label("New folder (arrives later)")
                                            .child(glyph("plus", 12., c(pal.fg4)))
                                            .test_support(),
                                    )
                                    .test_support(),
                            );
                        }
                        threads::Item::Folder { name, count, open, dot } => {
                            let key = name.clone();
                            list = list.child(
                                div()
                                    .id(("group", gi))
                                    .h(px(30.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .px(px(8.))
                                    .rounded(px(radius::TAG))
                                    .cursor_pointer()
                                    .hover(|d| d.bg(c(pal.hover)))
                                    .text_size(px(13.))
                                    .text_color(fg2)
                                    .aria_label(format!("{name}, {count}"))
                                    .aria_expanded(open)
                                    .child(glyph(if open { "chevron-down" } else { "chevron-right" }, 12., fg3))
                                    .child(glyph("folder", 14., fg3))
                                    .child(div().flex_1().min_w_0().truncate().child(name))
                                    .when_live(dot, |d| d.child(div().id(("folder-dot", gi)).size(px(6.)).flex_none().rounded_full().bg(c(pal.accent)).test_support()))
                                    .child(div().flex_none().text_size(px(11.)).text_color(fg3).font_features(tnum()).child(count.to_string()))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.folder_open.insert(key.clone(), !open);
                                        cx.notify();
                                    }))
                                    .test_support(),
                            );
                        }
                        threads::Item::Row { t, chip } => {
                            let on = self.selected.as_deref() == Some(t.id.as_str());
                            let id = t.id.clone();
                            // The state dot (the board): needs you filled accent, running a ring in
                            // accent, failed bad, done grey. No figures: they belong to Agents.
                            let (state, fill, ring) = match Group::of(t) {
                                Group::Running => ("Working", None, Some(c(pal.accent))),
                                Group::Failed => ("Failed", Some(c(pal.bad)), None),
                                Group::Unread => ("Needs you", Some(c(pal.accent)), None),
                                Group::Done => ("Done", Some(fg3), None),
                            };
                            let mut dotel = div().id(("thread-dot", i)).size(px(7.)).rounded_full();
                            if let Some(f) = fill {
                                dotel = dotel.bg(f);
                            }
                            if let Some(r) = ring {
                                dotel = dotel.border_1().border_color(r);
                            }
                            let slot = div().id(("thread-av", i)).w(px(18.)).h(px(22.)).flex_none().flex().items_center().justify_center().aria_label(state).child(dotel.test_support());
                            list = list.child(
                                div()
                                    .id(("thread", i))
                                    .h(px(30.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .pl(px(if chip.is_some() { 11. } else { 23. }))
                                    .pr(px(8.))
                                    .rounded(px(radius::TAG))
                                    .text_size(px(13.))
                                    .text_color(if on || t.unread { fg } else { fg2 })
                                    .when_live(on, |d| d.bg(c(pal.hover)))
                                    .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                                    .cursor_pointer()
                                    .aria_label(t.title.clone())
                                    .child(slot.test_support())
                                    .child(div().flex_1().min_w_0().truncate().font_weight(if t.unread { FontWeight::SEMIBOLD } else { FontWeight::NORMAL }).child(t.title.clone()))
                                    .when_live(t.pinned, |d| d.child(glyph("pin", 11., fg3)))
                                    .children(chip.map(|f| div().id(("thread-chip", i)).flex_none().max_w(px(72.)).truncate().px(px(5.)).rounded(px(radius::TAG)).bg(c(pal.inset)).text_size(px(11.)).text_color(fg3).aria_label(f.clone()).child(f).test_support()))
                                    .child(div().id(("thread-age", i)).flex_none().text_size(px(11.)).text_color(fg3).font_features(tnum()).child(threads::age(t.updated, now)).test_support())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.selected = Some(id.clone());
                                        this.open_thread(id.clone());
                                        if let Some(f) = &this.list_focus {
                                            window.focus(f, cx);
                                        }
                                        cx.notify();
                                    }))
                                    .test_support(),
                            );
                            i += 1;
                        }
                    }
                }
                list
            }
        }
    }

    /// Closes agent `j`'s 1:1. The agent stays in the thread, back in the + menu.
    fn close_agent(&mut self, j: usize) {
        let t = self.agents.remove(j);
        self.idle.push(t);
        let i = j + 1;
        if self.active > i {
            self.active -= 1;
        } else if self.active == i {
            self.active = i.min(self.agents.len());
        }
    }

    /// Opens a 1:1 with idle agent `j` as the last tab.
    fn open_agent(&mut self, j: usize) {
        let t = self.idle.remove(j);
        self.agents.push(t);
        self.active = self.agents.len();
        self.reveal = true;
        self.tab_menu = false;
    }

    /// The + menu: spawn a subagent (wired to core in UI4), or open a 1:1 with an agent not open yet.
    fn tab_menu_el(&self, pal: &'static Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let item = |id: ElementId| {
            div().id(id).flex().items_center().gap(px(8.)).px(px(8.)).py(px(6.)).rounded(px(radius::CONTROL))
        };
        let mut menu = div()
            .id("tab-menu")
            .relative()
            .w(px(236.))
            .p(px(4.))
            .flex()
            .flex_col()
            .gap(px(1.))
            .rounded(px(radius::PANEL))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .shadow(vec![drop(tokens::shadow::MENU)])
            .font_family(SANS).font_features(sans_features())
            .text_size(px(12.5))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(pal.fg2))
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.tab_menu = false;
                cx.notify();
            }))
            .child(self.tab_slide.measure(None))
            .children(self.tab_slide.highlight(ca(pal.fg, 0.06), radius::CONTROL))
            // ponytail: spawning needs the thread view's core wiring (UI4); shown, not live.
            .child(item("menu-spawn".into()).text_color(c(pal.fg3)).aria_label("Spawn a subagent").child(glyph("plus", 14., c(pal.fg3))).child("Spawn a subagent…").test_support().with_animation("in", motion::staggered(0), |d, t| d.opacity(t)));
        if !self.idle.is_empty() {
            menu = menu.child(div().px(px(8.)).pt(px(6.)).pb(px(2.)).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(c(pal.fg3)).child("Open 1:1 with").with_animation("in", motion::staggered(1), |d, t| d.opacity(t)));
        }
        for (j, a) in self.idle.iter().enumerate() {
            menu = menu.child(
                item(("menu-agent", j).into())
                    .relative()
                    .cursor_pointer()
                    .hover(|d| d.text_color(c(pal.fg)))
                    .child(self.tab_slide.measure(Some(j)))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if this.tab_slide.hover(j, *h) {
                            cx.notify();
                        }
                    }))
                    .aria_label(a.name.clone())
                    .child(div().flex_none().size(px(8.)).rounded_full().bg(c(tokens::AGENT_DOTS[a.dot % tokens::AGENT_DOTS.len()])))
                    .child(a.name.clone())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_agent(j);
                        cx.notify();
                    }))
                    .test_support()
                    .with_animation(("in", j + 2), motion::staggered(j + 2), |d, t| d.opacity(t)),
            );
        }
        deferred(motion::pop_in(div().absolute().top_full().right_0(), menu.test_support(), "menu-in", 6.)).with_priority(2)
    }

    fn hover_tip(&mut self, key: ElementId, hovered: bool, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        if !hovered {
            if self.tip.as_ref() == Some(&key) {
                if self.tip_shown {
                    self.tip_warm_until = Some(now + TIP_WARM);
                }
                self.tip = None;
                self.tip_shown = false;
                self.tip_wait = None;
                cx.notify();
            }
            return;
        }
        self.tip = Some(key.clone());
        self.tip_shown = self.tip_warm_until.is_some_and(|t| now < t);
        self.tip_wait = (!self.tip_shown).then(|| {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(TIP_DELAY).await;
                this.update(cx, |this, cx| {
                    if this.tip.as_ref() == Some(&key) {
                        this.tip_shown = true;
                        cx.notify();
                    }
                })
                .ok();
            })
        });
        cx.notify();
    }

    /// Gives an icon button the wireframe's hover tip on `side`.
    fn tipped(&self, key: impl Into<ElementId>, el: Stateful<Div>, label: impl Into<SharedString>, side: Side, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let key = key.into();
        let label = label.into();
        let shown = self.tip_shown && self.tip.as_ref() == Some(&key);
        let el = el.relative().on_hover(cx.listener(move |this, hovered: &bool, _, cx| this.hover_tip(key.clone(), *hovered, cx)));
        if shown {
            el.child(tip(label, side, pal))
        } else {
            el
        }
    }

    /// Selects a function. The sidebar indicator slides to it from wherever it is now.

    /// The sidebar folded to its rail (the pages board 5b7662c): usage on top, the three apps,
    /// search and New stream, a square per folder (its letter, a dot when it holds a stream that
    /// needs you), then the version, the cog and the avatar stacked at the foot.
    fn folded_rail(&self, pal: &'static Palette, r: Rect, alpha: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg3 = c(pal.fg3);
        let sep = || div().w(px(20.)).h(px(1.)).my(px(4.)).bg(c(pal.line_strong));
        let mut col = pane("rail", r).opacity(alpha).flex().flex_col().items_center().gap(px(4.)).pt(px(4.)).pb(px(8.));
        for (i, p) in pages::AppPage::ALL.into_iter().enumerate() {
            let on = self.page == p && self.settings.is_none();
            col = col.child(
                self.tipped(("rail-page", i), icon_button(("rail-page", i), p.icon(), p.label(), pal, on), p.label(), Side::Right, pal, cx)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_page(p);
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        col = col
            .child(sep())
            .child(
                self.tipped("rail-search", icon_button("rail-search", "search", "Search streams", pal, false), "Search streams", Side::Right, pal, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar_open = true;
                        cx.notify();
                    }))
                    .test_support(),
            )
            .child(
                self.tipped("rail-new", icon_button("rail-new", "plus", "New stream", pal, false), "New stream", Side::Right, pal, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.new_thread(window, cx)))
                    .test_support(),
            );
        if let Threads::Ready(v) = &self.threads {
            let folders = threads::by_folder(v, "");
            if !folders.is_empty() {
                col = col.child(sep());
            }
            for (i, (name, ts)) in folders.into_iter().enumerate() {
                let waiting = ts.iter().any(|t| threads::needs_you(t));
                let letter = name.chars().next().map(|ch| ch.to_uppercase().to_string()).unwrap_or_default();
                let key = name.clone();
                let sq = div()
                    .id(("rail-folder", i))
                    .relative()
                    .size(px(28.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::INNER))
                    .bg(c(pal.inset))
                    .border_1()
                    .border_color(c(pal.line))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(pal.fg2))
                    .aria_label(format!("{name}, {}", ts.len()))
                    .child(letter)
                    .when_live(waiting, |d| d.child(div().absolute().top(px(-2.)).right(px(-2.)).size(px(7.)).rounded_full().bg(c(pal.accent))))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // The sidebar opens on that folder.
                        this.sidebar_open = true;
                        this.folder_open.insert(key.clone(), true);
                        cx.notify();
                    }));
                col = col.child(self.tipped(("rail-folder", i), sq, name, Side::Right, pal, cx).test_support());
            }
        }
        let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "You".into());
        let initial = user.chars().next().map(|ch| ch.to_uppercase().to_string()).unwrap_or_else(|| "Y".into());
        col.child(div().flex_1())
            // Usage at the foot (Charles, 13 Nov 15:54Z): the ring, its card on hover, the page on click.
            .child(self.usage_ring("rail-usage", pal, cx).test_support())
            .child(
                self.tipped("rail-version", icon_button("rail-version", "circle-arrow-up", "Version and updates", pal, false), format!("Version {}", env!("CARGO_PKG_VERSION")), Side::Right, pal, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings = Some(1);
                        cx.notify();
                    }))
                    .test_support(),
            )
            .child(
                self.tipped("rail-settings", icon_button("rail-settings", "settings", "Settings", pal, self.settings.is_some()), "Settings", Side::Right, pal, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings = if this.settings.is_some() { None } else { Some(0) };
                        cx.notify();
                    }))
                    .test_support(),
            )
            .child(div().id("rail-avatar").mt(px(4.)).size(px(24.)).flex().items_center().justify_center().rounded_full().bg(c(pal.accent)).text_size(px(11.)).text_color(c(pal.on_accent)).aria_label(user.clone()).child(initial).test_support())
            .text_color(fg3)
    }

    /// Usage: a ring and its number where core reports usage, "No data yet" until then; hovering
    /// shows the card, a click opens the Usage page. Core has no per-subscription usage yet.
    fn usage_ring(&self, id: &'static str, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let rail = id == "rail-usage";
        let el = div()
            .id(id)
            .flex_none()
            .flex()
            .items_center()
            .gap(px(5.))
            .px(px(if rail { 0. } else { 4. }))
            .rounded(px(radius::TAG))
            .cursor_pointer()
            .hover(|d| d.bg(c(pal.hover)))
            .aria_label("No data yet")
            .child(div().size(px(if rail { 20. } else { 12. })).my(px(if rail { 4. } else { 0. })).rounded_full().border_2().border_color(c(pal.line_strong)))
            .when_live(!rail, |d| d.child(div().text_size(px(11.)).text_color(c(pal.fg3)).child("No data yet")))
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_page(pages::AppPage::Usage);
                cx.notify();
            }));
        self.tipped((id, 1usize), el, "Usage: no data yet. Core does not report it per subscription.", if rail { Side::Right } else { Side::Right }, pal, cx)
    }

    /// The version, as a small button by the cog: it opens Settings > General, where the version
    /// and how updates come are said. Core has no update check wired to the window yet.
    fn version_button(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        // An icon, so the name keeps its room; the version is in its name and tip.
        let label = format!("Version {}", env!("CARGO_PKG_VERSION"));
        let el = icon_button("version", "circle-arrow-up", "Version and updates", pal, false).aria_label(label.clone()).on_click(cx.listener(|this, _, _, cx| {
            this.settings = Some(1);
            cx.notify();
        }));
        self.tipped("version-tip", el, label, Side::Right, pal, cx)
    }

    /// The drawer's own button (the right rail's top, and the drawer strip's first): folds the open
    /// drawer, or opens it on the last panel, closing the terminal that took its place.
    fn toggle_drawer(&mut self) {
        if self.drawer.is_some() && !self.terminal {
            self.drawer = None;
        } else {
            self.terminal = false;
            self.drawer = Some(self.drawer_last.unwrap_or(Panel::Highlights));
            self.drawer_last = self.drawer;
        }
    }

    fn toggle_panel(&mut self, p: Panel) {
        // With the terminal open the drawer is folded to its rail: a rail button brings it back.
        if self.terminal {
            self.terminal = false;
            self.drawer = Some(p);
            self.drawer_last = Some(p);
            return;
        }
        let was = self.drawer;
        self.drawer = if was == Some(p) { None } else { Some(p) };
        // Switching panels while one is open crossfades the body; closing keeps the last panel.
        if let Some(a) = was.filter(|a| *a != p && self.drawer.is_some()) {
            self.swap = Some((a, std::time::Instant::now()));
        }
        if self.drawer.is_some() {
            self.drawer_last = self.drawer;
        }
    }
}

/// A square icon button, 28px, the wireframe's `.fn`, `.ib` and `.rb`.
fn icon_button(id: impl Into<ElementId>, name: &str, label: &'static str, pal: &'static Palette, on: bool) -> Stateful<Div> {
    icon_button_in(id, name, label, pal, on, true)
}

/// `paint`: whether a lit button draws its own fill. The sidebar row draws one indicator that
/// slides between its buttons, so its buttons do not.
fn icon_button_in(id: impl Into<ElementId>, name: &str, label: &'static str, pal: &'static Palette, on: bool, paint: bool) -> Stateful<Div> {
    let hover = ca(pal.fg, 0.05);
    let sel = ca(pal.fg, 0.09);
    let fill = on && paint;
    div()
        .id(id)
        .w(px(metric::SBTN))
        .h(px(metric::SBTN))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .aria_selected(on)
        .bg(if fill { sel } else { gpui_kit::transparent_black() })
        .hover(move |d| d.bg(if fill { sel } else { hover }))
        .child(glyph(name, 16., if on { c(pal.fg) } else { c(pal.fg2) }))
        .aria_label(label)
}


/// The sidebar's right padding: its content ends at the board's x 240.
const SIDE_PR: f32 = 9.0;

/// The new thread's bar under the card: usage and the stages.
const STAGE_H: f32 = 32.0;
const ADD_LABEL: &str = "Spawn a subagent or open a 1:1";

/// An agent of the thread: its name and its identity dot (an index into `tokens::AGENT_DOTS`).
pub(crate) struct AgentTab {
    pub(crate) name: SharedString,
    pub(crate) dot: usize,
}

/// `.when` for builders: gpui-kit does not re-export GPUI's FluentBuilder.
trait WhenLive: Sized {
    fn when_live(self, cond: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if cond { f(self) } else { self }
    }
}
impl<T> WhenLive for T {}

const TIP_DELAY: std::time::Duration = std::time::Duration::from_millis(450);
const TIP_WARM: std::time::Duration = std::time::Duration::from_millis(300);
/// Tip height: 16px line, 3px padding and a 1px ring each side.
const TIP_H: f32 = 24.0;

#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
    Below,
}

/// An icon button's name on the raised surface, 8px beside it and centred, the wireframe's
/// `.fn[data-tip]::after`. Deferred so it paints over the neighbouring panes.
fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Tabular figures, so counts and ages line up.
fn tnum() -> FontFeatures {
    crate::ui::sans_features()
}

fn drop(s: tokens::shadow::Shadow) -> BoxShadow {
    BoxShadow { color: hsla(0., 0., 0., s.alpha), offset: point(px(0.), px(s.y)), blur_radius: px(s.blur), spread_radius: px(s.spread), inset: false }
}

fn tip(label: SharedString, side: Side, pal: &'static Palette) -> impl IntoElement {
    let mid = (metric::SBTN - TIP_H) / 2.0;
    let d = div()
        .id("tooltip")
        .absolute()
        .whitespace_nowrap()
        .h(px(TIP_H))
        .px(px(8.))
        .flex()
        .items_center()
        .rounded(px(radius::INNER))
        .bg(c(pal.raised))
        .border_1()
        .border_color(c(pal.line))
        .shadow(vec![drop(tokens::shadow::TIP)])
        .font_family(SANS).font_features(sans_features())
        .text_size(px(12.))
        .line_height(px(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(pal.fg))
        .aria_label(label.clone())
        .child(label);
    let d = match side {
        Side::Right => d.left_full().top(px(mid)),
        Side::Left => d.right_full().top(px(mid)),
        Side::Below => d.top_full().right_0(),
    };
    // Fades in and settles the last RISE px away from its button.
    let d = d.test_support().with_animation("tip-in", motion::anim(mt::TIP), move |d, t| {
        let gap = px(8. - tokens::motion::RISE * (1. - t));
        let d = d.opacity(t);
        match side {
            Side::Right => d.ml(gap),
            Side::Left => d.mr(gap),
            Side::Below => d.mt(gap),
        }
    });
    deferred(d).with_priority(1)
}

/// Debug builds only: `CAULDRON_THEME=light|dark` overrides the OS theme, so screenshots of both
/// come from one build.
fn forced_dark() -> Option<bool> {
    #[cfg(debug_assertions)]
    if let Ok(t) = std::env::var("CAULDRON_THEME") {
        return Some(t != "light");
    }
    None
}

impl Render for Cauldron {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The theme: Settings' choice, or the OS's while it is System (followed live: the window's
        // appearance changing re-renders, painting included).
        if self.appearance_sub.is_none() {
            self.appearance_sub = Some(cx.observe_window_appearance(window, |_, _, cx| cx.notify()));
        }
        let look = appearance::load(self.prefs_file.as_deref());
        let os_dark = !matches!(window.appearance(), WindowAppearance::Light | WindowAppearance::VibrantLight);
        let dark = forced_dark().unwrap_or(match look.theme {
            appearance::Theme::System => os_dark,
            appearance::Theme::Light => false,
            appearance::Theme::Dark => true,
        });
        let pal: &'static Palette = if dark { &tokens::DARK } else { &tokens::LIGHT };
        let vs = window.viewport_size();
        let (w, h) = (f32::from(vs.width), f32::from(vs.height));
        let plat = self.plat;
        let want = ShellState { sidebar_open: self.sidebar_open, drawer_open: self.drawer.is_some(), split_open: false, terminal_open: self.terminal, plugins: self.settings.is_none() && self.page.has_drawer() };
        let now = std::time::Instant::now();
        let reduce = cx.reduce_motion();
        let (s, mut moving) = self.motion.frame(want, Frame::at_rest(&layout::solve(w, h, want, plat)), now, reduce);
        // The drawer body's crossfade, the + menu's open and close, and the Send/Stop glyphs.
        let busy_now = self.busy();
        if busy_now != self.busy_shown {
            self.busy_shown = busy_now;
            self.action_since = Some(now);
        }
        // A new bubble of the user's rises in; a thread opened fades its transcript in. A thread
        // change is not a new bubble.
        let mine_n = self.thread.as_ref().map_or(0, |t| thread::lines(t).iter().filter(|l| l.mine).count());
        if self.open != self.mine_for {
            self.mine_for = self.open.clone();
            self.mine_n_shown = mine_n;
        } else if mine_n > self.mine_n_shown {
            self.bubble_since = Some(now);
            self.mine_n_shown = mine_n;
        }
        let running = |t: Option<std::time::Instant>| t.is_some_and(|t0| crate::motion::run(now, t0, cauldron_chrome::motion::time::CARRY) < 1.0);
        moving |= running(self.open_since) || running(self.bubble_since);
        if !running(self.open_since) {
            self.open_since = None;
        }
        if !running(self.bubble_since) {
            self.bubble_since = None;
        }
        if let Some(t0) = self.action_since {
            if crate::motion::run(now, t0, cauldron_chrome::motion::time::SWAP) >= 1.0 {
                self.action_since = None;
            } else {
                moving = true;
            }
        }
        let swap = self.swap.and_then(|(a, t0)| {
            let k = crate::motion::run(now, t0, cauldron_chrome::motion::time::PANEL);
            (k < 1.0).then_some((a, k))
        });
        if swap.is_none() {
            self.swap = None;
        } else {
            moving = true;
        }
        if let Some((_, t0)) = self.layout_since {
            if reduce || crate::motion::run(now, t0, cauldron_chrome::motion::time::LAYOUT) >= 1.0 {
                self.layout_since = None;
            } else {
                moving = true;
            }
        }
        if moving {
            window.request_animation_frame();
        }

        let frame = c(pal.frame);
        let card = c(pal.card);
        let ring = ca(pal.fg, if dark { 0.08 } else { 0.09 });
        let fg = c(pal.fg);
        let fg2 = c(pal.fg2);
        let fg3 = c(pal.fg3);
        let accent = c(pal.accent);

        self.ensure_composer(window, cx);
        self.ensure_term_input(window, cx);
        self.ensure_start(window, cx);
        let mut root = div()
            .id("cauldron")
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(frame)
            .font_family(SANS).font_features(sans_features())
            .text_size(px(13.))
            .text_color(fg)
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| this.drag_line(ev, cx)))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                if this.pane_drag.take().is_some() {
                    cx.notify();
                }
            }));

        // The painting behind the whole window: made once per window size, painting and theme, then
        // its blurred copy under the sidebar's tint is the window itself (None: the plain frame).
        let wall = backdrop::painting(look.backdrop, dark);
        if wall.is_none() {
            // None: the plain frame, and no old painting kept (or shown) from before.
            self.backdrop.borrow_mut().drop_all();
        }
        if let Some(name) = wall {
            let mut b = self.backdrop.borrow_mut();
            b.window(name, dark, w.max(1.0) as u32, h.max(1.0) as u32, [pal.panel.0, pal.panel.1, pal.panel.2]);
            if let Some(f) = b.glass(0, 0, w as u32, h as u32) {
                root = root
                    .child(img(f).id("window-art").absolute().top_0().left_0().size_full().test_support())
                    .child(div().absolute().top_0().left_0().size_full().bg({
                        // The sidebar's text reads over this: its tint holds 4.5:1 there.
                        let r = s.sidebar.or(s.rail).map_or(Rect::new(0.0, 0.0, w, h), |p| p.rect);
                        b.hsla_for(backdrop::Surface::Sidebar, dark, (r.x as i32, r.y as i32, r.w as u32, r.h as u32), pal)
                    }));
            }
        }

        // Window drag strip, under every control.
        root = root.child(
            abs(s.bar).window_control_area(WindowControlArea::Drag),
        );
        if let Some(cp) = s.caps {
            // Windows: the caption band left of the buttons drags the window too.
            let band = Rect::new(s.shell.x + 22.0, s.shell.y, (cp.rect.x - s.shell.x - 22.0).max(0.0), metric::CAPS_BAND);
            root = root.child(abs(band).window_control_area(WindowControlArea::Drag));
        }

        // ---- rail (only while the sidebar is hidden)
        if let Some(rp) = s.rail {
            root = root.child(self.folded_rail(pal, rp.rect, rp.alpha, cx).test_support());
        }

        // ---- sidebar with the function row, on the window
        if let Some(sp) = s.sidebar {
            if self.search.is_none() {
                // The input's caret, selection and text come from the tokens, not the component theme.
                let t = Theme::global_mut(cx);
                t.foreground = fg;
                t.muted_foreground = fg3;
                t.caret = accent;
                t.selection = ca(pal.accent, 0.25);
                t.font_family = SANS.into();
                let st = cx.new(|cx| InputState::new(window, cx).placeholder("Search streams"));
                let sub = cx.subscribe(&st, |this, st, ev: &InputEvent, cx| {
                    if let InputEvent::Change = ev {
                        this.query = st.read(cx).value().to_string();
                        cx.notify();
                    }
                });
                self.search = Some((st, sub));
            }
            let search = self.search.as_ref().expect("just created").0.clone();
            self.list_focus.get_or_insert_with(|| cx.focus_handle());
            let list = self.thread_list(pal, cx);
            let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "You".into());
            let initial = user.chars().next().map(|ch| ch.to_uppercase().to_string()).unwrap_or_else(|| "Y".into());
            let new_thread = div()
                .id("new-thread")
                .h(px(32.))
                .flex()
                .items_center()
                .gap(px(9.))
                .px(px(8.))
                .rounded(px(radius::TAG))
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(fg)
                .child(glyph("square-pen", 15., fg))
                .child("New stream")
                .on_click(cx.listener(|this, _, window, cx| this.new_thread(window, cx)))
                .test_support();
            // The board's `.side`: on the window, no card; the apps row, New stream, Search, the list.
            // ponytail: the layout keeps the card era's 12px margin, so the content starts at x 13,
            // the board's at 10; the right edge is the board's 240 (9px in from the pane's 249).
            let card_el = pane("sidebar", sp.rect)
                .opacity(sp.alpha)
                .flex()
                .flex_col()
                .pr(px(SIDE_PR))
                .overflow_hidden()
                .child(self.page_switch(pal, cx).test_support())
                .when_live(self.settings.is_some(), |d| d.child(self.settings_nav(self.settings.unwrap_or(0), pal, cx).test_support()))
                .when_live(self.settings.is_none(), |d| d.child(new_thread))
                .when_live(self.settings.is_none(), |d| d.child(
                    div().child(
                        div()
                            .id("search")
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .gap(px(9.))
                            .px(px(8.))
                            .rounded(px(radius::TAG))
                            .hover(|d| d.bg(c(pal.hover)))
                            .text_color(fg2)
                            .aria_label("Search streams")
                            .child(glyph("search", 15., fg2))
                            .child(div().flex_1().min_w_0().child(Input::new(&search).appearance(false).text_size(px(13.)).p_0()))
                            .test_support(),
                    ),
                ))
                .when_live(self.settings.is_none(), |d| d.child(list))
                .child(
                    div()
                        .h(px(37.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(9.))
                        .pl(px(3.))
                        .pt(px(8.))
                        .child(
                            div()
                                .size(px(24.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .bg(accent)
                                .text_size(px(11.))
                                .text_color(c(pal.on_accent))
                                .child(initial),
                        )
                        // The name with usage beside it (Charles, 13 Nov 07:34Z), then the version and
                        // the cog. Core has no usage per subscription yet: an empty state, no number.
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(div().min_w_0().truncate().text_size(px(12.5)).child(user))
                                .child(self.usage_ring("usage-left", pal, cx).test_support()),
                        )
                        .child(self.version_button(pal, cx).test_support())
                        .child(self.tipped("settings", icon_button("settings", "settings", "Settings", pal, self.settings.is_some()), "Settings", Side::Right, pal, cx).on_click(cx.listener(|this, _, _, cx| {
                            this.settings = if this.settings.is_some() { None } else { Some(0) };
                            cx.notify();
                        })).test_support()),
                );
            root = root.child(card_el.test_support());
        }


        // The card's head is its pane's header (Charles, 13 Nov 06:52Z: tabs in each pane, no
        // breadcrumb): the thread's tabs, or the screen's name, then split, the terminal and the
        // Code pill. Settings has its name only.
        let streams = self.settings.is_none() && self.page == pages::AppPage::Streams;
        let head_title: SharedString = if self.settings.is_some() {
            "Settings".into()
        } else if !streams {
            self.page.label().into()
        } else {
            NEW_TAB.into()
        };
        let mut head = div()
            .id("card-head")
            .h(px(HEAD_H))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .pl(px(8.))
            .pr(px(8.))
            .text_size(px(13.))
            .text_color(fg3);
        let tabbed = self.coordinator.is_some() && streams;
        if tabbed && plat == Platform::Windows {
            head = head.child(self.crumb(pal).test_support());
        } else if tabbed {
            head = head.child(self.tab_strip(pal, s.main.w, cx).test_support());
        } else {
            head = head.child(
                div()
                    .id("card-title")
                    .relative()
                    .h(px(metric::TAB_H))
                    .flex()
                    .items_center()
                    .px(px(8.))
                    .rounded(px(radius::INNER))
                    .children(self.glass("head-title", radius::INNER))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(fg)
                    .aria_label(head_title.clone())
                    .child(div().relative().child(head_title))
                    .test_support(),
            );
        }
        if streams {
            head = head
                .child(div().id("head-drag").flex_1().h_full().window_control_area(WindowControlArea::Drag))
                .child(self.head_controls(None, pal, now, reduce, false, cx).test_support());
        }
        // A new thread keeps a 32px bar under its card, in the Shell's grey (Charles 13 Nov).
        let new_screen = self.open.is_none() && self.active == 0 && streams;
        let card_h = s.main.h - if new_screen { STAGE_H } else { 0.0 };
        // Split (direction B): under the head the card holds up to four panes on the Shell's grey;
        // a shape that no longer fits steps back, as the shell closes Split when the window narrows.
        // Split with the tabs in the panes, the card head goes: each pane's header carries its tabs.
        let full = Rect::new(s.main.x, s.main.y, s.main.w, card_h);
        let headed = Rect::new(s.main.x, s.main.y + HEAD_H, s.main.w, card_h - HEAD_H);
        let shape = if !streams { Shape::One } else { cauldron_chrome::panes::effective(full, self.panes.shape) };
        // Split, the card head goes: each pane's header carries its own tabs and controls.
        let headless = shape != Shape::One;
        let area = if headless { full } else { headed };
        let split = shape != Shape::One;
        // The thread view drops what does not fit its pane, so it needs that pane's size first.
        let sp = Split { shape, ..self.panes };
        let at = self.pane_content.iter().position(|p| *p == panes::Content::Thread).unwrap_or(0).min(shape.count() - 1);
        let tb = cauldron_chrome::panes::rects(area, sp)[at];
        self.thread_box.set(if split { (tb.w, tb.h - HEAD_H) } else { (tb.w, tb.h) });
        // The painting behind the whole window (painting X, the board e0fd268): frosted under every
        // surface, sharp only through the new thread's clear card.
        let sh = s.shell;
        let (frost, art, card_frost, card_tint) = match wall {
            Some(_) => {
                let mut b = self.backdrop.borrow_mut();
                let frost = b.glass(sh.x as i32, sh.y as i32, sh.w as u32, sh.h as u32);
                let cut = (s.main.x as i32, s.main.y as i32, s.main.w as u32, card_h as u32);
                let art = (new_screen && !split).then(|| b.sharp(cut.0, cut.1, cut.2, cut.3)).flatten();
                let card_frost = (!(new_screen && !split) && !split).then(|| b.glass(cut.0, cut.1, cut.2, cut.3)).flatten();
                let tint = b.hsla_for(backdrop::Surface::Card, dark, cut, pal);
                (frost, art, card_frost, tint)
            }
            None => (None, None, None, c(pal.panel)),
        };
        self.glass_card.set(wall.map(|_| (0.0, 0.0, dark)));
        let body = match self.settings {
            Some(page) => self.settings_page(page, pal, cx),
            None if !streams => self.page_body(pal, cx),
            None => self.card_body(pal, window, cx),
        };
        let inside = if split { self.pane_stage(pal, area, shape, body, window, cx).into_any_element() } else { body };
        let card_el = div()
            .id("card")
            .absolute()
            .left_0()
            .top_0()
            .w_full()
            .h(px(card_h))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(radius::PANEL))
            // A Panel in its Shell: darker than the Shell in dark, white on it in light. Split, the
            // panes are the Panels and the head sits on the Shell.
            .when_live(!split && wall.is_none(), |d| d.bg(c(pal.panel)))
            .when_live(!split, |d| d.border_1().border_color(ring))
            .text_color(fg3)
            .children(art.map(|art| img(art).id("card-art").absolute().top_0().left_0().size_full().rounded(px(radius::PANEL)).test_support()))
            .children(card_frost.clone().map(|f| img(f).id("card-frost").absolute().top_0().left_0().size_full().rounded(px(radius::PANEL)).test_support()))
            .when_live(card_frost.is_some(), |d| d.child(div().absolute().top_0().left_0().size_full().rounded(px(radius::PANEL)).bg(card_tint)))
            .when_live(!headless, |d| d.child(head.test_support()))
            .child(div().flex_1().min_h_0().child(inside));
        // The grey Shell behind the card, Split and the drawer: the drawer stands in its right
        // gutter and the new thread's bar in its bottom one (the coordinator's brief, 13 Nov).
        // The Shell meets the sidebar flush, square on its left (Charles, 13 Nov 07:34Z: no gap at
        // the corners), rounded on its right.
        let mut shell_el = pane("shell", s.shell).rounded_tr(px(radius::SHELL)).rounded_br(px(radius::SHELL)).bg(card);
        if let Some(f) = frost {
            // The frame is frosted painting: the blurred copy under the Shell colour at 77% / 83%.
            let t = self.backdrop.borrow_mut().hsla_for(backdrop::Surface::Shell, dark, (sh.x as i32, sh.y as i32, sh.w as u32, sh.h as u32), pal);
            shell_el = shell_el
                .child(img(f).id("shell-art").absolute().top_0().left_0().size_full().rounded_tr(px(radius::SHELL)).rounded_br(px(radius::SHELL)).test_support())
                .child(div().absolute().top_0().left_0().size_full().rounded_tr(px(radius::SHELL)).rounded_br(px(radius::SHELL)).bg(t));
        }
        root = root.child(shell_el.test_support());
        let mut main_el = pane("main", s.main).child(card_el.test_support());
        if new_screen {
            // ponytail: core has no stages or per-subscription usage yet (UI-PLAN decision 5), so the
            // bar is static: usage as an empty state, the stages as plain labels with none current.
            let mut stages = div().flex().gap(px(12.));
            for (i, name) in ["Plan", "Execute", "Ship", "Commit"].into_iter().enumerate() {
                stages = stages.child(div().id(("stage", i)).aria_label(name).child(name).test_support());
            }
            main_el = main_el.child(
                div()
                    .id("stage-bar")
                    .absolute()
                    .left_0()
                    .top(px(card_h))
                    .w_full()
                    .h(px(STAGE_H))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .text_size(px(11.))
                    .line_height(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(fg3)
                    .child(div().id("stage-usage").aria_label("No usage data yet").child("No usage data yet").test_support())
                    .child(stages)
                    .test_support(),
            );
        }
        root = root.child(main_el.test_support());

        // ---- Windows: each pane's tabs float in a strip above it, in the Shell's top band (the
        // board e0fd268, B); the rightmost leaves the caption buttons their room.
        if plat == Platform::Windows && streams {
            let cards: Vec<Rect> = if split { cauldron_chrome::panes::rects(area, sp) } else { vec![Rect::new(s.main.x, s.main.y, s.main.w, card_h)] };
            let top = s.shell.y + (metric::CAPS_BAND - 26.0) / 2.0;
            for (slot, r) in cards.into_iter().enumerate() {
                let content = if split { self.pane_content[slot] } else { panes::Content::Thread };
                let right = s.caps.map_or(r.right(), |cp| if cp.rect.x < r.right() { cp.rect.x - 8.0 } else { r.right() });
                let strip_r = Rect::new(r.x, top, (right - r.x).max(0.0), 26.0);
                let inner = if content == panes::Content::Thread {
                    if self.coordinator.is_none() { continue; }
                    self.tab_strip(pal, strip_r.w, cx)
                } else {
                    div().id(("strip-name", slot)).flex().items_center().h(px(metric::TAB_H)).px(px(8.)).rounded(px(radius::INNER)).bg(c(pal.selected)).text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(fg).child(content.title())
                };
                root = root.child(pane_strip(slot, strip_r).child(inner).test_support());
            }
        }

        // ---- drawer (on the Shell's grey, no border of its own)
        if let (Some(dp), Some(p)) = (s.drawer, self.drawer.or(self.drawer_last)) {
            // A panel's body: absolute so two bodies can cross-fade in the same pane.
            let body = |p: Panel, a: f32| {
                div()
                    .absolute()
                    .left(px(10.))
                    .right(px(10.))
                    .top(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .opacity(a)
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(p.title()))
                    .child(div().text_size(px(12.)).text_color(fg3).child(format!("This panel arrives in {}.", p.milestone())))
            };
            // The panels are switched from the rail beside the drawer (Charles, 13 Nov 07:34Z: icons
            // stacked, never in a row). Both bodies share one box, so the outgoing one cross-fades.
            // On Agents the drawer is Agents' own (a mockup until the page is designed).
            let mut bodies = div().relative().flex_1().min_h_0();
            if self.page == pages::AppPage::Agents {
                bodies = bodies.child(self.agents_drawer(pal).test_support());
            } else {
                bodies = bodies.child(body(p, swap.map_or(1.0, |(_, k)| k)));
                if let Some((a, k)) = swap {
                    bodies = bodies.child(body(a, 1.0 - k));
                }
            }
            let d = pane("drawer", dp.rect).opacity(dp.alpha).flex().flex_col().child(bodies);
            root = root.child(d.test_support());
        }

        // ---- terminal card, in the drawer's place
        if let Some(tp) = s.terminal {
            root = root.child(self.terminal_card(pal, tp.rect, tp.alpha, cx).test_support());
        }

        // ---- right rail: always standing, the drawer (when open) beside it
        if let Some(rp) = s.rrail {
        let mut rr = pane("rrail", rp.rect).opacity(rp.alpha).flex().flex_col().items_center().gap(px(4.)).pt(px(4.));
        // The drawer's button on top (Charles, 13 Nov 06:52Z), a hairline, then the panels.
        rr = rr
            .child(
                self.tipped("rail-drawer", icon_button("rail-drawer", "panel-right", if self.drawer.is_some() && !self.terminal { "Hide the drawer" } else { "Show the drawer" }, pal, self.drawer.is_some() && !self.terminal), if self.drawer.is_some() && !self.terminal { "Hide the drawer" } else { "Show the drawer" }, Side::Left, pal, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_drawer();
                        cx.notify();
                    }))
                    .test_support(),
            )
            .child(div().w(px(20.)).h(px(1.)).my(px(2.)).bg(c(pal.line_strong)));
        for (i, p) in TOP_PANELS.iter().enumerate() {
            let p = *p;
            rr = rr.child(
                self.tipped(("rb", i), icon_button(("rb", i), p.icon(), p.title(), pal, self.drawer == Some(p) && !self.terminal), p.title(), Side::Left, pal, cx)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_panel(p);
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        rr = rr.child(div().flex_1()).child(
            self.tipped("rb-patches", icon_button("rb-patches", Panel::Patches.icon(), Panel::Patches.title(), pal, self.drawer == Some(Panel::Patches) && !self.terminal), Panel::Patches.title(), Side::Left, pal, cx)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_panel(Panel::Patches);
                    cx.notify();
                }))
                .test_support(),
        );
        root = root.child(rr.pb(px(8.)).test_support());
        }

        // ---- top bar: the logo and name after the sidebar button
        let logo_x = s.sidebar_button.right() + metric::GAP;
        root = root.child(abs(Rect::new(logo_x, s.bar.y + (metric::BAR_H - 18.0) / 2.0, 18.0, 18.0)).rounded(px(9.)).bg(accent));
        root = root.child(
            abs(Rect::new(logo_x + 26.0, s.bar.y, metric::TITLE_W - 26.0, metric::BAR_H))
                .flex()
                .items_center()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(fg)
                .child("Cauldron"),
        );

        // ---- sidebar button (top bar, left)
        root = root.child(
            pane("sbtn-slot", s.sidebar_button).child(
                // occlude: buttons over the drag strip stop the hit test, or Windows drags instead of clicking.
                {
                    let label = if self.sidebar_open { "Hide sidebar" } else { "Show sidebar" };
                    self.tipped("sbtn", icon_button("sbtn", "panel-left", label, pal, false), label, Side::Right, pal, cx)
                }
                    .occlude()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar_open = !this.sidebar_open;
                        cx.notify();
                    }))
                    .test_support(),
            )
            .test_support(),
        );

        // ---- Windows caption buttons (macOS uses the native lights)
        if let Some(cp) = s.caps {
            let r = cp.rect;
            let bad = c(pal.bad);
            let cell = |id: &'static str, name: &'static str, area: WindowControlArea, danger: bool| {
                div()
                    .id(id)
                    .w(px(r.w / 3.0))
                    .h(px(r.h))
                    .flex()
                    .items_center()
                    .justify_center()
                    .window_control_area(area)
                    .occlude()
                    .hover(move |d| d.bg(if danger { bad } else { ca(pal.fg, 0.08) }))
                    .child(glyph(name, 14., fg2))
            };
            root = root.child(
                pane("caps", r)
                    .opacity(cp.alpha)
                    .flex()
                    .child(cell("cap-min", "minus", WindowControlArea::Min, false))
                    .child(cell("cap-max", "square", WindowControlArea::Max, false))
                    .child(cell("cap-close", "x", WindowControlArea::Close, true))
                    .test_support(),
            );
        }

        // ---- window ring, drawn last so it sits on top of every pane edge
        root.child(
            div()
                .absolute()
                .size_full()
                .border_1()
                .border_color(ring)
                .rounded(px(if cfg!(target_os = "macos") { 0.0 } else { 0.0 })),
        )
    }
}

#[cfg(test)]
mod tests {
    // Explicit imports: a glob of gpui_kit brings GPUI's `test` macro, which shadows Rust's.
    use super::{platform, AgentTab, Cauldron, Panel, Threads};
    use crate::threads::sample;

    use cauldron_chrome::layout::{self, Rect, ShellState};
    use cauldron_chrome::motion::Frame;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{point, px, size, AppContext, ElementId, TestAppContext, Window};

    pub(super) const W: f32 = 1240.0;
    pub(super) const H: f32 = 702.0;

    fn assert_rect(window: &Window, id: &'static str, want: Rect, state: ShellState) {
        let b = window.find(id).bounds();
        let got = Rect::new(f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height));
        let close = |a: f32, b: f32| (a - b).abs() < 0.5;
        assert!(
            close(got.x, want.x) && close(got.y, want.y) && close(got.w, want.w) && close(got.h, want.h),
            "{id} in {state:?}: drawn {got:?}, solve() says {want:?}"
        );
    }

    /// Every pane the shell draws sits exactly where `cauldron_chrome::layout::solve` puts it,
    /// in all four combinations of sidebar and drawer. Split lives inside the card now (direction B),
    /// so the solver's half pane is never drawn.
    #[gpui_kit::test]
    fn panes_match_the_layout_solver_in_every_state(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        for bits in 0..4u8 {
            let state = ShellState { sidebar_open: bits & 1 != 0, drawer_open: bits & 2 != 0, split_open: false, terminal_open: false, plugins: true };
            cx.update(|cx| cx.set_reduce_motion(true));
            let handle = cx.add_window(move |_, _| Cauldron {
                sidebar_open: state.sidebar_open,
                drawer: state.drawer_open.then_some(Panel::Highlights),
                ..Cauldron::new()
            });
            cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                let s = Frame::at_rest(&layout::solve(W, H, state, platform()));
                assert_rect(window, "main", s.main, state);
                assert_rect(window, "shell", s.shell, state);
                assert_rect(window, "sbtn-slot", s.sidebar_button, state);
                for (id, r) in [("sidebar", s.sidebar), ("rail", s.rail), ("rrail", s.rrail), ("drawer", s.drawer), ("half", s.half), ("caps", s.caps)] {
                    let r = r.map(|p| p.rect);
                    match r {
                        Some(r) => assert_rect(window, id, r, state),
                        None => assert!(window.try_find(id).is_none(), "{id} drawn in {state:?} but solve() has none"),
                    }
                }
            })
            .unwrap();
        }
    }

    /// The rail stands beside the open drawer (Charles, 13 Nov 07:34Z: its icons stacked, never in
    /// a row): its top button folds or opens the drawer, a panel's button switches to that panel,
    /// the open one lit, and the open one's button folds it.
    #[gpui_kit::test]
    fn the_rail_stands_beside_the_drawer_and_switches_it(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| cx.set_reduce_motion(true));
        let handle = cx.add_window(|_, _| Cauldron::new());
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        let rest = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| c.motion = Default::default()).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let (d, r) = (rect(window, "drawer"), rect(window, "rrail"));
            assert_eq!(d.right() + 6.0, r.x, "the drawer left of the rail");
            assert!(window.try_find("drawer-strip").is_none(), "no strip in a row");
            let ys: Vec<f32> = (0..4usize).map(|i| rect(window, ("rb", i)).y).collect();
            assert!(ys.windows(2).all(|w| w[1] > w[0]), "the panels stacked: {ys:?}");
            assert!(rect(window, "rail-drawer").y < ys[0], "the drawer's button on top");
            assert_eq!(window.find("rail-drawer").label(), Some("Hide the drawer"));
            assert_eq!(window.find(("rb", 0usize)).selected(), Some(true), "Highlights is open");
            window.click(("rb", 1usize), cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("rb", 1usize)).selected(), Some(true), "a rail button switches the panel");
            window.click("rail-drawer", cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("drawer").is_none() && window.try_find("rrail").is_some(), "folded: the rail alone");
            window.click("rail-drawer", cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("drawer").is_some(), "and open again on the last panel");
            assert_eq!(window.find(("rb", 1usize)).selected(), Some(true));
        })
        .unwrap();
    }

    /// Icon-only buttons carry their name for screen readers. Hovering one shows the name beside
    /// it, as the wireframe's `.fn[data-tip]`: 8px away, centred, after 450ms, and at once while
    /// the pointer moves between neighbours.
    #[gpui_kit::test]
    fn icon_buttons_have_names_and_tooltips(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| cx.set_reduce_motion(true));
        let handle = cx.add_window(|_, _| Cauldron { drawer: None, ..Cauldron::new() });
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        let ms = std::time::Duration::from_millis;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let named: [(ElementId, &str); 6] = [
                (("rb", 0usize).into(), "Highlights"),
                ("rb-patches".into(), "Patches"),
                ("sbtn".into(), "Hide sidebar"),
                ("term-btn".into(), "Terminal"),
                ("rail-drawer".into(), "Show the drawer"),
                ("settings".into(), "Settings"),
            ];
            for (id, label) in named {
                assert_eq!(window.find(id.clone()).label(), Some(label), "{id:?}");
            }
            window.hover(("rb", 1usize), cx);
        })
        .unwrap();
        cx.executor().advance_clock(ms(300));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("tooltip").is_none(), "no tip before 450ms");
        })
        .unwrap();
        cx.executor().advance_clock(ms(300));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("tooltip").label(), Some("Usage"));
            let (tip, btn) = (rect(window, "tooltip"), rect(window, ("rb", 1usize)));
            assert!((tip.right() - (btn.x - 8.0)).abs() < 0.5, "right-rail tip sits 8px left of its button: {tip:?} {btn:?}");
            assert!((tip.y + tip.h / 2.0 - (btn.y + btn.h / 2.0)).abs() < 0.5, "and centred on it: {tip:?} {btn:?}");
            window.hover(("rb", 0usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find("tooltip").label(), Some("Highlights"), "a neighbour's tip shows at once");
        })
        .unwrap();
        cx.executor().advance_clock(ms(1000));
        cx.update_window(handle.into(), |_, window, cx| {
            window.hover("settings", cx);
            window.render_frame(cx);
            assert_eq!(window.find("tooltip").label(), Some("Settings"));
            let (tip, btn) = (rect(window, "tooltip"), rect(window, "settings"));
            assert!((tip.x - (btn.right() + 8.0)).abs() < 0.5, "left-side tip sits 8px right of its button: {tip:?} {btn:?}");
        })
        .unwrap();
    }

    pub(super) fn rect(window: &Window, id: impl Into<ElementId>) -> Rect {
        let b = window.find(id).bounds();
        Rect::new(f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height))
    }

    /// A thread with a pinned coordinator and `n` open 1:1 agent tabs; the coordinator is open.
    fn with_agents(n: usize, idle: &[&'static str]) -> Cauldron {
        let names = ["Rampart", "Builder", "Reviewer", "Tester", "Docs", "Planner", "Linter", "Router", "Fixer", "Scout", "Indexer", "Mapper", "Pusher", "Watcher", "Herald", "Keeper"];
        Cauldron {
            coordinator: Some("Cauldron · coordinator".into()),
            agents: (0..n).map(|i| AgentTab { name: names[i % names.len()].into(), dot: i }).collect(),
            idle: idle.iter().enumerate().map(|(i, n)| AgentTab { name: (*n).into(), dot: i }).collect(),
            ..Cauldron::new()
        }
    }

    fn open(cx: &mut TestAppContext, c: Cauldron) -> gpui_kit::WindowHandle<Cauldron> {
        cx.update(|cx| cx.set_reduce_motion(true));
        let handle = cx.add_window(move |_, _| c);
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        handle
    }

    fn on(window: &Window, i: usize) -> bool {
        window.find(("ptab", i)).selected() == Some(true)
    }

    /// The New stream screen has no tabs; the card starts 12px from the window's top and its 40px
    /// head says New stream, then split, the terminal and Code.
    #[gpui_kit::test]
    fn new_thread_screen_has_no_tabs_and_a_card_head(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, Cauldron::new());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let s = layout::solve(W, H, ShellState { sidebar_open: true, drawer_open: true, split_open: false, terminal_open: false, plugins: true }, platform());
            assert!(window.try_find(("ptab", 0usize)).is_none(), "no tab before the thread exists");
            assert!(window.try_find("tab-add").is_none(), "no +");
            assert!(window.try_find("bar-crumb").is_none(), "no breadcrumb");
            let card = rect(window, "card");
            assert_eq!((card.x, card.y, card.w, card.h), (s.main.x, s.main.y, s.main.w, s.main.h - 32.0), "the card, the stage bar under it");
            assert_eq!(card.y, 1.0 + 6.0 + 6.0, "12px under the window's top edge");
            // The v3 bottom bar (Charles, 13 Nov): in the Shell's grey under the card, card-wide, 32 high,
            // usage at the left and the four stages at the right. Core has neither yet, so usage is
            // an empty state and no stage is current: plain fg3 labels, nothing to click.
            let bar = rect(window, "stage-bar");
            assert_eq!((bar.x, bar.y, bar.w, bar.h), (card.x, card.bottom(), card.w, 32.0), "stage bar");
            let shell = rect(window, "shell");
            assert_eq!((shell.x, shell.y, shell.right(), shell.bottom()), (s.shell.x, s.shell.y, s.shell.right(), s.shell.bottom()), "the Shell");
            assert_eq!(bar.bottom() + 6.0, shell.bottom(), "the bar in the Shell's bottom gutter, not on the frame");
            assert!(rect(window, "rrail").right() + 6.0 == shell.right() && rect(window, "drawer").right() + 6.0 == rect(window, "rrail").x, "the drawer and the rail in the Shell's right gutter");
            assert_eq!(window.find("stage-usage").label(), Some("No usage data yet"));
            let mut last = rect(window, "stage-usage").right();
            for (i, name) in ["Plan", "Execute", "Ship", "Commit"].into_iter().enumerate() {
                let st = rect(window, ("stage", i));
                assert_eq!(window.find(("stage", i)).label(), Some(name));
                assert!(st.x > last && st.y >= bar.y && st.bottom() <= bar.bottom(), "stage {i} in order inside the bar");
                assert_ne!(window.find(("stage", i)).selected(), Some(true), "stage {i}: none current before a thread exists");
                last = st.right();
            }
            assert!(bar.right() - last <= 13.0, "the stages end at the bar's right padding");
            let head = rect(window, "card-head");
            assert_eq!((head.x, head.y, head.w, head.h), (card.x + 1.0, card.y + 1.0, card.w - 2.0, 40.0), "head");
            assert_eq!(window.find("card-title").label(), Some("New stream"));
            assert_eq!(rect(window, "card-title").x, card.x + 9.0, "title 8px in");
            assert!(rect(window, "code-pill").right() <= head.right() - 8.0, "the Code pill at the head's right");
        })
        .unwrap();
    }

    /// A thread's tabs sit in its card head (Charles, 13 Nov 06:52Z): control-sm 24, 4px apart, the
    /// open one selected, + the same height.
    #[gpui_kit::test]
    fn thread_tabs_sit_in_the_card_head(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_agents(3, &[]));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let head = rect(window, "card-head");
            for i in 0..=3usize {
                let t = rect(window, ("ptab", i));
                assert_eq!((t.y, t.h), (head.y + 8.0, 24.0), "tab {i} centred in the 40px head");
                assert_eq!(on(window, i), i == 0, "tab {i}: only the coordinator is open");
            }
            for i in 1..=3usize {
                assert_eq!(rect(window, ("ptab", i)).x - rect(window, ("ptab", i - 1)).right(), 4.0, "4px between tabs {} and {i}", i - 1);
            }
            let add = rect(window, "tab-add");
            assert_eq!((add.y, add.h), (head.y + 8.0, 24.0), "+ as tall as the tabs");
            // On its name, clear of its close.
            window.click_at(("ptab", 2usize), point(px(10.), px(12.)), cx);
            window.render_frame(cx);
            assert!(on(window, 2) && !on(window, 0), "a click opens the tab");
        })
        .unwrap();
    }

    /// No agent figures in threads (Charles, 13 Nov): they belong to the Agents page. A tab is its
    /// label after the 8px padding, with agents on the thread or not.
    #[gpui_kit::test]
    fn tabs_carry_no_agent_figures(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_agents(2, &[]));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            for i in 0..=2usize {
                assert!(window.try_find(("ptab", i)).is_some() && window.try_find(("tab-fig", i)).is_none(), "tab {i} has no figure");
            }
        })
        .unwrap();
        let handle = open(cx, Cauldron { coordinator: Some("Plain thread".into()), ..Cauldron::new() });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("ptab", 0usize)).label(), Some("Plain thread"));
            assert!(window.try_find(("tab-fig", 0usize)).is_none(), "a normal thread's tab has no figure");
        })
        .unwrap();
    }

    /// Core has no Needs you or usage yet, so both draw as empty states (Charles, 12 Nov): the
    /// group's head with no rows and no count, and "No data yet" under the name.
    #[gpui_kit::test]
    fn needs_you_and_usage_are_empty_states(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("group-needs").label(), Some("Needs you, 1"));
            assert!(rect(window, "group-needs").bottom() <= rect(window, ("thread", 0usize)).y, "first, its stream under it");
            assert_eq!(window.find("usage-left").label(), Some("No data yet"));
        })
        .unwrap();
    }

    /// The + menu opens a 1:1 with an agent of the thread that is not open yet.
    #[gpui_kit::test]
    fn plus_opens_a_one_to_one(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_agents(2, &["Tester"]));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("tab-add").label(), Some("Spawn a subagent or open a 1:1"));
            window.click("tab-add", cx);
            assert_eq!(window.find(("menu-agent", 0usize)).label(), Some("Tester"));
            window.click(("menu-agent", 0usize), cx);
            // The menu fades out over MENU ms; the check reads the settled frame.
            std::thread::sleep(std::time::Duration::from_millis(cauldron_chrome::motion::time::MENU + 50));
            window.render_frame(cx);
            assert!(window.try_find("tab-menu").is_none(), "the menu closes");
            assert_eq!(window.find(("ptab", 3usize)).label(), Some("Tester"));
            assert!(on(window, 3), "the new tab is open");
        })
        .unwrap();
    }

    /// Five threads, one per state plus a pinned done one: r is unread, q running, f failed.
    fn with_threads() -> Cauldron {
        let mut v: Vec<_> = [("p", "Pinned notes"), ("q", "Rampart mutation score"), ("r", "Koi palette"), ("f", "M2 eval task texts"), ("s", "Agent delegation")]
            .iter()
            .map(|(id, t)| sample(id, t))
            .collect();
        v[0].pinned = true;
        v[1].busy = true;
        v[1].agent = Some("Rampart".into());
        v[2].unread = true;
        v[3].failed = Some("usage limit".into());
        // Folders: Cauldron (p, q), Ocean (r, s), and f unfiled; Unfiled opened by hand.
        for (i, f) in [(0, "/code/Cauldron"), (1, "/code/Cauldron"), (2, "/code/Ocean"), (4, "/code/Ocean")] {
            v[i].project = Some(f.into());
        }
        let folder_open = [("Unfiled".to_string(), true)].into_iter().collect();
        Cauldron { threads: Threads::Ready(v), folder_open, ..Cauldron::new() }
    }

    /// The sidebar's headings in order, with their labels.
    fn heads(window: &Window) -> Vec<String> {
        let mut v = Vec::new();
        for id in ["group-needs", "group-folders"] {
            if let Some(e) = window.try_find(id) {
                v.push((e.bounds().origin.y, e.label().unwrap_or_default().to_string()));
            }
        }
        for g in 0..32usize {
            if let Some(e) = window.try_find(("group", g)) {
                v.push((e.bounds().origin.y, e.label().unwrap_or_default().to_string()));
            }
        }
        v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        v.into_iter().map(|(_, l)| l).collect()
    }

    fn row_labels(window: &Window) -> Vec<String> {
        (0usize..).map_while(|i| window.try_find(("thread", i)).map(|e| e.label().unwrap_or_default().to_string())).collect()
    }

    /// The sidebar (the pages board, folders direction B): Needs you with its streams flat, then
    /// Folders, each folder with its count, open when it holds running or waiting work, and
    /// Unfiled last; the rows inside the sidebar, under New stream.
    #[gpui_kit::test]
    fn sidebar_is_needs_you_then_folders(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(heads(window), ["Needs you, 1", "Folders", "Cauldron, 2", "Ocean, 2", "Unfiled, 1"]);
            assert_eq!(row_labels(window), ["Koi palette", "Pinned notes", "Rampart mutation score", "Agent delegation", "M2 eval task texts"]);
            assert_eq!(window.find(("thread-chip", 0usize)).label(), Some("Ocean"), "a needs-you row shows its folder");
            assert!(window.try_find(("thread-chip", 1usize)).is_none(), "under its folder, no chip");
            let side = rect(window, "sidebar");
            for i in 0..5usize {
                let r = rect(window, ("thread", i));
                assert!(r.x >= side.x && r.right() <= side.right(), "row {i} inside the sidebar");
            }
            assert!(rect(window, ("thread", 0usize)).y > rect(window, "new-thread").bottom(), "rows under New stream");
        })
        .unwrap();
    }

    /// A folder folds by hand; closed over a stream that needs you it shows the accent dot.
    #[gpui_kit::test]
    fn folders_fold_and_keep_a_dot_for_needs_you(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut c = with_threads();
        if let Threads::Ready(v) = &mut c.threads {
            v[4].unread = true;
            v[2].unread = false;
        }
        let handle = open(cx, c);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let ocean = (0..32usize).find(|&g| window.try_find(("group", g)).is_some_and(|e| e.label().is_some_and(|l| l.starts_with("Ocean")))).unwrap();
            assert_eq!(window.find(("group", ocean)).expanded(), Some(true), "open: it holds a stream that needs you");
            window.click(("group", ocean), cx);
            window.render_frame(cx);
            assert_eq!(window.find(("group", ocean)).expanded(), Some(false));
            assert!(window.try_find(("folder-dot", ocean)).is_some(), "closed over a waiting stream: the dot");
        })
        .unwrap();
    }

    /// Folded, the rail (the destinations board): the six apps, search and New stream, a
    /// square per folder (a dot on the one with a stream that needs you), then the version, the cog
    /// and the avatar stacked at the foot. Unfolded, usage sits beside the name, then the version
    /// and the cog.
    #[gpui_kit::test]
    fn the_folded_rail_stacks_usage_apps_folders_and_the_foot(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let (name_row, usage, version, cog) = (rect(window, "usage-left"), rect(window, "usage-left"), rect(window, "version"), rect(window, "settings"));
            assert!((usage.y + usage.h / 2.0 - (version.y + version.h / 2.0)).abs() <= 2.0 && version.x > name_row.x && cog.x > version.x, "usage, then version, then the cog, on one row");
        })
        .unwrap();
        handle.update(cx, |c, _, _| {
            c.sidebar_open = false;
            c.motion = Default::default();
        })
        .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let order = ["rail-search", "rail-new"].map(|id| rect(window, id).y);
            let pages: Vec<f32> = (0..6usize).map(|i| rect(window, ("rail-page", i)).y).collect();
            assert!(pages.windows(2).all(|w| w[1] > w[0]) && pages[5] < order[0] && order[0] < order[1], "the six apps stacked, search, New stream");
            let folders: Vec<String> = (0..3usize).filter_map(|i| window.try_find(("rail-folder", i)).and_then(|e| e.label().map(str::to_string))).collect();
            assert_eq!(folders, ["Cauldron, 2", "Ocean, 2", "Unfiled, 1"]);
            let (u, v, g, a) = (rect(window, "rail-usage"), rect(window, "rail-version"), rect(window, "rail-settings"), rect(window, "rail-avatar"));
            assert!(u.y < v.y && v.y < g.y && g.y < a.y, "usage, version, cog, avatar stacked at the foot (Charles, 13 Nov 15:54Z)");
            assert!(a.bottom() >= rect(window, "rail").bottom() - 12.0, "at the foot");
            window.click(("rail-folder", 1usize), cx);
        })
        .unwrap();
        assert!(handle.update(cx, |c, _, _| c.sidebar_open).unwrap(), "a folder square opens the sidebar on it");
    }

    /// Rows are 30 high: the state dot in its 18x22 slot (needs you, working, done; no figures),
    /// the title and the age; a 6px ping on an unread row only.
    #[gpui_kit::test]
    fn thread_rows_are_the_boards(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(rect(window, "new-thread").h, 32.0);
            for (i, state) in [(0usize, "Needs you"), (1, "Done"), (2, "Working"), (3, "Done"), (4, "Failed")] {
                let r = rect(window, ("thread", i));
                assert_eq!(r.h, 30.0, "row {i}");
                let a = rect(window, ("thread-av", i));
                assert_eq!((a.w, a.h), (18.0, 22.0), "row {i}'s slot");
                assert!((a.y + a.h / 2.0 - (r.y + r.h / 2.0)).abs() <= 1.0, "row {i}'s slot is centred");
                assert_eq!(window.find(("thread-av", i)).label(), Some(state), "row {i}");
                assert!(window.try_find(("thread-fig", i)).is_none(), "row {i}: no figure");
                assert!(window.try_find(("thread-dot", i)).is_some() && window.try_find(("thread-age", i)).is_some(), "row {i}: a dot and an age");
            }
        })
        .unwrap();
    }

    /// Loading, empty, no search match and error each draw their own state, never a blank list.
    #[gpui_kit::test]
    fn sidebar_draws_loading_empty_no_match_and_error(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let states = [
            (Cauldron { threads: Threads::Loading, ..Cauldron::new() }, "threads-loading", "Loading streams"),
            (Cauldron { threads: Threads::Ready(Vec::new()), ..Cauldron::new() }, "threads-empty", "No streams yet"),
            (Cauldron { query: "zzz".into(), ..with_threads() }, "threads-no-match", "No streams match zzz"),
            (Cauldron { threads: Threads::Error("disk full".into()), ..Cauldron::new() }, "threads-error", "Streams could not load: disk full"),
        ];
        for (c, id, label) in states {
            let handle = open(cx, c);
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert_eq!(window.find(id).label(), Some(label), "{id}");
                assert!(window.try_find(("thread", 0usize)).is_none(), "{id}: no rows");
            })
            .unwrap();
        }
    }

    /// Search narrows the list; Up/Down move the selection through the rows in view without
    /// opening (and so without marking read) anything. A click also opens the row (UI4,
    /// `a_row_opens_its_thread`); here there is no core, so it only selects.
    #[gpui_kit::test]
    fn search_filters_and_arrows_move_the_selection(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("search").label(), Some("Search streams"));
            window.click(("thread", 1usize), cx);
            window.press("down", cx);
            window.press("down", cx);
        })
        .unwrap();
        let sel = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| c.selected.clone()).unwrap();
        assert_eq!(sel(cx).as_deref(), Some("s"), "click Pinned notes, then two down: Rampart, then Agent delegation");
        cx.update_window(handle.into(), |_, window, cx| {
            for _ in 0..9 {
                window.press("up", cx);
            }
        })
        .unwrap();
        assert_eq!(sel(cx).as_deref(), Some("r"), "Up stops at the first row");
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("search", cx);
            window.input("  KOI", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(row_labels(window), ["Koi palette"]);
            assert_eq!(window.find(("thread-chip", 0usize)).label(), Some("Ocean"), "a hit shows its folder");
            assert!(heads(window).is_empty(), "a search is a flat list of hits");
        })
        .unwrap();
        handle
            .update(cx, |c, _, _| {
                let Threads::Ready(v) = &c.threads else { panic!() };
                assert!(v.iter().any(|t| t.unread), "selecting does not mark read");
            })
            .unwrap();
    }
}
