/**
 * The per-avatar state a live renderer keeps between frames, independent of
 * any platform: the eased look toward a target (follow.ts) and the eased
 * change between moods (mood.ts). Feed it once per frame; it returns the
 * `gaze` and `motionBlend` render options for that frame and whether it is
 * still easing.
 */
import { REST, followGaze, followStep, type FollowState } from './follow.ts';
import { moodProgress, settleMood, type MoodState } from './mood.ts';
import type { AvatarMotion, AvatarMotionBlend } from './types.ts';

export interface AnimatorInput {
  /** Wall clock, ms (e.g. performance.now()). Easing runs on it even when the animation clock stops. */
  now: number;
  motion: AvatarMotion;
  /** Look target, -1..1 per axis (x right, y down), e.g. from pointerGaze; null eases back to rest. */
  target: { x: number; y: number } | null;
  /** Keep the current look as it is (e.g. while the user drags the view). */
  hold?: boolean;
  /** Snap instead of easing. */
  reducedMotion: boolean;
}

export interface AnimatorStep {
  gaze?: { x: number; y: number; weight: number };
  motionBlend?: AvatarMotionBlend;
  /** True while the look or a mood change is still easing. */
  moving: boolean;
}

export interface Animator {
  step(input: AnimatorInput): AnimatorStep;
  /** Back to rest with no mood history, e.g. when the avatar is remounted. */
  reset(): void;
}

export function createAnimator(): Animator {
  let follow: FollowState = REST;
  let last: number | null = null;
  // The mood on screen and when its change began.
  let mood: (MoodState & { since: number }) | null = null;

  return {
    step({ now, motion, target, hold = false, reducedMotion }) {
      const dt = last === null ? 0 : (now - last) / 1000;
      last = now;
      let moving = false;
      if (!hold) ({ state: follow, moving } = followStep(follow, target, dt, reducedMotion));

      // A new mood eases in from the pose on screen (a change under way included); reduced motion switches at once.
      if (mood && motion !== mood.motion) {
        const shown: MoodState = { motion: mood.motion, blend: mood.blend && { from: mood.blend.from, t: moodProgress(now - mood.since) } };
        mood = { motion, blend: reducedMotion ? undefined : { from: settleMood(shown), t: 0 }, since: now };
      }
      mood ??= { motion, since: now };
      const t = reducedMotion ? 1 : moodProgress(now - mood.since);
      let motionBlend: AvatarMotionBlend | undefined;
      if (mood.blend && t < 1) motionBlend = { from: mood.blend.from, t };
      else mood.blend = undefined;

      return { gaze: followGaze(follow), motionBlend, moving: moving || motionBlend !== undefined };
    },
    reset() {
      follow = REST;
      last = null;
      mood = null;
    },
  };
}
