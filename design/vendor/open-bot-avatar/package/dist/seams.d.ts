import type { Point } from './path.ts';
export type SeamSide = 'left' | 'right' | 'top' | 'bottom';
export interface Seam {
    side: SeamSide;
    /** Where along the side the step sits (y for left/right, x for top/bottom). */
    at: number;
    /** Step height, viewBox units: positive steps outward, negative inward (walking down or right). */
    size: number;
}
/** Steps below this are invisible; from this up to SEAM_MAX they read as accidental. */
export declare const SEAM_MIN = 0.3;
/** Steps at least this tall read as a deliberate, structural step. */
export declare const SEAM_MAX = 2.5;
/** Accidental steps in the body's outer silhouette (see the module comment). */
export declare function silhouetteSeams(polys: Point[][]): Seam[];
//# sourceMappingURL=seams.d.ts.map