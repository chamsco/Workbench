// Who the hero prompt is talking to: @mentions, the way the screenshot's
// "@Agent 1" works, extended for bots that live on another machine.
//
// Three shapes parse:
//   @Kuvo              a bot on this machine, by name (fuzzy: prefix, then
//                      substring, case-insensitive)
//   @//agent2/Kuvo     that bot on machine "agent2" (UNC-ish, as typed)
//   @agent2/Kuvo       the same with one slash, because hands type both
// A machine that is not this one still resolves the *name* locally: the
// session carries the machine as a tag, and the route itself is the core's
// business later. Parsing it here means the strip can say
// "→ Kuvo · on agent2" today, and the machine-qualified route can become real
// without touching the mention grammar.

import type { Agent } from "../bridge";

export type Mention = {
  /** The bot it resolved to, or null when the name is not a bot here. */
  agent: Agent | null;
  /** The name as typed, minus the @ and any machine prefix. */
  name: string;
  /** The machine prefix, or null for this machine. */
  machine: string | null;
  /** The raw token, e.g. "@//agent2/Kuvo". */
  raw: string;
};

const TOKEN = /(^|\s)(\/\/|\/)?@([A-Za-z0-9_][\w.-]*(?:\/[A-Za-z0-9_][\w.-]*)?)/g;

/** Every mention-shaped token in the text, in order, with where each sat. */
export function scanMentions(text: string): { mention: Mention; start: number; end: number }[] {
  const out: { mention: Mention; start: number; end: number }[] = [];
  for (const m of text.matchAll(TOKEN)) {
    const lead = m[1] ?? "";
    const slash = m[2] ?? "";
    const body = m[3] ?? "";
    const start = (m.index ?? 0) + lead.length;
    const end = start + (slash ? slash.length : 0) + 1 + body.length; // + the @
    const parts = body.split("/").filter(Boolean);
    let machine: string | null = null;
    let name = body;
    if (parts.length >= 2) {
      // @//machine/name or @machine/name: the last part is the bot.
      machine = parts.slice(0, -1).join("/") || null;
      name = parts[parts.length - 1];
    }
    out.push({ mention: { agent: null, name, machine, raw: text.slice(start, end) }, start, end });
  }
  return out;
}

/** A bot by fuzzy name: exact, then prefix, then substring. Ties go to the
 *  shortest name, so "@pip" picks Pip over Pipeline. */
export function matchAgent(query: string, agents: Agent[]): Agent | null {
  const q = query.trim().toLowerCase();
  if (!q) return null;
  const exact = agents.find((a) => a.name.toLowerCase() === q);
  if (exact) return exact;
  const prefix = agents.filter((a) => a.name.toLowerCase().startsWith(q)).sort((x, y) => x.name.length - y.name.length);
  if (prefix.length) return prefix[0];
  const sub = agents.filter((a) => a.name.toLowerCase().includes(q)).sort((x, y) => x.name.length - y.name.length);
  return sub[0] ?? null;
}

/** Mentions resolved against the bots that exist here, deduplicated by agent
 *  (a bot named twice is one destination). */
export function parseMentions(text: string, agents: Agent[]): Mention[] {
  const seen = new Set<string>();
  const out: Mention[] = [];
  for (const { mention } of scanMentions(text)) {
    const agent = matchAgent(mention.name, agents);
    if (agent && !seen.has(agent.id)) {
      seen.add(agent.id);
      out.push({ ...mention, agent });
    }
  }
  return out;
}

/** The mention being typed at the caret — for the autocomplete. Returns the
 *  query ("" right after the @) and the machine prefix, or null. */
export function mentionAtCaret(text: string, caret: number): { query: string; machine: string | null; start: number } | null {
  const before = text.slice(0, caret);
  const m = before.match(/(?:^|\s)(\/\/|\/)?@([\w.-]*)$/);
  if (!m) return null;
  const body = m[2] ?? "";
  const slash = m[1] ?? "";
  const start = caret - body.length - (slash ? slash.length : 0) - 1;
  const parts = body.split("/").filter(Boolean);
  if (parts.length >= 2) return { query: parts[parts.length - 1], machine: parts.slice(0, -1).join("/"), start };
  if (slash) return { query: body, machine: slash.replace(/\//g, "") || null, start };
  return { query: body, machine: null, start };
}

/** Bots to offer in the autocomplete, filtered by what is typed after the @. */
export function mentionOptions(query: string, machine: string | null, agents: Agent[]): Agent[] {
  const q = query.trim().toLowerCase();
  const pool = machine ? agents : agents;
  const hits = q
    ? pool.filter((a) => a.name.toLowerCase().startsWith(q) || a.name.toLowerCase().includes(q) || a.job.toLowerCase().includes(q))
    : pool;
  return hits.slice(0, 6);
}
