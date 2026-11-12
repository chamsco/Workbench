// The deterministic half: one parser per card kind. No model, no network —
// dates, amounts and math are computed here, exactly like shapeshift does it,
// so a card's fields cannot hallucinate. Every parser returns either the
// card's fields or null; the classifier only offers kinds that parsed.
//
// The date parser is deliberately word-shaped (tomorrow, friday, in 3 days,
// 8pm) rather than chrono-node: the face ships as one bundle, a full date
// library is most of a card's weight, and these eleven cards need about
// twenty lines of string handling.

import type { Card, CardData, CardKind } from "./types";

type Parsed = { title: string; detail: string; data: CardData };

const cap = (s: string) => (s ? s[0].toUpperCase() + s.slice(1) : s);
const clean = (s: string) => s.replace(/\s+/g, " ").trim();
const esc = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const r2 = (n: number) => Math.round(n * 100) / 100;

/** Money formatting: the symbol in the text, or this machine's default
 *  (¥ — one line to change when the face grows a locale setting). */
export function money(n: number, sym = "¥"): string {
  const v = r2(n);
  return `${sym}${Number.isInteger(v) ? v : v.toFixed(2)}`;
}

// ---------------------------------------------------------------- when

const WD: Record<string, number> = { monday: 1, tuesday: 2, wednesday: 3, thursday: 4, friday: 5, saturday: 6, sunday: 7 };
const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const TIME_AMPM = /\b(?:at\s+)?(\d{1,2})(?::(\d{2}))?\s*(am|pm)\b/i;
const TIME_24 = /\b(?:at\s+)?([01]?\d|2[0-3]):([0-5]\d)\b/;
const EVENING = /\b(tonight|this evening|dinner)\b/i;

const fmtTime = (h: number, m: number) => {
  const ap = h < 12 ? "AM" : "PM";
  const hh = h % 12 === 0 ? 12 : h % 12;
  return m ? `${hh}:${String(m).padStart(2, "0")} ${ap}` : `${hh} ${ap}`;
};

/** tomorrow, friday, next friday, in 3 days, 8pm, "friday 8pm", tonight.
 *  `ate` is the raw text the date consumed, so callers can strip it from a
 *  title without guessing where it was. */
export function parseWhen(text: string, now = new Date()): { ms: number; label: string; ate: string[] } | null {
  const t = text.toLowerCase();
  const ate: string[] = [];
  let day: Date | null = null;
  let dayLabel = "";

  const rel = t.match(/\bin\s+(\d+)\s+(day|days|week|weeks|month|months)\b/);
  if (rel) {
    const n = Number(rel[1]);
    day = new Date(now);
    if (rel[2].startsWith("d")) day.setDate(day.getDate() + n);
    else if (rel[2].startsWith("w")) day.setDate(day.getDate() + 7 * n);
    else day.setMonth(day.getMonth() + n);
    dayLabel = `in ${n} ${rel[2]}`;
    ate.push(rel[0]);
  }
  if (!day) {
    const wd = t.match(/\b(next\s+)?(sunday|monday|tuesday|wednesday|thursday|friday|saturday)\b/);
    if (wd) {
      const iso = WD[wd[2]];
      day = new Date(now);
      const cur = day.getDay() === 0 ? 7 : day.getDay();
      let add = (iso - cur + 7) % 7;
      if (add === 0) add = 7;
      if (wd[1]) add += 7;
      day.setDate(day.getDate() + add);
      dayLabel = `${wd[1] ? "next " : ""}${wd[2]}`;
      ate.push(wd[0]);
    }
  }
  if (!day && /\btomorrow\b/.test(t)) {
    day = new Date(now);
    day.setDate(day.getDate() + 1);
    dayLabel = "tomorrow";
    ate.push("tomorrow");
  }
  if (!day && /\btonight\b/.test(t)) {
    day = new Date(now);
    dayLabel = "tonight";
    ate.push("tonight");
  }
  if (!day && /\btoday\b/.test(t)) {
    day = new Date(now);
    dayLabel = "today";
    ate.push("today");
  }
  if (!day && /\bnext week\b/.test(t)) {
    day = new Date(now);
    day.setDate(day.getDate() + 7);
    dayLabel = "next week";
    ate.push("next week");
  }

  let hh = -1;
  let mm = 0;
  const ap = t.match(TIME_AMPM);
  const h24 = ap ? null : t.match(TIME_24);
  if (ap) {
    hh = Number(ap[1]) % 12;
    if (ap[3].toLowerCase() === "pm") hh += 12;
    mm = ap[2] ? Number(ap[2]) : 0;
    ate.push(ap[0]);
  } else if (h24) {
    hh = Number(h24[1]);
    mm = Number(h24[2]);
    ate.push(h24[0]);
  }

  if (!day && hh < 0) return null;
  const d = day ?? new Date(now);
  if (hh >= 0) d.setHours(hh, mm, 0, 0);
  else d.setHours(EVENING.test(t) ? 19 : 9, 0, 0, 0);

  const parts: string[] = [];
  parts.push(dayLabel || "today");
  if (hh >= 0) parts.push(fmtTime(hh, mm));
  return { ms: d.getTime(), label: parts.join(" at "), ate };
}

/** Strip the consumed fragments and any preposition they left dangling. */
function stripAte(text: string, frags: (string | undefined)[]): string {
  let out = text;
  for (const f of frags) if (f) out = out.replace(new RegExp(esc(f), "ig"), " ");
  return clean(out.replace(/^(at|on|in)\s+/i, "").replace(/[,\s]+$/, ""));
}

// ---------------------------------------------------------------- cards

function tryPoll(text: string): Parsed | null {
  if (!/\bor\b/i.test(text) || !/\?\s*$/.test(text)) return null;
  const options = text
    .split(/\bor\b/i)
    .map((s) => cap(clean(s.replace(/[?!.,]+$/, "").replace(/\s+for\s+.+$/i, ""))))
    .filter((s) => s.length > 0 && s.length <= 40);
  if (options.length < 2) return null;
  const question = cap(clean(text.replace(/\s+for\s+[^?]*\?$/i, "?")));
  return { title: question, detail: options.join(" / "), data: { question, options } };
}

function tryReminder(text: string, now: Date): Parsed | null {
  if (!/\bremind\b/i.test(text)) return null;
  const w = parseWhen(text, now);
  if (!w) return null;
  const title = stripAte(
    text.replace(/^\s*remind\s+me\s+(?:to|that|about)\s+/i, ""),
    w.ate,
  );
  return { title: cap(title) || "Reminder", detail: w.label, data: { when: w.ms } };
}

function trySplit(text: string): Parsed | null {
  const cur = "¥";
  let m = text.match(/\bsplit\b.*?([₹$€£¥]?)(\d+(?:[.,]\d+)?).*?\b(?:between|with|among)\s+(\d+)\b/i);
  let amount: number, people: number, sym = cur;
  if (m) {
    sym = m[1] || cur;
    amount = num(m[2]);
    people = Number(m[3]);
  } else {
    m = text.match(/\bsplit\b.*?\b(\d+)\s*(?:ways|people|persons?)\b.*?([₹$€£¥]?)(\d+(?:[.,]\d+)?)/i);
    if (!m) return null;
    people = Number(m[1]);
    sym = m[2] || cur;
    amount = num(m[3]);
  }
  if (!(people > 0) || !(amount > 0)) return null;
  const each = r2(amount / people);
  return { title: `${money(each, sym)} each`, detail: `${money(amount, sym)} between ${people}`, data: { amount, people } };
}

function tryExpense(text: string): Parsed | null {
  const m = text.match(/\b(?:spent|paid|pay|bought)\s+([₹$€£¥]?)\s*(\d+(?:[.,]\d+)?)\s+(?:on|for)\s+(.+)$/i);
  if (!m) return null;
  const amount = num(m[2]);
  if (!(amount > 0)) return null;
  const what = cap(clean(m[3]).replace(/[.,!?]+$/, ""));
  return { title: what, detail: money(amount, m[1] || "¥"), data: { amount, what } };
}

const UNIT_WORDS = "km|kilomet(?:re|er)s?|mi|miles?|kg|kilos?|kilograms?|lbs?|pounds?|°?f|°?c|celsius|fahrenheit|hours?|hrs?|minutes?|mins?";
const CONVERT_RE = new RegExp(`(\\d+(?:\\.\\d+)?)\\s*(${UNIT_WORDS})\\s+(?:in|to|as|=)\\s*(${UNIT_WORDS})\\b`, "i");

const normUnit = (u: string): string => {
  switch (u.toLowerCase()) {
    case "km": case "kilometer": case "kilometers": case "kilometre": case "kilometres": return "km";
    case "mi": case "mile": case "miles": return "mi";
    case "kg": case "kilo": case "kilos": case "kilogram": case "kilograms": return "kg";
    case "lb": case "lbs": case "pound": case "pounds": return "lb";
    case "c": case "°c": case "celsius": return "°C";
    case "f": case "°f": case "fahrenheit": return "°F";
    case "h": case "hr": case "hrs": case "hour": case "hours": return "h";
    default: return "min";
  }
};

const CONVERT: Record<string, Record<string, (v: number) => number>> = {
  km: { mi: (v) => v / 1.609344 },
  mi: { km: (v) => v * 1.609344 },
  kg: { lb: (v) => v * 2.2046226 },
  lb: { kg: (v) => v / 2.2046226 },
  "°C": { "°F": (v) => (v * 9) / 5 + 32 },
  "°F": { "°C": (v) => ((v - 32) * 5) / 9 },
  h: { min: (v) => v * 60 },
  min: { h: (v) => v / 60 },
};

function tryConvert(text: string): Parsed | null {
  const m = text.match(CONVERT_RE);
  if (!m) return null;
  const value = Number(m[1]);
  const from = normUnit(m[2]);
  const to = normUnit(m[3]);
  const f = CONVERT[from]?.[to];
  if (!f) return null;
  const result = r2(f(value));
  return { title: `${value} ${from} = ${result} ${to}`, detail: `${from} to ${to}`, data: { value, from, to, result } };
}

function tryCalc(text: string): Parsed | null {
  const pct = text.match(/(\d+(?:\.\d+)?)\s*%\s*of\s*(\d+(?:[.,]\d+)?)/i);
  if (pct) {
    const a = Number(pct[1]);
    const b = num(pct[2]);
    const result = r2((a * b) / 100);
    return { title: `${a}% of ${b} = ${result}`, detail: "Percentage", data: { expr: `${a}% of ${b}`, result } };
  }
  const m = text.match(/(-?\d+(?:\.\d+)?)\s*([+\-*x×/÷])\s*(-?\d+(?:\.\d+)?)/);
  if (!m) return null;
  const a = Number(m[1]);
  const b = Number(m[3]);
  let r: number;
  switch (m[2]) {
    case "+": r = a + b; break;
    case "-": r = a - b; break;
    case "*": case "x": case "×": r = a * b; break;
    default: if (b === 0) return null; r = a / b;
  }
  const result = r2(r);
  return { title: `${m[0]} = ${result}`, detail: "Arithmetic", data: { expr: m[0], result } };
}

function tryTimer(text: string): Parsed | null {
  const m = text.match(/\b(\d{1,3})\s*(min|mins|minutes)\b/i);
  if (!m) return null;
  const minutes = Number(m[1]);
  if (!(minutes > 0) || minutes > 600) return null;
  const bare = /^\s*\d{1,3}\s*(min|mins|minutes)\s*$/i.test(text);
  if (!bare && !/\b(timer|focus|pomodoro|break|work)\b/i.test(text)) return null;
  return { title: `${minutes} min`, detail: "Timer", data: { minutes } };
}

function tryChecklist(text: string, now: Date): Parsed | null {
  const verb = text.match(/^\s*(?:buy|get|grab|pick up|pack)\s+(.+)$/i);
  let body: string | null = verb ? verb[1] : null;
  // A bare list ("milk, eggs, bread and coffee") is a checklist too — but not
  // when the line carries a date: "dinner with priya and anna friday 8pm" is
  // an event with two guests, not groceries.
  if (!body && text.includes(",") && !parseWhen(text, now)) body = text;
  if (!body) return null;
  const items = body
    .split(/,|\band\b/i)
    .map((s) => cap(clean(s.replace(/\b(?:un|please|thanks)\b/gi, ""))))
    .filter((s) => s.length > 0 && s.length <= 60);
  if (items.length < 2) return null;
  return {
    title: items.length > 2 ? `${items[0]}, ${items[1]} +${items.length - 2}` : items.join(", "),
    detail: `${items.length} items`,
    data: { items, done: [] },
  };
}

function tryCountdown(text: string, now: Date): Parsed | null {
  const m = text.match(/\b(?:days?\s+(?:until|to)|until|til)\s+(.+)$/i);
  if (!m) return null;
  const what = clean(m[1]).replace(/[.,!?]+$/, "");
  const year = now.getFullYear();
  let target: Date | null = null;
  const fixed: [RegExp, number, number][] = [
    [/^christmas/i, 11, 25],
    [/^halloween/i, 9, 31],
    [/^new year/i, 0, 1],
  ];
  for (const [re, mo, d] of fixed) {
    if (re.test(what)) {
      target = new Date(year, mo, d);
      if (target.getTime() < now.getTime() - 86_400_000) target.setFullYear(year + 1);
      break;
    }
  }
  if (!target) {
    const w = parseWhen(what, now);
    if (w) target = new Date(w.ms);
  }
  if (!target) return null;
  const days = Math.max(0, Math.ceil((target.getTime() - now.getTime()) / 86_400_000));
  return { title: cap(what), detail: `${days} day${days === 1 ? "" : "s"} left`, data: { until: target.getTime() } };
}

function tryEvent(text: string, now: Date): Parsed | null {
  const w = parseWhen(text, now);
  if (!w) return null;
  const whoM = text.match(/\bwith\s+([A-Za-z][\w'-]*)/i);
  const whereM = text.match(/\b(?:on|at|in)\s+(zoom|google meet|teams|phone|call|the office|my place|their place)\b/i);
  const title = stripAte(text, [...w.ate, whoM?.[0], whereM?.[0]]);
  return {
    title: cap(title) || cap(w.label),
    detail: w.label,
    data: { when: w.ms, who: whoM ? cap(whoM[1]) : "", where: whereM ? cap(whereM[1]) : "" },
  };
}

/** The note is the fallback: anything that isn't recognizably something else
 *  is still worth keeping, the way shapeshift does. */
function tryNote(text: string): Parsed {
  return { title: text.slice(0, 60) || "Note", detail: "Note", data: {} };
}

// ---------------------------------------------------------------- entry

const num = (s: string) => Number(s.replace(/[,₹$€£\s]/g, ""));

/** Everything a card could be, in the order the classifier prefers them.
 *  Each entry's confidence is how strong its trigger is — an explicit
 *  verb or pattern beats a date that merely matches. */
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

/** Parse `text` as `kind`, or null when the pattern isn't there. The
 *  classifier calls this to rank; buildCard calls it to fill. */
export function attempt(text: string, kind: CardKind, now = new Date()): Parsed | null {
  const t = text.trim();
  if (!t) return null;
  switch (kind) {
    case "poll": return tryPoll(t);
    case "reminder": return tryReminder(t, now);
    case "split": return trySplit(t);
    case "expense": return tryExpense(t);
    case "timer": return tryTimer(t);
    case "convert": return tryConvert(t);
    case "calc": return tryCalc(t);
    case "countdown": return tryCountdown(t, now);
    case "checklist": return tryChecklist(t, now);
    case "event": return tryEvent(t, now);
    case "note": return tryNote(t);
  }
}

/** Build the card. Always succeeds: an unparseable line is a note. */
export function buildCard(text: string, kind: CardKind, now = new Date()): Card {
  const t = text.trim();
  const parsed = attempt(t, kind, now) ?? tryNote(t);
  return { id: "", kind, text: t, done: false, at: now.getTime(), ...parsed } as Card;
}
