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

const TOP_PANELS: [Panel; 4] = [Panel::Highlights, Panel::Usage, Panel::Koi, Panel::Review];

pub(crate) struct Cauldron {
    sidebar_open: bool,
    drawer: Option<Panel>,
    split: bool,
}

impl Cauldron {
    pub(crate) fn new() -> Self {
        Cauldron { sidebar_open: true, drawer: Some(Panel::Highlights), split: false }
    }

    fn toggle_panel(&mut self, p: Panel) {
        self.drawer = if self.drawer == Some(p) { None } else { Some(p) };
    }
}

/// A square icon button, 28px, the wireframe's `.fn`, `.ib` and `.rb`.
fn icon_button(id: impl Into<ElementId>, name: &str, label: &'static str, pal: &Palette, on: bool) -> Stateful<Div> {
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
        .tooltip_text(label)
}

trait TooltipText: Sized {
    fn tooltip_text(self, label: &'static str) -> Self;
}

impl TooltipText for Stateful<Div> {
    // Real tooltips land with UI1b; the label is kept so the accessible name is not lost.
    fn tooltip_text(self, _label: &'static str) -> Self {
        self
    }
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
            let mut col = abs(r).flex().flex_col().items_center().gap(px(4.)).pt(px(metric::NOTCH_H));
            for (i, (name, label)) in FUNCTIONS.iter().enumerate() {
                col = col.child(icon_button(("rail", i), name, label, pal, i == 0));
            }
            root = root.child(col);
        }

        // ---- sidebar card with the function row
        if let Some(r) = s.sidebar {
            let mut fnrow = div().flex().justify_between().px(px(2.)).pt(px(12.)).pb(px(8.));
            for (i, (name, label)) in FUNCTIONS.iter().enumerate() {
                fnrow = fnrow.child(icon_button(("fn", i), name, label, pal, i == 0));
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
            let card_el = abs(r)
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
                        .child(icon_button("settings", "settings", "Settings", pal, false)),
                );
            root = root.child(card_el);
        }

        // ---- main card
        let mut tabs = div()
            .h(px(metric::TAB_H))
            .flex_none()
            .flex()
            .items_center()
            .px(px(8.))
            .gap(px(8.))
            .text_size(px(13.))
            .child(glyph("plus", 14., fg2))
            .child(div().text_color(fg).child("New thread"));
        tabs = tabs.border_b_1().border_color(line);
        let main_card = abs(s.main)
            .flex()
            .flex_col()
            .rounded(px(radius::R))
            .bg(card)
            .border_1()
            .border_color(ring)
            .overflow_hidden()
            .child(tabs)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(fg3)
                    .child("The thread view arrives in UI4."),
            );
        root = root.child(main_card);

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
                abs(r)
                    .rounded(px(radius::R))
                    .bg(card)
                    .border_1()
                    .border_color(ring)
                    .overflow_hidden()
                    .child(div().px(px(12.)).pt(px(10.)).text_color(fg3).text_size(px(12.)).child("Open in this half"))
                    .child(grid),
            );
        }

        // ---- drawer (frame colour, no border on the shared edge)
        if let (Some(r), Some(p)) = (s.drawer, self.drawer) {
            root = root.child(
                abs(r)
                    .flex()
                    .flex_col()
                    .px(px(10.))
                    .pt(px(8.))
                    .gap(px(6.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(p.title()))
                    .child(div().text_size(px(12.)).text_color(fg3).child(format!("This panel arrives in {}.", p.milestone()))),
            );
        }

        // ---- right rail
        let mut rr = abs(s.rrail).flex().flex_col().items_center().gap(px(4.)).pt(px(metric::TAB_H + 4.0));
        for (i, p) in TOP_PANELS.iter().enumerate() {
            let p = *p;
            rr = rr.child(
                icon_button(("rb", i), p.icon(), p.title(), pal, self.drawer == Some(p))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_panel(p);
                        cx.notify();
                    })),
            );
        }
        rr = rr.child(div().flex_1()).child(
            icon_button("rb-patches", Panel::Patches.icon(), Panel::Patches.title(), pal, self.drawer == Some(Panel::Patches))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_panel(Panel::Patches);
                    cx.notify();
                })),
        );
        root = root.child(rr.pb(px(8.)));

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
            abs(s.sidebar_button).child(
                icon_button("sbtn", "panel-left", "Hide sidebar", pal, false).on_click(cx.listener(|this, _, _, cx| {
                    this.sidebar_open = !this.sidebar_open;
                    cx.notify();
                })),
            ),
        );

        // ---- Terminal and Split, left of the caption buttons
        root = root.child(
            abs(s.topr)
                .flex()
                .items_center()
                .gap(px(2.))
                .child(icon_button("top-term", "terminal", "Terminal", pal, false))
                .child(
                    icon_button("top-split", "columns-2", "Split", pal, self.split).on_click(cx.listener(|this, _, _, cx| {
                        this.split = !this.split;
                        cx.notify();
                    })),
                ),
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
                    .hover(move |d| d.bg(if danger { bad } else { ca(pal.fg, 0.08) }))
                    .child(glyph(name, 14., fg2))
            };
            root = root.child(
                abs(r)
                    .flex()
                    .child(cell("cap-min", "minus", WindowControlArea::Min, false))
                    .child(cell("cap-max", "square", WindowControlArea::Max, false))
                    .child(cell("cap-close", "x", WindowControlArea::Close, true)),
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
