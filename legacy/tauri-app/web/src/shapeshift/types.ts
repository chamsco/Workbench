// Shapeshift, for Backspace: one text box that becomes what you mean.
//
// After anishfn/shapeshift (MIT, https://github.com/anishfn/shapeshift) — the
// idea, the card list and the calm-UI rule are theirs; the parsers and the
// fusion with bots, routines and threads are ours. Vendored as source, not
// as a dependency: shapeshift is a Next.js app and this face is a Vite
// bundle inside Tauri, so the logic comes across and the app does not.
//
// The split is the one shapeshift uses: a model decides *what kind of card
// this is*, deterministic code computes *everything else* (dates, amounts,
// math). Today the "model" is the offline keyword classifier in classify.ts,
// which returns the same ranked-confidence shape a Jev call would return —
// so a later `jev_intent` command can drop in without touching the UI.

export type CardKind =
  | "event"
  | "reminder"
  | "checklist"
  | "timer"
  | "split"
  | "expense"
  | "convert"
  | "calc"
  | "poll"
  | "countdown"
  | "note";

/** The kind-specific payload. A card is data — renderable, sendable to a
 *  bot, storable — never a rendered element. */
export type CardDataOf<K extends CardKind> =
  K extends "event"
    ? { when: number; who: string; where: string }
    : K extends "reminder"
      ? { when: number }
      : K extends "checklist"
        ? { items: string[]; done: number[] }
        : K extends "timer"
          ? { minutes: number }
          : K extends "split"
            ? { amount: number; people: number }
            : K extends "expense"
              ? { amount: number; what: string }
              : K extends "convert"
                ? { value: number; from: string; to: string; result: number }
                : K extends "calc"
                  ? { expr: string; result: number }
                  : K extends "poll"
                    ? { question: string; options: string[] }
                    : K extends "countdown"
                      ? { until: number }
                      : Record<string, never>;

export type CardData = CardDataOf<CardKind>;

/** Discriminated on `kind`, so `card.kind === "checklist"` narrows `data`. */
export type Card = {
  [K in CardKind]: {
    id: string;
    kind: K;
    /** The line as typed. */
    text: string;
    /** Short label: the card's headline. */
    title: string;
    /** One line under it: the computed answer (¥800 each, 12 days left…). */
    detail: string;
    data: CardDataOf<K>;
    done: boolean;
    at: number;
    /** The bot this card is for, when it was written for one — a bot's page
     *  lists the cards addressed to it beside its sessions. */
    for?: string;
  };
}[CardKind];

/** A classifier candidate: a kind and how sure we are, 0..1. */
export type Ranked = { kind: CardKind; confidence: number };
