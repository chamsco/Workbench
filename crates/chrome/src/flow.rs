//! The moving background, S1 "Flow" (the coordinator's shader board b2328bb; Charles, 13 Nov
//! 18:49Z: "Shader build i love it"). A warped glow rising from the bottom right: 3D value noise in
//! fractal octaves, warped by itself, under a smooth glow, quantised to six levels of the theme's
//! violet ramp with the 8x8 Bayer pattern, one level per 3px cell. Ported from the board's
//! JavaScript (`fieldAt`, `frameS1`, `snapS1`) so the app moves as the board does.
//!
//! Pure and on the CPU: the app asks for the cells under one rect at one time and scales them up
//! with hard pixels. The frosted copy is the same field on a 100x60 grid, dimmed and smooth, so the
//! glass needs no blur pass.

use crate::dither::BAYER8;

/// One cell, in window pixels.
pub const CELL: u32 = 3;
/// The levels of the ramp the dither picks from.
pub const LEVELS: usize = 6;
/// The frosted copy's grid.
pub const FROST_W: u32 = 100;
pub const FROST_H: u32 = 60;

/// The ramp, darkest first: violet on near-black in dark, lilac on white in light (the tokens'
/// accents #a08cff and #6236d6 sit on it).
pub fn ramp(dark: bool) -> [[u8; 3]; LEVELS] {
    if dark {
        [[0x0e, 0x0e, 0x12], [0x1b, 0x13, 0x40], [0x3a, 0x23, 0x91], [0x6a, 0x45, 0xe0], [0xa0, 0x8c, 0xff], [0xe6, 0xe0, 0xff]]
    } else {
        [[0xf7, 0xf7, 0xf8], [0xef, 0xe9, 0xfe], [0xd9, 0xcd, 0xfb], [0xb9, 0xa6, 0xf5], [0x8d, 0x6d, 0xf0], [0x62, 0x36, 0xd6]]
    }
}

fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263) ^ z.wrapping_mul(1_442_695_041);
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(1_274_126_177);
    h ^= ((h as u32) >> 16) as i32;
    (h as u32) as f32 / 4_294_967_296.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Value noise in 3D, 0..1.
fn vn(x: f32, y: f32, z: f32) -> f32 {
    let (xi, yi, zi) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let (u, v, w) = (smooth(x - xi as f32), smooth(y - yi as f32), smooth(z - zi as f32));
    let plane = |z: i32| lerp(lerp(hash(xi, yi, z), hash(xi + 1, yi, z), u), lerp(hash(xi, yi + 1, z), hash(xi + 1, yi + 1, z), u), v);
    lerp(plane(zi), plane(zi + 1), w)
}

/// Three octaves, 0..1.
fn fbm(x: f32, y: f32, z: f32) -> f32 {
    let (mut a, mut s, mut f) = (0.5, 0.0, 1.0);
    for _ in 0..3 {
        s += a * vn(x * f, y * f, z * f);
        f *= 2.03;
        a *= 0.5;
    }
    s / 0.875
}

/// The glow's weight at (u, v), the window's 0..1: strongest at the bottom right.
pub fn glow(u: f32, v: f32) -> f32 {
    let (dx, dy) = (1.02 - u, (1.05 - v) * 1.25);
    let g = (1.0 - (dx * dx + dy * dy).sqrt() / 1.05).max(0.0);
    smooth(g)
}

/// The warped noise at (u, v) and time `t` seconds, before the glow: the field is `g * flow`.
fn flow(u: f32, v: f32, t: f32) -> f32 {
    let w1 = fbm(u * 1.6 + t * 0.04, v * 1.6 - t * 0.03, t * 0.12);
    let w2 = fbm(u * 2.6 + w1 * 1.6, v * 2.6 + w1 * 1.4, t * 0.09);
    0.25 + 1.25 * w2 + (w1 - 0.5) * 0.12
}

/// The field at (u, v) and time `t` seconds, under glow `g`: 0..1.
pub fn field(u: f32, v: f32, t: f32, g: f32) -> f32 {
    (g * flow(u, v, t)).clamp(0.0, 1.0)
}

/// The noise is worked out every `STEP` cells and blended between: its finest wave is about 100px
/// across, so 12px between samples does not show, and it costs a sixteenth.
const STEP: i32 = 4;

/// The cells under the rect (`x`, `y`, `w`, `h`) of a `win_w` x `win_h` window at time `t`, as
/// RGBA, one pixel per cell (`w / CELL` by `h / CELL`, rounded up), row by row. A cell on the
/// lowest level is transparent, so the card shows through, as on the board. The pattern is fixed
/// to the window, so two rects cut from it line up.
pub fn cells(win_w: f32, win_h: f32, x: f32, y: f32, w: u32, h: u32, t: f32, dark: bool) -> (u32, u32, Vec<u8>) {
    let r = ramp(dark);
    let (gw, gh) = (w.div_ceil(CELL).max(1), h.div_ceil(CELL).max(1));
    // The cell grid starts at the window's corner, so a rect's cells are whole window cells.
    let (cx0, cy0) = ((x / CELL as f32).floor() as i32, (y / CELL as f32).floor() as i32);
    // The samples sit on window cells that are multiples of STEP, so rects agree with the window.
    let (sx0, sy0) = (cx0.div_euclid(STEP), cy0.div_euclid(STEP));
    let (sw, sh) = (((cx0 + gw as i32 - 1).div_euclid(STEP) - sx0 + 2) as usize, ((cy0 + gh as i32 - 1).div_euclid(STEP) - sy0 + 2) as usize);
    let at = |c: i32, size: f32| (c as f32 * CELL as f32) / size;
    let mut samples = vec![0f32; sw * sh];
    for b in 0..sh {
        let v = at((sy0 + b as i32) * STEP, win_h);
        for a in 0..sw {
            let u = at((sx0 + a as i32) * STEP, win_w);
            // Far from the glow nothing is drawn; skip the noise there.
            samples[b * sw + a] = if glow(u, v) < 0.005 && glow(at((sx0 + a as i32 + 1) * STEP, win_w), at((sy0 + b as i32 + 1) * STEP, win_h)) < 0.005 { 0.0 } else { flow(u, v, t) };
        }
    }
    let mut out = vec![0u8; (gw * gh * 4) as usize];
    for j in 0..gh {
        let wy = cy0 + j as i32;
        let v = at(wy, win_h);
        let row = (wy.rem_euclid(8) * 8) as usize;
        let (b, fy) = ((wy.div_euclid(STEP) - sy0) as usize, wy.rem_euclid(STEP) as f32 / STEP as f32);
        for i in 0..gw {
            let wx = cx0 + i as i32;
            let u = at(wx, win_w);
            let g = glow(u, v);
            if g < 0.02 {
                continue;
            }
            let (a, fx) = ((wx.div_euclid(STEP) - sx0) as usize, wx.rem_euclid(STEP) as f32 / STEP as f32);
            let s00 = samples[b * sw + a];
            let s10 = samples[b * sw + a + 1];
            let s01 = samples[(b + 1) * sw + a];
            let s11 = samples[(b + 1) * sw + a + 1];
            let n = lerp(lerp(s00, s10, fx), lerp(s01, s11, fx), fy);
            let f = (g * n).clamp(0.0, 1.0);
            let q = ((f * (LEVELS - 1) as f32 + BAYER8[row + wx.rem_euclid(8) as usize]).floor() as usize).min(LEVELS - 1);
            if q == 0 {
                continue;
            }
            let p = ((j * gw + i) * 4) as usize;
            out[p..p + 3].copy_from_slice(&r[q]);
            out[p + 3] = 255;
        }
    }
    (gw, gh, out)
}

/// The frosted copy at time `t`: the field on the `FROST_W` x `FROST_H` grid, dimmed (about half,
/// so text on the glass never meets the glow's brightest part) and smooth along the ramp. Opaque RGBA.
pub fn frost(t: f32, dark: bool) -> Vec<u8> {
    let r = ramp(dark);
    let k = if dark { 0.55 } else { 0.4 };
    let mut out = vec![0u8; (FROST_W * FROST_H * 4) as usize];
    for y in 0..FROST_H {
        for x in 0..FROST_W {
            let (u, v) = ((x as f32 + 0.5) / FROST_W as f32, (y as f32 + 0.5) / FROST_H as f32);
            let i = field(u, v, t, glow(u, v)) * k;
            let p = i.clamp(0.0, 1.0) * (LEVELS - 1) as f32;
            let lo = (p.floor() as usize).min(LEVELS - 2);
            let f = p - lo as f32;
            let o = ((y * FROST_W + x) * 4) as usize;
            for c in 0..3 {
                out[o + c] = lerp(r[lo][c] as f32, r[lo + 1][c] as f32, f).round() as u8;
            }
            out[o + 3] = 255;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_noise_stays_in_range_and_repeats() {
        for k in 0..500 {
            let (x, y, z) = (k as f32 * 0.37 - 40.0, k as f32 * 0.11 + 3.0, k as f32 * 0.05);
            let n = fbm(x, y, z);
            assert!((0.0..=1.0).contains(&n), "{n}");
            assert_eq!(n, fbm(x, y, z));
        }
    }

    /// Every drawn cell is one of the ramp's upper five levels; the top left, far from the glow,
    /// stays clear; the bottom right is mostly drawn.
    #[test]
    fn cells_use_only_the_ramp_and_the_glow_sits_bottom_right() {
        for dark in [true, false] {
            let r = ramp(dark);
            let (gw, gh, px) = cells(1200.0, 720.0, 0.0, 0.0, 1200, 720, 7.0, dark);
            assert_eq!((gw, gh), (400, 240));
            let at = |i: u32, j: u32| &px[((j * gw + i) * 4) as usize..((j * gw + i) * 4 + 4) as usize];
            let mut used = [false; LEVELS];
            for p in px.chunks_exact(4) {
                if p[3] == 0 {
                    continue;
                }
                let l = r.iter().position(|c| c[..] == p[..3]).expect("a ramp colour");
                assert!(l > 0);
                used[l] = true;
            }
            assert!(used[1..].iter().filter(|u| **u).count() >= 4, "{dark}: the dither spreads over the levels: {used:?}");
            let drawn = |i0: u32, j0: u32| (i0..i0 + 40).flat_map(|i| (j0..j0 + 40).map(move |j| (i, j))).filter(|&(i, j)| at(i, j)[3] == 255).count();
            assert_eq!(drawn(0, 0), 0, "{dark}: the top left is clear");
            assert!(drawn(gw - 40, gh - 40) > 800, "{dark}: the bottom right glows");
        }
    }

    /// A rect cut from the window has the window's own cells there, so the live card lines up
    /// with the still frame around it.
    #[test]
    fn a_rect_matches_the_window_under_it() {
        let (gw, _, whole) = cells(1200.0, 720.0, 0.0, 0.0, 1200, 720, 3.0, true);
        let (rw, rh, part) = cells(1200.0, 720.0, 600.0, 360.0, 300, 240, 3.0, true);
        for j in 0..rh {
            for i in 0..rw {
                let (a, b) = (((j * rw + i) * 4) as usize, (((j + 120) * gw + i + 200) * 4) as usize);
                assert_eq!(part[a..a + 4], whole[b..b + 4], "cell {i},{j}");
            }
        }
    }

    #[test]
    fn it_moves_and_the_frost_is_dimmer() {
        let (_, _, a) = cells(1200.0, 720.0, 600.0, 360.0, 600, 360, 1.0, true);
        let (_, _, b) = cells(1200.0, 720.0, 600.0, 360.0, 600, 360, 2.0, true);
        assert_ne!(a, b, "a second later the flow has moved");
        let top = ramp(true)[LEVELS - 1];
        assert!(frost(5.0, true).chunks_exact(4).all(|p| p[..3] != top[..]), "the frost never reaches the brightest level");
    }
}
