//! Agent avatars, the figure Charles picked as A (design/cauldron-avatars-and-thread.html on
//! claude/project-thread-gg3ogp): a flat head circle over a body, in the agent's dot colour, at
//! every size. Only the face carries the mood; working turns the head into a cog and typing into
//! three small heads. Moods and the cog outline follow orbling (github.com/kochjar/orbling, MIT).
//!
//! Geometry is in the sheet's 100-unit box; the drawn box adds headroom above for the marks.

use crate::tokens::{avatar, Rgb, AGENT_DOTS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Idle,
    Thinking,
    Reading,
    Typing,
    Working,
    Asking,
    Waiting,
    Watching,
    Surprised,
    Sleepy,
    Error,
}

pub const MOODS: [Mood; 11] = [
    Mood::Idle,
    Mood::Thinking,
    Mood::Reading,
    Mood::Typing,
    Mood::Working,
    Mood::Asking,
    Mood::Waiting,
    Mood::Watching,
    Mood::Surprised,
    Mood::Sleepy,
    Mood::Error,
];

/// How to draw: `px` is the figure's height as on the sheet's size ladder (its 96-unit box), `ms`
/// the motion clock (None under reduced motion: a still frame with the marks showing).
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub px: f32,
    pub outline: bool,
    pub ms: Option<u64>,
    /// Head and the top of the shoulders only (the chat status line): `px` is then the square slot
    /// (the sheet's 22 9 56 56); the image is bigger, with room for the marks, see `bust_frame`.
    pub bust: bool,
    /// Draw in this colour instead of the agent's dot (the status line uses the step-line grey).
    pub ink: Option<Rgb>,
}

// The figure (sheet PROPS.now): head radius and centre, body radius and centre; the body is cut at y 100.
const HR: f32 = 22.0;
const HCY: f32 = 33.0;
const BR: f32 = 33.0;
const BCY: f32 = 92.0;
/// The sheet's box, and the same raised to y -12 so the marks fit (used from MARKS_FROM up).
const SHEET: (f32, f32, f32, f32) = (10.0, 4.0, 80.0, 96.0);
const TALL: (f32, f32, f32, f32) = (10.0, -12.0, 80.0, 112.0);
/// The bust's slot (the sheet's crop), and the drawn box: the slot plus room up and right for the marks.
const BUST: (f32, f32, f32, f32) = (18.0, 7.0, 64.0, 64.0);
const BUST_DRAW: (f32, f32, f32, f32) = (18.0, -18.0, 72.0, 89.0);
/// Marks on the bust from this slot size up.
pub const BUST_MARKS_FROM: f32 = 14.0;
/// Marks (?, !, z) only from this height up.
pub const MARKS_FROM: f32 = 28.0;

/// Orbling's cog outline (MIT) in a 100-unit box centred on 50,50, with a hole cut in the middle.
const GEAR: &str = "M39.23 15.65 L41.41 2.27 L58.59 2.27 L60.77 15.65 A36 36 0 0 1 74.37 23.5 L87.04 18.7 L95.63 33.57 L85.13 42.15 A36 36 0 0 1 85.13 57.85 L95.63 66.43 L87.04 81.3 L74.37 76.5 A36 36 0 0 1 60.77 84.35 L58.59 97.73 L41.41 97.73 L39.23 84.35 A36 36 0 0 1 25.63 76.5 L12.96 81.3 L4.37 66.43 L14.87 57.85 A36 36 0 0 1 14.87 42.15 L4.37 33.57 L12.96 18.7 L25.63 23.5 A36 36 0 0 1 39.23 15.65 Z M64 50 A14 14 0 1 0 36 50 A14 14 0 1 0 64 50 Z";

fn view(px: f32) -> (f32, f32, f32, f32) {
    if px >= MARKS_FROM {
        TALL
    } else {
        SHEET
    }
}

/// The drawn width and height for figure height `px`: taller than `px` when there is room for marks.
pub fn size(px: f32) -> (f32, f32) {
    let (_, _, w, h) = view(px);
    (px * w / SHEET.3, px * h / SHEET.3)
}

/// A bust in a `px` square slot: the image's width, height, and how far its top sits above the
/// slot's top (place it absolutely at left 0, top -that, so the marks overhang the slot).
pub fn bust_frame(px: f32) -> (f32, f32, f32) {
    let k = px / BUST.3;
    (BUST_DRAW.2 * k, BUST_DRAW.3 * k, (BUST.1 - BUST_DRAW.1) * k)
}

/// The cog turns once every 3.8s.
pub fn cog_angle(ms: u64) -> f32 {
    (ms % 3800) as f32 / 3800.0 * 360.0
}

/// Small head `i` of typing: hops 6 units every 1.2s, each 150ms after the one before.
pub fn hop(i: usize, ms: u64) -> f32 {
    let p = (ms as i64 - i as i64 * 150).rem_euclid(1200) as f32 / 1200.0;
    let s = |x: f32| x * x * (3.0 - 2.0 * x);
    if p < 0.3 {
        -6.0 * s(p / 0.3)
    } else if p < 0.6 {
        -6.0 * (1.0 - s((p - 0.3) / 0.3))
    } else {
        0.0
    }
}

/// The eyes' pose per mood, at the peak of the sheet's keyframes: offset, then scale about each eye.
// ponytail: still poses; the sheet's wander, blink and squint loops come with the animated status line.
fn pose(m: Mood) -> (f32, f32, f32, f32) {
    match m {
        Mood::Thinking => (-3.0, -3.0, 1.06, 0.78),
        Mood::Reading => (-3.0, 0.8, 1.0, 1.0),
        Mood::Asking => (3.2, -3.0, 1.0, 1.0),
        Mood::Waiting => (1.6, 4.0, 1.0, 1.0),
        Mood::Surprised => (0.0, -1.4, 1.3, 1.3),
        Mood::Sleepy => (0.0, 1.9, 1.6, 0.42),
        Mood::Error => (0.0, 2.0, 1.1, 0.55),
        _ => (0.0, 0.0, 1.0, 1.0),
    }
}

/// The agent with dot `dot` in `mood`, as an SVG.
// ponytail: a string per frame; the animated status line should paint these shapes directly
// instead of rasterising a new image every frame.
pub fn svg(dot: usize, mood: Mood, look: Look) -> String {
    let col = look.ink.unwrap_or(AGENT_DOTS[dot % AGENT_DOTS.len()]).to_hex();
    let (vx, vy, vw, vh) = if look.bust { BUST_DRAW } else { view(look.px) };
    // About 1.4px on the bust, 1.7px on the figure, never under 2.4 units.
    let sw = if look.bust { 1.4 * BUST.3 / look.px } else { 1.7 * SHEET.3 / look.px }.max(2.4);
    let paint = if look.outline {
        format!(r#"fill="none" stroke="{col}" stroke-width="{sw:.2}" stroke-linejoin="round" stroke-linecap="round""#)
    } else {
        format!(r#"fill="{col}""#)
    };
    let (w, h) = if look.bust { let (w, h, _) = bust_frame(look.px); (w, h) } else { size(look.px) };
    let mut out = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vx} {vy} {vw} {vh}" width="{w:.1}" height="{h:.1}">"#);

    let dx = (BR * BR - (100.0 - BCY).powi(2)).sqrt();
    let close = if look.outline { "" } else { "Z" };
    out += &format!(r#"<path class="body" d="M{:.2} 100 A{BR} {BR} 0 1 1 {:.2} 100{close}" {paint}/>"#, 50.0 - dx, 50.0 + dx);

    let r = HR - if look.outline { sw / 2.0 } else { 0.0 };
    match mood {
        Mood::Working => {
            let gs = HR * 1.08 / 47.7;
            let a = look.ms.map_or(0.0, cog_angle);
            let gp = if look.outline {
                format!(r#"fill="none" stroke="{col}" stroke-width="{:.2}" stroke-linejoin="round""#, sw / gs)
            } else {
                format!(r#"fill="{col}" fill-rule="evenodd""#)
            };
            out += &format!(r#"<path class="cog" transform="translate(50 {HCY}) rotate({a:.2}) scale({gs:.3}) translate(-50 -50)" d="{GEAR}" {gp}/>"#);
        }
        Mood::Typing => {
            let (mr, gap) = (HR * 0.33, HR * 0.33 * 2.2);
            for (i, x) in [-gap, 0.0, gap].into_iter().enumerate() {
                let y = HCY + look.ms.map_or(0.0, |ms| hop(i, ms));
                let mr = mr - if look.outline { sw / 2.0 } else { 0.0 };
                out += &format!(r#"<circle class="mini" cx="{:.2}" cy="{y:.2}" r="{mr:.2}" {paint}/>"#, 50.0 + x);
            }
        }
        _ => {
            out += &format!(r#"<circle class="head" cx="50" cy="{HCY}" r="{r:.2}" {paint}/>"#);
            out += &eyes(mood, look, &col);
        }
    }

    if look.bust && look.px >= BUST_MARKS_FROM {
        out += &mark(mood, &col, true);
    } else if !look.bust && look.px >= MARKS_FROM {
        out += &mark(mood, &col, false);
    }
    out + "</svg>"
}

/// Two pill eyes, 1.25x bigger below 40px so they still read.
fn eyes(mood: Mood, look: Look, col: &str) -> String {
    let fill = if look.outline { col.to_string() } else { avatar::EYE.to_hex() };
    let k = if look.bust { 1.1 } else if look.px < 40.0 { 1.25 } else { 1.0 };
    let d = 2.0 * HR;
    let (w, h) = (0.10 * d * k, 0.24 * d * k);
    let c = 0.14 * d / 2.0 + w / 2.0;
    let (ox, oy, sx, sy) = pose(mood);
    let (w, h) = (w * sx, h * sy);
    let cy = HCY - 0.9 + oy;
    [-c, c]
        .iter()
        .map(|x| {
            let cx = 50.0 + x + ox;
            format!(
                r#"<rect class="eye" x="{:.2}" y="{:.2}" width="{w:.2}" height="{h:.2}" rx="{:.2}" fill="{fill}"/>"#,
                cx - w / 2.0,
                cy - h / 2.0,
                w.min(h) / 2.0
            )
        })
        .collect()
}

/// The mark beside the head: ? asking, ! surprised, z's sleepy. Drawn whole (no draw-on), as paths.
/// On the bust they are about 1.5x bigger and placed clear of the head (sheet 5742272).
fn mark(mood: Mood, col: &str, bust: bool) -> String {
    let (q, b) = if bust { ("translate(61 -13) rotate(10) scale(.228)", "translate(68 -10) scale(.285)") } else { ("translate(62 -7) rotate(10) scale(.135)", "translate(68 -8) scale(.17)") };
    match mood {
        Mood::Asking => format!(
            r#"<g class="mark" transform="{q}"><path d="M24 40 C24 18 40 6 58 6 C78 6 92 20 92 38 C92 56 78 64 68 72 C61 78 58 84 58 96" fill="none" stroke="{col}" stroke-width="24" stroke-linecap="round" stroke-linejoin="round"/><circle cx="58" cy="124" r="13" fill="{col}"/></g>"#
        ),
        Mood::Surprised => format!(
            r#"<g class="mark" transform="{b}"><path d="M20 4 C27 4 31 9 30 16 L26 62 C25.5 67 23 69 20 69 C17 69 14.5 67 14 62 L10 16 C9 9 13 4 20 4 Z" fill="{col}"/><circle cx="20" cy="87" r="9" fill="{col}"/></g>"#
        ),
        Mood::Sleepy => {
            // Three z's, each smaller and higher, apart so they read as three.
            let z = |x: f32, y: f32, s: f32| format!("M{x} {} h{s} l-{s} {s} h{s}", y - s);
            let zs = if bust { [z(64.0, 15.0, 10.3), z(72.0, 3.0, 8.6), z(79.0, -8.0, 6.8)] } else { [z(65.0, 17.0, 7.0), z(75.0, 7.0, 5.5), z(83.0, -1.0, 4.0)] };
            let w = if bust { 3.65 } else { 2.4 };
            format!(
                r#"<path class="mark" d="{} {} {}" fill="none" stroke="{col}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                zs[0], zs[1], zs[2]
            )
        }
        _ => String::new(),
    }
}
