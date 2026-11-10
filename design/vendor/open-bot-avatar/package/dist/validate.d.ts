import type { Bounds, Point } from './path.ts';
import type { AvatarEyes, ValidationResult } from './types.ts';
/** Neutral eye size at scale 1, viewBox units. */
export declare const EYE_W = 4;
export declare const EYE_H = 11;
/** Hard limits; the contract's soft target for the body is 8..92. */
export declare const BODY_MIN = 6;
export declare const BODY_MAX = 94;
/**
 * Max gaze travel of the eyes inside the face at scale 1. Deliberately small:
 * a subtle shift reads as looking around, and the enlarged eyes stay well
 * inside the face instead of sliding toward its edge.
 */
export declare const GAZE_X = 1.5;
export declare const GAZE_Y = 1.1;
/**
 * Happy-eye arc, as fractions of the drawn eye size (w = width, h = height):
 * a quadratic from (x ∓ halfWidth·w, y + base·h) through the control point
 * (x, y − peak·h), stroked `stroke`·w wide with round caps.
 */
export declare const HAPPY_ARC: {
    readonly halfWidth: 0.72;
    readonly base: 0.1;
    readonly peak: 0.42;
    readonly stroke: 0.6;
};
/** Gap the renderer keeps between every drawn eye shape and the outline while the eyes look around. */
export declare const EYE_CLEARANCE = 1.5;
export interface BodyHole {
    /** The hole's own subpath, serialized. */
    d: string;
    /** Approximate cut-out area, units². */
    area: number;
}
export interface BodySubpath {
    /** The subpath, serialized. */
    d: string;
    /** Its own flattened polygon(s). */
    polys: Point[][];
    /** True when the subpath cuts a hole rather than adding fill. */
    hole: boolean;
    /** A point inside the subpath's own fill (for holes: inside the cut-out). */
    probe: Point | null;
}
export interface BodyAnalysis {
    polys: Point[][];
    bounds: Bounds;
    area: number;
    holes: BodyHole[];
    subpaths: BodySubpath[];
}
/** Holes up to this area count as small details (buttons, sockets). The renderer cuts every hole through. */
export declare const RECESS_MAX_AREA = 110;
export interface GazeRange {
    left: number;
    right: number;
    up: number;
    down: number;
}
export declare function eyeCenters(eyes: AvatarEyes): [Point, Point];
export declare function eyesFit(polys: Point[][], eyes: AvatarEyes, dx: number, dy: number, margin: number): Point | null;
/** Like eyesFit, but for everything the renderer draws: the pill and the wider happy arc. */
export declare function drawnEyesFit(polys: Point[][], eyes: AvatarEyes, dx: number, dy: number, margin: number): Point | null;
/** Largest gap (viewBox units, to 0.1) between the drawn eyes at rest and the outline; 0 when they touch it. */
export declare function eyeClearance(polys: Point[][], eyes: AvatarEyes): number;
/**
 * How far (0..1 of GAZE_X/GAZE_Y) the drawn eyes can travel each way and keep
 * EYE_CLEARANCE from the outline.
 */
export declare function gazeRange(polys: Point[][], eyes: AvatarEyes): GazeRange;
export declare function analyzeBody(path: string): BodyAnalysis;
export declare function validateAvatar(input: unknown): ValidationResult;
//# sourceMappingURL=validate.d.ts.map