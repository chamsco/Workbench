//! Acceptance tests for the shell geometry: the solver must reproduce the approved Tight tiles board
//! (design/cauldron-tight-tiles/index.html, Charles 15 Nov). The golden file is measured from the
//! board by tools/wireframe-golden/extract.js, never typed by hand.

use cauldron_chrome::layout::{layout, metric, solve, support_width, Platform, Rect, Shell, ShellState};
use serde_json::Value;

const GOLDEN: &str = include_str!("golden/wireframe.json");
const TOL: f32 = 1.0;

fn want(r: &Value) -> Rect {
    Rect {
        x: r["x"].as_f64().unwrap() as f32,
        y: r["y"].as_f64().unwrap() as f32,
        w: r["w"].as_f64().unwrap() as f32,
        h: r["h"].as_f64().unwrap() as f32,
    }
}

fn close(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() <= TOL && (a.y - b.y).abs() <= TOL && (a.w - b.w).abs() <= TOL && (a.h - b.h).abs() <= TOL
}

fn pick(s: &Shell, key: &str) -> Option<Rect> {
    match key {
        "sidebar" => s.sidebar,
        "main" => Some(s.main),
        "dock" => s.dock,
        "support" => s.support,
        "rail" => Some(s.rail),
        "bar" => s.bar,
        // The board's Windows notch is gone; the golden still lists it, hidden on macOS.
        "notch" => None,
        "caps" => s.caps,
        "lights" => s.lights,
        "sbtn" => Some(s.sidebar_button),
        "newb" => s.new_button,
        "tabs" => Some(s.tabs),
        other => panic!("unknown golden rect {other}"),
    }
}

fn state(sidebar_open: bool, support_open: bool, dock_open: bool) -> ShellState {
    ShellState { sidebar_open, support_open, dock_open }
}

#[test]
fn solver_matches_every_golden_state_and_size() {
    let g: Value = serde_json::from_str(GOLDEN).unwrap();
    let mut checked = 0;
    for size in g["sizes"].as_array().unwrap() {
        let (w, h) = (size["window"]["w"].as_f64().unwrap() as f32, size["window"]["h"].as_f64().unwrap() as f32);
        for (name, st) in size["states"].as_object().unwrap() {
            let platform = match st["platform"].as_str().unwrap() {
                "mac" => Platform::Mac,
                "windows" => Platform::Windows,
                other => panic!("unknown platform {other}"),
            };
            let flags = state(st["sidebar_open"].as_bool().unwrap(), st["support_open"].as_bool().unwrap(), st["dock_open"].as_bool().unwrap());
            let shell = layout(w, h, flags, platform);
            for (key, r) in st["rects"].as_object().unwrap() {
                // Windows (Charles, 20 Nov) swapped the board's notch for a top bar: its cards start
                // under the bar, so only their x and width still follow the board; the bar has its
                // own tests below. Header items moved into the bar and have none.
                if platform == Platform::Windows {
                    if let (true, Some(g)) = (["sidebar", "main", "dock", "support", "rail"].contains(&key.as_str()) && r["visible"].as_bool().unwrap(), pick(&shell, key)) {
                        let w = want(r);
                        assert!((g.x - w.x).abs() <= TOL && (g.w - w.w).abs() <= TOL, "{w:?} {name}: {key} x/w got {g:?}");
                    }
                    checked += 1;
                    continue;
                }
                let got = pick(&shell, key);
                if r["visible"].as_bool().unwrap() {
                    let got = got.unwrap_or_else(|| panic!("{w}x{h} {name}: {key} should be visible"));
                    assert!(close(got, want(r)), "{w}x{h} {name}: {key} got {got:?} want {:?}", want(r));
                } else {
                    assert!(got.is_none(), "{w}x{h} {name}: {key} should be hidden, got {got:?}");
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "golden file looks empty: {checked} checks");
}

#[test]
fn golden_has_two_window_sizes() {
    let g: Value = serde_json::from_str(GOLDEN).unwrap();
    let ws: Vec<i64> = g["sizes"].as_array().unwrap().iter().map(|s| s["window"]["w"].as_i64().unwrap()).collect();
    assert_eq!(ws.len(), 2);
    assert_ne!(ws[0], ws[1], "the second size must differ or the solver is only checked at one width");
}

fn inside(win: Rect, r: Rect) -> bool {
    r.x >= win.x - 0.01 && r.y >= win.y - 0.01 && r.x + r.w <= win.x + win.w + 0.01 && r.y + r.h <= win.y + win.h + 0.01
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.w - 0.01 && b.x < a.x + a.w - 0.01 && a.y < b.y + b.h - 0.01 && b.y < a.y + a.h - 0.01
}

fn cards(s: &Shell) -> Vec<Rect> {
    let mut v = vec![s.main];
    v.extend(s.dock);
    v.extend(s.support);
    v
}

fn all_states() -> impl Iterator<Item = ShellState> {
    (0..8u8).map(|b| state(b & 1 != 0, b & 2 != 0, b & 4 != 0))
}

#[test]
fn panes_never_overlap_and_stay_inside_the_window() {
    for &(w, h) in &[(1200.0, 760.0), (1000.0, 640.0), (800.0, 600.0), (1920.0, 1080.0)] {
        let win = Rect { x: 0.0, y: 0.0, w, h };
        for platform in [Platform::Mac, Platform::Windows] {
            for st in all_states() {
                let s = solve(w, h, st, platform);
                assert!(s.main.w >= 100.0 && s.main.h > 100.0, "{w}x{h} {platform:?} {st:?}: main collapsed {:?}", s.main);
                let mut ps = cards(&s);
                ps.extend(s.sidebar);
                for (i, a) in ps.iter().enumerate() {
                    assert!(inside(win, *a), "{w}x{h} {platform:?} {st:?}: pane {i} outside {a:?}");
                    for b in &ps[i + 1..] {
                        assert!(!overlaps(*a, *b), "{w}x{h} {platform:?} {st:?}: {a:?} overlaps {b:?}");
                    }
                }
            }
        }
    }
}

/// Tight tiles: the cards stand 4px from the window's right and bottom edges and 4px from each
/// other; the sidebar sits flat on the frame from the left edge, and with it folded the card takes
/// the 4px margin on the left too. macOS has no top bar: every card starts 4px from the top. On
/// Windows the 38px top bar runs over the window and the cards start under it.
#[test]
fn cards_are_tight_tiles_4px_apart() {
    for platform in [Platform::Mac, Platform::Windows] {
        for st in all_states() {
            let (w, h) = (1200.0, 760.0);
            let s = solve(w, h, st, platform);
            let at = format!("{platform:?} {st:?}");
            let top = if platform == Platform::Windows { metric::BAR_H } else { 4.0 };
            for c in cards(&s) {
                assert!(c.right() <= w - 4.0 + 0.01 && c.bottom() <= h - 4.0 + 0.01, "{at}: {c:?} keeps the 4px margin");
            }
            assert_eq!(s.main.y, top, "{at}: cards start under the bar, or 4px down without one");
            match s.sidebar {
                Some(sb) => {
                    assert_eq!((sb.x, sb.y, sb.w, sb.bottom()), (0.0, top, 236.0, h - 4.0), "{at}: sidebar on the frame");
                    assert_eq!(s.main.x, sb.right() + 4.0, "{at}: card 4px from the sidebar");
                }
                None => assert_eq!(s.main.x, 4.0, "{at}: the card takes the left margin"),
            }
            if let Some(sup) = s.support {
                assert_eq!((sup.y, sup.bottom(), sup.right()), (top, h - 4.0, w - 4.0), "{at}: support card full height at the right");
                assert_eq!(s.main.right() + 4.0, sup.x, "{at}: 4px between stream and support");
            } else {
                assert_eq!(s.main.right(), w - 4.0, "{at}: folded, the stream card runs to the margin");
            }
            match s.dock {
                Some(d) => {
                    assert_eq!((d.x, d.w, d.h, d.bottom()), (s.main.x, s.main.w, 200.0, h - 4.0), "{at}: the dock under the stream");
                    assert_eq!(s.main.bottom() + 4.0, d.y, "{at}: 4px above the dock");
                }
                None => assert_eq!(s.main.bottom(), h - 4.0, "{at}: the stream runs to the bottom margin"),
            }
        }
    }
}

/// The support card is 470 wide, never more than half the window and never under 360.
#[test]
fn support_card_is_470_within_360_and_half_the_window() {
    assert_eq!(support_width(1200.0), 470.0);
    assert_eq!(support_width(1920.0), 470.0);
    assert_eq!(support_width(860.0), 430.0, "half the window");
    assert_eq!(support_width(600.0), 360.0, "never under 360");
}

/// The quick-button rail is inside the rightmost card's right edge, 44 wide, under its header;
/// open and folded use the same rail.
#[test]
fn the_rail_is_inside_the_rightmost_card() {
    for platform in [Platform::Mac, Platform::Windows] {
        for st in all_states() {
            let s = solve(1200.0, 760.0, st, platform);
            let host = s.support.unwrap_or(s.main);
            let r = s.rail;
            assert_eq!((r.w, r.right(), r.bottom()), (44.0, host.right() - 1.0, host.bottom() - 1.0), "{platform:?} {st:?}: rail in the card's right edge");
            assert!(r.y > host.y + metric::HEAD_H, "{platform:?} {st:?}: rail under the header");
        }
    }
}

/// Windows: a 38px top bar runs across the window with the caption buttons at its right end and
/// "N needs you" before the tools; macOS has no bar and its lights sit top-left. The cards are the
/// same width on both, and Windows' start under the bar.
#[test]
fn platforms_differ_only_in_window_controls() {
    for st in all_states() {
        let mac = solve(1200.0, 760.0, st, Platform::Mac);
        let win = solve(1200.0, 760.0, st, Platform::Windows);
        assert!(mac.lights.is_some() && mac.caps.is_none() && mac.bar.is_none() && mac.pill.is_none(), "{st:?}");
        assert!(win.lights.is_none() && win.caps.is_some() && win.bar.is_some() && win.pill.is_some(), "{st:?}");
        assert_eq!((mac.main.x, mac.main.w), (win.main.x, win.main.w), "{st:?}: same widths on both platforms");
        assert_eq!(win.main.y, metric::BAR_H, "{st:?}: the cards start under the bar");
        let bar = win.bar.unwrap();
        assert_eq!((bar.x, bar.y, bar.w, bar.h), (0.0, 0.0, 1200.0, 38.0), "{st:?}");
        let caps = win.caps.unwrap();
        assert_eq!((caps.right(), caps.y, caps.h, caps.w), (1200.0, 0.0, 38.0, 138.0), "{st:?}: three 46px cells at the bar's right end");
        assert!(inside(bar, caps) && inside(bar, win.pill.unwrap()) && inside(bar, win.tools) && inside(bar, win.tabs), "{st:?}");
        for r in [win.pill.unwrap(), win.tools, win.tabs, win.sidebar_button, win.search_button, win.bell_button] {
            assert!(!overlaps(r, caps), "{st:?}: {r:?} clear of the caption buttons");
        }
        assert!(win.pill.unwrap().right() <= win.tools.x && win.tabs.right() <= win.pill.unwrap().x, "{st:?}: tabs, then the pill, then the tools");
    }
}

/// The header items (tabs, pill, tools) share one row: the stream card's 38px header on macOS,
/// the top bar on Windows. Clear of the lights, the sidebar button, search and inbox, New stream.
#[test]
fn tabs_live_in_the_header_row() {
    for &(w, h) in &[(1200.0, 760.0), (1000.0, 640.0), (1920.0, 1080.0)] {
        for platform in [Platform::Mac, Platform::Windows] {
            for st in all_states() {
                let s = solve(w, h, st, platform);
                let t = s.tabs;
                let at = format!("{w}x{h} {platform:?} {st:?}: tabs {t:?}");
                let (row_y, edge) = if platform == Platform::Windows { (5.0, w) } else { (s.main.y + 6.0, s.main.right() - 8.0) };
                assert_eq!((t.y, t.h), (row_y, 28.0), "{at}: in the header row");
                assert!(t.x >= 6.0 && t.right() <= edge, "{at}: inside the row");
                if platform == Platform::Windows {
                    assert!(t.x >= s.main.x + 6.0, "{at}: from the stream card's left edge");
                } else {
                    assert!(t.x >= s.main.x + 6.0 && t.right() <= s.main.right() - 8.0, "{at}: inside the card");
                }
                for r in [s.lights, Some(s.sidebar_button), s.new_button, s.pill, s.caps].into_iter().flatten().chain((platform == Platform::Windows).then_some([s.search_button, s.bell_button]).into_iter().flatten()) {
                    assert!(!overlaps(t, r), "{at}: clear of {r:?}");
                }
            }
        }
    }
}

/// Windows: the sidebar button is the first thing in the bar, folded or not, with search and
/// inbox after it. On macOS it is in the sidebar's top row (open), or after the lights (folded),
/// with search and inbox to its left.
#[test]
fn the_sidebar_button_leads_the_bar_on_windows() {
    for st in all_states() {
        let win = solve(1200.0, 760.0, st, Platform::Windows);
        assert_eq!((win.sidebar_button.x, win.sidebar_button.y), (10.0, 5.0), "{st:?}");
        assert!(win.search_button.x == win.sidebar_button.x + 36.0 && win.bell_button.x == win.sidebar_button.x + 72.0, "{st:?}");
    }
    for platform in [Platform::Mac] {
        let open = solve(1200.0, 760.0, state(true, true, false), platform);
        assert!(inside(open.sidebar.unwrap(), open.sidebar_button), "{platform:?}: open, the button in the sidebar");
        assert!(open.search_button.x < open.sidebar_button.x, "{platform:?}: search left of the button");
    }
    let mac = solve(1200.0, 760.0, state(false, true, false), Platform::Mac);
    assert!(mac.sidebar_button.x > mac.lights.unwrap().right(), "Mac: button after the lights");
}

/// The stream column keeps 360px: a narrow window closes the support card first. The sidebar is
/// the user's choice and stays; the dock does not take width.
#[test]
fn main_keeps_a_minimum_width_by_closing_the_support_card() {
    let s = solve(900.0, 640.0, state(true, true, true), Platform::Mac);
    assert!(s.main.w >= metric::MAIN_MIN_W && s.support.is_none() && !s.effective.support_open);
    assert!(s.sidebar.is_some() && s.dock.is_some());
    let wide = solve(1400.0, 800.0, state(true, true, false), Platform::Mac);
    assert!(wide.support.is_some() && wide.effective == state(true, true, false));
}
