//! Motion for the shell: how one state becomes the next, never what it is. Built on GPUI's
//! `Animation`, which lands on the final frame and skips straight to it under
//! `App::reduce_motion`; the app sets that flag from the OS at launch.

use cauldron_chrome::tokens::motion::{ease, stagger, INDICATOR, MENU, RISE};
use gpui_kit::prelude::*;
use gpui_kit::{canvas, div, px, AnimationExt, AnyElement, Bounds, Div, Hsla, Pixels, Point};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Windows' "Show animations in Windows" (SPI_GETCLIENTAREAANIMATION) is off.
#[cfg(windows)]
pub(crate) fn os_reduces_motion() -> bool {
    #[link(name = "user32")]
    extern "system" {
        fn SystemParametersInfoW(action: u32, param: u32, out: *mut core::ffi::c_void, ini: u32) -> i32;
    }
    let mut on: i32 = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to `out`.
    let ok = unsafe { SystemParametersInfoW(0x1042, 0, (&mut on as *mut i32).cast(), 0) };
    ok != 0 && on == 0
}

// ponytail: macOS reads NSWorkspace.accessibilityDisplayShouldReduceMotion when the Mac build ships.
#[cfg(not(windows))]
pub(crate) fn os_reduces_motion() -> bool {
    false
}

/// Debug builds stretch every duration by CAULDRON_MOTION_SLOW (e.g. 10) so mid-frames can be
/// screenshotted; release builds ignore it.
fn slow() -> f32 {
    if !cfg!(debug_assertions) {
        return 1.0;
    }
    std::env::var("CAULDRON_MOTION_SLOW").ok().and_then(|v| v.parse::<f32>().ok()).filter(|v| *v >= 1.0).unwrap_or(1.0)
}

/// A GPUI animation of `ms` on the shared curve (stretched in debug by CAULDRON_MOTION_SLOW).
pub(crate) fn anim(ms: f32) -> gpui_kit::Animation {
    gpui_kit::Animation::new(std::time::Duration::from_secs_f32(ms * slow() / 1000.0)).with_easing(ease)
}

/// Item `i` of a menu: fades in STAGGER after the one before, all done by MENU.
pub(crate) fn staggered(i: usize) -> gpui_kit::Animation {
    let (s, e) = stagger(i, MENU);
    gpui_kit::Animation::new(std::time::Duration::from_secs_f32(MENU * slow() / 1000.0)).with_easing(move |k| ease((k * MENU - s) / (e - s)))
}

/// A menu's panel inside `wrap` (which places it): fades in and rises the last RISE px toward
/// its button, over MENU. The wrapper animates so the panel keeps its test hooks.
pub(crate) fn pop_in(wrap: Div, el: impl IntoElement, id: &'static str, base_mt: f32) -> impl IntoElement {
    wrap.child(el).with_animation(id, anim(MENU), move |d, t| d.opacity(t).mt(px(base_mt - RISE * (1. - t))))
}

/// The hover highlight of a menu, one shape that slides between items (the item rects come
/// from the previous frame, measured by `measure`).
#[derive(Default)]
pub(crate) struct Slide {
    rects: Rc<RefCell<HashMap<usize, Bounds<Pixels>>>>,
    origin: Rc<Cell<Point<Pixels>>>,
    hover: Option<usize>,
    from: Option<usize>,
    moves: usize,
}

impl Slide {
    /// Pointer entered or left item `i`; true when that changed what is highlighted.
    pub(crate) fn hover(&mut self, i: usize, on: bool) -> bool {
        if on && self.hover != Some(i) {
            self.from = self.hover;
            self.hover = Some(i);
            self.moves += 1;
            true
        } else if !on && self.hover == Some(i) {
            self.hover = None;
            self.from = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn reset(&mut self) {
        self.hover = None;
        self.from = None;
    }

    /// An invisible layer that records item `i`'s rect (or the menu's own origin for None).
    pub(crate) fn measure(&self, i: Option<usize>) -> impl IntoElement {
        let (rects, origin) = (self.rects.clone(), self.origin.clone());
        canvas(
            move |b, _, _| match i {
                Some(i) => {
                    rects.borrow_mut().insert(i, b);
                }
                None => origin.set(b.origin),
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    /// The highlight, drawn first so the items sit over it.
    pub(crate) fn highlight(&self, fill: Hsla, radius: f32) -> Option<AnyElement> {
        let rects = self.rects.borrow();
        let o = self.origin.get();
        let rel = |i: usize| rects.get(&i).map(|b| (b.origin.y - o.y, b.origin.x - o.x, b.size));
        let (y, x, size) = rel(self.hover?)?;
        let from_y = self.from.and_then(rel).map_or(y, |f| f.0);
        let el = div().absolute().left(x).w(size.width).h(size.height).rounded(px(radius)).bg(fill);
        Some(el.with_animation(("slide", self.moves), anim(INDICATOR), move |d, t| d.top(from_y + (y - from_y) * t)).into_any_element())
    }
}
