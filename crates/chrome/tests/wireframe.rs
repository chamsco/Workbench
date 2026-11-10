//! Acceptance tests for UI0: the solver must reproduce the reviewed wireframe.
//! The golden file is measured from the published review page, never typed by hand.

use cauldron_chrome::layout::{layout, solve, Platform, Rect, Shell, ShellState};
use serde_json::Value;

const GOLDEN: &str = include_str!("golden/wireframe.json");
const TOL: f32 = 1.0;

fn state(name: &str) -> ShellState {
    let (sidebar_open, drawer_open, split_open) = match name {
        "default" => (true, true, false),
        "drawer_closed" => (true, false, false),
        "split_drawer_open" => (true, true, true),
        "sidebar_hidden" => (false, true, false),
        "sidebar_hidden_drawer_closed" => (false, false, false),
        other => panic!("unknown golden state {other}"),
    };
    ShellState { sidebar_open, drawer_open, split_open }
}

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
        "rail" => s.rail,
        "sidebar" => s.sidebar,
        "main" => Some(s.main),
        "half" => s.half,
        "drawer" => s.drawer,
        "rrail" => Some(s.rrail),
        "notch" => s.notch,
        "sbtn" => Some(s.sidebar_button),
        "tabs" => Some(s.tabs),
        "topr" => Some(s.topr),
        "wcaps" => s.caps,
        "lights" => s.lights,
        other => panic!("unknown golden rect {other}"),
    }
}

#[test]
fn solver_matches_every_golden_state_and_size() {
    let g: Value = serde_json::from_str(GOLDEN).unwrap();
    let mut checked = 0;
    for size in g["sizes"].as_array().unwrap() {
        let (w, h) = (size["window"]["w"].as_f64().unwrap() as f32, size["window"]["h"].as_f64().unwrap() as f32);
        for (name, st) in size["states"].as_object().unwrap() {
            let shell = layout(w, h, state(name), Platform::Reference);
            for (key, r) in st["rects"].as_object().unwrap() {
                let visible = r["visible"].as_bool().unwrap();
                let got = pick(&shell, key);
                if visible {
                    let got = got.unwrap_or_else(|| panic!("{w}x{h} {name}: {key} should be visible"));
                    assert!(close(got, want(r)), "{w}x{h} {name}: {key} got {got:?} want {:?}", want(r));
                    checked += 1;
                } else {
                    assert!(got.is_none(), "{w}x{h} {name}: {key} should be hidden, got {got:?}");
                    checked += 1;
                }
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

fn panes(s: &Shell) -> Vec<Rect> {
    let mut v = vec![s.main, s.rrail];
    v.extend(s.sidebar);
    v.extend(s.half);
    v.extend(s.drawer);
    v
}

#[test]
fn panes_never_overlap_and_stay_inside_the_window() {
    for &(w, h) in &[(1240.0, 702.0), (1000.0, 640.0), (800.0, 600.0), (1920.0, 1080.0)] {
        let win = Rect { x: 0.0, y: 0.0, w, h };
        for platform in [Platform::Mac, Platform::Windows, Platform::Reference] {
            for bits in 0..8u8 {
                let st = ShellState { sidebar_open: bits & 1 != 0, drawer_open: bits & 2 != 0, split_open: bits & 4 != 0 };
                let s = solve(w, h, st, platform);
                assert!(s.main.w >= 100.0 && s.main.h > 100.0, "{w}x{h} {platform:?} {st:?}: main collapsed {:?}", s.main);
                let ps = panes(&s);
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

// Proposed rule, not drawn in the wireframe: the thread column never gets narrower than 360px.
// Split closes first, then the drawer. The sidebar is the user's choice and is never closed by the solver.
#[test]
fn main_keeps_a_minimum_width_by_closing_split_then_drawer() {
    let all = ShellState { sidebar_open: true, drawer_open: true, split_open: true };
    let s = solve(1240.0, 702.0, all, Platform::Reference);
    assert_eq!(s.effective, all, "nothing closes at the reviewed size");
    let s = solve(900.0, 600.0, all, Platform::Reference);
    assert!(s.main.w >= 360.0, "main {}", s.main.w);
    assert!(!s.effective.split_open && s.effective.drawer_open, "split closes before the drawer: {:?}", s.effective);
    let s = solve(700.0, 600.0, all, Platform::Reference);
    assert!(!s.effective.split_open && !s.effective.drawer_open && s.effective.sidebar_open);
}

#[test]
fn platforms_differ_only_in_window_controls() {
    let st = ShellState { sidebar_open: true, drawer_open: true, split_open: false };
    let mac = solve(1240.0, 702.0, st, Platform::Mac);
    let win = solve(1240.0, 702.0, st, Platform::Windows);
    assert!(mac.lights.is_some() && mac.caps.is_none());
    assert!(win.lights.is_none() && win.caps.is_some());
    assert_eq!(mac.main, win.main, "same pane geometry on both platforms");
    assert_eq!(mac.rrail, win.rrail);
    assert_eq!(mac.sidebar, win.sidebar);
    // The sidebar button stays inside the notch on both.
    for s in [&mac, &win] {
        let n = s.notch.unwrap();
        assert!(inside(n, s.sidebar_button), "button {:?} outside notch {n:?}", s.sidebar_button);
    }
    // Windows: Terminal and Split sit directly left of the caption buttons.
    let caps = win.caps.unwrap();
    assert!((win.topr.x + win.topr.w + 8.0 - caps.x).abs() <= TOL);
    // Mac: lights and button do not touch.
    assert!(!overlaps(mac.lights.unwrap(), mac.sidebar_button));
}
