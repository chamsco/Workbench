# UI4: the thread view

Captured on Charles's Windows 11 PC, 11 Nov 2026, 100% scaling, client area 1240x702, dark theme, debug build.

| File | State |
|---|---|
| `01-thread-sample.png` | An open thread: user bubbles right, replies left with who answered, the model and the cost; composer; thread cost |
| `02-error-retry-sample.png` | The last reply failed: the error in red with Retry |
| `03-empty-sample.png` | A thread with no messages yet |

**Sample data.** Each shot is the real core path (`Chats::open`, `thread`, `send`), run against a data dir seeded with three made-up threads (`BACKSPACE_DATA` and `BACKSPACE_PREFS` pointed at a temp folder). No real thread is committed and no model was contacted.

## What the view does

- A row click opens the thread and marks it read. The coordinator tab takes the thread's title.
- Header: title, a Code chip and the project chip.
- Transcript: your messages on the right (at most 72% wide); replies on the left (at most 560px) with author, model and cost underneath. A failed reply shows its error, and the last one gets Retry, which calls `Chats::retry`.
- Composer: Enter sends through `Chats::send` and Shift+Enter adds a new line (both in `the_composer_sends_through_core`, which also clicks Retry and checks a new attempt replaced the failed one). While a reply runs the button turns into Stop, and a failed send shows its error under the box; those two are drawn but not run (see Not checked).
- The effort chip cycles Auto, low, medium, high, max and saves `prefs.cli_effort` (`the_effort_chip_cycles_and_saves`, against a temp prefs file).
- Loading: a reply in flight reads "Waiting for the reply" or "Writing" in place of its text. Not screenshotted: core turns a pending reply into an interrupted one when it loads a thread, so it cannot be seeded, and the sample provider fails at once. A thread itself opens from memory with no loading step.
- The usage line shows this thread's cost in dollars (tabular figures), never "% left".
- Other states: no thread open, a thread deleted elsewhere, and agent tabs (a 1:1 placeholder until core lists a thread's agents).

## Fixed while checking on screen

1. Debug builds overflowed the 1MB Windows main-thread stack when a thread opened. `build.rs` now links with an 8MB stack (MSVC only). Release builds were fine before.
2. A thread with no costs showed "$-0.00"; the sum now starts from +0.0 (test `lines_carry_who_answered_and_the_cost`).
3. A reply holding only an error drew an empty text line above it.
4. The composer text sat 10px right of the chips under it: the multi-line editor pads itself and ignores style padding, so the box is pulled back by that amount.

## Left out, not faked

- The stage stepper, ask cards and teammates. Core has no per-thread stages, asks or members list for these yet.
- Replies are plain text: markdown is not rendered.
- The route chip is not a picker yet; it only shows who answers.

## Not checked

- Light theme and 125% or 150% scaling (both need Windows settings changed).
- A reply streaming in from a real model, Stop, and the send error under the box (screenshot sessions do not contact models). Sending, the error and Retry were run end to end in `the_composer_sends_through_core` with a provider that does not exist.
