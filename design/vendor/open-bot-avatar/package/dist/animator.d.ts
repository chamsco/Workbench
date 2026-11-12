import type { AvatarMotion, AvatarMotionBlend } from './types.ts';
export interface AnimatorInput {
    /** Wall clock, ms (e.g. performance.now()). Easing runs on it even when the animation clock stops. */
    now: number;
    motion: AvatarMotion;
    /** Look target, -1..1 per axis (x right, y down), e.g. from pointerGaze; null eases back to rest. */
    target: {
        x: number;
        y: number;
    } | null;
    /** Keep the current look as it is (e.g. while the user drags the view). */
    hold?: boolean;
    /** Snap instead of easing. */
    reducedMotion: boolean;
}
export interface AnimatorStep {
    gaze?: {
        x: number;
        y: number;
        weight: number;
    };
    motionBlend?: AvatarMotionBlend;
    /** True while the look or a mood change is still easing. */
    moving: boolean;
}
export interface Animator {
    step(input: AnimatorInput): AnimatorStep;
    /** Back to rest with no mood history, e.g. when the avatar is remounted. */
    reset(): void;
}
export declare function createAnimator(): Animator;
//# sourceMappingURL=animator.d.ts.map