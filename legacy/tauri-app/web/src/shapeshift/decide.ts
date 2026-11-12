// The calm-UI rule, straight from shapeshift: raw classifier output flickers
// as you type, so a card only changes when a challenger wins twice in a row
// (or is very sure). Without this the preview is a slot machine — "dinner
// with priya" would flash event → note → event with every keystroke.

import type { CardKind, Ranked } from "./types";

/** Confidence at which a challenger switches the card immediately. */
export const SURE = 0.85;

export type Calm = { card: CardKind; challenger: CardKind | null; streak: number };

export const calmStart = (): Calm => ({ card: "note", challenger: null, streak: 0 });

/** One keystroke's verdict. Pure: the caller holds the state. */
export function calmStep(prev: Calm, ranked: Ranked[]): Calm {
  const top = ranked[0];
  if (!top) return prev;
  if (top.kind === prev.card) return { card: prev.card, challenger: null, streak: 0 };
  if (top.confidence >= SURE) return { card: top.kind, challenger: null, streak: 0 };
  if (prev.challenger === top.kind && prev.streak >= 1) return { card: top.kind, challenger: null, streak: 0 };
  return { card: prev.card, challenger: top.kind, streak: prev.challenger === top.kind ? prev.streak + 1 : 1 };
}
