// The bot face: a brain, after Tabler's `brain` icon (see orb.tsx for why the
// old radial-blob engine could not draw one). This file stays the one place
// every surface imports a bot face from, so a bot looks the same in the
// sidebar, the bot page, the creation sheet and the editor.
//
// The emoji the core stores is no longer drawn and no longer picked: every bot
// is the same brain, and colour is the one shape axis left. The token still
// seeds the blink phase, so a row of bots never blinks in unison.

import type { Agent } from "../bridge";
import { ACCESSORIES, Orb, Stack, type OrbState } from "./orb";

export { ACCESSORIES };

// Body tones; each bot's folds are the same hue darkened (see orb.tsx `darken`).
export const COLORS = ["#ff8597", "#fb923c", "#a78bfa", "#60a5fa", "#22d3ee", "#4ade80", "#facc15", "#94a3b8"];

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

const ACCESSORY_LABEL: Record<string, string> = {
  "": "Nothing", glasses: "Glasses", tophat: "Top hat", beanie: "Beanie",
  bow: "Bow", crown: "Crown", headphones: "Headphones",
};

/** What the brain wears: each choice previewed on the bot itself. */
export function AccessoryPicker({ agent, value, onChange, size = 30 }: { agent: Agent; value: string; onChange: (v: string) => void; size?: number }) {
  return (
    <div className="flex flex-wrap items-center gap-1">
      {ACCESSORIES.map((k) => (
        <button
          key={k || "none"}
          type="button"
          title={ACCESSORY_LABEL[k]}
          aria-label={`Accessory: ${ACCESSORY_LABEL[k]}`}
          aria-pressed={value === k}
          onClick={() => onChange(k)}
          className={`grid cursor-pointer place-items-center rounded-lg p-0.5 ${value === k ? "ring-2 ring-foreground" : "opacity-70 hover:opacity-100"}`}
        >
          <Orb agent={{ ...agent, avatar: { ...agent.avatar, accessory: k } }} size={size} />
        </button>
      ))}
    </div>
  );
}
