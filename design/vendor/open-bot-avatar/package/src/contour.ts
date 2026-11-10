/**
 * Iso-contours of a sampled scalar field (marching squares), assembled into
 * closed loops and simplified. Used once per definition to turn a part's
 * depth field into plain cross-section paths, so slices need no SVG masks.
 */
import type { Point } from './path.ts';

export interface Field {
  /** Row-major samples, nx * ny. The outermost ring should be below every level. */
  values: Float64Array;
  nx: number;
  ny: number;
  x0: number;
  y0: number;
  step: number;
}

/** Closed loops where the field crosses `level` (inside = value ≥ level). */
export function contourLoops(field: Field, level: number): Point[][] {
  const { values, nx, ny, x0, y0, step } = field;
  const at = (i: number, j: number) => values[j * nx + i] - level;
  // Edge keys: horizontal edge (i,j)-(i+1,j) = 2*(j*nx+i), vertical (i,j)-(i,j+1) = 2*(j*nx+i)+1.
  const points = new Map<number, Point>();
  const links = new Map<number, number[]>();
  const point = (key: number): void => {
    if (points.has(key)) return;
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
  const link = (p: number, q: number) => {
    point(p); point(q);
    (links.get(p) ?? links.set(p, []).get(p)!).push(q);
    (links.get(q) ?? links.set(q, []).get(q)!).push(p);
  };

  for (let j = 0; j < ny - 1; j++) {
    for (let i = 0; i < nx - 1; i++) {
      const tl = at(i, j) >= 0, tr = at(i + 1, j) >= 0, br = at(i + 1, j + 1) >= 0, bl = at(i, j + 1) >= 0;
      const code = (tl ? 8 : 0) | (tr ? 4 : 0) | (br ? 2 : 0) | (bl ? 1 : 0);
      if (code === 0 || code === 15) continue;
      const top = 2 * (j * nx + i), bottom = 2 * ((j + 1) * nx + i);
      const left = 2 * (j * nx + i) + 1, right = 2 * (j * nx + i + 1) + 1;
      if (code === 10 || code === 5) {
        const center = (at(i, j) + at(i + 1, j) + at(i + 1, j + 1) + at(i, j + 1)) / 4 >= 0;
        // Diagonal corners: the center decides whether they connect through the cell.
        if ((code === 10) === center) { link(top, right); link(bottom, left); }
        else { link(left, top); link(right, bottom); }
        continue;
      }
      const crossed: number[] = [];
      if (tl !== tr) crossed.push(top);
      if (tr !== br) crossed.push(right);
      if (br !== bl) crossed.push(bottom);
      if (bl !== tl) crossed.push(left);
      link(crossed[0], crossed[1]);
    }
  }

  const loops: Point[][] = [];
  const seen = new Set<number>();
  for (const start of links.keys()) {
    if (seen.has(start)) continue;
    const loop: Point[] = [];
    let prev = -1, cur = start;
    while (!seen.has(cur)) {
      seen.add(cur);
      loop.push(points.get(cur)!);
      const next = links.get(cur)!.find((n) => n !== prev && !seen.has(n));
      if (next === undefined) break;
      prev = cur; cur = next;
    }
    if (loop.length >= 3) loops.push(loop);
  }
  return loops;
}

function perpendicular(p: Point, a: Point, b: Point): number {
  const dx = b[0] - a[0], dy = b[1] - a[1];
  const len = Math.hypot(dx, dy);
  if (len === 0) return Math.hypot(p[0] - a[0], p[1] - a[1]);
  return Math.abs(dx * (a[1] - p[1]) - dy * (a[0] - p[0])) / len;
}

/** Ramer-Douglas-Peucker on a closed loop. */
export function simplifyLoop(loop: Point[], tolerance: number): Point[] {
  if (loop.length <= 8) return loop;
  // Split at the point farthest from the first so the closed loop becomes two open chains.
  let far = 0, best = -1;
  for (let k = 1; k < loop.length; k++) {
    const d = Math.hypot(loop[k][0] - loop[0][0], loop[k][1] - loop[0][1]);
    if (d > best) { best = d; far = k; }
  }
  const keep = new Uint8Array(loop.length);
  keep[0] = keep[far] = 1;
  const stack: [number, number][] = [[0, far], [far, loop.length]];
  while (stack.length) {
    const [s, e] = stack.pop()!;
    const end = loop[e % loop.length];
    let idx = -1, max = tolerance;
    for (let k = s + 1; k < e; k++) {
      const d = perpendicular(loop[k], loop[s], end);
      if (d > max) { max = d; idx = k; }
    }
    if (idx >= 0) { keep[idx] = 1; stack.push([s, idx], [idx, e]); }
  }
  return loop.filter((_, k) => keep[k]);
}

const n1 = (v: number) => {
  const r = Math.round(v * 10) / 10;
  return (Object.is(r, -0) ? 0 : r).toString();
};

/** Compact closed path from loops (draw with fill-rule evenodd). */
export function loopsToPath(loops: Point[][]): string {
  return loops
    .map((loop) => `M${n1(loop[0][0])} ${n1(loop[0][1])}L${loop.slice(1).map((p) => `${n1(p[0])} ${n1(p[1])}`).join(' ')}Z`)
    .join('');
}
