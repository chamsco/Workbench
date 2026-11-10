// The ASCII sea behind the window: sharks hunting schools of small fish, or
// koi turning in a pond, drawn as text on one canvas and seen from above.
//
// Ocean.start(el, kind) fills `el` with a canvas and runs until the returned
// function is called; kind is "sharks" or "koi". Every animal is a spine of
// points that follows its head, so a turn travels down the body and the tail
// sweeps; its outline (head, fins, tail) is laid along that spine in body
// coordinates and rasterised into a grid of ink, which is then set as
// characters from a ramp. Sharks leave a short wake; small fish flock and
// scatter when a shark comes close. It pauses with the page and draws one
// still frame under prefers-reduced-motion.

var Ocean = (() => {
  const RAMP = " .:-=+*%#@";
  const TAU = Math.PI * 2;
  const rnd = (a, b) => a + Math.random() * (b - a);
  const clamp = (x, a, b) => Math.max(a, Math.min(b, x));
  // Shortest signed angle from a to b.
  const turn = (a, b) => Math.atan2(Math.sin(b - a), Math.cos(b - a));

  // Half-widths along the body (u = 0 at the snout, 1 at the root of the
  // tail), as fractions of body length.
  const lerpTable = (t, u) => {
    if (u <= t[0][0]) return t[0][1];
    for (let i = 1; i < t.length; i++) if (u <= t[i][0]) {
      const [u0, w0] = t[i - 1], [u1, w1] = t[i];
      return w0 + ((u - u0) / (u1 - u0)) * (w1 - w0);
    }
    return t[t.length - 1][1];
  };

  // Each body plan: a profile, fins as polygons in (u, v) with v across the
  // body (both in body lengths, v mirrored), and how hard each part inks.
  const PLANS = {
    // A hammerhead: the cephalofoil is a wide flat bar at the snout, the
    // pectorals are long and swept back, the tail a narrow fork.
    shark: {
      profile: [[0, 0.03], [0.07, 0.045], [0.3, 0.075], [0.55, 0.055], [0.8, 0.025], [1, 0.012]],
      fins: [
        { pts: [[0, 0], [0, 0.2], [0.015, 0.23], [0.04, 0.2], [0.05, 0.03]], ink: 0.9 },
        { pts: [[0.25, 0.06], [0.36, 0.06], [0.47, 0.21], [0.43, 0.22]], ink: 0.5 },
        { pts: [[0.6, 0.04], [0.66, 0.04], [0.7, 0.08]], ink: 0.45 },
        { pts: [[0.92, 0], [1.14, 0.07], [1.1, 0.075], [0.95, 0.02]], ink: 0.55 },
      ],
      core: 0.95, edge: 0.4,
    },
    // A koi: a round body, fins like paddles, a long fan of a tail.
    koi: {
      profile: [[0, 0.05], [0.06, 0.1], [0.25, 0.13], [0.5, 0.11], [0.72, 0.05], [0.82, 0.03], [1, 0.02]],
      fins: [
        { pts: [[0.18, 0.1], [0.3, 0.11], [0.36, 0.22], [0.3, 0.26], [0.24, 0.24]], ink: 0.45 },
        { pts: [[0.5, 0.09], [0.58, 0.08], [0.62, 0.15], [0.56, 0.17]], ink: 0.4 },
        { pts: [[0.8, 0], [0.9, 0.05], [1.05, 0.17], [1.12, 0.12], [1.15, 0.04], [1.13, 0]], ink: 0.4 },
      ],
      core: 0.9, edge: 0.35, patches: true,
    },
  };

  function inPoly(pts, u, v) {
    let inside = false;
    for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
      const [ui, vi] = pts[i], [uj, vj] = pts[j];
      if ((vi > v) !== (vj > v) && u < ((uj - ui) * (v - vi)) / (vj - vi) + ui) inside = !inside;
    }
    return inside;
  }

  // One animal: a head that steers, and a spine that follows it.
  function animal(plan, len, x, y, heading, speed) {
    const n = 14, seg = len / (n - 1), spine = [];
    for (let i = 0; i < n; i++) spine.push([x - Math.cos(heading) * seg * i, y - Math.sin(heading) * seg * i]);
    return { plan, len, seg, spine, heading, want: heading, speed, cruise: speed, phase: rnd(0, TAU), seed: rnd(0, 100) };
  }

  function stepAnimal(a, dt, t, bounds) {
    // Swim: a beat on the heading, which the spine carries down the body.
    const beat = Math.sin(t * (a.plan === PLANS.koi ? 2.2 : 3) + a.phase) * 0.35;
    const maxTurn = (a.plan === PLANS.koi ? 0.9 : 0.6) * dt;
    a.heading += clamp(turn(a.heading, a.want), -maxTurn, maxTurn);
    const h = a.heading + beat * 0.25;
    const head = a.spine[0];
    head[0] += Math.cos(h) * a.speed * dt;
    head[1] += Math.sin(h) * a.speed * dt;
    for (let i = 1; i < a.spine.length; i++) {
      const p = a.spine[i - 1], q = a.spine[i];
      const dx = q[0] - p[0], dy = q[1] - p[1], d = Math.hypot(dx, dy) || 1;
      q[0] = p[0] + (dx / d) * a.seg;
      q[1] = p[1] + (dy / d) * a.seg;
    }
    // Stay near the window: once well outside, turn for its middle.
    const [hx, hy] = head, m = a.len * 0.6;
    if (hx < -m || hx > bounds.w + m || hy < -m * 0.5 || hy > bounds.h + m * 0.5) a.want = Math.atan2(bounds.h / 2 - hy, bounds.w / 2 - hx);
  }

  // Lay the body plan along the spine and ink the grid with it.
  function inkAnimal(a, g) {
    const { spine, len, plan } = a, n = spine.length;
    // Position and normal at u, extrapolating past the tail for the fin.
    const at = u => {
      const f = clamp(u, 0, 1.3) * (n - 1);
      const i = Math.min(n - 2, Math.floor(f)), r = f - i;
      const p = spine[i], q = spine[i + 1];
      const tx = q[0] - p[0], ty = q[1] - p[1], d = Math.hypot(tx, ty) || 1;
      return [p[0] + tx * r, p[1] + ty * r, -ty / d, tx / d];
    };
    const step = Math.min(g.cw, g.ch) * 0.45 / len;
    const plot = (u, v, ink) => {
      const [x, y, nx, ny] = at(u);
      const c = Math.floor((x + nx * v * len) / g.cw), r = Math.floor((y + ny * v * len) / g.ch);
      if (c < 0 || r < 0 || c >= g.cols || r >= g.rows) return;
      const k = r * g.cols + c;
      if (ink > g.ink[k]) g.ink[k] = ink;
    };
    for (let u = 0; u <= 1; u += step) {
      const w = lerpTable(plan.profile, u);
      for (let v = -w; v <= w; v += step) {
        const across = 1 - Math.abs(v) / w;
        let ink = plan.edge + (plan.core - plan.edge) * Math.sqrt(across);
        // Koi patches ride the body: they are a function of body coordinates.
        if (plan.patches && Math.sin(u * 17 + a.seed) * Math.cos(v * 40 + a.seed * 1.7) > 0.25) ink = Math.min(1, ink + 0.25);
        plot(u, v, ink);
      }
    }
    for (const fin of plan.fins) {
      const us = fin.pts.map(p => p[0]), vs = fin.pts.map(p => p[1]);
      const u0 = Math.min(...us), u1 = Math.max(...us), v0 = Math.min(...vs), v1 = Math.max(...vs);
      for (let u = u0; u <= u1; u += step) for (let v = v0; v <= v1; v += step) {
        if (!inPoly(fin.pts, u, v)) continue;
        plot(u, v, fin.ink);
        plot(u, -v, fin.ink);
      }
    }
  }

  function start(el, kind) {
    el.innerHTML = "";
    const cv = document.createElement("canvas");
    cv.style.cssText = "position:absolute;inset:0;width:100%;height:100%;display:block";
    el.appendChild(cv);
    const ctx = cv.getContext("2d");
    const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const g = { cols: 0, rows: 0, cw: 0, ch: 0, ink: null, wake: null, font: "" };
    let W = 0, H = 0, sharks = [], koi = [], schools = [], raf = 0, last = 0, t = 0, alive = true;

    function layout() {
      const r = el.getBoundingClientRect(), dpr = window.devicePixelRatio || 1;
      if (!r.width || !r.height) return false;
      const fresh = !W;
      W = r.width; H = r.height;
      cv.width = Math.round(W * dpr); cv.height = Math.round(H * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const size = clamp(Math.round(W / 120), 9, 14);
      const mono = getComputedStyle(el).getPropertyValue("--font-mono").trim() || "monospace";
      g.font = `600 ${size}px ${mono}`;
      ctx.font = g.font;
      g.cw = ctx.measureText("M").width; g.ch = size * 1.15;
      g.cols = Math.ceil(W / g.cw); g.rows = Math.ceil(H / g.ch);
      g.ink = new Float32Array(g.cols * g.rows);
      g.wake = new Float32Array(g.cols * g.rows);
      if (fresh) populate();
      return true;
    }

    function populate() {
      const s = Math.min(W, H * 1.8);
      if (kind === "koi") {
        koi = [0.3, 0.26, 0.22, 0.2, 0.17].map(f => animal(PLANS.koi, s * f, rnd(0, W), rnd(H * 0.35, H), rnd(0, TAU), s * f * rnd(0.18, 0.28)));
      } else {
        sharks = [animal(PLANS.shark, s * 0.42, W * 0.65, H * 0.78, rnd(-0.3, 0.3), s * 0.06), animal(PLANS.shark, s * 0.26, W * 0.2, H * 0.55, Math.PI + rnd(-0.3, 0.3), s * 0.05)];
        schools = [0, 1, 2].map(() => {
          const cx = rnd(W * 0.1, W * 0.9), cy = rnd(H * 0.4, H * 0.95), h = rnd(0, TAU), n = Math.round(rnd(14, 24));
          return Array.from({ length: n }, () => ({ x: cx + rnd(-40, 40), y: cy + rnd(-20, 20), vx: Math.cos(h) * 40, vy: Math.sin(h) * 40 }));
        });
      }
    }

    function stepSchools(dt) {
      for (const fish of schools) {
        let cx = 0, cy = 0, ax = 0, ay = 0;
        for (const f of fish) { cx += f.x; cy += f.y; ax += f.vx; ay += f.vy; }
        cx /= fish.length; cy /= fish.length; ax /= fish.length; ay /= fish.length;
        for (const f of fish) {
          let fx = (cx - f.x) * 0.6 + (ax - f.vx) * 1.2 + rnd(-30, 30), fy = (cy - f.y) * 0.6 + (ay - f.vy) * 1.2 + rnd(-30, 30);
          for (const o of fish) {
            if (o === f) continue;
            const dx = f.x - o.x, dy = f.y - o.y, d2 = dx * dx + dy * dy, min = g.cw * 2.2;
            if (d2 < min * min && d2 > 0) { const d = Math.sqrt(d2); fx += (dx / d) * (min - d) * 30; fy += (dy / d) * (min - d) * 30; }
          }
          // Scatter from a shark's head.
          for (const s of sharks) {
            const [hx, hy] = s.spine[Math.floor(s.spine.length * 0.2)];
            const dx = f.x - hx, dy = f.y - hy, d = Math.hypot(dx, dy) || 1, r = s.len * 0.45;
            if (d < r) { fx += (dx / d) * (r - d) * 14; fy += (dy / d) * (r - d) * 14; }
          }
          // Back toward the window when wandering off it.
          if (f.x < 0) fx += 120; if (f.x > W) fx -= 120; if (f.y < H * 0.25) fy += 120; if (f.y > H) fy -= 120;
          f.vx += fx * dt; f.vy += fy * dt * 0.6;
          const sp = Math.hypot(f.vx, f.vy), max = 130, min = 30;
          if (sp > max) { f.vx *= max / sp; f.vy *= max / sp; } else if (sp < min) { f.vx *= min / sp; f.vy *= min / sp; }
          f.x += f.vx * dt; f.y += f.vy * dt;
        }
      }
    }

    function steer(dt) {
      // Sharks drift on a slow wander and now and then go for the nearest school.
      for (const s of sharks) {
        const [hx, hy] = s.spine[0];
        if (Math.random() < dt * 0.25) {
          let best = null, bd = Infinity;
          for (const fish of schools) {
            const f = fish[0], d = Math.hypot(f.x - hx, f.y - hy);
            if (d < bd) { bd = d; best = f; }
          }
          s.want = best && Math.random() < 0.6 ? Math.atan2(best.y - hy, best.x - hx) : s.want + rnd(-0.8, 0.8);
        }
        s.speed += ((s.cruise * (Math.abs(turn(s.heading, s.want)) > 0.6 ? 0.7 : 1)) - s.speed) * dt;
      }
      // Koi meander and keep out of one another's way.
      for (const k of koi) {
        if (Math.random() < dt * 0.4) k.want += rnd(-1, 1);
        const [hx, hy] = k.spine[0];
        for (const o of koi) {
          if (o === k) continue;
          const [ox, oy] = o.spine[3], d = Math.hypot(hx - ox, hy - oy);
          if (d < (k.len + o.len) * 0.35) k.want = Math.atan2(hy - oy, hx - ox);
        }
      }
    }

    function frame(now) {
      if (!alive) return;
      const dt = Math.min(0.05, (now - last) / 1000 || 0);
      last = now;
      // Hidden window, or the layer turned off: nothing to draw.
      if (!document.hidden && el.offsetParent !== null && (W || layout())) {
        if (dt > 0) {
          t += dt;
          steer(dt);
          for (const a of sharks.concat(koi)) stepAnimal(a, dt, t, { w: W, h: H });
          stepSchools(dt);
        }
        draw(dt);
      }
      if (!still) raf = requestAnimationFrame(frame);
    }

    function draw(dt) {
      g.ink.fill(0);
      // The wake: the tail of each shark stirs the cells it passes, and they settle.
      const fade = Math.pow(0.25, dt);
      for (let i = 0; i < g.wake.length; i++) g.wake[i] *= fade;
      for (const s of sharks) {
        const [x, y] = s.spine[s.spine.length - 1];
        const c = Math.floor(x / g.cw), r = Math.floor(y / g.ch);
        if (c >= 0 && r >= 0 && c < g.cols && r < g.rows) g.wake[r * g.cols + c] = Math.min(0.32, g.wake[r * g.cols + c] + 0.2);
      }
      for (let i = 0; i < g.wake.length; i++) g.ink[i] = g.wake[i];
      for (const a of sharks.concat(koi)) inkAnimal(a, g);
      ctx.clearRect(0, 0, W, H);
      ctx.font = g.font;
      ctx.textBaseline = "top";
      ctx.fillStyle = getComputedStyle(el).color;
      for (let r = 0; r < g.rows; r++) for (let c = 0; c < g.cols; c++) {
        const v = g.ink[r * g.cols + c];
        if (v < 0.06) continue;
        ctx.fillText(RAMP[Math.min(RAMP.length - 1, Math.ceil(v * (RAMP.length - 1)))], c * g.cw, r * g.ch);
      }
      // Small fish: one glyph each, pointing the way it swims.
      for (const fish of schools) for (const f of fish) {
        ctx.fillText(Math.abs(f.vx) < Math.abs(f.vy) * 0.5 ? "'" : f.vx > 0 ? ">" : "<", Math.floor(f.x / g.cw) * g.cw, Math.floor(f.y / g.ch) * g.ch);
      }
    }

    const ro = new ResizeObserver(() => { if (W) layout(); if (still) frame(performance.now()); });
    ro.observe(el);
    raf = requestAnimationFrame(frame);
    return () => { alive = false; cancelAnimationFrame(raf); ro.disconnect(); cv.width = cv.height = 0; el.innerHTML = ""; };
  }

  return { start };
})();
