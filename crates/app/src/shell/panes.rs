//! Split panes inside the thread card, direction B (Charles, 13 Nov): the card head's layout
//! button steps through `cauldron_chrome::panes::Shape::ALL`, drawing the shape it would switch to
//! and morphing to the next one when clicked; two lines drag the splits. The geometry is
//! `cauldron_chrome::panes`; nothing here picks a position.

use super::*;
use cauldron_chrome::panes::{self, Shape, GAP};

/// The layout button's drawing: an 18x14 sketch of a shape.
const SKETCH_W: f32 = 18.0;
const SKETCH_H: f32 = 14.0;
/// A line's grab area across the 4px gap.
const GRAB: f32 = 12.0;
/// The tiles a new pane offers until those apps exist (the old half's list).
const TILES: [&str; 6] = ["Highlights", "Files", "Changes", "Terminal", "Browser", "Add an app"];

/// A shape's panes at sketch size, even splits, each inset 1px so the gaps show. Always four:
/// a shape with fewer repeats its last pane, so a morph between any two shapes has four to move.
fn sketch(shape: Shape) -> [Rect; 4] {
    let big = Rect::new(0.0, 0.0, 1000.0, 1000.0 * SKETCH_H / SKETCH_W);
    let rs = panes::rects(big, panes::Split { shape, ..Default::default() });
    let k = SKETCH_W / big.w;
    let at = |i: usize| {
        let r = rs[i.min(rs.len() - 1)];
        Rect::new(r.x * k + 0.75, r.y * k + 0.75, r.w * k - 1.5, r.h * k - 1.5)
    };
    [at(0), at(1), at(2), at(3)]
}

impl Cauldron {
    /// The layout button for the card head: it draws the shape a click switches to, and morphs
    /// from the last one over 200ms (none with reduced motion).
    pub(super) fn layout_button(&self, pal: &'static Palette, now: std::time::Instant, reduce: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let to = self.panes.shape.next();
        let k = match self.layout_since {
            Some((_, t0)) if !reduce => mt_ease(crate::motion::run(now, t0, cauldron_chrome::motion::time::LAYOUT)),
            _ => 1.0,
        };
        let from = self.layout_since.map_or(to, |(f, _)| f);
        let (a, b) = (sketch(from), sketch(to));
        let mut art = div().relative().w(px(SKETCH_W)).h(px(SKETCH_H)).flex_none();
        for i in 0..4 {
            let r = cauldron_chrome::motion::lerp_rect(a[i], b[i], k);
            art = art.child(abs(r).rounded(px(2.)).border_1().border_color(c(pal.fg2)));
        }
        let label: SharedString = format!("Layout: {}", to.label()).into();
        let el = div()
            .id("layout")
            .w(px(28.))
            .h(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::CONTROL))
            .cursor_pointer()
            .hover(|d| d.bg(c(pal.hover)))
            .aria_label(label.clone())
            .child(art)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_shape(to);
                cx.notify();
            }));
        self.tipped("layout-tip", el, label, Side::Below, pal, cx)
    }

    /// Switches the card to `shape`, starting the layout button's morph from what it drew.
    pub(super) fn set_shape(&mut self, shape: Shape) {
        self.layout_since = Some((self.panes.shape.next(), std::time::Instant::now()));
        self.panes.shape = shape;
    }

    /// The card's inside, split: pane 0 holds `body` (the thread or the new thread), the others
    /// offer what to open, and the two lines sit over the gaps. `area` is in window coordinates.
    pub(super) fn pane_stage(&mut self, pal: &'static Palette, area: Rect, shape: Shape, body: AnyElement, cx: &mut Context<Self>) -> Stateful<Div> {
        let sp = panes::Split { shape, ..self.panes };
        let local = |r: Rect| Rect::new(r.x - area.x, r.y - area.y, r.w, r.h);
        let ring = c(pal.line);
        let fg3 = c(pal.fg3);
        let mut stage = div().id("pane-stage").relative().size_full();
        let mut body = Some(body);
        for (i, r) in panes::rects(area, sp).into_iter().enumerate() {
            let mut p = abs(local(r))
                .id(("pane", i))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(radius::INSET))
                .bg(c(pal.panel))
                .border_1()
                .border_color(ring);
            if i == 0 {
                p = p.child(body.take().unwrap());
            } else {
                let close = div()
                    .id(("pane-close", i))
                    .w(px(24.))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::INNER))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label("Close pane")
                    .child(glyph("x", 13., fg3))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(p) = this.panes.shape.prev() {
                            this.set_shape(p);
                        }
                        cx.notify();
                    }));
                let mut tiles = div().flex().flex_wrap().gap(px(8.)).p(px(12.));
                for t in TILES {
                    tiles = tiles.child(div().px(px(10.)).py(px(8.)).rounded(px(radius::CONTROL)).bg(c(pal.inset)).border_1().border_color(ring).text_size(px(12.)).text_color(c(pal.fg2)).child(t));
                }
                p = p
                    .child(
                        div()
                            .h(px(32.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .pl(px(12.))
                            .pr(px(4.))
                            .border_b_1()
                            .border_color(ring)
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(pal.fg2))
                            .child(div().flex_1().child("Open in this pane"))
                            .child(close.test_support()),
                    )
                    .child(tiles);
            }
            stage = stage.child(p.test_support());
        }
        let (v, h) = panes::lines(area, sp);
        for (vertical, line) in [(true, v), (false, h)] {
            let Some(l) = line else { continue };
            let l = local(l);
            let hit = if vertical { Rect::new(l.x + GAP / 2.0 - GRAB / 2.0, l.y, GRAB, l.h) } else { Rect::new(l.x, l.y + GAP / 2.0 - GRAB / 2.0, l.w, GRAB) };
            let id = if vertical { "line-v" } else { "line-h" };
            let dragging = self.pane_drag.is_some_and(|(v, _)| v == vertical);
            let accent = c(pal.accent);
            let mark = if vertical { div().w(px(2.)).h(px(28.)) } else { div().h(px(2.)).w(px(28.)) };
            stage = stage.child(
                abs(hit)
                    .id(id)
                    .flex()
                    .items_center()
                    .justify_center()
                    .group(id)
                    .when_live(vertical, |d| d.cursor_col_resize())
                    .when_live(!vertical, |d| d.cursor_row_resize())
                    .aria_label(if vertical { "Resize panes left and right" } else { "Resize panes above and below" })
                    .child(mark.rounded(px(1.)).when_live(dragging, |d| d.bg(accent)).group_hover(id, |d| d.bg(accent)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            if ev.click_count >= 2 {
                                if vertical { this.panes.reset_v() } else { this.panes.reset_h() }
                                this.pane_drag = None;
                            } else {
                                this.pane_drag = Some((vertical, area));
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .test_support(),
            );
        }
        stage
    }

    /// While a line is held, the pointer moves it (on the root, so it keeps up outside the line).
    pub(super) fn drag_line(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some((vertical, area)) = self.pane_drag else { return };
        if ev.pressed_button != Some(MouseButton::Left) {
            self.pane_drag = None;
        } else if vertical {
            self.panes.drag_v(area, f32::from(ev.position.x));
        } else {
            self.panes.drag_h(area, f32::from(ev.position.y));
        }
        cx.notify();
    }
}

fn mt_ease(p: f32) -> f32 {
    cauldron_chrome::motion::SETTLE.at(p)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{rect, W, H};
    use super::super::Cauldron;
    use cauldron_chrome::panes::Shape;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{point, px, size, AppContext, TestAppContext, Window, WindowHandle};

    fn open(cx: &mut TestAppContext) -> WindowHandle<Cauldron> {
        cx.update(gpui_kit::init);
        cx.update(|cx| cx.set_reduce_motion(true));
        let handle = cx.add_window(|_, _| Cauldron { drawer: None, ..Cauldron::new() });
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        handle
    }

    fn count(window: &Window) -> usize {
        (0..6usize).filter(|&i| window.try_find(("pane", i)).is_some()).count()
    }

    /// The card head's layout button names the shape it switches to, and each click steps to the
    /// next of the six, the grid back to one pane: never more than four panes.
    #[gpui_kit::test]
    fn the_layout_button_steps_through_six_shapes(cx: &mut TestAppContext) {
        let handle = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let head = rect(window, "card-head");
            let b = rect(window, "layout");
            assert!(b.x >= head.x && b.right() <= head.right() && b.y >= head.y && b.bottom() <= head.bottom(), "the button sits in the card head");
            assert_eq!(count(window), 0, "one pane: the card as it was, no pane frames");
            let mut seen = Vec::new();
            for _ in 0..6 {
                let label = window.find("layout").label().unwrap().to_string();
                window.click("layout", cx);
                seen.push((label, count(window)));
            }
            let want = [
                ("Layout: Two side by side", 2),
                ("Layout: Two stacked", 2),
                ("Layout: One beside two", 3),
                ("Layout: Two on top, one below", 3),
                ("Layout: Grid of four", 4),
                ("Layout: One pane", 0),
            ];
            assert_eq!(seen, want.map(|(l, n)| (l.to_string(), n)));
        })
        .unwrap();
    }

    /// Panes sit 4px apart; closing one returns to the shape before.
    #[gpui_kit::test]
    fn panes_are_4px_apart_and_close_steps_back(cx: &mut TestAppContext) {
        let handle = open(cx);
        handle.update(cx, |c, _, _| c.panes.shape = Shape::Grid).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let (a, b, d) = (rect(window, ("pane", 0usize)), rect(window, ("pane", 1usize)), rect(window, ("pane", 2usize)));
            assert_eq!(b.x - a.right(), 4.0, "4px between columns");
            assert_eq!(d.y - a.bottom(), 4.0, "4px between rows");
            for i in 0..4usize {
                let r = rect(window, ("pane", i));
                assert!(r.w >= 200.0 && r.h >= 120.0, "pane {i} {r:?} keeps 200 x 120");
            }
            window.click(("pane-close", 3usize), cx);
            assert_eq!(count(window), 3, "closing a pane of the grid leaves one beside two");
            assert_eq!(window.find("layout").label(), Some("Layout: Two on top, one below"));
        })
        .unwrap();
    }

    /// Dragging the vertical line moves the split with the pointer and stops at 200px; a
    /// double-click makes it even.
    #[gpui_kit::test]
    fn the_lines_drag_the_split(cx: &mut TestAppContext) {
        let handle = open(cx);
        handle.update(cx, |c, _, _| c.panes.shape = Shape::TwoOne).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let v = rect(window, "line-v");
            let (mx, my) = (v.x + v.w / 2.0, v.y + v.h / 2.0);
            window.drag(point(px(mx), px(my)), point(px(mx - 100.0), px(my)), cx);
            window.render_frame(cx);
            let (a, b) = (rect(window, ("pane", 0usize)), rect(window, ("pane", 1usize)));
            assert!((a.right() + 2.0 - (mx - 100.0)).abs() <= 1.0, "the line follows the pointer: {a:?}");
            assert_eq!(b.x - a.right(), 4.0);
            let v = rect(window, "line-v");
            let (mx, my) = (v.x + v.w / 2.0, v.y + v.h / 2.0);
            window.drag(point(px(mx), px(my)), point(px(0.0), px(my)), cx);
            window.render_frame(cx);
            assert_eq!(rect(window, ("pane", 0usize)).w, 200.0, "no narrower than 200");
            let h = rect(window, "line-h");
            let bottom = rect(window, ("pane", 2usize));
            assert_eq!((h.x, h.w), (bottom.x, bottom.w), "the horizontal line spans the long bottom pane");
            window.double_click("line-v", cx);
            window.render_frame(cx);
            let (a, b) = (rect(window, ("pane", 0usize)), rect(window, ("pane", 1usize)));
            assert!((a.w - b.w).abs() <= 1.0, "even again: {a:?} {b:?}");
        })
        .unwrap();
    }
}
