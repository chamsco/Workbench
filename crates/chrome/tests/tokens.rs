//! Acceptance tests for the tokens: parity with the design system's tokens.json and the
//! contrast rules (WCAG AA 4.5:1 for text, 3:1 for glyphs and control edges), as code.

use cauldron_chrome::tokens::{radius, Palette, Rgb, Rgba, AGENT_DOTS, DARK, LIGHT};
use serde_json::Value;

const GOLDEN: &str = include_str!("golden/design-system.json");

fn pal(name: &str) -> &'static Palette {
    if name == "dark" { &DARK } else { &LIGHT }
}

fn colour(g: &Value, name: &str, theme: &str) -> String {
    let t = g["color"]["tokens"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("no token {name}"));
    t["value"][theme].as_str().unwrap().to_string()
}

#[test]
fn palettes_match_the_design_system_exactly() {
    let g: Value = serde_json::from_str(GOLDEN).unwrap();
    for theme in ["dark", "light"] {
        let p = pal(theme);
        let solid = [
            ("bg", p.bg), ("frame", p.frame), ("card", p.card), ("panel", p.panel), ("raised", p.raised),
            ("line-control", p.line_control), ("fg", p.fg), ("fg-2", p.fg2), ("fg-3", p.fg3), ("fg-4", p.fg4),
            ("accent", p.accent), ("accent-hover", p.accent_hover), ("on-accent", p.on_accent),
            ("ok", p.ok), ("warn", p.warn), ("bad", p.bad),
        ];
        for (name, got) in solid {
            assert_eq!(got.to_hex(), colour(&g, name, theme), "{theme} {name}");
        }
        let over = [
            ("inset", p.inset), ("hover", p.hover), ("selected", p.selected), ("line", p.line),
            ("line-strong", p.line_strong), ("accent-soft", p.accent_soft), ("bad-soft", p.bad_soft),
        ];
        for (name, got) in over {
            assert_eq!(got.to_css(), colour(&g, name, theme), "{theme} {name}");
        }
    }
}

#[test]
fn agent_colours_are_agent_1_to_9() {
    let g: Value = serde_json::from_str(GOLDEN).unwrap();
    for (i, d) in AGENT_DOTS.iter().enumerate() {
        assert_eq!(d.to_hex(), colour(&g, &format!("agent-{}", i + 1), "dark"));
        assert!(!AGENT_DOTS[i + 1..].contains(d), "{} twice", d.to_hex());
    }
}

#[test]
fn radii_match_the_design_system_and_nest_by_the_shell_padding() {
    let g: Value = serde_json::from_str(GOLDEN).unwrap();
    let px = |k: &str| {
        let t = g["radius"]["tokens"].as_array().unwrap().iter().find(|t| t["name"] == k).unwrap();
        t["value"].as_str().unwrap().trim_end_matches("px").parse::<f32>().unwrap()
    };
    assert_eq!(radius::SHELL, px("radius-shell"));
    assert_eq!(radius::PANEL, px("radius-panel"));
    assert_eq!(radius::INSET, px("radius-inset"));
    assert_eq!(radius::CONTROL, px("radius-control"));
    assert_eq!(radius::INNER, px("radius-inner"));
    assert_eq!(radius::TAG, px("radius-tag"));
    assert_eq!(radius::nested(radius::SHELL, radius::SHELL_PAD), radius::PANEL, "a Shell's 6px around its Panel");
    assert_eq!(radius::nested(6.0, 8.0), 0.0, "never negative");
}

#[test]
fn overlays_composite_like_a_browser() {
    assert_eq!(Rgba::new(0xffffff, 0.5).over(Rgb::hex(0x000000)), Rgb::hex(0x808080));
    assert_eq!(Rgba::from(Rgb::hex(0x123456)).over(Rgb::hex(0xffffff)), Rgb::hex(0x123456));
    assert_eq!(Rgba::new(0x1c1c1f, 0.04).to_css(), "rgba(28,28,31,0.04)");
}

#[test]
fn contrast_follows_wcag() {
    let black = Rgb::hex(0x000000);
    let white = Rgb::hex(0xffffff);
    assert!((black.contrast(white) - 21.0).abs() < 0.01);
    assert!((white.contrast(black) - 21.0).abs() < 0.01, "symmetric");
    assert!((white.contrast(white) - 1.0).abs() < 0.001);
}

/// Text that someone must read passes AA on every surface it sits on, overlays included.
#[test]
fn every_text_pair_passes_aa_in_both_themes() {
    for theme in ["dark", "light"] {
        let p = pal(theme);
        let mut grounds = vec![("bg", p.bg), ("card", p.card), ("panel", p.panel), ("raised", p.raised)];
        for (n, s) in [("bg", p.bg), ("card", p.card), ("panel", p.panel)] {
            grounds.push((n, p.hover.over(s)));
            grounds.push((n, p.inset.over(s)));
        }
        // Selected rows live in the sidebar (on bg) and in panels, never on a card: see below.
        grounds.push(("selected on bg", p.selected.over(p.bg)));
        grounds.push(("selected on panel", p.selected.over(p.panel)));
        for (what, fg) in [("fg", p.fg), ("fg-2", p.fg2), ("fg-3", p.fg3)] {
            for (gname, g) in &grounds {
                let c = fg.contrast(*g);
                assert!(c >= 4.5, "{theme}: {what} on {gname} is {c:.2}:1");
            }
        }
        assert!(p.on_accent.contrast(p.accent) >= 4.5, "{theme}: on-accent");
        for (gname, g) in [("panel", p.panel), ("card", p.card)] {
            assert!(p.accent.contrast(g) >= 4.5, "{theme}: accent text on {gname}");
        }
    }
}

/// Glyph colours and control edges hold 3:1 on the surfaces they sit on.
#[test]
fn glyphs_and_control_edges_hold_3_to_1() {
    for theme in ["dark", "light"] {
        let p = pal(theme);
        for (gname, g) in [("bg", p.bg), ("card", p.card), ("panel", p.panel)] {
            for (what, c) in [("line-control", p.line_control), ("accent", p.accent), ("ok", p.ok), ("warn", p.warn), ("bad", p.bad)] {
                let r = c.contrast(g);
                assert!(r >= 3.0, "{theme}: {what} on {gname} is {r:.2}:1");
            }
        }
    }
}

/// Found by this test: light fg-3 on `selected` over a card is 4.37:1. Rule for the build:
/// the selected fill goes on bg (the sidebar) or a panel, not on a card.
#[test]
fn selected_on_a_card_is_under_aa_in_light() {
    assert!(LIGHT.fg3.contrast(LIGHT.selected.over(LIGHT.card)) < 4.5, "if this ever passes AA, the rule above can go");
}

#[test]
fn one_accent_and_status_colours_stay_distinct() {
    for theme in ["dark", "light"] {
        let p = pal(theme);
        let hues = [p.accent, p.ok, p.warn, p.bad];
        for (i, a) in hues.iter().enumerate() {
            for b in &hues[i + 1..] {
                assert_ne!(a, b, "{theme}: two status colours collapsed into one");
            }
        }
    }
}

#[test]
fn motion_stays_within_the_200ms_cap() {
    use cauldron_chrome::tokens::motion::*;
    for d in [MENU, INDICATOR, BASE, TIP] {
        assert!(d <= MAX && d > 0.0, "{d}");
    }
    // Staggered items finish with the menu: a later item starts later, never past half of it.
    for i in 0..20 {
        let (start, end) = stagger(i, MENU);
        assert!(start >= 0.0 && start <= MENU / 2.0 && end == MENU, "item {i}: {start}..{end}");
        assert!(i == 0 || start >= stagger(i - 1, MENU).0);
    }
}

#[test]
fn the_easing_curve_runs_from_rest_to_rest_without_overshoot() {
    use cauldron_chrome::tokens::motion::ease;
    assert_eq!(ease(0.0), 0.0);
    assert_eq!(ease(1.0), 1.0);
    assert_eq!(ease(-1.0), 0.0);
    assert_eq!(ease(2.0), 1.0);
    let mut last = 0.0;
    for i in 1..=100 {
        let y = ease(i as f32 / 100.0);
        assert!(y >= last && y <= 1.0, "{i}: {y}");
        last = y;
    }
    // cubic-bezier(.32,.72,0,1) is front-loaded: well past half way at a third of the time.
    assert!(ease(1.0 / 3.0) > 0.6, "{}", ease(1.0 / 3.0));
}
