// The bot face: the real `open-bot-avatar` brain when the shell has loaded the
// package, with the in-repo orb as the fallback. Every surface imports a bot
// face from this file, so a bot looks the same in the sidebar, the bot page,
// the creation sheet and the editor.

import type { Agent } from "../bridge";
import { PackageFace, hasPackageFace } from "./bot-face";
import { Orb, Stack, type OrbState } from "./orb";

export const COLORS = ["#f97316", "#e11d48", "#8b5cf6", "#2563eb", "#0891b2", "#16a34a", "#ca8a04", "#64748b"];

/** Blink-phase tokens. The core stores the chosen one in `avatar.emoji`; the
 *  face never draws it. Templates still set it, and a bot made from scratch
 *  phases off its own name. */
export const FACES = ["🙂", "🧭", "✍️", "🔬", "🎬", "🧪", "📈", "🛠️", "🎨", "📚", "🤖", "🦊", "🐙", "🌿", "🛰️", "🧠"];

// Every orb lives now (breath, blink, gaze) — see orb.tsx — so there is no
// `live` prop to pass: the state alone decides how lively.
export function Avatar({ agent, size = 32, state = "idle" }: { agent?: Agent; size?: number; state?: OrbState }) {
  if (hasPackageFace()) {
    return <PackageFace color={agent?.avatar.color || "#64748b"} size={size} state={state} />;
  }
  return <Orb agent={agent} size={size} state={state} />;
}

export { Orb, Stack, type OrbState };
