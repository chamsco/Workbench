// The face: a bot's brain.
//
// The design is the owner's mascot: a flat, lumpy brain in two tones — the
// bot's colour for the body, the same hue darker for the folds — with two
// white pill eyes and a small smile. The geometry is drawn in the mascot's own
// art-board units (about 1180 x 760) and scaled into the -50..50 viewBox.
//
// The motion follows the pointer on two separate axes, on purpose:
//   - looking UP tilts the BODY back on its base, the crown swinging toward the
//     pointer's side; looking down leans it forward;
//   - left/right on its own only leans the FACE: eyes and smile slide, tip and
//     slant toward the pointer while the body stays upright.
// Both ride soft springs so the head has weight; the face follows the body a
// beat later.
//
// Everything rides one shared rAF clock for every face on screen (rects read
// in one pass, DOM written through refs, so a frame never touches React
// state). The blink phase is seeded by the bot, so a row never blinks in
// unison, and a 1.2s hover closes the eyes into two smiles.
//
// Accessories (`avatar.accessory`) ride the body, so a hat leans back with it.

import { useEffect, useMemo, useRef } from "react";

import { agentById, type Agent } from "../bridge";

// ---------------------------------------------------------------- the brain

/** Art-board centre and scale into the -50..50 viewBox (with room for hats). */
const CX = 768;
const CY = 540;
const K = 0.07;
/** Where the body pivots when it pitches: the base, like a head on a neck. */
const BASE_Y = 860;
/** ...and where it pivots when it tips forward: the crown. */
const TOP_Y = 180;

const LOBES: [number, number, number][] = [
  [640, 330, 170], [895, 330, 170],
  [430, 440, 170], [1105, 440, 170],
  [360, 615, 150], [1180, 615, 150],
  [525, 705, 145], [1015, 705, 145],
  [690, 745, 110], [845, 745, 110],
];

const FOLDS = [
  "M767 245 V462",
  "M582 312 C650 278 718 285 767 380 C816 285 884 278 950 312",
  "M412 462 C440 395 490 377 541 377",
  "M1122 462 C1094 395 1044 377 993 377",
  "M362 502 C330 558 330 600 362 638",
  "M1162 502 C1194 558 1194 600 1162 638",
];

const EYE = { w: 98, h: 192, lx: 594, rx: 939, y: 576 };
const MOUTH = "M720 642 C745 675 790 675 815 645";

export const ACCESSORIES = ["", "glasses", "tophat", "beanie", "bow", "crown", "headphones"] as const;

/** The fold tone: the same hue, darker. */
export function darken(hex: string, amt = 0.2): string {
  const n = Number.parseInt(hex.slice(1), 16);
  if (hex[0] !== "#" || !Number.isFinite(n)) return hex;
  const m = (c: number) => Math.round(c * (1 - amt));
  return `rgb(${m((n >> 16) & 255)},${m((n >> 8) & 255)},${m(n & 255)})`;
}

/** Drawn in art-board units. Body-worn pieces go in `Body`, face-worn in `Face`. */
function Accessory({ kind, tone, on }: { kind: string; tone: string; on: "body" | "face" }) {
  const ink = "#1f2937";
  if (on === "face") {
    if (kind !== "glasses") return null;
    return (
      <g fill="none" stroke={ink} strokeWidth="26" strokeLinejoin="round">
        <rect x="500" y="470" width="190" height="200" rx="60" fill="rgba(255,255,255,.18)" />
        <rect x="845" y="470" width="190" height="200" rx="60" fill="rgba(255,255,255,.18)" />
        <path d="M690 540 Q767 505 845 540" />
      </g>
    );
  }
  switch (kind) {
    case "tophat":
      return (
        <g fill={ink}>
          <rect x="560" y="70" width="330" height="230" rx="24" transform="rotate(-6 725 185)" />
          <rect x="470" y="270" width="520" height="58" rx="29" transform="rotate(-6 730 300)" />
          <rect x="565" y="220" width="330" height="40" fill={tone} transform="rotate(-6 725 240)" />
        </g>
      );
    case "beanie":
      return (
        <g>
          <path d="M455 330 C470 120 1065 120 1080 330 Z" fill="#facc15" />
          <rect x="430" y="300" width="675" height="85" rx="42" fill="#eab308" />
          <circle cx="768" cy="140" r="52" fill="#fde047" />
        </g>
      );
    case "bow":
      return (
        <g fill="#f472b6" transform="rotate(14 1010 230)">
          <path d="M1010 230 L880 150 Q850 230 880 310 Z" />
          <path d="M1010 230 L1140 150 Q1170 230 1140 310 Z" />
          <circle cx="1010" cy="230" r="42" fill="#ec4899" />
        </g>
      );
    case "crown":
      return (
        <g fill="#fbbf24" stroke="#f59e0b" strokeWidth="14" strokeLinejoin="round">
          <path d="M570 290 L585 120 L680 210 L768 90 L856 210 L951 120 L966 290 Z" />
          <circle cx="768" cy="235" r="26" fill="#ef4444" stroke="none" />
        </g>
      );
    case "headphones":
      return (
        <g>
          <path d="M290 560 C290 140 1246 140 1246 560" fill="none" stroke={ink} strokeWidth="44" strokeLinecap="round" />
          <rect x="230" y="480" width="120" height="210" rx="56" fill={ink} />
          <rect x="1186" y="480" width="120" height="210" rx="56" fill={ink} />
        </g>
      );
    default:
      return null;
  }
}

// ---------------------------------------------------------------- the face

export type OrbState = "idle" | "working" | "needs" | "wake";

const BLINK_EVERY = 4.6;
const CHEER_AT = 1.2;
const NOTIF = { x: 40, y: -36, r: 5.2 };

function hash(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}
const r2 = (n: number) => Math.round(n * 100) / 100;

type Refs = {
  el: HTMLElement | null;
  body: SVGGElement | null;
  face: SVGGElement | null;
  eyes: SVGGElement | null;
  cheer: SVGGElement | null;
  notif: SVGCircleElement | null;
};

// One pointer for every face.
const pointer = { x: Number.NaN, y: Number.NaN };
if (typeof window !== "undefined") {
  window.addEventListener("pointermove", (e) => { pointer.x = e.clientX; pointer.y = e.clientY; }, { passive: true });
  document.addEventListener("pointerleave", () => { pointer.x = Number.NaN; pointer.y = Number.NaN; });
}

const reduceMotion = () => typeof window !== "undefined" && !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

type Clocked = { read(): DOMRect | null; draw(t: number, dt: number, rect: DOMRect | null): void };
const faces = new Set<Clocked>();
let clock = 0;
let lastT = 0;
function tick(tms: number) {
  const entries = [...faces];
  const rects = entries.map((f) => f.read());
  const t = tms / 1000;
  const dt = Math.min(lastT ? t - lastT : 0.016, 0.05);
  lastT = t;
  for (let i = 0; i < entries.length; i++) entries[i].draw(t, dt, rects[i]);
  clock = entries.length ? requestAnimationFrame(tick) : 0;
}
const joinClock = (f: Clocked) => {
  faces.add(f);
  if (!clock && !reduceMotion()) clock = requestAnimationFrame(tick);
  return () => {
    faces.delete(f);
    if (!faces.size && clock) {
      cancelAnimationFrame(clock);
      clock = 0;
      lastT = 0;
    }
  };
};

export function Orb({ agent, size = 32, state = "idle" }: { agent?: Agent; size?: number; state?: OrbState }) {
  const seed = agent?.avatar.emoji || agent?.id || agent?.name || "orb";
  const color = agent?.avatar.color || "#64748b";
  const accessory = agent?.avatar.accessory || "";
  const fold = useMemo(() => darken(color), [color]);
  const phase = useMemo(() => ((hash(seed) % 1000) / 1000) * BLINK_EVERY, [seed]);
  const refs = useRef<Refs>({ el: null, body: null, face: null, eyes: null, cheer: null, notif: null });
  const hover = useRef(false);
  const live = useRef({ state, phase });
  live.current = { state, phase };
  // Spring state: the body pitches (y), the face leans (x and y).
  const sim = useRef({ bx: 0, by: 0, vx: 0, vy: 0, lean: 0, vl: 0, fx: 0, fy: 0, vfx: 0, vfy: 0, cheer: 0 });

  const draw = useMemo(
    () => (t: number, dt: number, rect: DOMRect | null) => {
      const f = refs.current;
      if (!f.body || !f.face) return;
      const s = sim.current;
      const { state: st, phase: ph } = live.current;

      // Where the pointer is, from THIS face, -1..1 with a soft falloff.
      let tx = 0;
      let ty = 0;
      if (rect && rect.width > 0 && Number.isFinite(pointer.x)) {
        // Full lean a short way off the face, so it reads at any icon size.
        const reach = 140 + rect.width * 0.8;
        tx = Math.tanh((pointer.x - (rect.left + rect.width / 2)) / reach);
        ty = Math.tanh((pointer.y - (rect.top + rect.height / 2)) / reach);
      }
      if (st === "needs") { tx = 0.5; ty = -0.6; } // looks up at the badge

      // Soft springs (weight, a touch of overshoot, a little lag behind the
      // pointer). The body leads and the face follows a beat later on a
      // softer one. The lean gets its own spring on its own target: derived
      // from the lagging x and y it would be their product, and ramp in late.
      const spring = (x: number, v: number, to: number, stiff: number, damp: number) => {
        const nv = v + ((to - x) * stiff - v * damp) * dt;
        return [x + nv * dt, nv];
      };
      [s.bx, s.vx] = spring(s.bx, s.vx, tx, 44, 11.25);
      [s.by, s.vy] = spring(s.by, s.vy, ty, 44, 11.25);
      [s.lean, s.vl] = spring(s.lean, s.vl, (Math.max(0, -ty) - Math.max(0, ty)) * tx, 44, 11.25);
      [s.fx, s.vfx] = spring(s.fx, s.vfx, tx, 25, 8.75);
      [s.fy, s.vfy] = spring(s.fy, s.vfy, ty, 25, 8.75);

      // Breath and working bob.
      const rate = st === "working" ? 2.1 : 1.05;
      const breath = Math.sin((t + ph) * rate) * (st === "working" ? 0.04 : 0.015);
      const bob = st === "working" ? Math.sin((t + ph) * rate + 1.2) * 14 : 0;

      // Body: tilts only off the level line, and the two halves mirror each
      // other. Looking up, it tips back on its base with the crown swinging
      // toward the pointer's side; looking down, it tips forward from the
      // crown with the base swinging toward it. Both squash a touch, as the
      // receding half foreshortens. Pure left/right leaves the body upright —
      // that axis is the face's.
      const up = Math.max(0, -s.by);
      const down = Math.max(0, s.by);
      const lean = s.lean * 22;
      const pivot = up >= down ? BASE_Y : TOP_Y;
      const sy = 1 - (up + down) * 0.06 + breath;
      const sx = 1 + (up + down) * 0.02 - breath * 0.5;
      f.body.setAttribute(
        "transform",
        `translate(${CX} ${pivot + bob}) rotate(${r2(lean)}) scale(${r2(sx)} ${r2(sy)}) translate(${-CX} ${-pivot})`,
      );

      // Face: slides toward the pointer and leans that way — tipped and
      // slanted, so the eyes read as turning, not just sliding. Below the
      // level line the tip and slant mirror, like the body's.
      const mirror = 1 - 2 * Math.min(1, Math.max(0, s.fy) * 3);
      const fx = s.fx * 110;
      const fy = s.fy * 85;
      f.face.setAttribute(
        "transform",
        `translate(${r2(fx + CX)} ${r2(fy + 580)}) rotate(${r2(s.fx * 8 * mirror)}) skewX(${r2(s.fx * 14 * mirror)}) translate(${-CX} -580)`,
      );

      // Blink: two incommensurate periods so it never reads as a loop.
      const lid = Math.max(0, Math.sin(((t + ph) / BLINK_EVERY) * Math.PI * 2 + Math.sin(t * 0.37) * 0.6));
      const blink = lid > 0.985 ? 1 - (lid - 0.985) / 0.015 : 0;
      const to = hover.current ? 1 : 0;
      s.cheer = Math.abs(s.cheer - to) < 0.004 ? to : s.cheer + (to - s.cheer) * 0.12;
      const open = Math.max((1 - blink) * (1 - s.cheer) * (st === "working" ? 0.82 : 1), 0.05);
      f.eyes?.setAttribute("transform", `translate(0 ${EYE.y}) scale(1 ${r2(open)}) translate(0 ${-EYE.y})`);
      f.cheer?.setAttribute("opacity", String(r2(s.cheer)));
      f.notif?.setAttribute("opacity", st === "needs" ? "1" : "0");
    },
    [],
  );

  useEffect(() => joinClock({ read: () => refs.current.el?.getBoundingClientRect() ?? null, draw }), [draw]);

  const arm = () => {
    hover.current = true;
    window.setTimeout(() => { if (hover.current) sim.current.cheer = 1; }, CHEER_AT * 1000);
  };

  return (
    <span
      ref={(n) => { refs.current.el = n; }}
      className="relative inline-grid shrink-0 place-items-center"
      style={{ width: size, height: size }}
      onPointerEnter={arm}
      onPointerLeave={() => { hover.current = false; }}
      aria-hidden
    >
      <svg viewBox="-50 -50 100 100" width="100%" height="100%" overflow="visible" aria-hidden focusable="false">
        <g transform={`scale(${K}) translate(${-CX} ${-CY})`}>
          <g ref={(n) => { refs.current.body = n; }}>
            <g fill={color}>
              <ellipse cx="768" cy="520" rx="430" ry="270" />
              {LOBES.map(([x, y, r]) => <circle key={`${x}-${y}`} cx={x} cy={y} r={r} />)}
            </g>
            <g fill="none" stroke={fold} strokeWidth="32" strokeLinecap="round" strokeLinejoin="round">
              {FOLDS.map((d) => <path key={d} d={d} />)}
            </g>
            <g ref={(n) => { refs.current.face = n; }}>
              <g ref={(n) => { refs.current.eyes = n; }} fill="#fff">
                <rect x={EYE.lx - EYE.w / 2} y={EYE.y - EYE.h / 2} width={EYE.w} height={EYE.h} rx={EYE.w / 2} />
                <rect x={EYE.rx - EYE.w / 2} y={EYE.y - EYE.h / 2} width={EYE.w} height={EYE.h} rx={EYE.w / 2} />
              </g>
              <g ref={(n) => { refs.current.cheer = n; }} fill="none" stroke="#fff" strokeWidth="34" strokeLinecap="round" opacity={0}>
                <path d={`M${EYE.lx - 60} 600 Q${EYE.lx} 520 ${EYE.lx + 60} 600`} />
                <path d={`M${EYE.rx - 60} 600 Q${EYE.rx} 520 ${EYE.rx + 60} 600`} />
              </g>
              <path d={MOUTH} fill="none" stroke="#fff" strokeWidth="30" strokeLinecap="round" />
              <Accessory kind={accessory} tone={fold} on="face" />
            </g>
            <Accessory kind={accessory} tone={fold} on="body" />
          </g>
        </g>
        <circle
          ref={(n) => { refs.current.notif = n; }}
          cx={NOTIF.x} cy={NOTIF.y} r={NOTIF.r}
          fill="var(--orb-notif, #0ea5e9)"
          opacity={state === "needs" ? 1 : 0}
        />
      </svg>
    </span>
  );
}

/** A group's face: up to three member faces, overlapping, each clipped to a
 *  circle so the cluster reads as a tidy row. */
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
