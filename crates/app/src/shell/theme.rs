//! The user's own colours (board round 6, Charles 21 Nov 04:59Z): one for the frame and sidebar,
//! one for the stream and support cards. They override the theme's `bg`/`frame` and `card`/`panel`
//! and keep the theme's text, so each is held inside the theme's own range of lightness and
//! text stays readable on it.

use std::sync::Mutex;

use cauldron_chrome::tokens::{self, Palette, Rgb};

use backspace_core::prefs::Prefs;

/// The two things a colour can be set for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Sidebar,
    Main,
}

/// `#rrggbb` or `rrggbb`, any case.
pub(crate) fn parse_hex(s: &str) -> Option<Rgb> {
    let h = s.trim().trim_start_matches('#');
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| Rgb::hex(u32::from_str_radix(h, 16).unwrap_or(0)))
}

fn hsl(c: Rgb) -> (f32, f32, f32) {
    let (r, g, b) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    if (mx - mn).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if mx == r { ((g - b) / d).rem_euclid(6.0) } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    (h * 60.0, s, l)
}

fn from_hsl(h: f32, s: f32, l: f32) -> Rgb {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0).floor() as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let q = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(q(r), q(g), q(b))
}

/// Holds `c` inside the theme's range: dark themes keep L 0.03 to 0.26, light ones 0.80 to 1.0.
pub(crate) fn clamp(c: Rgb, dark: bool) -> Rgb {
    let (h, s, l) = hsl(c);
    let (lo, hi) = if dark { (0.03, 0.26) } else { (0.80, 1.0) };
    from_hsl(h, s, l.clamp(lo, hi))
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgb(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// The swatches offered for a target, as `#rrggbb`: the theme's own first, then tints.
pub(crate) fn presets(target: Target, dark: bool) -> [u32; 7] {
    match (target, dark) {
        (Target::Sidebar, true) => [0x0a0a0a, 0x101319, 0x15111c, 0x0e1814, 0x19130f, 0x0c1520, 0x1a1a1a],
        (Target::Main, true) => [0x151515, 0x1a1d25, 0x1d1827, 0x16231d, 0x251c16, 0x131f2c, 0x232323],
        (Target::Sidebar, false) => [0xf7f7f8, 0xeaedf3, 0xf1ebf6, 0xe7f1ec, 0xf5ede6, 0xe6eff7, 0xe4e4e7],
        (Target::Main, false) => [0xffffff, 0xf3f5fa, 0xf7f2fb, 0xf1f8f4, 0xfaf4ee, 0xf1f6fb, 0xf0f0f2],
    }
}

/// What the Settings preview and the shell paint a target with: the saved colour held inside the
/// theme's range, or the theme's own.
pub(crate) fn colour(p: &Prefs, target: Target, dark: bool) -> Rgb {
    let base = if dark { &tokens::DARK } else { &tokens::LIGHT };
    let (saved, own) = match target {
        Target::Sidebar => (p.sidebar_color.as_deref(), base.frame),
        Target::Main => (p.main_color.as_deref(), base.panel),
    };
    saved.and_then(parse_hex).map_or(own, |c| clamp(c, dark))
}

static CACHE: Mutex<Vec<((bool, Rgb, Rgb), &'static Palette)>> = Mutex::new(Vec::new());

/// The theme's palette with the user's two colours in it. Palettes are kept (a handful at most,
/// one per colour pair tried), so the shell can keep handing out `&'static`.
pub(crate) fn themed(dark: bool) -> &'static Palette {
    let base: &'static Palette = if dark { &tokens::DARK } else { &tokens::LIGHT };
    let p = Prefs::load();
    if p.sidebar_color.is_none() && p.main_color.is_none() {
        return base;
    }
    let (side, main) = (colour(&p, Target::Sidebar, dark), colour(&p, Target::Main, dark));
    if (side, main) == (base.frame, base.panel) {
        return base;
    }
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, pal)) = cache.iter().find(|(k, _)| *k == (dark, side, main)) {
        return pal;
    }
    let mut pal = *base;
    pal.bg = side;
    pal.frame = side;
    pal.panel = main;
    // The Shell and the raised surfaces sit a step off the panel, the way the theme's own do.
    let (toward, t) = if dark { (Rgb(255, 255, 255), 0.07) } else { (Rgb(0, 0, 0), 0.07) };
    pal.card = mix(main, toward, t * 0.7);
    pal.raised = mix(main, toward, t);
    let leaked: &'static Palette = Box::leak(Box::new(pal));
    cache.push(((dark, side, main), leaked));
    leaked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn hex_parses_and_colours_stay_inside_the_theme() {
        assert_eq!(parse_hex("#0A0b0C"), Some(Rgb(10, 11, 12)));
        assert_eq!(parse_hex("0a0b0c"), Some(Rgb(10, 11, 12)));
        assert_eq!(parse_hex("#0a0b"), None);
        assert_eq!(parse_hex("#gggggg"), None);
        // White on a dark theme is pulled down; black on a light one is pulled up.
        let dark_white = clamp(Rgb(255, 255, 255), true);
        let light_black = clamp(Rgb(0, 0, 0), false);
        assert!(hsl(dark_white).2 <= 0.261 && hsl(light_black).2 >= 0.799);
        // Text keeps its contrast on the darkest and lightest the range allows.
        assert!(tokens::DARK.fg2.contrast(dark_white) >= 4.5, "{}", tokens::DARK.fg2.contrast(dark_white));
        assert!(tokens::LIGHT.fg2.contrast(light_black) >= 4.5, "{}", tokens::LIGHT.fg2.contrast(light_black));
        // Every swatch is already inside its own range.
        for dark in [true, false] {
            for t in [Target::Sidebar, Target::Main] {
                for v in presets(t, dark) {
                    assert_eq!(clamp(Rgb::hex(v), dark), Rgb::hex(v), "{t:?} {dark} {v:06x}");
                }
            }
        }
    }
}
