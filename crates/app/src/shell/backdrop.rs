//! Background A (the coordinator's board 88454ee): a painting in the old dither on the new
//! thread's card, never behind a thread, and frosted glass on the composer and its chip. GPUI cannot
//! blur what is behind an element (docs/cauldron/GLASS-FEASIBILITY.md), so each glass element draws a
//! pre-blurred copy of the same painting cut at its own rect, under a tint. Everything is made on
//! the CPU once per card size and cached; a crop is cached by its rect. Nothing runs per frame.

use cauldron_chrome::dither::{self, Band};
use gpui_kit::RenderImage;
use image::{imageops, RgbaImage};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The backdrops Settings offers, as the Tauri app named them: (prefs id, name). "plain" is none.
pub(crate) const BACKDROPS: [(&str, &str); 6] =
    [("harbour", "Harbour"), ("dawn", "Lighthouse at dawn"), ("night", "Moorings at night"), ("open-sea", "Open sea"), ("fog", "Fog"), ("plain", "None")];

/// The painting for a backdrop in a theme: the harbour is the night in dark and the dawn in light.
pub(crate) fn painting(id: &str, dark: bool) -> Option<&'static str> {
    match id {
        "harbour" => Some(if dark { "harbour-night" } else { "harbour-dawn" }),
        "dawn" => Some("dawn"),
        "night" => Some("night"),
        "open-sea" => Some("open-sea"),
        "fog" => Some("fog"),
        _ => None,
    }
}

fn bytes(name: &str) -> &'static [u8] {
    match name {
        "harbour-night" => include_bytes!("../../assets/art/harbour-night.webp"),
        "harbour-dawn" => include_bytes!("../../assets/art/harbour-dawn.webp"),
        "dawn" => include_bytes!("../../assets/art/dawn.webp"),
        "night" => include_bytes!("../../assets/art/night.webp"),
        "open-sea" => include_bytes!("../../assets/art/open-sea.webp"),
        _ => include_bytes!("../../assets/art/fog.webp"),
    }
}

/// A painting, decoded once per run.
fn decoded(name: &'static str) -> Arc<RgbaImage> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, Arc<RgbaImage>>>> = OnceLock::new();
    let mut m = CACHE.get_or_init(Default::default).lock().unwrap();
    m.entry(name).or_insert_with(|| Arc::new(image::load_from_memory_with_format(bytes(name), image::ImageFormat::WebP).expect("bundled painting").into_rgba8())).clone()
}

/// The glass tint over the blurred painting: the panel colour at 74% in dark (the board's) and 75%
/// in light, a point over the board so the composer's fg3 clears the board's 5.2:1 here too.
pub(crate) fn tint(dark: bool) -> [f32; 4] {
    if dark { [21.0, 21.0, 21.0, 0.74] } else { [255.0, 255.0, 255.0, 0.75] }
}
/// How soft the glass is.
const BLUR_SIGMA: f32 = 14.0;

/// The painting drawn into a `w` x `h` card as `Art.band` drew it: covered, drawn small at
/// `band.soft` and scaled up, dithered at `band.cell` px per dot, then scaled up with hard pixels.
/// Straight alpha: dissolved dots are transparent and show the card.
pub(crate) fn render(src: &RgbaImage, w: u32, h: u32, band: &Band, bg: [u8; 3]) -> RgbaImage {
    let cell = band.cell.max(1);
    let (sw, sh) = (w.div_ceil(cell).max(1), h.div_ceil(cell).max(1));
    let (x, y, dw, dh) = dither::cover(src.width(), src.height(), sw, sh, band.position);
    let k = band.soft;
    let (ssw, ssh) = (((sw as f32) * k).round().max(1.0) as u32, ((sh as f32) * k).round().max(1.0) as u32);
    let scaled = imageops::resize(src, (dw * k).round().max(1.0) as u32, (dh * k).round().max(1.0) as u32, imageops::FilterType::CatmullRom);
    let mut soft = RgbaImage::new(ssw, ssh);
    imageops::overlay(&mut soft, &scaled, (x * k).round() as i64, (y * k).round() as i64);
    let mut small = imageops::resize(&soft, sw, sh, imageops::FilterType::Triangle);
    dither::dither(&mut small, sw as usize, sh as usize, band, bg);
    let big = imageops::resize(&small, sw * cell, sh * cell, imageops::FilterType::Nearest);
    imageops::crop_imm(&big, 0, 0, w, h).to_image()
}

/// GPUI's frames are BGRA.
fn to_render(mut img: RgbaImage) -> Arc<RenderImage> {
    for px in img.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![image::Frame::new(img)]))
}

#[derive(Clone, Debug, PartialEq)]
struct Key {
    name: &'static str,
    dark: bool,
    w: u32,
    h: u32,
}

/// The card's painting and its blurred copy for one size, and the glass crops cut from it.
#[derive(Default)]
pub(crate) struct Backdrop {
    key: Option<Key>,
    card: Option<Arc<RenderImage>>,
    blurred: Option<RgbaImage>,
    crops: HashMap<(i32, i32, u32, u32), Arc<RenderImage>>,
    /// How many times the painting was made; tests read it to see the cache work.
    pub(crate) made: usize,
}

impl Backdrop {
    /// The card's painting at `w` x `h`, made only when the painting, theme or size changed.
    pub(crate) fn card(&mut self, name: &'static str, dark: bool, w: u32, h: u32, bg: [u8; 3]) -> Arc<RenderImage> {
        let key = Key { name, dark, w, h };
        if self.key.as_ref() != Some(&key) {
            let band = Band::board(dark);
            let art = render(&decoded(name), w, h, &band, bg);
            // The glass sees the painting as the card shows it: over the card colour, then blurred.
            let mut flat = RgbaImage::from_pixel(w, h, image::Rgba([bg[0], bg[1], bg[2], 255]));
            imageops::overlay(&mut flat, &art, 0, 0);
            self.blurred = Some(imageops::fast_blur(&flat, BLUR_SIGMA));
            self.card = Some(to_render(art));
            self.crops.clear();
            self.key = Some(key);
            self.made += 1;
        }
        self.card.clone().expect("made above")
    }

    /// The blurred painting cut at `r` (card coordinates), cached by rect; None before `card`.
    pub(crate) fn glass(&mut self, x: i32, y: i32, w: u32, h: u32) -> Option<Arc<RenderImage>> {
        let b = self.blurred.as_ref()?;
        if w == 0 || h == 0 {
            return None;
        }
        Some(
            self.crops
                .entry((x, y, w, h))
                .or_insert_with(|| {
                    let mut out = RgbaImage::new(w, h);
                    imageops::overlay(&mut out, b, -(x as i64), -(y as i64));
                    to_render(out)
                })
                .clone(),
        )
    }

    /// The blurred copy, for the contrast checks.
    #[cfg(test)]
    pub(crate) fn blurred(&self) -> Option<&RgbaImage> {
        self.blurred.as_ref()
    }
}

/// The lowest contrast of `fg` over the glass: the tint composited over every pixel of the blurred
/// painting inside `r`.
#[cfg(test)]
pub(crate) fn worst_contrast(blurred: &RgbaImage, r: (u32, u32, u32, u32), tint: [f32; 4], fg: cauldron_chrome::tokens::Rgb) -> f32 {
    let mut worst = f32::MAX;
    for y in r.1..(r.1 + r.3).min(blurred.height()) {
        for x in r.0..(r.0 + r.2).min(blurred.width()) {
            let p = blurred.get_pixel(x, y).0;
            let mix = |c: usize| (tint[c] * tint[3] + p[c] as f32 * (1.0 - tint[3])).round() as u8;
            let under = cauldron_chrome::tokens::Rgb(mix(0), mix(1), mix(2));
            worst = worst.min(fg.contrast(under));
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;
    use cauldron_chrome::tokens::{DARK, LIGHT};

    fn rgb(c: cauldron_chrome::tokens::Rgb) -> [u8; 3] {
        [c.0, c.1, c.2]
    }

    /// Made once per size: the same size is a cache hit, a resize makes it again, and so does a
    /// theme change. A crop is cut once per rect.
    #[test]
    fn the_painting_is_made_once_per_size() {
        let mut b = Backdrop::default();
        let a = b.card("harbour-night", true, 300, 200, rgb(DARK.panel));
        let again = b.card("harbour-night", true, 300, 200, rgb(DARK.panel));
        assert!(Arc::ptr_eq(&a, &again) && b.made == 1, "same size: no work");
        b.card("harbour-night", true, 320, 200, rgb(DARK.panel));
        assert_eq!(b.made, 2, "a resize makes it again");
        b.card("harbour-dawn", false, 320, 200, rgb(LIGHT.panel));
        assert_eq!(b.made, 3, "so does the other theme");
        let g1 = b.glass(10, 20, 100, 40).unwrap();
        let g2 = b.glass(10, 20, 100, 40).unwrap();
        assert!(Arc::ptr_eq(&g1, &g2), "a crop is cut once per rect");
        assert_eq!(g1.size(0).width.0, 100);
    }

    /// The painting dissolves into the card toward the foot: the bottom row is mostly transparent.
    #[test]
    fn the_painting_dissolves_toward_the_foot() {
        let art = render(&decoded("harbour-night"), 400, 300, &Band::board(true), rgb(DARK.panel));
        assert_eq!((art.width(), art.height()), (400, 300));
        let clear = |y: u32| (0..400).filter(|&x| art.get_pixel(x, y).0[3] == 0).count();
        assert_eq!(clear(10), 0, "the top is the painting");
        assert!(clear(299) > 300, "the foot is the card: {}", clear(299));
    }

    /// Text on the glass reads: fg2 and fg3 at 4.5:1 or better over every pixel of the composer's
    /// glass, in both themes, at the 1200x720 window's new-thread card (the board measured the
    /// composer at 5.5:1 dark and 5.2:1 light).
    #[test]
    fn text_on_the_glass_holds_4_5_to_1() {
        // The card at 1200x720 with the drawer open is 676 x 618 (inside its border); the box is
        // 628 x 84, centred, with its chip row above it.
        let (w, h) = (676, 618);
        for (dark, pal, name) in [(true, &DARK, "harbour-night"), (false, &LIGHT, "harbour-dawn")] {
            let mut b = Backdrop::default();
            b.card(name, dark, w, h, rgb(pal.panel));
            let blurred = b.blurred().unwrap();
            let composer = ((w - 628) / 2, h / 2 - 42, 628, 84);
            let chip = ((w - 628) / 2, h / 2 - 42 - 34, 140, 26);
            for (what, r) in [("composer", composer), ("chip", chip)] {
                for (fgn, fg) in [("fg2", pal.fg2), ("fg3", pal.fg3)] {
                    let c = worst_contrast(blurred, r, tint(dark), fg);
                    assert!(c >= 4.5, "{name} {what} {fgn}: {c:.2}:1");
                }
            }
        }
    }

    /// The painting is on the new thread's card only: a thread's card has none, and neither does
    /// the composer there.
    #[gpui_kit::test]
    fn the_painting_shows_on_the_new_thread_only(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        use gpui_kit::AppContext;
        let (handle, _chats) = super::super::thread::tests::open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("card-art").is_some(), "the new thread shows the painting");
            window.click(("thread", 0usize), cx);
            window.render_frame(cx);
            assert!(window.try_find("card-art").is_none(), "a thread's card is solid");
        })
        .unwrap();
    }
}
