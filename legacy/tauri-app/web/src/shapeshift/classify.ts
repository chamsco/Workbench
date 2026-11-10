// The deciding half: which card kind is this line?
//
// Today that is an offline classifier — the parsers themselves are the
// questions ("does a date parse? does 'buy … and …' split into items?"),
// and a kind's confidence is how strong its trigger was. This returns the
// same ranked shape a Jev call would answer with (shapeshift asks one model
// fourteen typed questions in parallel; the offline mock answers the same
// questions from keywords), so the follow-up — a `jev_intent` command with
// the TYPESAFE_API_KEY the router already uses — swaps in here without the
// UI noticing.

import { attempt } from "./parse";
import type { CardKind, Ranked } from "./types";

const ORDER: [CardKind, number][] = [
  ["split", 0.92],
  ["convert", 0.92],
  ["poll", 0.9],
  ["reminder", 0.9],
  ["expense", 0.9],
  ["timer", 0.9],
  ["calc", 0.9],
  ["countdown", 0.8],
  ["checklist", 0.72],
  ["event", 0.66],
];

/** Ranked candidates, best first, at most three. Empty for empty input. */
export function classify(text: string, now = new Date()): Ranked[] {
  const t = text.trim();
  if (t.length < 2) return [];
  const out: Ranked[] = [];
  for (const [kind, confidence] of ORDER) {
    if (attempt(t, kind, now)) out.push({ kind, confidence });
  }
  if (out.length === 0) out.push({ kind: "note", confidence: 0.4 });
  return out.sort((a, b) => b.confidence - a.confidence).slice(0, 3);
}
