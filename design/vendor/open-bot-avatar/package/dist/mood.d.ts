/**
 * Mood changes (idle, thinking, happy) as pure functions a frame loop steps:
 * the pose eases from the mood on screen into the new one (see blendPoses in
 * motion.ts), and a change during a change continues from where it is.
 */
import type { AvatarMotion, AvatarMotionBlend } from './types.ts';
/** Length of a mood change, ms. */
export declare const MOOD_MS = 420;
/** Eased progress of a mood change from elapsed time (smoothstep, clamped). */
export declare function moodProgress(elapsedMs: number): number;
/** A mood on screen, possibly still easing in over what came before. */
export interface MoodState {
    motion: AvatarMotion;
    blend?: AvatarMotionBlend;
}
/**
 * What a new mood change eases in from: the mood on screen with any change
 * still under way frozen at its progress, so the pose continues from exactly
 * where it is. Older changes whose share has all but faded (or beyond a few
 * levels) are dropped, so rapid clicking never grows the chain.
 */
export declare function settleMood(state: MoodState, share?: number, depth?: number): MoodState;
//# sourceMappingURL=mood.d.ts.map