//! The review canvas: a core `Diagram` on a dotted, pannable, zoomable
//! stage. Geometry is the HTML replica's (210x60 nodes, 80/34 gaps, waves as
//! columns); dots, edges, arrowheads and node boxes are painted in one
//! canvas pass, labels are text elements laid over it.

use std::cell::Cell;
use std::rc::Rc;

use backspace_core::diagram::{Diagram, Node, Tone};
use backspace_core::FileChange;
use gpui_kit::*;

use crate::pal::Pal;

const NW: f32 = 210.;
const NH: f32 = 60.;
const GX: f32 = 80.;
const GY: f32 = 34.;

/// World point `(x, y)` sits at the stage's top-left; one world unit is `k` px.
#[derive(Clone, Copy, Debug)]
pub struct Cam {
    pub x: f32,
    pub y: f32,
    pub k: f32,
}

fn pos(n: &Node) -> (f32, f32) {
    (
        40. + n.layer as f32 * (NW + GX),
        120. + n.row as f32 * (NH + GY),
    )
}

/// The drawing's world-space box: (x, y, w, h).
fn extent(d: &Diagram) -> (f32, f32, f32, f32) {
    let max_x = d.nodes.iter().map(|n| pos(n).0).fold(0., f32::max);
    let max_y = d.nodes.iter().map(|n| pos(n).1).fold(0., f32::max);
    (0., 80., max_x + NW + 40., max_y + NH - 80. + 40.)
}

/// Never magnify past 1.25x, and keep the drawing in the band above the
/// review card (`card_h` px tall at the bottom right).
pub fn fit(d: &Diagram, sw: f32, sh: f32, card_h: f32) -> Cam {
    let (bx, by, bw, bh) = extent(d);
    let band = (1. - (card_h + 32.) / sh).max(0.5);
    let k = 1.25_f32.min(sw / (bw + 80.)).min(sh * band / (bh + 80.));
    let (w, h) = (sw / k, sh / k);
    Cam {
        x: bx + bw / 2. - w / 2.,
        y: by + bh / 2. - h * band / 2. - 20. / k,
        k,
    }
}

impl Cam {
    /// Zoom by `f` (view width multiplier, as the wheel gives it) around the
    /// stage-relative point `(px, py)`, within the replica's 300..8000 range.
    pub fn zoom(&mut self, f: f32, px: f32, py: f32, sw: f32) {
        let wx = self.x + px / self.k;
        let wy = self.y + py / self.k;
        let w = (sw / self.k * f).clamp(300., 8000.);
        self.k = sw / w;
        self.x = wx - px / self.k;
        self.y = wy - py / self.k;
    }
}

fn tone(p: &Pal, t: Tone) -> Hsla {
    match t {
        Tone::Done => p.green,
        Tone::Warn => p.amber,
        Tone::Neutral => p.fg4,
        Tone::Active => p.blue,
        Tone::Fail => p.red,
    }
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        s.chars().take(n - 1).collect::<String>() + "…"
    } else {
        s.to_string()
    }
}

/// The painted stage plus label overlay. `bounds_out` receives the stage's
/// window bounds each frame so pan, zoom and fit can use its size.
pub fn layer(
    d: &Diagram,
    cam: Cam,
    p: &Pal,
    ui: SharedString,
    mono: &'static str,
    bounds_out: Rc<Cell<Bounds<Pixels>>>,
) -> impl IntoElement {
    let sx = move |wx: f32| (wx - cam.x) * cam.k;
    let sy = move |wy: f32| (wy - cam.y) * cam.k;
    let nodes: Vec<(f32, f32, Hsla)> = d
        .nodes
        .iter()
        .map(|n| {
            let (x, y) = pos(n);
            (x, y, tone(p, n.tone))
        })
        .collect();
    let edges: Vec<((f32, f32), (f32, f32))> = d
        .edges
        .iter()
        .map(|&(a, b)| (pos(&d.nodes[a]), pos(&d.nodes[b])))
        .collect();
    let (dot, line, node_bg) = (p.grid_dot, p.fg3, p.node);

    let paint = canvas(
        move |b, _, _| {
            bounds_out.set(b);
            b
        },
        move |b, _, window, _| {
            let o = b.origin;
            let at = |x: f32, y: f32| point(o.x + px(x), o.y + px(y));
            // Dot grid every 22 world units, offset like the SVG pattern.
            let step = 22. * cam.k;
            if step >= 5. {
                let r = (1.1 * cam.k).max(0.6);
                let first_x = ((cam.x - 1.5) / 22.).ceil() * 22. + 1.5;
                let first_y = ((cam.y - 1.5) / 22.).ceil() * 22. + 1.5;
                let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
                let mut y = sy(first_y);
                while y < h {
                    let mut x = sx(first_x);
                    while x < w {
                        window.paint_quad(
                            fill(
                                Bounds::new(at(x - r, y - r), size(px(2. * r), px(2. * r))),
                                dot,
                            )
                            .corner_radii(px(r)),
                        );
                        x += step;
                    }
                    y += step;
                }
            }
            for &((ax, ay), (bx, by)) in &edges {
                let (x1, y1) = (ax + NW, ay + NH / 2.);
                let (x2, y2) = (bx, by + NH / 2.);
                let xm = x1 + GX / 2.;
                let mut pb = PathBuilder::stroke(px(1.5 * cam.k));
                pb.move_to(at(sx(x1), sy(y1)));
                pb.line_to(at(sx(xm), sy(y1)));
                pb.line_to(at(sx(xm), sy(y2)));
                pb.line_to(at(sx(x2 - 8.), sy(y2)));
                if let Ok(path) = pb.build() {
                    window.paint_path(path, line);
                }
                // Arrowhead: the SVG marker is 7 units long at stroke 1.5.
                let mut head = PathBuilder::fill();
                head.move_to(at(sx(x2 - 10.5), sy(y2 - 5.25)));
                head.line_to(at(sx(x2), sy(y2)));
                head.line_to(at(sx(x2 - 10.5), sy(y2 + 5.25)));
                head.close();
                if let Ok(path) = head.build() {
                    window.paint_path(path, line);
                }
            }
            for &(x, y, c) in &nodes {
                window.paint_quad(quad(
                    Bounds::new(at(sx(x), sy(y)), size(px(NW * cam.k), px(NH * cam.k))),
                    px(10. * cam.k),
                    node_bg,
                    px(1.6 * cam.k),
                    c,
                    BorderStyle::Solid,
                ));
            }
        },
    )
    .absolute()
    .inset_0()
    .size_full();

    let k = cam.k;
    let layers = d.nodes.iter().map(|n| n.layer).max().map_or(0, |l| l + 1);
    let mut el = div().absolute().inset_0().overflow_hidden().child(paint);
    if layers > 1 {
        for l in 0..layers {
            el = el.child(
                div()
                    .absolute()
                    .left(px(sx(40. + l as f32 * (NW + GX))))
                    .top(px(sy(100. - 10.)))
                    .text_size(px(10.5 * k))
                    .line_height(px(12. * k))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.fg3)
                    .font_family(ui.clone())
                    .child(spaced(&format!("WAVE {}", l + 1))),
            );
        }
    }
    for n in &d.nodes {
        let (x, y) = pos(n);
        el = el
            .child(
                div()
                    .absolute()
                    .left(px(sx(x + 14.)))
                    .top(px(sy(y + 25. - 13.)))
                    .text_size(px(14. * k))
                    .line_height(px(16. * k))
                    .font_weight(FontWeight::SEMIBOLD)
                    .font_family(ui.clone())
                    .text_color(p.fg)
                    .whitespace_nowrap()
                    .child(clip(&n.label, 24)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(sx(x + 14.)))
                    .top(px(sy(y + 44. - 10.)))
                    .text_size(px(11. * k))
                    .line_height(px(13. * k))
                    .font_family(mono)
                    .text_color(p.fg3)
                    .whitespace_nowrap()
                    .child(clip(&n.sub, 30)),
            );
    }
    el
}

/// Letter-spacing stand-in (0.08em on the wave labels): thin spaces.
fn spaced(s: &str) -> String {
    s.chars()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join("\u{200a}")
}

/// Per-file +/− bars, widest change = 90px, like the replica's review card.
pub fn file_bars(changes: &[FileChange], p: &Pal, mono: &'static str) -> impl IntoElement {
    let max = changes
        .iter()
        .map(|c| c.added + c.removed)
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .font_family(mono)
        .text_size(px(11.5))
        .children(changes.iter().take(8).map(|c| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().flex_1().min_w_0().truncate().child(c.path.clone()))
                .child(
                    div()
                        .w(px(90.))
                        .flex_none()
                        .flex()
                        .h(px(6.))
                        .child(div().h_full().w(px(c.added as f32 / max * 90.)).bg(p.green))
                        .child(div().h_full().w(px(c.removed as f32 / max * 90.)).bg(p.red)),
                )
                .child(
                    div()
                        .w(px(62.))
                        .flex_none()
                        .text_color(p.fg3)
                        .child(format!("+{} −{}", c.added, c.removed)),
                )
        }))
}
