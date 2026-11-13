//! The painting's grain: an ordered (Bayer 8x8) dither of a picture's lightness, strongest on edges
//! and in gradients, faint in flat areas, dissolving dot by dot toward its foot. Ported line for line
//! from the Tauri app's `Art.band` (legacy/tauri-app/ui/art.js), itself ported from Berth's
//! DitherBand (MIT, Sean Brydon; see THIRD_PARTY_NOTICES.md). Pure arithmetic on an RGBA buffer, so
//! the shell decodes, scales and caches, and this is tested on any machine.

/// The 8x8 Bayer matrix, each level centred in its cell: `(v + 0.5) / 64`.
pub const BAYER8: [f32; 64] = {
    const M: [u8; 64] = [
        0, 32, 8, 40, 2, 34, 10, 42, 48, 16, 56, 24, 50, 18, 58, 26, 12, 44, 4, 36, 14, 46, 6, 38, 60, 28, 52, 20, 62, 30, 54, 22, 3, 35, 11, 43, 1, 33, 9, 41,
        51, 19, 59, 27, 49, 17, 57, 25, 15, 47, 7, 39, 13, 45, 5, 37, 63, 31, 55, 23, 61, 29, 53, 21,
    ];
    let mut out = [0.0f32; 64];
    let mut i = 0;
    while i < 64 {
        out[i] = (M[i] as f32 + 0.5) / 64.0;
        i += 1;
    }
    out
};

/// How a painting is drawn into a card.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// Lightness levels.
    pub levels: u32,
    /// The share of the height, from the foot, over which it dissolves.
    pub fade: f32,
    /// How far each pixel moves toward the card colour first.
    pub mute: f32,
    /// Screen pixels per dither pixel.
    pub cell: u32,
    /// The painting is drawn at this share of the frame, then scaled up, so its brushwork softens and
    /// the grain is the dither's.
    pub soft: f32,
    /// Where the painting sits when it is cropped top and bottom: 0 top, 1 bottom.
    pub position: f32,
}

impl Band {
    /// The board's settings (88454ee): 6 levels, fade 0.42, cell 2, drawn at 0.6, muted toward the
    /// card 0.3 in dark (the night, so its stars stay calm) and 0.07 in light.
    pub fn board(dark: bool) -> Band {
        Band { levels: 6, fade: 0.42, mute: if dark { 0.3 } else { 0.07 }, cell: 2, soft: 0.6, position: 0.5 }
    }
}

/// Where a `iw` x `ih` picture lands to cover a `w` x `h` frame: (x, y, width, height), centred
/// across and `position` of the way down.
pub fn cover(iw: u32, ih: u32, w: u32, h: u32, position: f32) -> (f32, f32, f32, f32) {
    let scale = (w as f32 / iw as f32).max(h as f32 / ih as f32);
    let (dw, dh) = (iw as f32 * scale, ih as f32 * scale);
    ((w as f32 - dw) / 2.0, (h as f32 - dh) * position, dw, dh)
}

fn clamp8(v: f32) -> u8 {
    // A canvas's Uint8ClampedArray rounds half to even; round() is within a level of it.
    v.round().clamp(0.0, 255.0) as u8
}

/// Dithers `rgba` (`w` x `h`, straight alpha) in place, as `Art.band`'s `dither()`: mute toward
/// `bg`, then per pixel either dissolve it (alpha 0, toward the foot) or add the Bayer step of its
/// lightness, scaled by how much edge is around it.
pub fn dither(rgba: &mut [u8], w: usize, h: usize, band: &Band, bg: [u8; 3]) {
    assert_eq!(rgba.len(), w * h * 4);
    let l = band.levels.max(2) as f32 - 1.0;
    let from = h as f32 * (1.0 - band.fade);
    let mut lum = vec![0f32; w * h];
    for p in 0..w * h {
        let i = p * 4;
        if band.mute > 0.0 {
            for c in 0..3 {
                let v = rgba[i + c] as f32;
                rgba[i + c] = clamp8(v + (bg[c] as f32 - v) * band.mute);
            }
        }
        lum[p] = 0.299 * rgba[i] as f32 + 0.587 * rgba[i + 1] as f32 + 0.114 * rgba[i + 2] as f32;
    }
    for y in 0..h {
        let mut keep = 1.0f32;
        if y as f32 > from {
            let t = 1.0 - (y as f32 - from) / (h as f32 - from);
            keep = t * t * (3.0 - 2.0 * t);
        }
        let row = (y & 7) * 8;
        let row_b = ((y + 3) & 7) * 8;
        let up = y.saturating_sub(1) * w;
        let down = (y + 1).min(h - 1) * w;
        for x in 0..w {
            let p = y * w + x;
            let i = p * 4;
            if BAYER8[row_b + ((x + 5) & 7)] >= keep {
                rgba[i + 3] = 0;
                continue;
            }
            let lv = lum[p];
            let edge = (lum[y * w + (x + 1).min(w - 1)] - lum[y * w + x.saturating_sub(1)]).abs() + (lum[down + x] - lum[up + x]).abs();
            let amount = (0.35 + edge / 40.0).min(1.0);
            let d = (((lv / 255.0) * l + BAYER8[row + (x & 7)]).floor() / l * 255.0 - lv) * amount;
            for c in 0..3 {
                rgba[i + c] = clamp8(rgba[i + c] as f32 + d);
            }
            rgba[i + 3] = 255;
        }
    }
}
