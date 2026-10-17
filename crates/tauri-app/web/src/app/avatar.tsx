// The bot face: a coloured disc with an emoji face (Kuvo's shape + face, in the
// vocabulary Backspace agents already store). Shared by the sidebar, the bot
// page, the creation sheet and the editor, so a bot looks the same everywhere.

import { agentById, type Agent } from "../bridge";

export const COLORS = ["#f97316", "#e11d48", "#8b5cf6", "#2563eb", "#0891b2", "#16a34a", "#ca8a04", "#64748b"];

export const FACES = ["🙂", "🧭", "✍️", "🔬", "🎬", "🧪", "📈", "🛠️", "🎨", "📚", "🤖", "🦊", "🐙", "🌿", "🛰️", "🧠"];

export function Avatar({ agent, size = 32, ring = false }: { agent?: Agent; size?: number; ring?: boolean }) {
  const color = agent?.avatar.color || "#64748b";
  const emoji = agent?.avatar.emoji;
  return (
    <span
      className={`inline-grid shrink-0 place-items-center rounded-full font-semibold text-white ${ring ? "ring-2 ring-(--surface)" : ""}`}
      style={{ width: size, height: size, background: color, fontSize: emoji ? size * 0.5 : size * 0.42 }}
      aria-hidden
    >
      {emoji || (agent?.name || "?").slice(0, 1).toUpperCase()}
    </span>
  );
}

export function Stack({ ids, size = 22 }: { ids: string[]; size?: number }) {
  return (
    <span className="flex -space-x-2">
      {ids.slice(0, 3).map((id) => (
        <Avatar key={id} agent={agentById(id)} size={size} ring />
      ))}
    </span>
  );
}
