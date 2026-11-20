//! Design tokens, value for value from the design system (`tests/golden/design-system.json`,
//! design/cauldron-design-system/tokens.json at 45d2432). `tests/tokens.rs` compares them.

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

/// A colour laid over whatever is beneath it: sRGB plus alpha 0..=1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub Rgb, pub f32);

impl Rgba {
    pub const fn new(v: u32, a: f32) -> Self {
        Rgba(Rgb::hex(v), a)
    }

    /// The solid colour this makes over `surface` (source-over in sRGB, as a browser paints it).
    pub fn over(self, surface: Rgb) -> Rgb {
        let m = |c: u8, s: u8| (c as f32 * self.1 + s as f32 * (1.0 - self.1)).round() as u8;
        Rgb(m(self.0 .0, surface.0), m(self.0 .1, surface.1), m(self.0 .2, surface.2))
    }

    /// `rgba(r,g,b,a)` as tokens.json writes it.
    pub fn to_css(self) -> String {
        format!("rgba({},{},{},{})", self.0 .0, self.0 .1, self.0 .2, self.1)
    }
}

impl From<Rgb> for Rgba {
    fn from(c: Rgb) -> Self {
        Rgba(c, 1.0)
    }
}

/// One theme, value for value from the design system's tokens.json (Tuff surfaces, purple
/// accent). The window is `bg`; a Shell (`card`) holds a Panel (`panel`); menus are `raised`.
/// `inset`, `hover`, `selected`, `line` and `line_strong` are overlays on any surface.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Rgb,
    pub frame: Rgb,
    pub card: Rgb,
    pub panel: Rgb,
    pub raised: Rgb,
    pub inset: Rgba,
    pub hover: Rgba,
    pub selected: Rgba,
    pub line: Rgba,
    pub line_strong: Rgba,
    /// Edges that carry meaning (an off toggle, a checkbox): 3:1 on bg, card and panel.
    pub line_control: Rgb,
    pub fg: Rgb,
    pub fg2: Rgb,
    pub fg3: Rgb,
    /// Disabled labels and the idle dot only; never text someone must read.
    pub fg4: Rgb,
    /// The one accent: primary button, focus ring, needs-you dot, current stage.
    pub accent: Rgb,
    pub accent_hover: Rgb,
    pub on_accent: Rgb,
    pub accent_soft: Rgba,
    pub ok: Rgb,
    pub warn: Rgb,
    pub bad: Rgb,
    pub bad_soft: Rgba,
}

pub const DARK: Palette = Palette {
    bg: Rgb::hex(0x111111),
    frame: Rgb::hex(0x111111),
    card: Rgb::hex(0x1d1d1d),
    panel: Rgb::hex(0x151515),
    raised: Rgb::hex(0x282828),
    inset: Rgba::new(0xf5f5f5, 0.05),
    hover: Rgba::new(0xf5f5f5, 0.05),
    selected: Rgba::new(0xf5f5f5, 0.09),
    line: Rgba::new(0xffffff, 0.06),
    line_strong: Rgba::new(0xffffff, 0.12),
    line_control: Rgb::hex(0x7c7c82),
    fg: Rgb::hex(0xf5f5f5),
    fg2: Rgb::hex(0xc9c9cd),
    fg3: Rgb::hex(0x9e9ea3),
    fg4: Rgb::hex(0x5e5e64),
    accent: Rgb::hex(0xa08cff),
    accent_hover: Rgb::hex(0xb4a4ff),
    on_accent: Rgb::hex(0x12082e),
    accent_soft: Rgba::new(0xa08cff, 0.14),
    ok: Rgb::hex(0x30d158),
    warn: Rgb::hex(0xff9f0a),
    bad: Rgb::hex(0xff453a),
    bad_soft: Rgba::new(0xff453a, 0.12),
};

pub const LIGHT: Palette = Palette {
    bg: Rgb::hex(0xf7f7f8),
    frame: Rgb::hex(0xf7f7f8),
    card: Rgb::hex(0xeeeef0),
    panel: Rgb::hex(0xffffff),
    raised: Rgb::hex(0xe4e4e7),
    inset: Rgba::new(0x1c1c1f, 0.04),
    hover: Rgba::new(0x1c1c1f, 0.05),
    selected: Rgba::new(0x1c1c1f, 0.09),
    line: Rgba::new(0x1c1c1f, 0.07),
    line_strong: Rgba::new(0x1c1c1f, 0.14),
    line_control: Rgb::hex(0x7e7e86),
    fg: Rgb::hex(0x1c1c1f),
    fg2: Rgb::hex(0x45454c),
    fg3: Rgb::hex(0x62626a),
    fg4: Rgb::hex(0xa6a6ad),
    accent: Rgb::hex(0x6236d6),
    accent_hover: Rgb::hex(0x5229bd),
    on_accent: Rgb::hex(0xffffff),
    accent_soft: Rgba::new(0x6236d6, 0.08),
    ok: Rgb::hex(0x23784f),
    warn: Rgb::hex(0x8a5a00),
    bad: Rgb::hex(0xb83232),
    bad_soft: Rgba::new(0xb83232, 0.08),
};

/// Agent identity colours `agent-1` to `agent-9`, the same in both themes.
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
/// Not in tokens.json, so not compared with the golden file.
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

/// Tuff radii from tokens.json. A box nests by its padding: outer = inner + padding (a Shell's
/// 6px around a Panel). Circles are drawn as circles, not as a radius.
pub mod radius {
    /// The Shell, the window, the composer shell.
    pub const SHELL: f32 = 22.0;
    /// The Panel in a shell; cards, menus, dialogs.
    pub const PANEL: f32 = 16.0;
    /// The Inset well, tool rows, the segmented track.
    pub const INSET: f32 = 12.0;
    /// Buttons, inputs, thread rows.
    pub const CONTROL: f32 = 10.0;
    /// Segments, icon tiles, anything inside a padded inset.
    pub const INNER: f32 = 8.0;
    /// Tags and Kbd.
    pub const TAG: f32 = 6.0;
    /// A Shell's padding around its Panel.
    pub const SHELL_PAD: f32 = SHELL - PANEL;

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
