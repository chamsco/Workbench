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
mod panes;
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
    let s = layout::layout(1240.0, 702.0, ShellState { sidebar_open: true, drawer_open: false, split_open: false }, Platform::Mac);
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

const FUNCTIONS: [(&str, &str); 7] = [
    ("inbox", "Threads"),
    ("folder", "Projects"),
    ("calendar", "Schedule"),
    ("bot", "Agents"),
    ("gauge", "Usage"),
    ("brain", "Memory"),
    ("layout-grid", "Apps"),
];

const NEW_TAB: &str = "New thread";

const TOP_PANELS: [Panel; 4] = [Panel::Highlights, Panel::Usage, Panel::Koi, Panel::Review];
/// The drawer's strip: the rail's panels in order, Patches last.
const ALL_PANELS: [Panel; 5] = [Panel::Highlights, Panel::Usage, Panel::Koi, Panel::Review, Panel::Patches];

/// The sidebar's threads from core: still opening, opened, or core failed to open.
pub(crate) enum Threads {
    Loading,
    Ready(Vec<ThreadInfo>),
    Error(SharedString),
}

pub(crate) struct Cauldron {
    sidebar_open: bool,
    drawer: Option<Panel>,
    /// The thread card's panes (direction B): the shape and where its two lines sit.
    panes: Split,
    /// The layout button's morph: the shape it drew before, and since when.
    layout_since: Option<(Shape, std::time::Instant)>,
    /// A line held down: vertical or not, and the panes' area when it was grabbed.
    pane_drag: Option<(bool, Rect)>,
    /// The thread's coordinator, pinned as the first tab; None on the New thread screen.
    coordinator: Option<SharedString>,
    /// Agents with an open 1:1 tab, in tab order, and the thread's other agents (the + menu).
    agents: Vec<AgentTab>,
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
            if std::env::var("CAULDRON_DRAWER").as_deref() == Ok("folded") {
                c.drawer_last = c.drawer.take();
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
        Cauldron { motion: ShellMotion::default(), section: 0, section_from: 0.0, section_since: None, swap: None, action_since: None, busy_shown: false, open_since: None, bubble_since: None, mine_for: None, mine_n_shown: 0, drawer_last: None, sidebar_open: true, drawer: Some(Panel::Highlights), panes: Split::default(), layout_since: None, pane_drag: None, coordinator: None, agents: Vec::new(), idle: Vec::new(), active: 0, reveal: false, tab_window: 0, wheel: 0.0, tab_menu: false, tab_slide: Default::default(), tab_focus: None, tip: None, tip_shown: false, tip_warm_until: None, tip_wait: None, tab_first: 0, threads: Threads::Ready(Vec::new()), selected: None, query: String::new(), search: None, list_focus: None, core: None, watch: None, open: None, thread: None, steps: Default::default(), composer: None, send_error: None, start: Default::default() }
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
                let groups = threads::grouped(v, &self.query);
                if groups.is_empty() {
                    let q = self.query.trim().to_string();
                    return list.child(note("threads-no-match", format!("No threads match {q}"), None, format!("No threads match \u{201c}{q}\u{201d}"), None));
                }
                // Core has no Needs you yet, so the group is its head alone, without a count
                // (Charles chose empty states, 12 Nov).
                if self.query.trim().is_empty() {
                    list = list.child(
                        div()
                            .id("group-needs")
                            .flex()
                            .items_center()
                            .h(px(36.))
                            .flex_none()
                            .px(px(8.))
                            .pt(px(14.))
                            .pb(px(4.))
                            .text_size(px(12.))
                            .line_height(px(18.))
                            .text_color(fg3)
                            .aria_label("Needs you")
                            .child("Needs you")
                            .test_support(),
                    );
                }
                let mut i = 0usize;
                for (gi, (g, ts)) in groups.into_iter().enumerate() {
                    let n = ts.len();
                    list = list.child(
                        div()
                            .id(("group", gi))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .h(px(36.))
                            .flex_none()
                            .px(px(8.))
                            .pt(px(14.))
                            .pb(px(4.))
                            .text_size(px(12.))
                            .line_height(px(18.))
                            .text_color(if g == Group::Failed { c(pal.bad) } else { fg3 })
                            .aria_label(format!("{}, {n}", g.label()))
                            .child(g.label())
                            .child(div().ml_auto().font_features(tnum()).child(n.to_string()))
                            .test_support(),
                    );
                    for t in ts {
                        let on = self.selected.as_deref() == Some(t.id.as_str());
                        let id = t.id.clone();
                        // A state dot, agent or not: figures belong to the Agents page (Charles, 13 Nov).
                        let (state, dot) = match g {
                            Group::Running => ("Working", c(pal.accent)),
                            Group::Failed => ("Failed", c(pal.bad)),
                            Group::Unread => ("Unread", fg3),
                            Group::Done => ("Done", fg3),
                        };
                        let slot = div()
                            .id(("thread-av", i))
                            .w(px(18.))
                            .h(px(22.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .aria_label(state)
                            .child(div().id(("thread-dot", i)).size(px(6.)).rounded_full().bg(dot).test_support());
                        list = list.child(
                            div()
                                .id(("thread", i))
                                .h(px(30.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .pl(px(11.))
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
                                .child({
                                    let m = threads::meta(t);
                                    div().id(("thread-meta", i)).flex_none().max_w(px(54.)).truncate().text_size(px(11.)).text_color(fg3).aria_label(m.clone()).child(m).test_support()
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
                let st = (self.wheel / (TAB_MIN + TAB_GAP)).trunc();
                self.wheel -= st * (TAB_MIN + TAB_GAP);
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
    fn select_section(&mut self, i: usize, cx: &mut Context<Self>) {
        if i != self.section {
            let now = std::time::Instant::now();
            self.section_from = crate::motion::ease_between(self.section_from, self.section as f32, self.section_since, now, cauldron_chrome::motion::time::INDICATOR);
            self.section = i;
            self.section_since = Some(now);
        }
        cx.notify();
    }

    fn toggle_panel(&mut self, p: Panel) {
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
        .bg(if fill { sel } else { gpui_kit::transparent_black() })
        .hover(move |d| d.bg(if fill { sel } else { hover }))
        .child(glyph(name, 16., if on { c(pal.fg) } else { c(pal.fg2) }))
        .aria_label(label)
}

/// The left edge of function slot `pos` (fractional while the indicator slides) in the sidebar's
/// apps row, `row_w` wide. The row spreads its buttons edge to edge with equal gaps (justify_between).
fn fn_slot_x(row_w: f32, pos: f32) -> f32 {
    let n = FUNCTIONS.len() as f32;
    let gap = (row_w - n * metric::SBTN) / (n - 1.0);
    pos * (metric::SBTN + gap)
}

/// The sidebar's right padding: its content ends at the board's x 240.
const SIDE_PR: f32 = 9.0;

/// Agent tabs share the room between these widths; below TAB_MIN the row scrolls.
const TAB_MIN: f32 = 72.0;
const TAB_MAX: f32 = 200.0;
/// A floating tab, on the design-system scale (Charles, 13 Nov): 4px between tabs, 8px padding,
/// 6px from the name to the close. No figure: agent figures belong to the Agents page.
const TAB_GAP: f32 = 4.0;
const TAB_PAD: f32 = 8.0;
const CLOSE_GAP: f32 = 6.0;
/// The + after the tabs, as tall as a tab.
const ADD_W: f32 = metric::TAB_H;
/// The new thread's bar under the card: usage and the stages.
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
        let dark = forced_dark().unwrap_or_else(|| !matches!(window.appearance(), WindowAppearance::Light | WindowAppearance::VibrantLight));
        let pal: &'static Palette = if dark { &tokens::DARK } else { &tokens::LIGHT };
        let vs = window.viewport_size();
        let (w, h) = (f32::from(vs.width), f32::from(vs.height));
        let plat = platform();
        let want = ShellState { sidebar_open: self.sidebar_open, drawer_open: self.drawer.is_some(), split_open: false };
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
        let card = c(pal.card);
        let ring = ca(pal.fg, if dark { 0.08 } else { 0.09 });
        let fg = c(pal.fg);
        let fg2 = c(pal.fg2);
        let fg3 = c(pal.fg3);
        let accent = c(pal.accent);

        self.ensure_composer(window, cx);
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

        // Window drag strip, under every control.
        root = root.child(
            abs(Rect::new(1.0, 1.0, w - 2.0, metric::BAR_H)).window_control_area(WindowControlArea::Drag),
        );

        // ---- rail (only while the sidebar is hidden)
        if let Some(rp) = s.rail {
            let mut col = pane("rail", rp.rect).opacity(rp.alpha).flex().flex_col().items_center().gap(px(4.)).pt(px(4.));
            for (i, (name, label)) in FUNCTIONS.iter().enumerate() {
                let b = icon_button(("rail", i), name, label, pal, i == self.section).on_click(cx.listener(move |this, _, _, cx| this.select_section(i, cx)));
                col = col.child(self.tipped(("rail", i), b, *label, Side::Right, pal, cx).test_support());
            }
            root = root.child(col.test_support());
        }

        // ---- sidebar with the function row, on the window
        if let Some(sp) = s.sidebar {
            let mut fnrow = div().relative().flex().justify_between().pt(px(4.)).pb(px(16.));
            // The indicator sits under the buttons and slides to the selected one (INDICATOR ms).
            let pos = crate::motion::ease_between(self.section_from, self.section as f32, self.section_since, now, cauldron_chrome::motion::time::INDICATOR);
            fnrow = fnrow.child(
                div()
                    .absolute()
                    .left(px(fn_slot_x(sp.rect.w - SIDE_PR, pos)))
                    .top(px(4.))
                    .size(px(metric::SBTN))
                    .rounded(px(radius::CONTROL))
                    .bg(ca(pal.fg, 0.09)),
            );
            for (i, (name, label)) in FUNCTIONS.iter().enumerate() {
                let b = icon_button_in(("fn", i), name, label, pal, i == self.section, false).on_click(cx.listener(move |this, _, _, cx| this.select_section(i, cx)));
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
                let st = cx.new(|cx| InputState::new(window, cx).placeholder("Search threads"));
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
                .child("New thread")
                .on_click(cx.listener(|this, _, window, cx| this.new_thread(window, cx)))
                .test_support();
            // The board's `.side`: on the window, no card; the apps row, New thread, Search, the list.
            // ponytail: the layout keeps the card era's 12px margin, so the content starts at x 13,
            // the board's at 10; the right edge is the board's 240 (9px in from the pane's 249).
            let card_el = pane("sidebar", sp.rect)
                .opacity(sp.alpha)
                .flex()
                .flex_col()
                .pr(px(SIDE_PR))
                .overflow_hidden()
                .child(fnrow)
                .child(new_thread)
                .child(
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
                            .aria_label("Search threads")
                            .child(glyph("search", 15., fg2))
                            .child(div().flex_1().min_w_0().child(Input::new(&search).appearance(false).text_size(px(13.)).p_0()))
                            .test_support(),
                    ),
                )
                .child(list)
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
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .line_height(px(15.))
                                .child(div().truncate().text_size(px(12.5)).child(user))
                                // Core has no usage per subscription yet: an empty state, no number.
                                .child(div().id("usage-left").text_size(px(11.)).text_color(fg3).aria_label("No data yet").child("No data yet").test_support()),
                        )
                        .child(self.tipped("settings", icon_button("settings", "settings", "Settings", pal, false), "Settings", Side::Right, pal, cx).test_support()),
                );
            root = root.child(card_el.test_support());
        }

        // ---- the thread's tabs, floating in the top bar (the board's `.ftab`). Only an open thread
        // has them: the coordinator pinned at the left, then its agents, each sized to its name
        // within an equal share of the room (TAB_MIN to TAB_MAX); when TAB_MIN no longer fits, arrows appear beside +
        // and the row shows a window of 72px tabs. No figures (Charles, 13 Nov).
        let mut tab_row = None;
        if let Some(anchor_label) = self.coordinator.clone() {
            let focus = self.tab_focus.get_or_insert_with(|| cx.focus_handle()).clone();
            let label_w = {
                let run = TextRun { len: anchor_label.len(), font: Font { weight: FontWeight::MEDIUM, features: sans_features(), ..font(SANS) }, color: fg, background_color: None, underline: None, strikethrough: None };
                f32::from(window.text_system().shape_line(anchor_label.clone(), px(12.), &[run], None).width)
            };
            let anchor_w = (TAB_PAD + label_w + TAB_PAD).ceil().min(ANCHOR_MAX);
            let n = self.agents.len();
            let room = s.tabs.w - anchor_w - (TAB_GAP + ADD_W);
            let scrolls = n as f32 * (TAB_MIN + TAB_GAP) + TAB_GAP > room;
            let (k, tab_w) = if scrolls {
                (((room - 54.0) / (TAB_MIN + TAB_GAP)).floor().max(1.0) as usize, Some(TAB_MIN))
            } else if n > 0 {
                let per = ((room - TAB_GAP * (n as f32 + 1.0)) / n as f32).floor();
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
                .gap(px(TAB_GAP))
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
            // `selected` fill with no ring or shadow (tabs take none); the rest are fg3 on the frame.
            let ftab = |id: (&'static str, usize), on: bool, cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(CLOSE_GAP))
                    .h(px(metric::TAB_H))
                    .rounded(px(radius::INNER))
                    .when_live(on, |d| d.bg(c(pal.selected)))
                    .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                    .occlude()
                    .cursor_pointer()
                    .aria_selected(on)
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if on { fg } else { fg3 })
                    .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| this.wheel_tabs(ev, cx)))
            };

            // The pinned tab.
            let on = self.active == 0;
            let anchor = ftab(("tab", 0), on, cx)
                .flex_none()
                .w(px(anchor_w))
                .px(px(TAB_PAD))
                .aria_label(anchor_label.clone())
                .child(div().min_w_0().truncate().child(anchor_label.clone()))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.active = 0;
                    this.focus_tabs(window, cx);
                    cx.notify();
                }));
            row = row.child(anchor.test_support());

            for (j, agent) in self.agents.iter().enumerate().skip(first).take(k) {
                let i = j + 1;
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
                    .when_live(!on, |d| d.absolute().right(px(4.)).top(px((metric::TAB_H - 18.0) / 2.0)).bg(frame).opacity(0.).group_hover(group.clone(), |s| s.opacity(1.)))
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
                tab = if scrolls { tab.flex_none().w(px(TAB_MIN)) } else { tab.flex_none().min_w(px(TAB_MIN)).max_w(px(share)) };
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
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tab_menu = !this.tab_menu;
                    this.tab_slide.reset();
                    cx.notify();
                }));
            if self.tab_menu {
                add = add.child(self.tab_menu_el(pal, cx));
            }
            let add = if self.tab_menu { add } else { self.tipped("tab-add", add, ADD_LABEL, Side::Below, pal, cx) };
            tab_row = Some(row.child(add.test_support()));
        }

        // The card right under the bar; its 44px head names the thread (the board's `.ch`).
        let head_title: SharedString = self.coordinator.clone().unwrap_or_else(|| NEW_TAB.into());
        let mut head = div()
            .id("card-head")
            .h(px(44.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .pl(px(13.))
            .pr(px(14.))
            .text_size(px(13.))
            .text_color(fg3);
        if let Some(p) = self.thread.as_ref().and_then(|t| t.project.as_ref()) {
            let folder = p.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(p).to_string();
            head = head.child(div().id("card-crumb").flex_none().aria_label(folder.clone()).child(format!("{folder} /")).test_support());
        }
        head = head.child(
            div()
                .id(if self.thread.is_some() { "thread-title" } else { "card-title" })
                .min_w_0()
                .truncate()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(fg)
                .aria_label(head_title.clone())
                .child(head_title)
                .test_support(),
        );
        head = head.child(div().flex_1()).child(self.layout_button(pal, now, reduce, cx).test_support());
        // A new thread keeps a 32px bar under its card, in the Shell's grey (Charles 13 Nov).
        let new_screen = self.open.is_none() && self.active == 0;
        let card_h = s.main.h - if new_screen { STAGE_H } else { 0.0 };
        // Split (direction B): under the head the card holds up to four panes on the Shell's grey;
        // a shape that no longer fits steps back, as the shell closes Split when the window narrows.
        let area = Rect::new(s.main.x, s.main.y + 44.0, s.main.w, card_h - 44.0);
        let shape = cauldron_chrome::panes::effective(area, self.panes.shape);
        let split = shape != Shape::One;
        let body = self.card_body(pal, window, cx);
        let inside = if split { self.pane_stage(pal, area, shape, body, cx).into_any_element() } else { body };
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
            .when_live(!split, |d| d.bg(c(pal.panel)).border_1().border_color(ring))
            .text_color(fg3)
            .child(head.test_support())
            .child(div().flex_1().min_h_0().child(inside));
        // The grey Shell behind the card, Split and the drawer: the drawer stands in its right
        // gutter and the new thread's bar in its bottom one (the coordinator's brief, 13 Nov).
        root = root.child(pane("shell", s.shell).rounded(px(radius::SHELL)).bg(card).test_support());
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
            // The panels as a strip on top (the v3 board's `.dtabs`): the open one selected, its
            // button folds the drawer back to the rail.
            let mut strip = div().id("drawer-strip").flex().gap(px(4.)).pb(px(8.)).border_b_1().border_color(c(pal.line));
            for (i, q) in ALL_PANELS.iter().enumerate() {
                let q = *q;
                let on = self.drawer == Some(q);
                strip = strip.child(
                    self.tipped(
                        ("dt", i),
                        div()
                            .id(("dt", i))
                            .flex_1()
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(radius::INNER))
                            .when_live(on, |d| d.bg(c(pal.selected)))
                            .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                            .cursor_pointer()
                            .aria_selected(on)
                            .aria_label(q.title())
                            .child(glyph(q.icon(), 16., if on { fg } else { fg3 })),
                        q.title(),
                        Side::Below,
                        pal,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_panel(q);
                        cx.notify();
                    }))
                    .test_support(),
                );
            }
            // Both bodies share one box under the strip, so the outgoing one cross-fades in place.
            let mut bodies = div().relative().flex_1().min_h_0().child(body(p, swap.map_or(1.0, |(_, k)| k)));
            if let Some((a, k)) = swap {
                bodies = bodies.child(body(a, 1.0 - k));
            }
            let d = pane("drawer", dp.rect).opacity(dp.alpha).flex().flex_col().child(strip.test_support()).child(bodies);
            root = root.child(d.test_support());
        }

        // ---- right rail, only while the drawer is folded
        if let Some(rp) = s.rrail {
        let mut rr = pane("rrail", rp.rect).opacity(rp.alpha).flex().flex_col().items_center().gap(px(4.)).pt(px(4.));
        for (i, p) in TOP_PANELS.iter().enumerate() {
            let p = *p;
            rr = rr.child(
                self.tipped(("rb", i), icon_button(("rb", i), p.icon(), p.title(), pal, self.drawer == Some(p)), p.title(), Side::Left, pal, cx)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_panel(p);
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        rr = rr.child(div().flex_1()).child(
            self.tipped("rb-patches", icon_button("rb-patches", Panel::Patches.icon(), Panel::Patches.title(), pal, self.drawer == Some(Panel::Patches)), Panel::Patches.title(), Side::Left, pal, cx)
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
        if let Some(row) = tab_row {
            root = root.child(row.test_support());
        }

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

        // ---- Terminal and Split, left of the caption buttons
        root = root.child(
            pane("topr", s.topr)
                .flex()
                .items_center()
                .gap(px(2.))
                .child(self.tipped("top-term", icon_button("top-term", "terminal", "Terminal", pal, false), "Terminal", Side::Below, pal, cx).occlude().test_support())
                .child(
                    self.tipped("top-split", icon_button("top-split", "columns-2", "Split", pal, self.panes.shape != Shape::One), "Split", Side::Below, pal, cx).occlude().on_click(cx.listener(|this, _, _, cx| {
                        // Split opens two side by side, or goes back to one pane.
                        this.set_shape(if this.panes.shape == Shape::One { Shape::Cols } else { Shape::One });
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
    use super::{fn_slot_x, platform, AgentTab, Cauldron, Panel, Threads};
    use cauldron_chrome::layout::metric;
    use crate::threads::sample;

    #[test]
    fn function_slots_span_the_row_with_equal_gaps() {
        // The row has no card border or padding now: the board's `.apps` runs edge to edge.
        let w = 227.0;
        assert_eq!(fn_slot_x(w, 0.0), 0.0);
        assert!((fn_slot_x(w, 6.0) + metric::SBTN - w).abs() < 1e-3, "last button ends at the row's edge");
        let step = fn_slot_x(w, 1.0) - fn_slot_x(w, 0.0);
        assert!((fn_slot_x(w, 6.0) - fn_slot_x(w, 0.0) - 6.0 * step).abs() < 1e-3, "equal steps");
    }
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

    /// Every pane the shell draws sits exactly where `cauldron_chrome::layout::solve` puts it,
    /// in all four combinations of sidebar and drawer. Split lives inside the card now (direction B),
    /// so the solver's half pane is never drawn.
    #[gpui_kit::test]
    fn panes_match_the_layout_solver_in_every_state(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        for bits in 0..4u8 {
            let state = ShellState { sidebar_open: bits & 1 != 0, drawer_open: bits & 2 != 0, split_open: false };
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
                assert_rect(window, "topr", s.topr, state);
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

    /// The open drawer carries its panels as a strip on top (the v3 board's `.dtabs`): one 28px
    /// button per panel sharing the width, the open one selected. Its button folds the drawer back to
    /// the rail; a rail button opens it again on that panel.
    #[gpui_kit::test]
    fn the_drawer_switches_panels_from_its_own_strip(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| cx.set_reduce_motion(true));
        let handle = cx.add_window(|_, _| Cauldron::new());
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let d = rect(window, "drawer");
            assert!(window.try_find("rrail").is_none(), "no rail beside the open drawer");
            for i in 0..5usize {
                let b = rect(window, ("dt", i));
                assert_eq!((b.y, b.h), (d.y, 28.0), "strip button {i} at the drawer's top");
                assert!(b.x >= d.x && b.right() <= d.right(), "strip button {i} inside the drawer");
                assert_eq!(dt_on(window, i), i == 0, "strip button {i}: Highlights is open");
            }
            assert_eq!(window.find(("dt", 4usize)).label(), Some("Patches"));
            window.click(("dt", 4usize), cx);
            assert!(dt_on(window, 4usize) && !dt_on(window, 0usize), "a strip button switches the panel");
            window.click(("dt", 4usize), cx);
        })
        .unwrap();
        // The shell moves on the wall clock, so start each check from rest.
        let rest = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| c.motion = Default::default()).unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("drawer").is_none() && window.try_find("rrail").is_some(), "the open panel's button folds the drawer to the rail");
            window.click(("rb", 1usize), cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("drawer").is_some() && dt_on(window, 1usize), "a rail button opens the drawer on its panel");
            assert!(window.try_find("rrail").is_none(), "and the rail gives way");
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
            let named: [(ElementId, &str); 7] = [
                (("fn", 4usize).into(), "Usage"),
                (("rb", 0usize).into(), "Highlights"),
                ("rb-patches".into(), "Patches"),
                ("sbtn".into(), "Hide sidebar"),
                ("top-term".into(), "Terminal"),
                ("top-split".into(), "Split"),
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
            window.hover(("fn", 4usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find("tooltip").label(), Some("Usage"));
            let (tip, btn) = (rect(window, "tooltip"), rect(window, ("fn", 4usize)));
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

    fn visible(window: &Window, n: usize) -> Vec<usize> {
        (1..=n).filter(|&i| window.try_find(("tab", i)).is_some()).collect()
    }

    fn dt_on(window: &Window, i: usize) -> bool {
        window.find(("dt", i)).selected() == Some(true)
    }

    fn on(window: &Window, i: usize) -> bool {
        window.find(("tab", i)).selected() == Some(true)
    }

    /// The New thread screen has no tabs; the card starts right under the bar and its 44px head
    /// says New thread at the board's x (card + 14).
    #[gpui_kit::test]
    fn new_thread_screen_has_no_tabs_and_a_card_head(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, Cauldron::new());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let s = layout::solve(W, H, ShellState { sidebar_open: true, drawer_open: true, split_open: false }, platform());
            assert!(window.try_find("tabs").is_none() && window.try_find(("tab", 0usize)).is_none(), "no tab before the thread exists");
            assert!(window.try_find("tab-add").is_none(), "no +");
            let card = rect(window, "card");
            assert_eq!((card.x, card.y, card.w, card.h), (s.main.x, s.main.y, s.main.w, s.main.h - 32.0), "card right under the bar, the stage bar under it");
            // The v3 bottom bar (Charles, 13 Nov): in the Shell's grey under the card, card-wide, 32 high,
            // usage at the left and the four stages at the right. Core has neither yet, so usage is
            // an empty state and no stage is current: plain fg3 labels, nothing to click.
            let bar = rect(window, "stage-bar");
            assert_eq!((bar.x, bar.y, bar.w, bar.h), (card.x, card.bottom(), card.w, 32.0), "stage bar");
            let shell = rect(window, "shell");
            assert_eq!((shell.x, shell.y, shell.right(), shell.bottom()), (s.shell.x, s.shell.y, s.shell.right(), s.shell.bottom()), "the Shell");
            assert_eq!(bar.bottom() + 6.0, shell.bottom(), "the bar in the Shell's bottom gutter, not on the frame");
            assert!(rect(window, "drawer").right() + 6.0 == shell.right(), "the drawer in the Shell's right gutter");
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
            assert_eq!((head.x, head.y, head.w, head.h), (card.x + 1.0, card.y + 1.0, card.w - 2.0, 44.0), "head");
            assert_eq!(window.find("card-title").label(), Some("New thread"));
            assert_eq!(rect(window, "card-title").x, card.x + 14.0, "title at the board's x");
        })
        .unwrap();
    }

    /// A thread's tabs float in the top bar (the v3 board's `.ftab`): control-sm 24, the open one
    /// selected, + the same height.
    #[gpui_kit::test]
    fn thread_tabs_float_in_the_bar(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_agents(3, &[]));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let s = layout::solve(W, H, ShellState { sidebar_open: true, drawer_open: true, split_open: false }, platform());
            let row = rect(window, "tabs");
            assert_eq!((row.x, row.y, row.w, row.h), (s.tabs.x, s.tabs.y, s.tabs.w, s.tabs.h), "tab row");
            assert!(row.bottom() <= s.bar.bottom(), "in the bar");
            for i in 0..=3usize {
                let t = rect(window, ("tab", i));
                assert_eq!((t.y, t.h), (row.y, 24.0), "tab {i}");
                assert_eq!(on(window, i), i == 0, "tab {i}: only the coordinator is open");
            }
            for i in 1..=3usize {
                assert_eq!(rect(window, ("tab", i)).x - rect(window, ("tab", i - 1)).right(), 4.0, "4px between tabs {} and {i}", i - 1);
            }
            let add = rect(window, "tab-add");
            assert_eq!((add.y, add.h), (row.y, 24.0), "+ as tall as the tabs");
            window.click(("tab", 2usize), cx);
            assert!(on(window, 2) && !on(window, 0), "a click opens the tab");
            assert_eq!(rect(window, "card").y, s.main.y, "the card stays under the bar");
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
                assert!(window.try_find(("tab-fig", i)).is_none(), "tab {i} has no figure");
            }
        })
        .unwrap();
        let handle = open(cx, Cauldron { coordinator: Some("Plain thread".into()), ..Cauldron::new() });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("tab", 0usize)).label(), Some("Plain thread"));
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
            assert_eq!(window.find("group-needs").label(), Some("Needs you"), "no count");
            assert!(rect(window, "group-needs").bottom() <= rect(window, ("group", 0usize)).y, "first, with no rows under it");
            assert_eq!(window.find("usage-left").label(), Some("No data yet"));
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
                let room = row.w - anchor.w - 28.0;
                let need = n as f32 * 76.0 + 4.0;
                let arrows = window.try_find("tab-prev").is_some();
                assert_eq!(arrows, need > room, "{n}: arrows iff 72px tabs do not fit (need {need}, room {room})");
                let seen = visible(window, n);
                if arrows {
                    assert!(seen.len() < n, "{n}: scrolling hides some tabs");
                    for id in ["tab-prev", "tab-next"] {
                        let r = rect(window, id);
                        assert_eq!((r.w, r.h), (24.0, 24.0), "{id}: as tall as the tabs");
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
            let heads: Vec<String> = (0..4usize).map(|g| window.find(("group", g)).label().unwrap().to_string()).collect();
            assert_eq!(heads, ["Unread, 1", "Running, 1", "Failed, 1", "Done, 2"]);
            for g in 1..4usize {
                assert!(rect(window, ("group", g - 1)).y < rect(window, ("group", g)).y, "group {g} below the one before");
            }
            assert_eq!(row_labels(window), ["Koi palette", "Rampart mutation score", "M2 eval task texts", "Pinned notes", "Agent delegation"]);
            let side = rect(window, "sidebar");
            for i in 0..5usize {
                let r = rect(window, ("thread", i));
                assert!(r.x >= side.x && r.right() <= side.right(), "row {i} inside the sidebar");
            }
            assert!(rect(window, ("thread", 0usize)).y > rect(window, "new-thread").bottom(), "rows under New thread");
        })
        .unwrap();
    }

    /// The rows are the new-look board's `.th`: 30 high, the title, the repo in 11px, and a 6px ping
    /// on an unread row only. Every row shows a state dot in its 18x22 slot, an agentic thread too:
    /// agent figures belong to the Agents page (Charles, 13 Nov). Group heads are the board's 36px
    /// `.sect`; New thread is a 32px row, not a button.
    #[gpui_kit::test]
    fn thread_rows_are_the_boards(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = open(cx, with_threads());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(rect(window, "new-thread").h, 32.0);
            for g in 0..4usize {
                assert_eq!(rect(window, ("group", g)).h, 36.0, "group {g}");
            }
            // Row 1 (Rampart mutation score) has an agent; it still shows a dot.
            for (i, state) in [(0usize, "Unread"), (1, "Working"), (2, "Failed"), (3, "Done"), (4, "Done")] {
                let r = rect(window, ("thread", i));
                assert_eq!(r.h, 30.0, "row {i}");
                let a = rect(window, ("thread-av", i));
                assert_eq!((a.w, a.h), (18.0, 22.0), "row {i}'s slot");
                assert!((a.y + a.h / 2.0 - (r.y + r.h / 2.0)).abs() <= 1.0, "row {i}'s slot is centred");
                assert!((10.0..=13.0).contains(&(a.x - r.x)), "row {i}'s slot is 11px in, as the board's");
                assert_eq!(window.find(("thread-av", i)).label(), Some(state), "row {i}");
                assert!(window.try_find(("thread-fig", i)).is_none(), "row {i}: no figure");
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
            assert_eq!(window.find("search").label(), Some("Search threads"));
            window.click(("thread", 1usize), cx);
            window.press("down", cx);
            window.press("down", cx);
        })
        .unwrap();
        let sel = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| c.selected.clone()).unwrap();
        assert_eq!(sel(cx).as_deref(), Some("p"), "click Running row, then two down: Failed, then Done's first");
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
            assert_eq!(window.find(("group", 0usize)).label(), Some("Unread, 1"));
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
