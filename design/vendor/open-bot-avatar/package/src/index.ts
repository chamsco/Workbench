export type { AvatarDefinition, AvatarEyes, AvatarMotion, AvatarMotionBlend, RenderOptions, ValidationResult } from './types.ts';
export { validateAvatar } from './validate.ts';
export {
  avatarFrame,
  renderAvatarSvg,
  renderAvatarScene,
  sceneFrame,
  DEFAULT_COLOR,
  DEFAULT_DEPTH,
  DEFAULT_EYE_COLOR,
} from './render.ts';
export type { AvatarFrame, EyeMark, FrameShape, Matrix, SceneFrame } from './render.ts';
export { BASE_PITCH, BASE_YAW, blendPoses, computePose } from './motion.ts';
export type { Pose, PoseInput } from './motion.ts';
export { MOOD_MS, moodProgress, settleMood } from './mood.ts';
export type { MoodState } from './mood.ts';
export { REST, followGaze, followStep, pointerGaze } from './follow.ts';
export type { FollowState } from './follow.ts';
export { createAnimator } from './animator.ts';
export type { Animator, AnimatorInput, AnimatorStep } from './animator.ts';
export { examples, getExample } from './examples.ts';
