//! Thinking Orbs, six animated dot spheres for what an agent is doing: working, searching,
//! solving, listening, composing and shaping. A Rust port of the vanilla JavaScript adaptation
//! of Jakub Antalik's Thinking Orbs (https://github.com/Jakubantalik/thinking-orbs): the same
//! renderers, the same 20px/64px presets with interpolation between them, drawn as dots on a
//! transparent surface. The Canvas's status pill shows one (Charles, 27 Nov).
//!
//! MIT License
//!
//! Copyright (c) 2026 Jakub Antalik
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use std::f64::consts::PI;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Orb {
    Working,
    Searching,
    Solving,
    Listening,
    Composing,
    Shaping,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Orbits,
    Globe,
    Rubik,
    Wave,
    Ribbon,
    Morph,
}

impl Orb {
    fn mode(self) -> Mode {
        match self {
            Orb::Working => Mode::Orbits,
            Orb::Searching => Mode::Globe,
            Orb::Solving => Mode::Rubik,
            Orb::Listening => Mode::Wave,
            Orb::Composing => Mode::Ribbon,
            Orb::Shaping => Mode::Morph,
        }
    }
}

/// Every tunable of the renderers; each mode uses the ones it has.
#[derive(Clone, Copy, Default)]
struct Opts {
    lat_rings: f64,
    lon_density: f64,
    r_base: f64,
    r_depth: f64,
    r_boost: f64,
    ink_far: f64,
    ink_span: f64,
    rs_pow: f64,
    r_min: f64,
    orbit_n: f64,
    ghost_n: f64,
    ghost_r: f64,
    ghost_a: f64,
    particles: f64,
    part_r: f64,
    part_r_depth: f64,
    move_count: f64,
    r_active: f64,
    rings: f64,
    lanes: f64,
    segs: f64,
    r_dot: f64,
    icon_d: f64,
    scan_mul: f64,
    dim_base: f64,
    spin: f64,
    band_mul: f64,
    wob_mul: f64,
    spread: f64,
}

fn base(mode: Mode) -> Opts {
    let d = Opts { rs_pow: 0.6, r_min: 0.3, r_base: 0.6, r_depth: 1.7, ink_far: 0.62, ink_span: 0.54, r_boost: 1., scan_mul: 1., dim_base: 1., spin: 1., band_mul: 1., wob_mul: 1., spread: 1., ..Default::default() };
    match mode {
        Mode::Globe => Opts { lat_rings: 17., lon_density: 44., ..d },
        Mode::Orbits => Opts { orbit_n: 12., ghost_n: 40., ghost_r: 0.9, ghost_a: 0.5, particles: 3., part_r: 1.2, part_r_depth: 1.6, ..d },
        Mode::Rubik => Opts { lat_rings: 15., lon_density: 40., move_count: 14., r_active: 0.3, ..d },
        Mode::Wave => Opts { rings: 15., lon_density: 40., ..d },
        Mode::Ribbon => Opts { lanes: 5., segs: 88., ghost_n: 150., r_base: 1.1, ..d },
        Mode::Morph => Opts { r_dot: 0.021, icon_d: 1., r_min: 0.25, ..d },
    }
}

/// The 20px and 64px presets: speed, count, size and extras.
struct Preset {
    speed: f64,
    count: f64,
    size: f64,
    /// scanMul, dimBase, spin, bandMul, wobMul, spread (the ones a mode sets).
    extra: [Option<f64>; 6],
}

const NONE6: [Option<f64>; 6] = [None; 6];

fn presets(mode: Mode) -> (Preset, Preset) {
    let p = |speed, count, size, extra| Preset { speed, count, size, extra };
    match mode {
        Mode::Orbits => (p(3.9, 0.238, 2.4, NONE6), p(1.885, 1., 1., NONE6)),
        Mode::Globe => (p(2.665, 0.105, 1.75, [Some(4.335), Some(0.45), None, None, None, None]), p(2.015, 0.42, 1.15, [Some(4.08), Some(0.45), None, None, None, None])),
        Mode::Rubik => (p(1.95, 0.088, 1.9, NONE6), p(1.82, 0.35, 1.05, NONE6)),
        Mode::Wave => (p(3.998, 0.105, 1.6, NONE6), p(4.388, 0.341, 1., NONE6)),
        Mode::Ribbon => (p(3.12, 0.051, 1.073, [None, None, Some(0.), Some(4.94), Some(1.), None]), p(2.34, 0.25, 0.85, [None, None, Some(0.), Some(3.9), Some(1.), None])),
        Mode::Morph => (p(2.08, 0.53, 1.011, [None, None, None, None, None, Some(1.45)]), p(2.405, 0.54, 0.395, [None, None, None, None, None, Some(1.45)])),
    }
}

fn mix(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn scale_counts(o: &mut Opts, mode: Mode, m: f64) {
    let pair = m.sqrt();
    let rnd = |v: f64, k: f64| (v * k).round().max(2.);
    match mode {
        Mode::Globe | Mode::Rubik => {
            o.lat_rings = rnd(o.lat_rings, pair);
            o.lon_density = rnd(o.lon_density, pair);
        }
        Mode::Wave => {
            o.rings = rnd(o.rings, pair);
            o.lon_density = rnd(o.lon_density, pair);
        }
        Mode::Ribbon => {
            o.lanes = rnd(o.lanes, pair);
            o.segs = rnd(o.segs, pair);
            o.ghost_n = (o.ghost_n * m).round().max(1.);
        }
        Mode::Orbits => {
            o.orbit_n = (o.orbit_n * m).round().max(1.);
            o.ghost_n = (o.ghost_n * m).round().max(1.);
        }
        Mode::Morph => o.icon_d = (o.icon_d * m).max(0.02),
    }
}

fn scale_radii(o: &mut Opts, k: f64) {
    for v in [&mut o.r_base, &mut o.r_depth, &mut o.r_active, &mut o.r_dot, &mut o.ghost_r, &mut o.part_r, &mut o.part_r_depth] {
        *v *= k;
    }
}

/// An orb's settings at `size` px: the draw options and its tuned base speed.
fn resolve(orb: Orb, size: f64) -> (Mode, Opts, f64) {
    let mode = orb.mode();
    let (small, large) = presets(mode);
    let t = ((size - 20.) / 44.).clamp(0., 1.);
    let mut o = base(mode);
    scale_counts(&mut o, mode, mix(small.count, large.count, t));
    scale_radii(&mut o, mix(small.size, large.size, t));
    for i in 0..6 {
        let (a, b) = (small.extra[i], large.extra[i]);
        let Some(v) = a.or(b).zip(b.or(a)).map(|(a, b)| mix(a, b, t)) else { continue };
        match i {
            0 => o.scan_mul = v,
            1 => o.dim_base = v,
            2 => o.spin = v,
            3 => o.band_mul = v,
            4 => o.wob_mul = v,
            _ => o.spread = v,
        }
    }
    (mode, o, mix(small.speed, large.speed, t))
}

struct Dot {
    x: f64,
    y: f64,
    z: f64,
    r: f64,
    white: f64,
    a: f64,
}

fn dot(x: f64, y: f64, z: f64, r: f64, white: f64, a: f64) -> Dot {
    Dot { x, y, z, r, white, a }
}

fn hash(a: f64, b: f64) -> f64 {
    let v = (a * 12.9898 + b * 78.233).sin() * 43758.5453;
    v - v.floor()
}

fn angle_delta(a: f64, b: f64) -> f64 {
    (a - b).sin().atan2((a - b).cos())
}

fn radius_scale(size: f64, pow: f64) -> f64 {
    (size / 300.).powf(pow)
}

fn fibonacci(i: f64, n: f64) -> [f64; 3] {
    let golden = PI * (3. - 5f64.sqrt());
    let y = 1. - 2. * (i + 0.5) / n;
    let r = (1. - y * y).sqrt();
    let a = i * golden;
    [r * a.cos(), y, r * a.sin()]
}

fn projector(yaw: f64, tilt: f64, cx: f64, cy: f64, scale: f64) -> impl Fn(f64, f64, f64) -> (f64, f64, f64) {
    let (st, ct, sy, cy_) = (tilt.sin(), tilt.cos(), yaw.sin(), yaw.cos());
    move |x, y, z| {
        let rx = x * cy_ + z * sy;
        let rz = -x * sy + z * cy_;
        let ry = y * ct - rz * st;
        let depth = y * st + rz * ct;
        (cx + rx * scale, cy - ry * scale, depth)
    }
}

fn draw_orbits(size: f64, time: f64, o: &Opts) -> Vec<Dot> {
    let c = size / 2.;
    let radius = c * 0.82;
    let project = projector(time * 0.12, 0.3, c, c, 1.);
    let ds = radius_scale(size, o.rs_pow);
    let mut dots = Vec::new();
    for orbit in 0..o.orbit_n as usize {
        let (r1, r2, r3) = (hash(orbit as f64, 1.7), hash(orbit as f64, 5.2), hash(orbit as f64, 8.9));
        let orbit_r = radius * (0.45 + 0.52 * r1);
        let lon = r1 * 2. * PI;
        let lat = (2. * r2 - 1.).acos();
        let (nx, ny, nz) = (lat.sin() * lon.cos(), lat.cos(), lat.sin() * lon.sin());
        let (mut ux, mut uy, uz) = (-ny, nx, 0.);
        let ul = (ux * ux + uy * uy).sqrt().max(0.000001);
        ux /= ul;
        uy /= ul;
        let (px, py, pz) = (ny * uz - nz * uy, nz * ux - nx * uz, nx * uy - ny * ux);
        let dir = if r3 > 0.5 { 1. } else { -1. };
        let speed = (0.25 + 0.55 * r3) * dir;
        let at = |a: f64| project((ux * a.cos() + px * a.sin()) * orbit_r, (uy * a.cos() + py * a.sin()) * orbit_r, (uz * a.cos() + pz * a.sin()) * orbit_r);
        for i in 0..o.ghost_n as usize {
            let (x, y, z) = at(i as f64 / o.ghost_n * 2. * PI);
            let depth = (z / orbit_r + 1.) / 2.;
            dots.push(dot(x, y, z, o.ghost_r * ds, 0.72, o.ghost_a * (0.4 + 0.6 * depth)));
        }
        for i in 0..o.particles as usize {
            let (x, y, z) = at(time * speed + i as f64 / o.particles * 2. * PI + r2 * 6.);
            let depth = (z / orbit_r + 1.) / 2.;
            dots.push(dot(x, y, z, (o.part_r + o.part_r_depth * depth) * ds, 0.3 - 0.22 * depth, 1.));
        }
    }
    dots
}

fn draw_globe(size: f64, time: f64, o: &Opts) -> Vec<Dot> {
    let c = size / 2.;
    let radius = c * 0.82;
    let spin = 0.5;
    let tilt = 0.4 + 0.06 * (time * 0.35).sin();
    let project = projector(time * spin, tilt, c, c, radius);
    let scan = time * (spin + (1.7 - spin) * o.scan_mul);
    let ds = radius_scale(size, o.rs_pow);
    let mut dots = Vec::new();
    let rings = o.lat_rings as usize;
    for ring in 0..=rings {
        let lat = -PI / 2. + ring as f64 / o.lat_rings * PI;
        let (cl, sl) = (lat.cos(), lat.sin());
        let n = ((cl.abs() * o.lon_density).round() as usize).max(1);
        for i in 0..n {
            let lon = i as f64 / n as f64 * 2. * PI;
            let (x, y, z) = project(cl * lon.cos(), sl, cl * lon.sin());
            let depth = (z + 1.) / 2.;
            let dist = angle_delta(lon + time * spin, scan);
            let boost = (-(dist * dist) / 0.18).exp() * z.max(0.);
            dots.push(dot(x, y, z, (o.r_base + o.r_depth * depth + o.r_boost * boost) * ds, o.ink_far - o.ink_span * depth, o.dim_base + (1. - o.dim_base) * boost.min(1.)));
        }
    }
    dots
}

struct Move {
    axis: usize,
    min: f64,
    max: f64,
    angle: f64,
}

fn moves(count: usize) -> Vec<Move> {
    (0..count)
        .map(|i| {
            let i = i as f64;
            let axis = ((hash(i, 2.3) * 3.).floor() as usize).min(2);
            let min = -1. + 0.5 * (hash(i, 5.9) * 4.).floor().min(3.);
            let dir = if hash(i, 7.7) < 0.5 { 1. } else { -1. };
            Move { axis, min, max: min + 0.5, angle: dir * PI / 2. }
        })
        .collect()
}

fn draw_rubik(size: f64, time: f64, o: &Opts) -> Vec<Dot> {
    let c = size / 2.;
    let radius = c * 0.82;
    let project = projector(time * 0.55, 0.35 + 0.1 * (time * 0.9).sin(), c, c, radius);
    let ds = radius_scale(size, o.rs_pow);
    let count = o.move_count as usize;
    let mv = moves(count);
    // The solve cycle: each move turns in, then back out in reverse, then a rest.
    let (slot_d, rest_d) = (0.42, 1.2);
    let cycle = 2. * count as f64 * slot_d + rest_d;
    let ct = time % cycle;
    let mut amounts = vec![0.; count];
    let mut active: i64 = -1;
    if ct < 2. * count as f64 * slot_d {
        let slot = (ct / slot_d).floor() as usize;
        let p = ((ct - slot as f64 * slot_d) / slot_d / 0.7).min(1.);
        let e = 1. - (1. - p).powi(3);
        if slot < count {
            for a in amounts.iter_mut().take(slot) {
                *a = 1.;
            }
            amounts[slot] = e;
            active = slot as i64;
        } else {
            let r = 2 * count - 1 - slot;
            for a in amounts.iter_mut().take(r) {
                *a = 1.;
            }
            amounts[r] = 1. - e;
            active = r as i64;
        }
    }
    let mut dots = Vec::new();
    for ring in 0..=o.lat_rings as usize {
        let lat = -PI / 2. + ring as f64 / o.lat_rings * PI;
        let (cl, sl) = (lat.cos(), lat.sin());
        let n = ((cl.abs() * o.lon_density).round() as usize).max(1);
        for i in 0..n {
            let lon = i as f64 / n as f64 * 2. * PI;
            let (mut x, mut y, mut z) = (cl * lon.cos(), sl, cl * lon.sin());
            let mut in_active = false;
            for (k, m) in mv.iter().enumerate() {
                let amt = amounts[k];
                if amt <= 0. {
                    continue;
                }
                let coord = [x, y, z][m.axis];
                if coord < m.min || coord >= m.max {
                    continue;
                }
                if k as i64 == active {
                    in_active = true;
                }
                let (cs, sn) = ((m.angle * amt).cos(), (m.angle * amt).sin());
                match m.axis {
                    0 => {
                        let ny = y * cs - z * sn;
                        z = y * sn + z * cs;
                        y = ny;
                    }
                    1 => {
                        let nx = x * cs + z * sn;
                        z = -x * sn + z * cs;
                        x = nx;
                    }
                    _ => {
                        let nx = x * cs - y * sn;
                        y = x * sn + y * cs;
                        x = nx;
                    }
                }
            }
            let (px, py, pz) = project(x, y, z);
            let depth = (pz + 1.) / 2.;
            let extra = if in_active { o.r_active } else { 0. };
            dots.push(dot(px, py, pz, (o.r_base + o.r_depth * depth + extra) * ds, o.ink_far - o.ink_span * depth - if in_active { 0.14 } else { 0. }, 1.));
        }
    }
    dots
}

fn draw_wave(size: f64, time: f64, o: &Opts) -> Vec<Dot> {
    let c = size / 2.;
    let radius = c * 0.874;
    let project = projector(time * 0.18, 0.38, c, c, 1.);
    let ds = radius_scale(size, o.rs_pow);
    let mut dots = Vec::new();
    for ring in 0..=o.rings as usize {
        let rf = ring as f64;
        let lat = -PI / 2. + rf / o.rings * PI;
        let (cl, sl) = (lat.cos(), lat.sin());
        let wave = 0.62 * (time * 2.1 - rf * 0.52).sin() + 0.38 * (time * 1.27 + rf * 0.83).sin();
        let rr = radius * (0.88 + 0.105 * wave);
        let n = ((cl.abs() * o.lon_density).round() as usize).max(1);
        for i in 0..n {
            let lon = i as f64 / n as f64 * 2. * PI;
            let (x, y, z) = project(cl * lon.cos() * rr, sl * rr, cl * lon.sin() * rr);
            let depth = (z / radius + 1.) / 2.;
            let crest = wave.max(0.);
            dots.push(dot(x, y, z, (o.r_base + o.r_depth * depth) * (1. + 0.4 * crest) * ds, 0.66 - 0.56 * depth - 0.1 * crest, 1.));
        }
    }
    dots
}

fn draw_ribbon(size: f64, time: f64, o: &Opts) -> Vec<Dot> {
    let c = size / 2.;
    let radius = c * 0.78;
    let project = projector(time * 0.1 * o.spin, 0.3, c, c, 1.);
    let ds = radius_scale(size, o.rs_pow);
    let mut dots = Vec::new();
    for i in 0..o.ghost_n as usize {
        let d = fibonacci(i as f64, o.ghost_n);
        let (x, y, z) = project(d[0] * radius, d[1] * radius, d[2] * radius);
        let depth = (z / radius + 1.) / 2.;
        dots.push(dot(x, y, z, 0.8 * ds, 0.78, 0.1 + 0.22 * depth));
    }
    let yaw = time * 0.24 * o.spin;
    let tilt = 0.55 + 0.3 * (time * 0.18).sin() * o.spin;
    let (ux, uy, uz) = (yaw.cos(), 0., yaw.sin());
    let (px, py, pz) = (-uz * tilt.sin(), tilt.cos(), ux * tilt.sin());
    let (nx, ny, nz) = (uy * pz - uz * py, uz * px - ux * pz, ux * py - uy * px);
    let lanes = ((o.lanes * o.band_mul).round() as usize).max(1);
    let segs = o.segs as usize;
    for lane in 0..lanes {
        let mid = (lanes as f64 - 1.) / 2.;
        let lane_off = (lane as f64 - mid) * 0.075;
        let edge = (lane as f64 - mid).abs() / mid.max(1.);
        for s in 0..segs {
            let a = s as f64 / segs as f64 * 2. * PI;
            let wobble = (0.16 * (a * 3. - time * 1.7 + lane as f64 * 0.22).sin() + 0.07 * (a * 5. + time * 1.1).sin()) * o.wob_mul;
            let off = lane_off + wobble;
            let rx = ux * a.cos() + px * a.sin() + nx * off;
            let ry = uy * a.cos() + py * a.sin() + ny * off;
            let rz = uz * a.cos() + pz * a.sin() + nz * off;
            let len = (rx * rx + ry * ry + rz * rz).sqrt();
            let (x, y, z) = project(rx / len * radius, ry / len * radius, rz / len * radius);
            let depth = (z / radius + 1.) / 2.;
            dots.push(dot(x, y, z, (o.r_base + o.r_depth * depth) * (1. - 0.25 * edge) * ds, 0.52 - 0.44 * depth + 0.18 * edge, 0.4 + 0.6 * depth));
        }
    }
    dots
}

fn polygon(vs: &[[f64; 2]], progress: f64) -> [f64; 2] {
    let n = vs.len();
    let lens: Vec<f64> = (0..n).map(|i| ((vs[(i + 1) % n][0] - vs[i][0]).powi(2) + (vs[(i + 1) % n][1] - vs[i][1]).powi(2)).sqrt()).collect();
    let total: f64 = lens.iter().sum();
    let mut target = progress * total;
    let mut seg = 0;
    while target > lens[seg] && seg < n - 1 {
        target -= lens[seg];
        seg += 1;
    }
    let (a, b) = (vs[seg], vs[(seg + 1) % n]);
    let t = if lens[seg] > 0. { (target / lens[seg]).min(1.) } else { 0. };
    [mix(a[0], b[0], t), mix(a[1], b[1], t)]
}

fn shape(i: usize, p: f64) -> [f64; 2] {
    match i {
        0 => {
            let a = -PI / 2. + p * 2. * PI;
            [a.cos() * 0.24, a.sin() * 0.24]
        }
        1 => polygon(&[[0., -0.26], [0.24, 0.16], [-0.24, 0.16]], p),
        _ => polygon(&[[0., -0.2], [0.2, -0.2], [0.2, 0.2], [-0.2, 0.2], [-0.2, -0.2]], p),
    }
}

fn draw_morph(size: f64, time: f64, o: &Opts) -> Vec<Dot> {
    let (hold, morph) = (1.4, 0.9);
    let seg_d = hold + morph;
    let ct = time % (seg_d * 3.);
    let pi = (ct / seg_d).floor() as usize;
    let local = ct - pi as f64 * seg_d;
    let mp = if local > hold {
        let t = ((local - hold) / morph).clamp(0., 1.);
        t * t * (3. - 2. * t)
    } else {
        0.
    };
    let spread = o.spread;
    let samples = 160usize;
    let pts: Vec<[f64; 2]> = (0..samples)
        .map(|i| {
            let p = i as f64 / samples as f64;
            let (a, b) = (shape(pi, p), shape((pi + 1) % 3, p));
            [mix(a[0], b[0], mp) * spread, mix(a[1], b[1], mp) * spread]
        })
        .collect();
    let lens: Vec<f64> = (0..samples).map(|i| ((pts[(i + 1) % samples][0] - pts[i][0]).powi(2) + (pts[(i + 1) % samples][1] - pts[i][1]).powi(2)).sqrt()).collect();
    let total: f64 = lens.iter().sum();
    let count = ((34. * o.icon_d).round() as usize).max(6);
    let dot_r = o.r_dot * 1.35 * spread;
    let pulse = 1. + 0.02 * (local * 3.1).sin();
    let c = size / 2.;
    let mut dots = Vec::new();
    let (mut seg, mut acc) = (0usize, 0.);
    for i in 0..count {
        let target = i as f64 / count as f64 * total;
        while acc + lens[seg] < target && seg < samples - 1 {
            acc += lens[seg];
            seg += 1;
        }
        let (a, b) = (pts[seg], pts[(seg + 1) % samples]);
        let t = if lens[seg] > 0. { ((target - acc) / lens[seg]).min(1.) } else { 0. };
        let (x, y) = (mix(a[0], b[0], t) * pulse, mix(a[1], b[1], t) * pulse);
        dots.push(dot(c + x * size, c + y * size, 0., (dot_r * size).max(0.35), 0.1, 1.));
    }
    dots
}

/// The dots of `orb` at `size` px and `time` seconds (already scaled by `speed`).
fn frame(orb: Orb, size: f64, time: f64) -> (Vec<Dot>, f64) {
    let (mode, o, _) = resolve(orb, size);
    let mut dots = match mode {
        Mode::Orbits => draw_orbits(size, time, &o),
        Mode::Globe => draw_globe(size, time, &o),
        Mode::Rubik => draw_rubik(size, time, &o),
        Mode::Wave => draw_wave(size, time, &o),
        Mode::Ribbon => draw_ribbon(size, time, &o),
        Mode::Morph => draw_morph(size, time, &o),
    };
    dots.sort_by(|a, b| a.z.total_cmp(&b.z));
    (dots, o.r_min)
}

/// The orb as an element `size` px square. `now_ms` drives it; `still` draws the frozen frame
/// (reduced motion, or nothing happening). Transparent: it sits on any surface; `dark` picks ink.
pub(crate) fn orb(kind: Orb, size: f32, now_ms: u64, still: bool, dark: bool) -> impl IntoElement {
    let sz = size as f64;
    let (_, _, base_speed) = resolve(kind, sz);
    let time = if still { 0.6 } else { now_ms as f64 / 1000. * base_speed };
    let (dots, r_min) = frame(kind, sz, time);
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let o = b.origin;
            for d in &dots {
                if d.a < 0.02 {
                    continue;
                }
                let white = d.white.clamp(0., 1.);
                let g = (if dark { 1. - white } else { white }) as f32;
                let col = Hsla { h: 0., s: 0., l: g, a: d.a as f32 };
                let r = d.r.max(r_min) as f32;
                let at = point(o.x + px(d.x as f32 - r), o.y + px(d.y as f32 - r));
                window.paint_quad(fill(Bounds::new(at, gpui::size(px(2. * r), px(2. * r))), col).corner_radii(px(r)));
            }
        },
    )
    .size(px(size))
}
