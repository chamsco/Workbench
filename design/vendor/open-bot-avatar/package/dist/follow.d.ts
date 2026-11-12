/**
 * Cursor following, as pure functions the frame loop steps once per frame.
 *
 * The pointer is mapped to a look direction from the avatar's eyes (not its
 * box center, so a pointer level with the eyes reads as level): radial
 * (diagonals are no stronger than straight lines) and soft, so it uses most of
 * the (small) range across the stage around the avatar and saturates gently
 * beyond it. `attention` fades the whole response in and out, so the eyes and
 * the body's turn hand over to idle motion without a jump.
 */
export interface FollowState {
    x: number;
    y: number;
    /** 0 = at rest (idle wander, resting turn), 1 = fully following the pointer. */
    attention: number;
}
export declare const REST: FollowState;
/**
 * Look direction (-1..1, x right, y down) toward a viewport point, from the
 * avatar's bounding box. `origin` is where the look starts, as a fraction of
 * the box (the eyes; default the center).
 */
export declare function pointerGaze(pointer: {
    x: number;
    y: number;
}, box: {
    left: number;
    top: number;
    width: number;
    height: number;
}, origin?: {
    x: number;
    y: number;
}): {
    x: number;
    y: number;
};
/**
 * One easing step toward `target` (null = not following: back to rest).
 * Frame-rate independent; reduced motion snaps. Returns the next state and
 * whether it still needs frames.
 */
export declare function followStep(state: FollowState, target: {
    x: number;
    y: number;
} | null, dt: number, reduced: boolean): {
    state: FollowState;
    moving: boolean;
};
/**
 * The pose's look target for a follow state, nothing at all once at rest.
 * Attention eased through a smoothstep is the weight, so the turn starts and
 * ends gently rather than at full speed.
 */
export declare function followGaze(state: FollowState): {
    x: number;
    y: number;
    weight: number;
} | undefined;
//# sourceMappingURL=follow.d.ts.map