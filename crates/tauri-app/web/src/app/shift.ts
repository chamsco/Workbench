// When does a talking stretch become a new session?
//
// A thread is one long conversation, but a session is a sitting, and a sitting
// has a subject. "The banner is ready for review" and then "by the way, can you
// schedule dinner with Aya at 5?" are not one subject — the second is a
// different errand, and it deserves its own title, its own clock and its own
// marker to scroll back to. This file decides, purely and testably, and
// sessions.ts records the answer.
//
// Three signals, deliberately conservative — a false new session is worse
// than a missed one, because it chops a conversation in half:
//
//   * pivot — the user says so: "by the way", "unrelated", "switch gears".
//   * intent — the line is a different KIND of thing than the sitting (an
//     everyday card, a schedule change, a new bot) while the sitting was a
//     conversation, and the two share no subject words.
//   * never — a continuation ("also", "still", "and…"), or a line that keeps
//     the sitting's subject.

import { classify } from "../shapeshift";
import { NEW_AGENT, ROUTINE_INTENT } from "./router";

export type ShiftReason = "pivot" | "intent";

/** The user said the subject is moving. */
const PIVOT =
  /^\s*(by the way|btw|unrelated(ly)?|different (topic|question|subject|thing)|switch(ing)? (gear|gears|topics|subject)|on another note|separately|quick (question|favor|one)|while i remember|new topic|change of subject)\b/i;
/** The user is still inside the sitting. Checked before everything else, so
 *  "also, what about the deploy?" never starts a new one. */
const CONTINUATION = /^(and|but|so|also|plus|still|then|what about|how about|ok|okay|sure|yeah|yes|no|nope|thanks|right|now|actually)\b/i;
/** The same bar the hero uses to call a line a card, not a message. */
const CARD_SURE = 0.72;

const STOP = new Set(
  ("a an the this that these those is are was were be been being am do does did done have has had having i you he she it we they me him her them my your his its our their "
    + "to of in on at for with from by about into over after before between under again further once here there when where why how all any both each few more most other some such "
    + "only own same too very can will just should now and but or if while as up down out off over under it its it's let lets please make makes made get gets got need needs needed")
    .split(/\s+/),
);

const words = (s: string) =>
  new Set(
    s
      .toLowerCase()
      .replace(/[^a-z0-9\s']/g, " ")
      .split(/\s+/)
      .filter((w) => w.length >= 4 && !STOP.has(w)),
  );

/** What kind of thing a line is, in the router's vocabulary. */
function kindOf(text: string): "card" | "routine" | "new-agent" | "chat" {
  if (NEW_AGENT.test(text)) return "new-agent";
  if (ROUTINE_INTENT.test(text)) return "routine";
  const top = classify(text)[0];
  if (top && top.kind !== "note" && top.confidence >= CARD_SURE) return "card";
  return "chat";
}

/** Any shared subject word counts as the same subject. */
const sharesSubject = (a: string, b: string) => {
  const wa = words(a);
  for (const w of words(b)) if (wa.has(w)) return true;
  return false;
};

/** Does this line start a new sitting in a session already titled `title`?
 *  Null means no: keep talking in the sitting you are in. */
export function detectShift(text: string, title: string | undefined): { reason: ShiftReason } | null {
  const line = text.trim();
  if (!line || !title) return null;
  if (PIVOT.test(line)) return { reason: "pivot" };
  if (CONTINUATION.test(line)) return null;
  const became = kindOf(line);
  if (became === "chat") return null;
  // A different kind of thing, on no shared subject: a new errand. "also add
  // milk" to the list you were writing shares "milk", so it stays put.
  if (kindOf(title) === became) return null;
  return sharesSubject(line, title) ? null : { reason: "intent" };
}
