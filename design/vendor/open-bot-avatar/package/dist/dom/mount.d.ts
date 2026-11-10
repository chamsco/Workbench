import type { AvatarDefinition, AvatarMotion } from '../types.ts';
export interface AvatarOptions {
    definition: AvatarDefinition;
    /** Body color, #rgb or #rrggbb. */
    color?: string;
    /** Eye color, #rgb or #rrggbb. Defaults to a near-black ink. */
    eyeColor?: string;
    /** Volume amount 0..14 (6 = natural depth, 0 = flat). */
    depth?: number;
    mood?: AvatarMotion;
    /** Eyes look and the body turns toward the mouse, pen or finger anywhere on the page. */
    followPointer?: boolean;
    /** Fixed look direction (-1..1 per axis, x right, y down). Wins over the pointer. */
    lookAt?: {
        x: number;
        y: number;
    } | null;
    /** Keep the current look as it is (e.g. while the user drags the view). */
    holdGaze?: boolean;
    /** Extra view rotation in radians on top of the resting view (e.g. drag-to-turn). */
    view?: {
        yaw: number;
        pitch: number;
    };
    /** true or false to force; 'system' (default) follows prefers-reduced-motion. */
    reducedMotion?: boolean | 'system';
    /** Render one static SVG and never animate (thumbnails, lists). */
    still?: boolean;
    /** Accessible name; defaults to the definition's name. "" marks the avatar decorative. */
    label?: string;
}
export interface MountedAvatar {
    /** Change any options; only what changed is redrawn. */
    update(options: Partial<AvatarOptions>): void;
    destroy(): void;
}
export declare function mountAvatar(host: HTMLElement, initial: AvatarOptions): MountedAvatar;
//# sourceMappingURL=mount.d.ts.map