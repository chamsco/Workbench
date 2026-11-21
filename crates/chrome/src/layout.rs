//! Shell layout solver. Pure geometry: no drawing, no GPUI.
//!
//! Coordinates are logical pixels from the window's top-left corner. [`layout`] follows the approved
//! Tight tiles board (design/cauldron-tight-tiles/index.html, Charles 15 Nov), as measured into
//! `tests/golden/wireframe.json`: no top bar, the sidebar flat on the frame, the stream card and the
//! support card as tight tiles 4px apart, the terminal docked under the stream, the quick-button
//! rail inside the rightmost card. On Windows a slim strip runs across the top of the window on
//! the frame (Charles, 20 Nov, option B): sidebar button, search, inbox, "N needs you" and the
//! caption buttons, with the tabs and tools staying in the stream card's header. The old
//! notch is an Appearance option ([`Platform::WindowsNotch`]): no strip, the caption buttons in
//! a notch cut into the rightmost card's top-right. macOS has neither.
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
    /// A slim strip across the top of the window holds the caption buttons, no traffic lights.
    Windows,
    /// Caption buttons in a notch in the rightmost card's top-right, no strip (the option).
    WindowsNotch,
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
    /// Windows: the top strip across the window, on the frame.
    pub bar: Option<Rect>,
    /// Windows with the notch option: the 100x40 notch cut into the rightmost card's top-right.
    pub notch: Option<Rect>,
    /// Windows strip: "N needs you", at the strip's right, left of the caption buttons.
    pub pill: Option<Rect>,
    /// Windows: minimise, maximise, close (at the strip's right end, or inside the notch).
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
    /// Windows: the top strip's height, and the caption buttons (three 46px cells) in it.
    pub const BAR_H: f32 = 34.0;
    pub const CAP_W: f32 = 46.0;
    pub const CAPS_W: f32 = 138.0;
    pub const CAPS_H: f32 = 34.0;
    /// The notch option: 100x40 cut into the card, the buttons 84x28 inside it.
    pub const NOTCH_W: f32 = 100.0;
    pub const NOTCH_H: f32 = 40.0;
    pub const NOTCH_CAPS_W: f32 = 84.0;
    pub const NOTCH_CAPS_H: f32 = 28.0;
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
    let strip = platform == Platform::Windows;
    let windows = strip || platform == Platform::WindowsNotch;
    let (top, bottom, right) = (if strip { BAR_H } else { GAP }, h - GAP, w - GAP);
    let full_h = bottom - top;

    let sidebar = state.sidebar_open.then(|| Rect::new(0.0, top, SIDEBAR_W, full_h));
    let left = sidebar.map_or(GAP, |s| s.right() + GAP);
    let support = state.support_open.then(|| {
        let sw = support_width(w);
        Rect::new(right - sw, top, sw, full_h)
    });
    let col_right = support.map_or(right, |s| s.x - GAP);
    // Windows strip: the stream card's own tab rises into the strip (Charles, 20 Nov: "we don't
    // need that gap at the top"), so the stream column starts at the window's top gap; the sidebar
    // and the support card stay under the strip, which keeps its buttons over them.
    let col_top = if strip { GAP } else { top };
    let col = Rect::new(left, col_top, col_right - left, bottom - col_top);
    let (main, dock) = if state.dock_open {
        let d = Rect::new(col.x, bottom - DOCK_H, col.w, DOCK_H);
        (Rect::new(col.x, col.y, col.w, d.y - GAP - col.y), Some(d))
    } else {
        (col, None)
    };

    let host = support.unwrap_or(main);
    let rail = Rect::new(host.right() - 1.0 - RAIL_W, host.y + BODY_TOP, RAIL_W, host.h - BODY_TOP - 1.0);
    let bar = strip.then(|| Rect::new(0.0, 0.0, w, BAR_H));
    let notch = (platform == Platform::WindowsNotch).then(|| Rect::new(host.right() - NOTCH_W, host.y, NOTCH_W, NOTCH_H));
    let caps = if strip {
        // With the stream card risen into the strip, the buttons sit down on its top line
        // (Charles, 21 Nov); over a support column they stay on the strip.
        let y = if support.is_none() { main.y } else { 0.0 };
        Some(Rect::new(w - CAPS_W, y, CAPS_W, CAPS_H))
    } else {
        notch.map(|n| Rect::new(n.x + 7.0, n.y + 6.0, NOTCH_CAPS_W, NOTCH_CAPS_H))
    };

    // The header row: 6px in from the card's top and left.
    let row_y = main.y + HEAD_IN;
    let (lights, sidebar_button, new_button, title, tabs_x) = match (sidebar, platform) {
        // The strip's left end holds the sidebar button, search and inbox, open or folded.
        // With the sidebar folded the stream card reaches the strip's buttons, so its tabs
        // start after them (search and inbox end at 110).
        (_, Platform::Windows) => {
            let sbtn = Rect::new(10.0, (BAR_H - SBTN) / 2.0, SBTN, SBTN);
            let after = if sidebar.is_none() { (sbtn.x + 72.0 + SBTN + 10.0).max(main.x + HEAD_IN) } else { main.x + HEAD_IN };
            (None, sbtn, None, None, after)
        }
        // Open: the sidebar's top row holds the lights or the logo, then search and the button.
        (Some(sb), _) => {
            let lights = (!windows).then(|| Rect::new(sb.x + 12.0, sb.y + 17.0, LIGHTS_W, LIGHTS_H));
            let title = windows.then(|| Rect::new(sb.x + 16.0, sb.y + 9.0, 96.0, SBTN));
            (lights, Rect::new(sb.x + 196.0, sb.y + 9.0, SBTN, SBTN), None, title, main.x + HEAD_IN)
        }
        // macOS folded: lights, sidebar button, New stream, tabs.
        (None, Platform::Mac) => {
            let lights = Rect::new(main.x + HEAD_IN, main.y + 14.0, LIGHTS_W, LIGHTS_H);
            let sbtn = Rect::new(lights.right() + 2.0, row_y, SBTN, SBTN);
            let newb = Rect::new(sbtn.right() + HEAD_GAP, row_y, SBTN, SBTN);
            (Some(lights), sbtn, Some(newb), None, newb.right() + HEAD_GAP)
        }
        // Notch, folded: the sidebar button in the stream card, left of the tabs, with a rule between.
        (None, _) => {
            let sbtn = Rect::new(main.x + HEAD_IN, row_y, SBTN, SBTN);
            (None, sbtn, None, None, sbtn.right() + 15.0)
        }
    };
    let (search_button, bell_button) = if strip {
        (Rect::new(sidebar_button.x + 36.0, sidebar_button.y, SBTN, SBTN), Rect::new(sidebar_button.x + 72.0, sidebar_button.y, SBTN, SBTN))
    } else {
        (Rect::new(sidebar_button.x - 36.0, sidebar_button.y, SBTN, SBTN), Rect::new(sidebar_button.x - 72.0, sidebar_button.y, SBTN, SBTN))
    };
    let notch_room = if notch.is_some() && support.is_none() { NOTCH_W + HEAD_GAP } else { 0.0 };
    let (tools, pill, tabs) = if strip {
        // The tools sit inside the stream's content well, top right (left of the rail when the
        // rail is in this card); the tabs run up to the caption buttons, which now sit on the
        // frame right of the card's tab. No "needs you" pill (the sidebar has its own group).
        let rail_room = if support.is_none() { RAIL_W + HEAD_GAP } else { 0.0 };
        let tools = Rect::new(main.right() - 1.0 - HEAD_PR - rail_room - TOOLS_W, main.y + HEAD_IN + TAB_H + HEAD_IN + HEAD_PR, TOOLS_W, TAB_H);
        let end = if support.is_none() { (w - CAPS_W - HEAD_PR).min(main.right() - 1.0 - HEAD_PR) } else { main.right() - 1.0 - HEAD_PR };
        (tools, None, Rect::new(tabs_x, row_y, (end - tabs_x).max(0.0), TAB_H))
    } else {
        let tools = Rect::new(main.right() - 1.0 - HEAD_PR - notch_room - TOOLS_W, row_y, TOOLS_W, TAB_H);
        (tools, None, Rect::new(tabs_x, row_y, (tools.x - HEAD_GAP - tabs_x).max(0.0), TAB_H))
    };

    Shell { effective: state, sidebar, main, dock, support, rail, bar, notch, pill, caps, search_button, bell_button, lights, sidebar_button, new_button, title, tabs, tools }
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
