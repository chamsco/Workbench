// The face: a bot's brain.
//
// The design is the owner's "Brain Mascot": a pink brain built from lobes (a
// ring of discs around one wide ellipse), its folds drawn on as thick round
// strokes a shade deeper than the body, two tall white pill eyes and a small
// smile. The geometry below is that drawing's own coordinates (about 1100 by
// 700 units); the SVG scales it down into a 100-unit box.
//
// It looks at the pointer the way the mascot does: the head leans into the
// turn, the folds trail the face, the face slides further than the head, and
// the eyes squash and tilt as they turn. It blinks on its own, now and then
// twice. All of that runs on one shared rAF clock for every face on screen
// (rects read in a single pass, DOM written through refs, so a frame never
// touches React state). `frame()` stays pure in t, so a still frame is the
// same image every time.
//
// States add to that: working breathes faster and bobs, needs looks up at its
// badge, a 1.2s hover closes the eyes into two smiles, and voice turns the
// eyes into an equalizer and sways the whole head.
//
// The bot's colour is `avatar.color` (the mascot's pink by default); the folds
// are that colour a step darker. The stored emoji draws nothing; it only phases
// the blink, so a row of brains never blinks in unison.

import { useEffect, useMemo, useRef } from "react";

import { agentById, type Agent } from "../bridge";

// ---------------------------------------------------------------- the brain

const TAU = Math.PI * 2;
const r2 = (n: number) => Math.round(n * 100) / 100;

/** The mascot's lobes, in its own units. */
const LOBES: [number, number, number][] = [
  [640, 342, 195], [895, 342, 195],
  [445, 445, 180], [1092, 445, 180],
  [362, 610, 146], [1172, 610, 146],
  [520, 682, 160], [686, 692, 150],
  [850, 692, 150], [1012, 682, 160],
];
const CORE = { cx: 768, cy: 520, rx: 410, ry: 270 };
const FOLDS = [
  "M768 218V458",
  "M568 290C650 245 730 290 768 380",
  "M968 290C886 245 806 290 768 380",
  "M530 355C460 360 415 410 402 453",
  "M1003 355C1073 360 1118 410 1131 453",
  "M358 498C335 540 335 600 362 640",
  "M1176 498C1200 540 1200 600 1172 632",
];
const SMILE = "M720 645Q768 700 815 645";
/** The head's centre (its bounding box), the smile's centre and the eyes'. */
const HEAD = { x: 767, y: 495 };
const SMILE_C = { x: 767.5, y: 659 };
const EYES = [
  { x: 594, y: 575 },
  { x: 939, y: 575 },
];
const EYE = { w: 98, h: 194 };
/** Mascot units to the 100-unit box: the head is ~1100 wide. */
const FIT = 0.086;

/** The folds: the body's colour a step darker (#FF7F95 → #FF5C7A in the
 *  drawing is seven points of lightness). */
function deeper(hex: string, by = 0.07): string {
  const n = Number.parseInt(hex.slice(1), 16);
  if (hex.length !== 7 || !Number.isFinite(n)) return hex;
  let r = ((n >> 16) & 255) / 255;
  let g = ((n >> 8) & 255) / 255;
  let b = (n & 255) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  const d = max - min;
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
  let h = 0;
  if (d) {
    if (max === r) h = ((g - b) / d) % 6;
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
  }
  h = (h * 60 + 360) % 360;
  const L = Math.max(0, l - by);
  const c = (1 - Math.abs(2 * L - 1)) * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = L - c / 2;
  [r, g, b] = h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x] : h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
  const to = (v: number) => Math.round((v + m) * 255);
  return `rgb(${to(r)},${to(g)},${to(b)})`;
}

// ---------------------------------------------------------------- the face

export type OrbState = "idle" | "working" | "needs" | "wake" | "voice";

/** How loud the voice is right now, 0..1: voice mode writes it (the mic while
 *  you talk, the reply's words while it speaks), every face in "voice" reads
 *  it. A negative value means nothing is feeding it, and the face talks on its
 *  own rhythm. */
export const voiceLevel = { v: -1 };

/** The badge, parked off the top-right lobe, in the 100-unit box. */
const NOTIF = { x: 40, y: -30, r: 5.2 };
/** Blinks come every 2.2 to 5.8 s, as in the mascot; this is the mean. */
const BLINK_EVERY = 4;
/** Hover this long and the face cheers. */
const CHEER_AT = 1.2;

type Eye = { x: number; y: number; w: number; h: number; rot: number; alpha: number };
type Pose = { head: string; folds: string; face: string; smile: string; smileAlpha: number; eyes: Eye[] };

/** Blink, 0..1: a 180ms close-and-open, at uneven intervals, sometimes twice.
 *  Pure in t: the interval for each beat is a hash of its index. */
function blinkAt(t: number): number {
  const beat = Math.floor(t / BLINK_EVERY);
  const jit = (k: number) => {
    const s = Math.sin(k * 127.1 + 311.7) * 43758.5453;
    return s - Math.floor(s);
  };
  const at = beat * BLINK_EVERY + jit(beat) * (BLINK_EVERY - 0.6);
  const one = (t0: number) => {
    const u = (t - t0) / 0.18;
    return u >= 0 && u < 1 ? Math.sin(u * Math.PI) : 0;
  };
  return Math.max(one(at), jit(beat + 0.5) < 0.2 ? one(at + 0.33) : 0);
}

/** One frame of the face, pure in `t` (seconds), `look` (-1..1 toward the
 *  pointer, already eased) and `cheer` (0..1, the hover smile). */
function frame(state: OrbState, t: number, look: { x: number; y: number }, cheer: number): Pose {
  // The state's own aim, plus idle wander, on top of the pointer.
  const wander = state === "idle" ? { x: Math.sin(t * 0.31) * 0.12 + Math.sin(t * 0.73) * 0.06, y: Math.sin(t * 0.41 + 1) * 0.06 } : { x: 0, y: 0 };
  const aim = state === "needs" ? { x: 0.45, y: -0.4 } : { x: 0, y: 0 };
  const x = Math.max(-1, Math.min(1, look.x + wander.x + aim.x));
  const y = Math.max(-1, Math.min(1, look.y + wander.y + aim.y));
  const down = Math.max(y, 0);
  const up = Math.max(-y, 0);
  // Breath and bob, in mascot units.
  const rate = state === "working" || state === "voice" ? 2.4 : 1.05;
  const depth = state === "working" ? 0.04 : state === "voice" ? 0.03 : 0.015;
  const breath = Math.sin(t * rate);
  const fed = voiceLevel.v >= 0;
  const lv = fed ? Math.min(1, voiceLevel.v) : 0.45 + 0.55 * Math.abs(Math.sin(t * 6.1) * Math.sin(t * 2.3 + 0.7));
  const bob = state === "working" ? Math.sin(t * rate + 1.2) * 14 : state === "voice" ? -Math.abs(Math.sin(t * 2.6)) * 36 * (0.4 + lv * 0.6) : 0;
  const sway = state === "voice" ? Math.sin(t * 1.7) * 4 : 0;
  const swayX = state === "voice" ? Math.sin(t * 1.3) * 24 : 0;
  // The mascot's look, verbatim: lean into the turn, tuck when looking down.
  const head =
    `translate(${r2(HEAD.x + x * 16 + swayX)} ${r2(HEAD.y + y * 10 + bob)}) rotate(${r2(x * 5 + x * down * 2 + sway)})`
    + ` scale(${r2((1 - Math.abs(x) * 0.015) * (1 - depth * 0.6 * breath))} ${r2((1 - down * 0.04) * (1 + depth * breath))}) translate(${-HEAD.x} ${-HEAD.y})`;
  const folds = `translate(${r2(x * 44)} ${r2(y * 30)}) skewX(${r2(-x * 3)}) scale(1 ${r2(1 + down * 0.07 - up * 0.05)})`;
  const face = `translate(${r2(x * 82)} ${r2(y * 62)})`;
  const smile =
    `translate(${r2(SMILE_C.x + x * 16)} ${r2(SMILE_C.y - down * 8)}) rotate(${r2(-x * 16)})`
    + ` scale(${r2(1 - Math.abs(x) * 0.08 + down * 0.08)} ${r2(1 - down * 0.1)}) translate(${-SMILE_C.x} ${-SMILE_C.y})`;

  if (state === "voice") {
    // The eyes become an equalizer: a dot, a tall bar, a dot.
    const wob = (k: number) => 0.5 + 0.5 * Math.sin(t * (9 + k * 2.7) + k * 1.9);
    const cx = (EYES[0].x + EYES[1].x) / 2;
    const cy = EYES[0].y;
    const w = 112;
    const eyes = [
      { x: cx - 190, y: cy, w, h: w + lv * 110 * wob(1), rot: 0, alpha: 1 },
      { x: cx + 190, y: cy, w, h: w + lv * 110 * wob(2), rot: 0, alpha: 1 },
      { x: cx, y: cy - 10, w, h: 170 + lv * 200 * (0.6 + 0.4 * wob(3)), rot: 0, alpha: 1 },
    ];
    return { head, folds, face, smile, smileAlpha: 0, eyes };
  }

  const b = blinkAt(t);
  const squash = 1 - Math.abs(x) * 0.1 + down * 0.04;
  const tall = (state === "working" ? 0.82 : 1) * (1 - b * 0.92) * (1 - down * 0.12) * (1 - cheer * 0.85);
  const eyes = EYES.map((e) => {
    const side = e.x < HEAD.x ? 1 : -1; // eyes drift together when looking down
    return { x: e.x + side * down * 10, y: e.y, w: EYE.w * squash, h: Math.max(EYE.h * tall, 6), rot: -x * 22, alpha: 1 - cheer };
  });
  // The third eye is voice mode's middle bar; elsewhere it is not drawn.
  eyes.push({ x: HEAD.x, y: EYES[0].y, w: EYE.w, h: EYE.w, rot: 0, alpha: 0 });
  return { head, folds, face, smile, smileAlpha: 1, eyes };
}

const eyeTransform = (e: Eye) => `translate(${r2(e.x)} ${r2(e.y)}) rotate(${r2(e.rot)})`;

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
  head: SVGGElement | null;
  folds: SVGGElement | null;
  face: SVGGElement | null;
  smile: SVGPathElement | null;
  eyes: (SVGPathElement | null)[];
  cheer: SVGGElement | null;
  notif: SVGCircleElement | null;
};

// One pointer for every face, read inside each face's own frame. Until it has
// moved, or once it leaves the window, every face looks ahead.
const pointer = { x: 0, y: 0, in: false };
if (typeof window !== "undefined") {
  window.addEventListener(
    "pointermove",
    (e) => {
      pointer.x = e.clientX;
      pointer.y = e.clientY;
      pointer.in = true;
    },
    { passive: true },
  );
  document.addEventListener("pointerleave", () => {
    pointer.in = false;
  });
}
/** How far the pointer has to be for a full turn; tanh keeps a far one soft. */
const REACH = 420;

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
  const color = agent?.avatar.color || "#ff7f95";
  const phase = useMemo(() => ((hash(seed) % 1000) / 1000) * BLINK_EVERY * 7, [seed]);
  const foldTone = useMemo(() => deeper(color), [color]);
  const face = useRef<Face>({ el: null, head: null, folds: null, face: null, smile: null, eyes: [null, null, null], cheer: null, notif: null });
  const hover = useRef(false);
  const cheer = useRef(0);
  const look = useRef({ x: 0, y: 0 });
  // Live values the clock reads: the component may re-render (a new state, a
  // new colour) without restarting the loop.
  const live = useRef({ state, phase });
  live.current = { state, phase };

  // Stable for the clock's lifetime: everything it closes over is a ref, so a
  // re-render never re-subscribes the face.
  const draw = useMemo(
    () => (t: number, rect: DOMRect | null) => {
      const f = face.current;
      if (!f.head) return;
      const st = live.current.state;
      // The gaze aims from THIS face at the pointer, so a bot in the corner
      // looks where it should, and eases there as the mascot does. A zero-size
      // box (display:none parent) keeps the last aim.
      let to = { x: 0, y: 0 };
      if (pointer.in && rect && rect.width > 0) {
        to = { x: Math.tanh((pointer.x - (rect.left + rect.width / 2)) / REACH), y: Math.tanh((pointer.y - (rect.top + rect.height / 2)) / REACH) };
      }
      look.current = { x: look.current.x + (to.x - look.current.x) * 0.1, y: look.current.y + (to.y - look.current.y) * 0.1 };
      // The cheer eases in the clock, not in React state: a hover costs no
      // re-render, only a slow drift of one number.
      const c = hover.current ? 1 : 0;
      cheer.current = Math.abs(cheer.current - c) < 0.004 ? c : cheer.current + (c - cheer.current) * 0.12;
      const g = frame(st, t + live.current.phase, look.current, st === "voice" ? 0 : cheer.current);
      f.head.setAttribute("transform", g.head);
      f.folds?.setAttribute("transform", g.folds);
      f.face?.setAttribute("transform", g.face);
      f.smile?.setAttribute("transform", g.smile);
      f.smile?.setAttribute("opacity", String(g.smileAlpha));
      for (let i = 0; i < 3; i++) {
        const e = g.eyes[i];
        const el = f.eyes[i];
        if (!el) continue;
        el.setAttribute("d", capsulePath(e.w, e.h));
        el.setAttribute("transform", eyeTransform(e));
        el.setAttribute("opacity", String(e.alpha));
      }
      f.cheer?.setAttribute("opacity", String(st === "voice" ? 0 : cheer.current));
      if (f.notif) f.notif.setAttribute("opacity", st === "needs" ? "1" : "0");
    },
    [],
  );

  // The first paint is a still frame, eyes open: the clock takes over on the
  // next frame, and a reduced-motion visitor keeps exactly this image.
  const still = useMemo(() => frame(state, 1, { x: 0, y: 0 }, 0), [state]);

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
      <svg viewBox="-50 -50 100 100" width="100%" height="100%" aria-hidden focusable="false" overflow="visible">
        <g transform={`scale(${FIT}) translate(${-HEAD.x} ${-HEAD.y})`}>
          <g
            ref={(n) => {
              face.current.head = n;
            }}
            transform={still.head}
          >
            {/* The brain: lobes and a core in the bot's colour, folds a shade deeper. */}
            <g fill={color}>
              {LOBES.map(([cx, cy, r]) => (
                <circle key={`${cx}-${cy}`} cx={cx} cy={cy} r={r} />
              ))}
              <ellipse cx={CORE.cx} cy={CORE.cy} rx={CORE.rx} ry={CORE.ry} />
            </g>
            <g
              ref={(n) => {
                face.current.folds = n;
              }}
              transform={still.folds}
              fill="none"
              stroke={foldTone}
              strokeWidth="34"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              {FOLDS.map((d) => (
                <path key={d} d={d} />
              ))}
            </g>
            <g
              ref={(n) => {
                face.current.face = n;
              }}
              transform={still.face}
            >
              {still.eyes.map((e, i) => (
                <path
                  key={i}
                  ref={(n) => {
                    face.current.eyes[i] = n;
                  }}
                  d={capsulePath(e.w, e.h)}
                  fill="#fff"
                  opacity={e.alpha}
                  transform={eyeTransform(e)}
                />
              ))}
              <path
                ref={(n) => {
                  face.current.smile = n;
                }}
                d={SMILE}
                transform={still.smile}
                opacity={still.smileAlpha}
                fill="none"
                stroke="#fff"
                strokeWidth="28"
                strokeLinecap="round"
              />
              {/* The cheer: the eyes close up into two smiles, gliding with `cheer`. */}
              <g
                ref={(n) => {
                  face.current.cheer = n;
                }}
                stroke="#fff"
                strokeWidth="34"
                strokeLinecap="round"
                fill="none"
                opacity={0}
              >
                {EYES.map((e) => (
                  <path key={e.x} d={`M${e.x - 60} ${e.y + 10}q60 -70 120 0`} />
                ))}
              </g>
            </g>
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
 *   The brain, its look and its blink are the owner's "Brain Mascot" drawing
 *   and script. The shared clock and the hover cheer are after orbs
 *   (nikuscs/orbs, MIT), packages/avatar-blobs by Jérémy Perret.
 */
