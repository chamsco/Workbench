import type { AvatarDefinition } from './types.ts';
/**
 * Starter avatars, produced with the generate-avatar design prompt. The
 * face-bearing part is always the dominant mass, so the enlarged runtime eyes
 * keep visible room (see faceRoom in volume.ts), and accessories stay small by
 * their outer extent, hollow loops included. Outlines wind clockwise;
 * holes (handles, loops) wind counter-clockwise so the nonzero fill rule cuts
 * them. Tests keep these equal to the skill's example run and the examples in
 * skills/generate-avatar/references/system-prompt.md.
 */
export declare const examples: AvatarDefinition[];
export declare function getExample(id: string): AvatarDefinition | undefined;
//# sourceMappingURL=examples.d.ts.map