//! Status marks: a 5x5 dot grid that says what a stream or an agent is doing (Charles picked
//! option C of the status-motion board, 21 Nov). Plain streams get this mark in place of the old
//! state dot; agents keep their head and carry the same mark beside it. Patterns are drawn here
//! from scratch, one per state, stepped at `FRAME_MS`. No GPUI: the app only paints the 25 cells.

/// One mark per state. `Thinking` and `Waiting` exist for the chat's status line and for agents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Idle,
    Thinking,
    Working,
    Asking,
    Waiting,
    Error,
}

pub const MARKS: [Mark; 6] = [Mark::Idle, Mark::Thinking, Mark::Working, Mark::Asking, Mark::Waiting, Mark::Error];

/// The step of the motion: 8 frames a second, so a list of rows costs a redraw every 125 ms.
pub const FRAME_MS: u64 = 125;

/// The grid is `N` x `N`, row by row from the top left.
pub const N: usize = 5;

/// The sixteen cells of the outer ring, clockwise from the top left (the thinking spark runs on it).
const RING: [(usize, usize); 16] = [
    (0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (4, 1), (4, 2), (4, 3),
    (4, 4), (3, 4), (2, 4), (1, 4), (0, 4), (0, 3), (0, 2), (0, 1),
];

/// The question glyph the asking mark holds.
const QUESTION: [(usize, usize); 8] = [(1, 0), (2, 0), (3, 0), (0, 1), (4, 1), (3, 2), (2, 3), (2, 4)];

impl Mark {
    /// Screen readers and tooltips.
    pub fn label(self) -> &'static str {
        match self {
            Mark::Idle => "Idle",
            Mark::Thinking => "Thinking",
            Mark::Working => "Working",
            Mark::Asking => "Needs you",
            Mark::Waiting => "Waiting",
            Mark::Error => "Failed",
        }
    }
}

/// Each cell's strength, 0 to 1. `frame` is the step count since the clock started; `None` is a
/// still frame (reduced motion, tests): the glyph or shape at full strength, never blank.
pub fn grid(mark: Mark, frame: Option<u32>) -> [f32; N * N] {
    let mut g = [0.0f32; N * N];
    let mut set = |x: usize, y: usize, v: f32| {
        let i = y * N + x;
        g[i] = g[i].max(v);
    };
    let t = frame.map_or(0.0, |f| f as f32 * FRAME_MS as f32 / 1000.0);
    let live = frame.is_some();
    match mark {
        Mark::Idle => set(2, 2, if live { 0.35 + 0.15 * (t * 1.2).sin() } else { 0.4 }),
        Mark::Thinking => {
            let head = if live { (t * 6.0) as usize % 16 } else { 3 };
            for k in 0..4 {
                let (x, y) = RING[(head + 16 - k) % 16];
                set(x, y, 1.0 - k as f32 * 0.3);
            }
            set(2, 2, 0.15);
        }
        Mark::Working => {
            for x in 0..N {
                let h = if live { (3.0 + 1.5 * (t * 5.0 + x as f32 * 1.3).sin()).round() as usize } else { [2, 3, 5, 3, 2][x] };
                for y in 0..h.min(N) {
                    set(x, N - 1 - y, 1.0);
                }
            }
        }
        Mark::Asking => {
            let a = if live { 0.75 + 0.25 * (t * 4.0).sin() } else { 1.0 };
            for (x, y) in QUESTION {
                set(x, y, a);
            }
        }
        Mark::Waiting => {
            let on = if live { (t * 3.0) as usize % 3 } else { 1 };
            for i in 0..3 {
                set(1 + i, 2, if i == on { 1.0 } else { 0.28 });
            }
        }
        Mark::Error => {
            let a = if live && (t * 2.0) as usize % 2 == 1 { 0.5 } else { 1.0 };
            for i in 0..N {
                set(i, i, a);
                set(N - 1 - i, i, a);
            }
        }
    }
    g
}
