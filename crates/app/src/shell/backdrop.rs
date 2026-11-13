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

/// The backdrops, as saved in prefs: (id, name). "flow" is the moving Flow (S1), "flow-still" the
/// same held still; the paintings are the Tauri app's; "plain" is none.
pub(crate) const BACKDROPS: [(&str, &str); 8] = [
    ("flow", "Flow"),
    ("flow-still", "Flow"),
    ("harbour", "Harbour"),
    ("dawn", "Lighthouse at dawn"),
    ("night", "Moorings at night"),
    ("open-sea", "Open sea"),
    ("fog", "Fog"),
    ("plain", "None"),
];

/// The picture for a still background, Flow first.
pub(crate) const STILLS: [&str; 6] = ["flow-still", "harbour", "dawn", "night", "open-sea", "fog"];

/// The Flow's "picture" name, for `Backdrop::window`.
pub(crate) const FLOW: &str = "flow";

/// The painting for a backdrop in a theme: the harbour is the night in dark and the dawn in light.
pub(crate) fn painting(id: &str, dark: bool) -> Option<&'static str> {
    match id {
        "flow" | "flow-still" => Some(FLOW),
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    /// Each surface's tint alpha over its rect, for this painting and size.
    alphas: HashMap<(Surface, i32, i32, u32, u32), f32>,
    /// How many times the painting was made; tests read it to see the cache work.
    pub(crate) made: usize,
    /// Flow: the time shown, in seconds of the flow, and the time its frost was made at.
    pub(crate) flow_t: f32,
    frost_t: f32,
    /// Flow: the card's frame (its rect, the time, the picture), live or still.
    flow_frame: Option<((i32, i32, u32, u32), f32, Arc<RenderImage>)>,
    /// Pictures no longer shown, for the window to free from the GPU a frame later.
    retired: Vec<Arc<RenderImage>>,
}

/// Flow at the window's start: a calm frame, the glow already up.
pub(crate) const FLOW_START: f32 = 8.0;

/// The frost refresh while Flow moves: twice a second.
const FROST_EVERY: f32 = 0.5;

/// Flow's cells under a rect, scaled up with hard pixels to one texel per window point: the card's
/// picture, straight alpha, already in GPUI's BGRA. Pure, so it runs off the main thread. Each row
/// of cells is scaled once and copied down its cell's rows.
pub(crate) fn flow_image(win: (f32, f32), r: (i32, i32, u32, u32), t: f32, dark: bool) -> RgbaImage {
    use cauldron_chrome::flow::{self, CELL};
    let (x, y, w, h) = r;
    let (gw, gh, cells) = flow::cells(win.0, win.1, x as f32, y as f32, w + CELL, h + CELL, t, dark);
    // The rect's offset into its first cell, so the cells sit where the window's do.
    let (ox, oy) = (x.rem_euclid(CELL as i32) as u32, y.rem_euclid(CELL as i32) as u32);
    let stride = (w * 4) as usize;
    let mut buf = vec![0u8; stride * h as usize];
    let mut line = vec![0u8; stride];
    for j in 0..gh {
        let (top, bottom) = ((j * CELL).saturating_sub(oy), ((j + 1) * CELL).saturating_sub(oy).min(h));
        if top >= bottom {
            continue;
        }
        for px in 0..w {
            let s = ((j * gw + (px + ox) / CELL) * 4) as usize;
            let d = (px * 4) as usize;
            line[d..d + 4].copy_from_slice(&[cells[s + 2], cells[s + 1], cells[s], cells[s + 3]]);
        }
        for py in top..bottom {
            buf[py as usize * stride..(py as usize + 1) * stride].copy_from_slice(&line);
        }
    }
    RgbaImage::from_raw(w, h, buf).expect("flow frame size")
}

/// A frame already in BGRA, as GPUI takes it.
pub(crate) fn bgra_render(img: RgbaImage) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(img)]))
}

/// Flow's frosted copy at a window size: the 100x60 field stretched smooth, opaque.
fn flow_frost(t: f32, dark: bool, w: u32, h: u32) -> RgbaImage {
    use cauldron_chrome::flow::{frost, FROST_H, FROST_W};
    let small = RgbaImage::from_raw(FROST_W, FROST_H, frost(t, dark)).expect("frost size");
    imageops::resize(&small, w, h, imageops::FilterType::Triangle)
}

impl Backdrop {
    /// The painting at the window's `w` x `h`, made only when the painting, theme or size changed;
    /// `bg` is the colour it is muted toward and lies over.
    pub(crate) fn window(&mut self, name: &'static str, dark: bool, w: u32, h: u32, bg: [u8; 3]) {
        let key = Key { name, dark, w, h };
        if self.key.as_ref() != Some(&key) && name == FLOW {
            if self.key.is_none() {
                self.flow_t = FLOW_START;
            }
            self.blurred = Some(flow_frost(self.flow_t, dark, w, h));
            self.frost_t = self.flow_t;
            self.art = None;
            self.retire_all();
            self.alphas.clear();
            self.key = Some(key);
            self.made += 1;
            return;
        }
        if self.key.as_ref() != Some(&key) {
            let band = Band::board(dark);
            let art = render(&decoded(name), w, h, &band, bg);
            // The glass sees the painting as the card shows it: over the card colour, then blurred.
            let mut flat = RgbaImage::from_pixel(w, h, image::Rgba([bg[0], bg[1], bg[2], 255]));
            imageops::overlay(&mut flat, &art, 0, 0);
            self.blurred = Some(imageops::fast_blur(&flat, BLUR_SIGMA));
            self.art = Some(art);
            self.retire_all();
            self.alphas.clear();
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
            self.retired.extend(self.crops.drain().map(|(_, v)| v));
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

    /// The tint for surface `s` over the rect `r` (window coordinates): the board's alpha, raised
    /// only as far as this painting needs for fg2 and fg3 to keep 4.6:1 over every part of the
    /// rect (a bright painting under a light theme needs more frost than the harbour). Cached per
    /// painting, size and rect.
    pub(crate) fn tint_for(&mut self, s: Surface, dark: bool, r: (i32, i32, u32, u32), pal: &cauldron_chrome::tokens::Palette) -> [f32; 4] {
        let base = tint_of(s, dark);
        let Some(b) = self.blurred.as_ref() else { return base };
        let key = (s, r.0, r.1, r.2, r.3);
        if let Some(a) = self.alphas.get(&key) {
            return [base[0], base[1], base[2], *a];
        }
        let rr = (r.0.max(0) as u32, r.1.max(0) as u32, r.2, r.3);
        let mut a = base[3];
        while a < 0.99 {
            let t = [base[0], base[1], base[2], a];
            if worst_over(b, rr, t, pal.fg2, 2).min(worst_over(b, rr, t, pal.fg3, 2)) >= 4.6 {
                break;
            }
            a = (a + 0.02).min(0.99);
        }
        if self.alphas.len() > 64 {
            self.alphas.clear();
        }
        self.alphas.insert(key, a);
        [base[0], base[1], base[2], a]
    }

    /// The same as a GPUI colour.
    pub(crate) fn hsla_for(&mut self, s: Surface, dark: bool, r: (i32, i32, u32, u32), pal: &cauldron_chrome::tokens::Palette) -> gpui_kit::Hsla {
        let t = self.tint_for(s, dark, r, pal);
        let mut h: gpui_kit::Hsla = gpui_kit::rgb(((t[0] as u32) << 16) | ((t[1] as u32) << 8) | t[2] as u32).into();
        h.a = t[3];
        h
    }

    /// Forgets the painting, its blurred copy and every cut: the painting is None now.
    pub(crate) fn drop_all(&mut self) {
        if self.key.is_some() {
            self.retire_all();
            let retired = std::mem::take(&mut self.retired);
            *self = Backdrop { made: self.made, retired, ..Default::default() };
        }
    }

    fn retire_all(&mut self) {
        self.retired.extend(self.crops.drain().map(|(_, v)| v));
        self.retired.extend(self.sharp_crops.drain().map(|(_, v)| v));
        self.retired.extend(self.flow_frame.take().map(|f| f.2));
    }

    /// The pictures no longer shown, for the window to free (`Window::drop_image`).
    pub(crate) fn take_retired(&mut self) -> Vec<Arc<RenderImage>> {
        std::mem::take(&mut self.retired)
    }

    /// Whether the backdrop is Flow.
    pub(crate) fn is_flow(&self) -> bool {
        self.key.as_ref().is_some_and(|k| k.name == FLOW)
    }

    /// Flow moved on to `t`: the frost follows twice a second (not every frame; the frosted
    /// surfaces drift a little behind the card). The tints are kept: the frost is dimmed so text
    /// never meets the glow's brightest part.
    pub(crate) fn flow_to(&mut self, t: f32) {
        self.flow_t = t;
        let Some(k) = self.key.clone().filter(|k| k.name == FLOW) else { return };
        if (t - self.frost_t).abs() >= FROST_EVERY {
            self.blurred = Some(flow_frost(t, k.dark, k.w, k.h));
            self.frost_t = t;
            self.retired.extend(self.crops.drain().map(|(_, v)| v));
        }
    }

    /// A frame made off the main thread for the card's rect at time `t`.
    pub(crate) fn set_flow_frame(&mut self, r: (i32, i32, u32, u32), t: f32, img: Arc<RenderImage>) {
        if let Some(old) = self.flow_frame.replace((r, t, img)) {
            self.retired.push(old.2);
        }
    }

    /// Whether anything is held, for the tests.
    #[cfg(test)]
    pub(crate) fn holds(&self) -> bool {
        self.key.is_some() || self.art.is_some() || self.blurred.is_some() || !self.crops.is_empty() || !self.sharp_crops.is_empty() || !self.alphas.is_empty()
    }

    /// The painting itself cut at `r`, for the new thread's card: a clear window onto it.
    pub(crate) fn sharp(&mut self, x: i32, y: i32, w: u32, h: u32) -> Option<Arc<RenderImage>> {
        if let Some(k) = self.key.clone().filter(|k| k.name == FLOW) {
            // Flow: the latest frame if it is this rect's, else the still frame at the time shown.
            if w == 0 || h == 0 {
                return None;
            }
            let r = (x, y, w, h);
            match &self.flow_frame {
                Some((fr, _, img)) if *fr == r => return Some(img.clone()),
                _ => {
                    let img = bgra_render(flow_image((k.w as f32, k.h as f32), r, self.flow_t, k.dark));
                    self.set_flow_frame(r, self.flow_t, img.clone());
                    return Some(img);
                }
            }
        }
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
    worst_over(blurred, r, tint, fg, 1)
}

/// The same, looking at every `step`th pixel each way (the blur makes neighbours alike).
fn worst_over(blurred: &RgbaImage, r: (u32, u32, u32, u32), tint: [f32; 4], fg: cauldron_chrome::tokens::Rgb, step: usize) -> f32 {
    let mut worst = f32::MAX;
    for y in (r.1..(r.1 + r.3).min(blurred.height())).step_by(step) {
        for x in (r.0..(r.0 + r.2).min(blurred.width())).step_by(step) {
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

    /// No stale state: resizing away and back gives the same painting byte for byte; a theme or
    /// painting change drops every old cut; None drops everything held.
    #[test]
    fn resizing_back_gives_the_same_painting_and_nothing_stale_is_kept() {
        let mut b = Backdrop::default();
        b.window("fog", true, 300, 200, rgb(DARK.panel));
        let first = (b.art.clone().unwrap(), b.blurred.clone().unwrap());
        let cut = b.glass(0, 0, 50, 50).unwrap();
        b.window("fog", true, 340, 220, rgb(DARK.panel));
        assert!(b.crops.is_empty() && b.sharp_crops.is_empty(), "a resize drops the old cuts");
        b.window("fog", true, 300, 200, rgb(DARK.panel));
        assert_eq!((b.art.clone().unwrap(), b.blurred.clone().unwrap()), first, "identical after the round trip");
        assert!(!Arc::ptr_eq(&cut, &b.glass(0, 0, 50, 50).unwrap()), "cut afresh, not kept from before");
        b.window("fog", false, 300, 200, rgb(LIGHT.panel));
        assert_ne!(b.art.clone().unwrap(), first.0, "the other theme is another painting");
        b.drop_all();
        assert!(!b.holds(), "None keeps nothing");
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
        // Every painting Settings offers, in both themes.
        let mut cases = Vec::new();
        for (id, _) in BACKDROPS {
            for dark in [true, false] {
                if let Some(name) = painting(id, dark) {
                    cases.push((dark, if dark { &DARK } else { &LIGHT }, name));
                }
            }
        }
        assert_eq!(cases.len(), 14, "Flow moving and still, five paintings, two themes");
        for (dark, pal, name) in cases {
            let mut b = Backdrop::default();
            b.window(name, dark, 1200, 720, rgb(pal.panel));
            for (what, surf, r) in rects {
                let t = b.tint_for(surf, dark, (r.0 as i32, r.1 as i32, r.2, r.3), pal);
                assert!(t[3] >= tint_of(surf, dark)[3], "never below the board's tint");
                for (fgn, fg) in [("fg2", pal.fg2), ("fg3", pal.fg3)] {
                    let c = worst_contrast(b.blurred().unwrap(), r, t, fg);
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
            assert!(window.try_find("window-art").is_some() && window.try_find("shell-art").is_none(), "the window frosted, one frame colour: no second slab");
            assert!(window.try_find("card-art").is_some(), "the new thread is a clear window");
            window.click(("thread", 0usize), cx);
            window.render_frame(cx);
            assert!(window.try_find("card-art").is_none() && window.try_find("card-frost").is_some(), "a thread's card is frosted");
            assert!(window.try_find("window-art").is_some(), "and the window still shows the painting");
        })
        .unwrap();
    }

    /// Flow moves only behind the new stream: a stream, Settings or reduced motion hold it still,
    /// and Still never moves. The card shows Flow either way.
    #[gpui_kit::test]
    fn flow_moves_only_behind_the_new_stream(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        use gpui_kit::AppContext;
        let (handle, _chats) = super::super::thread::tests::open(cx);
        let path = std::env::temp_dir().join(format!("cauldron-flow-{}.json", std::process::id()));
        std::fs::write(&path, r#"{"theme":"dark","backdrop":"flow"}"#).unwrap();
        handle.update(cx, |c, _, _| c.prefs_file = Some(path.clone())).unwrap();
        cx.update(|cx| cx.set_reduce_motion(false));
        let moving = |cx: &mut gpui_kit::TestAppContext| handle.update(cx, |c, _, _| c.flow_at.is_some()).unwrap();
        cx.update_window(handle.into(), |_, window, _| window.activate_window()).unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.is_window_active(), "the window is in front");
            assert!(window.try_find("card-art").is_some(), "the new stream shows Flow");
        })
        .unwrap();
        assert!(moving(cx), "and it moves there");
        cx.update_window(handle.into(), |_, window, cx| {
            window.click(("thread", 0usize), cx);
            window.render_frame(cx);
        })
        .unwrap();
        assert!(!moving(cx), "a stream holds it still");
        handle.update(cx, |c, _, _| c.settings = Some(2)).unwrap();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx)).unwrap();
        assert!(!moving(cx), "so does Settings");
        handle.update(cx, |c, _, _| {
            c.settings = None;
            c.open = None;
            c.thread = None;
        })
        .unwrap();
        cx.update(|cx| cx.set_reduce_motion(true));
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx)).unwrap();
        assert!(!moving(cx), "reduced motion holds it still");
        cx.update(|cx| cx.set_reduce_motion(false));
        std::fs::write(&path, r#"{"theme":"dark","backdrop":"flow-still"}"#).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("card-art").is_some(), "Still shows Flow too");
        })
        .unwrap();
        assert!(!moving(cx), "and never moves");
        let _ = std::fs::remove_file(path);
    }

    /// Every theme x every painting (and None) x every view renders, and None falls back to the
    /// opaque tokens: no painting, no frost, nowhere.
    #[gpui_kit::test]
    fn every_theme_painting_and_view_renders(cx: &mut gpui_kit::TestAppContext) {
        use cauldron_chrome::panes::Shape;
        use gpui_kit::test::TestWindowExt;
        use gpui_kit::AppContext;
        let (handle, _chats) = super::super::thread::tests::open(cx);
        let path = std::env::temp_dir().join(format!("cauldron-look-{}.json", std::process::id()));
        handle.update(cx, |c, _, _| c.prefs_file = Some(path.clone())).unwrap();
        for theme in ["light", "dark", "system"] {
            for (id, _) in BACKDROPS {
                std::fs::write(&path, format!(r#"{{"theme":"{theme}","backdrop":"{id}"}}"#)).unwrap();
                for view in ["new", "thread", "two panes", "drawer folded", "settings"] {
                    handle
                        .update(cx, |c, _, _| {
                            c.settings = (view == "settings").then_some(2);
                            c.panes.shape = if view == "two panes" { Shape::Cols } else { Shape::One };
                            c.drawer = if view == "drawer folded" { None } else { Some(super::super::Panel::Highlights) };
                            c.motion = Default::default();
                        })
                        .unwrap();
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        if view != "new" && view != "settings" && window.try_find(("ptab", 0usize)).is_none() {
                            window.click(("thread", 0usize), cx);
                            window.render_frame(cx);
                        }
                        let at = format!("{theme} {id} {view}");
                        let painted = id != "plain";
                        assert_eq!(window.try_find("window-art").is_some(), painted, "{at}: the painting behind the window");
                        if !painted {
                            for none in ["shell-art", "card-art", "card-frost"] {
                                assert!(window.try_find(none).is_none(), "{at}: no {none} with None");
                            }
                        }
                        if view == "settings" {
                            assert!(window.try_find("drawer").is_none() && window.try_find("rrail").is_none(), "{at}: Settings has no drawer or rail");
                        }
                    })
                    .unwrap();
                }
            }
        }
        let _ = std::fs::remove_file(path);
    }
}
