//! Shell layout solver. Pure geometry: no drawing, no GPUI.
//!
//! Coordinates are logical pixels from the window's top-left corner, the 1px window border
//! included, exactly as `tests/golden/wireframe.json` was measured. [`layout`] is the literal
//! wireframe; [`solve`] first applies the minimum-width rule, which the wireframe does not draw.

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
    /// Traffic lights in the notch, no caption buttons.
    Mac,
    /// Caption buttons top right, no traffic lights. The notch shrinks to hold the sidebar button.
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
    /// The cut at the top-left of the sidebar that holds the lights and the sidebar button.
    pub notch: Option<Rect>,
    pub main: Rect,
    pub tabs: Rect,
    pub half: Option<Rect>,
    pub drawer: Option<Rect>,
    pub rrail: Rect,
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
    pub const MARGIN_TOP: f32 = 8.0;
    pub const MARGIN_BOTTOM: f32 = 12.0;
    pub const GAP: f32 = 8.0;
    pub const RAIL_W: f32 = 44.0;
    pub const SIDEBAR_W: f32 = 236.0;
    pub const RRAIL_W: f32 = 44.0;
    pub const DRAWER_W: f32 = 200.0;
    pub const SPLIT_W: f32 = 292.0;
    pub const TAB_H: f32 = 34.0;
    pub const NOTCH_W: f32 = 126.0;
    pub const NOTCH_W_NO_LIGHTS: f32 = 62.0;
    pub const NOTCH_H: f32 = 52.0;
    pub const SBTN: f32 = 28.0;
    pub const TOPR_W: f32 = 58.0;
    pub const TOPR_H: f32 = 28.0;
    pub const TOPR_Y: f32 = 13.0;
    pub const CAPS_W: f32 = 108.0;
    pub const CAPS_H: f32 = 32.0;
    pub const CAPS_Y: f32 = 11.0;
    pub const CAPS_MARGIN_RIGHT: f32 = 20.0;
    /// Proposed, not drawn in the wireframe.
    pub const MAIN_MIN_W: f32 = 360.0;
}

/// The wireframe's geometry for exactly the state asked for.
pub fn layout(w: f32, h: f32, state: ShellState, platform: Platform) -> Shell {
    use metric::*;
    let inner = Rect::new(BORDER, BORDER, w - 2.0 * BORDER, h - 2.0 * BORDER);
    let top = inner.y + MARGIN_TOP;
    let height = inner.h - MARGIN_TOP - MARGIN_BOTTOM;

    let (rail, sidebar, main_x) = if state.sidebar_open {
        let sb = Rect::new(inner.x + MARGIN_SIDE, top, SIDEBAR_W, height);
        (None, Some(sb), sb.right() + GAP)
    } else {
        (Some(Rect::new(inner.x, inner.y, RAIL_W, inner.h)), None, inner.x + RAIL_W + GAP)
    };

    let rrail = Rect::new(inner.right() - MARGIN_SIDE - RRAIL_W, top, RRAIL_W, height);
    let below_tabs = |x: f32, w: f32| Rect::new(x, top + TAB_H, w, height - TAB_H);

    let mut main_right = rrail.x;
    let drawer = state.drawer_open.then(|| {
        let d = below_tabs(rrail.x - DRAWER_W, DRAWER_W);
        main_right = d.x - GAP;
        d
    });
    let half = state.split_open.then(|| {
        let hf = below_tabs(main_right - SPLIT_W, SPLIT_W);
        main_right = hf.x - GAP;
        hf
    });
    let main = Rect::new(main_x, top, main_right - main_x, height);
    let tabs = Rect::new(main.x, top, main.w, TAB_H);

    let has_lights = platform != Platform::Windows;
    let has_caps = platform != Platform::Mac;
    let lights = has_lights.then(|| Rect::new(inner.x + 14.0, inner.y + 19.0, 52.0, 12.0));
    let sbtn_x = if has_lights { inner.x + 78.0 } else { inner.x + 14.0 };
    let sidebar_button = Rect::new(sbtn_x, inner.y + 12.0, SBTN, SBTN);
    let notch = state.sidebar_open.then(|| {
        Rect::new(inner.x, inner.y, if has_lights { NOTCH_W } else { NOTCH_W_NO_LIGHTS }, NOTCH_H)
    });

    let caps = has_caps.then(|| Rect::new(inner.right() - CAPS_MARGIN_RIGHT - CAPS_W, CAPS_Y, CAPS_W, CAPS_H));
    let topr_right = caps.map_or(inner.right() - CAPS_MARGIN_RIGHT, |c| c.x - GAP);
    let topr = Rect::new(topr_right - TOPR_W, TOPR_Y, TOPR_W, TOPR_H);

    Shell { effective: state, rail, sidebar, notch, main, tabs, half, drawer, rrail, sidebar_button, lights, topr, caps }
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
