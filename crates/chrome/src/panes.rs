//! Split panes inside the thread card, direction B of design/cauldron-split-panes.html (Charles,
//! 13 Nov): six fixed shapes, stepped through by one layout button in the card head, a 4px gap, one
//! vertical and one horizontal line to drag, no pane under 200 x 120 and four panes at most. Pure
//! geometry, like [`crate::layout`].

use crate::layout::Rect;

/// Between two panes.
pub const GAP: f32 = 4.0;
/// No pane gets smaller than this.
pub const MIN_W: f32 = 200.0;
pub const MIN_H: f32 = 120.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    One,
    /// Two side by side.
    Cols,
    /// Two stacked.
    Rows,
    /// One on the left, two stacked on the right.
    OneTwo,
    /// Two side by side on top, one long pane across the bottom (Charles, 13 Nov).
    TwoOne,
    /// Two by two.
    Grid,
}

impl Shape {
    /// The layout button's order.
    pub const ALL: [Shape; 6] = [Shape::One, Shape::Cols, Shape::Rows, Shape::OneTwo, Shape::TwoOne, Shape::Grid];

    pub fn count(self) -> usize {
        match self {
            Shape::One => 1,
            Shape::Cols | Shape::Rows => 2,
            Shape::OneTwo | Shape::TwoOne => 3,
            Shape::Grid => 4,
        }
    }

    /// What the layout button switches to (and draws): the next shape, the grid back to one.
    pub fn next(self) -> Shape {
        let i = Shape::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Shape::ALL[(i + 1) % Shape::ALL.len()]
    }

    /// What closing a pane leaves: one pane fewer, or `None` with one left.
    pub fn prev(self) -> Option<Shape> {
        match self {
            Shape::One => None,
            Shape::Cols | Shape::Rows => Some(Shape::One),
            Shape::OneTwo | Shape::TwoOne => Some(Shape::Cols),
            Shape::Grid => Some(Shape::OneTwo),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Shape::One => "One pane",
            Shape::Cols => "Two side by side",
            Shape::Rows => "Two stacked",
            Shape::OneTwo => "One beside two",
            Shape::TwoOne => "Two on top, one below",
            Shape::Grid => "Grid of four",
        }
    }

    fn has_v(self) -> bool {
        matches!(self, Shape::Cols | Shape::OneTwo | Shape::TwoOne | Shape::Grid)
    }

    fn has_h(self) -> bool {
        matches!(self, Shape::Rows | Shape::OneTwo | Shape::TwoOne | Shape::Grid)
    }
}

/// A shape and where its two lines sit, as shares of the room left after the gap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Split {
    pub shape: Shape,
    /// The vertical line: the left column's share of the width.
    pub cx: f32,
    /// The horizontal line: the top row's share of the height.
    pub ry: f32,
}

impl Default for Split {
    fn default() -> Self {
        Split { shape: Shape::One, cx: 0.5, ry: 0.5 }
    }
}

/// `v` clamped to `lo..=hi`, `lo` winning when the room is too small for both.
fn keep(v: f32, lo: f32, hi: f32) -> f32 {
    v.min(hi).max(lo)
}

fn left_w(area: Rect, cx: f32) -> f32 {
    keep(((area.w - GAP) * cx).round(), MIN_W, area.w - GAP - MIN_W)
}

fn top_h(area: Rect, ry: f32) -> f32 {
    keep(((area.h - GAP) * ry).round(), MIN_H, area.h - GAP - MIN_H)
}

impl Split {
    /// Moves the vertical line to window x `x` (the pointer sits in the middle of the gap).
    pub fn drag_v(&mut self, area: Rect, x: f32) {
        self.cx = keep(x - area.x - GAP / 2.0, MIN_W, area.w - GAP - MIN_W) / (area.w - GAP);
    }

    /// Moves the horizontal line to window y `y`.
    pub fn drag_h(&mut self, area: Rect, y: f32) {
        self.ry = keep(y - area.y - GAP / 2.0, MIN_H, area.h - GAP - MIN_H) / (area.h - GAP);
    }

    /// A double-click on the vertical line makes the columns even.
    pub fn reset_v(&mut self) {
        self.cx = 0.5;
    }

    pub fn reset_h(&mut self) {
        self.ry = 0.5;
    }
}

/// Whether every pane of `shape` keeps MIN_W x MIN_H in `area`.
pub fn fits(area: Rect, shape: Shape) -> bool {
    let w = if shape.has_v() { 2.0 * MIN_W + GAP } else { MIN_W };
    let h = if shape.has_h() { 2.0 * MIN_H + GAP } else { MIN_H };
    area.w >= w && area.h >= h
}

/// The shape drawn in `area`: the one asked for, or the nearest earlier one that fits, as the
/// shell closes Split before the drawer when the window narrows.
pub fn effective(area: Rect, mut shape: Shape) -> Shape {
    while !fits(area, shape) {
        match shape.prev() {
            Some(p) => shape = p,
            None => break,
        }
    }
    shape
}

/// The panes in reading order: left before right, top before bottom.
pub fn rects(area: Rect, sp: Split) -> Vec<Rect> {
    let a = left_w(area, sp.cx);
    let b = top_h(area, sp.ry);
    let (lx, rx, rw) = (area.x, area.x + a + GAP, area.w - a - GAP);
    let (ty, dy, dh) = (area.y, area.y + b + GAP, area.h - b - GAP);
    match sp.shape {
        Shape::One => vec![area],
        Shape::Cols => vec![Rect::new(lx, ty, a, area.h), Rect::new(rx, ty, rw, area.h)],
        Shape::Rows => vec![Rect::new(lx, ty, area.w, b), Rect::new(lx, dy, area.w, dh)],
        Shape::OneTwo => vec![Rect::new(lx, ty, a, area.h), Rect::new(rx, ty, rw, b), Rect::new(rx, dy, rw, dh)],
        Shape::TwoOne => vec![Rect::new(lx, ty, a, b), Rect::new(rx, ty, rw, b), Rect::new(lx, dy, area.w, dh)],
        Shape::Grid => vec![Rect::new(lx, ty, a, b), Rect::new(rx, ty, rw, b), Rect::new(lx, dy, a, dh), Rect::new(rx, dy, rw, dh)],
    }
}

/// The two lines, each over its gap: (vertical, horizontal). In One beside two the horizontal
/// line spans the right column only; in Two on top the vertical line spans the top row only.
pub fn lines(area: Rect, sp: Split) -> (Option<Rect>, Option<Rect>) {
    let a = left_w(area, sp.cx);
    let b = top_h(area, sp.ry);
    let v = sp.shape.has_v().then(|| Rect::new(area.x + a, area.y, GAP, if sp.shape == Shape::TwoOne { b } else { area.h }));
    let h = sp.shape.has_h().then(|| {
        let (x, w) = if sp.shape == Shape::OneTwo { (area.x + a + GAP, area.w - a - GAP) } else { (area.x, area.w) };
        Rect::new(x, area.y + b, w, GAP)
    });
    (v, h)
}
