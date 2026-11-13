//! The painting's grain (background A, the coordinator's board 88454ee): Berth's DitherBand as the
//! Tauri app had it (legacy/tauri-app/ui/art.js), ported line for line.

use cauldron_chrome::dither::{self, Band, BAYER8};

fn frame(w: usize, h: usize) -> Vec<u8> {
    // A gradient with an edge down the middle, so both flat and edged areas are in it.
    let mut v = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let base = if x < w / 2 { 60 } else { 170 };
            let g = (base + (y * 40 / h.max(1)) as i32).min(255) as u8;
            v[i..i + 4].copy_from_slice(&[g, g.saturating_sub(10), g.saturating_add(5), 255]);
        }
    }
    v
}

#[test]
fn the_bayer_matrix_is_the_8x8_one_centred_in_its_cells() {
    assert_eq!(BAYER8.len(), 64);
    assert_eq!(BAYER8[0], 0.5 / 64.0);
    assert_eq!(BAYER8[1], 32.5 / 64.0);
    let mut sorted = BAYER8.to_vec();
    sorted.sort_by(f32::total_cmp);
    for (i, v) in sorted.iter().enumerate() {
        assert_eq!(*v, (i as f32 + 0.5) / 64.0, "every level once");
    }
}

#[test]
fn the_same_size_gives_the_same_grain() {
    let band = Band::board(true);
    let (mut a, mut b) = (frame(64, 48), frame(64, 48));
    dither::dither(&mut a, 64, 48, &band, [21, 21, 21]);
    dither::dither(&mut b, 64, 48, &band, [21, 21, 21]);
    assert_eq!(a, b, "deterministic");
    assert_ne!(a, frame(64, 48), "and it changes the picture");
}

#[test]
fn it_dissolves_toward_the_foot_and_not_above_the_fade() {
    let (w, h) = (64, 80);
    let band = Band::board(false);
    let mut f = frame(w, h);
    dither::dither(&mut f, w, h, &band, [255, 255, 255]);
    let gone = |y: usize| (0..w).filter(|&x| f[(y * w + x) * 4 + 3] == 0).count();
    let from = (h as f32 * (1.0 - band.fade)) as usize;
    for y in 0..from {
        assert_eq!(gone(y), 0, "row {y} is above the fade");
    }
    // Monotone in blocks of the matrix's 8 rows: each block loses at least as much as the one above.
    let block = |b: usize| (b * 8..(b * 8 + 8).min(h)).map(gone).sum::<usize>();
    for b in (from / 8)..(h / 8 - 1) {
        assert!(block(b + 1) >= block(b), "block {} {} then {} {}", b, block(b), b + 1, block(b + 1));
    }
    assert!(gone(h - 1) > w * 3 / 4, "the last row is almost all gone: {}", gone(h - 1));
}

#[test]
fn the_grain_is_lightness_only_and_muted_toward_the_card() {
    let (w, h) = (16, 16);
    let band = Band { fade: 0.0, ..Band::board(true) };
    let mut f = frame(w, h);
    let before = f.clone();
    dither::dither(&mut f, w, h, &band, [21, 21, 21]);
    for p in 0..w * h {
        let i = p * 4;
        // Mute moves every channel toward the card colour by the same share; the dither then adds the
        // same amount to each channel, so the channel differences of the muted pixel survive (± clamp).
        let mute = |c: usize| before[i + c] as f32 + (21.0 - before[i + c] as f32) * band.mute;
        let d0 = f[i] as f32 - mute(0);
        let d1 = f[i + 1] as f32 - mute(1);
        assert!((d0 - d1).abs() <= 1.5, "pixel {p}: the same shift on every channel ({d0} vs {d1})");
        assert_eq!(f[i + 3], 255);
    }
}

#[test]
fn the_board_settings() {
    let d = Band::board(true);
    assert_eq!((d.levels, d.fade, d.cell, d.soft), (6, 0.42, 2, 0.6));
    assert_eq!(d.mute, 0.3);
    assert_eq!(Band::board(false).mute, 0.07);
}

#[test]
fn the_painting_covers_the_card() {
    // A 1600x900 picture on a 300x300 small frame: scaled to cover, centred across, at `position` down.
    let (x, y, w, h) = dither::cover(1600, 900, 300, 300, 0.5);
    assert_eq!(h, 300.0);
    assert!((w - 1600.0 * 300.0 / 900.0).abs() < 1e-3);
    assert!((x - (300.0 - w) / 2.0).abs() < 1e-3 && y == 0.0);
}
