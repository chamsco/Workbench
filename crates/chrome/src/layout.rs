//! Shell layout solver. Pure geometry: no drawing, no GPUI.
//!
//! Coordinates are logical pixels from the window's top-left corner. [`layout`] follows the approved
//! Tight tiles board (design/cauldron-tight-tiles/index.html, Charles 15 Nov), as measured into
//! `tests/golden/wireframe.json`: no top bar, the sidebar flat on the frame, the stream card and the
//! support card as tight tiles 4px apart, the terminal docked under the stream, the quick-button
//! rail inside the rightmost card. On Windows (Charles, 20 Nov: the notch is out) a 38px top bar
//! runs across the window on the frame: sidebar button, search and inbox, the tabs, "N needs you",
//! the tools and the caption buttons, with the cards starting under it. macOS keeps no bar.
//! [`solve`] first applies the minimum-width rule.

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
    /// A 38px top bar across the window holds the caption buttons, no traffic lights.
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
    /// Windows: the top bar across the window, on the frame.
    pub bar: Option<Rect>,
    /// Windows: "N needs you", in the bar left of the tools.
    pub pill: Option<Rect>,
    /// Windows: minimise, maximise, close, at the bar's right end.
    pub caps: Option<Rect>,
    /// The sidebar's search and inbox buttons: beside the sidebar button (left of it in the
    /// sidebar's top row; right of it in the Windows bar).
    pub search_button: Rect,
    pub bell_button: Rect,
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
    /// Windows: the top bar's height, and the caption buttons (three 46px cells) in it.
    pub const BAR_H: f32 = 38.0;
    pub const CAP_W: f32 = 46.0;
    pub const CAPS_W: f32 = 138.0;
    pub const CAPS_H: f32 = 38.0;
    /// Windows: the "N needs you" pill in the bar.
    pub const PILL_W: f32 = 100.0;
    pub const PILL_H: f32 = 24.0;
    pub const LIGHTS_W: f32 = 70.0;
    pub const LIGHTS_H: f32 = 12.0;
    /// The stream column's minimum width; the support card closes before it gets narrower.
    pub const MAIN_MIN_W: f32 = 360.0;
}

/// The support card's width in a window `w` wide.
pub fn support_width(w: f32) -> f32 {
    metric::SUPPORT_W.min(w / 2.0).max(metric::SUPPORT_MIN_W)
}

/// The window's geometry for exactly the state asked for.
pub fn layout(w: f32, h: f32, state: ShellState, platform: Platform) -> Shell {
    use metric::*;
    let windows = platform == Platform::Windows;
    let (top, bottom, right) = (if windows { BAR_H } else { GAP }, h - GAP, w - GAP);
    let full_h = bottom - top;

    let sidebar = state.sidebar_open.then(|| Rect::new(0.0, top, SIDEBAR_W, full_h));
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
    let bar = windows.then(|| Rect::new(0.0, 0.0, w, BAR_H));
    let caps = windows.then(|| Rect::new(w - CAPS_W, 0.0, CAPS_W, CAPS_H));

    // The header row: 6px in from the card's top and left; on Windows it is the bar's row.
    let row_y = if windows { (BAR_H - TAB_H) / 2.0 } else { main.y + HEAD_IN };
    let (lights, sidebar_button, new_button, title, tabs_x) = match (sidebar, platform) {
        // Windows: the bar's left end holds the sidebar button, search and inbox; the tabs start
        // at the stream card's left edge, or after the inbox when the sidebar is folded.
        (sb, Platform::Windows) => {
            let sbtn = Rect::new(10.0, row_y, SBTN, SBTN);
            let after = sbtn.x + 3.0 * 36.0 + 8.0;
            let _ = sb;
            (None, sbtn, None, None, (main.x + HEAD_IN).max(after))
        }
        // Open: the sidebar's top row holds the lights, then search and the button.
        (Some(sb), _) => {
            let lights = Rect::new(sb.x + 12.0, sb.y + 17.0, LIGHTS_W, LIGHTS_H);
            (Some(lights), Rect::new(sb.x + 196.0, sb.y + 9.0, SBTN, SBTN), None, None, main.x + HEAD_IN)
        }
        // macOS folded: lights, sidebar button, New stream, tabs.
        (None, _) => {
            let lights = Rect::new(main.x + HEAD_IN, main.y + 14.0, LIGHTS_W, LIGHTS_H);
            let sbtn = Rect::new(lights.right() + 2.0, row_y, SBTN, SBTN);
            let newb = Rect::new(sbtn.right() + HEAD_GAP, row_y, SBTN, SBTN);
            (Some(lights), sbtn, Some(newb), None, newb.right() + HEAD_GAP)
        }
    };
    let (search_button, bell_button) = if windows {
        (Rect::new(sidebar_button.x + 36.0, sidebar_button.y, SBTN, SBTN), Rect::new(sidebar_button.x + 72.0, sidebar_button.y, SBTN, SBTN))
    } else {
        (Rect::new(sidebar_button.x - 36.0, sidebar_button.y, SBTN, SBTN), Rect::new(sidebar_button.x - 72.0, sidebar_button.y, SBTN, SBTN))
    };
    let tools_right = if windows { w - CAPS_W - HEAD_PR } else { main.right() - 1.0 - HEAD_PR };
    let tools = Rect::new(tools_right - TOOLS_W, row_y, TOOLS_W, TAB_H);
    let pill = windows.then(|| Rect::new(tools.x - HEAD_GAP - 4.0 - PILL_W, (BAR_H - PILL_H) / 2.0, PILL_W, PILL_H));
    let tabs_end = pill.map_or(tools.x, |p| p.x);
    let tabs = Rect::new(tabs_x, row_y, (tabs_end - HEAD_GAP - tabs_x).max(0.0), TAB_H);

    Shell { effective: state, sidebar, main, dock, support, rail, bar, pill, caps, search_button, bell_button, lights, sidebar_button, new_button, title, tabs, tools }
}

/// The state that actually fits: the stream column keeps [`metric::MAIN_MIN_W`], so the support
/// card closes. The sidebar is the user's choice and stays; the dock takes no width.
pub fn resolve(w: f32, h: f32, mut state: ShellState, platform: Platform) -> ShellState {
    if state.support_open && layout(w, h, state, platform).main.w < metric::MAIN_MIN_W {
        state.support_open = false;
    }
    state
}

pub fn solve(w: f32, h: f32, state: ShellState, platform: Platform) -> Shell {
    layout(w, h, resolve(w, h, state, platform), platform)
}
