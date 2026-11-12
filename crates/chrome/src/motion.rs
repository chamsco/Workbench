//! Motion rules for the shell: the timing tokens, the settle curve, and how one drawn frame
//! becomes the next (docs/cauldron/MOTION.md). Pure geometry and arithmetic, no GPUI, so the
//! first and last frame of every move are tested on any machine.
//!
//! The wireframe's own values are the starting point: `cubic-bezier(.23,1,.32,1)` for layout,
//! 120 to 200ms for fades. Desktop only for now; phone timings wait for a phone layout.

use crate::layout::{Rect, Shell};

/// The curve for every open and close: a fast start and a long settle.
pub const SETTLE: Bezier = Bezier::new(0.32, 0.72, 0.0, 1.0);

/// Durations in milliseconds.
pub mod time {
    /// A pane moves, opens or closes (sidebar, drawer, split).
    pub const LAYOUT: u64 = 300;
    /// The open panel's content changes (drawer panel switch).
    pub const PANEL: u64 = 240;
    /// The selected function icon's indicator travels.
    pub const INDICATOR: u64 = 220;
    /// A menu grows in or out.
    pub const MENU: u64 = 160;
    /// Each menu item starts this much after the one above.
    pub const STAGGER: u64 = 25;
    /// The sent text lands in its bubble.
    pub const CARRY: u64 = 300;
    /// Send and Stop swap.
    pub const SWAP: u64 = 120;
    /// The cross-fade used when the system asks for less motion.
    pub const REDUCED: u64 = 150;
}

/// The drawer's slide-in distance, from the wireframe's `.drawer .dc` translate.
pub const DRAWER_SLIDE: f32 = 12.0;

/// A cubic Bezier easing with its endpoints fixed at (0,0) and (1,1), as CSS defines them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bezier {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl Bezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Bezier { x1, y1, x2, y2 }
    }

    fn sample(a: f32, b: f32, s: f32) -> f32 {
        let c = 3.0 * a;
        let bb = 3.0 * (b - a) - c;
        let aa = 1.0 - c - bb;
        ((aa * s + bb) * s + c) * s
    }

    /// The eased value at linear progress `t`. The x curve is monotone, so bisection finds the
    /// parameter for `t` reliably in a few dozen steps.
    pub fn at(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        let (mut lo, mut hi, mut s) = (0.0f32, 1.0f32, t);
        for _ in 0..40 {
            let x = Self::sample(self.x1, self.x2, s);
            if (x - t).abs() < 1e-6 {
                break;
            }
            if x < t {
                lo = s;
            } else {
                hi = s;
            }
            s = 0.5 * (lo + hi);
        }
        Self::sample(self.y1, self.y2, s)
    }
}

/// Linear progress of a timed move: 0 before `delay`, 1 after `delay + dur`.
pub fn progress(elapsed_ms: u64, delay_ms: u64, dur_ms: u64) -> f32 {
    if dur_ms == 0 {
        return 1.0;
    }
    let e = elapsed_ms.saturating_sub(delay_ms) as f32;
    (e / dur_ms as f32).clamp(0.0, 1.0)
}

/// Content that exists only at the end fades in over the second half of the move.
pub fn fade_in(p: f32) -> f32 {
    ((p - 0.5) / 0.5).clamp(0.0, 1.0)
}

/// Content that exists only at the start fades out over the first third of the move.
pub fn fade_out(p: f32) -> f32 {
    1.0 - (p / 0.3).clamp(0.0, 1.0)
}

/// One pane as drawn at a moment: where, and how opaque.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pane {
    pub rect: Rect,
    pub alpha: f32,
}

/// The shell as drawn at one moment. Optional panes are `None` when they are not drawn at all
/// (absent, or fully faded), so the view draws exactly what is listed here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub sidebar: Option<Pane>,
    pub rail: Option<Pane>,
    pub drawer: Option<Pane>,
    pub half: Option<Pane>,
    pub caps: Option<Pane>,
    pub lights: Option<Pane>,
    pub main: Rect,
    pub tabs: Rect,
    pub rrail: Rect,
    pub sidebar_button: Rect,
    pub topr: Rect,
    pub bar: Rect,
    pub crumbs: Rect,
}

fn at_rest(r: Option<Rect>) -> Option<Pane> {
    r.map(|rect| Pane { rect, alpha: 1.0 })
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t), lerp(a.w, b.w, t), lerp(a.h, b.h, t))
}

impl Frame {
    /// The resting frame of a solved shell: every pane fully opaque where it is drawn.
    pub fn at_rest(s: &Shell) -> Frame {
        Frame {
            sidebar: at_rest(s.sidebar),
            rail: at_rest(s.rail),
            drawer: at_rest(s.drawer),
            half: at_rest(s.half),
            caps: at_rest(s.caps),
            lights: at_rest(s.lights),
            main: s.main,
            tabs: s.tabs,
            rrail: s.rrail,
            sidebar_button: s.sidebar_button,
            topr: s.topr,
            bar: s.bar,
            crumbs: s.crumbs,
        }
    }

    /// The frame `p` of the way from `from` to `to` (`p` is linear progress, 0 to 1).
    ///
    /// Panes present in both move in a straight line along the settle curve. A pane only in
    /// `to` slides in from its rest place and fades in over the second half; a pane only in
    /// `from` stays where it was and fades out over the first third. A drawer that appears
    /// slides in from `DRAWER_SLIDE` to the right. With `reduce`, nothing moves and the panes
    /// cross-fade on linear progress.
    pub fn between(from: &Frame, to: &Frame, p: f32, reduce: bool) -> Frame {
        let p = p.clamp(0.0, 1.0);
        let e = if reduce { p } else { SETTLE.at(p) };
        let pane = |a: Option<Pane>, b: Option<Pane>, slide: f32| -> Option<Pane> {
            let (rect, alpha) = match (a, b) {
                (Some(a), Some(b)) => {
                    let rect = if reduce { b.rect } else { lerp_rect(a.rect, b.rect, e) };
                    (rect, lerp(a.alpha, b.alpha, if reduce { p } else { e }))
                }
                (None, Some(b)) => {
                    let fade = if reduce { p } else { fade_in(p) };
                    let mut rect = b.rect;
                    if !reduce {
                        rect.x += slide * (1.0 - e);
                    }
                    (rect, b.alpha * fade)
                }
                (Some(a), None) => {
                    let fade = if reduce { 1.0 - p } else { fade_out(p) };
                    (a.rect, a.alpha * fade)
                }
                (None, None) => return None,
            };
            (alpha > 0.0).then_some(Pane { rect, alpha })
        };
        let rect = |a: Rect, b: Rect| if reduce { b } else { lerp_rect(a, b, e) };
        Frame {
            sidebar: pane(from.sidebar, to.sidebar, 0.0),
            rail: pane(from.rail, to.rail, 0.0),
            drawer: pane(from.drawer, to.drawer, DRAWER_SLIDE),
            half: pane(from.half, to.half, 0.0),
            caps: pane(from.caps, to.caps, 0.0),
            lights: pane(from.lights, to.lights, 0.0),
            main: rect(from.main, to.main),
            tabs: rect(from.tabs, to.tabs),
            rrail: rect(from.rrail, to.rrail),
            sidebar_button: rect(from.sidebar_button, to.sidebar_button),
            topr: rect(from.topr, to.topr),
            bar: rect(from.bar, to.bar),
            crumbs: rect(from.crumbs, to.crumbs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{solve, Platform, ShellState};

    const W: f32 = 1240.0;
    const H: f32 = 702.0;

    fn shell(sidebar: bool, drawer: bool, split: bool) -> Shell {
        solve(W, H, ShellState { sidebar_open: sidebar, drawer_open: drawer, split_open: split }, Platform::Windows)
    }

    fn close(a: Rect, b: Rect) -> bool {
        (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3 && (a.w - b.w).abs() < 1e-3 && (a.h - b.h).abs() < 1e-3
    }

    #[test]
    fn the_curve_pins_its_ends_and_is_monotone() {
        assert_eq!(SETTLE.at(0.0), 0.0);
        assert_eq!(SETTLE.at(1.0), 1.0);
        let mut last = 0.0;
        for i in 0..=100 {
            let y = SETTLE.at(i as f32 / 100.0);
            assert!(y >= last - 1e-4, "eased value went backwards at {i}");
            last = y;
        }
        // Fast start and long settle: well past half way in the first third of the time.
        assert!(SETTLE.at(0.3) > 0.6, "the settle curve should start fast, got {}", SETTLE.at(0.3));
    }

    #[test]
    fn progress_respects_delay_and_clamps() {
        assert_eq!(progress(0, 0, 300), 0.0);
        assert_eq!(progress(150, 0, 300), 0.5);
        assert_eq!(progress(900, 0, 300), 1.0);
        assert_eq!(progress(50, 100, 300), 0.0, "still in the delay");
        assert_eq!(progress(10, 0, 0), 1.0, "a zero-length move is already done");
    }

    #[test]
    fn first_frame_is_the_old_shell_and_last_frame_is_the_new_one() {
        let (a, b) = (shell(true, false, false), shell(true, true, false));
        let (fa, fb) = (Frame::at_rest(&a), Frame::at_rest(&b));
        let f0 = Frame::between(&fa, &fb, 0.0, false);
        assert!(close(f0.main, a.main), "main starts where it was");
        assert!(close(f0.rrail, a.rrail));
        assert_eq!(f0.drawer, None, "a drawer that is opening is not drawn at its first frame");
        let f1 = Frame::between(&fa, &fb, 1.0, false);
        assert!(close(f1.main, b.main), "main ends where the new shell says");
        let d = f1.drawer.expect("drawer is drawn at the end");
        assert!(close(d.rect, b.drawer.unwrap()));
        assert_eq!(d.alpha, 1.0);
    }

    #[test]
    fn hiding_the_sidebar_carries_the_main_card_and_swaps_the_rail_in() {
        let (a, b) = (shell(true, true, false), shell(false, true, false));
        let (fa, fb) = (Frame::at_rest(&a), Frame::at_rest(&b));
        let mut last_x = fa.main.x;
        for i in 1..=50 {
            let f = Frame::between(&fa, &fb, i as f32 / 50.0, false);
            assert!(f.main.x <= last_x + 1e-3, "main card moves left, never jumps back");
            last_x = f.main.x;
        }
        // The sidebar leaves first; the rail's icons only arrive once the move is half done.
        let early = Frame::between(&fa, &fb, 0.15, false);
        assert!(early.sidebar.is_some_and(|p| p.alpha > 0.0 && p.alpha < 1.0), "the sidebar fades out early");
        assert!(early.rail.is_none(), "the rail is not in yet");
        let late = Frame::between(&fa, &fb, 0.75, false);
        assert!(late.sidebar.is_none(), "the sidebar is gone by three quarters");
        assert!(late.rail.is_some_and(|p| p.alpha > 0.0 && p.alpha < 1.0), "the rail is fading in");
    }

    #[test]
    fn content_that_exists_only_at_the_end_waits_half_way() {
        assert_eq!(fade_in(0.5), 0.0);
        assert_eq!(fade_in(1.0), 1.0);
        assert!(fade_in(0.75) > 0.0 && fade_in(0.75) < 1.0);
        assert_eq!(fade_out(0.0), 1.0);
        assert_eq!(fade_out(0.3), 0.0);
    }

    #[test]
    fn reduced_motion_moves_nothing_and_only_cross_fades() {
        let (a, b) = (shell(true, false, false), shell(true, true, false));
        let (fa, fb) = (Frame::at_rest(&a), Frame::at_rest(&b));
        let mid = Frame::between(&fa, &fb, 0.5, true);
        assert!(close(mid.main, b.main), "the card snaps to its end size");
        let d = mid.drawer.expect("drawer cross-fades in");
        assert!((d.alpha - 0.5).abs() < 1e-3, "linear cross-fade, got {}", d.alpha);
        assert!(close(d.rect, b.drawer.unwrap()), "no slide");
    }

    #[test]
    fn an_interrupted_move_starts_from_where_it_was_drawn() {
        let (a, b) = (shell(true, false, false), shell(true, true, false));
        let c = shell(false, true, false);
        let (fa, fb, fc) = (Frame::at_rest(&a), Frame::at_rest(&b), Frame::at_rest(&c));
        let drawn = Frame::between(&fa, &fb, 0.4, false);
        // A new request arrives mid-move: the next move starts from the frame on screen.
        let next0 = Frame::between(&drawn, &fc, 0.0, false);
        assert!(close(next0.main, drawn.main), "no jump at the hand-over");
    }
}
