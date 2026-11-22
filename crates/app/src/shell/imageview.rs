//! A picture over everything: a step's screenshot, a link to an image, a board node's example.
//! The stream shows the picture as a link (its name); a click opens it here, fitted to the
//! window, with its name, Open outside and Close (Charles, 22 Nov: "a hyperlink to the preview").

use std::path::{Path, PathBuf};

use super::*;

pub(crate) fn is_image(p: &Path) -> bool {
    let low = p.to_string_lossy().to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg"].iter().any(|e| low.ends_with(e))
}

impl Cauldron {
    #[inline(never)]
    pub(super) fn image_view_el(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p: PathBuf = self.image_view.clone()?;
        let name = p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().to_string());
        let (fg, fg2) = (c(pal.fg), c(pal.fg2));
        let btn = |id: &'static str, label: &'static str| {
            div().id(id).h(px(30.)).px(px(12.)).flex().items_center().rounded(px(radius::CONTROL)).bg(c(pal.raised)).border_1().border_color(c(pal.line)).text_size(px(12.5)).text_color(fg).cursor_pointer().hover(|d| d.bg(c(pal.hover))).aria_label(label).child(label)
        };
        let outside = p.clone();
        let bar = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .w_full()
            .child(glyph("image", 14., fg2))
            .child(div().id("image-name").min_w_0().flex_1().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(gpui::white()).aria_label(name.clone()).child(name).test_support())
            .child(btn("image-outside", "Open outside").on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.open_link(super::links::Link::File(outside.clone()), false, cx);
            })))
            .child(btn("image-close", "Close").on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.image_view = None;
                cx.notify();
            })));
        let scrim = div()
            .id("image-view")
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(28.))
            .bg(gpui::rgba(0x0b0c12eb))
            .occlude()
            .on_click(cx.listener(|this, _, _, cx| {
                this.image_view = None;
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" {
                    this.image_view = None;
                    cx.notify();
                }
            }))
            .child(bar)
            .child(div().flex_1().min_h_0().w_full().flex().items_center().justify_center().child(img(p).size_full().object_fit(ObjectFit::Contain)))
            .test_support();
        Some(deferred(scrim).with_priority(7).into_any_element())
    }
}
