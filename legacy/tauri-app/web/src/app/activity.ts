// Activity: what the bots have been doing, and what they want from you.
//
// The feed has two halves, and they come from two different places:
//
//   * What they did — derived, not stored. A routine that ran is that
//     routine's `last_run`; a session is the session; a card a bot was given
//     is the card's `for`. All of that is already in the face's stores, so
//     this file folds it into one newest-first list and stores nothing.
//   * What they want — an Ask, which has no other home yet. Only asks are
//     persisted (localStorage, the same stopgap as the cards and the
//     sessions): a question a bot is waiting on, the thread the answer
//     belongs in, and the choices it offered.
//
// The seam is the seam of the sessions: when the core starts streaming what
// its bots did (a routine fired, a webhook woke one, a reply landed while
// you were elsewhere), those rows arrive as events and this file keeps only
// the asks. `deriveActivity` is the part that shrinks.

import { useMemo, useSyncExternalStore } from "react";

import type { Agent, Routine } from "../bridge";
import type { Card } from "../shapeshift/types";
import { type Session, liveState, sessionLabel } from "./sessions";

/** A question a bot is waiting on you for. */
export type Ask = {
  id: string;
  at: number;
  /** Who is asking — one bot, or a group's bots. */
  agentIds: string[];
  /** The thread the answer belongs in, when the ask is about one. */
  threadId: string | null;
  /** The question, in the bot's words. */
  text: string;
  /** Why it's asking, when the bot said more than the question. */
  detail?: string;
  /** Likely answers the bot offered, as one-tap replies. */
  choices?: string[];
  state: "open" | "answered" | "dismissed";
  answer?: string;
  answeredAt?: number;
};

const KEY = "bs.asks.v1";
const MAX = 200;

function load(): Ask[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const v = JSON.parse(raw) as { v?: number; asks?: unknown };
    if (!Array.isArray(v.asks)) return [];
    return v.asks.filter((a): a is Ask => !!a && typeof a === "object" && typeof (a as Ask).id === "string" && Array.isArray((a as Ask).agentIds));
  } catch {
    return [];
  }
}

let asks: Ask[] = load();
const subs = new Set<() => void>();
const emit = () => {
  try {
    localStorage.setItem(KEY, JSON.stringify({ v: 1, asks }));
  } catch {
    /* private mode: asks live for this session only */
  }
  subs.forEach((f) => f());
};
const set = (next: Ask[]) => {
  asks = next.slice(0, MAX);
  emit();
};

export function useAsks(): Ask[] {
  return useSyncExternalStore(
    (f) => {
      subs.add(f);
      return () => subs.delete(f);
    },
    () => asks,
  );
}

export const openAsks = (list: Ask[] = asks) => list.filter((a) => a.state === "open");

/** An answer was given: the ask keeps its place in the feed, answered. */
export function answerAsk(id: string, answer: string) {
  const a = asks.find((x) => x.id === id);
  if (!a || a.state !== "open") return;
  const now = Date.now();
  set(asks.map((x) => (x.id === id ? { ...x, state: "answered", answer: answer.trim(), answeredAt: now } : x)));
}

/** Not now, not never: it stays in its thread, out of the feed. */
export function dismissAsk(id: string) {
  if (!asks.some((x) => x.id === id && x.state === "open")) return;
  set(asks.map((x) => (x.id === id ? { ...x, state: "dismissed" } : x)));
}

// ---------------------------------------------------------------- derived rows

export type ActivityRow =
  | { kind: "ask"; at: number; agentIds: string[]; ask: Ask }
  | { kind: "routine"; at: number; agentIds: string[]; agent: Agent; routine: Routine }
  | { kind: "session"; at: number; agentIds: string[]; session: Session }
  | { kind: "card"; at: number; agentIds: string[]; card: Card };

/** How many rows the feed keeps. */
const KEEP = 80;

/** Fold the stores into one feed, newest first. Everything here is derived:
 *  a routine that fired is its `last_run`, a session is its last word, a card
 *  a bot owes is its `for`. A core activity stream would replace exactly
 *  this function. */
export function deriveActivity(input: { agents: Agent[]; sessions: Session[]; cards: Card[]; asks: Ask[] }): ActivityRow[] {
  const { agents, sessions, cards, asks: askList } = input;
  const known = new Set(agents.map((a) => a.id));
  const rows: ActivityRow[] = [];

  for (const s of sessions) {
    if (!s.agentIds.some((id) => known.has(id))) continue;
    rows.push({ kind: "session", at: Math.max(s.lastAt, s.endedAt ?? 0), agentIds: s.agentIds, session: s });
  }
  for (const a of agents) {
    for (const r of a.routines) {
      if (!r.last_run) continue;
      rows.push({ kind: "routine", at: r.last_run, agentIds: [a.id], agent: a, routine: r });
    }
  }
  for (const c of cards) {
    if (!c.for || !known.has(c.for)) continue;
    rows.push({ kind: "card", at: c.at, agentIds: [c.for], card: c });
  }
  for (const a of askList) {
    if (a.state === "dismissed") continue;
    rows.push({ kind: "ask", at: a.state === "open" ? a.at : a.answeredAt ?? a.at, agentIds: a.agentIds, ask: a });
  }
  return rows.sort((x, y) => y.at - x.at).slice(0, KEEP);
}

/** The whole feed, live. The store lists are new arrays on every emit, so
 *  their identity is the signal that the derived rows need recomputing. */
export function useActivity(input: { agents: Agent[]; sessions: Session[]; cards: Card[] }): ActivityRow[] {
  const askList = useAsks();
  const { agents, sessions, cards } = input;
  return useMemo(() => deriveActivity({ agents, sessions, cards, asks: askList }), [agents, sessions, cards, askList]);
}

/** A row's stamp in sidebar units: "now", "5m", "2h", "3d". */
export function stamp(at: number, now = Date.now()): string {
  const s = Math.max(0, now - at) / 1000;
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  return `${Math.floor(s / 86400)}d`;
}

/** Which day header a row sits under. */
export function dayOf(at: number, now = Date.now()): "Today" | "Yesterday" | "Earlier" {
  const midnight = (t: number) => {
    const x = new Date(t);
    x.setHours(0, 0, 0, 0);
    return x.getTime();
  };
  const days = Math.round((midnight(now) - midnight(at)) / 86400e3);
  return days <= 0 ? "Today" : days === 1 ? "Yesterday" : "Earlier";
}

/** What the row says the session is doing, in the feed's words. */
export function sessionDetail(s: Session, now = Date.now()): string {
  const st = liveState(s, now);
  if (st === "awake") return "still talking";
  if (st === "concluding") return "wrapping up";
  return "ended";
}
