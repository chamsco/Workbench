//! Panels that learn the width you like and snap to it (the coordinator's panels spec, from
//! Charles, 13 Nov 19:29Z: "resizable panels that learn your preferred width and snap to it;
//! weighted for recency; snap unlocks for precise resizing"). Ported from the prototype's reference
//! model (`design/cauldron-resizable-panels.html`). Pure: the app keeps each panel's history and
//! asks where a drag lands.
//!
//! A panel keeps its last [`CAP`] settled widths (recorded on release, not on open). A width weighs
//! `0.5^(k / HALF)`, k being how many resizes came after it, so the newest counts most whatever
//! the time between. The preferred width is the peak of the weighted density (a Gaussian of
//! [`SIGMA`] px per width) over the panel's range, in 1px steps. It snaps only when enough weight
//! agrees: a total of at least 2 and a peak holding at least 34% of it.

/// How many settled widths a panel keeps.
pub const CAP: usize = 40;
/// A width's weight halves every this many later resizes.
pub const HALF: f32 = 5.0;
/// How wide one width's bump is, in px.
pub const SIGMA: f32 = 14.0;

/// A panel's allowed widths and its width with no history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

impl Range {
    pub fn clamp(&self, w: f32) -> f32 {
        w.clamp(self.min, self.max)
    }
}

/// What the history says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Learned {
    /// The most preferred width.
    pub peak: f32,
    /// The peak's share of the total weight, 0..1.
    pub confidence: f32,
    pub total: f32,
    /// Whether the peak is trusted enough to snap to.
    pub snaps: bool,
    /// Within this of the peak the magnet acts: 8px, up to 16 at full confidence.
    pub radius: f32,
}

fn weights(hist: &[f32]) -> impl Iterator<Item = (f32, f32)> + '_ {
    let n = hist.len();
    hist.iter().enumerate().map(move |(i, &w)| (w, 0.5f32.powf((n - 1 - i) as f32 / HALF)))
}

/// The weighted density of the history at `x`.
pub fn density(hist: &[f32], x: f32) -> f32 {
    weights(hist).map(|(w, wt)| wt * (-0.5 * ((x - w) / SIGMA).powi(2)).exp()).sum()
}

/// What a history has learned for a panel's range.
pub fn learn(hist: &[f32], r: Range) -> Learned {
    let total: f32 = weights(hist).map(|(_, wt)| wt).sum();
    let (mut peak, mut best) = (r.default, -1.0f32);
    let mut x = r.min.ceil();
    while x <= r.max {
        let f = density(hist, x);
        if f > best {
            best = f;
            peak = x;
        }
        x += 1.0;
    }
    let confidence = if total > 0.0 { best.max(0.0) / total } else { 0.0 };
    Learned { peak, confidence, total, snaps: total >= 2.0 && confidence >= 0.34, radius: 8.0 + 8.0 * confidence.min(1.0) }
}

/// Where a drag lands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landing {
    pub width: f32,
    /// On the learned width: "your usual".
    pub locked: bool,
}

/// The magnet: within `0.4 * radius` of the peak the width is the peak; out to the radius it
/// eases from the peak back to the raw width, joining both ends with no jump; beyond, the raw
/// width. `free` (Alt or Shift held) turns it off.
pub fn land(raw: f32, l: &Learned, free: bool) -> Landing {
    if free || !l.snaps {
        return Landing { width: raw, locked: false };
    }
    let (d, r) = (raw - l.peak, l.radius);
    let (a, r0) = (d.abs(), 0.4 * r);
    if a <= r0 {
        Landing { width: l.peak, locked: true }
    } else if a <= r {
        Landing { width: l.peak + d.signum() * (a - r0) * r / (r - r0), locked: false }
    } else {
        Landing { width: raw, locked: false }
    }
}

/// Keeps a settled width: the newest last, at most [`CAP`].
pub fn record(hist: &mut Vec<f32>, w: f32) {
    hist.push(w.round());
    if hist.len() > CAP {
        hist.remove(0);
    }
}

/// The width a panel opens at: the learned one when it snaps, else the default.
pub fn usual(hist: &[f32], r: Range) -> f32 {
    let l = learn(hist, r);
    if l.snaps {
        l.peak
    } else {
        r.default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIDE: Range = Range { min: 200.0, max: 360.0, default: 236.0 };

    #[test]
    fn the_peak_sits_on_the_seeded_history() {
        // The prototype's seed for the sidebar: most widths near 248.
        let hist = [240.0, 244.0, 248.0, 246.0, 252.0, 248.0, 250.0, 248.0];
        let l = learn(&hist, SIDE);
        assert!((l.peak - 248.0).abs() <= 1.0, "{l:?}");
        assert!(l.snaps && l.confidence > 0.34, "{l:?}");
        assert!(l.radius > 8.0 && l.radius <= 16.0);
        assert_eq!(usual(&hist, SIDE), l.peak);
    }

    /// No jump where the magnet starts or ends: at the lock zone's edge the width is the peak, at
    /// the radius it is the raw width.
    #[test]
    fn the_magnet_is_continuous() {
        let l = Learned { peak: 250.0, confidence: 0.5, total: 4.0, snaps: true, radius: 12.0 };
        let r0 = 0.4 * l.radius;
        for side in [-1.0f32, 1.0] {
            let at_r0 = land(250.0 + side * r0, &l, false).width;
            assert!((at_r0 - 250.0).abs() <= 0.01, "at R0: {at_r0}");
            let at_r = land(250.0 + side * l.radius, &l, false).width;
            assert!((at_r - (250.0 + side * l.radius)).abs() <= 0.01, "at R: {at_r}");
            let just_in = land(250.0 + side * (r0 + 0.001), &l, false).width;
            assert!((just_in - 250.0).abs() <= 0.01, "just past R0: {just_in}");
            let just_out = land(250.0 + side * (l.radius + 0.001), &l, false).width;
            assert!((just_out - (250.0 + side * (l.radius + 0.001))).abs() <= 0.01);
        }
        assert!(land(252.0, &l, false).locked && !land(258.0, &l, false).locked);
    }

    /// Eight old widths at 248, then six at 300: the newest win, so the peak moves to 300.
    #[test]
    fn the_peak_drifts_to_recent_widths() {
        let mut hist = Vec::new();
        for _ in 0..8 {
            record(&mut hist, 248.0);
        }
        assert!((learn(&hist, SIDE).peak - 248.0).abs() <= 1.0);
        for _ in 0..6 {
            record(&mut hist, 300.0);
        }
        assert!((learn(&hist, SIDE).peak - 300.0).abs() <= 1.0, "{:?}", learn(&hist, SIDE));
    }

    /// One odd resize is not a habit: no magnet from a single width.
    #[test]
    fn a_single_resize_does_not_snap() {
        let l = learn(&[312.0], SIDE);
        assert!(!l.snaps, "total weight 1 < 2: {l:?}");
        assert_eq!(land(309.0, &l, false), Landing { width: 309.0, locked: false });
        assert_eq!(usual(&[312.0], SIDE), SIDE.default);
        // A long habit with one odd width after it still snaps to the habit.
        let mut hist = vec![248.0; 10];
        record(&mut hist, 330.0);
        let l = learn(&hist, SIDE);
        assert!(l.snaps && (l.peak - 248.0).abs() <= 1.0, "{l:?}");
    }

    #[test]
    fn alt_or_shift_unlocks() {
        let l = learn(&[248.0; 8], SIDE);
        assert!(land(250.0, &l, false).locked);
        assert_eq!(land(250.0, &l, true), Landing { width: 250.0, locked: false }, "exact while held");
    }

    #[test]
    fn the_history_keeps_forty() {
        let mut hist = Vec::new();
        for i in 0..50 {
            record(&mut hist, 200.0 + i as f32);
        }
        assert_eq!(hist.len(), CAP);
        assert_eq!(hist[0], 210.0, "the oldest dropped");
    }
}
