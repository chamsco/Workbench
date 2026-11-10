/**
 * One shared animation loop for every live avatar on the page.
 *
 * Subscribers measure first and draw second, so layout reads never interleave
 * with DOM writes. The loop sleeps once no subscriber needs frames (every
 * avatar is still, or under reduced motion with its easing settled).
 * Subscribers that ask to wait are skipped until their time comes.
 */
export interface Pointer {
    x: number;
    y: number;
}
export interface FrameSubscriber {
    measure?: () => void;
    /**
     * `time` is the animation clock in seconds, `pointer` the last pointer
     * position in viewport pixels (null when there is none). Return true for
     * the next frame, a number of seconds to skip (neither measured nor drawn
     * until then), or false once no more frames are needed.
     */
    draw: (time: number, pointer: Pointer | null) => boolean | number;
}
export declare function subscribeFrames(subscriber: FrameSubscriber): () => void;
/** Set the pointer every following avatar looks toward (viewport pixels), or null when there is none. */
export declare function setPointer(next: Pointer | null): void;
/** Request a frame after a non-animated change (color, depth, definition). */
export declare function requestFrame(): void;
//# sourceMappingURL=clock.d.ts.map