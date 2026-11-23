//! Tuff UI's Surface layers: Shell (the frame), Panel (its content, a step in) and Inset (a deeper
//! block inside), with ShellHeader (icon tile, title, subtitle, aside). Borderless: depth comes
//! from three tones, not lines. The step blocks in the transcript use them.

use super::*;
use gpui_kit::prelude::FluentBuilder;

/// The outer frame: a tint of the ink over whatever is behind, with a little padding so the panel
/// inside reads as sunk into it.
pub(super) fn shell(pal: &'static Palette) -> Div {
    div().flex().flex_col().gap(px(4.)).p(px(4.)).rounded(px(radius::INSET + 4.)).bg(ca(pal.fg, 0.05))
}

/// The content area inside a shell.
pub(super) fn panel(pal: &'static Palette) -> Div {
    div().flex().flex_col().gap(px(6.)).p(px(8.)).rounded(px(radius::INSET)).bg(c(pal.inset))
}

/// A deeper block inside a panel: code, payloads, details.
pub(super) fn inset(pal: &'static Palette) -> Div {
    div().rounded(px(radius::CONTROL)).bg(ca(pal.fg, 0.04)).px(px(10.)).py(px(7.))
}

/// A 28px tinted square holding a glyph.
pub(super) fn icon_tile(name: &'static str, pal: &'static Palette) -> Div {
    div().size(px(28.)).flex_none().flex().items_center().justify_center().rounded(px(radius::CONTROL)).bg(ca(pal.fg, 0.07)).child(glyph(name, 14., c(pal.fg2)))
}

/// Icon tile, title and subtitle on the left, `aside` on the right.
pub(super) fn shell_header(icon: &'static str, title: String, subtitle: String, aside: Option<AnyElement>, pal: &'static Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(4.))
        .pt(px(2.))
        .child(icon_tile(icon, pal))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().truncate().text_size(px(12.5)).font_weight(FontWeight::MEDIUM).text_color(c(pal.fg)).child(title))
                .child(div().truncate().text_size(px(11.5)).text_color(c(pal.fg3)).child(subtitle)),
        )
        .when_some(aside, |d, a| d.child(a))
}

/// A block in the transcript (a command, a tool call, a failure, a thought opened): solid, so it
/// reads over a painted backdrop or a glass card (Charles, 23 Nov: "way too transparent, hard to
/// read"). The panel tone with a hairline edge and the buttons' small lift; every block uses this
/// one face so they look alike.
pub(super) fn block(pal: &'static Palette) -> Div {
    div().rounded(px(radius::INSET)).bg(c(pal.panel)).border_1().border_color(c(pal.line_strong)).shadow(super::lift(super::Lift::Button, pal))
}
