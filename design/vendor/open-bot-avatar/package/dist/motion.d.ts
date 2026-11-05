/**
 * Deterministic pose model. A pose is a pure function of (seed, time, motion,
 * gaze, reducedMotion): no wall clock, no Math.random, so any frame can be
 * reproduced exactly (tests, SSR, scrubbing).
 */
import type { AvatarMotion, AvatarMotionBlend } from './types.ts';
export interface Pose {
    /** View rotation in radians: yaw turns about the vertical axis, pitch about the horizontal. */
    yaw: number;
    pitch: number;
    /** In-plane rotation, degrees. */
    roll: number;
    /** Vertical offset (negative = up), viewBox units. */
    lift: number;
    /** Squash and stretch about the base of the body. */
    sx: number;
    sy: number;
    /** Eye look direction, -1..1 per axis. */
    gazeX: number;
    gazeY: number;
    /** 0 = open, 1 = fully closed. */
    blink: number;
    /** Extra lid narrowing per eye, 0..1. */
    squintL: number;
    squintR: number;
    eyeShape: 'pill' | 'happy';
}
/** Resting view: turned a little and seen a little from above, so the volume reads without dominating. */
export declare const BASE_YAW = -0.3;
export declare const BASE_PITCH = 0.2;
/**
 * Following a look target: the body turns toward it by up to these angles
 * (radians, at full deflection) and the eyes use this share of their small
 * range, so the whole character looks rather than the eyes sliding alone.
 */
export declare const LOOK_YAW = 0.32;
export declare const LOOK_PITCH = 0.18;
export declare const LOOK_EYES = 0.7;
export declare function hashString(s: string): number;
/** Stateless hash noise in [0, 1). */
export declare function noise(seed: number, n: number): number;
/** Blink closure 0..1: one blink per ~4.2 s slot at a seeded offset, sometimes a double blink. */
export declare function blinkAt(seed: number, t: number): number;
/** Idle eye wander: short holds between quick saccades, centered more often than not. */
export declare function wanderAt(seed: number, t: number): [number, number];
export interface PoseInput {
    seed: number;
    time: number;
    motion: AvatarMotion;
    /** Look target, -1..1 per axis (x right, y down); `weight` 0..1 fades it in over the idle pose (default 1). */
    gaze?: {
        x: number;
        y: number;
        weight?: number;
    };
    reducedMotion: boolean;
    /** Extra view rotation in radians (e.g. from drag-to-turn), added to the resting view. */
    view?: {
        yaw: number;
        pitch: number;
    };
    /** Easing in `motion` from the mood shown before (see blendPoses). */
    blend?: AvatarMotionBlend;
}
/**
 * Pose part way from `a` to `b`, both taken at the same moment so either
 * side's motion keeps running. A change of eye shape swaps halfway, inside a
 * blink: pills close toward the swap and open after it.
 */
export declare function blendPoses(a: Pose, b: Pose, t: number): Pose;
export declare function computePose(input: PoseInput): Pose;
//# sourceMappingURL=motion.d.ts.map