use cauldron_chrome::status::*;

#[test]
fn every_mark_draws_in_range_and_never_blank() {
    for m in MARKS {
        for f in [None, Some(0), Some(7), Some(19)] {
            let g = grid(m, f);
            assert!(g.iter().all(|v| (0.0..=1.0).contains(v)), "{m:?} {f:?}");
            assert!(g.iter().any(|v| *v > 0.2), "{m:?} {f:?} is blank");
        }
    }
}

#[test]
fn the_six_still_marks_are_all_different() {
    for (i, a) in MARKS.iter().enumerate() {
        for b in &MARKS[i + 1..] {
            assert_ne!(grid(*a, None), grid(*b, None), "{a:?} vs {b:?}");
        }
    }
}

#[test]
fn working_and_thinking_move_and_asking_keeps_its_glyph() {
    assert_ne!(grid(Mark::Working, Some(0)), grid(Mark::Working, Some(2)));
    assert_ne!(grid(Mark::Thinking, Some(0)), grid(Mark::Thinking, Some(2)));
    // Asking pulses in strength but the lit cells never change, so it stays a question mark.
    let lit = |f| grid(Mark::Asking, Some(f)).map(|v| v > 0.0);
    assert_eq!(lit(0), lit(5));
}
