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
    /// Open tabs in the main card, by title, and which one is showing.
    tabs: Vec<SharedString>,
    active: usize,
    /// The icon button under the pointer, whether its tip is showing yet, and until when a
    /// neighbour's tip may show at once (the wireframe's `.rail.warm`).
    tip: Option<ElementId>,
    tip_shown: bool,
    tip_warm_until: Option<std::time::Instant>,
    tip_wait: Option<Task<()>>,
    /// First tab in view once the row scrolls; past the end means "show the last ones".
    tab_first: usize,
}

impl Cauldron {
    pub(crate) fn new() -> Self {
        Cauldron { sidebar_open: true, drawer: Some(Panel::Highlights), split: false, tabs: vec![NEW_TAB.into()], active: 0, tip: None, tip_shown: false, tip_warm_until: None, tip_wait: None, tab_first: 0 }
    }

    fn open_tab(&mut self) {
        self.tabs.push(NEW_TAB.into());
        self.active = self.tabs.len() - 1;
        self.tab_first = usize::MAX;
    }

    fn close_tab(&mut self, i: usize) {
        self.tabs.remove(i);
        if self.tabs.is_empty() {
            self.tabs.push(NEW_TAB.into());
        }
        if self.active > i || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
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
    fn tipped(&self, key: impl Into<ElementId>, el: Stateful<Div>, label: &'static str, side: Side, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let key = key.into();
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

/// Narrowest a tab gets before the row scrolls instead.
const TAB_MIN: f32 = 96.0;

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
fn tip(label: &'static str, side: Side, pal: &'static Palette) -> impl IntoElement {
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
        .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.3), offset: point(px(0.), px(6.)), blur_radius: px(16.), spread_radius: px(0.), inset: false }])
        .font_family(UI)
        .text_size(px(12.))
        .line_height(px(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(pal.fg))
        .aria_label(label)
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
                col = col.child(self.tipped(("rail", i), icon_button(("rail", i), name, label, pal, i == 0), label, Side::Right, pal, cx).test_support());
            }
            root = root.child(col.test_support());
        }

        // ---- sidebar card with the function row
        if let Some(r) = s.sidebar {
            let mut fnrow = div().flex().justify_between().px(px(2.)).pt(px(12.)).pb(px(8.));
            for (i, (name, label)) in FUNCTIONS.iter().enumerate() {
                fnrow = fnrow.child(self.tipped(("fn", i), icon_button(("fn", i), name, label, pal, i == 0), label, Side::Right, pal, cx).test_support());
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

        // ---- main column: folder tabs on the frame, the card under them
        let mut row = div().id("tabs").absolute().left_0().top_0().w(px(s.tabs.w)).h(px(s.tabs.h)).flex().items_start().gap(px(2.));
        // Tabs share the row with + (24px) and 2px gaps, up to the wireframe's 208px each. Below
        // TAB_MIN they stop shrinking: arrows (24px each) appear and the row shows a window of tabs.
        let n = self.tabs.len();
        let fit = ((s.tabs.w - 24.0 - 2.0 * n as f32) / n as f32).floor();
        let (first, shown, tab_w) = if fit >= TAB_MIN {
            (0, n, fit.min(208.0))
        } else {
            let k = (((s.tabs.w - 76.0) / (TAB_MIN + 2.0)).floor() as usize).max(1);
            let first = self.tab_first.min(n - k);
            (first, k, ((s.tabs.w - 76.0 - 2.0 * k as f32) / k as f32).floor())
        };
        let scrolls = shown < n;
        let narrow = tab_w < 88.0;
        let small_btn = |id: &'static str, icon: &'static str, label: &'static str, live: bool| {
            div()
                .id(id)
                .mt(px(5.))
                .size(px(24.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::R1))
                .occlude()
                .when_live(live, |d| d.cursor_pointer().hover(|d| d.bg(ca(pal.fg, 0.08))))
                .aria_label(label)
                .child(glyph(icon, 14., if live { fg3 } else { ca(pal.fg3, 0.4) }))
        };
        if scrolls {
            let prev = small_btn("tab-prev", "chevron-left", "Earlier tabs", first > 0).on_click(cx.listener(move |this, _, _, cx| {
                this.tab_first = first.saturating_sub(1);
                cx.notify();
            }));
            row = row.child(prev.test_support());
        }
        for (i, title) in self.tabs.iter().enumerate().skip(first).take(shown) {
            let on = i == self.active;
            let close = div()
                .id(("tab-close", i))
                .size(px(24.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::R1))
                .hover(|d| d.bg(ca(pal.fg, 0.08)))
                .aria_label("Close tab")
                .child(glyph("x", 12., fg3))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.close_tab(i);
                    cx.notify();
                }))
                .test_support();
            let mut tab = div()
                .id(("tab", i))
                .relative()
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(metric::TAB_H + if on { 1.0 } else { 0.0 }))
                .pl(px(12.))
                .pr(px(6.))
                .max_w(px(tab_w))
                .when_live(scrolls, |d| d.w(px(tab_w)))
                .rounded_t(px(radius::R))
                .occlude()
                .cursor_pointer()
                .aria_label(title.clone())
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if on { fg } else { fg3 })
                .children((!narrow).then(|| div().flex_none().child(glyph("square-pen", 14., if on { fg2 } else { fg3 }))))
                .child(div().min_w_0().truncate().child(title.clone()))
                .child(close)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active = i;
                    cx.notify();
                }));
            if on {
                tab = tab.bg(card).child(foot(Foot::Right, card));
                if !(i == 0 && !scrolls) {
                    tab = tab.child(foot(Foot::Left, card));
                }
            }
            row = row.child(tab.test_support());
        }
        if scrolls {
            let next = small_btn("tab-next", "chevron-right", "Later tabs", first + shown < n).on_click(cx.listener(move |this, _, _, cx| {
                this.tab_first = (first + 1).min(n - shown);
                cx.notify();
            }));
            row = row.child(next.test_support());
        }
        let add = small_btn("tab-add", "plus", "New tab", true)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.open_tab();
                    cx.notify();
                }));
        row = row.child(self.tipped("tab-add", add, "New tab", Side::Below, pal, cx).test_support());
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
        if self.active == 0 && !scrolls {
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
    use super::{platform, Cauldron, Panel};
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

    /// Tabs open with +, switch on click, close with their x; closing the last leaves a New thread.
    /// The tab row and the card sit where `solve()` says, and the open tab covers the card's top edge.
    #[gpui_kit::test]
    fn tabs_open_switch_close_and_join_the_card(cx: &mut TestAppContext) {
        let handle = cx.add_window(|_, _| Cauldron::new());
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let s = layout::solve(W, H, ShellState { sidebar_open: true, drawer_open: true, split_open: false }, platform());
            let row = rect(window, "tabs");
            assert_eq!((row.x, row.y, row.w, row.h), (s.tabs.x, s.tabs.y, s.tabs.w, s.tabs.h), "tab row");
            let card = rect(window, "card");
            assert_eq!((card.x, card.y, card.w, card.h), (s.main.x, s.main.y + s.tabs.h, s.main.w, s.main.h - s.tabs.h), "card");
            let open = rect(window, ("tab", 0usize));
            assert_eq!(open.bottom(), card.y + 1.0, "the open tab covers the card's top edge");
            assert_eq!(window.find(("tab", 0usize)).label(), Some("New thread"));

            window.click("tab-add", cx);
            assert!(window.try_find(("tab", 1usize)).is_some(), "+ opens a second tab");
            assert_eq!(rect(window, ("tab", 1usize)).bottom(), card.y + 1.0, "the new tab is the open one");
            assert_eq!(rect(window, ("tab", 0usize)).bottom(), card.y, "the other tab sits on the frame");

            window.click(("tab", 0usize), cx);
            assert_eq!(rect(window, ("tab", 0usize)).bottom(), card.y + 1.0, "click switches");

            window.click(("tab-close", 1usize), cx);
            assert!(window.try_find(("tab", 1usize)).is_none(), "x closes");
            window.click(("tab-close", 0usize), cx);
            assert_eq!(window.find(("tab", 0usize)).label(), Some("New thread"), "closing the last leaves a New thread");
        })
        .unwrap();
    }

    /// Tabs shrink as more open; below 96px they stop shrinking and arrows scroll the row,
    /// as in Explorer. The open tab and + always stay in view.
    #[gpui_kit::test]
    fn many_tabs_scroll_with_arrows(cx: &mut TestAppContext) {
        let handle = cx.add_window(|_, _| Cauldron::new());
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            for _ in 0..3 {
                window.click("tab-add", cx);
            }
            assert!(window.try_find("tab-prev").is_none(), "4 tabs fit, no arrows");
            let w4 = rect(window, ("tab", 0usize)).w;
            for _ in 0..16 {
                window.click("tab-add", cx);
            }
            let row = rect(window, "tabs");
            let visible: Vec<usize> = (0..20).filter(|&i| window.try_find(("tab", i)).is_some()).collect();
            assert!(visible.len() < 20, "20 tabs do not all fit: {visible:?}");
            assert!(visible.contains(&19), "the open tab (the newest) is in view: {visible:?}");
            for &i in &visible {
                let t = rect(window, ("tab", i));
                assert!(t.w >= 96.0 && t.w <= w4, "tab {i} is {t:?}");
                assert!(t.x >= row.x - 0.5 && t.right() <= row.right() + 0.5, "tab {i} stays in the row: {t:?}");
            }
            for id in ["tab-prev", "tab-next", "tab-add"] {
                let r = rect(window, id);
                assert!(r.x >= row.x - 0.5 && r.right() <= row.right() + 0.5, "{id} in the row: {r:?}");
            }
            let first = visible[0];
            window.click("tab-prev", cx);
            assert!(window.try_find(("tab", first - 1)).is_some(), "< shows the tab before");
            window.click("tab-next", cx);
            assert!(window.try_find(("tab", first - 1)).is_none(), "> scrolls back");
        })
        .unwrap();
    }
}
