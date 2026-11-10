var OpenBotAvatar = (() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, { get: all[name], enumerable: true });
  };
  var __copyProps = (to, from, except, desc) => {
    if (from && typeof from === "object" || typeof from === "function") {
      for (let key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(to, key) && key !== except)
          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
    }
    return to;
  };
  var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

  // design/vendor/open-bot-avatar/entry.js
  var entry_exports = {};
  __export(entry_exports, {
    brain: () => brain_default,
    cat: () => cat_default,
    getExample: () => getExample,
    mountAvatar: () => mountAvatar,
    renderAvatarSvg: () => renderAvatarSvg,
    star: () => star_default,
    validateAvatar: () => validateAvatar
  });

  // design/vendor/open-bot-avatar/package/dist/follow.js
  var REST = { x: 0, y: 0, attention: 0 };
  var TAU = 0.12;
  var ATTEND_TAU = 0.3;
  var REACH = 1;
  var MIN_SIZE = 160;
  function pointerGaze(pointer2, box, origin = { x: 0.5, y: 0.5 }) {
    const reach = Math.max(box.width, box.height, MIN_SIZE) * REACH;
    const dx = (pointer2.x - (box.left + box.width * origin.x)) / reach;
    const dy = (pointer2.y - (box.top + box.height * origin.y)) / reach;
    const m = Math.hypot(dx, dy);
    if (!Number.isFinite(m) || m < 1e-9)
      return { x: 0, y: 0 };
    const k = Math.tanh(m) / m;
    return { x: dx * k, y: dy * k };
  }
  function followStep(state, target, dt, reduced) {
    const goal = target ? { x: target.x, y: target.y, attention: 1 } : REST;
    const step = Math.max(0, Math.min(dt, 0.1));
    const ease = (tau) => reduced ? 1 : 1 - Math.exp(-step / tau);
    const k = ease(target ? TAU : ATTEND_TAU);
    const next = {
      x: state.x + (goal.x - state.x) * k,
      y: state.y + (goal.y - state.y) * k,
      attention: state.attention + (goal.attention - state.attention) * ease(ATTEND_TAU)
    };
    const moving = Math.abs(goal.x - next.x) > 2e-3 || Math.abs(goal.y - next.y) > 2e-3 || Math.abs(goal.attention - next.attention) > 2e-3;
    return { state: moving ? next : goal, moving };
  }
  function followGaze(state) {
    const a = Math.min(1, Math.max(0, state.attention));
    return a > 0 ? { x: state.x, y: state.y, weight: a * a * (3 - 2 * a) } : void 0;
  }

  // design/vendor/open-bot-avatar/package/dist/mood.js
  var MOOD_MS = 420;
  var smooth = (t) => {
    const x = Math.min(1, Math.max(0, t));
    return x * x * (3 - 2 * x);
  };
  function moodProgress(elapsedMs) {
    return smooth(elapsedMs / MOOD_MS);
  }
  function settleMood(state, share = 1, depth = 0) {
    const b = state.blend;
    if (!b)
      return state;
    const rest = share * (1 - b.t);
    if (rest < 0.02 || depth >= 3)
      return { motion: state.motion };
    return { motion: state.motion, blend: { from: typeof b.from === "string" ? b.from : settleMood(b.from, rest, depth + 1), t: b.t } };
  }

  // design/vendor/open-bot-avatar/package/dist/animator.js
  function createAnimator() {
    let follow = REST;
    let last = null;
    let mood = null;
    return {
      step({ now, motion, target, hold = false, reducedMotion }) {
        const dt = last === null ? 0 : (now - last) / 1e3;
        last = now;
        let moving = false;
        if (!hold)
          ({ state: follow, moving } = followStep(follow, target, dt, reducedMotion));
        if (mood && motion !== mood.motion) {
          const shown = { motion: mood.motion, blend: mood.blend && { from: mood.blend.from, t: moodProgress(now - mood.since) } };
          mood = { motion, blend: reducedMotion ? void 0 : { from: settleMood(shown), t: 0 }, since: now };
        }
        mood ??= { motion, since: now };
        const t = reducedMotion ? 1 : moodProgress(now - mood.since);
        let motionBlend;
        if (mood.blend && t < 1)
          motionBlend = { from: mood.blend.from, t };
        else
          mood.blend = void 0;
        return { gaze: followGaze(follow), motionBlend, moving: moving || motionBlend !== void 0 };
      },
      reset() {
        follow = REST;
        last = null;
        mood = null;
      }
    };
  }

  // design/vendor/open-bot-avatar/package/dist/color.js
  function parseHex(input) {
    if (typeof input !== "string")
      return null;
    const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(input.trim());
    if (!m)
      return null;
    let h = m[1];
    if (h.length === 3)
      h = h[0] + h[0] + h[1] + h[1] + h[2] + h[2];
    return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
  }
  function toHex([r, g, b]) {
    const c = (v) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, "0");
    return `#${c(r)}${c(g)}${c(b)}`;
  }

  // design/vendor/open-bot-avatar/package/dist/motion.js
  var BASE_YAW = -0.3;
  var BASE_PITCH = 0.2;
  var LOOK_YAW = 0.32;
  var LOOK_PITCH = 0.18;
  var LOOK_EYES = 0.7;
  var TAU2 = Math.PI * 2;
  function hashString(s) {
    let h = 2166136261;
    for (let i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h = Math.imul(h, 16777619);
    }
    return h >>> 0;
  }
  function noise(seed, n) {
    let h = (seed ^ Math.imul(n | 0, 2654435761)) >>> 0;
    h = Math.imul(h ^ h >>> 16, 2246822507);
    h = Math.imul(h ^ h >>> 13, 3266489909);
    h ^= h >>> 16;
    return (h >>> 0) / 4294967296;
  }
  var clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
  var smooth2 = (u2) => {
    const x = clamp(u2, 0, 1);
    return x * x * (3 - 2 * x);
  };
  function blinkAt(seed, t) {
    const SLOT = 4.2, DUR = 0.19;
    const slot = Math.floor(t / SLOT);
    let closure = 0;
    for (const s of [slot - 1, slot]) {
      const start = s * SLOT + 0.5 + noise(seed, s * 2) * 2.9;
      const starts = noise(seed, s * 2 + 1) > 0.8 ? [start, start + 0.34] : [start];
      for (const b of starts) {
        const u2 = (t - b) / DUR;
        if (u2 > 0 && u2 < 1)
          closure = Math.max(closure, Math.sin(Math.PI * u2));
      }
    }
    return closure;
  }
  function wanderAt(seed, t) {
    const SLOT = 1.9;
    const s = Math.floor(t / SLOT);
    const target = (k) => {
      if (noise(seed, 1e3 + k * 3) < 0.4)
        return [0, 0];
      return [(noise(seed, 1001 + k * 3) * 2 - 1) * 0.6, (noise(seed, 1002 + k * 3) * 2 - 1) * 0.35];
    };
    const a = target(s - 1), b = target(s);
    const e = smooth2((t - s * SLOT) / 0.16);
    return [a[0] + (b[0] - a[0]) * e, a[1] + (b[1] - a[1]) * e];
  }
  function blendPoses(a, b, t) {
    const k = clamp(t, 0, 1);
    const mix = (x, y) => x + (y - x) * k;
    const pose = {
      yaw: mix(a.yaw, b.yaw),
      pitch: mix(a.pitch, b.pitch),
      roll: mix(a.roll, b.roll),
      lift: mix(a.lift, b.lift),
      sx: mix(a.sx, b.sx),
      sy: mix(a.sy, b.sy),
      gazeX: mix(a.gazeX, b.gazeX),
      gazeY: mix(a.gazeY, b.gazeY),
      blink: mix(a.blink, b.blink),
      squintL: mix(a.squintL, b.squintL),
      squintR: mix(a.squintR, b.squintR),
      eyeShape: b.eyeShape
    };
    if (a.eyeShape !== b.eyeShape) {
      pose.eyeShape = k < 0.5 ? a.eyeShape : b.eyeShape;
      pose.blink = Math.max(pose.blink, 1 - Math.abs(2 * k - 1));
    }
    return pose;
  }
  function computePose(input) {
    const pose = moodPose(input);
    const blend = input.blend;
    if (!blend || !(blend.t < 1))
      return pose;
    const from = typeof blend.from === "string" ? { motion: blend.from } : blend.from;
    return blendPoses(computePose({ ...input, motion: from.motion, blend: from.blend }), pose, Number.isFinite(blend.t) ? blend.t : 1);
  }
  function moodPose({ seed, time, motion, gaze, reducedMotion, view }) {
    const t = reducedMotion ? 0 : time + noise(seed, 7) * 10;
    const wave = (period, phase = 0) => reducedMotion ? 0 : Math.sin(TAU2 * t / period + phase);
    const given = gaze && Number.isFinite(gaze.x) && Number.isFinite(gaze.y) ? [clamp(gaze.x, -1, 1), clamp(gaze.y, -1, 1)] : null;
    const weight = given ? Number.isFinite(gaze?.weight) ? clamp(gaze?.weight, 0, 1) : 1 : 0;
    const sway = 1 - 0.7 * weight;
    const pose = {
      yaw: BASE_YAW,
      pitch: BASE_PITCH,
      roll: 0,
      lift: 0,
      sx: 1,
      sy: 1,
      gazeX: 0,
      gazeY: 0,
      blink: 0,
      squintL: 0,
      squintR: 0,
      eyeShape: "pill"
    };
    if (motion === "happy") {
      const period = 0.92;
      const p = reducedMotion ? 0.5 : (t % period + period) % period / period;
      const air = 4 * p * (1 - p);
      const land = reducedMotion ? 0 : clamp(1 - Math.min(p, 1 - p) / 0.12, 0, 1);
      pose.lift = reducedMotion ? 0 : -3.2 * air;
      pose.sy = 1 - 0.09 * land + 0.03 * air * (reducedMotion ? 0 : 1);
      pose.sx = 1 + 0.07 * land - 0.015 * air * (reducedMotion ? 0 : 1);
      pose.roll = 4.5 * wave(period * 2);
      pose.yaw += 0.1 * wave(period * 2, 0.6);
      pose.eyeShape = "happy";
    } else if (motion === "thinking") {
      pose.yaw += 0.12 + 0.05 * sway * wave(7.3);
      pose.pitch += 0.08;
      pose.roll = -5 + 1.4 * wave(4.1);
      pose.lift = -0.5 * (1 - Math.cos(TAU2 * t / 5.2)) * (reducedMotion ? 0 : 1);
      pose.gazeX = 0.55 + 0.3 * wave(5.6);
      pose.gazeY = -0.8;
      pose.squintL = 0.22;
      pose.squintR = 0.32 + 0.1 * wave(3.1);
      if (!reducedMotion)
        pose.blink = blinkAt(seed, t * 0.7);
    } else {
      pose.yaw += 0.07 * sway * wave(6.1);
      pose.pitch += 0.04 * sway * wave(4.7, 1);
      pose.roll = 1.6 * wave(5.3, 2);
      pose.lift = reducedMotion ? 0 : -0.45 * (1 - Math.cos(TAU2 * t / 3.4));
      const breath = wave(3.4, -Math.PI / 2);
      pose.sy = 1 + 0.012 * breath;
      pose.sx = 1 - 8e-3 * breath;
      if (!reducedMotion) {
        pose.blink = blinkAt(seed, t);
        [pose.gazeX, pose.gazeY] = wanderAt(seed, t);
      }
    }
    if (given && weight > 0) {
      const mix = (a, b) => a + (b - a) * weight;
      const share = reducedMotion ? 1 : LOOK_EYES;
      if (pose.eyeShape === "pill") {
        pose.gazeX = mix(pose.gazeX, given[0] * share);
        pose.gazeY = mix(pose.gazeY, given[1] * share);
      }
      if (!reducedMotion) {
        pose.yaw += weight * (given[0] * LOOK_YAW - BASE_YAW);
        pose.pitch += weight * (-given[1] * LOOK_PITCH - BASE_PITCH / 2);
      }
    }
    if (view && Number.isFinite(view.yaw) && Number.isFinite(view.pitch)) {
      pose.yaw += view.yaw;
      pose.pitch += view.pitch;
    }
    return pose;
  }

  // design/vendor/open-bot-avatar/package/dist/path.js
  var PATH_LIMITS = {
    maxChars: 6e3,
    maxSegments: 400,
    maxSubpaths: 16,
    coordMin: -20,
    coordMax: 120
  };
  var ARG_COUNT = { M: 2, L: 2, H: 1, V: 1, C: 6, S: 4, Q: 4, T: 2, A: 7, Z: 0 };
  var PathError = class extends Error {
  };
  function tokenize(d) {
    if (/[^MmLlHhVvCcSsQqTtAaZz0-9.,eE+\-\s]/.test(d)) {
      const bad = d.match(/[^MmLlHhVvCcSsQqTtAaZz0-9.,eE+\-\s]/)[0];
      throw new PathError(`unsupported character ${JSON.stringify(bad)} (allowed commands: M L H V C S Q T A Z)`);
    }
    const tokens = [];
    const re = /([MmLlHhVvCcSsQqTtAaZz])|([+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?)|([\s,]+)/y;
    let i = 0;
    while (i < d.length) {
      re.lastIndex = i;
      const m = re.exec(d);
      if (!m)
        throw new PathError(`cannot parse path near ${JSON.stringify(d.slice(i, i + 12))}`);
      if (m[1])
        tokens.push(m[1]);
      else if (m[2])
        tokens.push(Number(m[2]));
      i = re.lastIndex;
    }
    return tokens;
  }
  function parsePath(d) {
    if (typeof d !== "string" || d.trim() === "")
      throw new PathError("path is empty");
    if (d.length > PATH_LIMITS.maxChars)
      throw new PathError(`path longer than ${PATH_LIMITS.maxChars} characters`);
    const tokens = tokenize(d);
    const out = [];
    let i = 0;
    let cx = 0, cy = 0, sx = 0, sy = 0;
    let prevC = null;
    let prevQ = null;
    let cmd = "";
    let subpaths = 0;
    let open = false;
    while (i < tokens.length) {
      const t = tokens[i];
      if (typeof t === "string") {
        cmd = t;
        i++;
        if (cmd === "Z" || cmd === "z") {
          if (!open)
            throw new PathError("Z without an open subpath");
          out.push({ c: "Z" });
          cx = sx;
          cy = sy;
          prevC = prevQ = null;
          open = false;
          continue;
        }
      } else if (!cmd) {
        throw new PathError("path must start with M");
      } else if (cmd === "Z" || cmd === "z") {
        throw new PathError("numbers after Z");
      }
      const up = cmd.toUpperCase();
      const rel = cmd !== up;
      if (out.length === 0 && up !== "M")
        throw new PathError("path must start with M");
      const n = ARG_COUNT[up];
      const args = [];
      for (let k = 0; k < n; k++) {
        const v = tokens[i + k];
        if (typeof v !== "number")
          throw new PathError(`command ${cmd} expects ${n} numbers`);
        if (!Number.isFinite(v))
          throw new PathError("non-finite number");
        args.push(v);
      }
      i += n;
      const ox = rel ? cx : 0, oy = rel ? cy : 0;
      let nextC = null;
      let nextQ = null;
      switch (up) {
        case "M": {
          if (open)
            out.push({ c: "Z" });
          cx = args[0] + ox;
          cy = args[1] + oy;
          sx = cx;
          sy = cy;
          out.push({ c: "M", x: cx, y: cy });
          subpaths++;
          open = true;
          cmd = rel ? "l" : "L";
          break;
        }
        case "L":
          cx = args[0] + ox;
          cy = args[1] + oy;
          out.push({ c: "L", x: cx, y: cy });
          break;
        case "H":
          cx = args[0] + ox;
          out.push({ c: "L", x: cx, y: cy });
          break;
        case "V":
          cy = args[0] + oy;
          out.push({ c: "L", x: cx, y: cy });
          break;
        case "C": {
          const s = { c: "C", x1: args[0] + ox, y1: args[1] + oy, x2: args[2] + ox, y2: args[3] + oy, x: args[4] + ox, y: args[5] + oy };
          out.push(s);
          nextC = [s.x2, s.y2];
          cx = s.x;
          cy = s.y;
          break;
        }
        case "S": {
          const x1 = prevC ? 2 * cx - prevC[0] : cx, y1 = prevC ? 2 * cy - prevC[1] : cy;
          const s = { c: "C", x1, y1, x2: args[0] + ox, y2: args[1] + oy, x: args[2] + ox, y: args[3] + oy };
          out.push(s);
          nextC = [s.x2, s.y2];
          cx = s.x;
          cy = s.y;
          break;
        }
        case "Q": {
          const s = { c: "Q", x1: args[0] + ox, y1: args[1] + oy, x: args[2] + ox, y: args[3] + oy };
          out.push(s);
          nextQ = [s.x1, s.y1];
          cx = s.x;
          cy = s.y;
          break;
        }
        case "T": {
          const x1 = prevQ ? 2 * cx - prevQ[0] : cx, y1 = prevQ ? 2 * cy - prevQ[1] : cy;
          const s = { c: "Q", x1, y1, x: args[0] + ox, y: args[1] + oy };
          out.push(s);
          nextQ = [x1, y1];
          cx = s.x;
          cy = s.y;
          break;
        }
        case "A": {
          const [rx, ry, rot, la, sw] = args;
          if (rx < 0 || ry < 0)
            throw new PathError("arc radii must be non-negative");
          if (la !== 0 && la !== 1 || sw !== 0 && sw !== 1)
            throw new PathError("arc flags must be 0 or 1, written as separate numbers");
          cx = args[5] + ox;
          cy = args[6] + oy;
          out.push({ c: "A", rx, ry, rot, large: la, sweep: sw, x: cx, y: cy });
          break;
        }
        default:
          throw new PathError(`unsupported command ${cmd}`);
      }
      prevC = nextC;
      prevQ = nextQ;
      if (out.length > PATH_LIMITS.maxSegments)
        throw new PathError(`more than ${PATH_LIMITS.maxSegments} segments`);
    }
    if (open)
      out.push({ c: "Z" });
    if (subpaths > PATH_LIMITS.maxSubpaths)
      throw new PathError(`more than ${PATH_LIMITS.maxSubpaths} subpaths`);
    for (const s of out) {
      for (const [k, v] of Object.entries(s)) {
        if (k === "c" || typeof v !== "number")
          continue;
        if (k === "rot" || k === "large" || k === "sweep")
          continue;
        if (v < PATH_LIMITS.coordMin || v > PATH_LIMITS.coordMax)
          throw new PathError(`coordinate ${v} far outside the 0..100 viewBox`);
      }
    }
    return out;
  }
  function fmt(n) {
    const r = Math.round(n * 100) / 100;
    return (Object.is(r, -0) ? 0 : r).toString();
  }
  function serializePath(segs) {
    const parts = [];
    for (const s of segs) {
      switch (s.c) {
        case "M":
          parts.push(`M${fmt(s.x)} ${fmt(s.y)}`);
          break;
        case "L":
          parts.push(`L${fmt(s.x)} ${fmt(s.y)}`);
          break;
        case "C":
          parts.push(`C${fmt(s.x1)} ${fmt(s.y1)} ${fmt(s.x2)} ${fmt(s.y2)} ${fmt(s.x)} ${fmt(s.y)}`);
          break;
        case "Q":
          parts.push(`Q${fmt(s.x1)} ${fmt(s.y1)} ${fmt(s.x)} ${fmt(s.y)}`);
          break;
        case "A":
          parts.push(`A${fmt(s.rx)} ${fmt(s.ry)} ${fmt(s.rot)} ${s.large} ${s.sweep} ${fmt(s.x)} ${fmt(s.y)}`);
          break;
        case "Z":
          parts.push("Z");
          break;
      }
    }
    return parts.join("");
  }
  function sampleArc(x0, y0, s, out) {
    let { rx, ry } = s;
    const { x, y } = s;
    if (rx === 0 || ry === 0 || x0 === x && y0 === y) {
      out.push([x, y]);
      return;
    }
    const phi = s.rot * Math.PI / 180;
    const cos = Math.cos(phi), sin = Math.sin(phi);
    const dx = (x0 - x) / 2, dy = (y0 - y) / 2;
    const x1p = cos * dx + sin * dy, y1p = -sin * dx + cos * dy;
    rx = Math.abs(rx);
    ry = Math.abs(ry);
    const lambda = x1p * x1p / (rx * rx) + y1p * y1p / (ry * ry);
    if (lambda > 1) {
      const k = Math.sqrt(lambda);
      rx *= k;
      ry *= k;
    }
    const num2 = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    const den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let coef = Math.sqrt(Math.max(0, num2 / den));
    if (s.large === s.sweep)
      coef = -coef;
    const cxp = coef * rx * y1p / ry, cyp = -coef * ry * x1p / rx;
    const cx = cos * cxp - sin * cyp + (x0 + x) / 2;
    const cy = sin * cxp + cos * cyp + (y0 + y) / 2;
    const ang = (ux, uy, vx, vy) => Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
    const t1 = ang(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let dt = ang((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
    if (!s.sweep && dt > 0)
      dt -= 2 * Math.PI;
    if (s.sweep && dt < 0)
      dt += 2 * Math.PI;
    const steps = Math.max(4, Math.ceil(Math.abs(dt) / (Math.PI / 2) * 6));
    for (let k = 1; k <= steps; k++) {
      const t = t1 + dt * k / steps;
      const ex = rx * Math.cos(t), ey = ry * Math.sin(t);
      out.push([cos * ex - sin * ey + cx, sin * ex + cos * ey + cy]);
    }
  }
  function flattenPath(segs) {
    const polys = [];
    let cur = [];
    let px = 0, py = 0;
    for (const s of segs) {
      switch (s.c) {
        case "M":
          if (cur.length)
            polys.push(cur);
          cur = [[s.x, s.y]];
          px = s.x;
          py = s.y;
          break;
        case "L":
          cur.push([s.x, s.y]);
          px = s.x;
          py = s.y;
          break;
        case "C": {
          const n = 12;
          for (let k = 1; k <= n; k++) {
            const t = k / n, u2 = 1 - t;
            cur.push([
              u2 * u2 * u2 * px + 3 * u2 * u2 * t * s.x1 + 3 * u2 * t * t * s.x2 + t * t * t * s.x,
              u2 * u2 * u2 * py + 3 * u2 * u2 * t * s.y1 + 3 * u2 * t * t * s.y2 + t * t * t * s.y
            ]);
          }
          px = s.x;
          py = s.y;
          break;
        }
        case "Q": {
          const n = 8;
          for (let k = 1; k <= n; k++) {
            const t = k / n, u2 = 1 - t;
            cur.push([u2 * u2 * px + 2 * u2 * t * s.x1 + t * t * s.x, u2 * u2 * py + 2 * u2 * t * s.y1 + t * t * s.y]);
          }
          px = s.x;
          py = s.y;
          break;
        }
        case "A":
          sampleArc(px, py, s, cur);
          px = s.x;
          py = s.y;
          break;
        case "Z":
          if (cur.length) {
            polys.push(cur);
            px = cur[0][0];
            py = cur[0][1];
          }
          cur = [];
          break;
      }
    }
    if (cur.length)
      polys.push(cur);
    return polys.filter((p) => p.length >= 3);
  }
  function winding(polys, x, y) {
    let w = 0;
    for (const poly of polys) {
      for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
        const [xi, yi] = poly[i];
        const [xj, yj] = poly[j];
        if (yj <= y) {
          if (yi > y && (xi - xj) * (y - yj) - (x - xj) * (yi - yj) > 0)
            w++;
        } else if (yi <= y && (xi - xj) * (y - yj) - (x - xj) * (yi - yj) < 0)
          w--;
      }
    }
    return w;
  }
  function isInside(polys, x, y) {
    return winding(polys, x, y) !== 0;
  }
  function insideRow(polys, y, xs, out) {
    const hits = [];
    for (const poly of polys) {
      for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
        const [xj, yj] = poly[j], [xi, yi] = poly[i];
        if (yj <= y && yi > y)
          hits.push([xj + (y - yj) * (xi - xj) / (yi - yj), 1]);
        else if (yi <= y && yj > y)
          hits.push([xj + (y - yj) * (xi - xj) / (yi - yj), -1]);
      }
    }
    hits.sort((a, b) => a[0] - b[0]);
    let w = 0, h = 0;
    for (let k = 0; k < xs.length; k++) {
      while (h < hits.length && hits[h][0] < xs[k])
        w += hits[h++][1];
      out[k] = w !== 0 ? 1 : 0;
    }
  }
  function bounds(polys) {
    const b = { minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity };
    for (const p of polys)
      for (const [x, y] of p) {
        if (x < b.minX)
          b.minX = x;
        if (y < b.minY)
          b.minY = y;
        if (x > b.maxX)
          b.maxX = x;
        if (y > b.maxY)
          b.maxY = y;
      }
    return b;
  }
  function filledArea(polys, b = bounds(polys)) {
    let n = 0;
    const xs = [];
    for (let x = Math.floor(b.minX) + 0.5; x < b.maxX; x += 1)
      xs.push(x);
    const row = new Uint8Array(xs.length);
    for (let y = Math.floor(b.minY) + 0.5; y < b.maxY; y += 1) {
      insideRow(polys, y, xs, row);
      for (const v of row)
        n += v;
    }
    return n;
  }
  function splitSubpaths(segs) {
    const out = [];
    for (const s of segs) {
      if (s.c === "M")
        out.push([]);
      out[out.length - 1]?.push(s);
    }
    return out;
  }

  // design/vendor/open-bot-avatar/package/dist/validate.js
  var EYE_W = 4;
  var EYE_H = 11;
  var BODY_MIN = 6;
  var BODY_MAX = 94;
  var EYE_MARGIN = 1.2;
  var GAZE_X = 1.5;
  var GAZE_Y = 1.1;
  var HAPPY_ARC = { halfWidth: 0.72, base: 0.1, peak: 0.42, stroke: 0.6 };
  var EYE_CLEARANCE = 1.5;
  var TOP_KEYS = ["version", "id", "name", "body", "eyes"];
  var EYE_KEYS = ["x", "y", "spacing", "scale"];
  function isRecord(v) {
    return typeof v === "object" && v !== null && !Array.isArray(v);
  }
  function num(v) {
    return typeof v === "number" && Number.isFinite(v);
  }
  function eyeOutline(cx, cy, scale, margin) {
    const hw = EYE_W * scale / 2 + margin;
    const hh = EYE_H * scale / 2 + margin;
    const pts = [];
    const straight = hh - hw;
    for (let k = 0; k < 16; k++) {
      const a = k / 16 * Math.PI * 2;
      const x = Math.cos(a) * hw;
      const y = Math.sin(a) * hw + Math.sign(Math.sin(a)) * Math.max(0, straight);
      pts.push([cx + x, cy + y]);
    }
    for (const f2 of [-0.5, 0, 0.5]) {
      pts.push([cx - hw, cy + f2 * straight], [cx + hw, cy + f2 * straight]);
    }
    pts.push([cx, cy]);
    return pts;
  }
  function drawnEyeOutline(cx, cy, scale, margin) {
    const pts = eyeOutline(cx, cy, scale, margin);
    const w = EYE_W * scale, h = EYE_H * scale;
    const x0 = cx - HAPPY_ARC.halfWidth * w, x1 = cx + HAPPY_ARC.halfWidth * w;
    const yb = cy + HAPPY_ARC.base * h, yc = cy - HAPPY_ARC.peak * h;
    const r = HAPPY_ARC.stroke * w / 2 + margin;
    for (let i = 0; i <= 8; i++) {
      const t = i / 8, m = 1 - t;
      const x = m * m * x0 + 2 * m * t * cx + t * t * x1;
      const y = (m * m + t * t) * yb + 2 * m * t * yc;
      for (let k = 0; k < 8; k++) {
        const a = k / 8 * Math.PI * 2;
        pts.push([x + Math.cos(a) * r, y + Math.sin(a) * r]);
      }
    }
    return pts;
  }
  function eyeCenters(eyes) {
    return [[eyes.x - eyes.spacing / 2, eyes.y], [eyes.x + eyes.spacing / 2, eyes.y]];
  }
  function eyesFit(polys, eyes, dx, dy, margin) {
    for (const [cx, cy] of eyeCenters(eyes)) {
      for (const [x, y] of eyeOutline(cx + dx, cy + dy, eyes.scale, margin)) {
        if (!isInside(polys, x, y))
          return [x, y];
      }
    }
    return null;
  }
  function drawnEyesFit(polys, eyes, dx, dy, margin) {
    for (const [cx, cy] of eyeCenters(eyes)) {
      for (const [x, y] of drawnEyeOutline(cx + dx, cy + dy, eyes.scale, margin)) {
        if (!isInside(polys, x, y))
          return [x, y];
      }
    }
    return null;
  }
  function gazeRange(polys, eyes) {
    const reach = (ux, uy) => {
      for (const k of [1, 0.8, 0.6, 0.4, 0.2]) {
        if (!drawnEyesFit(polys, eyes, ux * k * GAZE_X * eyes.scale, uy * k * GAZE_Y * eyes.scale, EYE_CLEARANCE))
          return k;
      }
      return 0;
    };
    return { left: reach(-1, 0), right: reach(1, 0), up: reach(0, -1), down: reach(0, 1) };
  }
  function analyzeBody(path) {
    const segs = parsePath(path);
    const polys = flattenPath(segs);
    const b = bounds(polys);
    const holes = [];
    const subpaths = [];
    for (const sub of splitSubpaths(segs)) {
      const own = flattenPath(sub);
      if (!own.length)
        continue;
      const ob = bounds(own);
      let inside = 0, cut = 0;
      let probe = null, cutProbe = null;
      const step = (ob.maxX - ob.minX) * (ob.maxY - ob.minY) < 900 ? 0.5 : 1;
      const alone = own.length === polys.length;
      const xs = [];
      for (let x = Math.floor(ob.minX) + step / 2; x < ob.maxX; x += step)
        xs.push(x);
      const mine = new Uint8Array(xs.length), all = new Uint8Array(xs.length);
      for (let y = Math.floor(ob.minY) + step / 2; y < ob.maxY; y += step) {
        insideRow(own, y, xs, mine);
        if (!alone)
          insideRow(polys, y, xs, all);
        for (let k = 0; k < xs.length; k++) {
          if (!mine[k])
            continue;
          inside++;
          probe ??= [xs[k], y];
          if (!alone && !all[k]) {
            cut++;
            cutProbe ??= [xs[k], y];
          }
        }
      }
      cut *= step * step * 4;
      inside *= step * step * 4;
      const d = serializePath(sub);
      const hole = inside > 0 && cut / inside > 0.6;
      if (hole)
        holes.push({ d, area: Math.round(cut / 4) });
      subpaths.push({ d, polys: own, hole, probe: hole ? cutProbe : probe });
    }
    return { polys, bounds: b, area: filledArea(polys, b), holes, subpaths };
  }
  var r1 = (n) => Math.round(n * 10) / 10;
  function validateAvatar(input) {
    const errors = [];
    if (!isRecord(input))
      return { ok: false, errors: ["definition must be a JSON object"] };
    for (const k of Object.keys(input))
      if (!TOP_KEYS.includes(k))
        errors.push(`unknown field "${k}" (allowed: ${TOP_KEYS.join(", ")})`);
    if (input.version !== 1)
      errors.push("version must be 1");
    const id = input.id;
    if (typeof id !== "string" || !/^[a-z0-9](?:[a-z0-9-]{0,38}[a-z0-9])?$/.test(id)) {
      errors.push("id must be a lowercase slug of 1-40 chars [a-z0-9-]");
    }
    const name = typeof input.name === "string" ? input.name.trim() : "";
    if (!name || name.length > 40 || /[\u0000-\u001f<>]/.test(name)) {
      errors.push("name must be 1-40 printable characters without < or >");
    }
    let path = "";
    if (!isRecord(input.body))
      errors.push('body must be an object { "path": "..." }');
    else {
      for (const k of Object.keys(input.body))
        if (k !== "path")
          errors.push(`unknown field "body.${k}" (only "path" is allowed)`);
      if (typeof input.body.path !== "string")
        errors.push("body.path must be a string");
      else
        path = input.body.path;
    }
    let eyes = null;
    if (!isRecord(input.eyes))
      errors.push("eyes must be an object { x, y, spacing, scale }");
    else {
      const e = input.eyes;
      for (const k of Object.keys(e))
        if (!EYE_KEYS.includes(k))
          errors.push(`unknown field "eyes.${k}" (allowed: ${EYE_KEYS.join(", ")})`);
      const bad = EYE_KEYS.filter((k) => !num(e[k]));
      if (bad.length)
        errors.push(`eyes.${bad.join(", eyes.")} must be finite numbers`);
      else {
        eyes = { x: e.x, y: e.y, spacing: e.spacing, scale: e.scale };
        if (eyes.scale < 0.6 || eyes.scale > 1.8)
          errors.push(`eyes.scale ${eyes.scale} out of range 0.6..1.8`);
        const minSpacing = EYE_W * eyes.scale * 1.75;
        if (eyes.spacing < minSpacing || eyes.spacing > 40) {
          errors.push(`eyes.spacing ${eyes.spacing} out of range ${r1(minSpacing)}..40 for scale ${eyes.scale}`);
        }
      }
    }
    let normalized = "";
    let body = null;
    if (path) {
      try {
        const segs = parsePath(path);
        normalized = serializePath(segs);
        body = analyzeBody(normalized);
      } catch (err) {
        if (err instanceof PathError)
          errors.push(`body.path: ${err.message}`);
        else
          throw err;
      }
    }
    if (body) {
      const b = body.bounds;
      const out = [];
      if (b.minX < BODY_MIN)
        out.push(`left edge x=${r1(b.minX)}`);
      if (b.minY < BODY_MIN)
        out.push(`top edge y=${r1(b.minY)}`);
      if (b.maxX > BODY_MAX)
        out.push(`right edge x=${r1(b.maxX)}`);
      if (b.maxY > BODY_MAX)
        out.push(`bottom edge y=${r1(b.maxY)}`);
      if (out.length)
        errors.push(`body.path leaves the safe area (${out.join(", ")}); keep geometry within 8..92`);
      const w = b.maxX - b.minX, h = b.maxY - b.minY;
      if (Math.max(w, h) < 40 || Math.min(w, h) < 14) {
        errors.push(`body is too small (${r1(w)}x${r1(h)}); use at least 40 units on the long side and 14 on the short side`);
      }
      if (body.area < 600)
        errors.push(`body fill area ${body.area} is too thin to carry eyes; must be at least 600 units\xB2`);
      if (eyes) {
        const miss = eyesFit(body.polys, eyes, 0, 0, EYE_MARGIN);
        if (miss) {
          errors.push(`eyes must sit inside the body with ${EYE_MARGIN} units margin; point (${r1(miss[0])}, ${r1(miss[1])}) is outside or in a hole. Move eyes.x/y toward the widest solid area, reduce spacing or scale.`);
        }
      }
    }
    if (errors.length || !eyes)
      return { ok: false, errors };
    return {
      ok: true,
      value: { version: 1, id, name, body: { path: normalized }, eyes: { ...eyes } }
    };
  }

  // design/vendor/open-bot-avatar/package/dist/contour.js
  function contourLoops(field, level) {
    const { values, nx, ny, x0, y0, step } = field;
    const at = (i, j) => values[j * nx + i] - level;
    const points = /* @__PURE__ */ new Map();
    const links = /* @__PURE__ */ new Map();
    const point = (key) => {
      if (points.has(key))
        return;
      const idx = key >> 1, i = idx % nx, j = (idx - i) / nx;
      const a = at(i, j);
      if (key & 1) {
        const b = at(i, j + 1), t = a / (a - b);
        points.set(key, [x0 + i * step, y0 + (j + t) * step]);
      } else {
        const b = at(i + 1, j), t = a / (a - b);
        points.set(key, [x0 + (i + t) * step, y0 + j * step]);
      }
    };
    const link = (p, q) => {
      point(p);
      point(q);
      (links.get(p) ?? links.set(p, []).get(p)).push(q);
      (links.get(q) ?? links.set(q, []).get(q)).push(p);
    };
    for (let j = 0; j < ny - 1; j++) {
      for (let i = 0; i < nx - 1; i++) {
        const tl = at(i, j) >= 0, tr = at(i + 1, j) >= 0, br = at(i + 1, j + 1) >= 0, bl = at(i, j + 1) >= 0;
        const code = (tl ? 8 : 0) | (tr ? 4 : 0) | (br ? 2 : 0) | (bl ? 1 : 0);
        if (code === 0 || code === 15)
          continue;
        const top = 2 * (j * nx + i), bottom = 2 * ((j + 1) * nx + i);
        const left = 2 * (j * nx + i) + 1, right = 2 * (j * nx + i + 1) + 1;
        if (code === 10 || code === 5) {
          const center = (at(i, j) + at(i + 1, j) + at(i + 1, j + 1) + at(i, j + 1)) / 4 >= 0;
          if (code === 10 === center) {
            link(top, right);
            link(bottom, left);
          } else {
            link(left, top);
            link(right, bottom);
          }
          continue;
        }
        const crossed = [];
        if (tl !== tr)
          crossed.push(top);
        if (tr !== br)
          crossed.push(right);
        if (br !== bl)
          crossed.push(bottom);
        if (bl !== tl)
          crossed.push(left);
        link(crossed[0], crossed[1]);
      }
    }
    const loops = [];
    const seen = /* @__PURE__ */ new Set();
    for (const start of links.keys()) {
      if (seen.has(start))
        continue;
      const loop = [];
      let prev = -1, cur = start;
      while (!seen.has(cur)) {
        seen.add(cur);
        loop.push(points.get(cur));
        const next = links.get(cur).find((n) => n !== prev && !seen.has(n));
        if (next === void 0)
          break;
        prev = cur;
        cur = next;
      }
      if (loop.length >= 3)
        loops.push(loop);
    }
    return loops;
  }
  function perpendicular(p, a, b) {
    const dx = b[0] - a[0], dy = b[1] - a[1];
    const len = Math.hypot(dx, dy);
    if (len === 0)
      return Math.hypot(p[0] - a[0], p[1] - a[1]);
    return Math.abs(dx * (a[1] - p[1]) - dy * (a[0] - p[0])) / len;
  }
  function simplifyLoop(loop, tolerance) {
    if (loop.length <= 8)
      return loop;
    let far = 0, best = -1;
    for (let k = 1; k < loop.length; k++) {
      const d = Math.hypot(loop[k][0] - loop[0][0], loop[k][1] - loop[0][1]);
      if (d > best) {
        best = d;
        far = k;
      }
    }
    const keep = new Uint8Array(loop.length);
    keep[0] = keep[far] = 1;
    const stack = [[0, far], [far, loop.length]];
    while (stack.length) {
      const [s, e] = stack.pop();
      const end = loop[e % loop.length];
      let idx = -1, max = tolerance;
      for (let k = s + 1; k < e; k++) {
        const d = perpendicular(loop[k], loop[s], end);
        if (d > max) {
          max = d;
          idx = k;
        }
      }
      if (idx >= 0) {
        keep[idx] = 1;
        stack.push([s, idx], [idx, e]);
      }
    }
    return loop.filter((_, k) => keep[k]);
  }
  var n1 = (v) => {
    const r = Math.round(v * 10) / 10;
    return (Object.is(r, -0) ? 0 : r).toString();
  };
  function loopsToPath(loops) {
    return loops.map((loop) => `M${n1(loop[0][0])} ${n1(loop[0][1])}L${loop.slice(1).map((p) => `${n1(p[0])} ${n1(p[1])}`).join(" ")}Z`).join("");
  }

  // design/vendor/open-bot-avatar/package/dist/volume.js
  var MAX_PART_DEPTH = 30;
  var EYE_BOOST = 1.45;
  var EYE_BOOST_PAD = 3;
  var OUTLINE_SLOPE = 4;
  var MAX_X_SLICES = 40;
  var THIN = 4;
  var THIN_REACH = 5;
  var THIN_MARGIN = 2;
  var FINE_Z = 0.6;
  function segDist(px, py, ax, ay, bx, by) {
    const dx = bx - ax, dy = by - ay;
    const len = dx * dx + dy * dy;
    let t = len > 0 ? ((px - ax) * dx + (py - ay) * dy) / len : 0;
    t = t < 0 ? 0 : t > 1 ? 1 : t;
    const ex = ax + t * dx - px, ey = ay + t * dy - py;
    return Math.sqrt(ex * ex + ey * ey);
  }
  function edgeDistance(polys, x, y) {
    let best = Infinity;
    for (const poly of polys) {
      for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
        const d = segDist(x, y, poly[j][0], poly[j][1], poly[i][0], poly[i][1]);
        if (d < best)
          best = d;
      }
    }
    return best;
  }
  function surfaceZ(part, dist) {
    const half = part.depth / 2;
    if (part.rxy <= 0 || dist >= part.rxy)
      return half;
    const k = 1 - Math.max(0, dist) / part.rxy;
    return half - part.rz + Math.sqrt(Math.max(0, 1 - k * k)) * part.rz;
  }
  function partDepth(thickness, b, volume) {
    const long = Math.max(b.maxX - b.minX, b.maxY - b.minY);
    const raw = 0.8 * Math.max(thickness, Math.min(0.4 * long, 2.4 * thickness));
    return Math.min(MAX_PART_DEPTH, Math.max(3, raw)) * volume;
  }
  function crossings(polys, rows, s) {
    const out = [];
    const a = rows ? 1 : 0, b = rows ? 0 : 1;
    for (const poly of polys) {
      for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
        const p = poly[j], q = poly[i];
        if (p[a] <= s && q[a] > s || q[a] <= s && p[a] > s)
          out.push(p[b] + (s - p[a]) * (q[b] - p[b]) / (q[a] - p[a]));
      }
    }
    return out.sort((m, n) => m - n);
  }
  function revolveProfile(polys, rows, lo, hi, center) {
    const step = 0.2;
    const n = Math.max(2, Math.ceil((hi - lo) / step) + 1);
    const r = new Float64Array(n);
    let span = 0;
    const mids = [];
    for (let k = 0; k < n; k++) {
      const s = Math.min(hi - 0.01, Math.max(lo + 0.01, lo + k * step));
      const xs = crossings(polys, rows, s);
      if (xs.length === 0)
        continue;
      if (xs.length !== 2)
        return null;
      r[k] = (xs[1] - xs[0]) / 2;
      span = Math.max(span, xs[1] - xs[0]);
      mids.push((xs[0] + xs[1]) / 2);
    }
    const tol = Math.max(0.8, 0.04 * span);
    if (!mids.length || mids.some((m) => Math.abs(m - center) > tol))
      return null;
    return { start: lo, step, r };
  }
  function radiusAt(p, s) {
    const t = (s - p.start) / p.step;
    if (t < 0 || t > p.r.length - 1)
      return 0;
    const k = Math.min(p.r.length - 2, Math.floor(t));
    return p.r[k] + (p.r[k + 1] - p.r[k]) * (t - k);
  }
  function revolveAxis(body) {
    const b = body.bounds;
    const outlines = body.subpaths.filter((s) => !s.hole);
    if (outlines.length < 2 || b.maxX - b.minX < 1.1 * (b.maxY - b.minY))
      return null;
    const axis = (b.minY + b.maxY) / 2;
    for (const o of outlines) {
      const ob = bounds(o.polys);
      if (Math.abs((ob.minY + ob.maxY) / 2 - axis) > 1.5)
        return null;
    }
    let filled = 0, off = 0;
    const xs = [];
    for (let x = b.minX + 0.5; x < b.maxX; x += 1)
      xs.push(x);
    const top = new Uint8Array(xs.length), bottom = new Uint8Array(xs.length);
    for (let y = b.minY + 0.5; y < axis; y += 1) {
      insideRow(body.polys, y, xs, top);
      insideRow(body.polys, 2 * axis - y, xs, bottom);
      for (let k = 0; k < xs.length; k++) {
        if (top[k] || bottom[k])
          filled++;
        if (top[k] !== bottom[k])
          off++;
      }
    }
    return filled > 0 && off / filled < 0.02 ? axis : null;
  }
  var EdgeIndex = class {
    cell = 3;
    buckets = /* @__PURE__ */ new Map();
    e = [];
    ox;
    oy;
    span;
    constructor(polys, b) {
      this.ox = b.minX - 4;
      this.oy = b.minY - 4;
      this.span = Math.ceil((Math.max(b.maxX - b.minX, b.maxY - b.minY) + 8) / this.cell) + 1;
      for (const poly of polys) {
        for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
          const k = this.e.length;
          this.e.push(poly[j][0], poly[j][1], poly[i][0], poly[i][1]);
          const [x0, x1] = [Math.min(poly[j][0], poly[i][0]), Math.max(poly[j][0], poly[i][0])];
          const [y0, y1] = [Math.min(poly[j][1], poly[i][1]), Math.max(poly[j][1], poly[i][1])];
          for (let cy = this.cy(y0); cy <= this.cy(y1); cy++) {
            for (let cx = this.cx(x0); cx <= this.cx(x1); cx++) {
              const key = cy * this.span + cx;
              (this.buckets.get(key) ?? this.buckets.set(key, []).get(key)).push(k);
            }
          }
        }
      }
    }
    cx(x) {
      return Math.max(0, Math.min(this.span - 1, Math.floor((x - this.ox) / this.cell)));
    }
    cy(y) {
      return Math.max(0, Math.min(this.span - 1, Math.floor((y - this.oy) / this.cell)));
    }
    /** Distance to the nearest edge, searching rings of buckets outward until no closer edge can exist. */
    distance(x, y) {
      const bx = this.cx(x), by = this.cy(y);
      let best = Infinity;
      const e = this.e;
      for (let r = 0; r < this.span; r++) {
        for (let cy = by - r; cy <= by + r; cy++) {
          if (cy < 0 || cy >= this.span)
            continue;
          const edgeRow = cy === by - r || cy === by + r;
          for (let cx = bx - r; cx <= bx + r; cx += edgeRow ? 1 : 2 * r || 1) {
            if (cx < 0 || cx >= this.span)
              continue;
            const list = this.buckets.get(cy * this.span + cx);
            if (!list)
              continue;
            for (const k of list) {
              const d = segDist(x, y, e[k], e[k + 1], e[k + 2], e[k + 3]);
              if (d < best)
                best = d;
            }
          }
        }
        if (best <= r * this.cell)
          break;
      }
      return best;
    }
  };
  function sampleGrid(polys, b, step, height) {
    const pad = 1;
    const x0 = b.minX - pad, y0 = b.minY - pad;
    const nx = Math.ceil((b.maxX - b.minX + 2 * pad) / step) + 1;
    const ny = Math.ceil((b.maxY - b.minY + 2 * pad) / step) + 1;
    const sd = new Float64Array(nx * ny), h = new Float64Array(nx * ny);
    const index = new EdgeIndex(polys, b);
    const xs = Array.from({ length: nx }, (_, i) => x0 + i * step);
    const inside = new Uint8Array(nx);
    for (let j = 0; j < ny; j++) {
      const y = y0 + j * step;
      insideRow(polys, y, xs, inside);
      for (let i = 0; i < nx; i++) {
        const d = index.distance(xs[i], y);
        sd[j * nx + i] = inside[i] ? d : -d;
        h[j * nx + i] = inside[i] ? height(xs[i], y, d) : 0;
      }
    }
    return { x0, y0, step, nx, ny, sd, h };
  }
  function loopBox(loops) {
    const b = bounds(loops);
    return { x: b.minX, y: b.minY, w: b.maxX - b.minX, h: b.maxY - b.minY };
  }
  function levelLoops(g, level, box) {
    const i0 = box ? Math.max(0, box.i0 - 1) : 0, i1 = box ? Math.min(g.nx - 1, box.i1 + 1) : g.nx - 1;
    const j0 = box ? Math.max(0, box.j0 - 1) : 0, j1 = box ? Math.min(g.ny - 1, box.j1 + 1) : g.ny - 1;
    const nx = i1 - i0 + 1, ny = j1 - j0 + 1;
    const values = new Float64Array(nx * ny);
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        const k = (j + j0) * g.nx + i + i0;
        let v = Math.min(g.h[k] - level, OUTLINE_SLOPE * g.sd[k]);
        if (box)
          v = Math.min(v, OUTLINE_SLOPE * g.step * Math.min(i + i0 - box.i0, box.i1 - i - i0, j + j0 - box.j0, box.j1 - j - j0));
        values[j * nx + i] = v;
      }
    }
    return contourLoops({ values, nx, ny, x0: g.x0 + i0 * g.step, y0: g.y0 + j0 * g.step, step: g.step }, 0).map((l) => simplifyLoop(l, 0.04)).filter((l) => l.length >= 3);
  }
  function thinBoxes(g) {
    const { nx, ny } = g;
    const reach = Math.ceil(THIN_REACH / g.step);
    const rows = new Float64Array(nx * ny), widest = new Float64Array(nx * ny);
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        let m = 0;
        for (let k = Math.max(0, i - reach), e = Math.min(nx - 1, i + reach); k <= e; k++)
          m = Math.max(m, g.sd[j * nx + k]);
        rows[j * nx + i] = m;
      }
    }
    for (let i = 0; i < nx; i++) {
      for (let j = 0; j < ny; j++) {
        let m = 0;
        for (let k = Math.max(0, j - reach), e = Math.min(ny - 1, j + reach); k <= e; k++)
          m = Math.max(m, rows[k * nx + i]);
        widest[j * nx + i] = m;
      }
    }
    const thin = (k) => g.sd[k] > 0 && widest[k] < THIN;
    const seen = new Uint8Array(nx * ny);
    const grow = Math.ceil(THIN_MARGIN / g.step);
    const boxes = [];
    for (let start = 0; start < nx * ny; start++) {
      if (seen[start] || !thin(start))
        continue;
      const box = { i0: nx, i1: -1, j0: ny, j1: -1, top: 0 };
      let cells = 0;
      const stack = [start];
      seen[start] = 1;
      while (stack.length) {
        const k = stack.pop();
        const i = k % nx, j = (k - i) / nx;
        cells++;
        box.top = Math.max(box.top, g.h[k]);
        box.i0 = Math.min(box.i0, i);
        box.i1 = Math.max(box.i1, i);
        box.j0 = Math.min(box.j0, j);
        box.j1 = Math.max(box.j1, j);
        for (const n of [i > 0 ? k - 1 : -1, i < nx - 1 ? k + 1 : -1, j > 0 ? k - nx : -1, j < ny - 1 ? k + nx : -1]) {
          if (n >= 0 && !seen[n] && thin(n)) {
            seen[n] = 1;
            stack.push(n);
          }
        }
      }
      if (cells * g.step * g.step < 2)
        continue;
      boxes.push({
        i0: Math.max(0, box.i0 - grow),
        i1: Math.min(nx - 1, box.i1 + grow),
        j0: Math.max(0, box.j0 - grow),
        j1: Math.min(ny - 1, box.j1 + grow),
        top: box.top
      });
    }
    for (let a = 0; a < boxes.length; a++) {
      for (let b = a + 1; b < boxes.length; b++) {
        const p = boxes[a], q = boxes[b];
        if (p.i0 > q.i1 || q.i0 > p.i1 || p.j0 > q.j1 || q.j0 > p.j1)
          continue;
        boxes[a] = { i0: Math.min(p.i0, q.i0), i1: Math.max(p.i1, q.i1), j0: Math.min(p.j0, q.j0), j1: Math.max(p.j1, q.j1), top: Math.max(p.top, q.top) };
        boxes.splice(b, 1);
        b = a;
      }
    }
    return boxes;
  }
  function columnLoops(g, c0, c1, edgeHeight) {
    const loops = [];
    const sd = new Float64Array(g.ny).fill(-Infinity), h = new Float64Array(g.ny);
    for (let j2 = 0; j2 < g.ny; j2++) {
      for (let c = c0; c <= c1; c++) {
        const k = j2 * g.nx + c;
        if (g.sd[k] > sd[j2])
          sd[j2] = g.sd[k];
        if (g.sd[k] > 0 && g.h[k] > h[j2])
          h[j2] = g.h[k];
      }
    }
    const yAt = (jj) => g.y0 + jj * g.step;
    const edge = (inside, outside) => yAt(inside) + (yAt(outside) - yAt(inside)) * sd[inside] / (sd[inside] - sd[outside]);
    const rim = (y, j2) => {
      let top = 0;
      for (let c = c0; c <= c1; c++)
        top = Math.max(top, edgeHeight(g.x0 + c * g.step, y));
      return Math.min(h[j2], top);
    };
    let j = 0;
    while (j < g.ny) {
      if (sd[j] <= 0) {
        j++;
        continue;
      }
      const j0 = j;
      while (j < g.ny && sd[j] > 0)
        j++;
      const j1 = j - 1;
      const yTop = j0 > 0 ? edge(j0, j0 - 1) : yAt(j0);
      const yBot = j1 < g.ny - 1 ? edge(j1, j1 + 1) : yAt(j1);
      const right = [[j0 > 0 ? rim(yTop, j0) : h[j0], yTop]];
      for (let k = j0; k <= j1; k++)
        right.push([h[k], yAt(k)]);
      right.push([j1 < g.ny - 1 ? rim(yBot, j1) : h[j1], yBot]);
      if (Math.max(...right.map((p) => p[0])) < 0.05)
        continue;
      const left = right.map(([u2, y]) => [-u2, y]).reverse();
      loops.push(simplifyLoop([...right, ...left], 0.04));
    }
    return loops;
  }
  function levels(flat, round, steps) {
    const out = [];
    for (let k = 1; k < steps; k++)
      out.push(flat + round * Math.sin(k / steps * (Math.PI / 2)));
    return out;
  }
  function buildVolume(def, volume = 1, body = analyzeBody(def.body.path)) {
    const outlines = body.subpaths.filter((s) => !s.hole);
    const holes = body.subpaths.filter((s) => s.hole);
    const v = Number.isFinite(volume) ? Math.max(0, volume) : 1;
    const xAxis = revolveAxis(body);
    const parts = outlines.map((o) => {
      const cuts = holes.filter((h) => h.probe && isInside(o.polys, h.probe[0], h.probe[1]));
      const polys = [...o.polys, ...cuts.flatMap((h) => h.polys)];
      const b = bounds(o.polys);
      const center = [(b.minX + b.maxX) / 2, (b.minY + b.maxY) / 2];
      const minDim = Math.min(b.maxX - b.minX, b.maxY - b.minY);
      const step = Math.max(0.25, Math.min(0.45, minDim / 24));
      let kind = "inflate";
      let profile = null;
      if (xAxis !== null) {
        profile = revolveProfile(polys, false, b.minX, b.maxX, xAxis);
        if (profile)
          kind = "revolve-x";
      } else if (outlines.length > 1 && cuts.length === 0) {
        profile = revolveProfile(polys, true, b.minY, b.maxY, center[0]);
        if (profile)
          kind = "revolve-y";
      }
      let thickness, depth, rxy = 0, rz = 0;
      let heightOf;
      if (kind === "inflate" || !profile) {
        const coarse = sampleGrid(polys, b, Math.max(step, 0.6), () => 0);
        thickness = 2 * coarse.sd.reduce((m, d) => Math.max(m, d), 0);
        depth = partDepth(thickness, b, v);
        rxy = Math.min(thickness / 2, depth / 2) * 0.9;
        rz = depth / 2 * 0.85;
        const shape = { depth, rxy, rz };
        heightOf = (_x, _y, sd) => surfaceZ(shape, sd);
      } else {
        const p = profile;
        const axis = kind === "revolve-x" ? xAxis : center[0];
        const radius = Math.max(...p.r);
        thickness = 2 * radius;
        depth = 2 * radius * v;
        heightOf = kind === "revolve-x" ? (x, y) => v * Math.sqrt(Math.max(0, radiusAt(p, x) ** 2 - (y - axis) ** 2)) : (x, y) => v * Math.sqrt(Math.max(0, radiusAt(p, y) ** 2 - (x - axis) ** 2));
      }
      const height = (x, y) => isInside(polys, x, y) ? heightOf(x, y, kind === "inflate" ? edgeDistance(polys, x, y) : 0) : 0;
      const core = {
        axis: "z",
        at: 0,
        d: o.d + cuts.map((h) => h.d).join(""),
        loops: polys,
        evenOdd: false,
        box: { x: b.minX, y: b.minY, w: b.maxX - b.minX, h: b.maxY - b.minY },
        inset: 0
      };
      const zSlices = [core];
      const xSlices = [];
      const half2 = depth / 2;
      if (half2 > 0.05) {
        const g = sampleGrid(polys, b, step, heightOf);
        const steps = kind === "inflate" ? Math.max(4, Math.min(14, Math.round(rz / 0.8))) : Math.max(5, Math.min(18, Math.round(half2 / 1.1)));
        const ls = kind === "inflate" ? [half2 - rz, ...levels(half2 - rz, rz, steps)] : levels(0, half2, steps);
        const addLevel = (level, box) => {
          const loops = levelLoops(g, level, box);
          if (!loops.length)
            return false;
          const sample = loops.flat().filter((_, k) => k % 3 === 0);
          const inset = Math.min(...sample.map(([x, y]) => edgeDistance(polys, x, y)));
          const slice = { axis: "z", d: loopsToPath(loops), loops, evenOdd: true, box: loopBox(loops), inset };
          zSlices.push({ ...slice, at: level }, { ...slice, at: -level });
          return true;
        };
        const used = [0];
        for (const level of ls)
          if (level >= 0.02 && addLevel(level))
            used.push(level);
        const thin = kind === "inflate" ? thinBoxes(g) : [];
        for (const box of thin) {
          for (let k = 1; k < used.length && used[k - 1] < box.top; k++) {
            const a = used[k - 1], gap = used[k] - a, m = Math.ceil(gap / FINE_Z - 1e-9);
            for (let q = 1; q < m && a + gap * q / m < box.top; q++)
              addLevel(a + gap * q / m, box);
          }
        }
        zSlices.sort((m, n) => m.at - n.at);
        const width = b.maxX - b.minX;
        const every = Math.max(1, Math.round(Math.max(1.1, width / MAX_X_SLICES) / g.step));
        const edgeHeight = (x, y) => heightOf(x, y, 0);
        const before = Math.floor((every - 1) / 2);
        for (let i = 1; i < g.nx - 1; i += every) {
          const loops = columnLoops(g, Math.max(0, i - before), Math.min(g.nx - 1, i - before + every - 1), edgeHeight);
          if (loops.length)
            xSlices.push({ axis: "x", at: g.x0 + i * g.step, d: loopsToPath(loops), loops, evenOdd: true, box: loopBox(loops), inset: 0 });
        }
      }
      return {
        kind,
        d: core.d,
        polys,
        bounds: b,
        center,
        thickness,
        depth,
        rxy,
        rz,
        axis: kind === "revolve-x" ? xAxis : kind === "revolve-y" ? center[0] : void 0,
        zSlices,
        xSlices,
        height
      };
    });
    const eyeScale = boostedEyeScale(body, def.eyes);
    const centers = eyeCenters(def.eyes);
    let eyePart = 0, bestZ = -Infinity;
    parts.forEach((part, i) => {
      const zs = centers.map(([x, y]) => part.height(x, y));
      if (zs.some((z2) => z2 <= 0))
        return;
      const z = Math.min(...zs);
      if (z > bestZ) {
        bestZ = z;
        eyePart = i;
      }
    });
    const host = parts[eyePart];
    const w = EYE_W * eyeScale / 2 + GAZE_X * eyeScale, hh = EYE_H * eyeScale / 2 + GAZE_Y * eyeScale;
    const eyeZ = centers.map(([x, y]) => {
      let top = 0;
      for (const fx of [-1, -0.5, 0, 0.5, 1])
        for (const fy of [-1, -0.5, 0, 0.5, 1])
          top = Math.max(top, host ? host.height(x + fx * w, y + fy * hh) : 0);
      return top;
    });
    const normal = (x, y) => {
      if (!host)
        return [0, 0, 1];
      const e = 0.75;
      const gx = (host.height(x + e, y) - host.height(x - e, y)) / (2 * e);
      const gy = (host.height(x, y + e) - host.height(x, y - e)) / (2 * e);
      const n = Math.hypot(gx, gy, 1);
      return [-gx / n, -gy / n, 1 / n];
    };
    const half = EYE_W * eyeScale / 2;
    const eyeNormals = centers.map(([x, y]) => [normal(x - half, y), normal(x, y), normal(x + half, y)]);
    return { parts, bounds: body.bounds, eyePart, eyeZ, eyeScale, eyeNormals };
  }
  function boostedEyeScale(body, eyes) {
    for (const k of [EYE_BOOST, 1.38, 1.3, 1.22, 1.15, 1.08]) {
      if (!drawnEyesFit(body.polys, { ...eyes, scale: eyes.scale * k }, 0, 0, EYE_BOOST_PAD))
        return eyes.scale * k;
    }
    return eyes.scale;
  }

  // design/vendor/open-bot-avatar/package/dist/render.js
  var DEFAULT_COLOR = "#ff6a1a";
  var DEFAULT_EYE_COLOR = "#151515";
  var DEFAULT_DEPTH = 6;
  var MAX_DEPTH = 14;
  var EYE_LIFT = 1.5;
  var cache = /* @__PURE__ */ new Map();
  var f = (n) => {
    const r = Math.round(n * 100) / 100;
    return (Object.is(r, -0) ? 0 : r).toString();
  };
  var clamp2 = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
  function depthOption(options) {
    const d = clamp2(Number.isFinite(options.depth) ? options.depth : DEFAULT_DEPTH, 0, MAX_DEPTH);
    return Math.round(d * 2) / 2;
  }
  function prepare(definition, depth) {
    const key = JSON.stringify([definition?.id, definition?.name, definition?.body?.path, definition?.eyes, depth]);
    const hit = cache.get(key);
    if (hit)
      return hit;
    const res = validateAvatar(definition);
    if (!res.ok)
      throw new Error(`Invalid avatar definition: ${res.errors.join("; ")}`);
    const def = res.value;
    const body = analyzeBody(def.body.path);
    const vol = buildVolume(def, depth / DEFAULT_DEPTH, body);
    const id = `oba-${hashString(`${def.id}|${def.body.path}|${depth}`).toString(36)}`;
    const ids = /* @__PURE__ */ new Map();
    for (const part of vol.parts)
      for (const s of [...part.zSlices, ...part.xSlices])
        if (!ids.has(s.d))
          ids.set(s.d, `${id}-${ids.size}`);
    const prepared = {
      def,
      vol,
      range: gazeRange(body.polys, { ...def.eyes, scale: vol.eyeScale }),
      seed: hashString(def.id),
      id,
      ids,
      detail: /* @__PURE__ */ new Map()
    };
    if (cache.size >= 64)
      cache.delete(cache.keys().next().value);
    cache.set(key, prepared);
    return prepared;
  }
  function poseOf(p, options) {
    const motion = options.motion === "thinking" || options.motion === "happy" ? options.motion : "idle";
    return computePose({
      seed: p.seed,
      time: Number.isFinite(options.time) ? options.time : 0,
      motion,
      gaze: options.gaze,
      reducedMotion: options.reducedMotion === true,
      view: options.view,
      blend: options.motionBlend
    });
  }
  function esc(s) {
    return s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
  }
  var mul = (m, n) => [
    m[0] * n[0] + m[2] * n[1],
    m[1] * n[0] + m[3] * n[1],
    m[0] * n[2] + m[2] * n[3],
    m[1] * n[2] + m[3] * n[3],
    m[0] * n[4] + m[2] * n[5] + m[4],
    m[1] * n[4] + m[3] * n[5] + m[5]
  ];
  var translate = (x, y) => [1, 0, 0, 1, x, y];
  var about = (x, y, m) => mul(translate(x, y), mul(m, translate(-x, -y)));
  var mat = (m) => `matrix(${m.map((v, i) => i < 4 ? (Math.round(v * 1e4) / 1e4).toString() : f(v)).join(" ")})`;
  function viewBasis(yaw, pitch) {
    const cY = Math.cos(yaw), cP = Math.cos(pitch);
    return { cY, sY: Math.sin(yaw), cP, sP: Math.sin(pitch), facing: cY * cP };
  }
  var EYE_FACING = 0.22;
  function facingOf(v, [x, y, z]) {
    return v.sP * y + v.cP * (-v.sY * x + v.cY * z);
  }
  function eyeVisible(vol, v, i) {
    return vol.eyeNormals[i].every((n) => facingOf(v, n) > EYE_FACING);
  }
  function activeStack(v) {
    return Math.abs(v.cY) >= Math.abs(v.sY) ? "z" : "x";
  }
  function planeMatrix(v, z, pivot) {
    const [cx, cy] = pivot;
    return [
      v.cY,
      v.sY * v.sP,
      0,
      v.cP,
      cx - v.cY * cx + v.sY * z,
      cy - v.cP * cy - v.sY * v.sP * cx - v.cY * v.sP * z
    ];
  }
  function sidePlaneMatrix(v, x0, pivot) {
    const [cx, cy] = pivot;
    return [-v.sY, v.cY * v.sP, 0, v.cP, cx + v.cY * (x0 - cx), cy - v.cP * cy + v.sY * v.sP * (x0 - cx)];
  }
  function projectPoint(v, [x, y, z], pivot) {
    const m = planeMatrix(v, z, pivot);
    const X = x - pivot[0], Y = y - pivot[1];
    return [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5], v.sP * Y + v.cP * (-v.sY * X + v.cY * z)];
  }
  function rootMatrix(pose, b) {
    const bx = (b.minX + b.maxX) / 2, by = b.maxY;
    const r = pose.roll * Math.PI / 180;
    const rot = [Math.cos(r), Math.sin(r), -Math.sin(r), Math.cos(r), 0, 0];
    return mul(translate(0, pose.lift), about(bx, by, mul(rot, [pose.sx, 0, 0, pose.sy, 0, 0])));
  }
  var pivotOf = (b) => [(b.minX + b.maxX) / 2, (b.minY + b.maxY) / 2];
  function drawnSlices(vol, v, visit) {
    const stack = activeStack(v);
    const pivot = pivotOf(vol.bounds);
    const drawn = sliceCull(v);
    vol.parts.forEach((part, i) => {
      (stack === "z" ? part.zSlices : part.xSlices).forEach((s, j) => {
        if (!drawn(s))
          return;
        visit(s, stack === "z" ? planeMatrix(v, s.at, pivot) : sidePlaneMatrix(v, s.at, pivot), i, j);
      });
    });
  }
  function sliceCull(v) {
    const sweep = Math.hypot(v.sY, v.cY * v.sP);
    const inset = Math.min(Math.abs(v.cY), Math.abs(v.cP));
    return (s) => s.axis !== "z" || s.inset <= 0 || Math.abs(s.at) * sweep > s.inset * inset;
  }
  function eyeStates(p, pose) {
    const s = p.vol.eyeScale;
    const w = EYE_W * s, h = EYE_H * s;
    const r = p.range;
    const dx = pose.gazeX * (pose.gazeX >= 0 ? r.right : r.left) * GAZE_X * s;
    const dy = pose.gazeY * (pose.gazeY >= 0 ? r.down : r.up) * GAZE_Y * s;
    const centers = eyeCenters(p.def.eyes);
    return [0, 1].map((i) => {
      const squint = i === 0 ? pose.squintL : pose.squintR;
      const open = Math.max(h * Math.max(0.09, (1 - pose.blink) * (1 - squint * 0.4)), w * 0.55);
      return { x: centers[i][0], y: centers[i][1], w, h, dx, dy, open, shift: (h - open) * 0.18, happy: pose.eyeShape === "happy" };
    });
  }
  function happyArc(e) {
    const ex = e.x + e.dx, ey = e.y + e.dy;
    const hw = e.w * HAPPY_ARC.halfWidth, base = ey + e.h * HAPPY_ARC.base;
    return `M${f(ex - hw)} ${f(base)}Q${f(ex)} ${f(ey - e.h * HAPPY_ARC.peak)} ${f(ex + hw)} ${f(base)}`;
  }
  function eyeMark(e) {
    if (e.happy)
      return { shape: "arc", d: happyArc(e), strokeWidth: e.w * HAPPY_ARC.stroke };
    const cx = e.x + e.dx, cy = e.y + e.dy + e.shift;
    return { shape: "pill", x: cx - e.w / 2, y: cy - e.open / 2, width: e.w, height: e.open, radius: Math.min(e.w, e.open) / 2 };
  }
  function colors(options) {
    return {
      color: toHex(parseHex(options.color) ?? parseHex(DEFAULT_COLOR)),
      ink: toHex(parseHex(options.eyeColor) ?? parseHex(DEFAULT_EYE_COLOR))
    };
  }
  function avatarFrame(definition, options = {}) {
    const p = prepare(definition, depthOption(options));
    const { vol } = p;
    const b = vol.bounds;
    const { color, ink } = colors(options);
    const pose = poseOf(p, options);
    const pivot = pivotOf(b);
    const v = viewBasis(pose.yaw, pose.pitch);
    const stack = activeStack(v);
    const eyesMid = [p.def.eyes.x, p.def.eyes.y];
    const paths = /* @__PURE__ */ new Map();
    const items = [];
    drawnSlices(vol, v, (s, m, i, j) => {
      let path = paths.get(s.d);
      if (!path) {
        path = { id: p.ids.get(s.d), d: s.d, evenOdd: s.evenOdd };
        paths.set(s.d, path);
      }
      const [rx, ry] = i === vol.eyePart ? eyesMid : vol.parts[i].center;
      const depth = stack === "z" ? projectPoint(v, [rx, ry, s.at], pivot)[2] : projectPoint(v, [s.at, ry, 0], pivot)[2];
      items.push({ depth, shape: { kind: "slice", key: `${i}${stack}${j}`, path: path.id, matrix: m } });
    });
    eyeStates(p, pose).forEach((e, idx) => {
      if (eyeVisible(vol, v, idx)) {
        const z = vol.eyeZ[idx] + EYE_LIFT;
        items.push({
          depth: projectPoint(v, [eyesMid[0], eyesMid[1], z], pivot)[2] + 1e-6,
          shape: { kind: "eye", key: idx === 0 ? "eye-0" : "eye-1", matrix: planeMatrix(v, z, pivot), mark: eyeMark(e) }
        });
      }
    });
    items.sort((a, c) => a.depth - c.depth);
    return { name: p.def.name, color, ink, paths: [...paths.values()], root: rootMatrix(pose, b), shapes: items.map((it) => it.shape) };
  }
  function eyeSvg(mark, ink) {
    if (mark.shape === "arc") {
      return `<path d="${mark.d}" fill="none" stroke="${ink}" stroke-width="${f(mark.strokeWidth)}" stroke-linecap="round"/>`;
    }
    return `<rect x="${f(mark.x)}" y="${f(mark.y)}" width="${f(mark.width)}" height="${f(mark.height)}" rx="${f(mark.radius)}" fill="${ink}"/>`;
  }
  function renderAvatarSvg(definition, options = {}) {
    const frame2 = avatarFrame(definition, options);
    const defsMarkup = frame2.paths.map((d) => `<path id="${d.id}" d="${d.d}"${d.evenOdd ? ' fill-rule="evenodd"' : ""}/>`).join("");
    const body = frame2.shapes.map((s) => s.kind === "slice" ? `<use href="#${s.path}" transform="${mat(s.matrix)}"/>` : `<g transform="${mat(s.matrix)}">${eyeSvg(s.mark, frame2.ink)}</g>`).join("");
    const size = Number.isFinite(options.size) && options.size > 0 ? f(clamp2(options.size, 8, 4096)) : null;
    const dims = size ? ` width="${size}" height="${size}"` : "";
    const label = esc(frame2.name);
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"${dims} role="img" aria-label="${label}" overflow="visible"><title>${label}</title><defs>${defsMarkup}</defs><g transform="${mat(frame2.root)}" fill="${frame2.color}">` + body + `</g></svg>`;
  }
  var u = (n) => `${f(n)}cqw`;
  var FILL = "position:absolute;left:0;top:0;width:100%;height:100%;";
  function sliceMarkup(s) {
    const pad = 0.3;
    const x = s.box.x - pad, y = s.box.y - pad, w = s.box.w + 2 * pad, h = s.box.h + 2 * pad;
    const rule = s.evenOdd ? ' fill-rule="evenodd"' : "";
    const place = s.axis === "z" ? `left:${u(x)};top:${u(y)};transform:translateZ(${u(s.at)})` : (
      // The side plane's u = 0 line sits at x = s.at; turning it 90° about that line maps u to -z.
      `left:${u(s.at + x)};top:${u(y)};transform-origin:${u(-x)} 50%;transform:rotateY(90deg)`
    );
    return `<svg viewBox="${f(x)} ${f(y)} ${f(w)} ${f(h)}" style="position:absolute;width:${u(w)};height:${u(h)};overflow:visible;${place}" aria-hidden="true"><path d="${s.d}"${rule} fill="currentColor"/></svg>`;
  }
  function renderAvatarScene(definition, options = {}) {
    const p = prepare(definition, depthOption(options));
    const { vol } = p;
    const b = vol.bounds;
    const [cx, cy] = pivotOf(b);
    const ink = `var(--avatar-ink,${DEFAULT_EYE_COLOR})`;
    const stack = (axis) => `<div data-stack="${axis}" style="${FILL}transform-style:preserve-3d">` + vol.parts.map((part) => (axis === "z" ? part.zSlices : part.xSlices).map(sliceMarkup).join("")).join("") + `</div>`;
    const eyes = eyeStates(p, poseOf(p, { reducedMotion: true })).map((e, i) => {
      const box = `left:${u(e.x - e.w / 2)};top:${u(e.y - e.h / 2)};width:${u(e.w)};height:${u(e.h)}`;
      return `<div data-eye="${i}" style="position:absolute;${box};backface-visibility:hidden;transform:translateZ(${u(vol.eyeZ[i] + EYE_LIFT)})"><i style="position:absolute;inset:0;border-radius:999px;background:${ink}"></i><svg viewBox="${f(e.x - e.w / 2)} ${f(e.y - e.h / 2)} ${f(e.w)} ${f(e.h)}" style="${FILL}overflow:visible;display:none" aria-hidden="true"><path d="${happyArc(e)}" fill="none" stroke="${ink}" stroke-width="${f(e.w * HAPPY_ARC.stroke)}" stroke-linecap="round"/></svg></div>`;
    });
    return `<div data-avatar-scene="${p.id}" style="position:relative;width:100%;height:100%;container-type:size"><div data-motion style="${FILL}will-change:transform;transform-origin:${u((b.minX + b.maxX) / 2)} ${u(b.maxY)}"><div data-view style="${FILL}transform-style:preserve-3d;transform-origin:${u(cx)} ${u(cy)}">` + stack("z") + stack("x") + eyes.join("") + `</div></div></div>`;
  }
  var r4 = (n) => Math.round(n * 1e4) / 1e4;
  var SLICE_GAP_PX = 0.75;
  function sliceDetail(p, stack, size) {
    const px = Number.isFinite(size) && size > 0 ? Math.round(size) : 0;
    const key = `${stack}${px}`;
    let keep = p.detail.get(key);
    if (keep)
      return keep;
    const gap = px ? SLICE_GAP_PX * 100 / px : 0;
    keep = p.vol.parts.flatMap((part) => {
      const slices = stack === "z" ? part.zSlices : part.xSlices;
      let last = -Infinity;
      return slices.map((s, j) => {
        const core = s.axis === "z" && s.at === 0 && s.inset === 0;
        const kept = core || j === 0 || j === slices.length - 1 || s.at - last >= gap;
        if (kept)
          last = s.at;
        return kept;
      });
    });
    if (p.detail.size >= 32)
      p.detail.clear();
    p.detail.set(key, keep);
    return keep;
  }
  function sceneFrame(definition, options = {}) {
    const p = prepare(definition, depthOption(options));
    const pose = poseOf(p, options);
    const v = viewBasis(pose.yaw, pose.pitch);
    const states = eyeStates(p, pose);
    const stack = activeStack(v);
    const drawn = sliceCull(v);
    const kept = sliceDetail(p, stack, options.size);
    let k = 0;
    return {
      scene: p.id,
      motion: `translate(0,${u(pose.lift)}) rotate(${f(pose.roll)}deg) scale(${r4(pose.sx)},${r4(pose.sy)})`,
      view: `rotateX(${r4(pose.pitch)}rad) rotateY(${r4(pose.yaw)}rad)`,
      stack,
      slices: p.vol.parts.flatMap((part) => (stack === "z" ? part.zSlices : part.xSlices).map((s) => kept[k++] && drawn(s))),
      eyes: states.map((e, i) => ({
        visible: eyeVisible(p.vol, v, i),
        happy: e.happy,
        transform: `translateZ(${u(p.vol.eyeZ[i] + EYE_LIFT)}) translate(${u(e.dx)},${u(e.dy)})`,
        pill: `translateY(${u(e.shift)}) scaleY(${r4(e.open / e.h)})`
      }))
    };
  }

  // design/vendor/open-bot-avatar/package/dist/dom/clock.js
  var subscribers = /* @__PURE__ */ new Set();
  var frame = 0;
  var due = /* @__PURE__ */ new WeakMap();
  var pointer = null;
  var startedAt = typeof performance !== "undefined" ? performance.now() : 0;
  function tick(now) {
    frame = 0;
    const time = (now - startedAt) / 1e3;
    let busy = false;
    const ready = [];
    for (const s of subscribers) {
      const at = due.get(s);
      if (at !== void 0 && at > time)
        busy = true;
      else
        ready.push(s);
    }
    for (const s of ready)
      s.measure?.();
    for (const s of ready) {
      const next = s.draw(time, pointer);
      if (typeof next === "number")
        due.set(s, time + next);
      else
        due.delete(s);
      busy = next !== false || busy;
    }
    if (busy)
      wake();
  }
  function wake() {
    if (!frame && subscribers.size > 0 && typeof requestAnimationFrame !== "undefined")
      frame = requestAnimationFrame(tick);
  }
  function subscribeFrames(subscriber) {
    subscribers.add(subscriber);
    wake();
    return () => {
      subscribers.delete(subscriber);
      if (subscribers.size === 0) {
        if (frame)
          cancelAnimationFrame(frame);
        frame = 0;
      }
    };
  }
  function setPointer(next) {
    pointer = next;
    wake();
  }
  function requestFrame() {
    wake();
  }

  // design/vendor/open-bot-avatar/package/dist/dom/pointer.js
  var users = 0;
  var detach = null;
  function attach() {
    const move = (e) => setPointer({ x: e.clientX, y: e.clientY });
    const leave = () => setPointer(null);
    const out = (e) => !e.relatedTarget && leave();
    const lift = (e) => e.pointerType !== "mouse" && leave();
    const hidden = () => document.hidden && leave();
    window.addEventListener("pointermove", move, { passive: true });
    window.addEventListener("pointerdown", move, { passive: true });
    window.addEventListener("pointerout", out);
    window.addEventListener("pointerup", lift);
    window.addEventListener("pointercancel", lift);
    window.addEventListener("blur", leave);
    document.addEventListener("visibilitychange", hidden);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerdown", move);
      window.removeEventListener("pointerout", out);
      window.removeEventListener("pointerup", lift);
      window.removeEventListener("pointercancel", lift);
      window.removeEventListener("blur", leave);
      document.removeEventListener("visibilitychange", hidden);
      setPointer(null);
    };
  }
  function trackPointer() {
    if (typeof window === "undefined")
      return () => {
      };
    if (users++ === 0)
      detach = attach();
    let released = false;
    return () => {
      if (released)
        return;
      released = true;
      if (--users === 0) {
        detach?.();
        detach = null;
      }
    };
  }

  // design/vendor/open-bot-avatar/package/dist/dom/reduced.js
  var QUERY = "(prefers-reduced-motion: reduce)";
  var mql;
  function media() {
    if (mql === void 0) {
      mql = typeof matchMedia === "undefined" ? null : matchMedia(QUERY);
      mql?.addEventListener("change", requestFrame);
    }
    return mql;
  }
  function prefersReducedMotion() {
    return media()?.matches ?? false;
  }

  // design/vendor/open-bot-avatar/package/dist/dom/mount.js
  function mountScene(el, definition, depth) {
    el.innerHTML = renderAvatarScene(definition, { depth });
    const q = (s) => el.querySelector(s);
    const stacks = { z: q('[data-stack="z"]'), x: q('[data-stack="x"]') };
    return {
      key: q("[data-avatar-scene]").dataset.avatarScene ?? "",
      geometry: geometryKey(definition),
      motion: q("[data-motion]"),
      view: q("[data-view]"),
      stacks,
      slices: { z: [...stacks.z.children], x: [...stacks.x.children] },
      eyes: [0, 1].map((i) => {
        const root = q(`[data-eye="${i}"]`);
        return { root, pill: root.querySelector("i"), arc: root.querySelector("svg") };
      }),
      last: {}
    };
  }
  function put(m, key, el, prop, value) {
    if (m.last[key] === value)
      return;
    m.last[key] = value;
    el.style[prop] = value;
  }
  function apply(m, frame2) {
    put(m, "motion", m.motion, "transform", frame2.motion);
    put(m, "view", m.view, "transform", frame2.view);
    put(m, "z", m.stacks.z, "display", frame2.stack === "z" ? "block" : "none");
    put(m, "x", m.stacks.x, "display", frame2.stack === "x" ? "block" : "none");
    const slices = m.slices[frame2.stack];
    frame2.slices.forEach((shown, i) => slices[i] && put(m, `${frame2.stack}${i}`, slices[i], "display", shown ? "block" : "none"));
    frame2.eyes.forEach((eye, i) => {
      const e = m.eyes[i];
      put(m, `e${i}`, e.root, "transform", eye.transform);
      put(m, `v${i}`, e.root, "visibility", eye.visible ? "visible" : "hidden");
      put(m, `p${i}`, e.pill, "transform", eye.pill);
      put(m, `ph${i}`, e.pill, "display", eye.happy ? "none" : "block");
      put(m, `a${i}`, e.arc, "display", eye.happy ? "block" : "none");
    });
  }
  var keys = /* @__PURE__ */ new WeakMap();
  function geometryKey(definition) {
    let key = keys.get(definition);
    if (key === void 0) {
      key = JSON.stringify([definition?.id, definition?.name, definition?.body?.path, definition?.eyes]);
      keys.set(definition, key);
    }
    return key;
  }
  function eyesOrigin(definition) {
    const { x, y } = definition.eyes ?? {};
    const at = (v) => typeof v === "number" && Number.isFinite(v) ? Math.min(1, Math.max(0, v / 100)) : 0.5;
    return { x: at(x), y: at(y) };
  }
  var SMALL_SIZE = 128;
  var SMALL_FPS = 30;
  var FRAME_SLACK = 4e-3;
  function mountAvatar(host, initial) {
    let opts = { ...initial };
    const animator = createAnimator();
    let rect = null;
    let scene = null;
    let lastStill = "";
    let failed = false;
    let dirty = true;
    let releasePointer = null;
    let size = host.clientWidth;
    const resize = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(([entry]) => {
      const next = entry.contentRect.width;
      if (next === size)
        return;
      size = next;
      requestFrame();
    });
    resize?.observe(host);
    if (getComputedStyle(host).position === "static")
      host.style.position = "relative";
    const syncHost = () => {
      host.style.color = opts.color ?? DEFAULT_COLOR;
      host.style.setProperty("--avatar-ink", opts.eyeColor ?? DEFAULT_EYE_COLOR);
      const decorative = opts.label === "";
      if (decorative) {
        host.removeAttribute("role");
        host.removeAttribute("aria-label");
        host.setAttribute("aria-hidden", "true");
      } else {
        host.setAttribute("role", "img");
        host.setAttribute("aria-label", opts.label ?? opts.definition?.name ?? "Avatar");
        host.removeAttribute("aria-hidden");
      }
      const wantsPointer = !!opts.followPointer && !opts.still;
      if (wantsPointer && !releasePointer)
        releasePointer = trackPointer();
      else if (!wantsPointer && releasePointer) {
        releasePointer();
        releasePointer = null;
      }
    };
    const clear = () => {
      host.textContent = "";
      scene = null;
      lastStill = "";
    };
    const draw = (time, pointer2) => {
      const p = opts;
      const reducedMotion = p.reducedMotion === "system" || p.reducedMotion === void 0 ? prefersReducedMotion() : p.reducedMotion;
      const now = performance.now();
      try {
        if (p.still) {
          if (scene)
            clear();
          const svg = renderAvatarSvg(p.definition, {
            color: p.color,
            eyeColor: p.eyeColor,
            depth: p.depth,
            motion: p.mood,
            view: p.view,
            gaze: p.lookAt ?? void 0,
            reducedMotion: true
          });
          if (svg !== lastStill) {
            host.innerHTML = svg;
            lastStill = svg;
          }
          failed = false;
          return false;
        }
        if (lastStill)
          clear();
        const target = p.lookAt ? p.lookAt : p.followPointer && pointer2 && rect && rect.width > 0 ? pointerGaze(pointer2, rect, eyesOrigin(p.definition)) : null;
        const step = animator.step({ now, motion: p.mood ?? "idle", target, hold: p.holdGaze, reducedMotion });
        const options = {
          depth: p.depth,
          motion: p.mood ?? "idle",
          motionBlend: step.motionBlend,
          size: size || void 0,
          time,
          gaze: step.gaze,
          view: p.view,
          reducedMotion
        };
        const frame2 = sceneFrame(p.definition, options);
        if (!scene || scene.key !== frame2.scene || scene.geometry !== geometryKey(p.definition) || !host.contains(scene.view)) {
          scene = mountScene(host, p.definition, p.depth);
        }
        apply(scene, frame2);
        failed = false;
        return !reducedMotion || step.moving;
      } catch (error) {
        if (!failed) {
          clear();
          console.error("open-bot-avatar: render failed", error);
          failed = true;
        }
        return false;
      }
    };
    syncHost();
    const unsubscribe = subscribeFrames({
      measure: () => {
        rect = opts.followPointer && !opts.still && !opts.holdGaze && !opts.lookAt ? host.getBoundingClientRect() : null;
      },
      draw: (time, pointer2) => {
        if (opts.still && !dirty)
          return false;
        dirty = false;
        const more = draw(time, pointer2);
        return more && size > 0 && size < SMALL_SIZE ? 1 / SMALL_FPS - FRAME_SLACK : more;
      }
    });
    return {
      update(next) {
        opts = { ...opts, ...next };
        syncHost();
        dirty = true;
        requestFrame();
      },
      destroy() {
        unsubscribe();
        resize?.disconnect();
        releasePointer?.();
        releasePointer = null;
        clear();
      }
    };
  }

  // design/vendor/open-bot-avatar/package/dist/examples.js
  var examples = [
    // Front view: one squat body; the face sits on the wide center above the grips.
    {
      version: 1,
      id: "controller",
      name: "Controller",
      body: {
        path: "M28 22C38 20 44 23 50 23C56 23 62 20 72 22C84 24 90 34 91 48C92 62 91 76 82 80C75 83 69 78 66 72C63 67 58 66 50 66C42 66 37 67 34 72C31 78 25 83 18 80C9 76 8 62 9 48C10 34 16 24 28 22Z"
      },
      eyes: { x: 50, y: 42, spacing: 16, scale: 1.05 }
    },
    // Side view, mirror-symmetric about y 50 so it turns as a bar with round plates: two weight tiers a side and a soft bar end.
    {
      version: 1,
      id: "barbell",
      name: "Barbell",
      body: {
        path: "M12 41H12A4 4 0 0 1 16 45V55A4 4 0 0 1 12 59H12A4 4 0 0 1 8 55V45A4 4 0 0 1 12 41ZM18 29H18A4 4 0 0 1 22 33V67A4 4 0 0 1 18 71H18A4 4 0 0 1 14 67V33A4 4 0 0 1 18 29ZM26 17H26A5 5 0 0 1 31 22V78A5 5 0 0 1 26 83H26A5 5 0 0 1 21 78V22A5 5 0 0 1 26 17ZM38 33H62A10 10 0 0 1 72 43V57A10 10 0 0 1 62 67H38A10 10 0 0 1 28 57V43A10 10 0 0 1 38 33ZM74 17H74A5 5 0 0 1 79 22V78A5 5 0 0 1 74 83H74A5 5 0 0 1 69 78V22A5 5 0 0 1 74 17ZM82 29H82A4 4 0 0 1 86 33V67A4 4 0 0 1 82 71H82A4 4 0 0 1 78 67V33A4 4 0 0 1 82 29ZM88 41H88A4 4 0 0 1 92 45V55A4 4 0 0 1 88 59H88A4 4 0 0 1 84 55V45A4 4 0 0 1 88 41Z"
      },
      eyes: { x: 50, y: 50, spacing: 15, scale: 1.05 }
    },
    // Side view: the cup carries the face and dominates; a compact handle loop stays a see-through cut-out.
    {
      version: 1,
      id: "mug",
      name: "Mug",
      body: {
        path: "M22 20H62C67 20 70 23 70 28V72C70 80 64 86 56 86H28C20 86 14 80 14 72V28C14 23 17 20 22 20ZM68 36C80 34 87 42 87 53C87 64 80 72 68 70ZM69 45L69 61C75.5 62 79 58.5 79 53C79 47.5 76 44 69 45Z"
      },
      eyes: { x: 42, y: 49, spacing: 16, scale: 1.15 }
    },
    // Front view: a big round globe for the face; its neck runs flush into a straight base (one smooth outline), then a clear step down to the contact cap.
    {
      version: 1,
      id: "lightbulb",
      name: "Lightbulb",
      body: {
        path: "M36 72V66C36 60 18 55 18 40A32 32 0 0 1 82 40C82 55 64 60 64 66V72ZM36 68H64V77A4 4 0 0 1 60 81H40A4 4 0 0 1 36 77ZM43 78H57V84A4 4 0 0 1 53 88H47A4 4 0 0 1 43 84Z"
      },
      eyes: { x: 50, y: 40, spacing: 17, scale: 1.2 }
    },
    // A wide speech bubble with a small tail.
    {
      version: 1,
      id: "chat",
      name: "Chat",
      body: {
        path: "M28 14H72C82 14 90 22 90 32V54C90 64 82 72 72 72H42L25 86C23.5 87.3 21.5 86 22.3 84L27 71.5C17 70 10 63 10 54V32C10 22 18 14 28 14Z"
      },
      eyes: { x: 50, y: 42, spacing: 18, scale: 1.25 }
    },
    // Side view: a big pot (rim + tapered body) hosts the face; a stem with a small bud and two curving leaf blades that taper to soft tips.
    {
      version: 1,
      id: "sprout",
      name: "Sprout",
      body: {
        path: "M49.5 19H50.5A3 3 0 0 1 53.5 22V46H46.5V22A3 3 0 0 1 49.5 19ZM49 38C36 41 22.5 36 17.6 23.4A1.4 1.4 0 0 1 18.4 21.6C29 17.5 41.5 21.5 49 34ZM51 30C57 14.5 72 8.5 82.6 11.8A1.4 1.4 0 0 1 83.5 13.4C80.5 27.5 66 35.5 51 34ZM24 44H76A4 4 0 0 1 80 48V54A4 4 0 0 1 76 58H24A4 4 0 0 1 20 54V48A4 4 0 0 1 24 44ZM24 56H76L71 84C70.4 87.5 68 90 64.5 90H35.5C32 90 29.6 87.5 29 84Z"
      },
      eyes: { x: 50, y: 71, spacing: 16, scale: 1.05 }
    },
    // Side view: a big can body hosts the face; a small rear handle loop, joined to the body at both ends, and a short spout name it.
    {
      version: 1,
      id: "watering-can",
      name: "Watering Can",
      body: {
        path: "M32 38H62C68 38 71 41 72 47L74 76C74.5 82 70 86 64 86H30C24 86 19.5 82 20 76L22 47C23 41 26 38 32 38ZM23 63C14 63 8.5 57 8.5 48C8.5 37 13 30.5 19 30.5C25 30.5 28.5 34 30 39.5L26 52ZM23.5 42C22 38 20 36 17.5 36C15 36 14 41 14 48C14 54 16 57.5 22 58L23.5 47ZM69 66C75 61 78 55 80 46.5L88 49.5C85 62 80 73 72 81ZM78.7 43A7 4.5 25 0 1 91.3 49A7 4.5 25 0 1 78.7 43Z"
      },
      eyes: { x: 47, y: 61, spacing: 16, scale: 1.15 }
    }
  ];
  function getExample(id) {
    return examples.find((e) => e.id === id);
  }

  // design/vendor/open-bot-avatar/gallery/brain.json
  var brain_default = {
    version: 1,
    id: "brain",
    name: "Brain",
    body: {
      path: "M14 50C10 42 14 32 22 30C24 21 34 17 41 21C46 15 56 15 61 20C68 16 78 20 79 28C87 30 91 39 88 47C92 54 89 64 81 66C80 71 76 74 71 73C67 75 64 74 62 72C61 74 61 76 61.5 78C62 81 59 82.5 57 81.5C55.5 80.5 55 78 54 76C53 74 50 72.5 47 72.5C42 74 38 72 36 70C30 73 20 71 18 64C12 62 10 56 14 50Z"
    },
    eyes: {
      x: 50,
      y: 45,
      spacing: 17,
      scale: 1.15
    }
  };

  // design/vendor/open-bot-avatar/gallery/cat.json
  var cat_default = {
    version: 1,
    id: "cat",
    name: "Cat",
    body: {
      path: "M50 23C68 23 82 30 82 45C82 58 68 65 50 65C32 65 18 58 18 45C18 30 32 23 50 23ZM24 42C21 32 20 22 22 15C23 11 26 10 29 12C34 16 39 21 44 28L36 42ZM64 42L56 28C61 21 66 16 71 12C74 10 77 11 78 15C80 22 79 32 76 42ZM38 56L62 56C70 56 74 66 74 76C74 83 70 86 63 86L37 86C30 86 26 83 26 76C26 66 30 56 38 56ZM70 72C77 74 81 70 80 64C79 60 81 57 84 58C87 59 88.5 63 88 68C87 77 80 83 70 81Z"
    },
    eyes: {
      x: 50,
      y: 43,
      spacing: 17,
      scale: 1.1
    }
  };

  // design/vendor/open-bot-avatar/gallery/star.json
  var star_default = {
    version: 1,
    id: "star",
    name: "Star",
    body: {
      path: "M53.7 17.95L62 31.2Q64.1 34.6 68 35.6L83.1 39.3Q89.9 41 85.4 46.4L75.4 58.3Q72.8 61.4 73.1 65.4L74.2 81Q74.7 88 68.2 85.4L53.7 79.5Q50 78 46.3 79.5L31.8 85.4Q25.3 88 25.8 81L26.9 65.4Q27.2 61.4 24.6 58.3L14.6 46.4Q10.1 41 16.9 39.3L32 35.6Q35.9 34.6 38 31.2L46.3 17.95Q50 12 53.7 17.95Z"
    },
    eyes: {
      x: 50,
      y: 52,
      spacing: 16,
      scale: 1.1
    }
  };
  return __toCommonJS(entry_exports);
})();
