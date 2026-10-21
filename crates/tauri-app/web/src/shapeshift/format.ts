// How a card reads as one line of prose — what "Send to a chat" puts in the
// message and what a bot's routine instructions are. Kept out of the
// components so a card has exactly one voice everywhere it appears.

import { money } from "./parse";
import type { Card, CardKind } from "./types";

export const KIND_LABEL: Record<CardKind, string> = {
  event: "Event",
  reminder: "Reminder",
  checklist: "Checklist",
  timer: "Timer",
  split: "Split",
  expense: "Expense",
  convert: "Convert",
  calc: "Calculate",
  poll: "Poll",
  countdown: "Countdown",
  note: "Note",
};

const hhmm = (ms: number) => {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
};

export function cardLine(c: Card): string {
  switch (c.kind) {
    case "event": {
      const d = new Date(c.data.when);
      const at = d.toLocaleString([], { weekday: "short", hour: "numeric", minute: "2-digit" });
      const withWho = c.data.who ? ` with ${c.data.who}` : "";
      const where = c.data.where ? ` (${c.data.where})` : "";
      return `${c.title} — ${at}${withWho}${where}`;
    }
    case "reminder":
      return `Reminder — ${c.title}: ${c.detail}`;
    case "checklist":
      return `Checklist: ${c.data.items.join(", ")}`;
    case "timer":
      return `Timer: ${c.data.minutes} minutes`;
    case "split":
      return `Split ${money(c.data.amount)} between ${c.data.people} — ${money(c.data.amount / c.data.people)} each`;
    case "expense":
      return `Spent ${money(c.data.amount)} on ${c.data.what}`;
    case "convert":
      return `${c.data.value} ${c.data.from} = ${c.data.result} ${c.data.to}`;
    case "calc":
      return `${c.data.expr} = ${c.data.result}`;
    case "poll":
      return `${c.data.question} ${c.data.options.join(" / ")}`;
    case "countdown": {
      const days = Math.max(0, Math.ceil((c.data.until - Date.now()) / 86_400_000));
      return `${c.title}: ${days} day${days === 1 ? "" : "s"} left`;
    }
    case "note":
      return c.text;
  }
}

/** The local "HH:MM" a card's time implies, for a routine built from it. */
export const cardTime = (c: Card): string =>
  c.kind === "event" || c.kind === "reminder" ? hhmm(c.data.when) : "08:00";
