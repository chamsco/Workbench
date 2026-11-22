//! Status marks in the shell: the 5x5 dot grid from `cauldron_chrome::status`, painted as 25 small
//! cells. One wall clock steps every mark at `FRAME_MS`; a ticker redraws the shell only while a
//! moving mark was drawn, and reduced motion shows the still frame.

use super::*;
use cauldron_chrome::status::{self, Mark};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static START: OnceLock<std::time::Instant> = OnceLock::new();
static STILL: AtomicBool = AtomicBool::new(false);
static DRAWN: AtomicBool = AtomicBool::new(false);

/// The mark clock's step, or None under reduced motion.
fn frame() -> Option<u32> {
    if STILL.load(Ordering::Relaxed) {
        return None;
    }
    let t0 = START.get_or_init(std::time::Instant::now);
    Some((t0.elapsed().as_millis() as u64 / status::FRAME_MS) as u32)
}

/// The size every row uses: a 3px cell, so each lit dot is about 2px and holds on a 1x screen.
pub(crate) const SIZE: f32 = 15.0;

/// What a tab spends on its mark: the mark and the gap after it.
pub(crate) const TAB_W: f32 = SIZE + 6.0;

/// A `px` square mark in `color`. `id` is the element id; the state is its aria label.
pub(crate) fn mark(id: impl Into<ElementId>, m: Mark, px_: f32, color: Hsla) -> Stateful<Div> {
    let f = frame();
    if f.is_some() {
        DRAWN.store(true, Ordering::Relaxed);
    }
    let g = status::grid(m, f);
    let cell = px_ / status::N as f32;
    let dot = cell * 0.72;
    let mut d = div().id(id).size(px(px_)).flex_none().flex().flex_wrap().aria_label(m.label());
    for v in g {
        d = d.child(div().size(px(cell)).flex_none().flex().items_center().justify_center().child(div().size(px(dot)).bg(color.opacity(color.a * (0.05 + 0.95 * v)))));
    }
    d
}

impl Cauldron {
    /// The mark and colour of stream `id`, as its sidebar row shows it.
    pub(crate) fn stream_mark(&self, id: &str, pal: &Palette) -> (Mark, Rgb) {
        use crate::threads::Group;
        let Threads::Ready(v) = &self.threads else { return (Mark::Idle, pal.fg4) };
        match v.iter().find(|t| t.id == id) {
            Some(t) => match Group::of(t) {
                Group::Running => (Mark::Working, pal.accent),
                Group::NeedsYou if t.failed.is_some() => (Mark::Error, pal.bad),
                Group::NeedsYou => (Mark::Asking, pal.warn),
                Group::Done | Group::Archived => (Mark::Idle, pal.fg4),
            },
            None => (Mark::Idle, pal.fg4),
        }
    }

    /// Called once at the top of each render: reads whether the last frame drew a mark, and keeps
    /// the ticker running only then.
    pub(crate) fn mark_clock(&mut self, reduce: bool, cx: &mut Context<Self>) {
        STILL.store(reduce, Ordering::Relaxed);
        let drawn = DRAWN.swap(false, Ordering::Relaxed);
        if !drawn {
            self.mark_ticker = None;
        } else if self.mark_ticker.is_none() {
            self.mark_ticker = Some(cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| loop {
                cx.background_executor().timer(std::time::Duration::from_millis(status::FRAME_MS)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }));
        }
    }
}
