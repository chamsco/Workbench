// Browser bundle entry for the vendored open-bot-avatar package (MIT).
// Everything here comes from the package plus the repository's gallery files;
// nothing is reimplemented.
export { mountAvatar } from "./package/dist/dom/index.js";
export { renderAvatarSvg, validateAvatar, getExample } from "./package/dist/index.js";
export { default as brain } from "./gallery/brain.json";
export { default as cat } from "./gallery/cat.json";
export { default as star } from "./gallery/star.json";
