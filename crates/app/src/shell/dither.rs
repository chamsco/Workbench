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
    }
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

/// The plane last drawn, by what it was drawn for. Moving keeps a loop of frames.
#[derive(Default)]
pub(crate) struct Plane {
    key: Option<(Kind, u32, u32, u32, bool, bool)>,
    frames: Vec<Arc<RenderImage>>,
    /// The ticker that redraws the Moving loop.
    pub(crate) ticker: Option<gpui_kit::Task<()>>,
}

impl Plane {
    /// The image for this card, drawing it when the card's size, the theme or the plane changed.
    /// Images it no longer needs go to `stale`, for the window to drop.
    pub(crate) fn get(
        &mut self,
        kind: Kind,
        w: f32,
        h: f32,
        scale: f32,
        dark: bool,
        moving: bool,
        stale: &mut Vec<Arc<RenderImage>>,
    ) -> Arc<RenderImage> {
        let key = (
            kind,
            w.round() as u32,
            h.round() as u32,
            (scale * 100.0).round() as u32,
            dark,
            moving,
        );
        if self.key != Some(key) || self.frames.is_empty() {
            stale.append(&mut self.frames);
            self.key = Some(key);
            self.frames = if moving {
                // The loop at one pixel per point (a third of the memory of 12 full frames).
                let period = std::f32::consts::TAU / 2.2;
                (0..FRAMES)
                    .map(|i| {
                        Arc::new(to_render(render(
                            kind,
                            w,
                            h,
                            1.0,
                            dark,
                            Some(period * i as f32 / FRAMES as f32),
                        )))
                    })
                    .collect()
            } else {
                vec![Arc::new(to_render(render(kind, w, h, scale, dark, None)))]
            };
        }
        let i = if moving {
            (now_ms() / FRAME_MS) as usize % self.frames.len()
        } else {
            0
        };
        self.frames[i].clone()
    }

    pub(crate) fn clear(&mut self, stale: &mut Vec<Arc<RenderImage>>) {
        stale.append(&mut self.frames);
        self.key = None;
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
}
