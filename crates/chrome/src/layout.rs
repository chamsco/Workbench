//! Shell layout solver. Pure geometry: no drawing, no GPUI.
//!
//! Coordinates are logical pixels from the window's top-left corner. [`layout`] follows the approved
//! Tight tiles board (design/cauldron-tight-tiles/index.html, Charles 15 Nov), as measured into
//! `tests/golden/wireframe.json`: no top bar, the sidebar flat on the frame, the stream card and the
//! support card as tight tiles 4px apart, the terminal docked under the stream, the quick-button
//! rail inside the rightmost card and, on Windows, the caption buttons in a notch cut into that
//! card's top-right. [`solve`] first applies the minimum-width rule.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn right(self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(self) -> f32 {
        self.y + self.h
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// Traffic lights top-left, no notch.
    Mac,
    /// Caption buttons in a notch in the rightmost card's top-right, no traffic lights.
    Windows,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellState {
    pub sidebar_open: bool,
    /// The support card (Browser, Changes, Git, Files, ...) right of the stream.
    pub support_open: bool,
    /// The terminal, docked under the stream (Ctrl+J).
    pub dock_open: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Shell {
    /// What was drawn after the minimum-width rule; equals the request when everything fits.
    pub effective: ShellState,
    /// Flat on the frame, from the window's left edge.
    pub sidebar: Option<Rect>,
    /// The stream card.
    pub main: Rect,
    /// The terminal card under the stream.
    pub dock: Option<Rect>,
    /// The support card right of the stream.
    pub support: Option<Rect>,
    /// The 44px quick-button rail inside the rightmost card's right edge, under its header.
    pub rail: Rect,
    /// Windows: the 100x40 notch cut into the rightmost card's top-right.
    pub notch: Option<Rect>,
    /// Windows: minimise, maximise, close, inside the notch.
    pub caps: Option<Rect>,
    /// macOS: the traffic lights, in the sidebar's top row or the stream header.
    pub lights: Option<Rect>,
    pub sidebar_button: Rect,
    /// macOS with the sidebar folded: New stream, between the sidebar button and the tabs.
    pub new_button: Option<Rect>,
    /// Windows with the sidebar open: the logo and the name in the sidebar's top row.
    pub title: Option<Rect>,
    /// The tab row in the stream header, up to the header's tools on the right.
    pub tabs: Rect,
    /// The stream header's tools on the right: open-in, list, dock toggle, layout.
    pub tools: Rect,
}

/// Measured from the board.
pub mod metric {
    /// The window margin around the cards and the gap between them.
    pub const GAP: f32 = 4.0;
    pub const SIDEBAR_W: f32 = 236.0;
    /// The sidebar can be dragged between these widths; dragged below `SIDEBAR_COLLAPSE` it folds
    /// into the folded state (the hover band).
    pub const SIDEBAR_MIN: f32 = 200.0;
    pub const SIDEBAR_MAX: f32 = 380.0;
    pub const SIDEBAR_COLLAPSE: f32 = 150.0;
    /// The support card's default width; it stays within [`SUPPORT_MIN_W`] and half the window.
    pub const SUPPORT_W: f32 = 470.0;
    pub const SUPPORT_MIN_W: f32 = 360.0;
    pub const DOCK_H: f32 = 200.0;
    /// A card's header, inside its 1px border; tabs live here.
    pub const HEAD_H: f32 = 38.0;
    /// The card's body starts this far under its top edge (the header and the space under it).
    pub const BODY_TOP: f32 = 55.0;
    pub const RAIL_W: f32 = 44.0;
    /// Tabs and header buttons.
    pub const TAB_H: f32 = 28.0;
    pub const SBTN: f32 = 28.0;
    /// Header items start this far inside the card's edge (1px border, 5px padding).
    pub const HEAD_IN: f32 = 6.0;
    /// The header's right padding, inside the border.
    pub const HEAD_PR: f32 = 8.0;
    /// Gap between items in a header.
    pub const HEAD_GAP: f32 = 2.0;
    /// Open-in (54), three buttons (28 each) and their 4px gaps.
    pub const TOOLS_W: f32 = 146.0;
    pub const NOTCH_W: f32 = 100.0;
    pub const NOTCH_H: f32 = 40.0;
    pub const CAPS_W: f32 = 84.0;
    pub const CAPS_H: f32 = 28.0;
    pub const LIGHTS_W: f32 = 70.0;
    pub const LIGHTS_H: f32 = 12.0;
    /// The stream column's minimum width; the support card closes before it gets narrower.
    pub const MAIN_MIN_W: f32 = 360.0;
}

/// The support card's width in a window `w` wide.
pub fn support_width(w: f32) -> f32 {
    metric::SUPPORT_W.min(w / 2.0).max(metric::SUPPORT_MIN_W)
}

/// A dragged sidebar width, held to its range.
pub fn clamp_sidebar(w: f32) -> f32 {
    w.clamp(metric::SIDEBAR_MIN, metric::SIDEBAR_MAX)
}

/// The window's geometry for exactly the state asked for, at the default sidebar width.
pub fn layout(w: f32, h: f32, state: ShellState, platform: Platform) -> Shell {
    layout_at(w, h, state, platform, metric::SIDEBAR_W)
}

/// [`layout`] with the sidebar `sidebar_w` wide (the user dragged it).
pub fn layout_at(w: f32, h: f32, state: ShellState, platform: Platform, sidebar_w: f32) -> Shell {
    use metric::*;
    let (top, bottom, right) = (GAP, h - GAP, w - GAP);
    let full_h = bottom - top;

    let sidebar = state.sidebar_open.then(|| Rect::new(0.0, top, sidebar_w, full_h));
    let left = sidebar.map_or(GAP, |s| s.right() + GAP);
    let support = state.support_open.then(|| {
        let sw = support_width(w);
        Rect::new(right - sw, top, sw, full_h)
    });
    let col_right = support.map_or(right, |s| s.x - GAP);
    let col = Rect::new(left, top, col_right - left, full_h);
    let (main, dock) = if state.dock_open {
        let d = Rect::new(col.x, bottom - DOCK_H, col.w, DOCK_H);
        (Rect::new(col.x, top, col.w, d.y - GAP - top), Some(d))
    } else {
        (col, None)
    };

    let host = support.unwrap_or(main);
    let rail = Rect::new(host.right() - 1.0 - RAIL_W, host.y + BODY_TOP, RAIL_W, host.h - BODY_TOP - 1.0);
    let windows = platform == Platform::Windows;
    let notch = windows.then(|| Rect::new(host.right() - NOTCH_W, host.y, NOTCH_W, NOTCH_H));
    let caps = notch.map(|n| Rect::new(n.x + 7.0, n.y + 6.0, CAPS_W, CAPS_H));

    // The header row: 6px in from the card's top and left.
    let row_y = main.y + HEAD_IN;
    let (lights, sidebar_button, new_button, title, tabs_x) = match (sidebar, platform) {
        // Open: the sidebar's top row holds the lights or the logo, then search and the button.
        (Some(sb), _) => {
            let lights = (!windows).then(|| Rect::new(sb.x + 12.0, sb.y + 17.0, LIGHTS_W, LIGHTS_H));
            let title = windows.then(|| Rect::new(sb.x + 16.0, sb.y + 9.0, 96.0, SBTN));
            (lights, Rect::new(sb.right() - 40.0, sb.y + 9.0, SBTN, SBTN), None, title, main.x + HEAD_IN)
        }
        // macOS folded: lights, sidebar button, New stream, tabs.
        (None, Platform::Mac) => {
            let lights = Rect::new(main.x + HEAD_IN, main.y + 14.0, LIGHTS_W, LIGHTS_H);
            let sbtn = Rect::new(lights.right() + 2.0, row_y, SBTN, SBTN);
            let newb = Rect::new(sbtn.right() + HEAD_GAP, row_y, SBTN, SBTN);
            (Some(lights), sbtn, Some(newb), None, newb.right() + HEAD_GAP)
        }
        // Windows folded (Charles, 15 Nov): the sidebar button in the stream card, left of the
        // tabs, with a separator between.
        (None, Platform::Windows) => {
            let sbtn = Rect::new(main.x + HEAD_IN, row_y, SBTN, SBTN);
            (None, sbtn, None, None, sbtn.right() + 15.0)
        }
    };
    let notch_room = if notch.is_some() && support.is_none() { NOTCH_W + HEAD_GAP } else { 0.0 };
    let tools = Rect::new(main.right() - 1.0 - HEAD_PR - notch_room - TOOLS_W, row_y, TOOLS_W, TAB_H);
    let tabs = Rect::new(tabs_x, row_y, (tools.x - HEAD_GAP - tabs_x).max(0.0), TAB_H);

    Shell { effective: state, sidebar, main, dock, support, rail, notch, caps, lights, sidebar_button, new_button, title, tabs, tools }
}

/// The state that actually fits: the stream column keeps [`metric::MAIN_MIN_W`], so the support
/// card closes. The sidebar is the user's choice and stays; the dock takes no width.
pub fn resolve(w: f32, h: f32, state: ShellState, platform: Platform) -> ShellState {
    resolve_at(w, h, state, platform, metric::SIDEBAR_W)
}

pub fn resolve_at(w: f32, h: f32, mut state: ShellState, platform: Platform, sidebar_w: f32) -> ShellState {
    if state.support_open && layout_at(w, h, state, platform, sidebar_w).main.w < metric::MAIN_MIN_W {
        state.support_open = false;
    }
    state
}

pub fn solve(w: f32, h: f32, state: ShellState, platform: Platform) -> Shell {
    solve_at(w, h, state, platform, metric::SIDEBAR_W)
}

pub fn solve_at(w: f32, h: f32, state: ShellState, platform: Platform, sidebar_w: f32) -> Shell {
    layout_at(w, h, resolve_at(w, h, state, platform, sidebar_w), platform, sidebar_w)
}

#[cfg(test)]
mod sidebar_width_tests {
    use super::*;

    fn open() -> ShellState {
        ShellState { sidebar_open: true, support_open: false, dock_open: false }
    }

    /// A dragged sidebar moves the card's left edge and keeps the sidebar button on its right edge;
    /// the default width is exactly the old layout.
    #[test]
    fn the_sidebar_width_moves_the_card_and_the_button() {
        let a = solve(1240.0, 702.0, open(), Platform::Windows);
        let b = solve_at(1240.0, 702.0, open(), Platform::Windows, 300.0);
        assert_eq!(a.sidebar.unwrap().w, metric::SIDEBAR_W);
        assert_eq!(b.sidebar.unwrap().w, 300.0);
        assert_eq!(b.main.x - a.main.x, 300.0 - metric::SIDEBAR_W);
        assert_eq!(a.sidebar_button.x, a.sidebar.unwrap().x + 196.0, "unchanged at the default width");
        assert_eq!(b.sidebar_button.right(), b.sidebar.unwrap().right() - 12.0, "the button keeps 12px from the sidebar's right edge");
    }

    #[test]
    fn dragged_widths_are_held_to_the_range() {
        assert_eq!(clamp_sidebar(100.0), metric::SIDEBAR_MIN);
        assert_eq!(clamp_sidebar(900.0), metric::SIDEBAR_MAX);
        assert_eq!(clamp_sidebar(260.0), 260.0);
        assert!(metric::SIDEBAR_COLLAPSE < metric::SIDEBAR_MIN);
    }
}
