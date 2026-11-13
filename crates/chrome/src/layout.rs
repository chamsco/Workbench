//! Shell layout solver. Pure geometry: no drawing, no GPUI.
//!
//! Coordinates are logical pixels from the window's top-left corner, the 1px window border
//! included, as `tests/golden/wireframe.json` was measured. [`layout`] started as the wireframe and
//! now follows the v3 board (design/cauldron-new-look-v3.html): the top bar, floating tabs and the
//! drawer in the right gutter. [`solve`] first applies the minimum-width rule.

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
    /// Traffic lights in the top bar, no caption buttons.
    Mac,
    /// Caption buttons top right, no traffic lights.
    Windows,
    /// The wireframe window, which draws both so the review page shows each control once.
    Reference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellState {
    pub sidebar_open: bool,
    pub drawer_open: bool,
    pub split_open: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Shell {
    /// What was drawn after the minimum-width rule; equals the request when everything fits.
    pub effective: ShellState,
    /// The 44px function rail, shown only while the sidebar is hidden.
    pub rail: Option<Rect>,
    pub sidebar: Option<Rect>,
    pub bar: Rect,
    pub main: Rect,
    pub tabs: Rect,
    pub half: Option<Rect>,
    pub drawer: Option<Rect>,
    /// The 44px panel rail, shown only while the drawer is folded (the open drawer carries the
    /// panels as a strip on top).
    pub rrail: Option<Rect>,
    pub sidebar_button: Rect,
    pub lights: Option<Rect>,
    /// Terminal and Split.
    pub topr: Rect,
    pub caps: Option<Rect>,
}

/// Measured from the wireframe.
pub mod metric {
    pub const BORDER: f32 = 1.0;
    pub const MARGIN_SIDE: f32 = 12.0;
    pub const MARGIN_BOTTOM: f32 = 12.0;
    pub const GAP: f32 = 8.0;
    pub const RAIL_W: f32 = 44.0;
    pub const SIDEBAR_W: f32 = 236.0;
    pub const RRAIL_W: f32 = 44.0;
    /// The drawer in the right gutter: as wide as the sidebar (v3 board, Charles 13 Nov).
    pub const DRAWER_W: f32 = SIDEBAR_W;
    pub const SPLIT_W: f32 = 292.0;
    /// The top bar, full width: the board's 44, the height of the card's head, so the two lines
    /// match. The card and the side panes start right under it.
    pub const BAR_H: f32 = 44.0;
    /// A floating tab in the top bar: control-sm, a little smaller than v2's (Charles, 13 Nov).
    pub const TAB_H: f32 = 24.0;
    /// The logo and the name after the sidebar button; the tabs never start before its end.
    pub const TITLE_W: f32 = 96.0;
    pub const SBTN: f32 = 28.0;
    pub const TOPR_W: f32 = 58.0;
    pub const TOPR_H: f32 = 28.0;
    pub const CAPS_W: f32 = 108.0;
    pub const CAPS_H: f32 = 32.0;
    pub const CAPS_MARGIN_RIGHT: f32 = 20.0;
    /// Proposed, not drawn in the wireframe.
    pub const MAIN_MIN_W: f32 = 360.0;
}

/// The window's geometry for exactly the state asked for. The top bar runs the full width; the
/// sidebar or rail, the main card and the right rail all start under it.
pub fn layout(w: f32, h: f32, state: ShellState, platform: Platform) -> Shell {
    use metric::*;
    let inner = Rect::new(BORDER, BORDER, w - 2.0 * BORDER, h - 2.0 * BORDER);
    let bar = Rect::new(inner.x, inner.y, inner.w, BAR_H);
    let top = inner.y + BAR_H;
    let height = inner.bottom() - top - MARGIN_BOTTOM;

    let (rail, sidebar, main_x) = if state.sidebar_open {
        let sb = Rect::new(inner.x + MARGIN_SIDE, top, SIDEBAR_W, height);
        (None, Some(sb), sb.right() + GAP)
    } else {
        (Some(Rect::new(inner.x, top, RAIL_W, height)), None, inner.x + RAIL_W + GAP)
    };

    // The right gutter holds the open drawer, or the rail while it is folded.
    let right = inner.right() - MARGIN_SIDE;
    let (drawer, rrail) = if state.drawer_open {
        (Some(Rect::new(right - DRAWER_W, top, DRAWER_W, height)), None)
    } else {
        (None, Some(Rect::new(right - RRAIL_W, top, RRAIL_W, height)))
    };
    let mut main_right = drawer.or(rrail).map_or(right, |r| r.x) - GAP;
    let half = state.split_open.then(|| {
        let hf = Rect::new(main_right - SPLIT_W, top, SPLIT_W, height);
        main_right = hf.x - GAP;
        hf
    });
    let main = Rect::new(main_x, top, main_right - main_x, height);

    let has_lights = platform != Platform::Windows;
    let has_caps = platform != Platform::Mac;
    let lights = has_lights.then(|| Rect::new(inner.x + 14.0, inner.y + (BAR_H - 12.0) / 2.0, 52.0, 12.0));
    let sbtn_x = if has_lights { inner.x + 78.0 } else { inner.x + 14.0 };
    let sidebar_button = Rect::new(sbtn_x, inner.y + (BAR_H - SBTN) / 2.0, SBTN, SBTN);

    let caps = has_caps.then(|| Rect::new(inner.right() - CAPS_MARGIN_RIGHT - CAPS_W, inner.y + (BAR_H - CAPS_H) / 2.0, CAPS_W, CAPS_H));
    let topr_right = caps.map_or(inner.right() - CAPS_MARGIN_RIGHT, |c| c.x - GAP);
    let topr = Rect::new(topr_right - TOPR_W, inner.y + (BAR_H - TOPR_H) / 2.0, TOPR_W, TOPR_H);

    // The tabs float in the bar from the card's left edge, clear of the logo and name, up to
    // Terminal and Split.
    let tabs_x = main.x.max(sidebar_button.right() + GAP + TITLE_W);
    let tabs = Rect::new(tabs_x, inner.y + (BAR_H - TAB_H) / 2.0, (topr.x - GAP - tabs_x).max(0.0), TAB_H);

    Shell { effective: state, rail, sidebar, bar, main, tabs, half, drawer, rrail, sidebar_button, lights, topr, caps }
}

/// The state that actually fits: the thread column keeps [`metric::MAIN_MIN_W`], so Split closes
/// first, then the drawer. The sidebar is the user's choice and stays.
pub fn resolve(w: f32, h: f32, mut state: ShellState, platform: Platform) -> ShellState {
    while layout(w, h, state, platform).main.w < metric::MAIN_MIN_W {
        if state.split_open {
            state.split_open = false;
        } else if state.drawer_open {
            state.drawer_open = false;
        } else {
            break;
        }
    }
    state
}

pub fn solve(w: f32, h: f32, state: ShellState, platform: Platform) -> Shell {
    layout(w, h, resolve(w, h, state, platform), platform)
}
