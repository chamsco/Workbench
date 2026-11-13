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
    /// The terminal card in the right gutter, in the drawer's place.
    pub terminal_open: bool,
    /// The right gutter's plugins (drawer, its rail, the terminal): coding screens only. Settings and
    /// the other non-coding pages have none, and the card takes the room.
    pub plugins: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Shell {
    /// What was drawn after the minimum-width rule; equals the request when everything fits.
    pub effective: ShellState,
    /// The 44px function rail, shown only while the sidebar is hidden.
    pub rail: Option<Rect>,
    pub sidebar: Option<Rect>,
    pub bar: Rect,
    /// The grey Shell (`card`, radius SHELL) right of the sidebar: the main card, Split and the
    /// drawer or its rail sit inside it, SHELL_PAD in, so the drawer stands in its right gutter
    /// and the new thread's bar in its bottom one.
    pub shell: Rect,
    pub main: Rect,
    pub half: Option<Rect>,
    pub drawer: Option<Rect>,
    /// The terminal card (v3 frame 5): in the right gutter, the drawer folded to its rail beside it.
    pub terminal: Option<Rect>,
    /// The 44px panel rail, shown only while the drawer is folded (the open drawer carries the
    /// panels as a strip on top).
    pub rrail: Option<Rect>,
    pub sidebar_button: Rect,
    /// Global search, immediately right of the sidebar button while the sidebar is open (Charles,
    /// 13 Nov 19:29Z: "at the top left next to the sidebar retract button"). Folded, it is on top of
    /// the rail instead.
    pub search_button: Option<Rect>,
    pub lights: Option<Rect>,
    /// Windows' caption buttons, inside the Shell's top right.
    pub caps: Option<Rect>,
}

/// Measured from the wireframe.
pub mod metric {
    pub const BORDER: f32 = 1.0;
    pub const MARGIN_SIDE: f32 = 12.0;
    /// From the window's edge to the Shell, top and bottom (the card sits 12px in).
    pub const MARGIN_TOP: f32 = 6.0;
    pub const MARGIN_BOTTOM: f32 = 6.0;
    pub const GAP: f32 = 8.0;
    pub const RAIL_W: f32 = 44.0;
    pub const SIDEBAR_W: f32 = 236.0;
    pub const RRAIL_W: f32 = 44.0;
    /// The drawer in the right gutter: 200, UI-PLAN's width (Charles, 13 Nov 18:49Z: at the
    /// sidebar's width it read too big).
    pub const DRAWER_W: f32 = 200.0;
    pub const SPLIT_W: f32 = 292.0;
    /// The terminal card, the v3 board's `.term`.
    pub const TERM_W: f32 = 430.0;
    /// The top bar, full width: the board's 44, the height of the card's head, so the two lines
    /// match. The card and the side panes start right under it.
    pub const BAR_H: f32 = 44.0;
    /// A floating tab in the top bar: control-sm, a little smaller than v2's (Charles, 13 Nov).
    pub const TAB_H: f32 = 24.0;
    /// The logo and the name after the sidebar button; the tabs never start before its end.
    pub const TITLE_W: f32 = 96.0;
    pub const SBTN: f32 = 28.0;
    /// Between the sidebar button and the search button.
    pub const SEARCH_GAP: f32 = 2.0;
    /// Windows' caption buttons: three of 40 x 24, in a 31px band at the Shell's top right that also
    /// carries the panes' tab strips.
    pub const CAPS_W: f32 = 120.0;
    pub const CAPS_H: f32 = 24.0;
    pub const CAPS_BAND: f32 = 31.0;
    /// Proposed, not drawn in the wireframe.
    pub const MAIN_MIN_W: f32 = 360.0;
    /// The Shell's inside padding: the card's radius nests in the Shell's (22 = 16 + 6).
    pub const PAD: f32 = crate::tokens::radius::SHELL_PAD;
}

/// The window's geometry for exactly the state asked for. There is no top bar over the content
/// (Charles, 13 Nov 06:52Z): a 44px row over the sidebar holds the lights, the sidebar button and
/// the name; the Shell runs from 6px under the window's top to 6px over its bottom (under that row
/// while the sidebar is folded, so it never meets the name). On Windows the caption buttons sit in a
/// 30px band at the Shell's top right, and the panes start under it.
pub fn layout(w: f32, h: f32, state: ShellState, platform: Platform) -> Shell {
    use metric::*;
    let inner = Rect::new(BORDER, BORDER, w - 2.0 * BORDER, h - 2.0 * BORDER);
    let foot = inner.bottom() - MARGIN_BOTTOM;
    let side_top = inner.y + BAR_H;

    let (rail, sidebar, main_x) = if state.sidebar_open {
        let sb = Rect::new(inner.x + MARGIN_SIDE, side_top, SIDEBAR_W, foot - side_top);
        (None, Some(sb), sb.right())
    } else {
        (Some(Rect::new(inner.x, side_top, RAIL_W, foot - side_top)), None, inner.x + RAIL_W)
    };
    let bar = if state.sidebar_open { Rect::new(inner.x, inner.y, main_x - inner.x, BAR_H) } else { Rect::new(inner.x, inner.y, inner.w, BAR_H) };
    let top = if state.sidebar_open { inner.y + MARGIN_TOP } else { side_top };

    // The Shell from the sidebar to the side margin; everything in it sits SHELL_PAD in (under the
    // caption band on Windows), and its right gutter holds the open drawer, or the rail while folded.
    let shell = Rect::new(main_x, top, inner.right() - MARGIN_SIDE - main_x, foot - top);
    let has_caps = platform != Platform::Mac;
    let caps = has_caps.then(|| Rect::new(shell.right() - PAD - CAPS_W, shell.y + (CAPS_BAND - CAPS_H) / 2.0, CAPS_W, CAPS_H));
    let in_top = shell.y + if has_caps { CAPS_BAND } else { PAD };
    let (in_h, right) = (shell.bottom() - PAD - in_top, shell.right() - PAD);
    // The rail always stands at the right edge (its icons stacked), the open drawer beside it.
    let (drawer, rrail) = if !state.plugins {
        (None, None)
    } else {
        let rail = Rect::new(right - RRAIL_W, in_top, RRAIL_W, in_h);
        let drawer = (state.drawer_open && !state.terminal_open).then(|| Rect::new(rail.x - PAD - DRAWER_W, in_top, DRAWER_W, in_h));
        (drawer, Some(rail))
    };
    let mut main_right = drawer.or(rrail).map_or(right, |r| r.x - PAD);
    let terminal = (state.terminal_open && state.plugins).then(|| {
        let t = Rect::new(main_right - TERM_W, in_top, TERM_W, in_h);
        main_right = t.x - PAD;
        t
    });
    let half = state.split_open.then(|| {
        let hf = Rect::new(main_right - SPLIT_W, in_top, SPLIT_W, in_h);
        main_right = hf.x - PAD;
        hf
    });
    let main = Rect::new(shell.x + PAD, in_top, main_right - shell.x - PAD, in_h);

    let has_lights = platform != Platform::Windows;
    let lights = has_lights.then(|| Rect::new(inner.x + 14.0, inner.y + (BAR_H - 12.0) / 2.0, 52.0, 12.0));
    let sbtn_x = if has_lights { inner.x + 78.0 } else { inner.x + 14.0 };
    let sidebar_button = Rect::new(sbtn_x, inner.y + (BAR_H - SBTN) / 2.0, SBTN, SBTN);
    let search_button = state.sidebar_open.then(|| Rect::new(sidebar_button.right() + SEARCH_GAP, sidebar_button.y, SBTN, SBTN));

    Shell { effective: state, rail, sidebar, bar, shell, main, half, drawer, terminal, rrail, sidebar_button, search_button, lights, caps }
}

/// The state that actually fits: the thread column keeps [`metric::MAIN_MIN_W`], so Split closes
/// first, then the terminal, then the drawer. The sidebar is the user's choice and stays.
pub fn resolve(w: f32, h: f32, mut state: ShellState, platform: Platform) -> ShellState {
    while layout(w, h, state, platform).main.w < metric::MAIN_MIN_W {
        if state.split_open {
            state.split_open = false;
        } else if state.terminal_open {
            state.terminal_open = false;
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
