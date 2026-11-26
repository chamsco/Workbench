# Stress test: implicit states and failure loops

26 Nov 2026, `main` at 0746d2e. Method: read each state machine (stream turns, letters between
streams, goals, side streams, automations, updater, OpenCode servers, single instance), list every
state it can reach, and ask for each: can it get stuck with nothing on screen, or repeat a failure
by itself? Then run the good and failing turn in the real app.

Fixed in this PR are marked **Fixed**, each with a regression test that fails on `main` (output in
the PR) unless it is wiring with no testable boundary.

## Found

| # | Where | What happens | Kind | Status |
|---|---|---|---|---|
| 1 | Single instance | macOS and Linux had no lock at all: two copies (e.g. the .app in Downloads and in Applications) ran together on one data folder, and each wrote threads back whole from its own memory, so one silently undid the other. Windows had a mutex but no notion of channel. | two copies | **Fixed**: lock on the data folder (`instance.lock`, OS-released on crash), channel stamp, nightly gets its own folder, second start brings the running window forward on every OS. Tests: `instance::tests` |
| 2 | `Chats::spawn_reply` / `stop` | Stop then send at once (steer "now", Plan edits): if the stopped turn had already passed its last await, it still ran to the end and removed the *new* turn's handle. The stream then read idle while a CLI ran, Stop did nothing, and a second send started two replies on one thread. | implicit state | **Fixed**: a turn releases only its own handle (`release`); `after_turn` of a replaced turn leaves the letters to the new one. Test: `a_stopped_turn_ending_late_leaves_the_new_turn_running` |
| 3 | `/goal` | A goal only looked at Done replies. A failed or stopped turn left the goal on, not paused, with nothing happening and nothing said: it looked alive and was dead. | implicit state | **Fixed**: failure or stop pauses the goal and posts why, once; a reply saying the goal is met still ends a paused goal. Test: `a_goal_whose_turn_failed_pauses_and_says_why` |
| 4 | `/goal resume` | Resume only unpaused. After "paused after N turns" the nudge count was unchanged, so the next tick paused it again at once: resume did nothing. | loop (pause/resume) | **Fixed**: resume posts a turn in your name ("Carry on toward the goal"), which resets the count. UI wiring, no unit test |
| 5 | OpenCode servers | Warm `opencode serve` processes live in a static; only the legacy shell called `shutdown_opencode`. Quitting Cauldron (and every self-update restart) left them running, holding ports, on all three OSes. | leak | **Fixed**: `on_app_quit` hook + the update's forced exit |
| 6 | Letters between streams | `inbox` and `answering` are memory only. Quit while a stream waits for another's answer and the answer never comes back, with nothing on screen saying it was lost. | implicit state | Open: persist the queue (T3 keeps queued messages across restarts) or post "(lost on restart)" on load |
| 7 | Automations | If booking a run fails (agents file unwritable, disk full), the routine stays due and is posted again every 30 s, each a paid turn. | failure loop | Open: hold an in-memory "fired at next_run" mark so a routine fires once per due time even when the write fails |
| 8 | Board "Take it from here" | `let _ = chats.send(...)`: a refused send (stream busy) is dropped with no message. | silent failure | Open: show the error like the composer does |
| 9 | Retry | `retry` pops every trailing assistant message, including goal notes and other streams' notes posted after the reply. | data loss (small) | Open: pop only the reply being retried |
| 10 | Sidebar | A finished stream sits under "Needs you" with Done (see PARITY finding 1). Done, unread and waiting on you are one state on screen. | ambiguous state | Open: design round (PLAN phase 1) |

## Checked and fine

- **Startup after a crash or quit mid-reply**: Sending/Streaming become Stopped + interrupted, and
  Resume carries on. No reply is left "running" with no process.
- **Hand-off chains**: capped at `MAX_HOPS`; a letter past the cap is answered with a note, not
  retried. A side stream can't start side streams; a main is capped per user message.
- **Goal turns**: capped at `GOAL_TURNS` in a row without you.
- **Updater**: every state has a way out (Failed → check again; Staged waits for you; restart
  waits up to 10 s for the old process, then runs anyway; the old process exits by force after 5 s).
- **Automations after the app was closed**: one catch-up run, then the next time from now; no burst.
- **OpenCode server that died while warm**: one fresh start and retry, not a loop.
- **Failure card**: rate limit read from stderr, named, with Change model / Retry / Show output.
