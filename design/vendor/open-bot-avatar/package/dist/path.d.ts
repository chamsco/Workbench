/**
 * Restricted SVG path geometry: parser, normalizer and flattener.
 *
 * Accepts the path command set M L H V C S Q T A Z (absolute and relative)
 * and nothing else. The parsed result is re-serialized from numbers, so the
 * renderer never echoes model-written text into markup.
 */
export type Segment = {
    c: 'M';
    x: number;
    y: number;
} | {
    c: 'L';
    x: number;
    y: number;
} | {
    c: 'C';
    x1: number;
    y1: number;
    x2: number;
    y2: number;
    x: number;
    y: number;
} | {
    c: 'Q';
    x1: number;
    y1: number;
    x: number;
    y: number;
} | {
    c: 'A';
    rx: number;
    ry: number;
    rot: number;
    large: 0 | 1;
    sweep: 0 | 1;
    x: number;
    y: number;
} | {
    c: 'Z';
};
export type Point = [number, number];
export declare const PATH_LIMITS: {
    readonly maxChars: 6000;
    readonly maxSegments: 400;
    readonly maxSubpaths: 16;
    readonly coordMin: -20;
    readonly coordMax: 120;
};
export declare class PathError extends Error {
}
/** Parse and convert to absolute M/L/C/Q/A/Z segments. Throws PathError. */
export declare function parsePath(d: string): Segment[];
/** Compact absolute serialization built only from parsed numbers. */
export declare function serializePath(segs: Segment[]): string;
/** Flatten into closed polygons, one per subpath. */
export declare function flattenPath(segs: Segment[]): Point[][];
/** Nonzero winding number of a point against all subpaths (SVG default fill rule). */
export declare function winding(polys: Point[][], x: number, y: number): number;
export declare function isInside(polys: Point[][], x: number, y: number): boolean;
/**
 * Nonzero inside flags for many points on one row (xs ascending), from a
 * single pass over the edges instead of one winding test per point.
 */
export declare function insideRow(polys: Point[][], y: number, xs: number[], out: Uint8Array): void;
export interface Bounds {
    minX: number;
    minY: number;
    maxX: number;
    maxY: number;
}
export declare function bounds(polys: Point[][]): Bounds;
/** Filled area estimated on a 1-unit sample grid (nonzero rule). */
export declare function filledArea(polys: Point[][], b?: Bounds): number;
/** Split normalized segments into one segment list per subpath. */
export declare function splitSubpaths(segs: Segment[]): Segment[][];
//# sourceMappingURL=path.d.ts.map