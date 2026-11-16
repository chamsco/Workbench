//! The Harbour painting behind the whole window (Tight tiles rule 7): dusk in dark, day in light,
//! under one frosted frame layer. GPUI has no backdrop blur (rule 8), so the frost is baked: the
//! painting is shrunk, blurred and saturated once per theme, cached on disk and in memory, and the
//! window draws that image under a tint. Cards are glass: their colour at `GLASS_CARD` over it.

use backspace_core::prefs::Prefs;
use cauldron_chrome::tokens::{Palette, Rgb};
use gpui_kit::RenderImage;
use image::{imageops, ImageFormat, RgbaImage};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

const DUSK: &[u8] = include_bytes!("../../assets/art/harbour-dusk.webp");
const DAY: &[u8] = include_bytes!("../../assets/art/harbour-day.webp");

/// The board's frame layer: `blur(28px) saturate(1.15)`, at the board's 1200px window.
const BLUR: f32 = 28.0;
const SATURATE: f32 = 1.15;
const REF_W: f32 = 1200.0;
/// The width the painting is blurred at. GPUI scales it up to the window; the blur hides that.
const WORK_W: u32 = 420;
/// Bump when the recipe changes, so an old cached file is not reused.
const VERSION: u32 = 1;

/// The shared `backdrop` pref's values Cauldron writes from Settings > Appearance.
pub(crate) const PAINTING: &str = "painting";
pub(crate) const PLAIN: &str = "plain";

/// Whether the prefs ask for the painting. Plain is the default (Charles, 16 Nov): only
/// Cauldron's own "painting" draws it; the old app's values ("harbour", ...) stay plain.
pub(crate) fn wants_painting(p: &Prefs) -> bool {
    p.backdrop == PAINTING
}

/// The frame tint over the painting: the frame colour at 70% in dark, 52% in light.
pub(crate) fn frame_tint(dark: bool) -> f32 {
    if dark { 0.70 } else { 0.52 }
}
/// Cards over the frame layer.
pub(crate) const GLASS_CARD: f32 = 0.58;
/// Reading surfaces stay more opaque: the composer, the terminal and diffs.
pub(crate) const GLASS_COMPOSER: f32 = 0.82;
pub(crate) const GLASS_TERM: f32 = 0.70;

/// A surface colour: glass over the painting, solid on the plain frame.
pub(crate) fn surface(painting: bool, v: Rgb, alpha: f32) -> gpui_kit::Hsla {
    if painting { super::ca(v, alpha) } else { super::c(v) }
}

/// The frame layer for a theme, built on first use and kept for the process.
pub(crate) fn frame_layer(dark: bool) -> Arc<RenderImage> {
    static DARK: OnceLock<Arc<RenderImage>> = OnceLock::new();
    static LIGHT: OnceLock<Arc<RenderImage>> = OnceLock::new();
    let cell = if dark { &DARK } else { &LIGHT };
    cell.get_or_init(|| Arc::new(to_render(blurred(dark)))).clone()
}

/// Builds both layers off the main thread so the first frame does not wait.
pub(crate) fn warm() {
    std::thread::spawn(|| {
        frame_layer(true);
        frame_layer(false);
    });
}

fn cache_path(dark: bool) -> PathBuf {
    Prefs::data_dir().join("cache").join(format!("backdrop-{}-v{VERSION}.png", if dark { "dusk" } else { "day" }))
}

/// The blurred, saturated painting, from the cache when it is there. Tests never touch the
/// user's data folder.
pub(crate) fn blurred(dark: bool) -> RgbaImage {
    let path = cache_path(dark);
    if !cfg!(test) {
        if let Ok(img) = image::open(&path) {
            return img.to_rgba8();
        }
    }
    let img = bake(if dark { DUSK } else { DAY });
    if !cfg!(test) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = img.save_with_format(&path, ImageFormat::Png);
    }
    img
}

/// Shrinks the painting to `WORK_W`, blurs it by the board's 28px (scaled to that width) and
/// saturates it by 1.15, as CSS's `saturate()` does.
pub(crate) fn bake(webp: &[u8]) -> RgbaImage {
    let img = image::load_from_memory_with_format(webp, ImageFormat::WebP).expect("bundled painting decodes").to_rgba8();
    let h = ((WORK_W as f32) * img.height() as f32 / img.width() as f32).round().max(1.0) as u32;
    let small = imageops::resize(&img, WORK_W, h, imageops::FilterType::Triangle);
    let mut out = imageops::blur(&small, BLUR * WORK_W as f32 / REF_W);
    for p in out.pixels_mut() {
        let [r, g, b, _] = p.0;
        let [r, g, b] = saturate([r, g, b], SATURATE);
        p.0[0] = r;
        p.0[1] = g;
        p.0[2] = b;
    }
    out
}

/// CSS `saturate(s)`: the Filter Effects matrix on 0..255 channels.
pub(crate) fn saturate([r, g, b]: [u8; 3], s: f32) -> [u8; 3] {
    let (r, g, b) = (r as f32, g as f32, b as f32);
    let ch = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    [
        ch((0.213 + 0.787 * s) * r + (0.715 - 0.715 * s) * g + (0.072 - 0.072 * s) * b),
        ch((0.213 - 0.213 * s) * r + (0.715 + 0.285 * s) * g + (0.072 - 0.072 * s) * b),
        ch((0.213 - 0.213 * s) * r + (0.715 - 0.715 * s) * g + (0.072 + 0.928 * s) * b),
    ]
}

/// GPUI keeps images as BGRA.
fn to_render(mut img: RgbaImage) -> RenderImage {
    for p in img.pixels_mut() {
        p.0.swap(0, 2);
    }
    RenderImage::new(smallvec::smallvec![image::Frame::new(img)])
}

/// The tint drawn over the frame layer.
pub(crate) fn tint(pal: &Palette, dark: bool) -> gpui_kit::Hsla {
    surface(true, pal.frame, frame_tint(dark))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frame layer is the painting shrunk to 420 wide, its shape kept, and blurred: neighbouring
    /// pixels differ far less than in the painting.
    #[test]
    fn the_frame_layer_is_the_painting_shrunk_and_blurred() {
        for webp in [DUSK, DAY] {
            let raw = image::load_from_memory_with_format(webp, ImageFormat::WebP).unwrap().to_rgba8();
            let out = bake(webp);
            assert_eq!(out.width(), WORK_W);
            let ratio = |w: u32, h: u32| w as f32 / h as f32;
            assert!((ratio(out.width(), out.height()) - ratio(raw.width(), raw.height())).abs() < 0.01, "aspect kept");
            let small = imageops::resize(&raw, out.width(), out.height(), imageops::FilterType::Triangle);
            let rough = |im: &RgbaImage| {
                let mut sum = 0u64;
                for y in 0..im.height() {
                    for x in 1..im.width() {
                        sum += (im.get_pixel(x, y).0[1] as i32 - im.get_pixel(x - 1, y).0[1] as i32).unsigned_abs() as u64;
                    }
                }
                sum
            };
            assert!(rough(&out) * 4 < rough(&small), "blurred: {} vs {}", rough(&out), rough(&small));
        }
    }

    /// `saturate()` keeps greys and pushes colours away from grey, as CSS does.
    #[test]
    fn saturate_matches_css() {
        assert_eq!(saturate([128, 128, 128], 1.15), [128, 128, 128], "grey stays grey");
        assert_eq!(saturate([200, 100, 50], 1.0), [200, 100, 50], "1 is identity");
        let [r, g, b] = saturate([200, 100, 50], 1.15);
        assert!(r > 200 && g < 100 && b < 50, "more saturated: {r} {g} {b}");
    }

    /// Plain is the default (Charles, 16 Nov): only Cauldron's own Settings choice, "painting",
    /// draws the painting; the old app's values ("harbour", "dawn", ...) and a fresh prefs file
    /// stay plain.
    #[test]
    fn plain_is_the_default_and_painting_is_a_choice() {
        let mut p = Prefs::default();
        assert!(!wants_painting(&p), "fresh prefs: plain");
        p.backdrop = "harbour".into();
        assert!(!wants_painting(&p), "the old app's backdrop: plain here");
        p.backdrop = PAINTING.into();
        assert!(wants_painting(&p));
        p.backdrop = PLAIN.into();
        assert!(!wants_painting(&p));
    }

    /// Glass only over the painting; on the plain frame every surface is solid.
    #[test]
    fn surfaces_are_glass_only_over_the_painting() {
        let v = Rgb::hex(0x151515);
        assert_eq!(surface(true, v, GLASS_CARD).a, GLASS_CARD);
        assert_eq!(surface(false, v, GLASS_CARD).a, 1.0);
        assert!(GLASS_COMPOSER > GLASS_TERM && GLASS_TERM > GLASS_CARD, "reading surfaces more opaque");
        assert_eq!((frame_tint(true), frame_tint(false)), (0.70, 0.52));
    }
}
