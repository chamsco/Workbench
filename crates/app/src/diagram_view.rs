//! Draws a core `Diagram` with plain divs: nodes absolutely positioned in
//! columns, edges as orthogonal 1.5px segments. No paths, no canvas, so it
//! themes and hit-tests like any other element.

use backspace_core::diagram::{Diagram, Node, Tone};
use backspace_core::FileChange;
use gpui_kit::component::{h_flex, v_flex, Theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

const NW: f32 = 156.;
const NH: f32 = 50.;
const GX: f32 = 40.;
const GY: f32 = 12.;
const STROKE: f32 = 1.5;

fn tone(t: &Theme, tone: Tone) -> Hsla {
    match tone {
        Tone::Neutral => t.border,
        Tone::Done => t.success,
        Tone::Active => t.info,
        Tone::Warn => t.warning,
        Tone::Fail => t.danger,
    }
}

fn pos(n: &Node) -> (f32, f32) {
    (n.layer as f32 * (NW + GX), n.row as f32 * (NH + GY))
}

fn seg(x: f32, y: f32, w: f32, h: f32, c: Hsla) -> Div {
    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(w))
        .h(px(h))
        .bg(c)
}

/// The drawing alone, in its own horizontal scroller.
pub fn drawing(
    d: &Diagram,
    t: &Theme,
    id: impl Into<ElementId>,
    mono: &'static str,
) -> impl IntoElement {
    let layers = d.layers().max(1) as f32;
    let rows = d.rows().max(1) as f32;
    let w = layers * NW + (layers - 1.) * GX;
    let h = rows * NH + (rows - 1.) * GY;
    let line = t.muted_foreground.opacity(0.7);

    let mut canvas = div().relative().w(px(w)).h(px(h)).flex_none();
    for &(a, b) in &d.edges {
        let (ax, ay) = pos(&d.nodes[a]);
        let (bx, by) = pos(&d.nodes[b]);
        let (x1, y1, x2, y2) = (ax + NW, ay + NH / 2., bx, by + NH / 2.);
        let xm = x1 + GX / 2.;
        canvas = canvas
            .child(seg(x1, y1 - STROKE / 2., xm - x1, STROKE, line))
            .child(seg(
                xm - STROKE / 2.,
                y1.min(y2),
                STROKE,
                (y2 - y1).abs() + STROKE,
                line,
            ))
            .child(seg(
                xm,
                y2 - STROKE / 2.,
                (x2 - xm - 5.).max(0.),
                STROKE,
                line,
            ))
            .child(
                div()
                    .absolute()
                    .left(px(x2 - 8.))
                    .top(px(y2 - 7.))
                    .text_size(px(10.))
                    .line_height(px(14.))
                    .text_color(line)
                    .child("▶"),
            );
    }
    for n in &d.nodes {
        let (x, y) = pos(n);
        let c = tone(t, n.tone);
        canvas = canvas.child(
            v_flex()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(NW))
                .h(px(NH))
                .px_2()
                .justify_center()
                .rounded_md()
                .border_1()
                .border_color(c)
                .bg(t.background)
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(n.label.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.5))
                        .font_family(mono)
                        .text_color(t.muted_foreground)
                        .truncate()
                        .child(n.sub.clone()),
                ),
        );
    }
    div()
        .id(id)
        .w_full()
        .overflow_x_scroll()
        .pb_1()
        .child(canvas)
}

/// Facts, drawing, warnings and file bars: the top of every review card.
pub fn card(d: &Diagram, t: &Theme, id: usize, mono: &'static str) -> impl IntoElement {
    v_flex()
        .gap_2()
        .p_3()
        .rounded_md()
        .bg(t.muted.opacity(0.35))
        .border_1()
        .border_color(t.border)
        .child(
            h_flex()
                .flex_wrap()
                .gap_x_4()
                .gap_y_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(d.title.clone()),
                )
                .children(d.facts.iter().map(|(k, v)| {
                    h_flex()
                        .gap_1()
                        .text_xs()
                        .child(div().font_family(mono).child(v.clone()))
                        .child(div().text_color(t.muted_foreground).child(k.clone()))
                })),
        )
        .child(drawing(d, t, ("diagram", id), mono))
        .when(!d.warnings.is_empty(), |el| {
            el.child(v_flex().gap_0p5().children(d.warnings.iter().map(|w| {
                h_flex()
                    .gap_1p5()
                    .text_xs()
                    .text_color(t.warning)
                    .child("!")
                    .child(div().min_w_0().child(w.clone()))
            })))
        })
        .when(!d.files.is_empty(), |el| el.child(files(&d.files, t, mono)))
}

fn files(changes: &[FileChange], t: &Theme, mono: &'static str) -> impl IntoElement {
    let max = changes
        .iter()
        .map(|c| c.added + c.removed)
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let mut sorted: Vec<&FileChange> = changes.iter().collect();
    sorted.sort_by_key(|c| std::cmp::Reverse(c.added + c.removed));
    let extra = sorted.len().saturating_sub(8);
    v_flex()
        .gap_0p5()
        .children(sorted.into_iter().take(8).map(|c| {
            let scale = 110. / max;
            h_flex()
                .gap_2()
                .text_size(px(11.))
                .font_family(mono)
                .child(div().flex_1().min_w_0().truncate().child(c.path.clone()))
                .child(
                    h_flex()
                        .w(px(112.))
                        .flex_none()
                        .child(
                            div()
                                .h(px(6.))
                                .w(px((c.added as f32 * scale).max(if c.added > 0 {
                                    2.
                                } else {
                                    0.
                                })))
                                .bg(t.success),
                        )
                        .child(
                            div()
                                .h(px(6.))
                                .w(px((c.removed as f32 * scale).max(if c.removed > 0 {
                                    2.
                                } else {
                                    0.
                                })))
                                .bg(t.danger),
                        ),
                )
                .child(
                    div()
                        .w(px(76.))
                        .flex_none()
                        .text_color(t.muted_foreground)
                        .child(format!("+{} −{}", c.added, c.removed)),
                )
        }))
        .when(extra > 0, |el| {
            el.child(
                div()
                    .text_xs()
                    .text_color(t.muted_foreground)
                    .child(format!("and {extra} more files")),
            )
        })
}
