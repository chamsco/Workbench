//! The shell's clock (the motion brief). The geometry and curves are pure and tested in
//! `cauldron_chrome::motion`; this only keeps time and decides when a state change starts a run.
//!
//! A run starts from the frame last drawn, so an interrupted move continues from where the
//! panes are, not from where they were going. A resize moves the target without restarting.

use cauldron_chrome::layout::ShellState;
use cauldron_chrome::motion::{self, Frame};
use std::time::Instant;

fn elapsed_ms(now: Instant, t0: Instant) -> u64 {
    now.saturating_duration_since(t0).as_millis() as u64
}

/// How far through a `dur` ms run that began at `t0` `now` is: 0 at the start, 1 once done.
pub(crate) fn run(now: Instant, t0: Instant, dur: u64) -> f32 {
    motion::progress(elapsed_ms(now, t0), 0, dur)
}

/// A position that eases from `from` to `to` on the settle curve over `dur` ms from `t0`. With no
/// run in progress it is `to`.
pub(crate) fn ease_between(from: f32, to: f32, t0: Option<Instant>, now: Instant, dur: u64) -> f32 {
    match t0 {
        Some(t0) => from + (to - from) * motion::SETTLE.at(run(now, t0, dur)),
        None => to,
    }
}

#[derive(Default)]
pub(crate) struct ShellMotion {
    /// The state the last render was asked for, to tell a change of state from a resize.
    state: Option<ShellState>,
    /// The frame drawn at the last render.
    shown: Option<Frame>,
    /// The run in progress: the frame it started from, and when.
    run: Option<(Frame, Instant)>,
}

impl ShellMotion {
    /// The frame to draw this render for `target` (the rest frame of `state`), and whether the
    /// next render needs another frame.
    pub(crate) fn frame(&mut self, state: ShellState, target: Frame, now: Instant, reduce: bool) -> (Frame, bool) {
        if self.state != Some(state) {
            self.state = Some(state);
            let from = self.shown.unwrap_or(target);
            self.run = (from != target).then_some((from, now));
        }
        if let Some((from, t0)) = self.run {
            let dur = if reduce { motion::time::REDUCED } else { motion::time::LAYOUT };
            let p = run(now, t0, dur);
            if p < 1.0 {
                let f = Frame::between(&from, &target, p, reduce);
                self.shown = Some(f);
                return (f, true);
            }
        }
        self.run = None;
        self.shown = Some(target);
        (target, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cauldron_chrome::layout::{solve, Platform};
    use std::time::Duration;

    const W: f32 = 1240.0;
    const H: f32 = 702.0;

    fn rest(state: ShellState) -> Frame {
        Frame::at_rest(&solve(W, H, state, Platform::Windows))
    }

    fn open() -> ShellState {
        ShellState { sidebar_open: true, drawer_open: false, split_open: false }
    }

    fn closed() -> ShellState {
        ShellState { sidebar_open: false, drawer_open: false, split_open: false }
    }

    #[test]
    fn the_first_render_is_at_rest_and_asks_for_no_frame() {
        let t = Instant::now();
        let mut m = ShellMotion::default();
        let (f, more) = m.frame(open(), rest(open()), t, false);
        assert_eq!(f, rest(open()));
        assert!(!more);
    }

    #[test]
    fn a_change_starts_at_the_frame_drawn_and_ends_at_the_new_rest() {
        let t = Instant::now();
        let mut m = ShellMotion::default();
        m.frame(open(), rest(open()), t, false);
        let (first, more) = m.frame(closed(), rest(closed()), t, false);
        assert!(more, "a change of state needs frames");
        assert_eq!(first, rest(open()), "the first frame of the run is the frame before it");
        let (last, more) = m.frame(closed(), rest(closed()), t + Duration::from_millis(motion::time::LAYOUT + 1), false);
        assert!(!more);
        assert_eq!(last, rest(closed()), "the last frame is the new rest");
    }

    #[test]
    fn an_interrupted_run_continues_from_the_drawn_frame() {
        let t = Instant::now();
        let mut m = ShellMotion::default();
        m.frame(open(), rest(open()), t, false);
        m.frame(closed(), rest(closed()), t, false);
        let half = t + Duration::from_millis(motion::time::LAYOUT / 2);
        let (mid, more) = m.frame(closed(), rest(closed()), half, false);
        assert!(more);
        assert_ne!(mid, rest(open()));
        assert_ne!(mid, rest(closed()));
        let (again, more) = m.frame(open(), rest(open()), half, false);
        assert!(more);
        assert_eq!(again, mid, "turning back starts from the frame on screen, not the old rest");
    }

    #[test]
    fn a_resize_moves_the_target_without_restarting_the_run() {
        let t = Instant::now();
        let mut m = ShellMotion::default();
        m.frame(open(), rest(open()), t, false);
        m.frame(closed(), rest(closed()), t, false);
        let bigger = solve(W + 100.0, H, closed(), Platform::Windows);
        let (_, more) = m.frame(closed(), Frame::at_rest(&bigger), t + Duration::from_millis(10), false);
        assert!(more, "the run goes on toward the new size");
    }

    #[test]
    fn a_slide_starts_at_from_ends_at_to_and_eases_between() {
        let t = Instant::now();
        let dur = motion::time::INDICATOR;
        assert_eq!(ease_between(2.0, 5.0, Some(t), t, dur), 2.0);
        assert_eq!(ease_between(2.0, 5.0, Some(t), t + Duration::from_millis(dur + 1), dur), 5.0);
        let mid = ease_between(2.0, 5.0, Some(t), t + Duration::from_millis(dur / 2), dur);
        assert!(mid > 2.0 && mid < 5.0, "{mid}");
        assert_eq!(ease_between(2.0, 5.0, None, t, dur), 5.0, "no run: at the target");
    }

    #[test]
    fn reduced_motion_is_quick_and_moves_nothing() {
        let t = Instant::now();
        let mut m = ShellMotion::default();
        m.frame(open(), rest(open()), t, true);
        let (_, more) = m.frame(closed(), rest(closed()), t, true);
        assert!(more);
        let (last, more) = m.frame(closed(), rest(closed()), t + Duration::from_millis(motion::time::REDUCED + 1), true);
        assert!(!more);
        assert_eq!(last, rest(closed()));
    }
}
