//! Split panes inside the thread card, direction B of design/cauldron-split-panes.html (Charles,
//! 13 Nov 01:49Z: three at most): six fixed shapes, stepped through by one layout button in the card
//! head, a 4px gap, a draggable line on every divider, and no pane under 200 x 120. Pure geometry,
//! like [`crate::layout`].

use crate::layout::Rect;

/// Between two panes.
pub const GAP: f32 = 4.0;
/// No pane gets smaller than this.
pub const MIN_W: f32 = 200.0;
pub const MIN_H: f32 = 120.0;
/// Panes in the card at most.
pub const MAX: usize = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    One,
    /// Two side by side.
    Cols,
    /// Two stacked.
    Rows,
    /// Three side by side ("3 vertical").
    Three,
    /// Two side by side on top, one long pane across the bottom.
    TwoOne,
    /// One tall pane, two stacked beside it.
    OneTwo,
}

/// A divider's line: the (first) vertical one, the second vertical one in Three, the horizontal one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    V,
    V2,
    H,
}

impl Shape {
    /// The layout button's order.
    pub const ALL: [Shape; 6] = [Shape::One, Shape::Cols, Shape::Rows, Shape::Three, Shape::TwoOne, Shape::OneTwo];

    pub fn count(self) -> usize {
        match self {
            Shape::One => 1,
            Shape::Cols | Shape::Rows => 2,
            Shape::Three | Shape::TwoOne | Shape::OneTwo => 3,
        }
    }

    /// What the layout button switches to (and draws): the next shape, the last back to one.
    pub fn next(self) -> Shape {
        let i = Shape::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Shape::ALL[(i + 1) % Shape::ALL.len()]
    }

    /// What closing a pane leaves: one pane fewer, or `None` with one left.
    pub fn prev(self) -> Option<Shape> {
        match self {
            Shape::One => None,
            Shape::Cols | Shape::Rows => Some(Shape::One),
            Shape::Three | Shape::TwoOne | Shape::OneTwo => Some(Shape::Cols),
        }
    }

    /// What a pane's + opens: one more pane beside, or `None` at three.
    pub fn add(self) -> Option<Shape> {
        match self {
            Shape::One => Some(Shape::Cols),
            Shape::Cols => Some(Shape::Three),
            Shape::Rows => Some(Shape::OneTwo),
            _ => None,
        }
    }

    /// What a pane's Split bottom opens: a long pane across the bottom, or `None`.
    pub fn split_down(self) -> Option<Shape> {
        match self {
            Shape::One => Some(Shape::Rows),
            Shape::Cols => Some(Shape::TwoOne),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Shape::One => "One pane",
            Shape::Cols => "Two side by side",
            Shape::Rows => "Two stacked",
            Shape::Three => "Three side by side",
            Shape::TwoOne => "Two on top, one below",
            Shape::OneTwo => "One tall, two stacked",
        }
    }

    fn min_size(self) -> (f32, f32) {
        let cols = match self {
            Shape::One | Shape::Rows => 1.0,
            Shape::Three => 3.0,
            _ => 2.0,
        };
        let rows = if matches!(self, Shape::Rows | Shape::TwoOne | Shape::OneTwo) { 2.0 } else { 1.0 };
        (cols * MIN_W + (cols - 1.0) * GAP, rows * MIN_H + (rows - 1.0) * GAP)
    }
}

/// A shape and where its lines sit, as shares of the room left after the gaps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Split {
    pub shape: Shape,
    /// The vertical line of a two-column shape: the left column's share.
    pub cx: f32,
    /// The horizontal line: the top row's share.
    pub ry: f32,
    /// Three side by side: where the first and second columns end, as shares.
    pub t1: f32,
    pub t2: f32,
}

impl Default for Split {
    fn default() -> Self {
        Split { shape: Shape::One, cx: 0.5, ry: 0.5, t1: 1.0 / 3.0, t2: 2.0 / 3.0 }
    }
}

/// `v` clamped to `lo..=hi`, `lo` winning when the room is too small for both.
fn keep(v: f32, lo: f32, hi: f32) -> f32 {
    v.min(hi).max(lo)
}

/// The sizes the shares come to in `area`: (left column, top row, Three's first and second column ends).
fn sizes(area: Rect, sp: &Split) -> (f32, f32, f32, f32) {
    let a = keep(((area.w - GAP) * sp.cx).round(), MIN_W, area.w - GAP - MIN_W);
    let b = keep(((area.h - GAP) * sp.ry).round(), MIN_H, area.h - GAP - MIN_H);
    let inner = area.w - 2.0 * GAP;
    let e1 = keep((inner * sp.t1).round(), MIN_W, inner - 2.0 * MIN_W);
    let e2 = keep((inner * sp.t2).round(), e1 + MIN_W, inner - MIN_W);
    (a, b, e1, e2)
}

impl Split {
    /// Moves `line` to the pointer at window x (vertical lines) or y (the horizontal one); the
    /// pointer sits in the middle of the gap. Only the panes on the line's two sides change.
    pub fn drag(&mut self, line: Line, area: Rect, at: f32) {
        let (_, _, e1, e2) = sizes(area, self);
        let inner = area.w - 2.0 * GAP;
        match (line, self.shape) {
            (Line::V, Shape::Three) => self.t1 = keep(at - area.x - GAP / 2.0, MIN_W, e2 - MIN_W) / inner,
            (Line::V2, _) => self.t2 = keep(at - area.x - GAP - GAP / 2.0, e1 + MIN_W, inner - MIN_W) / inner,
            (Line::V, _) => self.cx = keep(at - area.x - GAP / 2.0, MIN_W, area.w - GAP - MIN_W) / (area.w - GAP),
            (Line::H, _) => self.ry = keep(at - area.y - GAP / 2.0, MIN_H, area.h - GAP - MIN_H) / (area.h - GAP),
        }
    }

    /// A double-click on a line makes its panes even (all three columns in Three).
    pub fn reset(&mut self, line: Line) {
        match (line, self.shape) {
            (Line::H, _) => self.ry = 0.5,
            (_, Shape::Three) => {
                self.t1 = 1.0 / 3.0;
                self.t2 = 2.0 / 3.0;
            }
            _ => self.cx = 0.5,
        }
    }
}

/// Whether every pane of `shape` keeps MIN_W x MIN_H in `area`.
pub fn fits(area: Rect, shape: Shape) -> bool {
    let (w, h) = shape.min_size();
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
    let (a, b, e1, e2) = sizes(area, &sp);
    let (x, y, w, h) = (area.x, area.y, area.w, area.h);
    let (rx, rw) = (x + a + GAP, w - a - GAP);
    let (dy, dh) = (y + b + GAP, h - b - GAP);
    match sp.shape {
        Shape::One => vec![area],
        Shape::Cols => vec![Rect::new(x, y, a, h), Rect::new(rx, y, rw, h)],
        Shape::Rows => vec![Rect::new(x, y, w, b), Rect::new(x, dy, w, dh)],
        Shape::Three => vec![
            Rect::new(x, y, e1, h),
            Rect::new(x + e1 + GAP, y, e2 - e1, h),
            Rect::new(x + e2 + 2.0 * GAP, y, w - 2.0 * GAP - e2, h),
        ],
        Shape::TwoOne => vec![Rect::new(x, y, a, b), Rect::new(rx, y, rw, b), Rect::new(x, dy, w, dh)],
        Shape::OneTwo => vec![Rect::new(x, y, a, h), Rect::new(rx, y, rw, b), Rect::new(rx, dy, rw, dh)],
    }
}

/// The lines, each over its gap, in the order V, V2, H. In Two on top the vertical line spans the
/// top row; in One tall the horizontal line spans the stacked side.
pub fn lines(area: Rect, sp: Split) -> Vec<(Line, Rect)> {
    let (a, b, e1, e2) = sizes(area, &sp);
    let (x, y, w, h) = (area.x, area.y, area.w, area.h);
    let v = |at: f32, hh: f32| Rect::new(at, y, GAP, hh);
    let hz = |x0: f32, ww: f32| Rect::new(x0, y + b, ww, GAP);
    match sp.shape {
        Shape::One => vec![],
        Shape::Cols => vec![(Line::V, v(x + a, h))],
        Shape::Rows => vec![(Line::H, hz(x, w))],
        Shape::Three => vec![(Line::V, v(x + e1, h)), (Line::V2, v(x + e2 + GAP, h))],
        Shape::TwoOne => vec![(Line::V, v(x + a, b)), (Line::H, hz(x, w))],
        Shape::OneTwo => vec![(Line::V, v(x + a, h)), (Line::H, hz(x + a + GAP, w - a - GAP))],
    }
}
