// The face: a bot's brain.
//
// The design is the owner's: a FILLED, lumpy, organic brain in the bot's colour,
// with the folds cut in as negative space and two white pill eyes. An earlier
// pass used Tabler's `brain` icon — six stroked paths — which was close but
// read as a line drawing; this one has body.
//
// The silhouette is a union of discs: one core, a crown of lobes, two sides, a
// base and a little stem underneath. That generator is exactly an organic lumpy
// blob, and — the useful part — the valleys BETWEEN the discs are where the
// folds belong, so the gyri are derived from the lobe layout rather than drawn
// by hand. Anatomically the folds are the lobe boundaries; here they literally
// are. One light smoothing pass takes the cusps off without erasing the lobes
// (two passes, which is what the old blob engine used, flattens it to a circle).
//
// Everything around the silhouette is the animation layer: one shared rAF clock
// for every face on screen (rects read in a single pass, DOM written through
// refs, so a frame never touches React state), breath, a slow wander, a blink
// whose phase is the bot's own, a gaze that aims from THIS face at the pointer,
// and a 1.2s hover that closes the eyes into two smiles. `frame()` stays pure
// in t, so pause/resume/seek give the same image.
//
// The bot's colour is `avatar.color`. The stored emoji draws nothing; it only
// phases the blink, so a row of brains never blinks in unison.

import { useEffect, useMemo, useRef } from "react";

import { agentById, type Agent } from "../bridge";

// ---------------------------------------------------------------- the brain

const TAU = Math.PI * 2;
const SAMPLES = 72;
const ANGLES = Array.from({ length: SAMPLES }, (_, i) => (i / SAMPLES) * TAU);
const COS = ANGLES.map(Math.cos);
const SIN = ANGLES.map(Math.sin);

/** A lobe: `a` degrees from the top (clockwise), `d` out from the centre, `r`
 *  its radius — all in ball radii. The last two are the base and the stem. */
const LOBES = [
  { a: 0, d: 0.0, r: 0.6 },
  { a: 0, d: 0.46, r: 0.32 },
  { a: 40, d: 0.48, r: 0.3 },
  { a: -40, d: 0.48, r: 0.3 },
  { a: 85, d: 0.52, r: 0.3 },
  { a: -85, d: 0.52, r: 0.3 },
  { a: 130, d: 0.48, r: 0.28 },
  { a: -130, d: 0.48, r: 0.28 },
  { a: 180, d: 0.34, r: 0.22 },
  { a: 180, d: 0.64, r: 0.14 },
];

const polar = (aDeg: number, d: number) => {
  const rad = (aDeg * Math.PI) / 180 - Math.PI / 2; // 0deg = the top
  return { x: Math.cos(rad) * d, y: Math.sin(rad) * d };
};
const DISCS = LOBES.map((l) => ({ ...polar(l.a, l.d), r: l.r, a: l.a }));

/** The union of the discs, sampled around the circle, cusps taken off. */
const SIL = (() => {
  let radii = ANGLES.map((_, i) => {
    let best = 0;
    for (const c of DISCS) {
      const b = COS[i] * c.x + SIN[i] * c.y;
      const disc = b * b - (c.x * c.x + c.y * c.y - c.r * c.r);
      if (disc < 0) continue;
      const t = b + Math.sqrt(disc);
      if (t > best) best = t;
    }
    return best;
  });
  const next = new Array<number>(SAMPLES);
  for (let i = 0; i < SAMPLES; i++) {
    next[i] = (radii[(i - 1 + SAMPLES) % SAMPLES] + 2 * radii[i] + radii[(i + 1) % SAMPLES]) / 4;
  }
  return next.map((r) => Math.min(Math.max(r, 0.5), 1.3));
})();

/** Profile radius at an angle in degrees, interpolated between samples. */
function radiusAt(aDeg: number): number {
  const t = ((((aDeg + 360) % 360) / 360) * SAMPLES + SAMPLES) % SAMPLES;
  const i = Math.floor(t);
  return (SIL[i % SAMPLES] ?? 1) * (1 - (t - i)) + (SIL[(i + 1) % SAMPLES] ?? 1) * (t - i);
}

/** The ball radius in the viewBox, and the vertical squash: a brain is a touch
 *  wider than it is tall. */
const R = 60;
const SQUASH = 0.95;
const r2 = (n: number) => Math.round(n * 100) / 100;
/** A point on the profile, in viewBox units. */
const at = (aDeg: number, rad: number) => {
  const a = (aDeg * Math.PI) / 180 - Math.PI / 2;
  return { x: Math.cos(a) * rad * R, y: Math.sin(a) * rad * R * SQUASH };
};

/** Closed polyline to Catmull-Rom cubics — centred tangents are enough. */
function closedPath(pts: { x: number; y: number }[]): string {
  const n = pts.length;
  if (n < 3) return "";
  let d = `M${r2(pts[0].x)} ${r2(pts[0].y)}`;
  for (let i = 0; i < n; i++) {
    const p0 = pts[(i - 1 + n) % n];
    const p1 = pts[i];
    const p2 = pts[(i + 1) % n];
    const p3 = pts[(i + 2) % n];
    d += `C${r2(p1.x + (p2.x - p0.x) / 6)} ${r2(p1.y + (p2.y - p0.y) / 6)} ${r2(p2.x - (p3.x - p1.x) / 6)} ${r2(p2.y + (p3.y - p1.y) / 6)} ${r2(p2.x)} ${r2(p2.y)}`;
  }
  return `${d}Z`;
}

/** The silhouette, once: every bot wears the same brain. */
const SILHOUETTE = closedPath(ANGLES.map((_, i) => at((i / SAMPLES) * 360, SIL[i])));

/** The folds, cut in as negative space. One per valley between neighbouring
 *  lobes — a coarser ring than the silhouette's, because five folds read as
 *  folds and eight read as noise — plus the longitudinal fissure from the crown
 *  down to just above the eyes. The eye zone is left clean. */
const GYRI_RING = [
  { a: 0, d: 0.46, r: 0.32 },
  { a: 45, d: 0.5, r: 0.3 },
  { a: -45, d: 0.5, r: 0.3 },
  { a: 90, d: 0.52, r: 0.3 },
  { a: -90, d: 0.52, r: 0.3 },
  { a: 135, d: 0.48, r: 0.28 },
  { a: -135, d: 0.48, r: 0.28 },
  { a: 180, d: 0.34, r: 0.22 },
].sort((x, y) => x.a - y.a);

function folds(): string[] {
  const out: string[] = [];
  const n = GYRI_RING.length;
  for (let i = 0; i < n; i++) {
    const A = GYRI_RING[i];
    const B = GYRI_RING[(i + 1) % n];
    let va = (A.a + B.a) / 2;
    if (Math.abs(A.a - B.a) > 180) va += 180; // wrap across the bottom
    const dir = polar(va, 1);
    // Folds live in the crown and on the shoulders. The eye zone and the stem
    // stay clean: a fold arcing around the eyes reads as eyewear, not as a
    // brain, and a symmetric pair of them reads as a designed face.
    if (dir.y > -0.12) continue;
    // Break the mirror symmetry the way a real brain's folds are broken:
    // alternate the reach and the length so no two folds match.
    const rad = radiusAt(va) * (i % 2 === 0 ? 0.78 : 0.86);
    const span = 13 + (i % 3) * 6;
    const p1 = at(va - span, rad);
    const c = at(va, rad * 1.04); // bows along the lobe edge, not into the face
    const p2 = at(va + span, rad);
    out.push(`M${r2(p1.x)} ${r2(p1.y)}Q${r2(c.x)} ${r2(c.y)} ${r2(p2.x)} ${r2(p2.y)}`);
  }
  // The fissure: crown down to just above the eyes.
  const t = at(0, radiusAt(0) * 0.94);
  const m = at(0, 0.34);
  const b = at(0, -0.16);
  out.push(`M${r2(t.x)} ${r2(t.y)}Q${r2(m.x)} ${r2(m.y)} ${r2(b.x)} ${r2(b.y)}`);
  return out;
}

/** The folds are the silhouette's colour lightened — the reference's negative
 *  space is a tint of the brain, not white. */
function lighten(hex: string, amt: number): string {
  const n = Number.parseInt(hex.slice(1), 16);
  if (!Number.isFinite(n)) return hex;
  const r = (n >> 16) & 255;
  const g = (n >> 8) & 255;
  const b = n & 255;
  const m = (c: number) => Math.round(c + (255 - c) * amt);
  return `rgb(${m(r)},${m(g)},${m(b)})`;
}

// ---------------------------------------------------------------- the face

export type OrbState = "idle" | "working" | "needs" | "wake";

/** Eyes, in viewBox units: white pills, wider apart than a blob's would be. */
const EYE = { w: 8.5, h: 19, split: 18.5, gaze: 3.2 };
/** The badge, parked off the top-right lobe. */
const NOTIF = { x: 40, y: -38, r: 5.2 };
const BLINK_EVERY = 4.6;
/** Hover this long and the face cheers. */
const CHEER_AT = 1.2;

/** One frame of the face, pure in `t` (seconds), `look` (-1..1 toward the
 *  pointer) and `cheer` (0..1, the hover smile). */
function frame(state: OrbState, t: number, look: { x: number; y: number }, cheer: number) {
  // Breath: quicker and deeper the busier the state.
  const rate = state === "working" ? 2.1 : 1.05;
  const depth = state === "working" ? 0.05 : 0.018;
  const breath = Math.sin(t * rate);
  const sx = 1 - depth * 0.6 * breath;
  const sy = 1 + depth * breath;
  // Wander: an idle face drifts a little, a working one bobs.
  const dx = state === "idle" ? Math.sin(t * 0.23) * 1.5 : 0;
  const dy = state === "working" ? Math.sin(t * rate + 1.2) * 1.2 : 0;
  // Blink: two incommensurate periods so it never reads as a loop.
  const lid = Math.max(0, Math.sin((t / BLINK_EVERY) * TAU + Math.sin(t * 0.37) * 0.6));
  const blink = lid > 0.985 ? 1 - (lid - 0.985) / 0.015 : 0;
  // Gaze: the state's own aim, plus the pointer's, plus idle wander.
  const wander = state === "idle" ? { x: Math.sin(t * 0.31) * 0.35 + Math.sin(t * 0.73) * 0.2, y: Math.sin(t * 0.41 + 1) * 0.18 } : { x: 0, y: 0 };
  const aim = state === "needs" ? { x: 0.55, y: -0.5 } : { x: 0, y: 0 };
  const ex = (aim.x + wander.x + look.x * 0.5) * EYE.gaze;
  const ey = (aim.y + wander.y + look.y * 0.5) * EYE.gaze;
  const eh = EYE.h * (state === "working" ? 0.82 : 1) * (1 - blink) * (1 - cheer * 0.7);
  const eyes = [-1, 1].map((s) => ({
    x: s * EYE.split + ex,
    y: ey,
    w: EYE.w,
    h: Math.max(eh, 0.6),
    alpha: 1 - cheer,
  }));
  return { sx, sy, dx, dy, eyes };
}

/** A stadium centred on the origin: the exact outline of an eye. */
function capsulePath(w: number, h: number): string {
  const hw = Math.max(w, 0.01) / 2;
  const hh = Math.max(h, 0.01) / 2;
  const r = Math.min(hw, hh);
  return (
    `M${r2(-hw)} ${r2(-hh + r)}`
    + `A${r2(r)} ${r2(r)} 0 0 1 ${r2(-hw + r)} ${r2(-hh)}`
    + `L${r2(hw - r)} ${r2(-hh)}`
    + `A${r2(r)} ${r2(r)} 0 0 1 ${r2(hw)} ${r2(-hh + r)}`
    + `L${r2(hw)} ${r2(hh - r)}`
    + `A${r2(r)} ${r2(r)} 0 0 1 ${r2(hw - r)} ${r2(hh)}`
    + `L${r2(-hw + r)} ${r2(hh)}`
    + `A${r2(r)} ${r2(r)} 0 0 1 ${r2(-hw)} ${r2(hh - r)}Z`
  );
}

/** FNV-1a: a stable seed from the bot's id. */
function hash(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

/** One painted frame, the DOM refs a tick writes into. */
type Face = {
  el: HTMLElement | null;
  /** The group the breath/wander transform lands on. */
  move: SVGGElement | null;
  eyes: (SVGPathElement | null)[];
  cheer: SVGGElement | null;
  notif: SVGCircleElement | null;
};

// One pointer for every face, read inside each face's own frame.
const pointer = { x: 0, y: 0 };
if (typeof window !== "undefined") {
  window.addEventListener(
    "pointermove",
    (e) => {
      pointer.x = e.clientX;
      pointer.y = e.clientY;
    },
    { passive: true },
  );
}

const reduceMotion = () => typeof window !== "undefined" && !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

/** The clock every face shares: one rAF for the whole app, so a sidebar of
 *  faces costs one loop. Rects are read in a first pass (no writes between the
 *  reads, so it is one layout at most) and painted in a second. */
const faces = new Set<{ read(): DOMRect | null; draw(t: number, rect: DOMRect | null): void }>();
let clock = 0;
function tick(tms: number) {
  const entries = [...faces];
  const rects = entries.map((f) => f.read());
  const t = tms / 1000;
  for (let i = 0; i < entries.length; i++) entries[i].draw(t, rects[i]);
  clock = entries.length ? requestAnimationFrame(tick) : 0;
}
const joinClock = (f: { read(): DOMRect | null; draw(t: number, rect: DOMRect | null): void }) => {
  faces.add(f);
  if (!clock && !reduceMotion()) clock = requestAnimationFrame(tick);
  return () => {
    faces.delete(f);
    if (!faces.size && clock) {
      cancelAnimationFrame(clock);
      clock = 0;
    }
  };
};

export function Orb({ agent, size = 32, state = "idle" }: { agent?: Agent; size?: number; state?: OrbState }) {
  // The seed only phases the blink: every bot has a face whichever way it was
  // made, and a row of them never blinks in unison.
  const seed = agent?.avatar.emoji || agent?.id || agent?.name || "orb";
  const color = agent?.avatar.color || "#64748b";
  const phase = useMemo(() => ((hash(seed) % 1000) / 1000) * BLINK_EVERY, [seed]);
  // The folds are the brain's own colour, lightened.
  const foldTone = useMemo(() => lighten(color, 0.5), [color]);
  const foldPaths = useMemo(() => folds(), []);
  const face = useRef<Face>({ el: null, move: null, eyes: [null, null], cheer: null, notif: null });
  const hover = useRef(false);
  const cheer = useRef(0);
  // Live values the clock reads: the component may re-render (a new state, a
  // new colour) without restarting the loop.
  const live = useRef({ state, phase });
  live.current = { state, phase };

  // Stable for the clock's lifetime: everything it closes over is a ref, so a
  // re-render never re-subscribes the face.
  const draw = useMemo(
    () => (t: number, rect: DOMRect | null) => {
      const f = face.current;
      if (!f.move) return;
      const st = live.current.state;
      // The gaze aims from THIS face at the pointer, so a bot in the corner
      // looks where it should. A zero-size box (display:none parent) gives 0/0:
      // keep the last aim.
      let look = { x: 0, y: 0 };
      if (rect && rect.width > 0) {
        const dx = pointer.x - (rect.left + rect.width / 2);
        const dy = pointer.y - (rect.top + rect.height / 2);
        const m = Math.hypot(dx, dy);
        if (m > 0 && Number.isFinite(dx) && Number.isFinite(dy)) look = { x: dx / m, y: dy / m };
      }
      // The cheer eases in the clock, not in React state: a hover costs no
      // re-render, only a slow drift of one number.
      const to = hover.current ? 1 : 0;
      cheer.current = Math.abs(cheer.current - to) < 0.004 ? to : cheer.current + (to - cheer.current) * 0.12;
      const g = frame(st, t + live.current.phase, look, cheer.current);
      f.move.setAttribute("transform", `translate(${r2(g.dx)} ${r2(g.dy)}) scale(${r2(g.sx)} ${r2(g.sy)})`);
      for (let i = 0; i < 2; i++) {
        const e = g.eyes[i];
        const el = f.eyes[i];
        if (!el) continue;
        el.setAttribute("d", capsulePath(e.w, e.h));
        el.setAttribute("transform", `translate(${r2(e.x)} ${r2(e.y)})`);
        el.setAttribute("opacity", String(e.alpha));
      }
      f.cheer?.setAttribute("opacity", String(cheer.current));
      if (f.notif) f.notif.setAttribute("opacity", st === "needs" ? "1" : "0");
    },
    [],
  );

  // The first paint is a still frame at t=0: the clock takes over on the next
  // frame, and a reduced-motion visitor keeps exactly this image.
  const still = useMemo(() => frame(state, phase + 0.42, { x: 0, y: 0 }, 0), [state, phase]);

  useEffect(
    () =>
      joinClock({
        read: () => face.current.el?.getBoundingClientRect() ?? null,
        draw,
      }),
    [draw],
  );

  const arm = () => {
    hover.current = true;
    // The clock is what eases the cheer; this only holds the pose once it is
    // fully in (CHEER_AT).
    if (cheer.current >= 0.999) return;
    window.setTimeout(() => {
      if (hover.current) cheer.current = 1;
    }, CHEER_AT * 1000);
  };

  return (
    <span
      ref={(n) => {
        face.current.el = n;
      }}
      className="relative inline-grid shrink-0 place-items-center"
      style={{ width: size, height: size }}
      onPointerEnter={arm}
      onPointerLeave={() => {
        hover.current = false;
      }}
      aria-hidden
    >
      <svg viewBox="-50 -50 100 100" width="100%" height="100%" aria-hidden focusable="false">
        <g
          ref={(n) => {
            face.current.move = n;
          }}
          transform={`translate(${r2(still.dx)} ${r2(still.dy)}) scale(${r2(still.sx)} ${r2(still.sy)})`}
        >
          {/* The brain: filled in the bot's colour, the folds cut into it. */}
          <path d={SILHOUETTE} fill={color} />
          <g fill="none" stroke={foldTone} strokeWidth="2.5" strokeLinecap="round">
            {foldPaths.map((d) => (
              <path key={d} d={d} />
            ))}
          </g>
          {still.eyes.map((e, i) => (
            <path
              key={i}
              ref={(n) => {
                face.current.eyes[i] = n;
              }}
              d={capsulePath(e.w, e.h)}
              fill="#fff"
              transform={`translate(${r2(e.x)} ${r2(e.y)})`}
            />
          ))}
          {/* The cheer: the eyes close up into two smiles, gliding with `cheer`. */}
          <g
            ref={(n) => {
              face.current.cheer = n;
            }}
            stroke="#fff"
            strokeWidth="3.6"
            strokeLinecap="round"
            fill="none"
            opacity={0}
          >
            <path d={`M${r2(-13 + 3)} 7 q${r2(4.5 + 1.5)} ${r2(-6.5 - 1.5)} ${r2(9 + 3)} 0`} />
            <path d={`M${r2(4 - 3)} 7 q${r2(4.5 + 1.5)} ${r2(-6.5 - 1.5)} ${r2(9 + 3)} 0`} />
          </g>
        </g>
        <circle
          ref={(n) => {
            face.current.notif = n;
          }}
          cx={NOTIF.x}
          cy={NOTIF.y}
          r={NOTIF.r}
          fill="var(--orb-notif, #0ea5e9)"
          opacity={state === "needs" ? 1 : 0}
        />
      </svg>
    </span>
  );
}

/** The face a group shows: up to three member faces, overlapping, each clipped
 *  to its own circle so the cluster reads as a tidy row of circular avatars
 *  rather than a pile of brains. */
export function Stack({ ids, size = 22 }: { ids: string[]; size?: number }) {
  const shown = ids.slice(0, 3);
  const step = Math.round(size * 0.32);
  return (
    <span
      className="relative inline-grid shrink-0 place-items-center"
      style={{ width: size + (shown.length - 1) * step, height: size }}
      aria-hidden
    >
      {shown.map((id, i) => (
        <span
          key={id}
          className="absolute overflow-hidden rounded-full ring-2 ring-(--surface)"
          style={{ width: size, height: size, left: i * step, zIndex: shown.length - i }}
        >
          <Orb agent={agentById(id)} size={size} />
        </span>
      ))}
      {ids.length > shown.length && (
        <span
          className="absolute grid place-items-center rounded-full bg-well text-[9px]/3 font-semibold text-muted-foreground ring-2 ring-(--surface)"
          style={{ width: size * 0.62, height: size * 0.62, left: size + (shown.length - 1) * step - size * 0.5, zIndex: shown.length + 1 }}
        >
          +{ids.length - shown.length}
        </span>
      )}
    </span>
  );
}

/*
 * Attribution:
 *   Silhouette — generated here from a union of discs; the lobe layout and the
 *   folds derived from it are this file's own. The animation posture (shared
 *   clock, breath/blink/gaze expressions, capsule eyes, the 1.2s hover cheer) is
 *   after orbs (nikuscs/orbs, MIT), packages/avatar-blobs by Jérémy Perret,
 *   simplified to four states.
 */
