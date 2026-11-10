import type { Bounds, Point } from './path.ts';
import type { BodyAnalysis, GazeRange } from './validate.ts';
import type { AvatarDefinition, AvatarEyes } from './types.ts';
export type PartKind = 'inflate' | 'revolve-x' | 'revolve-y';
export interface VolumeSlice {
    /** 'z': the plane z = at, path in (x, y). 'x': the plane x = at, path in (u = -z, y). */
    axis: 'z' | 'x';
    at: number;
    d: string;
    /** The same cross-section as polygons in its plane's coordinates (`d` is a rounded serialization of them). */
    loops: Point[][];
    /** Contour paths use evenodd; the core slice reuses the original nonzero outline. */
    evenOdd: boolean;
    /** Path bounds in its plane's coordinates. */
    box: {
        x: number;
        y: number;
        w: number;
        h: number;
    };
    /** z-slices: smallest distance from this cross-section to the outline (0 for the core and walls). */
    inset: number;
}
export interface VolumePart {
    kind: PartKind;
    /** Outline plus the holes that cut it, serialized. */
    d: string;
    polys: Point[][];
    bounds: Bounds;
    center: Point;
    /** Twice the largest inscribed radius (inflate) or the largest diameter (revolved). */
    thickness: number;
    /** Front-to-back extent, centered on z = 0. */
    depth: number;
    /** Inflate only: elliptical edge fillet radii (in-plane and in depth). */
    rxy: number;
    rz: number;
    /** Revolved parts: the axis (y for revolve-x, x for revolve-y). */
    axis?: number;
    zSlices: VolumeSlice[];
    xSlices: VolumeSlice[];
    /** Front surface height at a point, 0 outside the part. */
    height: (x: number, y: number) => number;
}
export interface Volume {
    parts: VolumePart[];
    bounds: Bounds;
    /** Index of the part carrying the eyes. */
    eyePart: number;
    /** Depth of each eye's plane: the highest surface under the eye and its gaze travel. */
    eyeZ: [number, number];
    /** Drawn eye scale: the definition's scale enlarged as far as the body allows (see EYE_BOOST). */
    eyeScale: number;
    /**
     * Host surface normals under each eye (left edge, center, right edge). On a
     * curved host an eye is hidden once any of them turns away, so a flat eye
     * never hangs past the curved silhouette.
     */
    eyeNormals: [Vec3[], Vec3[]];
}
export type Vec3 = [number, number, number];
/** Largest depth an inflated part may take, viewBox units. */
export declare const MAX_PART_DEPTH = 30;
/** Drawn eyes are up to this much larger than the contract's 4 × 11 pill when the body has room. */
export declare const EYE_BOOST = 1.45;
/**
 * Gap every drawn eye shape (pill and happy arc) keeps from the outline at rest
 * before the renderer enlarges it, viewBox units. Below this the eyes stay at
 * the definition's own scale.
 */
export declare const EYE_BOOST_PAD = 3;
/**
 * Design target for generated avatars: the enlarged eyes keep this much
 * clearance at rest, which also leaves full gaze travel with EYE_CLEARANCE.
 * Advisory (validate.sh --strict); v1 imports below it still render.
 */
export declare const FACE_ROOM_TARGET = 4.5;
/** Distance from a point to the nearest edge of the polygons. */
export declare function edgeDistance(polys: Point[][], x: number, y: number): number;
/** Inward distance to the outline, or 0 outside the fill. */
export declare function insideDistance(polys: Point[][], x: number, y: number): number;
/** Outline erosion of an inflated part at depth z (elliptical fillet). */
export declare function erosionAt(part: Pick<VolumePart, 'depth' | 'rxy' | 'rz'>, z: number): number;
/** Front surface depth of an inflated part at a point `dist` units inside the outline. */
export declare function surfaceZ(part: Pick<VolumePart, 'depth' | 'rxy' | 'rz'>, dist: number): number;
/**
 * Depth heuristic for inflated parts: as deep as the part is thick, but tall
 * slender parts get up to 2.4x their thickness so they read as discs.
 */
export declare function partDepth(thickness: number, b: Bounds, volume: number): number;
/**
 * Horizontal axis of revolution, if the body looks like a turned object seen
 * from the side: at least two parts, wider than tall, mirror-symmetric about
 * y = axis and with every part centered on it.
 */
export declare function revolveAxis(body: BodyAnalysis): number | null;
/** Build the implied volume. `volume` scales every depth (0 = flat). Pass `body` to reuse an analysis. */
export declare function buildVolume(def: AvatarDefinition, volume?: number, body?: BodyAnalysis): Volume;
/** Largest eye scale up to EYE_BOOST × the definition's whose drawn eyes keep EYE_BOOST_PAD from the outline. */
export declare function boostedEyeScale(body: BodyAnalysis, eyes: AvatarEyes): number;
export interface FaceRoom {
    /** Drawn eye size relative to the definition's (1..EYE_BOOST). */
    boost: number;
    /** Gap between the drawn eyes at rest (pill and happy arc) and the outline, viewBox units. */
    clearance: number;
    /** Gaze travel left at the drawn size (see gazeRange). */
    gaze: GazeRange;
    /** True when the face meets the design target: full boost, FACE_ROOM_TARGET clearance, full gaze. */
    ok: boolean;
}
/** How much room the face has as the renderer will actually draw it. */
export declare function faceRoom(def: AvatarDefinition, body?: BodyAnalysis): FaceRoom;
//# sourceMappingURL=volume.d.ts.map