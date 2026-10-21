// The face: a bot's brain.
//
// This used to be a radial-blob silhouette engine (ported from orbs'
// avatar-blobs). Two prototype passes proved a brain cannot come out of that
// machinery: its whole tuning — two binomial smoothing passes, a
// clamp(r, 0.88, 1.14) band, counterweighted disc pairs, the very fix that
// removed the "uneven lumps" — exists to suppress variation, and a brain needs
// the opposite: deliberate asymmetry plus interior detail a single-valued
// radial profile cannot hold. Both passes collapsed into circles.
//
// So the silhouette is now Tabler's `brain` icon (@tabler/icons-react v3.49.0,
// MIT — six stroked paths in a 24-box), which reads at 28, 32 and 56px alike.
// Everything AROUND the silhouette is unchanged and still does the work:
//
//   * one shared rAF clock for every face on screen (a single loop for the
//     whole sidebar; rects read in one batch, then painted; DOM written through
//     refs so a frame costs no React state);
//   * breath, a slow wander, a blink whose phase is the bot's own, and a gaze
//     that aims from THIS face at the pointer;
//   * hovering for 1.2s cheers it — the eyes close up into two smiles;
//   * `frame()` stays pure in t, so pause/resume/seek give the same image.
//
// The bot's colour still comes from `avatar.color`; the seed (emoji, else id)
// still phases the blink, so a row of brains never blinks in unison.

import { useEffect, useMemo, useRef } from "react";

import { agentById, type Agent } from "../bridge";

// ---------------------------------------------------------------- the brain

/** Tabler's `brain`, six paths in a 24-box. Stroked, as the icon draws them:
 *  filling them individually would fill the wrong regions — several are open
 *  lobe edges, not closed contours. */
const BRAIN = [
  "M15.5 13a3.5 3.5 0 0 0 -3.5 3.5v1a3.5 3.5 0 0 0 7 0v-1.8",
  "M8.5 13a3.5 3.5 0 0 1 3.5 3.5v1a3.5 3.5 0 0 1 -7 0v-1.8",
  "M17.5 16a3.5 3.5 0 0 0 0 -7h-.5",
  "M19 9.3v-2.8a3.5 3.5 0 0 0 -7 0",
  "M6.5 16a3.5 3.5 0 0 1 0 -7h.5",
  "M5 9.3v-2.8a3.5 3.5 0 0 1 7 0v10",
];
/** The 24-box into the 100-box the viewBox uses: centred on the origin. */
const BRAIN_FIT = "translate(-50 -50) scale(4.1667)";

// ---------------------------------------------------------------- the face

export type OrbState = "idle" | "working" | "needs" | "wake";

/** Measured in ball radii, off orbs' reference face. */
const EYE = { w: 0.236, h: 0.464, split: 0.62, gaze: 0.34, y: 0 };
/** The brain's safe radius for the gaze: a unit-ish disc, so the eyes never
 *  wander past the lobes. */
const FIT = 0.92;
/** The badge, parked off the brain's top-right lobe. */
const NOTIF = { x: 31, y: -29, r: 5.2 };
const BLINK_EVERY = 4.6;
/** Hover this long and the face cheers. */
const CHEER_AT = 1.2;
/** The viewBox is a 100-box centred on the ball; `R` is the ball's radius. */
const R = 40;

const TAU = Math.PI * 2;
const r2 = (n: number) => Math.round(n * 100) / 100;

/** One frame of the face, pure in `t` (seconds), `look` (-1..1 toward the
 *  pointer) and `cheer` (0..1, the hover smile). */
function frame(state: OrbState, t: number, look: { x: number; y: number }, cheer: number) {
  // Breath: quicker and deeper the busier the state.
  const rate = state === "working" ? 2.1 : 1.05;
  const depth = state === "working" ? 0.05 : 0.018;
  const breath = Math.sin(t * rate);
  // The silhouette is fixed, so the breath is a transform on the group rather
  // than a deformed profile: scale about the origin, which is its centre.
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
  const gx = aim.x + wander.x + look.x * 0.5;
  const gy = aim.y + wander.y + look.y * 0.5;
  const ex = gx * EYE.gaze * FIT;
  const ey = gy * EYE.gaze * FIT;
  const eh = EYE.h * (state === "working" ? 0.82 : 1) * (1 - blink) * (1 - cheer * 0.7);
  const eyes = [-1, 1].map((s) => ({
    x: s * EYE.split * 0.5 * R + ex * R,
    y: (EYE.y + ey) * R,
    w: EYE.w * R,
    h: Math.max(eh * R, 0.6),
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
  // The seed still phases the blink (and nothing else): every bot has a face
  // whichever way it was made, and a row of them never blinks in unison.
  const seed = agent?.avatar.emoji || agent?.id || agent?.name || "orb";
  const color = agent?.avatar.color || "#64748b";
  const phase = useMemo(() => ((hash(seed) % 1000) / 1000) * BLINK_EVERY, [seed]);
  const face = useRef<Face>({ el: null, move: null, eyes: [null, null], cheer: null, notif: null });
  const hover = useRef(false);
  const cheer = useRef(0);
  // Live values the clock reads: the component may re-render (a new state, a
  // new colour) without restarting the loop.
  const live = useRef({ state, phase });
  live.current = { state, phase };
  // A constant stroke weight on screen, whatever the size.
  const stroke = useMemo(() => r2(150 / size), [size]);

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
          <g transform={BRAIN_FIT} fill="none" stroke={color} strokeWidth={stroke} strokeLinecap="round" strokeLinejoin="round">
            {BRAIN.map((d) => (
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
              fill="var(--orb-eye, rgba(255,255,255,.92))"
              transform={`translate(${r2(e.x)} ${r2(e.y)})`}
            />
          ))}
          {/* The cheer: the eyes close up into two smiles, gliding with `cheer`. */}
          <g
            ref={(n) => {
              face.current.cheer = n;
            }}
            stroke="var(--orb-eye, rgba(255,255,255,.92))"
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
 *   Silhouette — Tabler Icons `brain` (v3.49.0), MIT: six paths in a 24-box,
 *   used stroked as the icon draws them.
 *   Animation posture — orbs (nikuscs/orbs, MIT), packages/avatar-blobs by
 *   Jérémy Perret: the shared clock, the breath/blink/gaze expressions, the
 *   capsule eyes and the 1.2s hover cheer are after that engine, simplified to
 *   four states.
 */
