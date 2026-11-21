//! The Harbour painting behind the whole window (Tight tiles rule 7): night in dark, day in light.
//! Two depths of glass (19 Nov, after Charles's Ghostex reference): the frame and the sidebar sit
//! on a heavy frost, and the stream and support cards show the same painting under a light frost,
//! lined up with the frame so the window reads as one pane of glass. GPUI has no backdrop blur
//! (rule 8), so both layers are baked once per theme off the UI thread, cached on disk and in
//! memory, and each card draws a crop of the light layer at its own rect.
//!
//! The painting is tone-mapped while it is baked: in dark every pixel is capped in luminance (in
//! light, lifted to a floor) so small grey text keeps its contrast wherever the painting is bright,
//! and the runtime tint over it can stay thin enough for the colours to show.

use backspace_core::prefs::Prefs;
use cauldron_chrome::layout::Rect;
use cauldron_chrome::tokens::{Palette, Rgb};
use gpui_kit::RenderImage;
use image::{imageops, ImageFormat, RgbaImage};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

const NIGHT: &[u8] = include_bytes!("../../assets/art/harbour-night.webp");
const DAY: &[u8] = include_bytes!("../../assets/art/harbour-day.webp");

/// One frost: its blur at the board's 1200px window, the width it is baked at (GPUI scales it up;
/// the blur hides that), its saturation, and the luminance cap (dark) or floor (light).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Frost {
    pub blur: f32,
    pub work_w: u32,
    pub saturate: f32,
    pub cap_dark: f32,
    pub floor_light: f32,
}

/// The frame and sidebar: heavy.
pub(crate) const FRAME: Frost = Frost { blur: 40.0, work_w: 420, saturate: 1.15, cap_dark: 0.045, floor_light: 0.68 };
/// The cards: light, so the painting reads through the content.
pub(crate) const CARD: Frost = Frost { blur: 10.0, work_w: 960, saturate: 1.1, cap_dark: 0.085, floor_light: 0.55 };
const REF_W: f32 = 1200.0;
/// Bump when the recipe changes, so an old cached file is not reused.
const VERSION: u32 = 2;

/// The shared `backdrop` pref's values Cauldron writes from Settings > Appearance.
pub(crate) const PAINTING: &str = "painting";
pub(crate) const PLAIN: &str = "plain";

/// Whether the prefs ask for the painting. Glass is the default again (Charles, 19 Nov, reversing
/// 16 Nov): only "plain" draws the plain frame; the old app's paintings ("harbour", ...) count as
/// the painting.
pub(crate) fn wants_painting(p: &Prefs) -> bool {
    p.backdrop != PLAIN
}

/// The tint over both frost layers. The bake already took the bright spots down, so it is thin.
pub(crate) fn frame_tint(dark: bool) -> f32 {
    if dark { 0.22 } else { 0.30 }
}
/// Cards while their light layer is not baked yet, and the support card's fallback: the old
/// single-depth glass.
pub(crate) const GLASS_CARD: f32 = 0.58;
/// Reading surfaces stay more opaque: the composer, the terminal and diffs.
pub(crate) const GLASS_COMPOSER: f32 = 0.82;
pub(crate) const GLASS_TERM: f32 = 0.70;
/// Over the user's own picture or GIF, which is not tone-mapped: a heavier scrim.
pub(crate) const MEDIA_SCRIM: f32 = 0.66;

/// How dark the scrim over the user's picture is: (pref name, alpha).
pub(crate) const DIMS: [(&str, f32); 3] = [("light", 0.30), ("medium", 0.48), ("dark", MEDIA_SCRIM)];

pub(crate) fn dim_alpha(p: &Prefs) -> f32 {
    DIMS.iter().find(|(n, _)| Some(*n) == p.stream_dim.as_deref()).map_or(MEDIA_SCRIM, |(_, a)| *a)
}

/// Pictures the stream card can show (GPUI decodes these; a GIF plays).
pub(crate) const MEDIA_EXT: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];

/// The user's picture or GIF behind the stream card, if the prefs name one that still exists.
pub(crate) fn stream_bg(p: &Prefs) -> Option<PathBuf> {
    let path = PathBuf::from(p.stream_bg.as_deref()?.trim());
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    (MEDIA_EXT.contains(&ext.as_str()) && path.is_file()).then_some(path)
}

/// A surface colour: glass over the painting, solid on the plain frame.
pub(crate) fn surface(painting: bool, v: Rgb, alpha: f32) -> gpui_kit::Hsla {
    if painting { super::ca(v, alpha) } else { super::c(v) }
}

/// A stream or support card's own fill when it has no glass under it. Plain: the panel's dark grey
/// on the black frame, one tone for the card's tab and its content (Charles, 21 Nov: the content
/// should be the tab's tone; the darker inner well was the second tone). Over the painting: the
/// card's translucent glass colour.
pub(crate) fn card_bg(painting: bool, pal: &Palette) -> gpui_kit::Hsla {
    surface(painting, pal.panel, GLASS_CARD)
}

/// The frame layer for a theme: the user's picture's when one is set and baked (the frame takes
/// its theme from the stream card's picture, Charles 20 Nov), else the Harbour painting's.
pub(crate) fn frame_layer(dark: bool) -> Arc<RenderImage> {
    if let Some(u) = user_layers() {
        return u.frame[dark as usize].clone();
    }
    static DARK: OnceLock<Arc<RenderImage>> = OnceLock::new();
    static LIGHT: OnceLock<Arc<RenderImage>> = OnceLock::new();
    let cell = if dark { &DARK } else { &LIGHT };
    cell.get_or_init(|| Arc::new(to_render(frame_rgba(dark).as_ref().clone()))).clone()
}

/// The frame layer's pixels, for crops that must match it exactly (the tab's frame pieces).
pub(crate) fn frame_rgba(dark: bool) -> Arc<RgbaImage> {
    if let Some(u) = user_layers() {
        return u.frame_px[dark as usize].clone();
    }
    static DARK: OnceLock<Arc<RgbaImage>> = OnceLock::new();
    static LIGHT: OnceLock<Arc<RgbaImage>> = OnceLock::new();
    let cell = if dark { &DARK } else { &LIGHT };
    cell.get_or_init(|| Arc::new(baked(dark, FRAME))).clone()
}

/// The frost layers baked from the user's own picture, so the frame and every card carry the
/// picture's colours, spread wide by the blur, instead of the Harbour painting's navy.
pub(crate) struct UserLayers {
    pub path: PathBuf,
    /// By `dark as usize`.
    pub frame: [Arc<RenderImage>; 2],
    pub frame_px: [Arc<RgbaImage>; 2],
    pub card: [Arc<RgbaImage>; 2],
    /// The picture itself (a GIF's first frame), for the stream card's small corner pieces.
    pub pic: Arc<RgbaImage>,
    /// The tint over the frame: the picture's mean colour, held to the theme frame's luminance so
    /// text on the frame keeps the contrast it has over the painting.
    pub tint: [Rgb; 2],
}

static USER: std::sync::Mutex<Option<Arc<UserLayers>>> = std::sync::Mutex::new(None);
/// Bumped whenever the source of the layers changes, so cached crops are rebuilt.
static GEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) fn user_layers() -> Option<Arc<UserLayers>> {
    USER.lock().ok()?.clone()
}

pub(crate) fn generation() -> u32 {
    GEN.load(std::sync::atomic::Ordering::Relaxed)
}

/// Back to the painting's layers (no picture).
pub(crate) fn clear_user() {
    if let Ok(mut u) = USER.lock() {
        if u.take().is_some() {
            GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Bakes the layers from the picture at `path` (a GIF's first frame) and makes them current.
/// Blocking: call it off the UI thread. A picture that won't decode leaves the painting's.
pub(crate) fn warm_user(path: &std::path::Path) -> Result<(), String> {
    if user_layers().is_some_and(|u| u.path == path) {
        return Ok(());
    }
    let src = image::open(path).map_err(|e| format!("can't read {}: {e}", path.display()))?.to_rgba8();
    let mut frame = Vec::new();
    let mut frame_px = Vec::new();
    let mut card = Vec::new();
    let mut tint = Vec::new();
    for dark in [false, true] {
        let f = bake_rgba(&src, FRAME, dark);
        tint.push(mean_tint(&f, dark));
        frame.push(Arc::new(to_render(f.clone())));
        frame_px.push(Arc::new(f));
        card.push(Arc::new(bake_rgba(&src, CARD, dark)));
    }
    // Small enough to crop corners from quickly; the card shows the real file.
    let pic = if src.width() > 1600 { imageops::resize(&src, 1600, (1600.0 * src.height() as f32 / src.width() as f32).round().max(1.0) as u32, imageops::FilterType::Triangle) } else { src };
    let layers = UserLayers {
        path: path.to_path_buf(),
        frame: [frame[0].clone(), frame[1].clone()],
        frame_px: [frame_px[0].clone(), frame_px[1].clone()],
        card: [card[0].clone(), card[1].clone()],
        pic: Arc::new(pic),
        tint: [tint[0], tint[1]],
    };
    if let Ok(mut u) = USER.lock() {
        *u = Some(Arc::new(layers));
        GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

/// The frame tint for a baked picture layer: its mean colour, capped (dark) or floored (light)
/// at the theme frame's own luminance, so it is never brighter (dark) or darker (light) than the
/// tint the contrast tests hold the painting to.
pub(crate) fn mean_tint(layer: &RgbaImage, dark: bool) -> Rgb {
    let n = (layer.width() * layer.height()).max(1) as f32;
    let mut sum = [0f32; 3];
    for p in layer.pixels() {
        for k in 0..3 {
            sum[k] += to_lin(p.0[k]);
        }
    }
    let mean = [to_srgb(sum[0] / n), to_srgb(sum[1] / n), to_srgb(sum[2] / n)];
    let theme = if dark { cauldron_chrome::tokens::DARK.frame } else { cauldron_chrome::tokens::LIGHT.frame };
    let level = 0.2126 * to_lin(theme.0) + 0.7152 * to_lin(theme.1) + 0.0722 * to_lin(theme.2);
    let [r, g, b] = tone(mean, dark, level);
    Rgb(r, g, b)
}

/// The colour drawn over the frame layer: the picture's when its layers are current.
pub(crate) fn tint_colour(pal: &Palette, dark: bool) -> Rgb {
    user_layers().map_or(pal.frame, |u| u.tint[dark as usize])
}

fn card_cell(dark: bool) -> &'static OnceLock<Arc<RgbaImage>> {
    static DARK: OnceLock<Arc<RgbaImage>> = OnceLock::new();
    static LIGHT: OnceLock<Arc<RgbaImage>> = OnceLock::new();
    if dark { &DARK } else { &LIGHT }
}

/// The cards' light layer, once `warm` has baked it; None until then (cards fall back to the
/// single-depth glass), so the UI thread never waits on it.
pub(crate) fn card_source(dark: bool) -> Option<Arc<RgbaImage>> {
    if let Some(u) = user_layers() {
        return Some(u.card[dark as usize].clone());
    }
    card_cell(dark).get().cloned()
}

/// Bakes all four layers. Call it off the UI thread.
pub(crate) fn warm() {
    for dark in [true, false] {
        frame_layer(dark);
        card_cell(dark).get_or_init(|| Arc::new(baked(dark, CARD)));
    }
}

fn cache_path(dark: bool, f: Frost) -> PathBuf {
    Prefs::data_dir().join("cache").join(format!("backdrop-{}-{}-v{VERSION}.png", if dark { "night" } else { "day" }, f.work_w))
}

/// A frost of the painting, from the cache when it is there. Tests never touch the user's data
/// folder.
pub(crate) fn baked(dark: bool, f: Frost) -> RgbaImage {
    let path = cache_path(dark, f);
    if !cfg!(test) {
        if let Ok(img) = image::open(&path) {
            return img.to_rgba8();
        }
    }
    let img = bake(if dark { NIGHT } else { DAY }, f, dark);
    if !cfg!(test) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = img.save_with_format(&path, ImageFormat::Png);
    }
    img
}

/// Shrinks the painting to the frost's width, blurs it (its 1200px blur scaled to that width),
/// saturates it as CSS's `saturate()` does, then caps (dark) or floors (light) each pixel's
/// luminance, keeping its hue.
pub(crate) fn bake(webp: &[u8], f: Frost, dark: bool) -> RgbaImage {
    let img = image::load_from_memory_with_format(webp, ImageFormat::WebP).expect("bundled painting decodes").to_rgba8();
    bake_rgba(&img, f, dark)
}

/// `bake` for any decoded picture.
pub(crate) fn bake_rgba(img: &RgbaImage, f: Frost, dark: bool) -> RgbaImage {
    let h = ((f.work_w as f32) * img.height() as f32 / img.width() as f32).round().max(1.0) as u32;
    let small = imageops::resize(img, f.work_w, h, imageops::FilterType::Triangle);
    let mut out = imageops::blur(&small, f.blur * f.work_w as f32 / REF_W);
    for p in out.pixels_mut() {
        let [r, g, b, _] = p.0;
        let [r, g, b] = tone(saturate([r, g, b], f.saturate), dark, if dark { f.cap_dark } else { f.floor_light });
        p.0[0] = r;
        p.0[1] = g;
        p.0[2] = b;
    }
    out
}

fn to_lin(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}
fn to_srgb(l: f32) -> u8 {
    let l = l.clamp(0.0, 1.0);
    let c = if l <= 0.003_130_8 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    (c * 255.0).round() as u8
}
/// WCAG relative luminance of an sRGB colour.
#[cfg(test)]
pub(crate) fn luminance([r, g, b]: [u8; 3]) -> f32 {
    0.2126 * to_lin(r) + 0.7152 * to_lin(g) + 0.0722 * to_lin(b)
}

/// Dark: scales the colour down (in linear light) until its luminance is at most `level`. Light:
/// mixes it toward white until its luminance is at least `level`. Hue is kept either way.
pub(crate) fn tone([r, g, b]: [u8; 3], dark: bool, level: f32) -> [u8; 3] {
    let l = [to_lin(r), to_lin(g), to_lin(b)];
    let y = 0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2];
    let out = if dark {
        if y <= level {
            return [r, g, b];
        }
        let k = level / y;
        [l[0] * k, l[1] * k, l[2] * k]
    } else {
        if y >= level {
            return [r, g, b];
        }
        let k = (level - y) / (1.0 - y).max(1e-6);
        [l[0] + (1.0 - l[0]) * k, l[1] + (1.0 - l[1]) * k, l[2] + (1.0 - l[2]) * k]
    };
    [to_srgb(out[0]), to_srgb(out[1]), to_srgb(out[2])]
}

/// The part of a layer under `r`, in a `win`-sized window where the layer is drawn like CSS
/// `object-fit: cover` (scaled to cover, centred), at the layer's own resolution. Drawn stretched
/// over `r`, it lines up with the full-window layer.
pub(crate) fn crop(src: &RgbaImage, win: (f32, f32), r: Rect) -> RgbaImage {
    let (iw, ih) = (src.width() as f32, src.height() as f32);
    let scale = (win.0 / iw).max(win.1 / ih).max(1e-6);
    let (ox, oy) = ((win.0 - iw * scale) / 2.0, (win.1 - ih * scale) / 2.0);
    let x0 = ((r.x - ox) / scale).floor().clamp(0.0, iw - 1.0) as u32;
    let y0 = ((r.y - oy) / scale).floor().clamp(0.0, ih - 1.0) as u32;
    let x1 = ((r.x + r.w - ox) / scale).ceil().clamp(x0 as f32 + 1.0, iw) as u32;
    let y1 = ((r.y + r.h - oy) / scale).ceil().clamp(y0 as f32 + 1.0, ih) as u32;
    imageops::crop_imm(src, x0, y0, x1 - x0, y1 - y0).to_image()
}

/// The part of `src` under `r` as `crop` would cut it, in the layer's pixels: what a card drawn
/// with that crop stretched over `r` shows.
fn snapped(src: &RgbaImage, win: (f32, f32), r: Rect) -> [f32; 4] {
    let (iw, ih) = (src.width() as f32, src.height() as f32);
    let scale = (win.0 / iw).max(win.1 / ih).max(1e-6);
    let (ox, oy) = ((win.0 - iw * scale) / 2.0, (win.1 - ih * scale) / 2.0);
    let x0 = ((r.x - ox) / scale).floor().clamp(0.0, iw - 1.0);
    let y0 = ((r.y - oy) / scale).floor().clamp(0.0, ih - 1.0);
    let x1 = ((r.x + r.w - ox) / scale).ceil().clamp(x0 + 1.0, iw);
    let y1 = ((r.y + r.h - oy) / scale).ceil().clamp(y0 + 1.0, ih);
    [x0, y0, x1, y1]
}

/// The part of `src` under `r` exactly, for a layer drawn `object-fit: cover` over `win`.
fn exact(src: &RgbaImage, win: (f32, f32), r: Rect) -> [f32; 4] {
    let (iw, ih) = (src.width() as f32, src.height() as f32);
    let scale = (win.0 / iw).max(win.1 / ih).max(1e-6);
    let (ox, oy) = ((win.0 - iw * scale) / 2.0, (win.1 - ih * scale) / 2.0);
    [(r.x - ox) / scale, (r.y - oy) / scale, (r.x + r.w - ox) / scale, (r.y + r.h - oy) / scale]
}

/// `src` over the layer-pixel rect `at`, resampled (bilinear, pixel centres) to `w` x `h`.
fn resample(src: &RgbaImage, at: [f32; 4], w: u32, h: u32) -> RgbaImage {
    let (sw, sh) = ((at[2] - at[0]) / w as f32, (at[3] - at[1]) / h as f32);
    let (mx, my) = (src.width() as f32 - 1.0, src.height() as f32 - 1.0);
    RgbaImage::from_fn(w, h, |x, y| {
        let u = (at[0] + (x as f32 + 0.5) * sw - 0.5).clamp(0.0, mx);
        let v = (at[1] + (y as f32 + 0.5) * sh - 0.5).clamp(0.0, my);
        imageops::interpolate_bilinear(src, u, v).unwrap_or(*src.get_pixel(u as u32, v as u32))
    })
}

/// The cards' crops of the light layer, by card: rebuilt only when a card's rect, the window or
/// the theme changes. A replaced image is handed back so the caller frees it from GPUI's atlas.
#[derive(Default)]
pub(crate) struct GlassCache {
    slots: Vec<(&'static str, (bool, [i32; 7]), Arc<RenderImage>)>,
}

impl GlassCache {
    /// The crop for card `id` at `r`; `stale` gets the image it replaced, if any. While the layout
    /// is `moving` a card keeps its last crop, stretched, so an animation does not upload a new
    /// image every frame; the crop catches up when it settles.
    pub(crate) fn get(&mut self, id: &'static str, dark: bool, win: (f32, f32), r: Rect, moving: bool, stale: &mut Vec<Arc<RenderImage>>) -> Option<Arc<RenderImage>> {
        let src = card_source(dark)?;
        let mut key = (dark, [r.x, r.y, r.w, r.h, win.0, win.1, 0.0].map(|v| v.round() as i32));
        key.1[6] = generation() as i32;
        if let Some(s) = self.slots.iter().find(|s| s.0 == id && (s.1 == key || (moving && s.1 .0 == dark && s.1 .1[6] == key.1[6]))) {
            return Some(s.2.clone());
        }
        let img = Arc::new(to_render(crop(&src, win, r)));
        if let Some(k) = self.slots.iter().position(|s| s.0 == id) {
            let (_, _, old) = self.slots.swap_remove(k);
            stale.push(old);
        }
        self.slots.push((id, key, img.clone()));
        Some(img)
    }
}

impl GlassCache {
    /// A crop of any layer `src` for piece `id` at `r`, where `src` covers a `win`-sized area
    /// (object-fit cover). Rebuilt when the rect, the area, the theme or the layers change.
    ///
    /// The piece is resampled at its own size, not cut on the layer's pixel grid, so a small piece
    /// lines up with what is around it: with `within`, the card rect whose own crop (`get`) the
    /// piece must match; without, the layer as drawn over the whole area.
    pub(crate) fn piece(&mut self, id: &'static str, src: &RgbaImage, dark: bool, win: (f32, f32), r: Rect, within: Option<Rect>, stale: &mut Vec<Arc<RenderImage>>) -> Arc<RenderImage> {
        let mut key = (dark, [r.x, r.y, r.w, r.h, win.0, win.1, 0.0].map(|v| v.round() as i32));
        key.1[6] = generation() as i32;
        if let Some(s) = self.slots.iter().find(|s| s.0 == id && s.1 == key) {
            return s.2.clone();
        }
        let at = match within {
            Some(m) => {
                let c = snapped(src, win, m);
                let (kx, ky) = ((c[2] - c[0]) / m.w.max(1.0), (c[3] - c[1]) / m.h.max(1.0));
                [c[0] + (r.x - m.x) * kx, c[1] + (r.y - m.y) * ky, c[0] + (r.right() - m.x) * kx, c[1] + (r.y + r.h - m.y) * ky]
            }
            None => exact(src, win, r),
        };
        // Twice the piece's size in points: sharp enough on a 2x screen, and the layers are soft.
        let img = Arc::new(to_render(resample(src, at, (r.w * 2.0).ceil().max(1.0) as u32, (r.h * 2.0).ceil().max(1.0) as u32)));
        if let Some(k) = self.slots.iter().position(|s| s.0 == id) {
            let (_, _, old) = self.slots.swap_remove(k);
            stale.push(old);
        }
        self.slots.push((id, key, img.clone()));
        img
    }
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
    surface(true, tint_colour(pal, dark), frame_tint(dark))
}

/// How frosted the frame and sidebar are (Settings > Appearance > Frost; Charles, 19 Nov: Heavy is
/// the board's glass B). Only the tint over the heavy layer changes; the cards keep their light
/// glass, and a heavier tint only raises the frame's contrast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FrostLevel {
    Light,
    Medium,
    Heavy,
}

impl FrostLevel {
    pub(crate) const ALL: [FrostLevel; 3] = [FrostLevel::Light, FrostLevel::Medium, FrostLevel::Heavy];
    pub(crate) fn name(self) -> &'static str {
        match self {
            FrostLevel::Light => "Light",
            FrostLevel::Medium => "Medium",
            FrostLevel::Heavy => "Heavy",
        }
    }
    /// The pref's value; anything else (or nothing) is Heavy.
    pub(crate) fn from_pref(p: &Prefs) -> Self {
        match p.frost.as_deref() {
            Some("light") => FrostLevel::Light,
            Some("medium") => FrostLevel::Medium,
            _ => FrostLevel::Heavy,
        }
    }
    pub(crate) fn pref(self) -> String {
        self.name().to_ascii_lowercase()
    }
    /// The frame tint at this level, dark and light.
    pub(crate) fn alpha(self, dark: bool) -> f32 {
        match (self, dark) {
            (FrostLevel::Light, d) => frame_tint(d),
            (FrostLevel::Medium, true) => 0.42,
            (FrostLevel::Medium, false) => 0.48,
            (FrostLevel::Heavy, true) => 0.62,
            (FrostLevel::Heavy, false) => 0.66,
        }
    }
}

/// The tint over the frame and sidebar at a frost level.
pub(crate) fn frame_tint_at(pal: &Palette, dark: bool, level: FrostLevel) -> gpui_kit::Hsla {
    surface(true, tint_colour(pal, dark), level.alpha(dark))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rough(im: &RgbaImage) -> f64 {
        let mut sum = 0u64;
        for y in 0..im.height() {
            for x in 1..im.width() {
                sum += (im.get_pixel(x, y).0[1] as i32 - im.get_pixel(x - 1, y).0[1] as i32).unsigned_abs() as u64;
            }
        }
        sum as f64 / (im.width() * im.height()) as f64
    }

    /// Both frosts are the painting shrunk to their width, its shape kept, and blurred; the cards'
    /// frost keeps more detail than the frame's (two depths of glass).
    #[test]
    fn both_frosts_are_the_painting_shrunk_and_blurred_the_card_one_less() {
        for (webp, dark) in [(NIGHT, true), (DAY, false)] {
            let raw = image::load_from_memory_with_format(webp, ImageFormat::WebP).unwrap().to_rgba8();
            let frame = bake(webp, FRAME, dark);
            let card = bake(webp, CARD, dark);
            assert_eq!((frame.width(), card.width()), (FRAME.work_w, CARD.work_w));
            let ratio = |w: u32, h: u32| w as f32 / h as f32;
            for out in [&frame, &card] {
                assert!((ratio(out.width(), out.height()) - ratio(raw.width(), raw.height())).abs() < 0.01, "aspect kept");
                let small = imageops::resize(&raw, out.width(), out.height(), imageops::FilterType::Triangle);
                assert!(rough(out) * 2.0 < rough(&small), "blurred: {} vs {}", rough(out), rough(&small));
            }
            let card_small = imageops::resize(&card, frame.width(), frame.height(), imageops::FilterType::Triangle);
            assert!(rough(&frame) < rough(&card_small), "the frame is frostier: {} vs {}", rough(&frame), rough(&card_small));
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

    /// Dark caps a bright pixel's luminance, light lifts a dark one; dim (dark) or bright (light)
    /// pixels pass through, and the channels keep their order (the hue).
    #[test]
    fn tone_caps_in_dark_and_floors_in_light() {
        let sky = [240, 180, 90];
        let d = tone(sky, true, 0.05);
        assert!((luminance(d) - 0.05).abs() < 0.006, "capped: {}", luminance(d));
        assert!(d[0] > d[1] && d[1] > d[2], "hue kept: {d:?}");
        assert_eq!(tone([20, 20, 30], true, 0.05), [20, 20, 30]);
        let l = tone([30, 60, 120], false, 0.6);
        assert!((luminance(l) - 0.6).abs() < 0.01, "lifted: {}", luminance(l));
        assert!(l[2] > l[1] && l[1] > l[0], "hue kept: {l:?}");
        assert_eq!(tone([250, 250, 250], false, 0.6), [250, 250, 250]);
    }

    fn contrast(a: f32, b: f32) -> f32 {
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Worst case over every pixel of the baked layers under the runtime tint: the sidebar's grey
    /// labels (fg3) keep 4.5:1 on the frame; on the cards the body text (fg2) keeps 4.5:1 and fg3
    /// 3.5:1 even on the brightest cloud.
    #[test]
    fn small_text_keeps_its_contrast_on_both_frosts() {
        for (webp, dark, pal) in [(NIGHT, true, &cauldron_chrome::tokens::DARK), (DAY, false, &cauldron_chrome::tokens::LIGHT)] {
            let t = frame_tint(dark);
            let fr = pal.frame;
            let mix = |p: [u8; 3]| -> [u8; 3] {
                let m = |v: u8, f: u8| (v as f32 * (1.0 - t) + f as f32 * t).round() as u8;
                [m(p[0], fr.0), m(p[1], fr.1), m(p[2], fr.2)]
            };
            let lum = |c: Rgb| luminance([c.0, c.1, c.2]);
            for (f, fg3_min, fg2_min) in [(FRAME, 4.5, 4.5), (CARD, 3.5, 4.5)] {
                let img = bake(webp, f, dark);
                let (mut w3, mut w2) = (f32::MAX, f32::MAX);
                for p in img.pixels() {
                    let bg = luminance(mix([p.0[0], p.0[1], p.0[2]]));
                    w3 = w3.min(contrast(bg, lum(pal.fg3)));
                    w2 = w2.min(contrast(bg, lum(pal.fg2)));
                }
                assert!(w3 >= fg3_min && w2 >= fg2_min, "dark {dark} blur {}: fg3 {w3:.2} fg2 {w2:.2}", f.blur);
            }
        }
    }

    /// A card's crop is the part of the light layer under it: a window twice the layer's size maps
    /// a rect to half its size, from the matching corner.
    #[test]
    fn a_crop_lines_up_with_the_full_window_layer() {
        let mut src = RgbaImage::new(100, 50);
        src.put_pixel(60, 30, image::Rgba([255, 0, 0, 255]));
        let r = Rect { x: 100.0, y: 40.0, w: 40.0, h: 40.0 };
        let c = crop(&src, (200.0, 100.0), r);
        assert_eq!((c.width(), c.height()), (20, 20));
        assert_eq!(c.get_pixel(10, 10).0, [255, 0, 0, 255], "the marked pixel sits where the window has it");
        // A wider window crops the layer's height (cover): the centre stays the centre.
        let c = crop(&src, (400.0, 100.0), Rect { x: 0.0, y: 0.0, w: 400.0, h: 100.0 });
        assert!(c.width() == 100 && c.height().abs_diff(25) <= 1, "{}x{}", c.width(), c.height());
    }

    /// Glass is the default again (Charles, 19 Nov): a fresh prefs file and the old app's
    /// paintings draw it; only "plain" draws the plain frame.
    #[test]
    fn glass_is_the_default_and_plain_is_a_choice() {
        let mut p = Prefs::default();
        assert!(wants_painting(&p), "fresh prefs: the painting");
        p.backdrop = "harbour".into();
        assert!(wants_painting(&p));
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
        assert_eq!((frame_tint(true), frame_tint(false)), (0.22, 0.30));
        for dark in [true, false] {
            let a: Vec<f32> = FrostLevel::ALL.iter().map(|l| l.alpha(dark)).collect();
            assert!(a[0] < a[1] && a[1] < a[2], "each level frostier: {a:?}");
            assert_eq!(a[0], frame_tint(dark), "Light is the tint the contrast test checks");
        }
        let mut p = Prefs::default();
        assert_eq!(FrostLevel::from_pref(&p), FrostLevel::Heavy, "Heavy by default");
        p.frost = Some(FrostLevel::Medium.pref());
        assert_eq!(FrostLevel::from_pref(&p), FrostLevel::Medium);
    }

    /// A picture's frame tint is its own colour (a warm picture gives a warm frame, not the
    /// theme's navy or grey), held to the theme frame's luminance so text keeps its contrast.
    #[test]
    fn a_pictures_tint_is_its_colour_at_the_themes_luminance() {
        let warm = RgbaImage::from_pixel(8, 8, image::Rgba([220, 120, 40, 255]));
        for dark in [true, false] {
            let t = mean_tint(&warm, dark);
            assert!(t.0 > t.1 && t.1 > t.2, "warm stays warm: {t:?}");
            let theme = if dark { cauldron_chrome::tokens::DARK.frame } else { cauldron_chrome::tokens::LIGHT.frame };
            let (lt, lf) = (luminance([t.0, t.1, t.2]), luminance([theme.0, theme.1, theme.2]));
            if dark {
                assert!(lt <= lf + 0.005, "no brighter than the dark frame: {lt} vs {lf}");
            } else {
                assert!(lt + 0.01 >= lf, "no darker than the light frame: {lt} vs {lf}");
            }
        }
    }
}
