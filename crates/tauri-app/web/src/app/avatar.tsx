// The bot face: a brain, after Tabler's `brain` icon (see orb.tsx for why the
// old radial-blob engine could not draw one). This file stays the one place
// every surface imports a bot face from, so a bot looks the same in the
// sidebar, the bot page, the creation sheet and the editor.
//
// The emoji the core stores is no longer drawn and no longer picked: every bot
// is the same brain, and colour is the one shape axis left. The token still
// seeds the blink phase, so a row of bots never blinks in unison.

import type { Agent } from "../bridge";
import { Orb, Stack, type OrbState } from "./orb";

export const COLORS = ["#ff7f95", "#f97316", "#e11d48", "#8b5cf6", "#2563eb", "#0891b2", "#16a34a", "#ca8a04", "#64748b"];

/** Blink-phase tokens. The core stores the chosen one in `avatar.emoji`; the
 *  face never draws it. Templates still set it, and a bot made from scratch
 *  phases off its own name. */
export const FACES = ["🙂", "🧭", "✍️", "🔬", "🎬", "🧪", "📈", "🛠️", "🎨", "📚", "🤖", "🦊", "🐙", "🌿", "🛰️", "🧠"];

// Every orb lives now (breath, blink, gaze) — see orb.tsx — so there is no
// `live` prop to pass: the state alone decides how lively.
export function Avatar({ agent, size = 32, state = "idle" }: { agent?: Agent; size?: number; state?: OrbState }) {
  return <Orb agent={agent} size={size} state={state} />;
}

export { Orb, Stack, type OrbState };
