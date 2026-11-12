//! Agent avatars: a textured head sphere over a body sphere, drawn as SVG from a seed, so every
//! agent keeps its own look with no image files. The rounded face carries the mood; the face
//! states follow orbling (github.com/kochjar/orbling, MIT).

use crate::tokens::{avatar as t, AGENT_DOTS};

/// What the face shows. Only the face changes with the mood.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Idle,
}

/// splitmix64: a tiny seeded generator, the same numbers on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in lo..hi.
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() >> 40) as f32 / (1u64 << 24) as f32 * (hi - lo)
    }
}

/// One sphere: the dot colour, patches of paint from seeded noise, then light and shade.
fn sphere(id: &str, cx: f32, cy: f32, r: f32, base: &str, rng: &mut Rng) -> String {
    let (freq, seed) = (rng.range(0.035, 0.09), rng.next() % 1000);
    // How much of the sphere the paint covers: the threshold on the noise.
    let cover = rng.range(0.42, 0.6);
    let (paint, light, shade) = (t::PAINT.to_hex(), t::LIGHT.to_hex(), t::SHADE.to_hex());
    format!(
        r#"<clipPath id="c{id}"><circle cx="{cx}" cy="{cy}" r="{r}"/></clipPath>
<filter id="p{id}" x="0" y="0" width="100%" height="100%"><feTurbulence type="fractalNoise" baseFrequency="{freq:.3}" numOctaves="2" seed="{seed}"/><feColorMatrix type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0"/><feComponentTransfer result="m"><feFuncA type="linear" slope="14" intercept="{icpt:.2}"/></feComponentTransfer><feFlood flood-color="{paint}"/><feComposite in2="m" operator="in"/></filter>
<radialGradient id="s{id}" cx="0.36" cy="0.3" r="0.78"><stop offset="0" stop-color="{light}" stop-opacity="0.55"/><stop offset="0.28" stop-color="{light}" stop-opacity="0"/><stop offset="0.72" stop-color="{shade}" stop-opacity="0.12"/><stop offset="1" stop-color="{shade}" stop-opacity="0.6"/></radialGradient>
<g clip-path="url(#c{id})"><circle cx="{cx}" cy="{cy}" r="{r}" fill="{base}"/><rect x="0" y="0" width="64" height="64" fill="{shade}" filter="url(#p{id})"/><circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#s{id})"/></g>"#,
        icpt = -14.0 * cover,
    )
}

/// The face: a rounded screen on the head, eyes drawn by mood.
fn face(cx: f32, cy: f32, mood: Mood) -> String {
    let (f, e, l) = (t::FACE.to_hex(), t::EYE.to_hex(), t::LIGHT.to_hex());
    let eyes = match mood {
        Mood::Idle => format!(
            r#"<rect x="{:.1}" y="{:.1}" width="3.2" height="5" rx="1.6" fill="{e}"/><rect x="{:.1}" y="{:.1}" width="3.2" height="5" rx="1.6" fill="{e}"/>"#,
            cx - 5.6, cy - 2.5, cx + 2.4, cy - 2.5
        ),
    };
    format!(
        r#"<rect x="{:.1}" y="{:.1}" width="21" height="14" rx="7" fill="{f}" stroke="{l}" stroke-opacity="0.1" stroke-width="0.6"/>{eyes}"#,
        cx - 10.5,
        cy - 7.0
    )
}

/// The avatar of agent `seed` with identity dot `dot`, as a 64x64 SVG.
pub fn svg(seed: u64, dot: usize, mood: Mood) -> String {
    let mut rng = Rng(seed);
    let base = AGENT_DOTS[dot % AGENT_DOTS.len()].to_hex();
    let shade = t::SHADE.to_hex();
    let body = sphere("b", 32.0, 46.0, 15.0, &base, &mut rng);
    let head = sphere("h", 32.0, 21.5, 15.5, &base, &mut rng);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
<ellipse cx="32" cy="61.5" rx="11" ry="1.8" fill="{shade}" fill-opacity="0.3"/>
{body}
<ellipse cx="32" cy="34" rx="11" ry="3" fill="{shade}" fill-opacity="0.35"/>
{head}
{face}
</svg>"#,
        face = face(32.0, 22.5, mood)
    )
}
