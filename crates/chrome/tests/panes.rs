//! Split panes, direction B (design/cauldron-split-panes.html; Charles, 13 Nov 01:49Z: "3 MAX and
//! make each pane draggable (either 3 vertical or 2 side by side and 1 vertical, or 2 stacked on
//! one side and 1 tall pane)"): six shapes inside the thread card, one layout button that steps
//! through them, a 4px gap, a draggable line on every divider, no pane under 200 x 120, three at most.

use cauldron_chrome::layout::Rect;
use cauldron_chrome::panes::{self, Line, Shape, Split, GAP, MAX, MIN_H, MIN_W};

const AREA: Rect = Rect::new(100.0, 50.0, 900.0, 600.0);

fn split(shape: Shape) -> Split {
    Split { shape, ..Split::default() }
}

#[test]
fn six_shapes_hold_one_to_three_panes() {
    assert_eq!(MAX, 3);
    let n: Vec<usize> = Shape::ALL.iter().map(|s| s.count()).collect();
    assert_eq!(n, [1, 2, 2, 3, 3, 3]);
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
    assert_eq!(seen, [Shape::One, Shape::Cols, Shape::Rows, Shape::Three, Shape::TwoOne, Shape::OneTwo, Shape::One]);
}

#[test]
fn closing_a_pane_returns_to_the_previous_shape() {
    assert_eq!(Shape::Three.prev(), Some(Shape::Cols));
    assert_eq!(Shape::TwoOne.prev(), Some(Shape::Cols));
    assert_eq!(Shape::OneTwo.prev(), Some(Shape::Cols));
    assert_eq!(Shape::Cols.prev(), Some(Shape::One));
    assert_eq!(Shape::Rows.prev(), Some(Shape::One));
    assert_eq!(Shape::One.prev(), None);
}

#[test]
fn adding_a_pane_splits_without_passing_three() {
    assert_eq!(Shape::One.add(), Some(Shape::Cols));
    assert_eq!(Shape::Cols.add(), Some(Shape::Three));
    assert_eq!(Shape::Rows.add(), Some(Shape::OneTwo));
    for s in [Shape::Three, Shape::TwoOne, Shape::OneTwo] {
        assert_eq!(s.add(), None, "{s:?}: three is the most");
    }
    assert_eq!(Shape::One.split_down(), Some(Shape::Rows));
    assert_eq!(Shape::Cols.split_down(), Some(Shape::TwoOne));
}

fn gaps_are_4(rs: &[Rect]) {
    for (i, a) in rs.iter().enumerate() {
        for b in &rs[i + 1..] {
            let overlap_y = a.y < b.bottom() && b.y < a.bottom();
            let overlap_x = a.x < b.right() && b.x < a.right();
            assert!(!(overlap_x && overlap_y), "{a:?} overlaps {b:?}");
            if overlap_y && a.right() <= b.x && rs.iter().all(|c| !(c.x >= a.right() && c.right() <= b.x)) {
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
        let gaps: f32 = panes::lines(AREA, split(s)).iter().map(|(_, r)| r.w * r.h).sum();
        assert!((area + gaps - AREA.w * AREA.h).abs() < 1.0, "{s:?} fills the area with its panes and gaps");
        for r in &rs {
            assert!(r.x >= AREA.x && r.right() <= AREA.right() && r.y >= AREA.y && r.bottom() <= AREA.bottom(), "{s:?} {r:?} inside");
        }
    }
}

#[test]
fn every_divider_has_a_line() {
    let kinds = |s: Shape| panes::lines(AREA, split(s)).into_iter().map(|(l, _)| l).collect::<Vec<_>>();
    assert_eq!(kinds(Shape::One), []);
    assert_eq!(kinds(Shape::Cols), [Line::V]);
    assert_eq!(kinds(Shape::Rows), [Line::H]);
    assert_eq!(kinds(Shape::Three), [Line::V, Line::V2]);
    assert_eq!(kinds(Shape::TwoOne), [Line::V, Line::H]);
    assert_eq!(kinds(Shape::OneTwo), [Line::V, Line::H]);
}

#[test]
fn dragging_any_line_keeps_every_pane_at_200_by_120() {
    for s in Shape::ALL {
        for x in [-500.0, AREA.x, AREA.x + 250.0, AREA.x + 450.0, AREA.right() - 10.0, 5000.0] {
            for y in [-500.0, AREA.y + 10.0, AREA.y + 300.0, AREA.bottom(), 5000.0] {
                let mut sp = split(s);
                for (l, _) in panes::lines(AREA, sp) {
                    sp.drag(l, AREA, if l == Line::H { y } else { x });
                }
                for r in panes::rects(AREA, sp) {
                    assert!(r.w >= MIN_W && r.h >= MIN_H, "{s:?} at ({x},{y}): {r:?}");
                }
                gaps_are_4(&panes::rects(AREA, sp));
            }
        }
    }
    let mut sp = split(Shape::Cols);
    sp.drag(Line::V, AREA, AREA.x + 300.0);
    assert_eq!(panes::rects(AREA, sp)[0].right() + GAP / 2.0, AREA.x + 300.0, "the line follows the pointer");
}

#[test]
fn a_line_resizes_only_the_panes_on_its_two_sides() {
    let mut sp = split(Shape::Three);
    let before = panes::rects(AREA, sp);
    sp.drag(Line::V2, AREA, before[1].right() + GAP / 2.0 + 60.0);
    let after = panes::rects(AREA, sp);
    assert_eq!(after[0], before[0], "the first column stays");
    assert_eq!(after[1].w, before[1].w + 60.0);
    assert_eq!(after[2].w, before[2].w - 60.0);
    sp.drag(Line::V, AREA, before[0].right() + GAP / 2.0 - 40.0);
    let moved = panes::rects(AREA, sp);
    assert_eq!(moved[2], after[2], "the last column stays when the first line moves");
    assert_eq!(moved[1].right(), after[1].right());
}

#[test]
fn the_lines_sit_in_the_gaps() {
    for s in Shape::ALL {
        let sp = split(s);
        let rs = panes::rects(AREA, sp);
        for (l, r) in panes::lines(AREA, sp) {
            let touching = rs.iter().filter(|p| {
                if l == Line::H { (p.bottom() == r.y || p.y == r.bottom()) && p.x < r.right() && r.x < p.right() } else { (p.right() == r.x || p.x == r.right()) && p.y < r.bottom() && r.y < p.bottom() }
            });
            assert!(touching.count() >= 2, "{s:?} {l:?} {r:?} sits between panes");
        }
    }
    let (rs, ls) = (panes::rects(AREA, split(Shape::TwoOne)), panes::lines(AREA, split(Shape::TwoOne)));
    assert_eq!((rs[2].x, rs[2].w), (AREA.x, AREA.w), "two on top, one long pane across the bottom");
    assert_eq!(ls[0].1.h, rs[0].h, "its vertical line spans the top row only");
    let (rs, ls) = (panes::rects(AREA, split(Shape::OneTwo)), panes::lines(AREA, split(Shape::OneTwo)));
    assert_eq!(rs[0].h, AREA.h, "one tall pane");
    assert_eq!((ls[1].1.x, ls[1].1.w), (rs[1].x, rs[1].w), "the stacked side's line spans that side only");
}

#[test]
fn a_reset_makes_the_line_even() {
    let mut sp = split(Shape::Three);
    sp.drag(Line::V, AREA, AREA.x + 250.0);
    sp.reset(Line::V);
    sp.reset(Line::V2);
    let rs = panes::rects(AREA, sp);
    assert!((rs[0].w - rs[1].w).abs() <= 1.0 && (rs[1].w - rs[2].w).abs() <= 1.0, "{rs:?}");
}

#[test]
fn a_shape_that_does_not_fit_steps_back() {
    let narrow = Rect::new(0.0, 0.0, 500.0, 600.0);
    assert!(panes::fits(narrow, Shape::Cols) && !panes::fits(narrow, Shape::Three));
    assert_eq!(panes::effective(narrow, Shape::Three), Shape::Cols);
    let thin = Rect::new(0.0, 0.0, 300.0, 600.0);
    assert_eq!(panes::effective(thin, Shape::OneTwo), Shape::One);
    let short = Rect::new(0.0, 0.0, 900.0, 200.0);
    assert_eq!(panes::effective(short, Shape::TwoOne), Shape::Cols);
    assert_eq!(panes::effective(AREA, Shape::Three), Shape::Three);
}
