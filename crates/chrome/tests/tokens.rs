//! Acceptance tests for UI0 tokens: parity with the wireframe and the contrast rules
//! from the UI skills (WCAG AA 4.5:1 for text), as code instead of a manual check.

use cauldron_chrome::tokens::{radius, Palette, Rgb, DARK, LIGHT};
use serde_json::Value;

const GOLDEN: &str = include_str!("golden/wireframe.json");

fn pal(name: &str) -> &'static Palette {
    if name == "dark" { &DARK } else { &LIGHT }
}

#[test]
fn palettes_match_the_wireframe_tokens_exactly() {
    let g: Value = serde_json::from_str(GOLDEN).unwrap();
    for theme in ["dark", "light"] {
        let t = &g["tokens"][theme];
        let p = pal(theme);
        let pairs = [
            ("bg", p.bg), ("frame", p.frame), ("card", p.card), ("raised", p.raised), ("well", p.well),
            ("line", p.line), ("strip", p.strip), ("fg", p.fg), ("fg-2", p.fg2), ("fg-3", p.fg3),
            ("accent", p.accent), ("on-accent", p.on_accent), ("run", p.run), ("ok", p.ok),
            ("warn", p.warn), ("bad", p.bad),
        ];
        for (name, got) in pairs {
            assert_eq!(got.to_hex(), t[name].as_str().unwrap(), "{theme} --{name}");
        }
    }
}

#[test]
fn radii_match_the_wireframe_and_nest_by_subtraction() {
    let g: Value = serde_json::from_str(GOLDEN).unwrap();
    let px = |k: &str| g["tokens"]["dark"][k].as_str().unwrap().trim_end_matches("px").parse::<f32>().unwrap();
    assert_eq!(radius::R, px("r"));
    assert_eq!(radius::R1, px("r1"));
    assert_eq!(radius::R2, px("r2"));
    assert_eq!(radius::WINDOW, radius::R + 4.0);
    assert_eq!(radius::nested(6.0, 3.0), 3.0);
    assert_eq!(radius::nested(6.0, 8.0), 0.0, "never negative");
}

#[test]
fn contrast_follows_wcag() {
    let black = Rgb::hex(0x000000);
    let white = Rgb::hex(0xffffff);
    assert!((black.contrast(white) - 21.0).abs() < 0.01);
    assert!((white.contrast(black) - 21.0).abs() < 0.01, "symmetric");
    assert!((white.contrast(white) - 1.0).abs() < 0.001);
    assert_eq!(Rgb::hex(0xff4d2d).to_hex(), "#ff4d2d");
}

#[test]
fn every_text_pair_passes_aa_in_both_themes() {
    for theme in ["dark", "light"] {
        let p = pal(theme);
        let text_on = [
            ("fg", p.fg, p.card), ("fg", p.fg, p.raised), ("fg-2", p.fg2, p.card), ("fg-2", p.fg2, p.well),
            ("fg-3 (placeholders, meta)", p.fg3, p.card), ("fg-3 on raised", p.fg3, p.raised),
            ("fg-3 on frame", p.fg3, p.frame), ("accent text", p.accent, p.card),
            ("accent text on raised", p.accent, p.raised), ("on-accent", p.on_accent, p.accent),
        ];
        for (what, fg, bg) in text_on {
            let c = fg.contrast(bg);
            assert!(c >= 4.5, "{theme}: {what} is {c:.2}:1 ({} on {})", fg.to_hex(), bg.to_hex());
        }
    }
}

/// Found by this test: light accent on the frame is 4.13:1. Fine for icons (3:1), not for text.
/// Rule for the GPUI build: accent text only on card or raised; on the frame the accent is a glyph.
#[test]
fn accent_on_the_frame_is_for_glyphs_only() {
    for theme in ["dark", "light"] {
        let p = pal(theme);
        let c = p.accent.contrast(p.frame);
        assert!(c >= 3.0, "{theme}: accent glyph on frame is {c:.2}:1");
    }
    assert!(LIGHT.accent.contrast(LIGHT.frame) < 4.5, "if this ever passes AA, the rule above can go");
}

#[test]
fn one_accent_and_status_colours_stay_distinct() {
    for theme in ["dark", "light"] {
        let p = pal(theme);
        let hues = [p.accent, p.run, p.ok, p.warn, p.bad];
        for (i, a) in hues.iter().enumerate() {
            for b in &hues[i + 1..] {
                assert_ne!(a, b, "{theme}: two status colours collapsed into one");
            }
        }
    }
}

const WIREFRAME: &str = include_str!("../../../design/cauldron-master-layout.html");

/// The value of `--name` in the wireframe's dark `:root` (first match) or light override (second).
fn css_var(name: &str, theme: &str) -> String {
    let key = format!("--{name}:");
    let mut hits = WIREFRAME.match_indices(&key).map(|(i, _)| &WIREFRAME[i + key.len()..i + key.len() + 7]);
    let first = hits.next().expect("var in wireframe");
    if theme == "dark" { first.to_string() } else { hits.next().expect("light override").to_string() }
}

/// Kind and librarian colours (UI3): code threads `info`, design threads `design`, librarian `mem`.
/// Not in the measured golden file, so read straight from the wireframe's stylesheet.
#[test]
fn kind_colours_match_the_wireframe_and_read_on_cards() {
    for theme in ["dark", "light"] {
        let p = pal(theme);
        for (name, got) in [("info", p.info), ("design", p.design), ("mem", p.mem)] {
            assert_eq!(got.to_hex(), css_var(name, theme), "{theme} --{name}");
            for (bg_name, bg) in [("card", p.card), ("raised", p.raised), ("well", p.well)] {
                let c = got.contrast(bg);
                assert!(c >= 3.0, "{theme}: {name} glyph on {bg_name} is {c:.2}:1");
            }
        }
    }
    // Found by this test: light `mem` on card is 4.24:1, under AA for the wireframe's 11.5px
    // "filed by librarian" text. Rule for the GPUI build, as for the accent on the frame: `mem`
    // is a glyph colour; the words go in fg-3.
    assert!(LIGHT.mem.contrast(LIGHT.card) < 4.5, "if this ever passes AA, the rule above can go");
}

/// Agent identity dots on tabs and in the + menu, the wireframe's `.bd` colours, all distinct.
#[test]
fn agent_dots_come_from_the_wireframe() {
    use cauldron_chrome::tokens::AGENT_DOTS;
    for (i, d) in AGENT_DOTS.iter().enumerate() {
        assert!(WIREFRAME.contains(&format!("class=\"bd\" style=\"background:{}\"", d.to_hex())), "{} not a wireframe dot", d.to_hex());
        assert!(!AGENT_DOTS[i + 1..].contains(d), "{} twice", d.to_hex());
    }
    assert_eq!(AGENT_DOTS.len(), 9);
}

/// Drop shadows are the wireframe's: the + menu's `0 12px 32px -12px rgb(0 0 0 / .6)` and the
/// tip's `0 6px 16px rgb(0 0 0 / .3)`.
#[test]
fn shadows_come_from_the_wireframe() {
    use cauldron_chrome::tokens::shadow;
    let html = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/cauldron-master-layout.html")).unwrap();
    for s in [shadow::MENU, shadow::TIP] {
        let spread = if s.spread == 0.0 { String::new() } else { format!(" {}px", s.spread) };
        let css = format!("0 {}px {}px{spread} rgb(0 0 0 / {})", s.y, s.blur, format!("{}", s.alpha).trim_start_matches('0'));
        assert!(html.contains(&css), "{css} not in the wireframe");
    }
}
