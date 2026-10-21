// Sessions: the thread, seen the way a conversation with a bot actually goes.
//
// A session is one sitting in one thread. The thread is one long conversation,
// iMessage-style; the session is how we talk about a stretch of it: it has a
// title — what you ended up talking about — and a life: awake while you trade
// messages, concluding once either side says it is wrapping up ("anything
// else?"), idle when the talking stops. Idle is not closed: say one more word
// and the same session picks up where it left off.
//
// One thread can hold several sessions. A new one starts when the subject
// moves on — "by the way, can you schedule dinner with Aya at 5?" is not more
// of the banner review, it is a different errand, and it gets its own title,
// its own clock and its own place to scroll back to (shift.ts decides, this
// file records). Each session carries its own id, so a generation is never
// confused with the sitting before it.
//
// Where the state lives: here, in the face, for now — the same stopgap as
// the cards. The seam is the seam of the cards: load/persist are the only two
// functions that touch localStorage, and the fields a core session would own
// (title written by the bot, concludedAt set by the core's wrap-up prompt,
// the idle timer on the scheduler's clock) are already here under those names.
// A later `session_*` set of commands replaces this file and nothing else.

import { useSyncExternalStore } from "react";

import type { Thread } from "../bridge";

export type SessionState = "awake" | "concluding" | "idle";

export type Session = {
  /** Unique: a thread can have held several sittings. */
  id: string;
  /** The thread this session is a stretch of. */
  threadId: string;
  /** 1, 2, 3… within the thread, oldest first. */
  n: number;
  /** What the session is about: the first line, until the bot renames it. */
  title: string;
  agentIds: string[];
  /** A machine the bot lives on, from an @//machine/name mention. */
  machine: string | null;
  startedAt: number;
  /** The last user message — the idle clock reads this. */
  lastAt: number;
  state: SessionState;
  /** When the wrap-up started (concluding), or the session went idle. */
  endedAt: number | null;
};

const KEY = "bs.sessions.v1";
const MAX = 300;
/** Silence after which a session is idle, awake or concluding alike. */
export const IDLE_MINUTES = 15;
export const IDLE_MS = IDLE_MINUTES * 60_000;

function load(): Session[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const v = JSON.parse(raw) as { v?: number; sessions?: unknown };
    if (!Array.isArray(v.sessions)) return [];
    const out: Session[] = [];
    for (const s of v.sessions) {
      if (!s || typeof s !== "object" || typeof (s as Session).threadId !== "string") continue;
      const t = s as Session;
      out.push({ ...t, id: t.id || t.threadId, n: t.n || 1 });
    }
    return out;
  } catch {
    return [];
  }
}

let sessions: Session[] = load();
const subs = new Set<() => void>();
const emit = () => {
  try {
    localStorage.setItem(KEY, JSON.stringify({ v: 1, sessions }));
  } catch {
    /* private mode: sessions live for this session only */
  }
  subs.forEach((f) => f());
};
const set = (next: Session[]) => {
  sessions = next.slice(0, MAX);
  emit();
};

export function useSessions(): Session[] {
  return useSyncExternalStore(
    (f) => {
      subs.add(f);
      return () => subs.delete(f);
    },
    () => sessions,
  );
}

/** Every session of a thread, newest sitting first. */
export const sessionsFor = (threadId: string): Session[] => sessions.filter((s) => s.threadId === threadId).sort((a, b) => b.n - a.n);

/** The sitting a thread is in now: its latest session. */
export const sessionFor = (threadId: string): Session | undefined => sessionsFor(threadId)[0];

export const sessionById = (id: string): Session | undefined => sessions.find((s) => s.id === id);

/** First line of a thread's first user message, or the thread's title. */
function titleFrom(line: string): string {
  const t = line.replace(/\s+/g, " ").trim();
  return (t.length > 48 ? t.slice(0, 47) + "…" : t) || "New session";
}

function autoTitle(t: Thread): string {
  const first = t.messages.find((m) => m.role === "user");
  return titleFrom(first?.text ?? t.title ?? "");
}

/** Open (or resume) the session for a bot's thread. Idempotent: opening the
 *  same thread again never restarts the clock, and an idle sitting wakes back
 *  up where it left off rather than starting over. */
export function ensureSession(t: Thread, machine: string | null = null) {
  const agentIds = [...(t.agent ? [t.agent] : []), ...(t.members ?? [])];
  if (agentIds.length === 0) return;
  const have = sessionsFor(t.id)[0];
  if (have) {
    if (have.state === "idle") touchSession(have.id);
    if (machine && have.machine !== machine) set(sessions.map((s) => (s.id === have.id ? { ...s, machine } : s)));
    return;
  }
  const now = Date.now();
  set([
    {
      id: `${t.id}#1`,
      threadId: t.id,
      n: 1,
      title: autoTitle(t),
      agentIds,
      machine,
      startedAt: now,
      lastAt: now,
      state: "awake",
      endedAt: null,
    },
    ...sessions,
  ]);
}

/** A new sitting in the same thread: the subject moved on. The sitting before
 *  it is left as it was — concluded if it was still awake, idle if it had
 *  already gone quiet — and the new one opens with the line that started it. */
export function renewSession(t: Thread, machine: string | null, firstText: string) {
  const agentIds = [...(t.agent ? [t.agent] : []), ...(t.members ?? [])];
  if (agentIds.length === 0) return;
  const prev = sessionsFor(t.id)[0];
  const now = Date.now();
  const closed = prev && prev.state === "awake" ? { ...prev, state: "concluding" as const, endedAt: now } : prev;
  set([
    {
      id: `${t.id}#${(prev?.n ?? 0) + 1}`,
      threadId: t.id,
      n: (prev?.n ?? 0) + 1,
      title: titleFrom(firstText),
      agentIds,
      machine: machine ?? prev?.machine ?? null,
      startedAt: now,
      lastAt: now,
      state: "awake",
      endedAt: null,
    },
    ...(closed ? sessions.map((s) => (s.id === closed.id ? closed : s)) : sessions),
  ]);
}

/** A user message: the session is awake again, whatever it was. */
export function touchSession(sessionId: string) {
  const s = sessions.find((x) => x.id === sessionId);
  if (!s) return;
  const now = Date.now();
  set(sessions.map((x) => (x.id === sessionId ? { ...x, lastAt: now, state: "awake", endedAt: null } : x)));
}

/** The wrap-up began: the bot asked "anything else?", the countdown runs. */
export function concludeSession(sessionId: string) {
  const s = sessions.find((x) => x.id === sessionId);
  if (!s || s.state !== "awake") return;
  const now = Date.now();
  set(sessions.map((x) => (x.id === sessionId ? { ...x, state: "concluding", endedAt: now } : x)));
}

/** Done: "no thanks that's all", or the countdown ran out. */
export function endSession(sessionId: string) {
  const s = sessions.find((x) => x.id === sessionId);
  if (!s || s.state === "idle") return;
  const now = Date.now();
  set(sessions.map((x) => (x.id === sessionId ? { ...x, state: "idle", endedAt: now } : x)));
}

/** The bot (or its owner) named what the session was about. */
export function renameSession(sessionId: string, title: string) {
  const t = title.trim();
  if (!t) return;
  set(sessions.map((x) => (x.id === sessionId ? { ...x, title: t } : x)));
}

/** The state as it stands right now, with the idle clock applied — a session
 *  nobody has spoken in for IDLE_MINUTES is idle whether anyone said so or not. */
export function liveState(s: Session, now = Date.now()): SessionState {
  if (s.state === "idle") return "idle";
  const since = s.state === "concluding" ? (s.endedAt ?? s.lastAt) : s.lastAt;
  return now - since > IDLE_MS ? "idle" : s.state;
}

/** "12m in", "wrapping up · ends in 3:58", "ended 20m ago". */
export function sessionLabel(s: Session, now = Date.now()): string {
  const st = liveState(s, now);
  if (st === "idle") {
    const at = s.endedAt ?? s.lastAt;
    const m = Math.max(0, Math.round((now - at) / 60_000));
    return m < 1 ? "just ended" : `ended ${m}m ago`;
  }
  if (st === "concluding") {
    const left = IDLE_MS - (now - (s.endedAt ?? now));
    if (left <= 0) return "ending…";
    const mins = Math.floor(left / 60_000);
    const secs = Math.floor((left % 60_000) / 1000);
    return `wrapping up · ends in ${mins}:${String(secs).padStart(2, "0")}`;
  }
  const m = Math.max(0, Math.round((now - s.startedAt) / 60_000));
  return m < 1 ? "just started" : `${m}m in`;
}

/** "no thanks that's all" and its cousins: the user's polite full stop. */
const GOODBYE = /^\s*(no|nope|nah|nah thanks|no thanks|thanks|thank you|that'?s (all|it)|nothing (else|more)|we'?re (done|good)|all (good|set|done)|bye|goodbye)\b[.!]?\s*$/i;

export const isGoodbye = (text: string) => GOODBYE.test(text.trim());

/** A clock that ticks, so "ends in 3:58" counts down without a refresh. */
import { useEffect, useState } from "react";

export function useNow(everyMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), everyMs);
    return () => clearInterval(t);
  }, [everyMs]);
  return now;
}

/** Which session a message belongs to: the latest sitting that had already
 *  started when the message was sent. Messages before the first session (a
 *  thread the face has not seen open) belong to none. */
export function sessionForMessage(list: Session[], threadId: string, at: number): Session | undefined {
  const mine = list.filter((s) => s.threadId === threadId && s.startedAt <= at + 1).sort((a, b) => b.startedAt - a.startedAt);
  return mine[0];
}

/** Smooth-scroll a session's opening into view — the scroll-back-up a session
 *  row asks for. Honours reduced motion by jumping instead. */
export function scrollToSession(sessionId: string) {
  const el = document.getElementById(`session-${sessionId}`);
  if (!el) return;
  const soft = !window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
  el.scrollIntoView({ behavior: soft ? "smooth" : "auto", block: "start" });
}
