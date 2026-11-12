// Where cards live, on this machine, for now.
//
// LocalStorage is the first slice's store, the same one shapeshift itself
// uses — the face has no generic KV command to reach the core with, and
// cards are personal scratch, not shared state. The seam is deliberate:
// load/persist are the only two functions that touch localStorage, so the
// follow-up (cards_list / cards_save / cards_delete Tauri commands, and
// cards syncing across machines like routines do) replaces this file and
// nothing else.
//
// Newest first, capped at 200: a card list nobody trims is a list nobody
// reads.

import { useSyncExternalStore } from "react";

import type { Card } from "./types";

const KEY = "bs.shapeshift.v1";
const MAX = 200;

function load(): Card[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const v = JSON.parse(raw) as { v?: number; cards?: unknown };
    if (!Array.isArray(v.cards)) return [];
    return v.cards.filter(
      (c): c is Card =>
        !!c && typeof c === "object" && typeof (c as Card).id === "string" && typeof (c as Card).kind === "string",
    );
  } catch {
    return [];
  }
}

function persist() {
  try {
    localStorage.setItem(KEY, JSON.stringify({ v: 1, cards }));
  } catch {
    /* private mode: the cards live for this session only */
  }
}

let cards: Card[] = load();
const subs = new Set<() => void>();
const emit = () => {
  persist();
  subs.forEach((f) => f());
};

export function useCards(): Card[] {
  return useSyncExternalStore(
    (f) => {
      subs.add(f);
      return () => subs.delete(f);
    },
    () => cards,
  );
}

export const newId = () => `c_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 7)}`;

export function addCard(c: Card) {
  cards = [c, ...cards].slice(0, MAX);
  emit();
}

export function removeCard(id: string) {
  cards = cards.filter((c) => c.id !== id);
  emit();
}

export function setDone(id: string, done: boolean) {
  cards = cards.map((c) => (c.id === id ? { ...c, done } : c));
  emit();
}

/** One checklist item ticked or unticked, by its index in the item list. */
export function toggleItem(id: string, index: number) {
  cards = cards.map((c) =>
    c.id === id && c.kind === "checklist"
      ? { ...c, data: { ...c.data, done: c.data.done.includes(index) ? c.data.done.filter((i) => i !== index) : [...c.data.done, index] } }
      : c,
  );
  emit();
}
