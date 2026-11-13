//! The painting behind the whole window (the coordinator's board e0fd268, painting X; Charles,
//! 13 Nov 07:03Z: it must not disappear on the sidebars and panes). It is dithered once at the
//! window's size and shown sharp only through the new thread's clear card; everywhere else one
//! blurred copy of it, cut at the surface's rect, lies under that surface's tint (the window and the
//! sidebar, the grey frame, each card, the composer and its chips). GPUI cannot blur what is behind an
//! element (docs/cauldron/GLASS-FEASIBILITY.md), so this is how glass is made. Everything is made on
//! the CPU once per window size and cached; a cut is cached by its rect. Nothing runs per frame.

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

/// What lies over the blurred painting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Surface {
    /// The window behind everything, and so the sidebar on it.
    Sidebar,
    /// The grey frame: gutters, drawer, rail, stage bar.
    Shell,
    /// A thread's card, a pane, the terminal, Settings' page.
    Card,
    /// The composer, its chips and the header's pills.
    Glass,
}

/// Each surface's tint over the blurred painting, dark and light (the board e0fd268, measured
/// again here: every one keeps fg2 and fg3 at 4.5:1 or better, see the tests).
pub(crate) fn tint_of(s: Surface, dark: bool) -> [f32; 4] {
    match (s, dark) {
        (Surface::Sidebar, true) => [17.0, 17.0, 17.0, 0.64],
        (Surface::Sidebar, false) => [247.0, 247.0, 248.0, 0.78],
        (Surface::Shell, true) => [29.0, 29.0, 29.0, 0.68],
        (Surface::Shell, false) => [238.0, 238.0, 240.0, 0.72],
        (Surface::Card, true) => [21.0, 21.0, 21.0, 0.78],
        (Surface::Card, false) => [255.0, 255.0, 255.0, 0.80],
        (Surface::Glass, true) => [21.0, 21.0, 21.0, 0.75],
        (Surface::Glass, false) => [255.0, 255.0, 255.0, 0.75],
    }
}

/// The glass tint (composer, chips, pills).
pub(crate) fn tint(dark: bool) -> [f32; 4] {
    tint_of(Surface::Glass, dark)
}

/// A surface's tint as a GPUI colour.
pub(crate) fn hsla(s: Surface, dark: bool) -> gpui_kit::Hsla {
    let t = tint_of(s, dark);
    let mut h: gpui_kit::Hsla = gpui_kit::rgb(((t[0] as u32) << 16) | ((t[1] as u32) << 8) | t[2] as u32).into();
    h.a = t[3];
    h
}

/// How soft the glass is: about 27px across (the board's).
const BLUR_SIGMA: f32 = 13.5;

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
    art: Option<RgbaImage>,
    blurred: Option<RgbaImage>,
    crops: HashMap<(i32, i32, u32, u32), Arc<RenderImage>>,
    sharp_crops: HashMap<(i32, i32, u32, u32), Arc<RenderImage>>,
    /// How many times the painting was made; tests read it to see the cache work.
    pub(crate) made: usize,
}

impl Backdrop {
    /// The painting at the window's `w` x `h`, made only when the painting, theme or size changed;
    /// `bg` is the colour it is muted toward and lies over.
    pub(crate) fn window(&mut self, name: &'static str, dark: bool, w: u32, h: u32, bg: [u8; 3]) {
        let key = Key { name, dark, w, h };
        if self.key.as_ref() != Some(&key) {
            let band = Band::board(dark);
            let art = render(&decoded(name), w, h, &band, bg);
            // The glass sees the painting as the card shows it: over the card colour, then blurred.
            let mut flat = RgbaImage::from_pixel(w, h, image::Rgba([bg[0], bg[1], bg[2], 255]));
            imageops::overlay(&mut flat, &art, 0, 0);
            self.blurred = Some(imageops::fast_blur(&flat, BLUR_SIGMA));
            self.art = Some(art);
            self.crops.clear();
            self.sharp_crops.clear();
            self.key = Some(key);
            self.made += 1;
        }
    }

    /// The blurred painting cut at a rect (window coordinates), cached by rect; None before
    /// `window`. A drag can cut many rects, so the cache starts over past 64 of them.
    pub(crate) fn glass(&mut self, x: i32, y: i32, w: u32, h: u32) -> Option<Arc<RenderImage>> {
        let b = self.blurred.as_ref()?;
        if w == 0 || h == 0 {
            return None;
        }
        if self.crops.len() > 64 {
            self.crops.clear();
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

    /// The painting itself cut at `r`, for the new thread's card: a clear window onto it.
    pub(crate) fn sharp(&mut self, x: i32, y: i32, w: u32, h: u32) -> Option<Arc<RenderImage>> {
        let a = self.art.as_ref()?;
        if w == 0 || h == 0 {
            return None;
        }
        Some(
            self.sharp_crops
                .entry((x, y, w, h))
                .or_insert_with(|| {
                    let mut out = RgbaImage::new(w, h);
                    imageops::overlay(&mut out, a, -(x as i64), -(y as i64));
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
    /// theme change. A cut is made once per rect.
    #[test]
    fn the_painting_is_made_once_per_size() {
        let mut b = Backdrop::default();
        b.window("harbour-night", true, 300, 200, rgb(DARK.panel));
        b.window("harbour-night", true, 300, 200, rgb(DARK.panel));
        assert_eq!(b.made, 1, "same size: no work");
        b.window("harbour-night", true, 320, 200, rgb(DARK.panel));
        assert_eq!(b.made, 2, "a resize makes it again");
        b.window("harbour-dawn", false, 320, 200, rgb(LIGHT.panel));
        assert_eq!(b.made, 3, "so does the other theme");
        let g1 = b.glass(10, 20, 100, 40).unwrap();
        let g2 = b.glass(10, 20, 100, 40).unwrap();
        assert!(Arc::ptr_eq(&g1, &g2), "a cut is made once per rect");
        assert_eq!(g1.size(0).width.0, 100);
    }

    /// The painting dissolves toward the window's foot: the bottom row is mostly transparent.
    #[test]
    fn the_painting_dissolves_toward_the_foot() {
        let art = render(&decoded("harbour-night"), 400, 300, &Band::board(true), rgb(DARK.panel));
        assert_eq!((art.width(), art.height()), (400, 300));
        let clear = |y: u32| (0..400).filter(|&x| art.get_pixel(x, y).0[3] == 0).count();
        assert_eq!(clear(10), 0, "the top is the painting");
        assert!(clear(299) > 300, "the foot is the card: {}", clear(299));
    }

    /// Every surface's text reads on its glass: fg2 and fg3 at 4.5:1 or better over every pixel of
    /// the surface, in both themes, at the 1200x720 Mac window with the drawer open.
    #[test]
    fn text_on_every_surface_holds_4_5_to_1() {
        // (surface, x, y, w, h) in the window: the sidebar, the drawer and the stage bar on the
        // frame, the thread's card, and the composer box on the new thread.
        let rects = [
            ("sidebar", Surface::Sidebar, (13, 45, 236, 668)),
            ("drawer", Surface::Shell, (945, 13, 236, 694)),
            ("stage bar", Surface::Shell, (263, 675, 676, 32)),
            ("thread card", Surface::Card, (263, 13, 676, 694)),
            ("composer", Surface::Glass, (287, 320, 628, 84)),
            ("chip", Surface::Glass, (287, 286, 140, 26)),
        ];
        for (dark, pal, name) in [(true, &DARK, "harbour-night"), (false, &LIGHT, "harbour-dawn")] {
            let mut b = Backdrop::default();
            b.window(name, dark, 1200, 720, rgb(pal.panel));
            for (what, surf, r) in rects {
                for (fgn, fg) in [("fg2", pal.fg2), ("fg3", pal.fg3)] {
                    let c = worst_contrast(b.blurred().unwrap(), r, tint_of(surf, dark), fg);
                    assert!(c >= 4.5, "{name} {what} {fgn}: {c:.2}:1");
                }
            }
        }
    }

    /// The painting is behind every screen: the window carries its frosted copy; the new thread's
    /// card is a clear window onto it; a thread's card is frosted, not solid.
    #[gpui_kit::test]
    fn the_painting_is_behind_every_screen(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        use gpui_kit::AppContext;
        let (handle, _chats) = super::super::thread::tests::open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("window-art").is_some() && window.try_find("shell-art").is_some(), "window and frame frosted");
            assert!(window.try_find("card-art").is_some(), "the new thread is a clear window");
            window.click(("thread", 0usize), cx);
            window.render_frame(cx);
            assert!(window.try_find("card-art").is_none() && window.try_find("card-frost").is_some(), "a thread's card is frosted");
            assert!(window.try_find("window-art").is_some(), "and the window still shows the painting");
        })
        .unwrap();
    }
}
