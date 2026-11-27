//! Dither planes on the plain background (board round 4, Charles 21 Nov 03:47Z and 03:54Z): a
//! 1-bit field of light dots at 6% over the grey card, in a 4×4 ordered dither, the way the
//! reference he sent is made. Two planes: the New stream page shows his valley picture with an
//! oval cleared behind the title and box (N2); a working stream shows the valley faded in under
//! the chat box (W2), or rings rippling out from the middle (Ripple) when Settings asks.
//! Still draws one frame per card size at the screen's own pixels; Moving cycles a short loop.

use std::sync::{Arc, OnceLock};

use gpui_kit::RenderImage;
use image::{Rgba, RgbaImage};

use backspace_core::prefs::Prefs;

const VALLEY: &[u8] = include_bytes!("../../assets/art/valley.jpg");
/// The dots' strength over the card (the reference sits near 5%).
const ALPHA: f32 = 0.06;
/// Frames in the Moving loop, and how fast it plays.
pub(crate) const FRAMES: usize = 12;
pub(crate) const FRAME_MS: u64 = 125;

const B4: [f32; 16] = [
    0., 8., 2., 10., 12., 4., 14., 6., 3., 11., 1., 9., 15., 7., 13., 5.,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    /// The valley with an oval cleared for the title and box (New stream).
    Clearing,
    /// The valley faded in toward the bottom (a working stream, the default).
    Horizon,
    /// Rings rolling outward (a working stream, from Settings).
    Ripple,
    /// Halftone clouds: dots that swell and shrink with a noise field, clear down the middle where
    /// the text sits (the dark background, Charles 27 Nov, after the mockup).
    Clouds,
}

/// Which plane the main card shows, if any.
pub(crate) fn wanted(p: &Prefs, painting: bool, new_screen: bool) -> Option<Kind> {
    if painting || p.dither.as_deref() == Some("off") {
        return None;
    }
    Some(if new_screen {
        Kind::Clearing
    } else if p.dither_work.as_deref() == Some("ripple") {
        Kind::Ripple
    } else {
        Kind::Horizon
    })
}

pub(crate) fn moving(p: &Prefs) -> bool {
    p.dither_motion.as_deref() == Some("moving")
}

/// The valley as luminance, 0..1, decoded once.
fn valley() -> &'static (u32, u32, Vec<f32>) {
    static V: OnceLock<(u32, u32, Vec<f32>)> = OnceLock::new();
    V.get_or_init(|| match image::load_from_memory(VALLEY) {
        Ok(i) => {
            let g = i.to_luma8();
            let (w, h) = g.dimensions();
            (
                w,
                h,
                g.pixels()
                    .map(|p| (p.0[0] as f32 / 255.0).powf(1.3))
                    .collect(),
            )
        }
        Err(_) => (1, 1, vec![0.0]),
    })
}

fn ring(x: f32, y: f32, w: f32, h: f32, t: f32) -> f32 {
    let dx = (x - w / 2.0) / (w * 0.5);
    let dy = (y - h * 0.52) / (h * 0.5);
    let r = (dx * dx + dy * dy * 1.9).sqrt();
    0.5 + 0.5 * (r * 22.0 - t * 2.2).cos()
}

/// The field's value at a logical point of a `w`×`h` card, 0..1 (how likely a dot is lit).
pub(crate) fn field(kind: Kind, x: f32, y: f32, w: f32, h: f32, t: Option<f32>) -> f32 {
    let (vw, vh, lum) = valley();
    let sample = || {
        // Cover-fit, centred.
        let s = (w / *vw as f32).max(h / *vh as f32);
        let (ox, oy) = ((w - *vw as f32 * s) / 2.0, (h - *vh as f32 * s) / 2.0);
        let (u, v) = (((x - ox) / s) as i64, ((y - oy) / s) as i64);
        let (u, v) = (
            u.clamp(0, *vw as i64 - 1) as usize,
            v.clamp(0, *vh as i64 - 1) as usize,
        );
        lum[v * *vw as usize + u]
    };
    // Moving: a slow ripple drifts through the picture.
    let swell = |v: f32| t.map_or(v, |t| v * (0.82 + 0.18 * ring(x, y, w, h, t)));
    match kind {
        Kind::Clearing => {
            let dx = (x - w / 2.0) / (w * 0.42);
            let dy = (y - h * 0.46) / (h * 0.30);
            let r = (dx * dx + dy * dy).sqrt();
            swell(sample() * ((r - 0.55) / 0.6).clamp(0.0, 1.0))
        }
        Kind::Horizon => swell(sample() * ((y - h * 0.55) / (h * 0.3)).clamp(0.0, 1.0)),
        Kind::Ripple => ring(x, y, w, h, t.unwrap_or(1.3)) * 0.55,
        Kind::Clouds => 0.0,
    }
}


/// Value noise in 0..1 at a lattice point.
fn lattice(i: f64, j: f64) -> f64 {
    let s = (i * 127.1 + j * 311.7).sin() * 43758.5453;
    s - s.floor()
}

fn noise(u: f64, v: f64) -> f64 {
    let (i, j) = (u.floor(), v.floor());
    let (fu, fv) = (u - i, v - j);
    let (a, b, c, d) = (lattice(i, j), lattice(i + 1.0, j), lattice(i, j + 1.0), lattice(i + 1.0, j + 1.0));
    let (su, sv) = (fu * fu * (3.0 - 2.0 * fu), fv * fv * (3.0 - 2.0 * fv));
    a + (b - a) * su + (c - a) * sv + (a - b - c + d) * su * sv
}

fn fbm(mut u: f64, mut v: f64) -> f64 {
    let (mut s, mut a) = (0.0, 0.5);
    for _ in 0..4 {
        s += a * noise(u, v);
        u *= 2.0;
        v *= 2.0;
        a *= 0.5;
    }
    s
}

/// The halftone clouds: a 6-point grid, a square dot in each cell sized and lit by the noise, and
/// none within a band down the middle so the text stays readable. `t` drifts the clouds slowly.
fn render_clouds(w: f32, h: f32, scale: f32, dark: bool, t: Option<f32>) -> RgbaImage {
    const G: f32 = 6.0;
    let (pw, ph) = ((w * scale).ceil().max(1.0) as u32, (h * scale).ceil().max(1.0) as u32);
    let mut img = RgbaImage::new(pw, ph);
    let ink = if dark { [200u8, 202, 220] } else { [20u8, 22, 40] };
    let drift = t.map_or(0.0, |t| f64::from(t) * 0.03);
    let (cols, rows) = ((w / G).ceil() as i32, (h / G).ceil() as i32);
    for gy in 0..rows {
        for gx in 0..cols {
            let (x, y) = (gx as f32 * G, gy as f32 * G);
            let n = fbm(f64::from(x / (w * 0.18)) + drift, f64::from(y / (h * 0.22)));
            let away = (f64::from(x / w) - 0.5).abs() * 2.0;
            let edge = ((away - 0.5) * 2.6).clamp(0.0, 1.0);
            let v = (n - 0.42) * 2.6 * edge;
            if v <= 0.05 {
                continue;
            }
            let half = (v.min(1.0) as f32) * G * 0.36;
            let a = ((0.18 + (v * 0.5).min(0.5)) * 255.0) as u8;
            let (cx, cy) = (x + G / 2.0, y + G / 2.0);
            let (x0, x1) = (((cx - half) * scale).floor().max(0.0) as u32, (((cx + half) * scale).ceil() as u32).min(pw));
            let (y0, y1) = (((cy - half) * scale).floor().max(0.0) as u32, (((cy + half) * scale).ceil() as u32).min(ph));
            for py in y0..y1 {
                for px in x0..x1 {
                    img.put_pixel(px, py, Rgba([ink[0], ink[1], ink[2], a]));
                }
            }
        }
    }
    img
}

/// One plane: `w`×`h` logical points drawn at `scale` pixels each, so a dot is one point and
/// stays crisp at 125% or 150%. `t` is the Moving loop's time, None for Still.
pub(crate) fn render(
    kind: Kind,
    w: f32,
    h: f32,
    scale: f32,
    dark: bool,
    t: Option<f32>,
) -> RgbaImage {
    if kind == Kind::Clouds {
        return render_clouds(w, h, scale, dark, t);
    }
    let (pw, ph) = (
        (w * scale).ceil().max(1.0) as u32,
        (h * scale).ceil().max(1.0) as u32,
    );
    let (lw, lh) = (w.ceil().max(1.0) as u32, h.ceil().max(1.0) as u32);
    // Lit or not per logical point, then spread over the pixels.
    let mut on = vec![false; (lw * lh) as usize];
    for ly in 0..lh {
        for lx in 0..lw {
            let th = (B4[((ly & 3) * 4 + (lx & 3)) as usize] + 0.5) / 16.0;
            on[(ly * lw + lx) as usize] = field(kind, lx as f32, ly as f32, w, h, t) > th;
        }
    }
    let ink = if dark { [255u8, 255, 255] } else { [0u8, 0, 0] };
    let a = (ALPHA * 255.0).round() as u8;
    let mut img = RgbaImage::new(pw, ph);
    for py in 0..ph {
        let ly = ((py as f32 / scale) as u32).min(lh - 1);
        for px in 0..pw {
            let lx = ((px as f32 / scale) as u32).min(lw - 1);
            if on[(ly * lw + lx) as usize] {
                img.put_pixel(px, py, Rgba([ink[0], ink[1], ink[2], a]));
            }
        }
    }
    img
}

/// GPUI wants BGRA.
fn to_render(mut img: RgbaImage) -> RenderImage {
    for p in img.pixels_mut() {
        p.0.swap(0, 2);
    }
    RenderImage::new(smallvec::smallvec![image::Frame::new(img)])
}

/// What a plane was drawn for: kind, size and scale (rounded), theme, Moving.
pub(crate) type Key = (Kind, u32, u32, u32, bool, bool);

pub(crate) fn key(kind: Kind, w: f32, h: f32, scale: f32, dark: bool, moving: bool) -> Key {
    (kind, w.round() as u32, h.round() as u32, (scale * 100.0).round() as u32, dark, moving)
}

/// Draws the frames for `key`: one at the screen's pixels for Still, the loop at one pixel per
/// point for Moving (a third of the memory of 12 full frames). Slow (7-130 ms): call it off the UI
/// thread.
pub(crate) fn frames(kind: Kind, w: f32, h: f32, scale: f32, dark: bool, moving: bool) -> Vec<Arc<RenderImage>> {
    if moving {
        let period = std::f32::consts::TAU / 2.2;
        (0..FRAMES)
            .map(|i| Arc::new(to_render(render(kind, w, h, 1.0, dark, Some(period * i as f32 / FRAMES as f32)))))
            .collect()
    } else {
        vec![Arc::new(to_render(render(kind, w, h, scale, dark, None)))]
    }
}

/// The plane last drawn, by what it was drawn for. Moving keeps a loop of frames. New frames are
/// drawn off the UI thread (`pending`); until they land, the card shows the last ones stretched.
#[derive(Default)]
pub(crate) struct Plane {
    pub(crate) key: Option<Key>,
    pub(crate) frames: Vec<Arc<RenderImage>>,
    /// The frames being drawn, by key.
    pub(crate) pending: Option<(Key, gpui_kit::Task<()>)>,
    /// Images replaced off the UI thread, for the next draw to free from the atlas.
    pub(crate) stale: Vec<Arc<RenderImage>>,
    /// The ticker that redraws the Moving loop.
    pub(crate) ticker: Option<gpui_kit::Task<()>>,
}

impl Plane {
    /// Whether frames for `key` must be drawn: neither shown nor on their way. While the layout
    /// animates (`settling`) a plane of the same kind, theme and motion is kept, stretched, the way
    /// the glass crops are, so an animation does not redraw the plane every frame.
    pub(crate) fn needs(&self, key: Key, settling: bool) -> bool {
        if self.key == Some(key) && !self.frames.is_empty() {
            return false;
        }
        if self.pending.as_ref().is_some_and(|(k, _)| *k == key) {
            return false;
        }
        let same_look = |k: &Key| k.0 == key.0 && k.3 == key.3 && k.4 == key.4 && k.5 == key.5;
        !(settling && !self.frames.is_empty() && self.key.as_ref().is_some_and(same_look))
    }

    /// Takes frames drawn for `key`; the ones they replace go to `stale`.
    pub(crate) fn land(&mut self, key: Key, frames: Vec<Arc<RenderImage>>) {
        if self.pending.as_ref().is_some_and(|(k, _)| *k == key) {
            self.pending = None;
        }
        let old = std::mem::replace(&mut self.frames, frames);
        self.stale.extend(old);
        self.key = Some(key);
    }

    /// The frame to show now, if any is drawn.
    pub(crate) fn current(&self) -> Option<Arc<RenderImage>> {
        let moving = self.key.is_some_and(|k| k.5);
        let n = self.frames.len();
        (n > 0).then(|| self.frames[if moving { (now_ms() / FRAME_MS) as usize % n } else { 0 }].clone())
    }

    pub(crate) fn clear(&mut self, stale: &mut Vec<Arc<RenderImage>>) {
        stale.append(&mut self.frames);
        stale.append(&mut self.stale);
        self.key = None;
        self.pending = None;
        self.ticker = None;
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::{field, render, wanted, Kind};
    use backspace_core::prefs::Prefs;

    #[::core::prelude::v1::test]
    fn the_planes_follow_the_prefs_and_clear_where_the_text_goes() {
        let mut p = Prefs::default();
        p.backdrop = "plain".into();
        assert_eq!(wanted(&p, false, true), Some(Kind::Clearing));
        assert_eq!(
            wanted(&p, false, false),
            Some(Kind::Horizon),
            "W2 is the default (Charles, 21 Nov)"
        );
        p.dither_work = Some("ripple".into());
        assert_eq!(wanted(&p, false, false), Some(Kind::Ripple));
        assert_eq!(wanted(&p, true, false), None, "never over the painting");
        p.dither = Some("off".into());
        assert_eq!(wanted(&p, false, true), None);
        // The middle of the New stream page and the top of a working stream stay clear.
        let (w, h) = (900.0, 640.0);
        assert_eq!(field(Kind::Clearing, w / 2.0, h * 0.46, w, h, None), 0.0);
        assert_eq!(field(Kind::Horizon, w / 2.0, 40.0, w, h, None), 0.0);
        // A dot is one point: at 150% each lit point fills its pixels.
        let img = render(Kind::Ripple, 40.0, 30.0, 1.5, true, None);
        assert_eq!(img.dimensions(), (60, 45));
        let lit = img.pixels().filter(|p| p.0[3] > 0).count();
        assert!(lit > 0 && lit < 60 * 45, "{lit}");
        assert!(
            img.pixels().all(|p| p.0[3] == 0 || p.0[3] == 15),
            "6% dots only"
        );
    }

    #[::core::prelude::v1::test]
    fn a_plane_is_drawn_once_and_kept_while_the_layout_animates() {
        use super::{key, Plane};
        let mut pl = Plane::default();
        let a = key(Kind::Horizon, 900.0, 600.0, 1.0, true, false);
        let b = key(Kind::Horizon, 640.0, 600.0, 1.0, true, false);
        assert!(pl.needs(a, false), "nothing drawn yet");
        pl.land(a, super::frames(Kind::Horizon, 30.0, 20.0, 1.0, true, false));
        assert!(!pl.needs(a, false), "same size: nothing to draw");
        assert!(!pl.needs(b, true), "an animation keeps the last plane, stretched");
        assert!(pl.needs(b, false), "settled at a new size: draw it");
        assert!(pl.needs(key(Kind::Horizon, 900.0, 600.0, 1.0, false, false), true), "a theme change is not an animation");
        assert!(pl.current().is_some());
    }
}
