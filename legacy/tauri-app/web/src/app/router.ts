// The hero prompt's brain: one line in, a plan out. This is the shapeshift
// idea pointed at agents instead of cards — a cheap intent pass over the
// text, and everything else (dates, amounts) computed, not guessed.
//
// The plan is what the strip under the input shows BEFORE Enter does it, so
// the routing is never a surprise:
//
//   "@kuvo is the banner copy ready?"   → chat   · Kuvo's session
//   "@kuvo @june ship the banner"       → group  · Kuvo + June, new session
//   "create an agent Watcher that…"     → new-agent · the new-bot sheet, prefilled
//   "move morning briefing to 9am"      → routine   · Morning briefing → 09:00
//   "buy milk, eggs and bread"          → card     · shapeshift, as before
//   "what's the relaunch status?"       → chat     · the best-matching bot
//
// The jev_intent seam from classify.ts applies here too: this file returns
// ranked plans in the same shape a model call would, so the offline pass can
// be replaced without touching the strip.

import type { Agent, Routine } from "../bridge";
import { classify } from "../shapeshift";
import { parseMentions } from "./mentions";

export type Plan =
  | { kind: "chat"; agents: Agent[]; machine: string | null; text: string }
  | { kind: "group"; agents: Agent[]; title: string; goal: string; text: string }
  | { kind: "new-agent"; name: string; job: string }
  | { kind: "routine"; agent: Agent; routine: Routine; time: string | null }
  | { kind: "card" }
  | { kind: "pick"; agents: Agent[]; text: string }
  | { kind: "empty" };

const STOP = new Set(
  ("a an the is are was were be been being do does did have has had i you he she it we they me him her them my your his its our their this that these those " +
    "what whats when where which who whom why how can could should would shall will may might must to of in on at for with from by about into over after before " +
    "and or but if then else so as not no yes ok okay please just get got make made let lets need needs needed want wants").split(" "),
);
const tokens = (s: string) => s.toLowerCase().match(/[a-z0-9']+/g)?.filter((t) => !STOP.has(t) && t.length > 1) ?? [];

/** "9am", "09:00", "at 7 pm" → "HH:MM" local. Null when there is no time. */
export function extractTime(text: string): string | null {
  const hhmm = text.match(/\b([01]?\d|2[0-3]):([0-5]\d)\b/);
  if (hhmm) return `${hhmm[1].padStart(2, "0")}:${hhmm[2]}`;
  const ampm = text.match(/\b([01]?\d|2[0-3])(?::([0-5]\d))?\s*(am|pm)\b/i);
  if (ampm) {
    let h = Number(ampm[1]) % 12;
    if (ampm[3].toLowerCase() === "pm") h += 12;
    return `${String(h).padStart(2, "0")}:${ampm[2] ?? "00"}`;
  }
  return null;
}

export const NEW_AGENT = /\b(?:create|make|add|spin up|build)\s+(?:me\s+)?(?:an?\s+)?(?:new\s+)?(?:bot|agent)\b/i;
export const ROUTINE_INTENT = /\b(?:routine|schedule|briefing|wake|reminder|remind|every ?day|daily|weekly|reschedule)\b|^move\b|\bmove\b.*\bto\b/i;

/** "create an agent named Watcher that watches deploys" → name + job. */
function newAgentSpec(text: string): { name: string; job: string } {
  const named = text.match(/\b(?:named|called)\s+([A-Za-z][\w-]{0,30})/i);
  let name = named?.[1] ?? "";
  if (!name) {
    const cap = text.match(/\b([A-Z][a-z]{2,30})\b/);
    name = cap && !/^(Create|Make|Add|Build|Agent|Bot|New)$/.test(cap[1]) ? cap[1] : "";
  }
  const job = text.match(/\b(?:that|who|which|whose job is to|to)\s+(.{3,})$/i)?.[1]?.trim() ?? "";
  return { name, job: job.replace(/[.?!]+$/, "") };
}

/** How well a line fits a bot: its name said out loud, then words from its job
 *  and its routines' names. The threshold is loose on purpose — "the
 *  relaunch status" should find the bot whose job mentions relaunch. */
function scoreAgent(text: string, a: Agent): number {
  const words = tokens(text);
  if (!words.length) return 0;
  let score = 0;
  const name = a.name.toLowerCase();
  if (text.toLowerCase().includes(name)) score += 5;
  for (const w of words) {
    if (name.startsWith(w) || w.startsWith(name)) score += 2;
    // One word from the job is enough to be *a* candidate; the tie-break
    // below is what makes it the answer.
    if (a.job.toLowerCase().includes(w)) score += 2;
    for (const r of a.routines) if (r.name.toLowerCase().includes(w)) score += 1.5;
  }
  return score;
}

/** The routine a change is about, by name words ("morning briefing"). */
function findRoutine(text: string, agents: Agent[]): { agent: Agent; routine: Routine } | null {
  const words = tokens(text);
  let best: { agent: Agent; routine: Routine; score: number } | null = null;
  for (const a of agents) {
    for (const r of a.routines) {
      const rWords = r.name.toLowerCase().match(/[a-z0-9']+/g) ?? [];
      const score = rWords.filter((w) => words.some((t) => t.startsWith(w) || w.startsWith(t))).length;
      if (score > 0 && (!best || score > best.score)) best = { agent: a, routine: r, score };
    }
  }
  return best ?? null;
}

/** Decide the plan. Pure: the same line always plans the same way. */
export function routePlan(text: string, agents: Agent[]): Plan {
  const line = text.trim();
  if (!line) return { kind: "empty" };
  const mentions = parseMentions(line, agents);
  const rest = line.replace(/(?:^|\s)(?:\/\/|\/)?@[\w.-]+(?:\/[\w.-]+)?/g, " ").trim();
  const machine = mentions[0]?.machine ?? null;

  // Two or more bots named: that is a group, the way @Name works in a thread.
  if (mentions.length >= 2) {
    const title = (rest.split(/\s+/).slice(0, 6).join(" ") || "New group").replace(/[.?!]+$/, "");
    return { kind: "group", agents: mentions.map((m) => m.agent!), title, goal: rest, text: rest };
  }
  if (mentions.length === 1) {
    const a = mentions[0].agent!;
    if (ROUTINE_INTENT.test(line) && findRoutine(line, agents)) {
      const { routine } = findRoutine(line, agents)!;
      return { kind: "routine", agent: a, routine, time: extractTime(line) };
    }
    return { kind: "chat", agents: [a], machine, text: rest };
  }

  // Nobody named: is this about making one?
  if (NEW_AGENT.test(line)) {
    const { name, job } = newAgentSpec(line);
    return { kind: "new-agent", name, job };
  }
  // A schedule change with no name: find whose routine it is.
  if (ROUTINE_INTENT.test(line) && extractTime(line)) {
    const hit = findRoutine(line, agents);
    if (hit) return { kind: "routine", agent: hit.agent, routine: hit.routine, time: extractTime(line) };
  }

  // Shapeshift's turn: an everyday thing stays a card, even here.
  const ranked = classify(line);
  const top = ranked[0];
  if (top && top.kind !== "note" && top.confidence >= 0.72) return { kind: "card" };

  // Otherwise it is work for a bot. One bot: it is that bot's. Many: the best
  // match, or the strip asks who.
  if (agents.length === 0) return { kind: "new-agent", name: "", job: line.replace(/[.?!]+$/, "") };
  if (agents.length === 1) return { kind: "chat", agents: [agents[0]], machine: null, text: rest };
  const scored = agents.map((a) => ({ a, s: scoreAgent(line, a) })).sort((x, y) => y.s - x.s);
  if (scored[0].s >= 2 && (scored.length === 1 || scored[0].s > scored[1].s)) {
    return { kind: "chat", agents: [scored[0].a], machine: null, text: rest };
  }
  return { kind: "pick", agents, text: rest };
}

/** The one line the strip shows for a plan — what Enter will do. */
export function planLine(p: Plan): { glyph: Plan["kind"] | "group"; text: string } {
  switch (p.kind) {
    case "chat":
      return { glyph: "chat", text: p.agents.map((a) => a.name).join(", ") + (p.machine ? ` · on ${p.machine}` : "") + " · its session" };
    case "group":
      return { glyph: "group", text: `New group · ${p.agents.map((a) => a.name).join(" + ")}` };
    case "new-agent":
      return { glyph: "new-agent", text: p.name ? `New bot · ${p.name}${p.job ? ` — ${p.job}` : ""}` : "New bot · from this line" };
    case "routine":
      return { glyph: "routine", text: `${p.routine.name || "Routine"} → ${p.time ?? "edit"} · ${p.agent.name}` };
    case "card":
      return { glyph: "card", text: "Everyday card · shapeshift" };
    case "pick":
      return { glyph: "pick", text: "Pick who · " + p.agents.map((a) => a.name).join(", ") };
    default:
      return { glyph: "pick", text: "" };
  }
}
