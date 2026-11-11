//! The Cauldron shell (UI1): the wireframe's frame, notch, sidebar card, main card with tabs,
//! right rail, drawer and split. Every rectangle comes from `cauldron_chrome::layout::solve`,
//! which is tested against numbers measured from the wireframe; nothing here picks a position.
//!
//! Content inside the panes is a labelled placeholder until the milestone that owns it
//! (docs/cauldron/UI-PLAN.md): the thread list is UI3, the thread view UI4, the drawer UI6.

use cauldron_chrome::layout::{self, metric, Platform, Rect, ShellState};
use cauldron_chrome::tokens::{self, radius, Palette, Rgb};
use gpui_kit::*;

use crate::ui::UI;

/// Where the shell draws window controls: lights on macOS, caption buttons elsewhere.
pub(crate) fn platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Mac
    } else {
        Platform::Windows
    }
}

/// Top-left of the traffic lights, from the wireframe (`layout::layout` Reference lights rect).
pub(crate) fn lights_origin() -> (f32, f32) {
    let s = layout::layout(1240.0, 702.0, ShellState { sidebar_open: true, drawer_open: false, split_open: false }, Platform::Mac);
    let l = s.lights.expect("mac layout has lights");
    (l.x, l.y)
}

fn c(v: Rgb) -> Hsla {
    gpui_kit::rgb(((v.0 as u32) << 16) | ((v.1 as u32) << 8) | v.2 as u32).into()
}

fn ca(v: Rgb, a: f32) -> Hsla {
    let mut h = c(v);
    h.a = a;
    h
}

fn abs(r: Rect) -> Div {
    div().absolute().left(px(r.x)).top(px(r.y)).w(px(r.w)).h(px(r.h))
}

#[derive(Clone, Copy)]
enum Foot {
    Left,
    Right,
}

/// The concave corner where the open tab flares into the card, the wireframe's `.tab.on::before/::after`.
fn foot(side: Foot, fill: Hsla) -> impl IntoElement {
    let r = radius::R;
    let el = canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            let p = |x: f32, y: f32| point(b.origin.x + px(x), b.origin.y + px(y));
            let mut f = PathBuilder::fill();
            // The square's corner away from the tab stays empty: a quarter circle centred there is cut out.
            match side {
                Foot::Left => {
                    f.move_to(p(r, 0.));
                    f.line_to(p(r, r));
                    f.line_to(p(0., r));
                    f.arc_to(point(px(r), px(r)), px(0.), false, false, p(r, 0.));
                }
                Foot::Right => {
                    f.move_to(p(0., 0.));
                    f.line_to(p(0., r));
                    f.line_to(p(r, r));
                    f.arc_to(point(px(r), px(r)), px(0.), false, true, p(0., 0.));
                }
            }
            f.close();
            if let Ok(path) = f.build() {
                window.paint_path(path, fill);
            }
        },
    )
    .absolute()
    .bottom_0()
    .size(px(r));
    match side {
        Foot::Left => el.left(px(-r)),
        Foot::Right => el.right(px(-r)),
    }
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

pub(crate) struct Cauldron {
    sidebar_open: bool,
    drawer: Option<Panel>,
    split: bool,
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
    tab_focus: Option<FocusHandle>,
    /// The icon button under the pointer, whether its tip is showing yet, and until when a
    /// neighbour's tip may show at once (the wireframe's `.rail.warm`).
    tip: Option<ElementId>,
    tip_shown: bool,
    tip_warm_until: Option<std::time::Instant>,
    tip_wait: Option<Task<()>>,
    /// First agent tab in view once the row scrolls.
    tab_first: usize,
}

impl Cauldron {
    /// The window's first state. Debug builds only: `CAULDRON_FIXTURE_AGENTS=n` opens a thread with
    /// n agent tabs, for screenshots until UI4 brings real agents from core.
    pub(crate) fn launch() -> Self {
        let mut c = Self::new();
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
        Cauldron { sidebar_open: true, drawer: Some(Panel::Highlights), split: false, coordinator: None, agents: Vec::new(), idle: Vec::new(), active: 0, reveal: false, tab_window: 0, wheel: 0.0, tab_menu: false, tab_focus: None, tip: None, tip_shown: false, tip_warm_until: None, tip_wait: None, tab_first: 0 }
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
                let st = (self.wheel / (TAB_MIN + 2.0)).trunc();
                self.wheel -= st * (TAB_MIN + 2.0);
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
            div().id(id).flex().items_center().gap(px(8.)).px(px(8.)).py(px(6.)).rounded(px(radius::R1))
        };
        let mut menu = div()
            .id("tab-menu")
            .absolute()
            .top_full()
            .mt(px(6.))
            .right_0()
            .w(px(236.))
            .p(px(4.))
            .flex()
            .flex_col()
            .gap(px(1.))
            .rounded(px(radius::R))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .shadow(vec![drop(tokens::shadow::MENU)])
            .font_family(UI)
            .text_size(px(12.5))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(pal.fg2))
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.tab_menu = false;
                cx.notify();
            }))
            // ponytail: spawning needs the thread view's core wiring (UI4); shown, not live.
            .child(item("menu-spawn".into()).text_color(c(pal.fg3)).aria_label("Spawn a subagent").child(glyph("plus", 14., c(pal.fg3))).child("Spawn a subagent…").test_support());
        if !self.idle.is_empty() {
            menu = menu.child(div().px(px(8.)).pt(px(6.)).pb(px(2.)).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(c(pal.fg3)).child("Open 1:1 with"));
        }
        for (j, a) in self.idle.iter().enumerate() {
            menu = menu.child(
                item(("menu-agent", j).into())
                    .cursor_pointer()
                    .hover(|d| d.bg(ca(pal.fg, 0.06)).text_color(c(pal.fg)))
                    .aria_label(a.name.clone())
                    .child(div().flex_none().size(px(8.)).rounded_full().bg(c(tokens::AGENT_DOTS[a.dot % tokens::AGENT_DOTS.len()])))
                    .child(a.name.clone())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_agent(j);
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        deferred(menu.test_support()).with_priority(2)
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

    fn toggle_panel(&mut self, p: Panel) {
        self.drawer = if self.drawer == Some(p) { None } else { Some(p) };
    }
}

/// A square icon button, 28px, the wireframe's `.fn`, `.ib` and `.rb`.
fn icon_button(id: impl Into<ElementId>, name: &str, label: &'static str, pal: &'static Palette, on: bool) -> Stateful<Div> {
    let hover = ca(pal.fg, 0.05);
    let sel = ca(pal.fg, 0.09);
    div()
        .id(id)
        .w(px(metric::SBTN))
        .h(px(metric::SBTN))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::R1))
        .cursor_pointer()
        .bg(if on { sel } else { gpui_kit::transparent_black() })
        .hover(move |d| d.bg(if on { sel } else { hover }))
        .child(glyph(name, 16., if on { c(pal.fg) } else { c(pal.fg2) }))
        .aria_label(label)
}

/// Agent tabs share the room between these widths; below TAB_MIN the row scrolls.
const TAB_MIN: f32 = 72.0;
const TAB_MAX: f32 = 210.0;
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
        .rounded(px(radius::R2))
        .bg(c(pal.raised))
        .border_1()
        .border_color(c(pal.line))
        .shadow(vec![drop(tokens::shadow::TIP)])
        .font_family(UI)
        .text_size(px(12.))
        .line_height(px(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(pal.fg))
        .aria_label(label.clone())
        .child(label);
    let d = match side {
        Side::Right => d.left_full().ml(px(8.)).top(px(mid)),
        Side::Left => d.right_full().mr(px(8.)).top(px(mid)),
        Side::Below => d.top_full().mt(px(8.)).right_0(),
    };
    deferred(d.test_support()).with_priority(1)
}

impl Render for Cauldron {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = !matches!(window.appearance(), WindowAppearance::Light | WindowAppearance::VibrantLight);
        let pal: &'static Palette = if dark { &tokens::DARK } else { &tokens::LIGHT };
        let vs = window.viewport_size();
        let (w, h) = (f32::from(vs.width), f32::from(vs.height));
        let plat = platform();
        let want = ShellState { sidebar_open: self.sidebar_open, drawer_open: self.drawer.is_some(), split_open: self.split };
        let s = layout::solve(w, h, want, plat);

        let frame = c(pal.frame);
        let card = c(pal.card);
        let line = c(pal.line);
        let ring = ca(pal.fg, if dark { 0.08 } else { 0.09 });
        let fg = c(pal.fg);
        let fg2 = c(pal.fg2);
        let fg3 = c(pal.fg3);
        let accent = c(pal.accent);

        let mut root = div()
            .id("cauldron")
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(frame)
            .font_family(UI)
            .text_size(px(13.))
            .text_color(fg);

        // Window drag strip, under every control.
        root = root.child(
            abs(Rect::new(1.0, 1.0, w - 2.0, metric::NOTCH_H - 8.0)).window_control_area(WindowControlArea::Drag),
        );

        // ---- rail (only while the sidebar is hidden)
        if let Some(r) = s.rail {
            let mut col = pane("rail", r).flex().flex_col().items_center().gap(px(4.)).pt(px(metric::NOTCH_H));
            for (i, (name, label)) in FUNCTIONS.iter().enumerate() {
                col = col.child(self.tipped(("rail", i), icon_button(("rail", i), name, label, pal, i == 0), *label, Side::Right, pal, cx).test_support());
            }
            root = root.child(col.test_support());
        }

        // ---- sidebar card with the function row
        if let Some(r) = s.sidebar {
            let mut fnrow = div().flex().justify_between().px(px(2.)).pt(px(12.)).pb(px(8.));
            for (i, (name, label)) in FUNCTIONS.iter().enumerate() {
                fnrow = fnrow.child(self.tipped(("fn", i), icon_button(("fn", i), name, label, pal, i == 0), *label, Side::Right, pal, cx).test_support());
            }
            let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "You".into());
            let initial = user.chars().next().map(|ch| ch.to_uppercase().to_string()).unwrap_or_else(|| "Y".into());
            let new_thread = div()
                .id("new-thread")
                .h(px(32.))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(10.))
                .rounded(px(radius::R1))
                .cursor_pointer()
                .bg(ca(pal.accent, 0.15))
                .border_1()
                .border_color(ca(pal.accent, 0.65))
                .shadow(vec![BoxShadow { color: ca(pal.accent, 0.30), offset: point(px(0.), px(0.)), blur_radius: px(16.), spread_radius: px(-2.), inset: false }])
                .child(glyph("plus", 14., accent))
                .child(div().text_size(px(13.)).text_color(fg).child("New thread"));
            let card_el = pane("sidebar", r)
                .flex()
                .flex_col()
                .rounded(px(radius::R))
                .bg(card)
                .border_1()
                .border_color(ring)
                .overflow_hidden()
                .child(
                    div()
                        .h(px(metric::TAB_H))
                        .flex_none()
                        .flex()
                        .items_center()
                        .pl(px(metric::NOTCH_W - metric::MARGIN_SIDE - 1.0))
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Threads"),
                )
                .child(div().px(px(5.)).child(fnrow))
                .child(
                    div().px(px(5.)).pb(px(6.)).child(
                        div()
                            .h(px(30.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(9.))
                            .rounded(px(radius::R1))
                            .border_1()
                            .border_color(line)
                            .text_color(fg3)
                            .child(glyph("search", 14., fg3))
                            .child("Search threads"),
                    ),
                )
                .child(div().px(px(5.)).pb(px(8.)).child(new_thread))
                .child(
                    div()
                        .flex_1()
                        .px(px(14.))
                        .pt(px(6.))
                        .text_size(px(12.))
                        .text_color(fg3)
                        .child("No threads yet. Start one with New thread."),
                )
                .child(
                    div()
                        .h(px(44.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(10.))
                        .border_t_1()
                        .border_color(line)
                        .child(
                            div()
                                .size(px(24.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .bg(gpui_kit::rgb(0xb5532e))
                                .text_size(px(11.))
                                .text_color(gpui_kit::rgb(0xffffff))
                                .child(initial),
                        )
                        .child(div().flex_1().text_size(px(12.5)).child(user))
                        .child(self.tipped("settings", icon_button("settings", "settings", "Settings", pal, false), "Settings", Side::Right, pal, cx).test_support()),
                );
            root = root.child(card_el.test_support());
        }

        // ---- main column: one thread's agents as folder tabs on the frame, the card under them.
        // The coordinator (or "New thread" before the thread exists) is pinned at the left. Agent
        // tabs share the room equally between TAB_MIN and TAB_MAX; when TAB_MIN no longer fits,
        // arrows appear beside + and the row shows a window of 72px tabs.
        let focus = self.tab_focus.get_or_insert_with(|| cx.focus_handle()).clone();
        let thread = self.coordinator.is_some();
        let anchor_label: SharedString = self.coordinator.clone().unwrap_or_else(|| NEW_TAB.into());
        let label_w = {
            let run = TextRun { len: anchor_label.len(), font: Font { weight: FontWeight::MEDIUM, ..font(UI) }, color: fg, background_color: None, underline: None, strikethrough: None };
            f32::from(window.text_system().shape_line(anchor_label.clone(), px(12.), &[run], None).width)
        };
        // padding 12 + icon 14 + gap 7 + label + padding 12, as the wireframe's `.tab`
        let anchor_w = (45.0 + label_w).ceil().min(ANCHOR_MAX);
        let n = self.agents.len();
        let room = s.tabs.w - anchor_w - if thread { 24.0 } else { 0.0 };
        let scrolls = n as f32 * (TAB_MIN + 2.0) + 2.0 > room;
        let (k, tab_w) = if scrolls {
            (((room - 54.0) / (TAB_MIN + 2.0)).floor().max(1.0) as usize, Some(TAB_MIN))
        } else if n > 0 {
            let per = ((room - 2.0 * (n as f32 + 1.0)) / n as f32).floor();
            (n, (per < TAB_MAX).then_some(per))
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

        let mut row = div()
            .id("tabs")
            .absolute()
            .left_0()
            .top_0()
            .w(px(s.tabs.w))
            .h(px(s.tabs.h))
            .flex()
            .items_start()
            .gap(px(2.))
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
        // from the row, so each tab carries the row's wheel handler too.

        // The pinned tab.
        let on = self.active == 0;
        let mut anchor = div()
            .id(("tab", 0usize))
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(7.))
            .w(px(anchor_w))
            .h(px(metric::TAB_H + if on { 1.0 } else { 0.0 }))
            .px(px(12.))
            .rounded_t(px(radius::R))
            .occlude()
            .cursor_pointer()
            .aria_label(anchor_label.clone())
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if on { fg } else { fg3 })
            .child(if thread { glyph("code", 14., c(pal.info)) } else { glyph("square-pen", 14., if on { fg2 } else { fg3 }) })
            .child(div().min_w_0().truncate().child(anchor_label.clone()))
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| this.wheel_tabs(ev, cx)))
            .on_click(cx.listener(|this, _, window, cx| {
                this.active = 0;
                this.focus_tabs(window, cx);
                cx.notify();
            }));
        if on {
            // The first tab sits on the card's corner: no left foot, and the card's corner is square.
            anchor = anchor.bg(card).child(foot(Foot::Right, card));
        }
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
                .rounded(px(radius::R2))
                // Inactive tabs show their close on hover, over the end of the name, so a 72px tab
                // still has room for a few letters.
                .when_live(!on, |d| d.absolute().right(px(6.)).top(px(8.)).bg(frame).opacity(0.).group_hover(group.clone(), |s| s.opacity(1.)))
                .hover(|d| d.bg(ca(pal.fg, 0.08)))
                .aria_label("Close tab")
                .child(glyph("x", 12., fg3))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.close_agent(j);
                    this.reveal = true;
                    cx.notify();
                }))
                .test_support();
            let mut tab = div()
                .id(("tab", i))
                .group(group)
                .relative()
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(metric::TAB_H + if on { 1.0 } else { 0.0 }))
                .pl(px(10.))
                .pr(px(6.))
                .rounded_t(px(radius::R))
                .occlude()
                .cursor_pointer()
                .aria_label(agent.name.clone())
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if on { fg } else { fg3 })
                .child(div().flex_none().size(px(8.)).rounded_full().bg(c(tokens::AGENT_DOTS[agent.dot % tokens::AGENT_DOTS.len()])))
                .child(div().min_w_0().truncate().child(agent.name.clone()))
                .child(close)
                .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| this.wheel_tabs(ev, cx)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.active = i;
                    this.reveal = true;
                    this.focus_tabs(window, cx);
                    cx.notify();
                }));
            tab = match tab_w {
                Some(w) => tab.flex_none().w(px(w)),
                None => tab.flex_none().max_w(px(TAB_MAX)),
            };
            if on {
                tab = tab.bg(card).child(foot(Foot::Right, card)).child(foot(Foot::Left, card));
            }
            // The full name as a tip once the tab is narrower than the widest it gets.
            let tab = if tab_w.is_some() { self.tipped(("tab", i), tab, agent.name.clone(), Side::Below, pal, cx) } else { tab };
            row = row.child(tab.test_support());
        }

        if thread {
            let small_btn = |id: &'static str, icon: &'static str, label: &'static str, live: bool| {
                div()
                    .id(id)
                    .w(px(24.))
                    .h(px(28.))
                    .mt(px(3.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::R1))
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
                .mt(px(5.))
                .size(px(24.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::R1))
                .occlude()
                .cursor_pointer()
                .hover(|d| d.bg(ca(pal.fg, 0.08)))
                .aria_label(ADD_LABEL)
                .child(glyph("plus", 14., fg3))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tab_menu = !this.tab_menu;
                    cx.notify();
                }));
            if self.tab_menu {
                add = add.child(self.tab_menu_el(pal, cx));
            }
            let add = if self.tab_menu { add } else { self.tipped("tab-add", add, ADD_LABEL, Side::Below, pal, cx) };
            row = row.child(add.test_support());
        }

        let mut card_el = div()
            .id("card")
            .absolute()
            .left_0()
            .top(px(s.tabs.h))
            .w(px(s.main.w))
            .h(px(s.main.h - s.tabs.h))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::R))
            .bg(card)
            .border_1()
            .border_color(ring)
            .text_color(fg3)
            .child("The thread view arrives in UI4.");
        if self.active == 0 {
            // The first tab sits on the card's corner, so the corner is square.
            card_el = card_el.rounded_tl(px(0.));
        }
        root = root.child(pane("main", s.main).child(card_el.test_support()).child(row.test_support()).test_support());

        // ---- split card
        if let Some(r) = s.half {
            let tiles = ["Highlights", "Files", "Changes", "Terminal", "Browser", "Add an app"];
            let mut grid = div().flex().flex_wrap().gap(px(8.)).p(px(12.));
            for t in tiles {
                grid = grid.child(
                    div()
                        .w(px(122.))
                        .p(px(10.))
                        .rounded(px(radius::R1))
                        .bg(c(pal.raised))
                        .border_1()
                        .border_color(ring)
                        .text_size(px(12.5))
                        .child(t),
                );
            }
            root = root.child(
                pane("half", r)
                    .rounded(px(radius::R))
                    .bg(card)
                    .border_1()
                    .border_color(ring)
                    .overflow_hidden()
                    .child(div().px(px(12.)).pt(px(10.)).text_color(fg3).text_size(px(12.)).child("Open in this half"))
                    .child(grid)
                    .test_support(),
            );
        }

        // ---- drawer (frame colour, no border on the shared edge)
        if let (Some(r), Some(p)) = (s.drawer, self.drawer) {
            root = root.child(
                pane("drawer", r)
                    .flex()
                    .flex_col()
                    .px(px(10.))
                    .pt(px(8.))
                    .gap(px(6.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(p.title()))
                    .child(div().text_size(px(12.)).text_color(fg3).child(format!("This panel arrives in {}.", p.milestone())))
                    .test_support(),
            );
        }

        // ---- right rail
        let mut rr = pane("rrail", s.rrail).flex().flex_col().items_center().gap(px(4.)).pt(px(metric::TAB_H + 4.0));
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

        // ---- notch: cuts the sidebar corner, holds the lights and the sidebar button
        if let Some(n) = s.notch {
            let nw = n.w;
            let (fill_c, line_c) = (frame, ring);
            root = root.child(
                canvas(
                    |_, _, _| (),
                    move |b, _, window, _| {
                        let o = b.origin;
                        let p = |x: f32, y: f32| point(o.x + px(x), o.y + px(y));
                        let r6 = point(px(6.), px(6.));
                        let mut f = PathBuilder::fill();
                        f.move_to(p(0., 0.));
                        f.line_to(p(nw, 0.));
                        f.line_to(p(nw, 8.));
                        f.curve_to(p(nw - 6., 14.), p(nw - 6., 8.));
                        f.line_to(p(nw - 6., 36.));
                        f.curve_to(p(nw - 12., 42.), p(nw - 6., 42.));
                        f.line_to(p(18., 42.));
                        f.arc_to(r6, px(0.), false, false, p(12., 48.));
                        f.line_to(p(0., 48.));
                        f.close();
                        if let Ok(path) = f.build() {
                            window.paint_path(path, fill_c);
                        }
                        let mut l = PathBuilder::stroke(px(1.));
                        l.move_to(p(12., 48.));
                        l.arc_to(r6, px(0.), false, true, p(18., 42.));
                        l.line_to(p(nw - 12., 42.));
                        l.curve_to(p(nw - 6., 36.), p(nw - 6., 42.));
                        l.line_to(p(nw - 6., 14.));
                        l.curve_to(p(nw, 8.), p(nw - 6., 8.));
                        if let Ok(path) = l.build() {
                            window.paint_path(path, line_c);
                        }
                    },
                )
                .absolute()
                .left(px(n.x))
                .top(px(n.y))
                .w(px(n.w))
                .h(px(n.h)),
            );
        }

        // ---- sidebar button (inside the notch when the sidebar is open)
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
                    self.tipped("top-split", icon_button("top-split", "columns-2", "Split", pal, self.split), "Split", Side::Below, pal, cx).occlude().on_click(cx.listener(|this, _, _, cx| {
                        this.split = !this.split;
                        cx.notify();
                    }))
                    .test_support(),
                )
                .test_support(),
        );

        // ---- Windows caption buttons (macOS uses the native lights)
        if let Some(r) = s.caps {
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
    use super::{platform, AgentTab, Cauldron, Panel};
    use cauldron_chrome::layout::{self, Rect, ShellState};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{px, size, AppContext, ElementId, TestAppContext, Window};

    const W: f32 = 1240.0;
    const H: f32 = 702.0;

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
    /// in all eight combinations of sidebar, drawer and split.
    #[gpui_kit::test]
    fn panes_match_the_layout_solver_in_every_state(cx: &mut TestAppContext) {
        for bits in 0..8u8 {
            let state = ShellState { sidebar_open: bits & 1 != 0, drawer_open: bits & 2 != 0, split_open: bits & 4 != 0 };
            let handle = cx.add_window(move |_, _| Cauldron {
                sidebar_open: state.sidebar_open,
                drawer: state.drawer_open.then_some(Panel::Highlights),
                split: state.split_open,
                ..Cauldron::new()
            });
            cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                let s = layout::solve(W, H, state, platform());
                assert_rect(window, "main", s.main, state);
                assert_rect(window, "rrail", s.rrail, state);
                assert_rect(window, "sbtn-slot", s.sidebar_button, state);
                assert_rect(window, "topr", s.topr, state);
                for (id, r) in [("sidebar", s.sidebar), ("rail", s.rail), ("drawer", s.drawer), ("half", s.half), ("caps", s.caps)] {
                    match r {
                        Some(r) => assert_rect(window, id, r, state),
                        None => assert!(window.try_find(id).is_none(), "{id} drawn in {state:?} but solve() has none"),
                    }
                }
            })
            .unwrap();
        }
    }

    /// Icon-only buttons carry their name for screen readers. Hovering one shows the name beside
    /// it, as the wireframe's `.fn[data-tip]`: 8px away, centred, after 450ms, and at once while
    /// the pointer moves between neighbours.
    #[gpui_kit::test]
    fn icon_buttons_have_names_and_tooltips(cx: &mut TestAppContext) {
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

    fn rect(window: &Window, id: impl Into<ElementId>) -> Rect {
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
        let handle = cx.add_window(move |_, _| c);
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        handle
    }

    fn visible(window: &Window, n: usize) -> Vec<usize> {
        (1..=n).filter(|&i| window.try_find(("tab", i)).is_some()).collect()
    }

    /// The New thread screen: one tab that cannot be closed and no +, sitting on the card's corner.
    #[gpui_kit::test]
    fn new_thread_screen_has_one_fixed_tab_joined_to_the_card(cx: &mut TestAppContext) {
        let handle = open(cx, Cauldron::new());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let s = layout::solve(W, H, ShellState { sidebar_open: true, drawer_open: true, split_open: false }, platform());
            let row = rect(window, "tabs");
            assert_eq!((row.x, row.y, row.w, row.h), (s.tabs.x, s.tabs.y, s.tabs.w, s.tabs.h), "tab row");
            let card = rect(window, "card");
            assert_eq!((card.x, card.y, card.w, card.h), (s.main.x, s.main.y + s.tabs.h, s.main.w, s.main.h - s.tabs.h), "card");
            assert_eq!(window.find(("tab", 0usize)).label(), Some("New thread"));
            assert_eq!(rect(window, ("tab", 0usize)).bottom(), card.y + 1.0, "the open tab covers the card's top edge");
            assert!(window.try_find(("tab-close", 0usize)).is_none(), "not closable");
            assert!(window.try_find("tab-add").is_none(), "no + before the thread exists");
        })
        .unwrap();
    }

    /// Agent tabs share the room equally between 72 and 210px; the arrows appear exactly when 72px
    /// tabs no longer fit; the coordinator is pinned at the left, never scrolls and has no close.
    #[gpui_kit::test]
    fn agent_tabs_clamp_then_scroll_with_the_coordinator_pinned(cx: &mut TestAppContext) {
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
                let room = row.w - anchor.w - 24.0;
                let need = n as f32 * 74.0 + 2.0;
                let arrows = window.try_find("tab-prev").is_some();
                assert_eq!(arrows, need > room, "{n}: arrows iff 72px tabs do not fit (need {need}, room {room})");
                let seen = visible(window, n);
                if arrows {
                    assert!(seen.len() < n, "{n}: scrolling hides some tabs");
                    for id in ["tab-prev", "tab-next"] {
                        let r = rect(window, id);
                        assert_eq!((r.w, r.h), (24.0, 28.0), "{id}");
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
        let n = 14;
        let handle = open(cx, with_agents(n, &[]));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("tab", 1usize), cx);
            window.press("end", cx);
            assert!(visible(window, n).contains(&n), "End opens and shows the last tab");
            assert_eq!(rect(window, ("tab", n)).bottom(), rect(window, "card").y + 1.0, "and it is the open one");
            for _ in 0..20 {
                window.click("tab-prev", cx);
            }
            assert!(!visible(window, n).contains(&n), "the arrows may scroll the open tab away");
            window.press("left", cx);
            assert!(visible(window, n).contains(&(n - 1)), "Left opens the tab before and shows it");
            window.press("home", cx);
            assert_eq!(rect(window, ("tab", 0usize)).bottom(), rect(window, "card").y + 1.0, "Home opens the coordinator");
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
        let handle = open(cx, with_agents(2, &["Tester"]));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("tab-add").label(), Some("Spawn a subagent or open a 1:1"));
            window.click("tab-add", cx);
            assert_eq!(window.find(("menu-agent", 0usize)).label(), Some("Tester"));
            window.click(("menu-agent", 0usize), cx);
            assert!(window.try_find("tab-menu").is_none(), "the menu closes");
            assert_eq!(window.find(("tab", 3usize)).label(), Some("Tester"));
            assert_eq!(rect(window, ("tab", 3usize)).bottom(), rect(window, "card").y + 1.0, "the new tab is open");
        })
        .unwrap();
    }
}
