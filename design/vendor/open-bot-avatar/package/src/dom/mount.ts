/**
 * A live avatar in any DOM element, without a framework.
 *
 *   const avatar = mountAvatar(el, { definition, color: '#1f7cf2', followPointer: true });
 *   avatar.update({ mood: 'happy' });
 *   avatar.destroy();
 *
 * Live avatars are a CSS 3D scene: every cross-section of the inferred volume
 * is mounted once as its own small SVG, and each frame only writes a few CSS
 * transforms (view, hop/roll/squash, the eyes' gaze and blink). Changing the
 * color only changes CSS `color`. `still` avatars are a single static SVG and
 * never animate. Every avatar on the page shares one animation loop.
 */
import { createAnimator } from '../animator.ts';
import { pointerGaze } from '../follow.ts';
import { DEFAULT_COLOR, DEFAULT_EYE_COLOR, renderAvatarScene, renderAvatarSvg, sceneFrame, type SceneFrame } from '../render.ts';
import type { AvatarDefinition, AvatarMotion, RenderOptions } from '../types.ts';
import { requestFrame, subscribeFrames } from './clock.ts';
import { trackPointer } from './pointer.ts';
import { prefersReducedMotion } from './reduced.ts';

export interface AvatarOptions {
  definition: AvatarDefinition;
  /** Body color, #rgb or #rrggbb. */
  color?: string;
  /** Eye color, #rgb or #rrggbb. Defaults to a near-black ink. */
  eyeColor?: string;
  /** Volume amount 0..14 (6 = natural depth, 0 = flat). */
  depth?: number;
  mood?: AvatarMotion;
  /** Eyes look and the body turns toward the mouse, pen or finger anywhere on the page. */
  followPointer?: boolean;
  /** Fixed look direction (-1..1 per axis, x right, y down). Wins over the pointer. */
  lookAt?: { x: number; y: number } | null;
  /** Keep the current look as it is (e.g. while the user drags the view). */
  holdGaze?: boolean;
  /** Extra view rotation in radians on top of the resting view (e.g. drag-to-turn). */
  view?: { yaw: number; pitch: number };
  /** true or false to force; 'system' (default) follows prefers-reduced-motion. */
  reducedMotion?: boolean | 'system';
  /** Render one static SVG and never animate (thumbnails, lists). */
  still?: boolean;
  /** Accessible name; defaults to the definition's name. "" marks the avatar decorative. */
  label?: string;
}

export interface MountedAvatar {
  /** Change any options; only what changed is redrawn. */
  update(options: Partial<AvatarOptions>): void;
  destroy(): void;
}

/** Handles into a mounted scene, and the last values written, so frames only touch what changed. */
interface Scene {
  key: string;
  geometry: string;
  motion: HTMLElement;
  view: HTMLElement;
  stacks: Record<'z' | 'x', HTMLElement>;
  slices: Record<'z' | 'x', HTMLElement[]>;
  eyes: { root: HTMLElement; pill: HTMLElement; arc: HTMLElement }[];
  last: Record<string, string>;
}

function mountScene(el: HTMLElement, definition: AvatarDefinition, depth: number | undefined): Scene {
  el.innerHTML = renderAvatarScene(definition, { depth });
  const q = (s: string) => el.querySelector(s) as HTMLElement;
  const stacks = { z: q('[data-stack="z"]'), x: q('[data-stack="x"]') };
  return {
    key: q('[data-avatar-scene]').dataset.avatarScene ?? '',
    geometry: geometryKey(definition),
    motion: q('[data-motion]'),
    view: q('[data-view]'),
    stacks,
    slices: { z: [...stacks.z.children] as HTMLElement[], x: [...stacks.x.children] as HTMLElement[] },
    eyes: [0, 1].map((i) => {
      const root = q(`[data-eye="${i}"]`);
      return { root, pill: root.querySelector('i') as HTMLElement, arc: root.querySelector('svg') as unknown as HTMLElement };
    }),
    last: {},
  };
}

/** Write a style property only when its value changed since the last frame. */
function put(m: Scene, key: string, el: HTMLElement, prop: 'transform' | 'visibility' | 'display', value: string) {
  if (m.last[key] === value) return;
  m.last[key] = value;
  el.style[prop] = value;
}

function apply(m: Scene, frame: SceneFrame) {
  put(m, 'motion', m.motion, 'transform', frame.motion);
  put(m, 'view', m.view, 'transform', frame.view);
  // display:none, not visibility: hidden slices would still each hold a 3D layer.
  put(m, 'z', m.stacks.z, 'display', frame.stack === 'z' ? 'block' : 'none');
  put(m, 'x', m.stacks.x, 'display', frame.stack === 'x' ? 'block' : 'none');
  const slices = m.slices[frame.stack];
  frame.slices.forEach((shown, i) => slices[i] && put(m, `${frame.stack}${i}`, slices[i], 'display', shown ? 'block' : 'none'));
  frame.eyes.forEach((eye, i) => {
    const e = m.eyes[i];
    put(m, `e${i}`, e.root, 'transform', eye.transform);
    put(m, `v${i}`, e.root, 'visibility', eye.visible ? 'visible' : 'hidden');
    put(m, `p${i}`, e.pill, 'transform', eye.pill);
    put(m, `ph${i}`, e.pill, 'display', eye.happy ? 'none' : 'block');
    put(m, `a${i}`, e.arc, 'display', eye.happy ? 'block' : 'none');
  });
}

const keys = new WeakMap<object, string>();
/** Everything in a definition the static scene markup depends on. */
function geometryKey(definition: AvatarDefinition): string {
  let key = keys.get(definition);
  if (key === undefined) {
    key = JSON.stringify([definition?.id, definition?.name, definition?.body?.path, definition?.eyes]);
    keys.set(definition, key);
  }
  return key;
}

/** Midpoint between the eyes as a fraction of the (square, 100-unit) box: the look starts there. */
function eyesOrigin(definition: AvatarDefinition): { x: number; y: number } {
  const { x, y } = definition.eyes ?? {};
  const at = (v: unknown) => (typeof v === 'number' && Number.isFinite(v) ? Math.min(1, Math.max(0, v / 100)) : 0.5);
  return { x: at(x), y: at(y) };
}

/** Avatars smaller than this (CSS px) redraw at SMALL_FPS: a frame of 3D layers costs the same at any size. */
const SMALL_SIZE = 128;
const SMALL_FPS = 30;
/** Slack so a 60 Hz display reliably draws every other frame rather than drifting between 1 and 3. */
const FRAME_SLACK = 0.004;

export function mountAvatar(host: HTMLElement, initial: AvatarOptions): MountedAvatar {
  let opts: AvatarOptions = { ...initial };
  const animator = createAnimator();
  let rect: DOMRect | null = null;
  let scene: Scene | null = null;
  let lastStill = '';
  let failed = false;
  let dirty = true;
  let releasePointer: (() => void) | null = null;
  let size = host.clientWidth;
  const resize = typeof ResizeObserver === 'undefined'
    ? null
    : new ResizeObserver(([entry]) => {
        const next = entry.contentRect.width;
        if (next === size) return;
        size = next;
        requestFrame();
      });
  resize?.observe(host);

  if (getComputedStyle(host).position === 'static') host.style.position = 'relative';

  const syncHost = () => {
    host.style.color = opts.color ?? DEFAULT_COLOR;
    host.style.setProperty('--avatar-ink', opts.eyeColor ?? DEFAULT_EYE_COLOR);
    const decorative = opts.label === '';
    if (decorative) {
      host.removeAttribute('role');
      host.removeAttribute('aria-label');
      host.setAttribute('aria-hidden', 'true');
    } else {
      host.setAttribute('role', 'img');
      host.setAttribute('aria-label', opts.label ?? opts.definition?.name ?? 'Avatar');
      host.removeAttribute('aria-hidden');
    }
    const wantsPointer = !!opts.followPointer && !opts.still;
    if (wantsPointer && !releasePointer) releasePointer = trackPointer();
    else if (!wantsPointer && releasePointer) {
      releasePointer();
      releasePointer = null;
    }
  };

  const clear = () => {
    host.textContent = '';
    scene = null;
    lastStill = '';
  };

  const draw = (time: number, pointer: { x: number; y: number } | null): boolean => {
    const p = opts;
    const reducedMotion = p.reducedMotion === 'system' || p.reducedMotion === undefined ? prefersReducedMotion() : p.reducedMotion;
    const now = performance.now();
    try {
      if (p.still) {
        if (scene) clear();
        const svg = renderAvatarSvg(p.definition, {
          color: p.color,
          eyeColor: p.eyeColor,
          depth: p.depth,
          motion: p.mood,
          view: p.view,
          gaze: p.lookAt ?? undefined,
          reducedMotion: true,
        });
        // The engine derives SVG ids from geometry, so repeated avatars can share them safely.
        if (svg !== lastStill) {
          host.innerHTML = svg;
          lastStill = svg;
        }
        failed = false;
        return false;
      }
      if (lastStill) clear();

      const target = p.lookAt
        ? p.lookAt
        : p.followPointer && pointer && rect && rect.width > 0
          ? pointerGaze(pointer, rect, eyesOrigin(p.definition))
          : null;
      const step = animator.step({ now, motion: p.mood ?? 'idle', target, hold: p.holdGaze, reducedMotion });
      const options: RenderOptions = {
        depth: p.depth,
        motion: p.mood ?? 'idle',
        motionBlend: step.motionBlend,
        size: size || undefined,
        time,
        gaze: step.gaze,
        view: p.view,
        reducedMotion,
      };
      const frame = sceneFrame(p.definition, options);
      if (!scene || scene.key !== frame.scene || scene.geometry !== geometryKey(p.definition) || !host.contains(scene.view)) {
        scene = mountScene(host, p.definition, p.depth);
      }
      apply(scene, frame);
      failed = false;
      return !reducedMotion || step.moving;
    } catch (error) {
      if (!failed) {
        clear();
        console.error('open-bot-avatar: render failed', error);
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
    draw: (time, pointer) => {
      // Stills only redraw after a change; live avatars every frame they ask for.
      if (opts.still && !dirty) return false;
      dirty = false;
      const more = draw(time, pointer);
      return more && size > 0 && size < SMALL_SIZE ? 1 / SMALL_FPS - FRAME_SLACK : more;
    },
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
    },
  };
}
