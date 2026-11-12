//! Design tokens, value for value from the wireframe `:root` (dark) and its light override.
//! `tests/tokens.rs` compares every value with `tests/golden/wireframe.json`.

/// An opaque sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn hex(v: u32) -> Self {
        Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// `#rrggbb`, lower case, as the stylesheet writes it.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// WCAG 2.x relative luminance.
    fn luminance(self) -> f32 {
        fn lin(c: u8) -> f32 {
            let c = c as f32 / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * lin(self.0) + 0.7152 * lin(self.1) + 0.0722 * lin(self.2)
    }

    /// WCAG contrast ratio, 1.0 to 21.0, the same in both directions.
    pub fn contrast(self, other: Rgb) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
}

/// One theme. Surfaces step up from `frame` (the window) to `card`, `raised` and `well`.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Rgb,
    pub frame: Rgb,
    pub card: Rgb,
    pub raised: Rgb,
    pub well: Rgb,
    pub line: Rgb,
    /// The dark band around the New thread box and the selected Code/Design tab.
    pub strip: Rgb,
    pub fg: Rgb,
    pub fg2: Rgb,
    pub fg3: Rgb,
    /// The one accent: focus, the current stage, the New thread button.
    pub accent: Rgb,
    pub on_accent: Rgb,
    pub run: Rgb,
    pub ok: Rgb,
    pub warn: Rgb,
    pub bad: Rgb,
    /// Code threads and tabs.
    pub info: Rgb,
    /// Design threads and tabs.
    pub design: Rgb,
    /// The librarian ("filed by librarian") and memory.
    pub mem: Rgb,
}

pub const DARK: Palette = Palette {
    bg: Rgb::hex(0x09090a),
    frame: Rgb::hex(0x0c0c0d),
    card: Rgb::hex(0x161618),
    raised: Rgb::hex(0x1f1f22),
    well: Rgb::hex(0x252528),
    line: Rgb::hex(0x2e2e32),
    strip: Rgb::hex(0x0f0f10),
    fg: Rgb::hex(0xf4f3f6),
    fg2: Rgb::hex(0xc9c4cf),
    fg3: Rgb::hex(0x8d8794),
    accent: Rgb::hex(0xff4d2d),
    on_accent: Rgb::hex(0x1a0400),
    run: Rgb::hex(0x38c0e8),
    ok: Rgb::hex(0x3fb27f),
    warn: Rgb::hex(0xe0a32a),
    bad: Rgb::hex(0xe05656),
    info: Rgb::hex(0x5b8cf0),
    design: Rgb::hex(0xd9669a),
    mem: Rgb::hex(0x9a82f5),
};

pub const LIGHT: Palette = Palette {
    bg: Rgb::hex(0xe9e8e5),
    frame: Rgb::hex(0xe4e3df),
    card: Rgb::hex(0xf7f7f5),
    raised: Rgb::hex(0xffffff),
    well: Rgb::hex(0xefeeec),
    line: Rgb::hex(0xdcdbd7),
    strip: Rgb::hex(0xecebe8),
    fg: Rgb::hex(0x1c1a1f),
    fg2: Rgb::hex(0x55505b),
    fg3: Rgb::hex(0x68626f),
    accent: Rgb::hex(0xc9330f),
    on_accent: Rgb::hex(0xffffff),
    run: Rgb::hex(0x0f7f9f),
    ok: Rgb::hex(0x2f9e6e),
    warn: Rgb::hex(0xc98a12),
    bad: Rgb::hex(0xd64545),
    info: Rgb::hex(0x3a72e8),
    design: Rgb::hex(0xc23f7e),
    mem: Rgb::hex(0x7a5cf0),
};

/// Agent identity dots (tabs, the + menu), the same in both themes as in the wireframe.
pub const AGENT_DOTS: [Rgb; 9] = [
    Rgb::hex(0x8b5cf6),
    Rgb::hex(0x38c0e8),
    Rgb::hex(0x3fb27f),
    Rgb::hex(0xe0a32a),
    Rgb::hex(0xd9669a),
    Rgb::hex(0x5b8cf0),
    Rgb::hex(0xe05656),
    Rgb::hex(0x9a82f5),
    Rgb::hex(0xe0703a),
];

/// Agent avatars (`crate::avatar`): the figure is the agent's dot colour; this draws the eyes.
/// Not in the wireframe (it has no avatars), so not compared with the golden file.
pub mod avatar {
    use super::Rgb;
    /// The eyes on a filled figure (an outline figure draws them in its own colour).
    pub const EYE: Rgb = Rgb::hex(0x14121a);
}

/// Black drop shadows from the wireframe, the same in both themes.
pub mod shadow {
    /// Offset y, blur and spread in px, and the black's alpha.
    #[derive(Clone, Copy, Debug)]
    pub struct Shadow {
        pub y: f32,
        pub blur: f32,
        pub spread: f32,
        pub alpha: f32,
    }
    /// Menus and popovers (the + menu).
    pub const MENU: Shadow = Shadow { y: 12.0, blur: 32.0, spread: -12.0, alpha: 0.6 };
    /// Hover tips.
    pub const TIP: Shadow = Shadow { y: 6.0, blur: 16.0, spread: 0.0, alpha: 0.3 };
    /// The New thread box (`.nt-box`).
    pub const BOX: Shadow = Shadow { y: 16.0, blur: 32.0, spread: -18.0, alpha: 0.65 };
}

/// One radius family, anchored on the folder tab. Circles are drawn as circles, not as a radius.
pub mod radius {
    /// Cards, tabs, the composer, the New thread box.
    pub const R: f32 = 6.0;
    /// Buttons, chips, rows.
    pub const R1: f32 = 4.0;
    /// Anything inside a padded box.
    pub const R2: f32 = 3.0;
    /// The window itself.
    pub const WINDOW: f32 = R + 4.0;
    /// The New thread box: R around its 4px padding, so the body inside keeps R (wireframe `.nt-box`).
    pub const BOX: f32 = R + 4.0;

    /// Inner radius of a box with `padding` inside an `outer` radius, never negative.
    pub fn nested(outer: f32, padding: f32) -> f32 {
        (outer - padding).max(0.0)
    }
}

/// Motion: how one state becomes the next. Every transition is 200ms or less and lands where
/// the state change alone would put it; with reduced motion it simply cuts.
pub mod motion {
    /// The cap (hard rule).
    pub const MAX: f32 = 200.0;
    /// Menus and popovers, in ms.
    pub const MENU: f32 = 150.0;
    /// Indicators that travel between controls (a selected mark, a highlight).
    pub const INDICATOR: f32 = 180.0;
    /// Panes and shared elements moving between screens.
    pub const BASE: f32 = 200.0;
    /// Tooltips.
    pub const TIP: f32 = 120.0;
    /// Gap between staggered items.
    pub const STAGGER: f32 = 25.0;
    /// How far a tooltip or menu rises as it appears, in px.
    pub const RISE: f32 = 4.0;

    /// cubic-bezier(.32,.72,0,1) at time `t` in 0..=1.
    pub fn ease(t: f32) -> f32 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        let bez = |s: f32, a: f32, b: f32| 3.0 * (1.0 - s) * (1.0 - s) * s * a + 3.0 * (1.0 - s) * s * s * b + s * s * s;
        // x(s) is monotonic for these control points, so bisection finds s for x = t.
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if bez(mid, 0.32, 0.0) < t { lo = mid } else { hi = mid }
        }
        bez((lo + hi) / 2.0, 0.72, 1.0).clamp(0.0, 1.0)
    }

    /// When item `i` of a staggered group starts and ends inside `total` ms: each starts
    /// STAGGER after the one before, at most half way in, and all end together.
    pub fn stagger(i: usize, total: f32) -> (f32, f32) {
        ((i as f32 * STAGGER).min(total / 2.0), total)
    }
}
