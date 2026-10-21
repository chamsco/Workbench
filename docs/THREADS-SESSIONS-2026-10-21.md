# Threads and Sessions — the one-transcript model

Written 21 Oct 2026, against `6a23149`, answering the owner's question:
*"messages are 1 element and not threads — it should be one long iMessage that's
separated into threads on the sidebar for ease of search and separation?"*

**Yes — that is exactly the model, and it is what the code does.** This document
is the record of it: the shapes, the rules, where each one shows on screen, and
the keypoints a later search needs to find this work again.

## The model in one page

```
Thread  ── one conversation, one long iMessage-style transcript
 │        (a bot's thread, a group's, or the plain chat)
 │
 ├─ Message … Message … Message …      one element per message, in order,
 │                                     never nested, never split off
 │
 └─ Session #1 ── Session #2 ── #3     sittings INSIDE the one transcript
```

Three words, kept distinct on purpose:

- **Thread** — the whole conversation with one bot (or one group, or the plain
  chat). It is the iMessage: one continuous transcript, oldest at the top,
  newest at the bottom. Nothing in a thread is ever "moved" to another thread;
  the only thing that forks is a *branch*, which is a copy, not a split.
- **Session** — one *sitting* in that thread. You talk about the banner review
  for twenty minutes, then "by the way, can you schedule dinner with Aya at 5?"
  — that second errand is a new session **in the same thread**, with its own
  title, its own clock, and its own place to scroll back to. A session is a
  label on a stretch of the transcript, not a container messages live in.
- **Sidebar entry** — navigation. The sidebar lists threads (under each bot,
  under Group chats, plus artifacts) so you can find and separate work. A
  sidebar row is a *door into* the transcript, never a second copy of it.

Why the sidebar is navigation, not structure: the transcript has to stay one
element so search, scroll-back and branching keep working on the whole
conversation. Sessions give the separation — "that dinner thing" is findable
by its title without the messages leaving the thread they belong to.

## The shapes

### Thread — `crates/core/src/chat.rs`

```rust
Thread {
  id, title, created, updated, route,
  messages: Vec<Msg>,        // the one long transcript
  life: Life,                // Keep | Ghost | Temporal — fixed at creation
  agent: Option<String>,     // a bot's thread
  members: Vec<String>,      // a group's thread (turn order)
  project, goal, hidden, …
}
```

A thread's life is decided **before it exists** (`Scope.life` on `chat_new`)
and never changes afterwards — the only way out of a ghost is to delete it.
`chat_purge_ghosts` scrubs leftovers when ghost mode is switched on.

### Session — `crates/tauri-app/web/src/app/sessions.ts`

```ts
Session {
  id: string;          // `${threadId}#${n}` — unique per sitting
  threadId: string;    // the one transcript this is a stretch of
  n: number;           // 1, 2, 3… within the thread, oldest first
  title: string;       // what the sitting is about (first line, until renamed)
  agentIds: string[];  // who is in it
  machine: string | null;
  startedAt: number; lastAt: number;
  state: "awake" | "concluding" | "idle";
  endedAt: number | null;
}
```

State lives in the face for now (`localStorage`, key `bs.sessions.v1`, capped
at 300) — the same deliberate stopgap as the cards. The seam is already cut:
`load`/persist are the only two functions touching localStorage, and the fields
a core session would own (`title` written by the bot, `concludedAt` set by the
core's wrap-up prompt, the idle timer on the scheduler's clock) already carry
those names. A later `session_*` command set replaces this file and nothing
else.

## The rules

**A session opens** when a bot's (or group's) thread is first opened —
`ensureSession` in `sessions.ts`, called from `openAgentThread`, `createGroup`
and `send` in `bridge.ts`. Idempotent: reopening the same thread never
restarts the clock, and an idle sitting wakes where it left off.

**A session renews when the subject moves** — `detectShift` in
`app/shift.ts`, wired at `bridge.send()`. "By the way, schedule dinner…" is not
more of the banner review; `renewSession` closes the previous sitting
(`concluding` if it was still awake) and opens a new one titled by the line
that started it. Pivot markers ("by the way", "new topic", "switching gears",
"unrelated", …) open it; a continuation guard ("and", "also", "but", "so",
"then", …) keeps the same sitting; a different errand *kind* (schedule vs
build vs question) counts too.

**A session concludes** when either side says it is wrapping up —
`isGoodbye` ("no thanks that's all") → `concludeSession`. The wrap-up runs a
countdown (`IDLE_MINUTES = 15`) shown as "wrapping up · ends in 3:58".

**A session goes idle** when the talking stops for 15 minutes —
`liveState` applies the clock on read, so idle is derived, not stored twice.
Idle is not closed: one more word and `touchSession` wakes the same sitting.

## Where it shows

| Surface | What you see |
|---|---|
| Sidebar (`Agents.tsx`, `ThreadList.tsx`) | Bots → folders → **threads** (and cards). One row per thread; the row opens the whole transcript. Sessions are not rows here — the sidebar separates *work*, the thread holds the *words*. |
| Bot page (`agent-page.tsx`) | The thread's **sessions**, newest first, each with its title, state, End / End now / Reopen, rename, and a click that scrolls back to where that sitting began. |
| Thread header (`SessionBar` in `Agents.tsx`) | The current sitting: "12m in", "wrapping up · ends in 3:58", "ended 20m ago" — plus End session / End now / Pick back up. |
| Transcript (`ThreadView.tsx`) | A marker where each session opens (`id={session-${id}}`, `data-session`), so a session row's scroll-back lands exactly on the first message of that sitting. Messages map to their session by time (`sessionForMessage`): the latest sitting that had already started when the message was sent. |

## The rules of the road this model keeps

- **One message, one element.** Nothing nests, nothing forks on topic shift —
  a new *session* is a label, the messages stay in place. Branches
  (`chat_branch`) are copies for side quests, not a restructuring of the
  transcript.
- **Search works on the whole conversation.** Because the transcript is one
  element, a search hits the thread and then the session title tells you which
  stretch of it matters.
- **Sessions are the seam to the core.** When sessions move server-side, the
  face keeps calling the same names; only `sessions.ts` is replaced.

## Related, same round

- **Group addressing.** In a group, `@Name` aims a message: the core's
  `named_in` (`crates/core/src/chat.rs`) parses the names case-insensitively
  and queues only the members named; the composer (`Composer.tsx`, roster from
  `ChatApp.tsx` via `thread.members`) autocompletes them — prefix before
  substring, Enter/Tab completes, Esc dismisses. The group system prompt tells
  members to hand work on the same way.
- **Ghost one-offs.** `Life::Ghost` on `chat_new` — now honoured by every
  creation path (`send`, `openAgentThread`, `createGroup`), so a one-off can be
  a plain chat, one bot's thread or a group. Never written to disk, never
  listed, deleted when you leave. Documented in the README's Chat section.

## Keypoints — for the next search

```
threads-sessions · one-transcript · iMessage model · sidebar is navigation
Session{id threadId n title agentIds machine startedAt lastAt state endedAt}
sessions.ts · bs.sessions.v1 · ensureSession · renewSession · touchSession
concludeSession · endSession · liveState · sessionLabel · sessionForMessage
scrollToSession · IDLE_MINUTES=15 · awake · concluding · idle
shift.ts · detectShift · pivot markers · continuation guard
SessionBar · agent-page sessions list · ThreadView session markers
data-session · session-${id} · scroll-mt-28
Thread.life · Life::Keep | Ghost | Temporal · Scope.life · chat_new
chat_purge_ghosts · setGhost · GhostToggle
named_in · @Name group addressing · Composer roster autocomplete
```

## Context — why it is built this way, and what it deliberately does not do

The owner's framing — "one long iMessage separated into threads on the sidebar
for ease of search and separation" — rules out the two shapes this is not:

- **Not one chat container per errand.** That would move messages out of the
  conversation they belong to and break whole-transcript search and
  scroll-back. (The old model this replaced.)
- **Not sessions as separate threads.** A session has no messages of its own;
  it is a named stretch with a clock. If a sitting ever needs its own
  transcript, that is a *branch*, which already exists.

What it does not do yet: sessions are face-side only (localStorage), titles are
the first line until a human (or later, the bot) renames them, and the idle
clock runs in the browser, not the scheduler. All three are the documented
seam — the core command set `session_*` is the intended next step, and nothing
else in the face changes when it lands.
