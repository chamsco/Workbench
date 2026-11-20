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
        "notch" => s.notch,
        "bar" => s.bar,
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
                // The golden is the board's Windows notch layout, now the Appearance option.
                "windows" => Platform::WindowsNotch,
                other => panic!("unknown platform {other}"),
            };
            let flags = state(st["sidebar_open"].as_bool().unwrap(), st["support_open"].as_bool().unwrap(), st["dock_open"].as_bool().unwrap());
            let shell = layout(w, h, flags, platform);
            for (key, r) in st["rects"].as_object().unwrap() {
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
        for platform in [Platform::Mac, Platform::Windows, Platform::WindowsNotch] {
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

/// Tight tiles: the cards stand 4px from the window's top, right and bottom edges and 4px from each
/// other; the sidebar sits flat on the frame from the left edge, and with it folded the card takes
/// the 4px margin on the left too. There is no top bar: every card starts 4px from the top.
#[test]
fn cards_are_tight_tiles_4px_apart() {
    for platform in [Platform::Mac, Platform::Windows, Platform::WindowsNotch] {
        for st in all_states() {
            let (w, h) = (1200.0, 760.0);
            let s = solve(w, h, st, platform);
            let at = format!("{platform:?} {st:?}");
            let top = if platform == Platform::Windows { metric::BAR_H } else { 4.0 };
            for c in cards(&s) {
                assert!(c.right() <= w - 4.0 + 0.01 && c.bottom() <= h - 4.0 + 0.01, "{at}: {c:?} keeps the 4px margin");
            }
            assert_eq!(s.main.y, top, "{at}: no bar, except the Windows strip");
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
    for platform in [Platform::Mac, Platform::Windows, Platform::WindowsNotch] {
        for st in all_states() {
            let s = solve(1200.0, 760.0, st, platform);
            let host = s.support.unwrap_or(s.main);
            let r = s.rail;
            assert_eq!((r.w, r.right(), r.bottom()), (44.0, host.right() - 1.0, host.bottom() - 1.0), "{platform:?} {st:?}: rail in the card's right edge");
            assert!(r.y > host.y + metric::HEAD_H, "{platform:?} {st:?}: rail under the header");
        }
    }
}

/// Windows: the caption buttons sit in a 100x40 notch cut into the top-right of the rightmost
/// card; macOS has no notch and its lights sit top-left. Pane geometry is the same on both.
#[test]
fn platforms_differ_only_in_window_controls() {
    for st in all_states() {
        let mac = solve(1200.0, 760.0, st, Platform::Mac);
        let win = solve(1200.0, 760.0, st, Platform::WindowsNotch);
        assert!(mac.lights.is_some() && mac.caps.is_none() && mac.notch.is_none(), "{st:?}");
        assert!(win.lights.is_none() && win.caps.is_some(), "{st:?}");
        assert_eq!(mac.main, win.main, "{st:?}: same pane geometry on both platforms");
        assert_eq!((mac.support, mac.dock, mac.sidebar, mac.rail), (win.support, win.dock, win.sidebar, win.rail), "{st:?}");
        let n = win.notch.expect("notch");
        let host = win.support.unwrap_or(win.main);
        assert_eq!((n.w, n.h, n.right(), n.y), (100.0, 40.0, host.right(), host.y), "{st:?}: notch in the card's top-right");
        assert!(inside(n, win.caps.unwrap()), "{st:?}: caption buttons inside the notch");
    }
}

/// The stream card's 38px header holds the tabs, clear of the lights, the sidebar button, New
/// stream, the tools on the right and, on Windows, the notch.
#[test]
fn tabs_live_in_the_stream_header() {
    for &(w, h) in &[(1200.0, 760.0), (1000.0, 640.0), (1920.0, 1080.0)] {
        for platform in [Platform::Mac, Platform::Windows, Platform::WindowsNotch] {
            for st in all_states() {
                let s = solve(w, h, st, platform);
                let t = s.tabs;
                let at = format!("{w}x{h} {platform:?} {st:?}: tabs {t:?}");
                assert_eq!((t.y, t.h), (s.main.y + 6.0, 28.0), "{at}: in the 38px header");
                assert!(t.x >= s.main.x + 6.0 && t.right() <= s.main.right() - 8.0, "{at}: inside the card");
                for r in [s.lights, Some(s.sidebar_button), s.new_button, s.notch].into_iter().flatten() {
                    assert!(!overlaps(t, r), "{at}: clear of {r:?}");
                }
            }
        }
    }
}

/// Windows with the sidebar folded (Charles, 15 Nov): the sidebar button sits in the main
/// content, directly left of the tabs. With the sidebar open it is in the sidebar's top row, on
/// both platforms; Mac folded keeps it after the lights.
#[test]
fn windows_folded_puts_the_sidebar_button_left_of_the_tabs() {
    let st = state(false, true, false);
    let win = solve(1200.0, 760.0, st, Platform::WindowsNotch);
    assert!(inside(win.main, win.sidebar_button), "button in the main card");
    assert_eq!(win.sidebar_button.x, win.main.x + 6.0);
    assert!(win.tabs.x > win.sidebar_button.right(), "tabs after the button");
    for platform in [Platform::Mac, Platform::WindowsNotch, Platform::WindowsNotchNotch] {
        let open = solve(1200.0, 760.0, state(true, true, false), platform);
        assert!(inside(open.sidebar.unwrap(), open.sidebar_button), "{platform:?}: open, the button in the sidebar");
    }
    let mac = solve(1200.0, 760.0, st, Platform::Mac);
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

/// Windows default (Charles, 20 Nov, option B): a 34px strip across the window on the frame holds the
/// sidebar button, search, inbox, "N needs you" and the caption buttons; tabs and tools stay in the
/// stream card's header. The cards start under the strip and are as wide as on the other layouts.
#[test]
fn the_windows_strip_holds_the_window_controls() {
    for st in all_states() {
        let strip = solve(1200.0, 760.0, st, Platform::Windows);
        let notch = solve(1200.0, 760.0, st, Platform::WindowsNotch);
        assert!(strip.notch.is_none() && notch.notch.is_some() && notch.bar.is_none() && notch.pill.is_none(), "{st:?}");
        let bar = strip.bar.expect("strip");
        assert_eq!((bar.x, bar.y, bar.w, bar.h), (0.0, 0.0, 1200.0, 34.0), "{st:?}");
        assert_eq!(strip.main.y, 34.0, "{st:?}: the cards start under the strip");
        assert_eq!((strip.main.x, strip.main.w), (notch.main.x, notch.main.w), "{st:?}: same widths");
        let caps = strip.caps.unwrap();
        assert_eq!((caps.right(), caps.y, caps.w, caps.h), (1200.0, 0.0, 138.0, 34.0), "{st:?}: three 46px cells at the right end");
        let pill = strip.pill.unwrap();
        for r in [pill, strip.sidebar_button, strip.search_button, strip.bell_button, caps] {
            assert!(inside(bar, r), "{st:?}: {r:?} inside the strip");
        }
        assert_eq!((strip.sidebar_button.x, strip.search_button.x, strip.bell_button.x), (10.0, 46.0, 82.0), "{st:?}: sidebar, search, inbox in order");
        assert!(strip.bell_button.right() < pill.x && pill.right() <= caps.x, "{st:?}: buttons, then the pill, then the caption buttons");
        // Tabs and tools are in the card header, unchanged by the strip.
        assert_eq!((strip.tabs.y, strip.tools.y), (strip.main.y + 6.0, strip.main.y + 6.0), "{st:?}");
        assert!(strip.tools.right() <= strip.main.right() - 8.0, "{st:?}: tools need no notch room");
    }
}
