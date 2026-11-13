# Wallpaper, dither and frosted glass in GPUI: what is possible (13 Nov)

These are findings only. Nothing is built: the coordinator's board picks the direction first. Read against gpui-pre 0.3.6 and gpui-kit 0.6.6, the versions in `Cargo.lock`.

## What GPUI gives us

- **No per-element backdrop blur.** There is no `backdrop-filter`. `WindowBackgroundAppearance::Blurred` blurs the desktop behind the whole window (macOS vibrancy), not what lies under one element.
- **Clipping is rectangular.** `ContentMask` holds only bounds (`window.rs`, `pub struct ContentMask { bounds }`). `overflow_hidden()` on a rounded div clips its children to the square, so an image placed inside it at an offset shows square corners.
- **An image can round its own corners.** The `img` element paints with its style's `corner_radii` (`elements/img.rs`, around line 494). So a picture that is exactly the element's size, given `.rounded(px(r))`, is drawn rounded.
- **No object-position.** `object_fit` (Contain, Cover, Fill, ...) exists, but there is no offset into the picture. To show "the part of the wallpaper behind this element", the crop has to be made on the CPU.
- **Pictures from memory.** `ImageSource::Render(Arc<RenderImage>)` takes frames we build ourselves (`RenderImage::new`, `assets.rs`), so a crop or a dithered frame made in Rust can be drawn directly with no file. The `image` crate (0.25.10, with WebP) is already in the dependency tree through GPUI.

## The plan this supports

1. **Wallpaper.** Decode the painting once: the legacy paintings in `legacy/tauri-app/ui/art/` are WebP, 676 KB in all.
2. **Dither once per size.** Port `dither()` from `legacy/tauri-app/ui/art.js` (an 8x8 Bayer matrix, stronger on edges, fading out toward the foot) to Rust. It works on a small frame (the window size divided by the cell, 2 by default) and is scaled up with hard pixels. Cache it by (picture, cell, width, height); only a resize redoes it. At 1200x720 with cell 2, that is 600x360 = 216k pixels, a few milliseconds on the CPU.
3. **Glass by pre-blurred crops.**
   - Blur the dithered wallpaper once per size (`image::imageops::blur`, or a box blur, which is cheaper).
   - For each glass element (the card, a pane, the composer box, the terminal), crop the blurred frame to the element's rect at the same offset as the wallpaper and add the tint (the panel token at around 60 to 75% opacity).
   - Hand the crop to `img(...).rounded(radius)`, under the element's content.
   - Cache crops by rect, so only a layout change (resize, a moved line, the drawer) recomputes them. During a drag, the crop for a moving rect would be redone every frame; the plan is to drop to a flat tint while dragging and crop again on release.
4. **Where rects come from.** The shell's panes, the card, the drawer and the terminal all have their rects from `layout::solve` and `panes::rects`, so their crops need nothing new. Elements laid out inside a view (the composer box) need their bounds read after layout (`on_children_prepainted` or a canvas), one frame late. Those are better kept as flat tint.

## Limits

- Glass is only true glass over the wallpaper, not over other UI. Two stacked glass elements show the wallpaper through both.
- Animated wallpaper (a GIF, a shader, the ASCII sea in `ocean.js`) would need the dither and the blur every frame. A still painting is cheap; motion is not. A GIF could pre-render its frames once.
- Text contrast on glass has to be checked against the darkest and lightest wallpaper under it. The token tests can take a "glass over picture" case.
- Licence: the paintings and the dither are Berth's (MIT). The notice is already in `THIRD_PARTY_NOTICES.md` and stays.
