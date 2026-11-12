# UI3: the sidebar thread list

Captured on Charles's Windows 11 PC, 11 Nov 2026, 100% scaling, client area 1240x702, dark theme, debug build.

| File | State |
|---|---|
| `01-groups-fixture.png` | Unread, Running, Failed, Done with counts; one row selected; a pinned row |
| `02-loading-fixture.png` | Loading: static skeleton rows shaped like the real ones |
| `03-empty-fixture.png` | No threads yet |
| `04-error-fixture.png` | Core could not open (the message is made up for the screenshot) |

**Fixtures.** Every screenshot here is a debug build with `CAULDRON_FIXTURE_THREADS=list|loading|empty|error`: made-up thread titles, so no real thread is committed. Without the variable the sidebar opens core's thread store (`Chats::open` on its own runtime, then `Chats::list()`, refreshed on core's change channel); that was checked on screen against real data, debug and release builds, and not saved. Only the store is opened: `Fleet`'s scheduled routines, dreams, relay, companion and sharing are not started by this window.

## What the list does

- Groups, in order: Unread (an unopened reply, where the wireframe's Needs you sits), Running, Failed (red label), Done. A thread in two states goes to the first of Running, Failed, Unread. Empty groups are left out. Rows keep core's order: pinned first, then newest.
- Row: unread dot, kind glyph, title (bold when unread), pin, age (tabular figures), project folder or last message.
- Search filters title and last message as you type (tested by typing into the box); "No threads match" when nothing does.
- Unread and Running reset at launch: core keeps both in memory, so Unread covers replies that arrive while the app is open, and Running fills in once threads run from this window (UI4). Against real data today the list shows Done only.
- Click selects a row; Up and Down move the selection. Selecting does not open the thread or mark it read; opening is UI4.

## Left out on purpose

Needs you per thread, Hibernating, Archived, the agents line and graph, the Design kind (core does not mark a thread as design work yet), and the usage meter (it sits on the stage bar, UI4, as tokens and cost). See `docs/cauldron/UI3-CORE-MAP.md`.

## Not checked

Light theme, 125% and 150% scaling, typing into search on screen (typing is tested headless; the input is gpui-component's, with caret, selection and text colours set from the tokens), the no-match state on screen (tested only), a screen reader.
