import type { Point } from './path.ts';
import type { Volume } from './volume.ts';
import type { AvatarDefinition, RenderOptions } from './types.ts';
export declare const DEFAULT_COLOR = "#ff6a1a";
export declare const DEFAULT_EYE_COLOR = "#151515";
/** `depth` option value that maps to the natural inferred volume. */
export declare const DEFAULT_DEPTH = 6;
/** The validated definition and its cached volume, exactly as the renderers use them. Throws if invalid. */
export declare function preparedVolume(definition: AvatarDefinition, depth?: number): {
    def: AvatarDefinition;
    vol: Volume;
};
type M = Matrix;
export interface ViewBasis {
    cY: number;
    sY: number;
    cP: number;
    sP: number;
    /** cos(yaw)·cos(pitch): > 0 when the front faces the viewer. */
    facing: number;
}
export declare function viewBasis(yaw: number, pitch: number): ViewBasis;
/** Toward-viewer component of a model-space direction under this view (1 = straight at the viewer). */
export declare function facingOf(v: ViewBasis, [x, y, z]: [number, number, number]): number;
/** Which slice stack faces the viewer more: front-facing z-planes or side-facing x-planes. */
export declare function activeStack(v: ViewBasis): 'z' | 'x';
/**
 * Orthographic projection of the plane at depth z, pivoting on `pivot` at z = 0.
 * Yaw turns about the vertical axis, then pitch tips about the horizontal axis.
 */
export declare function planeMatrix(v: ViewBasis, z: number, pivot: Point): M;
/** Projection of the side plane x = x0, whose local coordinates are (u = -z, y). */
export declare function sidePlaneMatrix(v: ViewBasis, x0: number, pivot: Point): M;
/** Project a 3D point: screen x, screen y and depth toward the viewer. */
export declare function projectPoint(v: ViewBasis, [x, y, z]: [number, number, number], pivot: Point): [number, number, number];
/** 2D affine matrix in SVG order (a b c d e f). */
export type Matrix = [number, number, number, number, number, number];
/** How one eye is drawn, in its face plane's coordinates (viewBox units). */
export type EyeMark = {
    shape: 'pill';
    x: number;
    y: number;
    width: number;
    height: number;
    radius: number;
} | {
    shape: 'arc';
    d: string;
    strokeWidth: number;
};
export type FrameShape = {
    kind: 'slice';
    /** Stable across frames for the same cross-section: use it as a key in retained renderers. */
    key: string;
    /** Id of an entry in AvatarFrame.paths. */
    path: string;
    matrix: Matrix;
} | {
    kind: 'eye';
    key: 'eye-0' | 'eye-1';
    matrix: Matrix;
    mark: EyeMark;
};
/**
 * One frame of the avatar flattened onto the screen: every cross-section the
 * view draws with its projection, and the visible eyes, sorted back to front.
 * Painting them in order (body paths in `color`, eyes in `ink`) inside the
 * `root` transform and a 0 0 100 100 viewBox gives the avatar exactly as
 * renderAvatarSvg draws it. Plain numbers and path strings, so any vector
 * renderer (SVG, canvas, Skia, react-native-svg) can draw it.
 */
export interface AvatarFrame {
    name: string;
    color: string;
    ink: string;
    /** Distinct cross-section paths this frame uses; ids are stable for equal geometry and depth. */
    paths: {
        id: string;
        d: string;
        evenOdd: boolean;
    }[];
    /** 2D motion (hop, roll, squash) about the base of the body, applied to every shape. */
    root: Matrix;
    /** Back to front. */
    shapes: FrameShape[];
}
/** Compute one flat frame of a definition. Throws if the definition is invalid. */
export declare function avatarFrame(definition: AvatarDefinition, options?: RenderOptions): AvatarFrame;
/** Render a definition to a standalone SVG string. Throws if the definition is invalid. */
export declare function renderAvatarSvg(definition: AvatarDefinition, options?: RenderOptions): string;
/**
 * Static markup of the live scene for a definition and depth. Mount it once,
 * then apply `sceneFrame` every frame. Body color comes from CSS `color` on
 * an ancestor, eye ink from `--avatar-ink`.
 */
export declare function renderAvatarScene(definition: AvatarDefinition, options?: Pick<RenderOptions, 'depth'>): string;
export interface SceneFrame {
    /** Identifies the static markup this frame belongs to. */
    scene: string;
    /** 2D motion (hop, roll, squash) about the base of the body. */
    motion: string;
    /** 3D view rotation about the body center. */
    view: string;
    stack: 'z' | 'x';
    /** Per slice of the active stack, in markup order: whether it shows. Hidden ones need no layer. */
    slices: boolean[];
    eyes: {
        visible: boolean;
        happy: boolean;
        transform: string;
        pill: string;
    }[];
}
/** Per-frame CSS for a mounted scene. Pure and cheap: no geometry is rebuilt. */
export declare function sceneFrame(definition: AvatarDefinition, options?: RenderOptions): SceneFrame;
export {};
//# sourceMappingURL=render.d.ts.map