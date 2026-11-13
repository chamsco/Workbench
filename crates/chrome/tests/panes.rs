//! Split panes, direction B (design/cauldron-split-panes.html; Charles, 13 Nov: "go for B", plus
//! two side by side on top with a long pane below): six shapes inside the thread card, one layout
//! button that steps through them, a 4px gap, one vertical and one horizontal line, no pane under
//! 200 x 120, four at most.

use cauldron_chrome::layout::Rect;
use cauldron_chrome::panes::{self, Shape, Split, GAP, MIN_H, MIN_W};

const AREA: Rect = Rect::new(100.0, 50.0, 900.0, 600.0);

fn split(shape: Shape) -> Split {
    Split { shape, ..Split::default() }
}

#[test]
fn six_shapes_hold_one_to_four_panes() {
    let n: Vec<usize> = Shape::ALL.iter().map(|s| s.count()).collect();
    assert_eq!(n, [1, 2, 2, 3, 3, 4]);
    for s in Shape::ALL {
        assert_eq!(panes::rects(AREA, split(s)).len(), s.count(), "{s:?}");
    }
}

#[test]
fn the_layout_button_steps_through_every_shape_and_wraps() {
    let mut s = Shape::One;
    let mut seen = vec![s];
    for _ in 0..6 {
        s = s.next();
        seen.push(s);
    }
    assert_eq!(seen, [Shape::One, Shape::Cols, Shape::Rows, Shape::OneTwo, Shape::TwoOne, Shape::Grid, Shape::One]);
    assert!(Shape::ALL.iter().all(|s| s.count() <= 4), "four panes is the most");
}

#[test]
fn closing_a_pane_returns_to_the_previous_shape() {
    assert_eq!(Shape::Grid.prev(), Some(Shape::OneTwo));
    assert_eq!(Shape::TwoOne.prev(), Some(Shape::Cols));
    assert_eq!(Shape::OneTwo.prev(), Some(Shape::Cols));
    assert_eq!(Shape::Cols.prev(), Some(Shape::One));
    assert_eq!(Shape::Rows.prev(), Some(Shape::One));
    assert_eq!(Shape::One.prev(), None);
    for s in Shape::ALL {
        if let Some(p) = s.prev() {
            assert_eq!(p.count() + 1, s.count(), "{s:?} closes one pane");
        }
    }
}

fn gaps_are_4(rs: &[Rect]) {
    for (i, a) in rs.iter().enumerate() {
        for b in &rs[i + 1..] {
            let overlap_y = a.y < b.bottom() && b.y < a.bottom();
            let overlap_x = a.x < b.right() && b.x < a.right();
            assert!(!(overlap_x && overlap_y), "{a:?} overlaps {b:?}");
            if overlap_y && a.right() <= b.x {
                assert_eq!(b.x - a.right(), GAP, "side by side: {a:?} {b:?}");
            }
            if overlap_x && a.bottom() <= b.y {
                assert_eq!(b.y - a.bottom(), GAP, "stacked: {a:?} {b:?}");
            }
        }
    }
}

#[test]
fn panes_fill_the_area_with_4px_gaps() {
    assert_eq!(GAP, 4.0);
    for s in Shape::ALL {
        let rs = panes::rects(AREA, split(s));
        gaps_are_4(&rs);
        let area: f32 = rs.iter().map(|r| r.w * r.h).sum();
        let gaps_w = match s {
            Shape::Cols | Shape::OneTwo | Shape::Grid => GAP * AREA.h,
            Shape::TwoOne => GAP * rs[0].h,
            _ => 0.0,
        };
        let gaps_h = match s {
            Shape::Rows | Shape::TwoOne => GAP * AREA.w,
            Shape::OneTwo => GAP * (AREA.w - rs[0].w - GAP),
            Shape::Grid => GAP * (AREA.w - GAP),
            _ => 0.0,
        };
        assert!((area + gaps_w + gaps_h - AREA.w * AREA.h).abs() < 1.0, "{s:?} fills the area");
        for r in &rs {
            assert!(r.x >= AREA.x && r.right() <= AREA.right() && r.y >= AREA.y && r.bottom() <= AREA.bottom(), "{s:?} {r:?} inside");
        }
    }
}

#[test]
fn dragging_a_line_keeps_every_pane_at_200_by_120() {
    for s in Shape::ALL {
        for x in [-500.0, AREA.x, AREA.x + 50.0, AREA.x + 450.0, AREA.right() - 10.0, 5000.0] {
            for y in [-500.0, AREA.y + 10.0, AREA.y + 300.0, AREA.bottom(), 5000.0] {
                let mut sp = split(s);
                sp.drag_v(AREA, x);
                sp.drag_h(AREA, y);
                for r in panes::rects(AREA, sp) {
                    assert!(r.w >= MIN_W && r.h >= MIN_H, "{s:?} at ({x},{y}): {r:?}");
                }
            }
        }
    }
    let mut sp = split(Shape::Cols);
    sp.drag_v(AREA, AREA.x + 300.0);
    let rs = panes::rects(AREA, sp);
    assert_eq!(rs[0].right() + GAP / 2.0, AREA.x + 300.0, "the line follows the pointer");
}

#[test]
fn the_lines_sit_in_the_gaps_and_line_up_in_the_grid() {
    let mut sp = split(Shape::Grid);
    sp.drag_v(AREA, AREA.x + 333.0);
    sp.drag_h(AREA, AREA.y + 222.0);
    let rs = panes::rects(AREA, sp);
    let (v, h) = panes::lines(AREA, sp);
    let (v, h) = (v.unwrap(), h.unwrap());
    assert_eq!((v.x, v.w, v.y, v.h), (rs[0].right(), GAP, AREA.y, AREA.h), "vertical line: the gap, full height");
    assert_eq!((h.y, h.h, h.x, h.w), (rs[0].bottom(), GAP, AREA.x, AREA.w), "horizontal line: the gap, full width");
    assert_eq!((rs[0].right(), rs[0].bottom()), (rs[2].right(), rs[1].bottom()), "the grid's panes line up");
    let one_two = split(Shape::OneTwo);
    let rs = panes::rects(AREA, one_two);
    let (_, h) = panes::lines(AREA, one_two);
    assert_eq!((h.unwrap().x, h.unwrap().w), (rs[1].x, rs[1].w), "one beside two: the line spans the right column only");
    let two_one = split(Shape::TwoOne);
    let rs = panes::rects(AREA, two_one);
    let (v, h) = panes::lines(AREA, two_one);
    assert_eq!((rs[2].x, rs[2].w), (AREA.x, AREA.w), "two on top, one long pane across the bottom");
    assert_eq!((v.unwrap().y, v.unwrap().h), (AREA.y, rs[0].h), "two on top: the vertical line spans the top row only");
    assert_eq!((h.unwrap().x, h.unwrap().w), (AREA.x, AREA.w));
    assert_eq!(panes::lines(AREA, split(Shape::One)), (None, None));
    assert!(panes::lines(AREA, split(Shape::Cols)).1.is_none() && panes::lines(AREA, split(Shape::Rows)).0.is_none());
}

#[test]
fn a_reset_makes_the_line_even() {
    let mut sp = split(Shape::Cols);
    sp.drag_v(AREA, AREA.x + 250.0);
    sp.reset_v();
    let rs = panes::rects(AREA, sp);
    assert_eq!(rs[0].w, rs[1].w);
}

#[test]
fn a_shape_that_does_not_fit_steps_back() {
    // 300 wide: two side by side need 404.
    let narrow = Rect::new(0.0, 0.0, 300.0, 600.0);
    assert!(!panes::fits(narrow, Shape::Cols) && panes::fits(narrow, Shape::Rows));
    assert_eq!(panes::effective(narrow, Shape::Grid), Shape::One);
    assert_eq!(panes::effective(narrow, Shape::Rows), Shape::Rows);
    let short = Rect::new(0.0, 0.0, 900.0, 200.0);
    assert_eq!(panes::effective(short, Shape::Grid), Shape::Cols);
    assert_eq!(panes::effective(AREA, Shape::Grid), Shape::Grid);
}
