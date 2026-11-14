//! Where the Library graph's nodes sit (core returns none: `memory_graph` is pure data). A small
//! force layout, deterministic so the same notes always draw the same picture: nodes start on
//! rings (Global in the middle, hubs around it, notes outside), edges pull like springs, every pair
//! pushes apart, and the whole cools over a fixed number of steps inside the box.

/// A node to place: whether it is a hub (drawn bigger, held nearer the middle).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Body {
    pub hub: bool,
}

/// Positions inside `w` x `h` (with a margin), one per body, for edges given as index pairs.
pub(crate) fn layout(bodies: &[Body], edges: &[(usize, usize)], w: f32, h: f32) -> Vec<(f32, f32)> {
    let n = bodies.len();
    if n == 0 {
        return Vec::new();
    }
    let (cx, cy) = (w / 2.0, h / 2.0);
    let margin = 28.0f32;
    let (rw, rh) = ((w / 2.0 - margin).max(10.0), (h / 2.0 - margin).max(10.0));
    // Start: the first body (Global) in the middle, hubs on an inner ring, notes on an outer one,
    // each ring spread evenly in list order.
    let hubs: Vec<usize> = (1..n).filter(|&i| bodies[i].hub).collect();
    let notes: Vec<usize> = (1..n).filter(|&i| !bodies[i].hub).collect();
    let mut p = vec![(cx, cy); n];
    for (k, &i) in hubs.iter().enumerate() {
        let a = k as f32 / hubs.len().max(1) as f32 * std::f32::consts::TAU + 0.3;
        p[i] = (cx + a.cos() * rw * 0.45, cy + a.sin() * rh * 0.45);
    }
    for (k, &i) in notes.iter().enumerate() {
        let a = k as f32 / notes.len().max(1) as f32 * std::f32::consts::TAU;
        p[i] = (cx + a.cos() * rw * 0.85, cy + a.sin() * rh * 0.85);
    }
    let area = (2.0 * rw) * (2.0 * rh);
    let k = (area / n as f32).sqrt() * 0.75;
    let steps = 300;
    for step in 0..steps {
        let t = (1.0 - step as f32 / steps as f32) * k * 0.5;
        let mut d = vec![(0.0f32, 0.0f32); n];
        for i in 0..n {
            for j in (i + 1)..n {
                let (dx, dy) = (p[i].0 - p[j].0, p[i].1 - p[j].1);
                let dist = (dx * dx + dy * dy).sqrt().max(0.5);
                let f = k * k / dist;
                let (ux, uy) = (dx / dist, dy / dist);
                d[i].0 += ux * f;
                d[i].1 += uy * f;
                d[j].0 -= ux * f;
                d[j].1 -= uy * f;
            }
        }
        for &(a, b) in edges {
            if a >= n || b >= n || a == b {
                continue;
            }
            let (dx, dy) = (p[a].0 - p[b].0, p[a].1 - p[b].1);
            let dist = (dx * dx + dy * dy).sqrt().max(0.5);
            let f = dist * dist / k;
            let (ux, uy) = (dx / dist, dy / dist);
            d[a].0 -= ux * f;
            d[a].1 -= uy * f;
            d[b].0 += ux * f;
            d[b].1 += uy * f;
        }
        for i in 0..n {
            if i == 0 {
                continue; // Global stays in the middle.
            }
            // Hubs feel a pull to the middle, so notes ring them rather than the reverse.
            let pull = if bodies[i].hub { 0.08 } else { 0.02 };
            d[i].0 += (cx - p[i].0) * pull * k / 10.0;
            d[i].1 += (cy - p[i].1) * pull * k / 10.0;
            let len = (d[i].0 * d[i].0 + d[i].1 * d[i].1).sqrt().max(0.001);
            let m = len.min(t);
            p[i].0 = (p[i].0 + d[i].0 / len * m).clamp(margin, w - margin);
            p[i].1 = (p[i].1 + d[i].1 / len * m).clamp(margin, h - margin);
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn star(hubs: usize, notes: usize) -> (Vec<Body>, Vec<(usize, usize)>) {
        let mut b = vec![Body { hub: true }];
        let mut e = Vec::new();
        for h in 0..hubs {
            b.push(Body { hub: true });
            e.push((0, 1 + h));
        }
        for i in 0..notes {
            b.push(Body { hub: false });
            e.push((1 + i % hubs.max(1), b.len() - 1));
        }
        (b, e)
    }

    #[test]
    fn the_same_graph_lays_out_the_same_inside_the_box() {
        let (b, e) = star(3, 14);
        let a = layout(&b, &e, 600.0, 420.0);
        assert_eq!(a, layout(&b, &e, 600.0, 420.0), "deterministic");
        assert!(a.iter().all(|&(x, y)| (28.0..=572.0).contains(&x) && (28.0..=392.0).contains(&y)), "inside the margin");
        assert_eq!(a[0], (300.0, 210.0), "Global in the middle");
    }

    #[test]
    fn nodes_spread_and_notes_sit_near_their_hub() {
        let (b, e) = star(3, 12);
        let p = layout(&b, &e, 600.0, 420.0);
        let dist = |i: usize, j: usize| ((p[i].0 - p[j].0).powi(2) + (p[i].1 - p[j].1).powi(2)).sqrt();
        for i in 0..p.len() {
            for j in (i + 1)..p.len() {
                assert!(dist(i, j) > 6.0, "{i} and {j} overlap");
            }
        }
        // A note is nearer its own hub than the average hub.
        for i in 4..p.len() {
            let own = 1 + (i - 4) % 3;
            let avg = (1..4).map(|h| dist(i, h)).sum::<f32>() / 3.0;
            assert!(dist(i, own) <= avg, "note {i} strays from its hub");
        }
    }
}
