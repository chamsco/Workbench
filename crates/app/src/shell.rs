//! The Cauldron shell (UI1): the frame, top bar, sidebar, main card with tabs,
//! right rail, drawer and split. Every rectangle comes from `cauldron_chrome::layout::solve`,
//! which is tested against numbers measured from the wireframe; nothing here picks a position.
//!
//! Content inside the panes is a labelled placeholder until the milestone that owns it
//! (docs/cauldron/UI-PLAN.md): the thread list is UI3, the thread view UI4, the drawer UI6.

use cauldron_chrome::avatar::{self, Look, Mood};
use cauldron_chrome::layout::{self, metric, Platform, Rect, ShellState};
use cauldron_chrome::motion::Frame;
use cauldron_chrome::panes::{Shape, Split};
use cauldron_chrome::tokens::{self, motion as mt, radius, Palette, Rgb, Rgba};
use gpui_kit::*;

use crate::motion::ShellMotion;
use crate::threads::{self, Group};
use crate::ui::{sans_features, MONO, SANS};
use backspace_core::chat::{Chats, Thread, ThreadInfo};
use backspace_core::prefs::Prefs;
use gpui_kit::component::input::{Input, InputEvent, InputState, TextareaState};
use gpui_kit::component::Theme;
use std::sync::Arc;

mod apps;
mod backdrop;
mod decide;
mod familiar;
mod motion;
mod newthread;
mod panes;
mod logos;
mod picker;
mod side;
mod settings;
mod subs;
mod support;
mod terminal;
mod tabshape;
mod thread;
mod update;
#[cfg(all(feature = "shots", target_os = "macos"))]
pub(crate) mod shots;

/// Where the shell draws window controls: lights on macOS, caption buttons elsewhere.
/// Appearance > Window on Windows: the old notch in the card instead of the top strip.
static NOTCH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn set_notch(on: bool) {
    NOTCH.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn platform() -> Platform {
    // Debug builds only: `CAULDRON_PLATFORM=windows` draws the Windows chrome on a Mac, for screenshots.
    if cfg!(debug_assertions) && std::env::var("CAULDRON_PLATFORM").is_ok_and(|v| v == "windows") {
        return if NOTCH.load(std::sync::atomic::Ordering::Relaxed) { Platform::WindowsNotch } else { Platform::Windows };
    }
    if cfg!(target_os = "macos") {
        Platform::Mac
    } else if NOTCH.load(std::sync::atomic::Ordering::Relaxed) {
        Platform::WindowsNotch
    } else {
        Platform::Windows
    }
}

pub(crate) use motion::os_reduces_motion;

/// Top-left of the traffic lights at launch (sidebar open, in its top row). The shell moves them
/// to the stream header when the sidebar folds (`Window::set_traffic_light_position`).
pub(crate) fn lights_origin() -> (f32, f32) {
    let s = layout::layout(1440.0, 900.0, ShellState { sidebar_open: true, support_open: false, dock_open: false }, Platform::Mac);
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

/// A pane the layout tests find by `id` and compare with `layout::solve`.
fn pane(id: &'static str, r: Rect) -> Stateful<Div> {
    abs(r).id(id)
}

fn glyph(name: &str, size: f32, color: Hsla) -> Svg {
    svg().path(format!("icons/{name}.svg")).size(px(size)).flex_none().text_color(color)
}

/// What the support card shows for the current stream (Tight tiles rule 1): never a second stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Panel {
    Highlights,
    Changes,
    Git,
    Trace,
    Browser,
    Files,
    Simulator,
    History,
}

impl Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Highlights => "Highlights",
            Panel::Changes => "Changes",
            Panel::Git => "Git",
            Panel::Trace => "Trace",
            Panel::Browser => "Browser",
            Panel::Files => "Files",
            Panel::Simulator => "Simulator",
            Panel::History => "History",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Panel::Highlights => "pin",
            Panel::Changes => "diff",
            Panel::Git => "git-branch",
            Panel::Trace => "activity",
            Panel::Browser => "globe",
            Panel::Files => "files",
            Panel::Simulator => "smartphone",
            Panel::History => "clock-fading",
        }
    }
    /// Panels with no source yet: drawn as clearly marked placeholders.
    fn placeholder(self) -> Option<&'static str> {
        match self {
            Panel::Trace => Some("Tool calls and sub-agents, nested, arrive in a later build."),
            Panel::Simulator => Some("The app on a device arrives in a later build."),
            Panel::History => Some("Earlier runs of this stream arrive in a later build."),
            _ => None,
        }
    }
}

/// The apps row (Tight tiles): what the sidebar and the hover indent switch between.
const FUNCTIONS: [(&str, &str); 6] = [
    ("message-square", "Streams"),
    ("folder", "Folders"),
    ("calendar", "Automations"),
    ("bot", "Agents"),
    ("gauge", "Usage"),
    ("database", "Library"),
];

gpui_kit::actions!(cauldron, [NewStream]);

/// The window's shortcuts: New stream on ⌘N (macOS) or Ctrl+N.
pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new(if cfg!(target_os = "macos") { "cmd-n" } else { "ctrl-n" }, NewStream, None)]);
}

/// New stream's shortcut, as the platform writes it.
const NEW_KEY: &str = if cfg!(target_os = "macos") { "\u{2318}N" } else { "Ctrl+N" };

const NEW_TAB: &str = "New stream";

/// The rail's quick buttons (the board's `.irail`): four, a rule, three, then the terminal, which
/// docks under the stream rather than opening in the support card.
const RAIL_TOP: [Panel; 4] = [Panel::Highlights, Panel::Changes, Panel::Git, Panel::Trace];
const RAIL_MID: [Panel; 3] = [Panel::Browser, Panel::Files, Panel::Simulator];
/// The support card's + menu; `None` is the terminal (Ctrl+J).
const SUPPORT_MENU: [Option<Panel>; 6] = [Some(Panel::Browser), None, Some(Panel::Files), Some(Panel::Changes), Some(Panel::Simulator), Some(Panel::History)];
/// Support tabs in view; the rest collapse into "+N".
const SUPPORT_TABS_SHOWN: usize = 3;

/// The sidebar's threads from core: still opening, opened, or core failed to open.
pub(crate) enum Threads {
    Loading,
    Ready(Vec<ThreadInfo>),
    Error(SharedString),
}

pub(crate) struct Cauldron {
    sidebar_open: bool,
    /// The support card's open tab; None while it is folded (its tabs stay).
    drawer: Option<Panel>,
    /// The support card's tabs, in order, and whether its + menu is open.
    support_tabs: Vec<Panel>,
    support_menu: bool,
    /// What the Changes, Git and Files panels read from the stream's folder, by panel and folder;
    /// and the folders open in the Files tree.
    support_data: std::collections::HashMap<(Panel, String), support::Data>,
    files_open: std::collections::BTreeSet<std::path::PathBuf>,
    /// The Harbour painting behind the window (Tight tiles), or the plain frame.
    painting: bool,
    /// The cards' crops of the light frost (two-depth glass), and the user's own picture or GIF
    /// behind the stream card instead (Settings > Appearance), if they chose one.
    glass: backdrop::GlassCache,
    /// The decision card: grown under the pointer, the option open in Highlights, the sets already
    /// answered here, and each live familiar's waiting set (refreshed with its view).
    ask_hover: bool,
    ask_open: Option<String>,
    ask_answered: std::collections::HashSet<String>,
    fam_questions: std::collections::HashMap<String, Option<backspace_core::options::OptionSet>>,
    frost: backdrop::FrostLevel,
    stream_bg: Option<std::path::PathBuf>,
    /// "system", "light" or "dark" (the Appearance page's Mode, saved in prefs).
    theme_mode: String,
    /// The shell's own focus: it takes the keys when nothing inside has them (⌘N, Ctrl+N).
    root_focus: Option<FocusHandle>,
    /// Plan mode for the next send, from the new-stream box or the composer (Tight tiles): the
    /// agent proposes first and edits nothing until the plan is approved. Core's `Ctx::plan`.
    plan: bool,
    /// The open-in split button's menu, and the last choice its left half repeats.
    open_in_menu: bool,
    /// Failed replies whose CLI output is unfolded (by message id).
    err_open: std::collections::HashSet<String>,
    open_in: thread::OpenIn,
    /// The search row is open (the top row's search button), and the folded sidebar's hover
    /// indent is showing.
    search_open: bool,
    indent: bool,
    /// The subscriptions' usage left, read from accounts.json at most every `USAGE_EVERY`.
    usage: std::cell::RefCell<Option<(std::time::Instant, Vec<(String, u64)>)>>,
    /// Settings is open on this page (v3 frame 7): its pages take the sidebar's list, the page the card.
    settings: Option<usize>,
    /// Where Settings reads the subscriptions; None is core's `accounts.json` (tests use their own).
    accounts_file: Option<std::path::PathBuf>,
    /// The terminal is docked under the stream (Tight tiles rule 5).
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
    /// The thread's coordinator, pinned as the first tab; None on the New thread screen.
    coordinator: Option<SharedString>,
    /// The open familiar (an index into `fam_views`): the card shows its view and the tabs are its crew.
    familiar: Option<usize>,
    /// Every familiar as the sidebar and card draw it; the live ones' harnesses, their change
    /// watches, the ones whose drive is running now, the last drive or send error, the
    /// familiar's composer, and whether the views are made-up fixtures.
    fam_views: Vec<backspace_core::familiar::View>,
    fam_live: Vec<familiar::FamLive>,
    fam_watch: Vec<Task<()>>,
    fam_driving: std::collections::HashSet<String>,
    fam_error: Option<String>,
    fam_composer: Option<(Entity<TextareaState>, Subscription)>,
    fam_fixture: bool,
    /// The platform's notification inbox (Inbox board, direction A): its notes newest first, the
    /// popover under the bell, and the note on show as a toast with when it came.
    inbox: Vec<backspace_core::platform::notify::Note>,
    inbox_open: bool,
    /// The version button's update check and whether its card is open.
    update: update::UpdateState,
    update_open: bool,
    inbox_toast: Option<(backspace_core::platform::notify::Note, std::time::Instant)>,
    /// A familiar's joined tab: where it slides from (x, width), where it was last headed, since
    /// when; and the open Highlights tab.
    fam_from: (f32, f32),
    fam_to: Option<(f32, f32)>,
    fam_since: Option<std::time::Instant>,
    fam_hl: usize,
    /// Agents with an open 1:1 tab, in tab order, and the thread's other agents (the + menu).
    agents: Vec<AgentTab>,
    /// Side streams (board "Side streams A"): the open stream's main, its side streams as
    /// (id, title), and the + window that starts one.
    side_root: Option<String>,
    sides: Vec<(String, String)>,
    side_win: bool,
    side_input: Option<(Entity<InputState>, Subscription)>,
    side_ctx: backspace_core::chat::SideCtx,
    idle: Vec<AgentTab>,
    /// The open tab: 0 is the pinned one, i is `agents[i - 1]`.
    active: usize,
    /// Scroll the open tab into view at the next render (set when it changes).
    reveal: bool,
    /// Agent tabs in view at the last render, and unspent wheel travel in pixels.
    tab_window: usize,
    wheel: f32,
    tab_menu: bool,
    tab_slide: motion::Slide,
    /// The shell's motion clock, the drawer's panel crossfade (from, when), the drawer's last
    /// panel (kept while it closes).
    motion: ShellMotion,
    swap: Option<(Panel, std::time::Instant)>,
    /// The selected function (an index into FUNCTIONS), and the indicator's slide: where it was
    /// from, in slots, and when the slide began.
    section: usize,
    section_from: f32,
    section_since: Option<std::time::Instant>,
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
    tab_focus: Option<FocusHandle>,
    /// The icon button under the pointer, whether its tip is showing yet, and until when a
    /// neighbour's tip may show at once (the wireframe's `.rail.warm`).
    tip: Option<ElementId>,
    tip_shown: bool,
    tip_warm_until: Option<std::time::Instant>,
    tip_wait: Option<Task<()>>,
    /// First agent tab in view once the row scrolls.
    tab_first: usize,
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
    /// The New thread screen (UI5).
    start: newthread::Start,
}

impl Cauldron {
    /// The quick-button rail belongs to a stream (Charles, 20 Nov): shown while a stream or a
    /// familiar is open, or while the support card is open (its rail is how you fold it), never on
    /// the New stream screen or in Settings.
    pub(crate) fn rail_on(&self, support_open: bool) -> bool {
        support_open || (self.settings.is_none() && (self.open.is_some() || self.familiar.is_some()))
    }

    /// The window's first state. Debug builds only: `CAULDRON_FIXTURE_AGENTS=n` opens a thread with
    /// n agent tabs, for screenshots until UI4 brings real agents from core.
    pub(crate) fn launch(cx: &mut Context<Self>) -> Self {
        let mut c = Self::new();
        let prefs = Prefs::load();
        c.painting = backdrop::wants_painting(&prefs);
        c.stream_bg = backdrop::stream_bg(&prefs);
        if let Some(path) = c.stream_bg.clone() {
            Self::warm_picture(path, cx);
        }
        c.theme_mode = prefs.theme.clone();
        c.frost = backdrop::FrostLevel::from_pref(&prefs);
        set_notch(prefs.window_style.as_deref() == Some("notch"));
        if c.painting {
            Self::warm_glass(cx);
        }
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
        // Debug builds only, for screenshots of the live window: `CAULDRON_BACKDROP=plain|painting`
        // overrides the pref, `CAULDRON_SIDEBAR=folded` starts with
        // the sidebar folded, `CAULDRON_SUPPORT=open` with the support card open on the board's three
        // tabs, and `CAULDRON_PANES=n` with the card split into `Shape::ALL[n]`.
        #[cfg(debug_assertions)]
        {
            if let Ok(b) = std::env::var("CAULDRON_BACKDROP") {
                c.painting = b != "plain";
            }
            // `CAULDRON_FROST=light|medium|heavy` overrides the frost pref.
            if let Ok(f) = std::env::var("CAULDRON_FROST") {
                c.frost = backdrop::FrostLevel::from_pref(&Prefs { frost: Some(f), ..Prefs::default() });
            }
            if std::env::var("CAULDRON_SIDEBAR").as_deref() == Ok("folded") {
                c.sidebar_open = false;
                // `CAULDRON_SIDEBAR=folded CAULDRON_INDENT=1`: the hover indent showing.
                c.indent = std::env::var_os("CAULDRON_INDENT").is_some();
            }
            if std::env::var("CAULDRON_SUPPORT").as_deref() == Ok("open") {
                c.support_tabs = vec![Panel::Changes, Panel::Browser, Panel::Highlights];
                c.drawer = Some(Panel::Changes);
            }
            if std::env::var_os("CAULDRON_SETTINGS").is_some() {
                c.settings = Some(0);
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
        // Debug builds only: `CAULDRON_FIXTURE_FILES=a;b` attaches these to the New stream box.
        #[cfg(debug_assertions)]
        if let Ok(v) = std::env::var("CAULDRON_FIXTURE_FILES") {
            c.start.files = v.split(';').filter(|p| !p.is_empty()).map(std::path::PathBuf::from).collect();
        }
        c
    }

    /// Bakes the frost layers off the UI thread, then draws again so the cards pick up their light
    /// layer (until then they use the single-depth glass).
    pub(crate) fn warm_glass(cx: &mut Context<Self>) {
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            cx.background_executor().spawn(async { backdrop::warm() }).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    /// Bakes the frame and card layers from the user's picture off the UI thread, then draws again
    /// (the frame keeps the painting's layers until then).
    pub(crate) fn warm_picture(path: std::path::PathBuf, cx: &mut Context<Self>) {
        if cfg!(test) {
            return;
        }
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let res = cx.background_executor().spawn(async move { backdrop::warm_user(&path) }).await;
            if let Err(e) = res {
                eprintln!("stream picture: {e}");
            }
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    /// What goes under a card over the painting: the user's picture under its scrim (stream card
    /// only), or this card's crop of the light frost under the tint. Empty on the plain frame or
    /// before the frost is baked; then the card keeps its own glass colour.
    fn glass_under(&mut self, id: &'static str, r: Rect, win: (f32, f32), dark: bool, pal: &'static Palette, moving: bool, window: &mut Window) -> Option<Vec<AnyElement>> {
        if !self.painting {
            return None;
        }
        let round = px(radius::PANEL);
        if id == "main" {
            if let Some(path) = self.stream_bg.clone() {
                return Some(vec![
                    img(path).id("stream-bg").absolute().left_0().top_0().size_full().rounded(round).object_fit(ObjectFit::Cover).test_support().into_any_element(),
                    div().absolute().left_0().top_0().size_full().rounded(round).bg(backdrop::surface(true, pal.frame, backdrop::dim_alpha(&Prefs::load()))).into_any_element(),
                ]);
            }
        }
        let mut stale = Vec::new();
        let crop = self.glass.get(id, dark, win, r, moving, &mut stale);
        for old in stale {
            let _ = window.drop_image(old);
        }
        let crop = crop?;
        Some(vec![
            img(crop).absolute().left_0().top_0().size_full().rounded(round).object_fit(ObjectFit::Fill).into_any_element(),
            div().absolute().left_0().top_0().size_full().rounded(round).bg(backdrop::tint(pal, dark)).into_any_element(),
        ])
    }

    pub(crate) fn new() -> Self {
        Cauldron { motion: ShellMotion::default(), section: 0, section_from: 0.0, section_since: None, swap: None, action_since: None, busy_shown: false, open_since: None, bubble_since: None, mine_for: None, mine_n_shown: 0, drawer_last: Some(Panel::Highlights), sidebar_open: true, drawer: None, support_tabs: vec![Panel::Highlights], support_menu: false, support_data: Default::default(), files_open: Default::default(), painting: false, glass: Default::default(), ask_hover: false, ask_open: None, ask_answered: Default::default(), fam_questions: Default::default(), frost: backdrop::FrostLevel::Heavy, stream_bg: None, theme_mode: String::new(), root_focus: None, plan: false, open_in_menu: false, open_in: thread::OpenIn::Folder, search_open: false, indent: false, usage: Default::default(), settings: None, accounts_file: None, terminal: false, term_blocks: Vec::new(), term_input: None, term_note: None, term_scroll: ScrollHandle::new(), panes: Split::default(), layout_since: None, pane_drag: None, pane_content: [panes::Content::Thread, panes::Content::Empty, panes::Content::Empty], pane_focus: Vec::new(), thread_box: std::cell::Cell::new((0.0, 0.0)), transcript_scroll: ScrollHandle::new(), transcript_seen: Default::default(), coordinator: None, familiar: None, fam_views: Vec::new(), fam_live: Vec::new(), fam_watch: Vec::new(), fam_driving: Default::default(), fam_error: None, fam_composer: None, fam_fixture: false, inbox: Vec::new(), inbox_open: false, update: Default::default(), update_open: false, inbox_toast: None, fam_from: (0.0, 0.0), fam_to: None, fam_since: None, fam_hl: 0, agents: Vec::new(), idle: Vec::new(), active: 0, reveal: false, tab_window: 0, wheel: 0.0, tab_menu: false, tab_slide: Default::default(), tab_focus: None, tip: None, tip_shown: false, tip_warm_until: None, tip_wait: None, tab_first: 0, threads: Threads::Ready(Vec::new()), selected: None, query: String::new(), search: None, list_focus: None, core: None, watch: None, open: None, thread: None, steps: Default::default(), composer: None, send_error: None, start: Default::default(), err_open: Default::default(), side_root: None, sides: Vec::new(), side_win: false, side_input: None, side_ctx: Default::default() }
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
                c.pump(cx);
                c.load_providers(cx);
                c.load_familiars(cx);
                cx.notify();
            });
        }));
    }

    /// Thread ids in the order the sidebar draws them.
    fn visible_ids(&self) -> Vec<String> {
        match &self.threads {
            Threads::Ready(v) => threads::grouped(v, &self.query).into_iter().flat_map(|(_, ts)| ts.into_iter().map(|t| t.id.clone())).collect(),
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
                let mut sk = div().id("threads-loading").flex().flex_col().pt(px(14.)).aria_label("Loading threads");
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
                format!("Threads could not load: {e}"),
                Some(("circle-alert", c(pal.bad))),
                "Threads could not load".into(),
                Some(e.to_string()),
            )),
            Threads::Ready(v) if v.is_empty() => {
                list.child(note("threads-empty", "No threads yet".into(), None, "No threads yet".into(), Some("Start one with New thread.".into())))
            }
            Threads::Ready(v) => {
                // Side streams live in their main's tab group, not as rows of their own.
                let side_n = |id: &str| v.iter().filter(|t| t.side_of.as_deref() == Some(id)).count();
                let mains: Vec<ThreadInfo> = v.iter().filter(|t| t.side_of.as_ref().is_none_or(|m| !v.iter().any(|x| &x.id == m))).cloned().collect();
                let groups = threads::grouped(&mains, &self.query);
                if groups.is_empty() {
                    let q = self.query.trim().to_string();
                    return list.child(note("threads-no-match", format!("No threads match {q}"), None, format!("No threads match \u{201c}{q}\u{201d}"), None));
                }
                let mut i = 0usize;
                for (gi, (g, ts)) in groups.into_iter().enumerate() {
                    let n = ts.len();
                    // The board's `.grp`: the name, then its count in a quieter grey.
                    list = list.child(
                        div()
                            .id(("group", gi))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .h(px(34.))
                            .flex_none()
                            .px(px(8.))
                            .pt(px(14.))
                            .pb(px(4.))
                            .text_size(px(11.5))
                            .line_height(px(16.))
                            .text_color(fg3)
                            .aria_label(format!("{}, {n}", g.label()))
                            .child(g.label())
                            .child(div().font_features(tnum()).text_color(c(pal.fg4)).child(n.to_string()))
                            .test_support(),
                    );
                    for t in ts {
                        let on = self.selected.as_deref() == Some(t.id.as_str());
                        let id = t.id.clone();
                        // A state dot, agent or not (figures belong to the Agents page): amber when it
                        // needs you (red when the reply failed), the accent with a halo while it runs,
                        // a hollow ring when done (the board's `.dot`).
                        let state = match g {
                            Group::Running => "Working",
                            Group::NeedsYou if t.failed.is_some() => "Failed",
                            Group::NeedsYou => "Needs you",
                            Group::Done => "Done",
                        };
                        let dot = div().id(("thread-dot", i)).size(px(7.)).rounded_full();
                        let dot = match state {
                            // The halo is the board's `box-shadow: 0 0 0 3px`, so the dot keeps its 7px.
                            "Working" => dot.bg(c(pal.accent)).shadow(vec![BoxShadow { color: ca(pal.accent, 0.18), offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(3.), inset: false }]),
                            "Failed" => dot.bg(c(pal.bad)),
                            "Needs you" => dot.bg(c(pal.warn)),
                            _ => dot.border_1().border_color(c(pal.fg4)),
                        };
                        let slot = div()
                            .id(("thread-av", i))
                            .w(px(7.))
                            .h(px(22.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .aria_label(state)
                            .child(dot.test_support());
                        list = list.child(
                            div()
                                .id(("thread", i))
                                .h(px(30.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .px(px(8.))
                                .rounded(px(radius::CONTROL))
                                .text_size(px(13.))
                                .text_color(if on || t.unread { fg } else { fg2 })
                                .when_live(on, |d| d.bg(c(pal.hover)))
                                .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                                .cursor_pointer()
                                .aria_label(t.title.clone())
                                .child(slot.test_support())
                                .child(div().flex_1().min_w_0().truncate().font_weight(if t.unread { FontWeight::SEMIBOLD } else { FontWeight::NORMAL }).child(t.title.clone()))
                                .when_live(t.pinned, |d| d.child(glyph("pin", 11., fg3)))
                                .when_live(side_n(&t.id) > 0, |d| {
                                    let n = side_n(&t.id);
                                    d.child(div().id(("thread-sides", i)).flex_none().flex().items_center().gap(px(2.)).text_size(px(10.5)).text_color(fg3).aria_label(format!("{n} side streams")).child(glyph("corner-down-right", 10., fg3)).child(n.to_string()).test_support())
                                })
                                // The repo as a tag (the board's `.r`).
                                .child({
                                    let m = threads::meta(t);
                                    div()
                                        .id(("thread-meta", i))
                                        .flex_none()
                                        .max_w(px(64.))
                                        .truncate()
                                        .h(px(17.))
                                        .px(px(4.))
                                        .flex()
                                        .items_center()
                                        .rounded(px(radius::TAG))
                                        .border_1()
                                        .border_color(c(pal.line))
                                        .text_size(px(10.5))
                                        .text_color(c(pal.fg4))
                                        .aria_label(m.clone())
                                        .child(m)
                                        .test_support()
                                })
                                .when_live(t.unread, |d| d.child(div().id(("ping", i)).size(px(6.)).flex_none().rounded_full().bg(c(pal.accent)).test_support()))
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

    /// One agent tab per wheel line; trackpads add up pixels, one tab per 74px.
    fn wheel_tabs(&mut self, ev: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let steps = match ev.delta {
            ScrollDelta::Lines(p) => -(p.y + p.x),
            ScrollDelta::Pixels(p) => {
                self.wheel -= f32::from(p.y + p.x);
                let st = (self.wheel / (TAB_MIN + TAB_SEP)).trunc();
                self.wheel -= st * (TAB_MIN + TAB_SEP);
                st
            }
        };
        if steps != 0.0 {
            let max = self.agents.len().saturating_sub(self.tab_window) as f32;
            self.tab_first = (self.tab_first as f32 + steps).clamp(0.0, max) as usize;
            cx.notify();
        }
    }

    fn focus_tabs(&self, window: &mut Window, cx: &mut App) {
        if let Some(f) = &self.tab_focus {
            window.focus(f, cx);
        }
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
            // A sub-agent is a stream the model starts with its `delegate` tool: this fills the box
            // with the ask, for the person to finish and send.
            .child(
                item("menu-spawn".into())
                    .text_color(c(pal.fg2))
                    .aria_label("Spawn a subagent")
                    .child(glyph("plus", 14., c(pal.fg3)))
                    .child("Spawn a subagent…")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.tab_menu = false;
                        if let Some((st, _)) = &this.composer {
                            st.update(cx, |s, cx| s.set_value("Use your delegate tool to hand this to a sub-agent on another CLI: ", window, cx));
                        }
                        cx.notify();
                    }))
                    .test_support()
                    .with_animation("in", motion::staggered(0), |d, t| d.opacity(t)),
            );
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

    /// The sidebar's foot (the board's `.foot`): you, what is left of your first subscription, the
    /// version, and Settings. Its own function: the shell's render is one large frame, and the
    /// tests' 2 MB threads overflowed once this grew inside it.
    #[inline(never)]
    fn sidebar_foot(&self, pal: &'static Palette, accent: Hsla, user: &str, initial: &str, cx: &mut Context<Self>) -> Div {
        let fg3 = c(pal.fg3);
        let fg2 = c(pal.fg2);
        let (user, initial) = (user.to_string(), initial.to_string());
        let left = self.usage_left();
        let usage_line = left.first().map_or_else(|| "No data yet".to_string(), |(_, pct)| format!("{pct}% left"));
        let version: SharedString = env!("CARGO_PKG_VERSION").into();
        // The version's dot: orange once a newer build is known, red when the check failed.
        let update_dot = match &self.update {
            update::UpdateState::Ready(i) if i.newer => c(pal.warn),
            update::UpdateState::Failed(_) => c(pal.bad),
            _ => accent,
        };
                    div()
                .h(px(36.))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(6.))
                .pl(px(4.))
                .pt(px(6.))
                .child(
                    // You: opens Settings on your agents and subscriptions (Charles, 20 Nov: it did nothing).
                    div()
                        .id("user-row")
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .ml(px(-4.))
                        .pl(px(4.))
                        .py(px(3.))
                        .rounded(px(radius::CONTROL))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .aria_label(format!("{user}, your subscriptions"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings = if this.settings == Some(0) { None } else { Some(0) };
                            cx.notify();
                        }))
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
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .line_height(px(15.))
                                .child(div().truncate().text_size(px(12.5)).child(user.clone()))
                                .child(div().id("usage-left").truncate().text_size(px(11.)).text_color(fg3).aria_label(usage_line.clone()).child(usage_line).test_support()),
                        )
                        .test_support(),
                )
                .child(
                    div()
                        .id("version")
                        .h(px(22.))
                        .px(px(7.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .rounded(px(radius::CONTROL))
                        .border_1()
                        .border_color(c(pal.line))
                        .text_size(px(11.))
                        .font_features(tnum())
                        .text_color(fg2)
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .aria_label(version.clone())
                        .child(div().size(px(5.)).rounded_full().bg(update_dot))
                        .child(version)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.update_open {
                                this.update_open = false;
                                cx.notify();
                            } else {
                                this.check_for_update(cx);
                            }
                        }))
                        .test_support(),
                )
                .child(self.tipped("settings", icon_button("settings", "settings", "Settings", pal, self.settings.is_some()), "Settings", Side::Right, pal, cx).on_click(cx.listener(|this, _, _, cx| {
                    this.settings = if this.settings.is_some() { None } else { Some(0) };
                    cx.notify();
                })).test_support())
    }

    /// Selects a function. The sidebar indicator slides to it from wherever it is now.
    fn select_section(&mut self, i: usize, cx: &mut Context<Self>) {
        if i != self.section {
            let now = std::time::Instant::now();
            self.section_from = crate::motion::ease_between(self.section_from, self.section as f32, self.section_since, now, cauldron_chrome::motion::time::INDICATOR);
            self.section = i;
            self.section_since = Some(now);
        }
        cx.notify();
    }

    /// A rail button or menu item: shows `p` in the support card, adding its tab; the open tab's
    /// own button folds the card (its tabs stay).
    fn toggle_panel(&mut self, p: Panel) {
        let was = self.drawer;
        if was == Some(p) {
            self.drawer = None;
            self.drawer_last = Some(p);
            return;
        }
        if !self.support_tabs.contains(&p) {
            self.support_tabs.push(p);
        }
        self.drawer = Some(p);
        self.drawer_last = Some(p);
        // Switching tabs while the card is open crossfades the body.
        if let Some(a) = was {
            self.swap = Some((a, std::time::Instant::now()));
        }
    }

    /// The rail's fold button: folds the support card, or opens it on its last tab.
    fn fold_support(&mut self) {
        if let Some(p) = self.drawer.take() {
            self.drawer_last = Some(p);
        } else {
            let p = self.drawer_last.filter(|p| self.support_tabs.contains(p)).or(self.support_tabs.first().copied()).unwrap_or(Panel::Highlights);
            self.toggle_panel(p);
        }
    }

    /// Each enabled subscription with a known limit and the share of it left, from accounts.json
    /// (the board's usage badges); empty while core has no usage.
    fn usage_left(&self) -> Vec<(String, u64)> {
        let mut cache = self.usage.borrow_mut();
        if let Some((at, v)) = cache.as_ref().filter(|(at, _)| at.elapsed() < USAGE_EVERY) {
            let _ = at;
            return v.clone();
        }
        let a = backspace_core::accounts::Accounts::load(&self.accounts_path());
        let v: Vec<(String, u64)> = a
            .list
            .iter()
            .filter(|a| a.enabled)
            .filter_map(|a| {
                let u = a.usage.as_ref()?;
                let (used, limit) = (u.used?, u.limit?);
                (limit > 0).then(|| (a.name.clone(), 100u64.saturating_sub(used * 100 / limit)))
            })
            .collect();
        *cache = Some((std::time::Instant::now(), v.clone()));
        v
    }

    /// The usage badges under the composer box (the board's `.ubar`): one per subscription, the
    /// low ones in the accent. None when there is nothing to show.
    pub(super) fn usage_badges(&self, pal: &'static Palette) -> Option<Stateful<Div>> {
        let left = self.usage_left();
        if left.is_empty() {
            return None;
        }
        let mut bar = div().id("usage-badges").flex().gap(px(6.)).pt(px(8.));
        for (i, (name, pct)) in left.into_iter().enumerate() {
            let low = pct < 20;
            let col = if low { c(pal.accent) } else { c(pal.fg2) };
            let label = format!("{name} {pct}%");
            bar = bar.child(
                div()
                    .id(("usage-badge", i))
                    .h(px(22.))
                    .px(px(7.))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .rounded(px(radius::CONTROL))
                    .border_1()
                    .border_color(if low { c(pal.accent) } else { c(pal.line) })
                    .text_size(px(11.))
                    .font_features(tnum())
                    .text_color(col)
                    .aria_label(label.clone())
                    .child(div().size(px(5.)).rounded_full().bg(if low { c(pal.accent) } else { c(pal.fg3) }))
                    .child(label)
                    .test_support(),
            );
        }
        Some(bar)
    }

    /// Opens the stream's folder in `how`, and remembers it for the split button's left half.
    fn open_folder(&mut self, how: thread::OpenIn, cx: &mut Context<Self>) {
        self.open_in = how;
        self.open_in_menu = false;
        let Some(dir) = self.thread.as_ref().and_then(|t| t.project.clone()) else { return };
        match how {
            thread::OpenIn::Folder => cx.reveal_path(std::path::Path::new(&dir)),
            thread::OpenIn::Editor => {
                if let Err(e) = backspace_runner::cmd("code").arg(&dir).spawn() {
                    self.send_error = Some(format!("Couldn't start VS Code: {e}"));
                }
            }
        }
    }

    /// A stream without a folder: ask for one and point the stream at it, so its CLI works there
    /// from the next turn (Charles, 21 Nov: the dim folder button looked broken).
    fn pick_stream_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.open.clone() else { return };
        let paths = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Work in this folder".into()) });
        cx.spawn_in(window, async move |this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
            let Ok(Ok(Some(ps))) = paths.await else { return };
            let Some(p) = ps.into_iter().next() else { return };
            let dir = p.display().to_string();
            let _ = this.update(cx, |c, cx| {
                let mut prefs = Prefs::load();
                prefs.touch_project(&dir);
                prefs.save();
                let res = c.core.as_ref().map(|(_, chats)| chats.set_project(&id, Some(dir.clone())));
                match res {
                    Some(Err(e)) => c.send_error = Some(format!("Couldn't set the folder: {e}")),
                    _ => {
                        if let Some(t) = c.thread.as_mut().filter(|t| t.id == id) {
                            t.project = Some(dir);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The board's open-in split button (`.oi`): the left half opens the stream's folder the way
    /// it was opened last, the arrow picks another way. Dim without a folder.
    fn open_in_button(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let has = self.thread.as_ref().is_some_and(|t| t.project.is_some());
        // Without a folder, an open stream's button picks one.
        let can_pick = !has && self.open.is_some();
        let how = self.open_in;
        let mut menu = None;
        if self.open_in_menu && has {
            let mut m = div()
                .id("open-in-menu")
                .absolute()
                .top(px(32.))
                .right_0()
                .w(px(200.))
                .p(px(4.))
                .rounded(px(radius::PANEL))
                .bg(c(pal.raised))
                .border_1()
                .border_color(c(pal.line))
                .shadow(vec![drop(tokens::shadow::MENU)])
                .text_size(px(12.5))
                .occlude()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.open_in_menu = false;
                    cx.notify();
                }));
            for (i, o) in thread::OpenIn::ALL.into_iter().enumerate() {
                m = m.child(
                    div()
                        .id(("open-in-item", i))
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .px(px(8.))
                        .rounded(px(radius::CONTROL))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .text_color(if o == how { c(pal.fg) } else { c(pal.fg2) })
                        .aria_label(o.label())
                        .child(o.label())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.open_folder(o, cx);
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            menu = Some(deferred(m.test_support()).with_priority(2));
        }
        div()
            .id("open-in-wrap")
            .relative()
            .h(px(28.))
            .flex_none()
            .flex()
            .rounded(px(7.))
            .border_1()
            .border_color(c(pal.line))
            .when_live(!has && !can_pick, |d| d.opacity(0.45))
            .occlude()
            .child(
                div()
                    .id("open-in")
                    .w(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when_live(has || can_pick, |d| d.cursor_pointer().hover(|d| d.bg(c(pal.hover))))
                    .aria_label(if has { how.label() } else { "Choose a folder" })
                    .child(glyph(if has { "folder" } else { "folder-plus" }, 14., c(pal.accent)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if has {
                            this.open_folder(how, cx);
                        } else if can_pick {
                            this.pick_stream_folder(window, cx);
                        }
                        cx.notify();
                    }))
                    .test_support(),
            )
            .child(
                div()
                    .id("open-in-more")
                    .w(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_l_1()
                    .border_color(c(pal.line))
                    .when_live(has, |d| d.cursor_pointer().hover(|d| d.bg(c(pal.hover))))
                    .aria_label("Open in")
                    .child(glyph("chevron-down", 12., c(pal.fg3)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_in_menu = has && !this.open_in_menu;
                        cx.notify();
                    }))
                    .test_support(),
            )
            .children(menu)
            .test_support()
            .into_any_element()
    }

    /// Closes a support tab; closing the last one folds the card.
    fn close_support_tab(&mut self, p: Panel) {
        let Some(i) = self.support_tabs.iter().position(|q| *q == p) else { return };
        self.support_tabs.remove(i);
        if self.drawer == Some(p) {
            self.drawer = self.support_tabs.get(i.min(self.support_tabs.len().saturating_sub(1))).copied();
            self.drawer_last = self.drawer.or(self.support_tabs.first().copied());
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
        .bg(if fill { sel } else { gpui_kit::transparent_black() })
        .hover(move |d| d.bg(if fill { sel } else { hover }))
        .child(glyph(name, 16., if on { c(pal.fg) } else { c(pal.fg2) }))
        .aria_label(label)
}

/// The sidebar's padding (the board's `.sb`: 8 8 6 10).
const SIDE_PL: f32 = 10.0;
/// An agent's figure on its tab and in lists (Charles, 20 Nov): only agents and captains get one;
/// streams do not.
const TAB_FIG: f32 = 16.0;
const SIDE_PR: f32 = 8.0;

/// How long the usage badges trust accounts.json before reading it again.
const USAGE_EVERY: std::time::Duration = std::time::Duration::from_secs(30);
/// The folded sidebar's hover indent: wide enough for the lights on macOS, one button on Windows.
fn indent_w(plat: Platform) -> f32 {
    if plat == Platform::Mac { 72.0 } else { 48.0 }
}

/// Agent tabs share the room between these widths; below TAB_MIN the row scrolls.
const TAB_MIN: f32 = 72.0;
const TAB_MAX: f32 = 200.0;
/// A header tab (the board's `.tab`): 2px between tabs, 10px padding, 6px from the name to the
/// close. No figure: agent figures belong to the Agents page.
const TAB_GAP: f32 = 2.0;
/// Between tabs: the card's tab edge, its foot and a stroke of frame, so the card's tab fits
/// around whichever tab is open.
const TAB_SEP: f32 = tabshape::S + tabshape::FOOT_R + tabshape::S;
const TAB_PAD: f32 = 10.0;
const CLOSE_GAP: f32 = 6.0;
/// What the open agent tab needs beyond the others: its close button, the gap before it and its 1px ring.
const CLOSE_W: f32 = 18.0 + CLOSE_GAP + 2.0;
/// The + after the tabs, as tall as a tab.
const ADD_W: f32 = metric::TAB_H;
/// The new stream's bar at the bottom of its card: usage and the stages.
const STAGE_H: f32 = 32.0;
/// The pinned coordinator tab is at most this wide.
const ANCHOR_MAX: f32 = 190.0;
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
        let dark = forced_dark().unwrap_or_else(|| match self.theme_mode.as_str() {
            "dark" => true,
            "light" => false,
            _ => !matches!(window.appearance(), WindowAppearance::Light | WindowAppearance::VibrantLight),
        });
        let pal: &'static Palette = if dark { &tokens::DARK } else { &tokens::LIGHT };
        let vs = window.viewport_size();
        let (w, h) = (f32::from(vs.width), f32::from(vs.height));
        let plat = platform();
        let want = ShellState { sidebar_open: self.sidebar_open, support_open: self.drawer.is_some(), dock_open: self.terminal };
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
        if let Some(t0) = self.section_since {
            if crate::motion::run(now, t0, cauldron_chrome::motion::time::INDICATOR) >= 1.0 {
                self.section_since = None;
            } else {
                moving = true;
            }
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
        let ring = ca(pal.fg, if dark { 0.08 } else { 0.09 });
        let fg = c(pal.fg);
        let fg2 = c(pal.fg2);
        let fg3 = c(pal.fg3);
        let accent = c(pal.accent);

        self.ensure_composer(window, cx);
        if self.familiar.is_some() {
            self.ensure_fam_composer(window, cx);
        }
        self.ensure_term_input(window, cx);
        self.ensure_start(window, cx);
        // Keys reach the shell's actions from any focused element inside it; with nothing focused
        // the shell itself takes the focus, so the shortcuts still work.
        let root_focus = self.root_focus.get_or_insert_with(|| cx.focus_handle()).clone();
        if window.focused(cx).is_none() {
            window.focus(&root_focus, cx);
        }
        let mut root = div()
            .id("cauldron")
            .key_context("Cauldron")
            .track_focus(&root_focus)
            .on_action(cx.listener(|this, _: &NewStream, window, cx| this.new_thread(window, cx)))
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

        // ---- the frame layer: the Harbour painting, baked frosted, under the frame tint
        // (Tight tiles rule 7). The plain frame is the root's own colour.
        let layer = self.painting.then(|| backdrop::frame_layer(dark));
        if let Some(img_src) = layer.clone() {
            root = root
                .child(img(img_src).id("backdrop").absolute().size_full().object_fit(ObjectFit::Cover).test_support())
                .child(div().absolute().size_full().bg(backdrop::frame_tint_at(pal, dark, self.frost)));
        }

        // ---- Mac: the native lights follow the board (sidebar's top row, or the stream header).
        #[cfg(target_os = "macos")]
        if let Some(l) = s.lights {
            window.set_traffic_light_position(gpui_kit::point(px(l.rect.x), px(l.rect.y)));
        }

        // ---- sidebar, flat on the frame (the board's `.sb`): the top row (lights or logo, and the
        // sidebar button, drawn on the window), a line, the apps row, New stream, Search, the list
        if let Some(sp) = s.sidebar {
            // The apps row (20 Nov): above New stream, each app its own button with its own fill and
            // border (the stream rows' look, but separate); the open one is darker. Each app swaps the list.
            let mut fnrow = div().id("apps-tray").flex().gap(px(apps::TRAY_GAP)).mt(px(if s.bar.is_some() { 0. } else { 6. })).mb(px(8.)).aria_label("Apps");
            for (i, (name, label)) in FUNCTIONS.iter().enumerate() {
                let on = i == self.section;
                let b = div()
                    .id(("fn", i))
                    .w(px(apps::TRAY_BTN_W))
                    .h(px(apps::TRAY_BTN_H))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::CONTROL))
                    .border_1()
                    .cursor_pointer()
                    .aria_label(*label)
                    .aria_selected(on)
                    .border_color(if on { c(pal.line_strong) } else { c(pal.line) })
                    .bg(ca(pal.fg, if on { if dark { 0.16 } else { 0.12 } } else if dark { 0.04 } else { 0.03 }))
                    .when_live(!on, |d| d.hover(|d| d.bg(ca(pal.fg, if dark { 0.08 } else { 0.06 }))))
                    .child(glyph(name, 15., if on { fg } else { fg3 }))
                    .on_click(cx.listener(move |this, _, _, cx| this.select_section(i, cx)));
                fnrow = fnrow.child(self.tipped(("fn", i), b, *label, Side::Right, pal, cx).test_support());
            }
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
            let familiars = self.familiar_group(pal, cx);
            let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "You".into());
            let initial = user.chars().next().map(|ch| ch.to_uppercase().to_string()).unwrap_or_else(|| "Y".into());
            // The board's `.nav`: the app's New button (New stream with its shortcut on Streams).
            let (new_id, new_icon, new_label) = Self::new_action(self.section).unwrap_or(("new-thread", "square-pen", NEW_TAB));
            let new_thread = div()
                .id(new_id)
                .mt(px(8.))
                .h(px(30.))
                .flex()
                .items_center()
                .gap(px(9.))
                .px(px(8.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .text_size(px(13.))
                .text_color(fg2)
                .aria_label(new_label)
                .child(glyph(new_icon, 15., fg2))
                .child(new_label)
                .when_live(new_id == "new-thread", |d| d.child(div().ml_auto().text_size(px(11.)).text_color(c(pal.fg4)).child(NEW_KEY)))
                .on_click(cx.listener(|this, _, window, cx| this.do_new(window, cx)))
                .test_support();
            let has_new = Self::new_action(self.section).is_some();
            // The top row is 30 high under 8px of padding; the window draws its contents.
            let card_el = pane("sidebar", sp.rect)
                .opacity(sp.alpha)
                .flex()
                .flex_col()
                .pl(px(SIDE_PL))
                .pr(px(SIDE_PR))
                .pt(px(8.))
                .pb(px(6.))
                .child(div().id("side-top").h(px(if s.bar.is_some() { 0. } else { 30. })).flex_none().window_control_area(WindowControlArea::Drag).test_support())
                .overflow_hidden()
                .when_live(self.settings.is_none(), |d| d.child(fnrow.test_support()))
                .when_live(self.settings.is_none() && has_new, |d| d.child(new_thread))
                .when_live(self.settings.is_some(), |d| d.child(self.settings_nav(self.settings.unwrap_or(0), pal, cx).test_support()))
                // The search row opens from the top row's search button and stays while it has text.
                .when_live(self.settings.is_none() && (self.search_open || !self.query.is_empty()), |d| d.child(
                    div().child(
                        div()
                            .id("search")
                            .h(px(30.))
                            .flex()
                            .items_center()
                            .gap(px(9.))
                            .px(px(8.))
                            .rounded(px(radius::CONTROL))
                            .bg(c(pal.hover))
                            .text_color(fg2)
                            .aria_label("Search streams")
                            .child(glyph("search", 15., fg2))
                            .child(div().flex_1().min_w_0().child(Input::new(&search).appearance(false).text_size(px(13.)).p_0()))
                            .test_support(),
                    ),
                ))
                .when_live(self.settings.is_none() && self.section == 0, |d| d.child(familiars).child(list))
                .when_live(self.settings.is_none() && self.section != 0, |d| d.child(self.app_list(self.section, pal, cx)))
                .child(self.sidebar_foot(pal, accent, &user, &initial, cx));
            root = root.child(card_el.test_support());
        }

        // ---- the stream's tabs, in the stream card's 38px header (the board's `.tab`). Only an open stream
        // has them: the coordinator pinned at the left, then its agents, each sized to its name
        // within an equal share of the room (TAB_MIN to TAB_MAX); when TAB_MIN no longer fits, arrows appear beside +
        // and the row shows a window of 72px tabs. No figures (Charles, 13 Nov).
        let mut tab_row = None;
        // Where the stream card's tab sits, from the card's left edge, and how wide (tabshape.rs).
        let mut bump: Option<(f32, f32)> = None;
        // A familiar's tabs are its own folder tabs on its stream card (familiar.rs).
        let fam = self.familiar.is_some().then(|| self.familiar_strips(s.main, s.support.is_none() && self.rail_on(false), pal, now, reduce, window, cx));
        if let Some(anchor_label) = self.coordinator.clone().filter(|_| self.familiar.is_none()) {
            let focus = self.tab_focus.get_or_insert_with(|| cx.focus_handle()).clone();
            let label_w = {
                let run = TextRun { len: anchor_label.len(), font: Font { weight: FontWeight::MEDIUM, features: sans_features(), ..font(SANS) }, color: fg, background_color: None, underline: None, strikethrough: None };
                f32::from(window.text_system().shape_line(anchor_label.clone(), px(12.5), &[run], None).width)
            };
            let anchor_w = (TAB_PAD + label_w + TAB_PAD + 2.0).ceil().min(ANCHOR_MAX);
            let n = self.agents.len();
            // The card's tab holds only the open tab; the others and + sit on the frame past its
            // feet (Charles, 20 Nov: "outside of the tab card, with a separation"; 21 Nov: the
            // card's tab follows the open tab), so every tab is a foot apart.
            let room = s.tabs.w - anchor_w - (TAB_SEP + ADD_W);
            let scrolls = n as f32 * (TAB_MIN + TAB_SEP) > room;
            let (k, tab_w) = if scrolls {
                (((room - 2.0 * (TAB_SEP + 24.0)) / (TAB_MIN + TAB_SEP)).floor().max(1.0) as usize, Some(TAB_MIN))
            } else if n > 0 {
                let per = ((room - TAB_SEP * n as f32 - CLOSE_W) / n as f32).floor();
                (n, Some(per.min(TAB_MAX)))
            } else {
                (0, None)
            };
            // Keep the open tab in view when it changed; the arrows and wheel may move it away after.
            let mut first = self.tab_first.min(n.saturating_sub(k));
            // A resize changes how many fit, which can push the open tab out too.
            let reveal = std::mem::take(&mut self.reveal) || k != self.tab_window;
            if reveal && self.active > 0 {
                let a = self.active - 1;
                if a < first {
                    first = a;
                } else if a >= first + k {
                    first = a + 1 - k;
                }
            }
            self.tab_first = first;
            self.tab_window = k;

            let mut row = abs(s.tabs)
                .id("tabs")
                .flex()
                .items_center()
                .gap(px(TAB_SEP))
                .track_focus(&focus)
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                    let last = this.agents.len();
                    match ev.keystroke.key.as_str() {
                        "left" => this.active = this.active.saturating_sub(1),
                        "right" => this.active = (this.active + 1).min(last),
                        "home" => this.active = 0,
                        "end" => this.active = last,
                        "delete" if this.active > 0 => this.close_agent(this.active - 1),
                        _ => return,
                    }
                    this.reveal = true;
                    cx.stop_propagation();
                    cx.notify();
                }))
                .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| this.wheel_tabs(ev, cx)));
            // Tabs occlude (or Windows would drag the window from them), which also hides the wheel
            // from the row, so each tab carries the row's wheel handler too. The open tab is the
            // `selected` fill with a hairline ring and no shadow; the rest are fg3 on the card.
            let ftab = |id: (&'static str, usize), on: bool, cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(CLOSE_GAP))
                    .h(px(tabshape::PILL_H))
                    .top(px(tabshape::PILL_DY))
                    .rounded(px(tabshape::PILL_R))
                    .when_live(on, |d| d.bg(c(pal.selected)).border_1().border_color(c(pal.line)))
                    .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                    .occlude()
                    .cursor_pointer()
                    .aria_selected(on)
                    .text_size(px(12.5))
                    .line_height(px(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if on { fg } else { fg3 })
                    .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| this.wheel_tabs(ev, cx)))
            };

            // The pinned tab: the stream, or with side streams the group's main.
            let in_side = self.familiar.is_none() && self.open.is_some() && self.open != self.side_root;
            let on = self.active == 0 && !in_side;
            let anchor = ftab(("tab", 0), on, cx)
                .flex_none()
                .w(px(anchor_w))
                .px(px(TAB_PAD))
                .aria_label(anchor_label.clone())
                .child(div().min_w_0().truncate().child(anchor_label.clone()))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.active = 0;
                    if let Some(r) = this.side_root.clone().filter(|r| this.open.as_ref() != Some(r)) {
                        this.open_thread(r);
                    }
                    this.focus_tabs(window, cx);
                    cx.notify();
                }));
            row = row.child(anchor.test_support());
            // The card's tab: one stroke round the open tab (tabshape.rs).
            let tab0 = s.tabs.x - s.main.x;
            if on {
                bump = Some((0.0, tab0 + anchor_w + tabshape::S));
            }
            let mut at = tab0 + anchor_w + TAB_SEP;
            // Side streams, in the main's group on the same line: a corner arrow, no figure.
            if self.familiar.is_none() && !self.sides.is_empty() {
                for el in self.side_tabs(&mut at, &mut bump, window, pal, cx) {
                    row = row.child(el);
                }
            }

            // Each tab is sized to its name, within its share of the room.
            let fig_w = avatar::size(TAB_FIG).0;
            for (j, agent) in self.agents.iter().enumerate().skip(first).take(k) {
                let i = j + 1;
                let name_w = {
                    let run = TextRun { len: agent.name.len(), font: Font { weight: FontWeight::MEDIUM, features: sans_features(), ..font(SANS) }, color: fg, background_color: None, underline: None, strikethrough: None };
                    f32::from(window.text_system().shape_line(agent.name.clone(), px(12.5), &[run], None).width)
                };
                let on = i == self.active;
                let group: SharedString = format!("tab{i}").into();
                let close = div()
                    .id(("tab-close", i))
                    .size(px(18.))
                    .ml_auto()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::TAG))
                    // Inactive tabs show their close on hover, over the end of the name, so a 72px tab
                    // still has room for a few letters.
                    .when_live(!on, |d| d.absolute().right(px(4.)).top(px((tabshape::PILL_H - 18.0) / 2.0)).bg(c(pal.panel)).opacity(0.).group_hover(group.clone(), |s| s.opacity(1.)))
                    .hover(|d| d.bg(ca(pal.fg, 0.08)))
                    .aria_label("Close tab")
                    .child(glyph("x", 13., fg3))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.close_agent(j);
                        this.reveal = true;
                        cx.notify();
                    }))
                    .test_support();
                let mut tab = ftab(("tab", i), on, cx)
                    .group(group)
                    .px(px(TAB_PAD))
                    .aria_label(agent.name.clone())
                    .child({
                        let (fw, fh) = avatar::size(TAB_FIG);
                        div()
                            .id(("tab-fig", i))
                            .flex_none()
                            .w(px(fw))
                            .h(px(fh))
                            .child(img(Arc::new(Image::from_bytes(ImageFormat::Svg, avatar::svg(i, Mood::Idle, Look { px: TAB_FIG, outline: false, ms: None, bust: false, ink: None }).into_bytes()))).size_full())
                            .test_support()
                    })
                    .child(div().min_w_0().truncate().child(agent.name.clone()))
                    .child(close)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.active = i;
                        this.reveal = true;
                        this.focus_tabs(window, cx);
                        cx.notify();
                    }));
                // Sized to the name, as the board, within the tab's share of the room; 72px while scrolling.
                let share = tab_w.unwrap_or(TAB_MAX);
                // The open tab also holds its close button, so it gets that much more than the others.
                let share = if on && !scrolls { share + CLOSE_W } else { share };
                let want = (TAB_PAD + fig_w + CLOSE_GAP + name_w + TAB_PAD + if on { CLOSE_W } else { 0.0 }).ceil();
                let tw = if scrolls { TAB_MIN } else { want.clamp(TAB_MIN, share.max(TAB_MIN)) };
                tab = tab.flex_none().w(px(tw));
                if on {
                    bump = Some((at - tabshape::S, tw + 2.0 * tabshape::S));
                }
                at += tw + TAB_SEP;
                // The full name as a tip once the tab may be narrower than the widest it gets.
                let tab = if share < TAB_MAX { self.tipped(("tab", i), tab, agent.name.clone(), Side::Below, pal, cx) } else { tab };
                row = row.child(tab.test_support());
            }

            let small_btn = |id: &'static str, icon: &'static str, label: &'static str, live: bool| {
                div()
                    .id(id)
                    .w(px(24.))
                    .h(px(metric::TAB_H))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::TAG))
                    .occlude()
                    .when_live(live, |d| d.cursor_pointer().hover(|d| d.bg(ca(pal.fg, 0.08))))
                    .aria_label(label)
                    .child(glyph(icon, 15., if live { fg2 } else { ca(pal.fg3, 0.45) }))
            };
            if scrolls {
                let prev = small_btn("tab-prev", "chevron-left", "Scroll tabs left", first > 0).ml_auto().on_click(cx.listener(|this, _, _, cx| {
                    this.tab_first = this.tab_first.saturating_sub(1);
                    cx.notify();
                }));
                let next = small_btn("tab-next", "chevron-right", "Scroll tabs right", first + k < n).on_click(cx.listener(move |this, _, _, cx| {
                    this.tab_first = (this.tab_first + 1).min(n.saturating_sub(k));
                    cx.notify();
                }));
                row = row.child(prev.test_support()).child(next.test_support());
            }
            let mut add = div()
                .id("tab-add")
                .relative()
                .size(px(ADD_W))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::TAG))
                .occlude()
                .cursor_pointer()
                .hover(|d| d.bg(ca(pal.fg, 0.08)))
                .aria_label(ADD_LABEL)
                .child(glyph("plus", 13., fg3))
                .on_click(cx.listener(|this, _, window, cx| {
                    // On an open stream the + starts a side stream; elsewhere it is the agents menu.
                    if this.familiar.is_none() && this.open.is_some() {
                        if this.side_win {
                            this.side_win = false;
                        } else {
                            this.open_side_win(window, cx);
                        }
                    } else {
                        this.tab_menu = !this.tab_menu;
                        this.tab_slide.reset();
                    }
                    cx.notify();
                }));
            if self.tab_menu {
                add = add.child(self.tab_menu_el(pal, cx));
            }
            if self.side_win {
                add = add.child(deferred(div().absolute().top(px(ADD_W + 6.0)).left(px(-8.0)).child(self.side_window(pal, cx))).with_priority(3));
            }
            let add = if self.tab_menu || self.side_win { add } else { self.tipped("tab-add", add, ADD_LABEL, Side::Below, pal, cx) };
            tab_row = Some(row.child(add.test_support()));
        }

        // ---- the stream card (the board's `.card`): its 38px header holds the tabs, its body the
        // stream or the New stream screen. The rail sits inside its right edge while the support
        // card is folded.
        let head_title: SharedString = if self.settings.is_some() {
            "Settings".into()
        } else {
            self.coordinator.clone().unwrap_or_else(|| if self.start.familiar && self.familiar.is_none() { apps::NEW_FAMILIAR.into() } else { NEW_TAB.into() })
        };
        let mut head = div()
            .id("card-head")
            .h(px(metric::HEAD_H))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .pl(px(s.tabs.x - s.main.x - 1.0 + TAB_PAD))
            .pr(px(14.))
            .text_size(px(12.5))
            .text_color(fg3)
            // Under a familiar's folder tabs the header is the frame they sit on.
            .when_live(self.familiar.is_some(), |d| d.bg(familiar::fam_layers(self.painting, pal).1))
            .window_control_area(WindowControlArea::Drag);
        // The title is the card's one tab when there are no stream tabs: a pill in the card's tab,
        // drawn on the window over the tab's corner pieces, as the tab row is.
        let mut title_el = None;
        if tab_row.is_none() && self.familiar.is_none() {
            let measure = |t: &str, weight: FontWeight, window: &Window| {
                let run = TextRun { len: t.len(), font: Font { weight, features: sans_features(), ..font(SANS) }, color: fg, background_color: None, underline: None, strikethrough: None };
                f32::from(window.text_system().shape_line(SharedString::from(t.to_string()), px(12.5), &[run], None).width)
            };
            let mut text_w = measure(&head_title, FontWeight::MEDIUM, window);
            let mut pill = div().flex().items_center().gap(px(4.)).px(px(TAB_PAD)).rounded(px(tabshape::PILL_R)).bg(c(pal.selected)).border_1().border_color(c(pal.line)).text_size(px(12.5)).text_color(fg3);
            if let Some(p) = self.thread.as_ref().filter(|_| self.settings.is_none()).and_then(|t| t.project.as_ref()) {
                let folder = p.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(p).to_string();
                text_w += measure(&format!("{folder} /"), FontWeight::NORMAL, window) + 4.0;
                pill = pill.child(div().id("card-crumb").flex_none().aria_label(folder.clone()).child(format!("{folder} /")).test_support());
            }
            let pill_w = (TAB_PAD + text_w + TAB_PAD + 2.0).ceil().min(s.tabs.w);
            bump = Some((0.0, s.tabs.x - s.main.x + pill_w + tabshape::S));
            pill = pill.child(
                div()
                    .id(if self.thread.is_some() { "thread-title" } else { "card-title" })
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(fg)
                    .aria_label(head_title.clone())
                    .child(head_title)
                    .test_support(),
            );
            title_el = Some(abs(Rect::new(s.tabs.x, s.main.y + tabshape::PILL_IN, pill_w, tabshape::PILL_H)).child(pill.size_full()));
        }
        let new_screen = self.open.is_none() && self.active == 0 && self.settings.is_none() && self.familiar.is_none();
        let rail_here = s.support.is_none() && self.rail_on(false);
        // The body under the header, clear of the rail when it is in this card.
        let body_w = s.main.w - 2.0 - if rail_here { metric::RAIL_W } else { 0.0 };
        // On the Windows strip the header's tools sit inside the content, under the tab (20 Nov):
        // the body starts below them, so a message never runs under the buttons (Charles, 21 Nov).
        let tools_band = if s.tools.y > s.main.y + metric::HEAD_H && s.tools.right() <= s.main.right() && self.settings.is_none() && self.familiar.is_none() {
            (s.tools.bottom() + metric::HEAD_IN - (s.main.y + 1.0 + metric::HEAD_H)).max(0.0).ceil()
        } else {
            0.0
        };
        let body_h = s.main.h - 2.0 - metric::HEAD_H - tools_band - if new_screen { STAGE_H } else { 0.0 };
        // Split (direction B): under the head the card holds up to four panes;
        // a shape that no longer fits steps back.
        let area = Rect::new(s.main.x + 1.0, s.main.y + 1.0 + metric::HEAD_H + tools_band, body_w, body_h);
        let shape = if self.settings.is_some() || self.familiar.is_some() { Shape::One } else { cauldron_chrome::panes::effective(area, self.panes.shape) };
        let split = shape != Shape::One;
        // The thread view drops what does not fit its pane, so it needs that pane's size first.
        let sp = Split { shape, ..self.panes };
        let at = self.pane_content.iter().position(|p| *p == panes::Content::Thread).unwrap_or(0).min(shape.count() - 1);
        let tb = cauldron_chrome::panes::rects(area, sp)[at];
        self.thread_box.set(if split { (tb.w, tb.h - 32.0) } else { (tb.w, tb.h) });
        let body = match self.settings {
            Some(page) => self.settings_page(page, pal, cx),
            None if self.familiar.is_some() => self.familiar_body(pal, fam.as_ref().map_or((radius::PANEL, 1.0), |f| (f.corner, f.fade)), cx),
            None => self.card_body(pal, window, cx),
        };
        let inside = if split { self.pane_stage(pal, area, shape, body, window, cx).into_any_element() } else { body };
        let mut card_el = pane("main", s.main)
            // The hover indent sits over this card's left edge: any move over the card itself means
            // the pointer left the band, even in one jump.
            .on_mouse_move(cx.listener(|this, _: &MouseMoveEvent, _, cx| {
                if this.indent {
                    this.indent = false;
                    cx.notify();
                }
            }))
            .child({
                let cut = s.notch.is_none() && bump.and_then(|b| tabshape::pieces(s.main, b)).is_some();
                let under = self.glass_under("main", s.main, (w, h), dark, pal, moving, window);

                let glass = under.is_some();
                div()
                    .id("card")
                    .size_full()
                    .relative()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(radius::PANEL))
                    // The tabbed card takes the tab's corners all round (Charles, 21 Nov).
                    .when_live(cut, |d| d.rounded(px(tabshape::TAB_R)))
                    .when_live(!glass, |d| d.bg(backdrop::card_bg(self.painting, pal)))
                    .children(under.unwrap_or_default())
                    .border_1()
                    .border_color(c(if glass { pal.line_strong } else { pal.line }))
                    .text_color(fg3)
                    .child(head.test_support())
                    .child(div().id("tools-band").w(px(body_w)).h(px(tools_band)).flex_none().test_support())
                    .child(div().w(px(body_w)).h(px(body_h)).flex_none().child(inside))
                    .test_support()
            });
        if new_screen {
            // ponytail: core has no stages or per-subscription usage yet (UI-PLAN decision 5), so the
            // bar is static: usage as an empty state, the stages as plain labels with none current.
            let mut stages = div().flex().gap(px(12.));
            for (i, name) in ["Plan", "Execute", "Verify", "Ship", "Learn"].into_iter().enumerate() {
                stages = stages.child(div().id(("stage", i)).aria_label(name).child(name).test_support());
            }
            card_el = card_el.child(
                div()
                    .id("stage-bar")
                    .absolute()
                    .left(px(1.))
                    .top(px(1.0 + metric::HEAD_H + tools_band + body_h))
                    .w(px(body_w))
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
        root = root.child(card_el.test_support());
        // The card's tab: the frame comes down right of it (tabshape.rs). Not with a familiar (its
        // own folder tabs) or the notch (it already cuts the card's top-right).
        if let (Some(b), None, None) = (bump, self.familiar, s.notch) {
            let pieces = self.tab_cut(s.main, b, (w, h), dark, pal, window);
            root = root.children(pieces);
        }
        if let Some(t) = title_el {
            root = root.child(t);
        }
        if let Some(row) = tab_row {
            root = root.child(row.test_support());
        }
        if let Some(f) = fam {
            root = root.child(f.strip.test_support()).child(f.hl_tabs.test_support());
        }

        // ---- the header's tools on the right: the layout and the dock toggle (Ctrl+J)
        if self.settings.is_none() {
            root = root.child(
                pane("tools", s.tools)
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(4.))
                    // A familiar has no folder to open.
                    .when_live(self.familiar.is_none(), |d| d.child(self.open_in_button(pal, cx)))
                    .child(self.layout_button(pal, now, reduce, cx).occlude().test_support())
                    .child(self.tipped("top-term", icon_button("top-term", "panel-bottom", "Terminal", pal, self.terminal), "Terminal", Side::Below, pal, cx).occlude().on_click(cx.listener(|this, _, _, cx| {
                        this.terminal = !this.terminal;
                        this.term_scroll.scroll_to_bottom();
                        cx.notify();
                    })).test_support())
                    .test_support(),
            );
        }

        // ---- the terminal, docked under the stream
        if let Some(tp) = s.dock {
            root = root.child(self.terminal_card(pal, tp.rect, tp.alpha, cx).test_support());
        }

        // ---- the support card (the board's `.card.sup`): its own tabs (three in view, the rest
        // under "+N") and a + menu; the rail sits inside its right edge.
        if let (Some(dp), Some(p)) = (s.support, self.drawer.or(self.drawer_last)) {
            let r = dp.rect;
            self.load_panel(p, cx);
            // A panel's body: absolute so two bodies can cross-fade in the same place.
            let body = |this: &Self, p: Panel, a: f32, cx: &mut Context<Self>| div().absolute().inset_0().opacity(a).child(this.support_body(p, pal, cx));
            let mut shead = div()
                .id("support-head")
                .h(px(metric::HEAD_H))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(TAB_GAP))
                .pl(px(metric::HEAD_IN - 1.0))
                .pr(px(metric::HEAD_PR + s.notch.map_or(0.0, |n| n.w)))
                .window_control_area(WindowControlArea::Drag);
            // The open tab stays in view: when it is past the first three it takes the third place.
            let open_at = self.support_tabs.iter().position(|q| Some(*q) == self.drawer);
            let mut shown: Vec<Panel> = self.support_tabs.iter().copied().take(SUPPORT_TABS_SHOWN).collect();
            if let Some(i) = open_at.filter(|i| *i >= SUPPORT_TABS_SHOWN) {
                shown[SUPPORT_TABS_SHOWN - 1] = self.support_tabs[i];
            }
            for q in shown.iter().copied() {
                let i = self.support_tabs.iter().position(|t| *t == q).unwrap_or(0);
                let on = self.drawer == Some(q);
                shead = shead.child(
                    // Tabs give way before the + does: with the Windows notch reserved on the
                    // right, three long tabs and "+N" would push + under the caption buttons.
                    div()
                        .id(("st", i))
                        .h(px(metric::TAB_H))
                        .flex_shrink_1()
                        .min_w(px(TAB_PAD * 2.0 + 13.0))
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .gap(px(CLOSE_GAP))
                        .px(px(TAB_PAD))
                        .rounded(px(radius::CONTROL))
                        .when_live(on, |d| d.bg(c(pal.selected)).border_1().border_color(c(pal.line)))
                        .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                        .occlude()
                        .cursor_pointer()
                        .aria_selected(on)
                        .aria_label(q.title())
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if on { fg } else { fg3 })
                        .child(glyph(q.icon(), 13., if on { fg2 } else { fg3 }))
                        .child(div().min_w(px(0.)).truncate().child(q.title()))
                        .when_live(on, |d| {
                            d.child(
                                div()
                                    .id(("st-close", i))
                                    .size(px(16.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(radius::TAG))
                                    .hover(|d| d.bg(ca(pal.fg, 0.08)))
                                    .aria_label("Close tab")
                                    .child(glyph("x", 12., fg3))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.close_support_tab(q);
                                        cx.notify();
                                    }))
                                    .test_support(),
                            )
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.drawer != Some(q) {
                                this.toggle_panel(q);
                            }
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            let hidden: Vec<Panel> = self.support_tabs.iter().copied().filter(|q| !shown.contains(q)).collect();
            if let Some(next) = hidden.first().copied() {
                let label: SharedString = format!("+{}", hidden.len()).into();
                shead = shead.child(
                    div()
                        .id("support-more")
                        .h(px(metric::TAB_H))
                        .flex_none()
                        .flex()
                        .items_center()
                        .px(px(8.))
                        .rounded(px(radius::CONTROL))
                        .occlude()
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .text_size(px(12.5))
                        .text_color(fg3)
                        .aria_label(label.clone())
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_panel(next);
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            let mut add = div()
                .id("support-add")
                .relative()
                .size(px(metric::TAB_H))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::CONTROL))
                .occlude()
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .aria_label("Add to the support card")
                .child(glyph("plus", 14., fg3))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.support_menu = !this.support_menu;
                    cx.notify();
                }));
            if self.support_menu {
                // Opens under +, its right edge on the button's, so it stays inside the card.
                let mut menu = div()
                    .id("support-menu")
                    .absolute()
                    .top(px(metric::TAB_H + 4.0))
                    .right_0()
                    .w(px(220.))
                    .p(px(4.))
                    .flex()
                    .flex_col()
                    .rounded(px(radius::PANEL))
                    .bg(c(pal.raised))
                    .border_1()
                    .border_color(c(pal.line))
                    .shadow(vec![drop(tokens::shadow::MENU)])
                    .text_size(px(12.5))
                    .occlude()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.support_menu = false;
                        cx.notify();
                    }));
                for (i, item) in SUPPORT_MENU.into_iter().enumerate() {
                    let (icon, name, key) = match item {
                        Some(q) => (q.icon(), q.title(), ""),
                        None => ("terminal", "Terminal", "Ctrl+J"),
                    };
                    menu = menu.child(
                        div()
                            .id(("support-menu-item", i))
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(8.))
                            .rounded(px(radius::CONTROL))
                            .cursor_pointer()
                            .hover(|d| d.bg(c(pal.hover)))
                            .text_color(fg2)
                            .aria_label(name)
                            .child(glyph(icon, 14., fg3))
                            .child(div().flex_1().child(name))
                            .child(div().text_size(px(11.)).font_family(MONO).text_color(c(pal.fg4)).child(key))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                // The menu sits inside +, which would open it again.
                                cx.stop_propagation();
                                this.support_menu = false;
                                match item {
                                    Some(q) => {
                                        if this.drawer != Some(q) {
                                            this.toggle_panel(q);
                                        }
                                    }
                                    None => this.terminal = true,
                                }
                                cx.notify();
                            }))
                            .test_support(),
                    );
                }
                add = add.child(deferred(menu.test_support()).with_priority(2));
            }
            shead = shead.child(add.test_support());
            // Both bodies share one box under the header, so the outgoing one cross-fades in place.
            let mut bodies = div().relative().w(px(r.w - 2.0 - metric::RAIL_W)).flex_1().min_h_0().child(body(self, p, swap.map_or(1.0, |(_, k)| k), cx));
            if let Some((a, k)) = swap {
                bodies = bodies.child(body(self, a, 1.0 - k, cx));
            }
            let under = self.glass_under("support", r, (w, h), dark, pal, moving, window);
            let glass = under.is_some();
            let d = pane("support", r)
                .opacity(dp.alpha)
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(radius::PANEL))
                .when_live(!glass, |d| d.bg(backdrop::card_bg(self.painting, pal)))
                .children(under.unwrap_or_default())
                .border_1()
                .border_color(c(if glass { pal.line_strong } else { pal.line }))
                .child(shead.test_support())
                .child(bodies);
            root = root.child(d.test_support());
        }

        // ---- the quick-button rail, inside the rightmost card's right edge (the board's `.irail`):
        // Highlights, Changes, Git, Trace, a rule, Browser, Files, Simulator, Terminal; the fold
        // toggle at the bottom. A short accent mark shows a panel that has a tab.
        if self.rail_on(s.support.is_some()) {
            let rail_button = |this: &Self, id: (&'static str, usize), p: Panel, cx: &mut Context<Self>| {
                // The mark shows the open panel only, not every panel that has a tab (Charles, 20 Nov).
                let has_tab = this.drawer == Some(p);
                this.tipped(id, icon_button(id, p.icon(), p.title(), pal, this.drawer == Some(p)), p.title(), Side::Left, pal, cx)
                    .when_live(has_tab, |d| d.child(div().absolute().left(px(-8.)).top(px(9.)).w(px(3.)).h(px(10.)).rounded(px(2.)).bg(accent)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_panel(p);
                        cx.notify();
                    }))
                    .test_support()
            };
            let mut rr = pane("rail", s.rail).flex().flex_col().items_center().gap(px(4.)).pt(px(6.)).pb(px(8.)).border_l_1().border_color(c(pal.line));
            for (i, p) in RAIL_TOP.iter().enumerate() {
                rr = rr.child(rail_button(self, ("rb", i), *p, cx));
            }
            rr = rr.child(div().w(px(20.)).h(px(1.)).my(px(4.)).bg(c(pal.line)));
            for (i, p) in RAIL_MID.iter().enumerate() {
                rr = rr.child(rail_button(self, ("rb", RAIL_TOP.len() + i), *p, cx));
            }
            rr = rr.child(
                self.tipped("rb-term", icon_button("rb-term", "terminal", "Terminal", pal, self.terminal), "Terminal", Side::Left, pal, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.terminal = !this.terminal;
                        this.term_scroll.scroll_to_bottom();
                        cx.notify();
                    }))
                    .test_support(),
            );
            let fold = if self.drawer.is_some() { "Hide support" } else { "Show support" };
            rr = rr.child(div().flex_1()).child(
                self.tipped("rb-fold", icon_button("rb-fold", "panel-right", fold, pal, false), fold, Side::Left, pal, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.fold_support();
                        cx.notify();
                    }))
                    .test_support(),
            );
            root = root.child(rr.test_support());
        }

        // ---- Windows: the top strip across the window (Charles, 20 Nov, option B). A drag area,
        // so it must come BEFORE every button drawn on it: Windows asks the topmost hitbox under the
        // pointer, and a strip painted over the sidebar, search, inbox and pill buttons made every
        // click on them a window drag (Charles's 20 Nov Windows test: none of them responded).
        if let Some(bar) = s.bar {
            root = root.child(pane("bar", bar).window_control_area(WindowControlArea::Drag).test_support());
        }
        // ---- Windows: the logo and name in the sidebar's top row
        if let Some(tp) = s.title {
            let t = tp.rect;
            root = root.child(abs(Rect::new(t.x, t.y + 5.0, 18.0, 18.0)).opacity(tp.alpha).rounded(px(9.)).bg(accent));
            root = root.child(
                abs(Rect::new(t.x + 26.0, t.y, t.w - 26.0, t.h))
                    .opacity(tp.alpha)
                    .flex()
                    .items_center()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(fg)
                    .child("Cauldron"),
            );
        }
        // The line under the sidebar's top row: the app frame above (lights or logo, the sidebar
        // button), the sidebar's elements below (Charles, 15 Nov).
        if let Some(sb) = s.sidebar.filter(|_| s.bar.is_none()) {
            root = root.child(abs(Rect::new(sb.rect.x, sb.rect.y + 41.0, sb.rect.w, 1.0)).opacity(sb.alpha).bg(c(pal.line_strong)));
        }

        // ---- search, in the sidebar's top row beside the sidebar button (the board's `.brand`)
        // On Windows the search and inbox buttons live in the top bar and stay with it when the sidebar folds.
        if let Some(alpha) = s.sidebar.map(|sb| sb.alpha).or(s.bar.map(|_| 1.0)) {
            root = root.child(
                pane("search-slot", s.search_button)
                    .opacity(alpha)
                    .child(
                        self.tipped("search-btn", icon_button("search-btn", "search", "Search streams", pal, self.search_open), "Search streams", Side::Below, pal, cx)
                            .occlude()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.search_open = !this.search_open;
                                if let (true, Some((st, _))) = (this.search_open, this.search.as_ref()) {
                                    st.update(cx, |s, cx| s.focus(window, cx));
                                }
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .test_support(),
            );
        }

        // ---- the inbox bell, beside search (Inbox board, direction A), and its popover under it
        if let Some(alpha) = s.sidebar.map(|sb| sb.alpha).or(s.bar.map(|_| 1.0)) {
            let b = s.bell_button;
            let label = self.inbox_label();
            let unread = self.inbox.iter().filter(|n| !n.read).count();
            let mut bell = icon_button("inbox-btn", "bell", "Inbox", pal, self.inbox_open).relative().aria_label(label.clone());
            if unread > 0 {
                bell = bell.child(
                    div()
                        .absolute()
                        .top(px(1.))
                        .right(px(0.))
                        .h(px(14.))
                        .min_w(px(14.))
                        .px(px(3.))
                        .rounded_full()
                        .bg(c(pal.accent))
                        .text_color(c(pal.on_accent))
                        .text_size(px(9.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(if unread > 9 { "9+".to_string() } else { unread.to_string() }),
                );
            }
            root = root.child(
                pane("inbox-slot", b)
                    .opacity(alpha)
                    .child(
                        // No tooltip while the popover is open: it would sit over the popover's head.
                        if self.inbox_open { bell } else { self.tipped("inbox-btn", bell, label, Side::Below, pal, cx) }
                            .occlude()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.inbox_open = !this.inbox_open;
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .test_support(),
            );
            if self.inbox_open {
                root = root.child(abs(Rect::new(b.x, b.y + metric::SBTN + 6.0, 300.0, 400.0)).child(self.inbox_popover(pal, cx).test_support()));
            }
        }
        // The version button's update card, above the user row, over every pane.
        if let (true, Some(sp)) = (self.update_open, s.sidebar) {
            root = root.child(div().absolute().left(px(sp.rect.x + 6.0)).bottom(px(h - (sp.rect.y + sp.rect.h) + 44.0)).child(self.update_card(pal, cx).test_support()));
        }
        if let Some(t) = self.inbox_toast(pal, cx) {
            let vp = window.viewport_size();
            root = root.child(abs(Rect::new(f32::from(vp.width) - 280.0 - 16.0, f32::from(vp.height) - 16.0 - 80.0, 280.0, 80.0)).flex().flex_col().justify_end().child(t.test_support()));
        }

        // ---- sidebar button: the sidebar's top row when open; the stream header when folded
        root = root.child(
            pane("sbtn-slot", s.sidebar_button).child(
                // occlude: buttons over a drag area stop the hit test, or Windows drags instead of clicking.
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
        // Windows notch, folded: a short rule between the button and the tabs.
        if s.sidebar.is_none() && s.lights.is_none() && s.bar.is_none() {
            let b = s.sidebar_button;
            root = root.child(abs(Rect::new(b.right() + 7.0, b.y + 6.0, 1.0, 16.0)).bg(c(pal.line)));
        }
        // ---- macOS folded: New stream after the sidebar button
        if let Some(np) = s.new_button {
            root = root.child(
                pane("newb-slot", np.rect)
                    .opacity(np.alpha)
                    .child(
                        self.tipped("newb", icon_button("newb", "square-pen", NEW_TAB, pal, false), NEW_TAB, Side::Below, pal, cx)
                            .occlude()
                            .on_click(cx.listener(|this, _, window, cx| this.new_thread(window, cx)))
                            .test_support(),
                    )
                    .test_support(),
            );
        }

        // ---- Windows notch option: cut into the rightmost card's top-right, with the caption buttons
        if let Some(n) = s.notch {
            // A cut, not a pane: over the painting it shows the same frame layer, cropped to it.
            let mut notch = pane("notch", n).bg(frame).overflow_hidden().rounded_bl(px(radius::PANEL));
            if let Some(img_src) = layer.clone() {
                notch = notch
                    .child(img(img_src).absolute().left(px(-n.x)).top(px(-n.y)).w(px(w)).h(px(h)).object_fit(ObjectFit::Cover))
                    .child(div().absolute().size_full().bg(backdrop::frame_tint_at(pal, dark, self.frost)));
            }
            root = root.child(
                notch
                    .child(div().absolute().size_full().border_l_1().border_b_1().border_color(c(pal.line)).rounded_bl(px(radius::PANEL)))
                    .window_control_area(WindowControlArea::Drag)
                    .test_support(),
            );
        }
        if let Some(p) = s.pill {
            let n = self.needs_you();
            let label: SharedString = format!("{n} needs you").into();
            root = root.child(
                pane("needs-pill", p)
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .rounded(px(radius::CONTROL))
                    .border_1()
                    .border_color(c(pal.line_strong))
                    .text_size(px(12.))
                    .text_color(fg2)
                    .opacity(if n > 0 { 1.0 } else { 0.0 })
                    .when_live(n > 0, |d| d.occlude().cursor_pointer().hover(|d| d.bg(c(pal.hover))))
                    .on_click(cx.listener(|this, _, _, cx| this.open_needs_you(cx)))
                    .aria_label(label.clone())
                    .child(div().size(px(6.)).rounded_full().bg(c(pal.accent)))
                    .child(label)
                    .test_support(),
            );
        }
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
                    .when_live(s.notch.is_some(), |d| d.rounded(px(radius::CONTROL)))
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

        // ---- the folded sidebar's hover indent (Tight tiles rule 6): the pointer at the window's
        // left edge shows a band of the frame layer over the card, with no reflow: search, New
        // stream, the apps, Settings and you. A rounded hairline is the card's edge beside it.
        if s.sidebar.is_none() && self.settings.is_none() {
            let bw = indent_w(plat);
            // On Windows the band starts under the top bar, so the bar's buttons stay in reach.
            let by = s.bar.map_or(0.0, |b| b.h);
            root = root.child(
                div()
                    .id("indent-zone")
                    .absolute()
                    .left_0()
                    .top(px(by))
                    .w(px(8.))
                    .h(px(h - by))
                    // It overlaps the card's first 4px; the card must not see the pointer here.
                    .occlude()
                    .on_hover(cx.listener(|this, on: &bool, _, cx| {
                        if *on && !this.indent {
                            this.indent = true;
                            cx.notify();
                        }
                    }))
                    .test_support(),
            );
            if self.indent {
                let x0 = (bw - metric::SBTN) / 2.0;
                let mut band = div()
                    .id("indent")
                    .absolute()
                    .left_0()
                    .top(px(by))
                    .w(px(bw))
                    .h(px(h - by))
                    .overflow_hidden()
                    .bg(frame)
                    .occlude()
                    .on_hover(cx.listener(|this, on: &bool, _, cx| {
                        if !*on && this.indent {
                            this.indent = false;
                            cx.notify();
                        }
                    }));
                if let Some(img_src) = layer.clone() {
                    band = band
                        .child(img(img_src).absolute().left_0().top(px(-by)).w(px(w)).h(px(h)).object_fit(ObjectFit::Cover))
                        .child(div().absolute().size_full().bg(backdrop::frame_tint_at(pal, dark, self.frost)));
                }
                let at = |y: f32, el: AnyElement| div().absolute().left(px(x0)).top(px(y)).child(el);
                // Below the lights on macOS, in their place on Windows.
                let top = if plat == Platform::Mac { 47.0 } else { 10.0 };
                band = band.child(at(top, self.tipped("indent-search", icon_button("indent-search", "search", "Search streams", pal, false), "Search streams", Side::Right, pal, cx).on_click(cx.listener(|this, _, _, cx| {
                    this.sidebar_open = true;
                    this.search_open = true;
                    this.indent = false;
                    cx.notify();
                })).test_support().into_any_element()));
                band = band.child(at(top + 32.0 + if plat == Platform::Mac { 0.0 } else { 5.0 }, self.tipped("indent-new", icon_button("indent-new", "square-pen", NEW_TAB, pal, false), NEW_TAB, Side::Right, pal, cx).on_click(cx.listener(|this, _, window, cx| {
                    this.indent = false;
                    this.new_thread(window, cx);
                })).test_support().into_any_element()));
                let rule_y = top + if plat == Platform::Mac { 68.0 } else { 73.0 };
                band = band.child(div().absolute().left(px(x0 + 3.0)).top(px(rule_y)).w(px(22.)).h(px(1.)).bg(c(pal.line)));
                for (i, (name, label)) in FUNCTIONS.iter().enumerate() {
                    let b = icon_button(("indent-fn", i), name, label, pal, i == self.section).on_click(cx.listener(move |this, _, _, cx| this.select_section(i, cx)));
                    band = band.child(at(rule_y + 9.0 + 32.0 * i as f32, self.tipped(("indent-fn", i), b, *label, Side::Right, pal, cx).test_support().into_any_element()));
                }
                band = band.child(at(h - by - 70.0, self.tipped("indent-settings", icon_button("indent-settings", "settings", "Settings", pal, false), "Settings", Side::Right, pal, cx).on_click(cx.listener(|this, _, _, cx| {
                    this.settings = Some(0);
                    this.sidebar_open = true;
                    this.indent = false;
                    cx.notify();
                })).test_support().into_any_element()));
                let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "You".into());
                let initial = user.chars().next().map(|ch| ch.to_uppercase().to_string()).unwrap_or_else(|| "Y".into());
                band = band.child(
                    div()
                        .absolute()
                        .left(px((bw - 24.0) / 2.0))
                        .top(px(h - by - 34.0))
                        .size(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(accent)
                        .text_size(px(11.))
                        .text_color(c(pal.on_accent))
                        .child(initial),
                );
                root = root.child(band.test_support());
                // The card's edge beside the band: a hairline with rounded ends, no shadow.
                root = root.child(
                    div()
                        .id("indent-edge")
                        .absolute()
                        .left(px(bw))
                        .top(px(s.main.y))
                        .w(px(9.))
                        .h(px(s.main.h))
                        .border_l_1()
                        .border_t_1()
                        .border_b_1()
                        .border_color(c(pal.line))
                        .rounded_l(px(radius::PANEL))
                        .test_support(),
                );
            }
        } else if self.indent {
            self.indent = false;
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
    use gpui_kit::{px, size, AppContext, ElementId, TestAppContext, Window};

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

    /// Every pane the shell draws sits exactly where `cauldron_chrome::layout::solve` puts it, in
    /// all eight combinations of sidebar, support card and dock.
    #[gpui_kit::test]
    fn panes_match_the_layout_solver_in_every_state(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        for bits in 0..8u8 {
            let state = ShellState { sidebar_open: bits & 1 != 0, support_open: bits & 2 != 0, dock_open: bits & 4 != 0 };
            cx.update(|cx| cx.set_reduce_motion(true));
            let handle = cx.add_window(move |_, _| Cauldron {
                sidebar_open: state.sidebar_open,
                drawer: state.support_open.then_some(Panel::Highlights),
                terminal: state.dock_open,
                // A stream is open, so its rail shows.
                open: Some("s".into()),
                ..Cauldron::new()
            });
            cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                let s = Frame::at_rest(&layout::solve(W, H, state, platform()));
                assert_rect(window, "main", s.main, state);
                assert_rect(window, "sbtn-slot", s.sidebar_button, state);
                assert_rect(window, "tools", s.tools, state);
                assert_rect(window, "rail", s.rail, state);
                for (id, r) in [("sidebar", s.sidebar), ("support", s.support), ("terminal", s.dock), ("caps", s.caps), ("newb-slot", s.new_button)] {
                    match r.map(|p| p.rect) {
                        Some(r) => assert_rect(window, id, r, state),
                        None => assert!(window.try_find(id).is_none(), "{id} drawn in {state:?} but solve() has none"),
                    }
                }
                match s.bar {
                    Some(b) => assert_rect(window, "bar", b, state),
                    None => assert!(window.try_find("bar").is_none() && window.try_find("notch").is_none(), "no bar on this platform"),
                }
                assert_eq!(window.try_find("needs-pill").is_some(), s.pill.is_some(), "the needs-you pill only where solve() has one");
                assert_rect(window, "search-slot", s.search_button, state);
                assert_rect(window, "inbox-slot", s.bell_button, state);
            })
            .unwrap();
        }
    }

    /// The support card (Tight tiles rule 2): a rail button opens it on that panel and adds its tab;
    /// three tabs show, the rest collapse into "+N"; the open tab's rail button folds it, its tabs
    /// kept; the + menu adds History; closing the last tab folds it.
    #[gpui_kit::test]
    fn the_support_card_has_its_own_tabs_and_a_plus_menu(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| cx.set_reduce_motion(true));
        let handle = cx.add_window(|_, _| Cauldron { open: Some("s".into()), ..Cauldron::new() });
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        // The shell moves on the wall clock, so start each check from rest.
        let rest = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| c.motion = Default::default()).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("support").is_none(), "folded by default (the board's first frame)");
            let (rail, main) = (rect(window, "rail"), rect(window, "main"));
            assert!(rail.right() < main.right() && rail.x > main.x, "folded: the rail is in the stream card");
            window.click(("rb", 1usize), cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let sup = rect(window, "support");
            let rail = rect(window, "rail");
            assert!(rail.x > sup.x && rail.right() < sup.right(), "open: the rail is in the support card");
            assert_eq!(window.find(("st", 1usize)).label(), Some("Changes"), "the rail added a Changes tab");
            assert_eq!(window.find(("st", 1usize)).selected(), Some(true), "and opened it");
            assert_eq!(window.find(("st", 0usize)).selected(), Some(false), "Highlights stays as a tab");
            for i in [2usize, 3, 4] {
                window.click(("rb", i), cx);
            }
            assert_eq!(window.find("support-more").label(), Some("+2"), "five tabs: three in view, +2");
            assert_eq!(window.find(("st", 4usize)).selected(), Some(true), "the open tab stays in view");
            window.click("support-add", cx);
            assert_eq!(window.find(("support-menu-item", 5usize)).label(), Some("History"));
            assert_eq!(window.find(("support-menu-item", 1usize)).label(), Some("Terminal"));
            window.click(("support-menu-item", 5usize), cx);
            window.render_frame(cx);
            assert!(window.try_find("support-menu").is_none(), "the menu closes");
            assert_eq!(window.find(("st", 5usize)).label(), Some("History"));
            window.click(("rb", 1usize), cx);
            window.click(("rb", 1usize), cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("support").is_none(), "the open tab's rail button folds the card");
            window.click("rb-fold", cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("st", 1usize)).selected(), Some(true), "the fold button opens it on its last tab");
        })
        .unwrap();
        handle
            .update(cx, |c, _, _| {
                for p in c.support_tabs.clone() {
                    c.close_support_tab(p);
                }
                assert!(c.drawer.is_none() && c.support_tabs.is_empty(), "closing the last tab folds the card");
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
        let handle = cx.add_window(|_, _| Cauldron { open: Some("s".into()), ..Cauldron::new() });
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        let ms = std::time::Duration::from_millis;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let named: [(ElementId, &str); 8] = [
                (("fn", 4usize).into(), "Usage"),
                (("rb", 0usize).into(), "Highlights"),
                (("rb", 1usize).into(), "Changes"),
                ("rb-term".into(), "Terminal"),
                ("rb-fold".into(), "Show support"),
                ("sbtn".into(), "Hide sidebar"),
                ("top-term".into(), "Terminal"),
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
            assert_eq!(window.find("tooltip").label(), Some("Changes"));
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
            window.hover(("fn", 4usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find("tooltip").label(), Some("Usage"));
            let (tip, btn) = (rect(window, "tooltip"), rect(window, ("fn", 4usize)));
            assert!((tip.x - (btn.right() + 8.0)).abs() < 1.5, "left-side tip sits 8px right of its button: {tip:?} {btn:?}");
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

    /// A test's own accounts.json path, absent unless the test writes it: never the user's.
    pub(super) fn no_accounts() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cauldron-accounts-none-{}.json", std::process::id()))
    }

    fn open(cx: &mut TestAppContext, mut c: Cauldron) -> gpui_kit::WindowHandle<Cauldron> {
        cx.update(|cx| cx.set_reduce_motion(true));
        c.accounts_file.get_or_insert_with(no_accounts);
        let handle = cx.add_window(move |_, _| c);
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        handle
    }

    fn visible(window: &Window, n: usize) -> Vec<usize> {
        (1..=n).filter(|&i| window.try_find(("tab", i)).is_some()).collect()
    }

    fn dt_on(window: &Window, i: usize) -> bool {
        window.find(("dt", i)).selected() == Some(true)
    }

    fn on(window: &Window, i: usize) -> bool {
        window.find(("tab", i)).selected() == Some(true)
    }

    /// The New stream screen has no tabs; the card is a tight tile with no top bar above it, its 38px
    /// header says New stream where the tabs start, and the stage bar sits at the card's bottom.
    #[gpui_kit::test]
    fn new_thread_screen_has_no_tabs_and_a_card_head(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, Cauldron::new());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let s = layout::solve(W, H, ShellState { sidebar_open: true, support_open: false, dock_open: false }, platform());
            assert!(window.try_find("tabs").is_none() && window.try_find(("tab", 0usize)).is_none(), "no tab before the stream exists");
            assert!(window.try_find("tab-add").is_none(), "no +");
            let card = rect(window, "card");
            assert_eq!((card.x, card.y, card.w, card.h), (s.main.x, s.main.y, s.main.w, s.main.h), "the card is the solver's main, 4px from the top");
            // Core has no stages or usage yet, so usage is an empty state and no stage is current:
            // plain fg3 labels, nothing to click.
            let bar = rect(window, "stage-bar");
            assert_eq!((bar.x, bar.w, bar.h), (card.x + 1.0, card.w - 2.0, 32.0), "stage bar card-wide: the rail is for an open stream, not the New stream screen");
            assert!(window.try_find("rail").is_none(), "no right rail on the New stream screen (Charles, 20 Nov)");
            assert_eq!(bar.bottom(), card.bottom() - 1.0, "at the card's bottom, inside its border");
            assert_eq!(window.find("stage-usage").label(), Some("No usage data yet"));
            let mut last = rect(window, "stage-usage").right();
            for (i, name) in ["Plan", "Execute", "Verify", "Ship", "Learn"].into_iter().enumerate() {
                let st = rect(window, ("stage", i));
                assert_eq!(window.find(("stage", i)).label(), Some(name));
                assert!(st.x > last && st.y >= bar.y && st.bottom() <= bar.bottom(), "stage {i} in order inside the bar");
                assert_ne!(window.find(("stage", i)).selected(), Some(true), "stage {i}: none current before a stream exists");
                last = st.right();
            }
            assert!(bar.right() - last <= 13.0, "the stages end at the bar's right padding");
            let head = rect(window, "card-head");
            assert_eq!((head.x, head.y, head.w, head.h), (card.x + 1.0, card.y + 1.0, card.w - 2.0, 38.0), "head");
            assert_eq!(window.find("card-title").label(), Some("New stream"));
            assert_eq!(rect(window, "card-title").x, s.tabs.x + 1.0 + 10.0, "the title where a tab's label would start, inside its pill's border");
        })
        .unwrap();
    }

    /// A stream's tabs live in the stream card's 38px header (the board's `.tab`): 28 high, 2px
    /// apart, the open one selected, + the same height.
    #[gpui_kit::test]
    fn thread_tabs_float_in_the_bar(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_agents(3, &[]));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let s = layout::solve(W, H, ShellState { sidebar_open: true, support_open: false, dock_open: false }, platform());
            let row = rect(window, "tabs");
            assert_eq!((row.x, row.y, row.w, row.h), (s.tabs.x, s.tabs.y, s.tabs.w, s.tabs.h), "tab row");
            let head = rect(window, "card-head");
            assert!(row.y >= head.y && row.bottom() <= head.bottom(), "in the card's header");
            for i in 0..=3usize {
                let t = rect(window, ("tab", i));
                assert_eq!((t.y, t.h), (row.y + 2.0 + super::tabshape::PILL_DY, super::tabshape::PILL_H), "tab {i}: slim pill, centred in the card's tab");
                assert_eq!(on(window, i), i == 0, "tab {i}: only the coordinator is open");
            }
            for i in 1..=3usize {
                // Every tab is a foot apart, so the card's tab fits round whichever is open.
                let want = super::TAB_SEP;
                assert_eq!(rect(window, ("tab", i)).x - rect(window, ("tab", i - 1)).right(), want, "between tabs {} and {i}", i - 1);
            }
            let add = rect(window, "tab-add");
            assert_eq!((add.y, add.h), (row.y, 28.0), "+ keeps the row's height");
            window.click(("tab", 2usize), cx);
            assert!(on(window, 2) && !on(window, 0), "a click opens the tab");
            assert_eq!(rect(window, "card").y, s.main.y, "the card stays put");
        })
        .unwrap();
    }

    /// A familiar's tab row: its own tab first, then one neutral tab per subagent; its
    /// view replaces the stream card's body and the sidebar lists it under Familiars.
    #[gpui_kit::test]
    fn a_familiar_pins_its_tab_first_and_shows_its_crew(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut c = Cauldron::new();
        c.fam_views = super::familiar::fixtures();
        c.fam_fixture = true;
        c.open_familiar(0);
        let n = c.agents.len();
        let handle = open(cx, c);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("familiar", 0usize)).label(), Some("Ship onboarding v2"));
            assert!(window.try_find("familiar").is_some(), "the familiar's view is the card body");
            assert!(window.try_find("stage-bar").is_none(), "no New stream bar");
            let main = rect(window, ("tab", 0usize));
            assert!(rect(window, ("tab", 1usize)).x >= main.right() + 2.0, "subagents after the familiar's tab, 2px apart");
            assert!(window.try_find(("tab-close", 0usize)).is_none(), "the familiar has no close");
            assert_eq!(visible(window, n).len(), n, "every subagent has a tab");
            assert!(window.try_find("familiar-ask").is_some(), "the ask shows on the familiar's own stream");
            window.click(("tab", 4usize), cx);
            assert!(window.try_find("familiar-ask").is_none() && window.try_find("sub-ticket").is_some(), "a subagent tab shows that subagent's stream");
        })
        .unwrap();
    }

    /// T1 (Charles, 16 Nov): a familiar's folder tabs sit on its stream card's top edge, within the
    /// card's left and right, never over Highlights, which has its own tabs on the same line; no
    /// title row between the tabs and the stream. The open tab is the joined shape, and the card's
    /// top-left corner is square under the first tab.
    fn note(id: u64, kind: backspace_core::platform::events::Kind, source: &str, title: &str, read: bool) -> backspace_core::platform::notify::Note {
        let event = backspace_core::platform::events::Event { app: "code".into(), source: source.into(), kind, title: title.into(), body: "body".into(), at: 0 };
        backspace_core::platform::notify::Note { id, event, read, alert: true }
    }

    /// Inbox board, direction A: a bell beside Search with the unread count; it opens the notes,
    /// newest first; a note opens what it is about and is read; Mark all read clears the count.
    #[gpui_kit::test]
    fn the_bell_counts_unread_notes_and_opens_them(cx: &mut TestAppContext) {
        use backspace_core::platform::events::Kind;
        cx.update(gpui_kit::init);
        let mut c = Cauldron::new();
        c.fam_views = super::familiar::fixtures();
        c.fam_fixture = true;
        c.inbox = vec![note(2, Kind::Delivered, "fx-2", "Add CSV export delivered", false), note(1, Kind::NeedsYou, "fx-1", "Ship onboarding v2 needs you", false), note(0, Kind::Failed, "x", "Old one", true)];
        let handle = open(cx, c);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("inbox-btn").label(), Some("Inbox, 2 unread"));
            let (bell, search) = (rect(window, "inbox-btn"), rect(window, "search-btn"));
            let side = if platform() == layout::Platform::Windows { -36.0 } else { 36.0 };
            assert_eq!((bell.y, search.x - bell.x), (search.y, side), "beside Search, as Search is beside the sidebar button");
            assert!(window.try_find("inbox").is_none(), "closed until clicked");
            window.click("inbox-btn", cx);
            window.render_frame(cx);
            assert_eq!(window.find(("inbox-note", 0usize)).label(), Some("Unread: Add CSV export delivered"), "newest first");
            assert_eq!(window.find(("inbox-note", 2usize)).label(), Some("Old one"));
            window.click(("inbox-note", 1usize), cx);
            window.render_frame(cx);
            assert!(window.try_find("inbox").is_none(), "a note closes the inbox");
            assert!(window.try_find("familiar-ask").is_some(), "and opens the familiar it is about");
            assert_eq!(window.find("inbox-btn").label(), Some("Inbox, 1 unread"));
            window.click("inbox-btn", cx);
            window.render_frame(cx);
            window.click("inbox-read-all", cx);
            window.render_frame(cx);
            assert_eq!(window.find("inbox-btn").label(), Some("Inbox"));
        })
        .unwrap();
    }

    /// A note that alerts shows as a toast at the window's bottom right; one that came in quiet
    /// hours does not.
    #[gpui_kit::test]
    fn an_alerting_note_shows_as_a_toast(cx: &mut TestAppContext) {
        use backspace_core::platform::events::Kind;
        cx.update(gpui_kit::init);
        let handle = open(cx, Cauldron::new());
        handle
            .update(cx, |c, _, cx| {
                let mut quiet = note(1, Kind::Delivered, "f", "Quiet one", false);
                quiet.alert = false;
                c.take_note(quiet, cx);
            })
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("inbox-toast").is_none(), "quiet hours: no toast");
            assert_eq!(window.find("inbox-btn").label(), Some("Inbox, 1 unread"), "but the note is in");
        })
        .unwrap();
        handle.update(cx, |c, _, cx| c.take_note(note(2, Kind::NeedsYou, "f", "Loud one", false), cx)).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("inbox-toast").label(), Some("Loud one"));
            let (t, vp) = (rect(window, "inbox-toast"), window.viewport_size());
            let (w, h) = (f32::from(vp.width), f32::from(vp.height));
            assert!(w - t.right() <= 24.0 && h - t.bottom() <= 24.0 && t.right() <= w && t.bottom() <= h, "bottom right, inside the window");
        })
        .unwrap();
    }

    /// D2 (Charles, 17 Nov): a delivered familiar's stream ends on the delivery card, its
    /// Highlights say where it shipped, and the foot names all five stages; merge is never offered.
    #[gpui_kit::test]
    fn a_delivered_familiar_ends_on_its_delivery_card(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut c = Cauldron::new();
        c.fam_views = super::familiar::fixtures();
        c.fam_fixture = true;
        c.open_familiar(1);
        let handle = open(cx, c);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("familiar-delivery").label().unwrap().contains("pull/52"));
            assert!(window.try_find(("delivery-criterion", 1usize)).is_some(), "each acceptance criterion on the card");
            assert!(window.try_find("familiar-ask").is_none(), "nothing asked of you once delivered");
            assert!(window.try_find("open-pr").is_some() && window.try_find("merge").is_none(), "open the PR, never merge it");
            assert_eq!(window.find("fam-stages").label(), Some("Stage: done"));
            assert_eq!(window.find(("familiar-dot", 1usize)).label(), Some("Done"));
        })
        .unwrap();
    }

    /// S1: with Familiar on, the New stream box shows where it ships, and Start without a folder
    /// says a familiar needs one instead of opening a stream.
    #[gpui_kit::test]
    fn the_new_stream_box_starts_a_familiar_only_in_a_folder(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, Cauldron::new());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("nt-ship").is_none() && window.try_find("nt-familiar").is_none(), "no Familiar chip on the box");
        })
        .unwrap();
        handle
            .update(cx, |c, window, cx| {
                c.start.folder = None;
                c.start.familiar = true;
                if let Some((st, _)) = &c.start.input {
                    st.update(cx, |s, cx| s.set_value("Add CSV export", window, cx));
                }
                c.start.route = Some(backspace_core::chat::Route { kind: backspace_core::chat::RouteKind::Cli, provider: "claude".into(), model: None });
                c.start_thread(window, cx);
                assert!(c.start.error.as_deref().is_some_and(|e| e.contains("folder")), "{:?}", c.start.error);
                assert!(c.fam_views.is_empty() && c.open.is_none());
            })
            .unwrap();
    }

    #[gpui_kit::test]
    fn a_familiars_tabs_sit_on_its_stream_card(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut c = Cauldron::new();
        c.fam_views = super::familiar::fixtures();
        c.fam_fixture = true;
        c.open_familiar(0);
        let handle = open(cx, c);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let card = rect(window, "familiar-stream");
            let strip = rect(window, "fam-strip");
            let hl = rect(window, "familiar-highlights");
            let hl_tabs = rect(window, "fam-hl-tabs");
            assert_eq!(strip.x, card.x, "the strip starts at the card's left");
            assert!(strip.right() <= card.right(), "and ends inside its right");
            assert!((strip.y + strip.h - card.y).abs() < 0.5, "the tabs sit on the card's top edge");
            assert!(strip.right() <= hl.x && hl_tabs.x >= card.right(), "nothing over Highlights");
            assert_eq!(hl_tabs.y, strip.y, "Highlights' tabs on the same line");
            assert!(window.try_find("familiar-head").is_none(), "no title row in the card");
            assert!(window.try_find("card-title").is_none() && window.try_find(("tab", 0usize)).is_some());
            let n = 1 + (1..10usize).take_while(|&i| window.try_find(("tab", i)).is_some()).count();
            for i in 0..n {
                let t = rect(window, ("tab", i));
                assert!(t.x >= card.x && t.right() <= card.right(), "tab {i} within the card");
            }
            let joined = rect(window, "fam-joined");
            let first = rect(window, ("tab", 0usize));
            assert_eq!((joined.x, joined.w, first.x), (card.x, first.w, card.x), "the joined shape is the first tab, at the card's left");
            window.click(("tab", 3usize), cx);
            window.render_frame(cx);
            let to = rect(window, ("tab", 3usize));
            let j = rect(window, "fam-joined");
            assert!(j.x <= to.x + 0.5 && j.x >= card.x, "the joined shape heads for the opened tab from where it was");
        })
        .unwrap();
    }

    /// No agent figures in threads (Charles, 13 Nov): they belong to the Agents page. A tab is its
    /// label after the 8px padding, with agents on the thread or not.
    #[gpui_kit::test]
    fn only_agent_tabs_carry_the_agent_figure(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_agents(2, &[]));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("tab-fig").is_none(), "the pinned stream tab has no figure");
            for i in 1..=2usize {
                assert!(window.try_find(("tab-fig", i)).is_some(), "agent tab {i} draws the agent's figure");
            }
        })
        .unwrap();
        let handle = open(cx, Cauldron { coordinator: Some("Plain thread".into()), ..Cauldron::new() });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("tab", 0usize)).label(), Some("Plain thread"));
            assert!(window.try_find("tab-fig").is_none(), "a normal thread's tab has no figure");
        })
        .unwrap();
    }

    /// With no usage in accounts.json the user row says "No data yet" and no badge is drawn; with
    /// usage, the row says what is left of the first subscription and each one gets a badge under
    /// the composer box, the low ones (under 20%) in the accent. The version sits beside you.
    #[gpui_kit::test]
    fn usage_badges_and_the_user_row_come_from_accounts(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("usage-left").label(), Some("No data yet"));
            assert!(window.try_find(("usage-badge", 0usize)).is_none(), "no usage, no badge");
            assert_eq!(window.find("version").label(), Some(env!("CARGO_PKG_VERSION")));
        })
        .unwrap();
        use backspace_core::accounts::{Account, Accounts, Usage};
        let path = std::env::temp_dir().join(format!("cauldron-accounts-usage-{}.json", std::process::id()));
        let acct = |id: &str, name: &str, used: u64| Account { id: id.into(), name: name.into(), provider: "claude".into(), enabled: true, usage: Some(Usage { used: Some(used), limit: Some(100), ..Default::default() }), ..Default::default() };
        let a = Accounts { list: vec![acct("a", "Claude", 38), acct("b", "Codex", 86)], ..Default::default() };
        a.save(&path).unwrap();
        let handle = open(cx, Cauldron { accounts_file: Some(path.clone()), ..with_threads() });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("usage-left").label(), Some("62% left"));
            assert_eq!(window.find(("usage-badge", 0usize)).label(), Some("Claude 62%"));
            assert_eq!(window.find(("usage-badge", 1usize)).label(), Some("Codex 14%"));
            let (b, nt) = (rect(window, ("usage-badge", 0usize)), rect(window, "nt-box"));
            assert!(b.y >= nt.bottom(), "under the box");
        })
        .unwrap();
        let _ = std::fs::remove_file(path);
    }

    /// The top row's search button opens the search row; the folded sidebar's hover indent (Tight
    /// tiles rule 6) appears when the pointer reaches the window's left edge, over the card with no
    /// reflow: search, New stream, the six apps, Settings. Leaving it hides it.
    #[gpui_kit::test]
    fn search_button_and_the_hover_indent(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("search").is_none(), "search waits behind its button");
            let (b, sb) = (rect(window, "search-btn"), rect(window, "sbtn"));
            // Windows: right of the button in the top bar. Elsewhere: left of it, as the board's 160 and 196.
            let side = if platform() == layout::Platform::Windows { -36.0 } else { 36.0 };
            assert_eq!((b.y, sb.x - b.x), (sb.y, side), "beside the sidebar button");
            window.click("search-btn", cx);
            window.render_frame(cx);
            assert!(window.try_find("search").is_some(), "the button opens the search row");
        })
        .unwrap();
        let handle = open(cx, Cauldron { sidebar_open: false, ..with_threads() });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let main = rect(window, "main");
            // The test pointer starts at the window's corner, inside the edge zone: move it away first.
            window.hover("card", cx);
            window.render_frame(cx);
            assert!(window.try_find("indent").is_none(), "hidden until the pointer comes");
            window.hover("indent-zone", cx);
            window.render_frame(cx);
            let band = rect(window, "indent");
            assert_eq!((band.x, band.w), (0.0, if platform() == layout::Platform::Mac { 72.0 } else { 48.0 }), "72 on macOS, 48 on Windows");
            assert_eq!(rect(window, "main"), main, "no reflow: the card stays where it was");
            for i in 0..6usize {
                assert!(window.try_find(("indent-fn", i)).is_some(), "app {i} in the band");
            }
            for id in ["indent-search", "indent-new", "indent-settings"] {
                let r = rect(window, id);
                assert!(r.x >= band.x && r.right() <= band.right(), "{id} inside the band");
            }
            assert_eq!(rect(window, "indent-edge").x, band.right(), "the card's edge beside it");
            window.hover("card", cx);
            window.render_frame(cx);
            assert!(window.try_find("indent").is_none(), "leaving the band hides it");
        })
        .unwrap();
    }

    /// Agent tabs fit their names within an equal share, 72 to 200px; the arrows appear exactly when 72px
    /// tabs no longer fit; the coordinator is pinned at the left, never scrolls and has no close.
    #[gpui_kit::test]
    fn agent_tabs_clamp_then_scroll_with_the_coordinator_pinned(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        for n in [1usize, 3, 5, 6, 8, 14] {
            let handle = open(cx, with_agents(n, &[]));
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                let row = rect(window, "tabs");
                let anchor = rect(window, ("tab", 0usize));
                assert_eq!(anchor.x, row.x, "{n}: coordinator pinned at the left");
                assert!(anchor.w <= 190.0, "{n}: coordinator at most 190: {anchor:?}");
                assert_eq!(window.find(("tab", 0usize)).label(), Some("Cauldron · coordinator"));
                assert!(window.try_find(("tab-close", 0usize)).is_none(), "{n}: coordinator has no close");
                let room = row.w - anchor.w - (super::TAB_SEP + 28.0);
                let need = n as f32 * (72.0 + super::TAB_SEP);
                let arrows = window.try_find("tab-prev").is_some();
                assert_eq!(arrows, need > room, "{n}: arrows iff 72px tabs do not fit (need {need}, room {room})");
                let seen = visible(window, n);
                if arrows {
                    assert!(seen.len() < n, "{n}: scrolling hides some tabs");
                    for id in ["tab-prev", "tab-next"] {
                        let r = rect(window, id);
                        assert_eq!((r.w, r.h), (24.0, 28.0), "{id}: as tall as the tabs");
                        assert!(r.x < rect(window, "tab-add").x, "{id} left of +");
                    }
                } else {
                    assert_eq!(seen.len(), n, "{n}: all tabs in view");
                }
                for &i in &seen {
                    let t = rect(window, ("tab", i));
                    assert!(t.w >= 71.5 && t.w <= 210.5, "{n}: tab {i} width {t:?}");
                    if arrows {
                        assert!((t.w - 72.0).abs() < 0.5, "{n}: scrolling tabs are 72px");
                    }
                    assert!(t.x > anchor.right() && t.right() <= row.right() + 0.5, "{n}: tab {i} in the row after the coordinator");
                }
                let add = rect(window, "tab-add");
                assert!(add.right() <= row.right() + 0.5, "{n}: + in the row");
                if arrows {
                    for _ in 0..20 {
                        window.click("tab-next", cx);
                    }
                    assert_eq!(rect(window, ("tab", 0usize)).x, row.x, "{n}: coordinator never scrolls");
                    assert!(visible(window, n).contains(&n), "{n}: > reaches the last tab");
                    window.click("tab-prev", cx);
                    assert!(!visible(window, n).contains(&n), "{n}: < scrolls back by one");
                }
            })
            .unwrap();
        }
    }

    /// The open tab is scrolled into view whenever it changes: keyboard, wheel and close.
    #[gpui_kit::test]
    fn the_open_tab_is_kept_in_view(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let n = 14;
        let handle = open(cx, with_agents(n, &[]));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("tab", 1usize), cx);
            window.press("end", cx);
            assert!(visible(window, n).contains(&n), "End opens and shows the last tab");
            assert!(on(window, n), "and it is the open one");
            for _ in 0..20 {
                window.click("tab-prev", cx);
            }
            assert!(!visible(window, n).contains(&n), "the arrows may scroll the open tab away");
            window.press("left", cx);
            assert!(visible(window, n).contains(&(n - 1)), "Left opens the tab before and shows it");
            window.press("home", cx);
            assert!(on(window, 0), "Home opens the coordinator");
            window.press("delete", cx);
            assert!(window.try_find(("tab", 0usize)).is_some(), "Delete never closes the coordinator");
            window.press("right", cx);
            window.press("delete", cx);
            assert_eq!(window.find(("tab", 1usize)).label(), Some("Builder"), "Delete closes the open agent tab");
            window.press("end", cx);
        })
        .unwrap();
        cx.simulate_window_resize(handle.into(), size(px(W - 200.0), px(H)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(visible(window, n - 1).contains(&(n - 1)), "a resize keeps the open tab in view");
        })
        .unwrap();
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.press("home", cx);
            let before = visible(window, n - 1)[0];
            window.scroll("tabs", gpui_kit::ScrollDelta::Lines(gpui_kit::point(0.0, 3.0)), cx);
            assert!(visible(window, n - 1)[0] < before, "the wheel scrolls the row back");
            window.scroll("tabs", gpui_kit::ScrollDelta::Lines(gpui_kit::point(0.0, -2.0)), cx);
            assert!(visible(window, n - 1)[0] > before - 3, "and forward");
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
            assert_eq!(window.find(("tab", 3usize)).label(), Some("Tester"));
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
        Cauldron { threads: Threads::Ready(v), ..Cauldron::new() }
    }

    fn row_labels(window: &Window) -> Vec<String> {
        (0usize..).map_while(|i| window.try_find(("thread", i)).map(|e| e.label().unwrap_or_default().to_string())).collect()
    }

    /// The sidebar lists core's threads as Unread, Running, Failed, Done, each with its count,
    /// top to bottom, inside the sidebar.
    #[gpui_kit::test]
    fn sidebar_lists_threads_in_groups_with_counts(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let heads: Vec<String> = (0..3usize).map(|g| window.find(("group", g)).label().unwrap().to_string()).collect();
            assert_eq!(heads, ["Needs you, 2", "Running, 1", "Done, 2"]);
            for g in 1..3usize {
                assert!(rect(window, ("group", g - 1)).y < rect(window, ("group", g)).y, "group {g} below the one before");
            }
            assert_eq!(row_labels(window), ["Koi palette", "M2 eval task texts", "Rampart mutation score", "Pinned notes", "Agent delegation"]);
            let side = rect(window, "sidebar");
            for i in 0..5usize {
                let r = rect(window, ("thread", i));
                assert!(r.x >= side.x && r.right() <= side.right(), "row {i} inside the sidebar");
            }
            assert!(rect(window, ("thread", 0usize)).y > rect(window, "new-thread").bottom(), "rows under New stream");
        })
        .unwrap();
    }

    /// Charles's 20 Nov Windows test: the version, your name and the New button did nothing, and
    /// New stream was the same in every app. The version opens the update card, your name opens
    /// Settings, and the New button follows the app (New familiar under Agents, none where an app
    /// has nothing to make yet).
    #[gpui_kit::test]
    fn the_version_your_name_and_the_new_button_all_act(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("update-card").is_none(), "closed until clicked");
            window.click("version", cx);
            window.render_frame(cx);
            assert_eq!(window.find("update-card").label(), Some("Checking for updates…"));
            window.click("user-row", cx);
            window.render_frame(cx);
            assert_eq!(window.find(("settings-page", 0usize)).label().map(|l| l.starts_with("Agents")), Some(true), "your name opens Settings");
            window.click("settings", cx);
            window.render_frame(cx);
            for (i, id) in [(0usize, Some("new-thread")), (1, Some("new-folder")), (2, None), (3, Some("new-familiar")), (4, Some("new-sub")), (5, None)] {
                window.click(("fn", i), cx);
                window.render_frame(cx);
                for other in ["new-thread", "new-folder", "new-familiar", "new-sub"] {
                    assert_eq!(window.try_find(other).is_some(), Some(other) == id, "app {i}: {other}");
                }
            }
            window.click(("fn", 3usize), cx);
            window.render_frame(cx);
            window.click("new-familiar", cx);
            window.render_frame(cx);
            assert_eq!(window.find("nt-heading").label(), Some("What should a familiar take on?"));
            window.click(("fn", 0usize), cx);
            window.render_frame(cx);
            window.click("new-thread", cx);
            window.render_frame(cx);
            assert_eq!(window.find("nt-heading").label(), Some("What should Cauldron work on?"), "New stream turns Familiar off again");
        })
        .unwrap();
    }

    /// The apps (20 Nov): a row above New stream, each its own button with a gap between, not one
    /// tray with a sliding key.
    #[gpui_kit::test]
    fn the_apps_are_separate_buttons_above_new_stream(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let (apps, new) = (rect(window, "apps-tray"), rect(window, "new-thread"));
            assert!(apps.bottom() <= new.y, "the apps sit above New stream");
            for i in 0..super::FUNCTIONS.len() {
                let b = rect(window, ("fn", i));
                assert!(b.x >= apps.x && b.right() <= apps.right() + 0.5, "app {i} inside the row");
                if i > 0 {
                    assert!(b.x - rect(window, ("fn", i - 1)).right() >= 3.5, "app {i} has its own space");
                }
            }
            assert_eq!(window.find(("fn", 0usize)).selected(), Some(true), "Streams is the open one");
        })
        .unwrap();
    }

    /// The rows are the Tight tiles board's `.th`: 30 high, the agent's 22px figure 8px in (its
    /// mood from the stream's state), the title, the repo as a tag, and a 6px ping on an unread row
    /// only. Every row shows a figure (Charles, 20 Nov, from the round 3 board; it had been
    /// dropped on 13 Nov). Group heads are the board's 34px `.grp`; New stream is a 30px row with
    /// its shortcut.
    #[gpui_kit::test]
    fn thread_rows_are_the_boards(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(rect(window, "new-thread").h, 30.0);
            assert_eq!(window.find("new-thread").label(), Some("New stream"));
            for g in 0..3usize {
                assert_eq!(rect(window, ("group", g)).h, 34.0, "group {g}");
            }
            // Row 2 (Rampart mutation score) has an agent; it still shows a dot.
            for (i, state) in [(0usize, "Needs you"), (1, "Failed"), (2, "Working"), (3, "Done"), (4, "Done")] {
                let r = rect(window, ("thread", i));
                assert_eq!(r.h, 30.0, "row {i}");
                let a = rect(window, ("thread-av", i));
                assert_eq!((a.w, a.h), (7.0, 22.0), "row {i}'s slot");
                assert!((a.y + a.h / 2.0 - (r.y + r.h / 2.0)).abs() <= 1.0, "row {i}'s slot is centred");
                assert_eq!(a.x - r.x, 8.0, "row {i}'s dot is 8px in, as the board's");
                assert_eq!(window.find(("thread-av", i)).label(), Some(state), "row {i}");
                assert!(window.try_find(("thread-fig", i)).is_none(), "row {i}: no figure, streams do not have agents (Charles, 20 Nov)");
                assert!(window.try_find(("thread-dot", i)).is_some(), "row {i}: a state dot");
                assert_eq!(window.find(("thread-meta", i)).label(), Some("no repo"), "row {i}");
            }
            let (row, ping) = (rect(window, ("thread", 0usize)), rect(window, ("ping", 0usize)));
            assert_eq!((ping.w, ping.h), (6.0, 6.0));
            assert!(row.right() - ping.right() <= 10.0, "the ping is at the row's right");
            for i in 1..5usize {
                assert!(window.try_find(("ping", i)).is_none(), "row {i} is read");
            }
        })
        .unwrap();
    }

    /// Loading, empty, no search match and error each draw their own state, never a blank list.
    #[gpui_kit::test]
    fn sidebar_draws_loading_empty_no_match_and_error(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let states = [
            (Cauldron { threads: Threads::Loading, ..Cauldron::new() }, "threads-loading", "Loading threads"),
            (Cauldron { threads: Threads::Ready(Vec::new()), ..Cauldron::new() }, "threads-empty", "No threads yet"),
            (Cauldron { query: "zzz".into(), ..with_threads() }, "threads-no-match", "No threads match zzz"),
            (Cauldron { threads: Threads::Error("disk full".into()), ..Cauldron::new() }, "threads-error", "Threads could not load: disk full"),
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
            window.click("search-btn", cx);
            assert_eq!(window.find("search").label(), Some("Search streams"));
            window.click(("thread", 2usize), cx);
            window.press("down", cx);
            window.press("down", cx);
        })
        .unwrap();
        let sel = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| c.selected.clone()).unwrap();
        assert_eq!(sel(cx).as_deref(), Some("s"), "click the Running row, then two down: Done's first, then its second");
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
            assert_eq!(window.find(("group", 0usize)).label(), Some("Needs you, 1"));
            assert!(window.try_find(("group", 1usize)).is_none(), "groups with no match are left out");
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
