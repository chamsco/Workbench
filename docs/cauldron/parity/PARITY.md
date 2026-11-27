# Cauldron vs other ADEs: parity

26 Nov 2026, against `main` at 0746d2e. Cauldron was run headless (Linux, Xvfb) with a fake `claude`
for a good turn and a failing one; the rest is from the code. Competitors are from their docs and
changelogs (Codex app, Cursor 3 Agents Window, Zed Agent Panel, Conductor, T3 Code) plus the
earlier source studies of Orca, T3 and Zuse (`cauldron/agents-real/RESEARCH.md`,
`cauldron/streams-study/STUDY.md`). Zuse is AGPL: studied, never copied.

**Verdict.** The look is ahead of most of the field; the working loop is behind it. Every
competitor treats *review the change, then ship it* as the core loop. In Cauldron a turn ends with
text and a Done card; the diff lives behind an unlabeled icon in the right rail, there is no
accept/reject, no per-stream worktree, and nothing tells you a stream finished while you were in
another app. Those four gaps matter more than any visual polish left.

## The matrix

✅ = has it, ◐ = partial, ✗ = missing. "Who" = which products set the bar.

| Area | What good looks like | Who | Cauldron today |
|---|---|---|---|
| **Parallel streams** | Sidebar of sessions grouped by project, status per row, jump with mod+1..9 | all | ◐ sidebar, status pills, folders; no mod+1..9, no per-project grouping by default |
| **Status vocabulary** | idle / working / waiting on you / done / failed, consistent everywhere | T3, Conductor, Codex | ◐ pills exist, but a *finished* stream is filed under "Needs you" (screenshot flow-2): done and waiting read the same |
| **OS notifications** | Desktop notification + optional sound when a turn ends or needs approval while the app is in the background | Zed, T3, Codex | ✗ in-app bell only |
| **Isolation** | Each stream in its own git worktree/branch, opt-in or default | Conductor, Codex, Cursor, T3, Zed | ✗ streams run in the folder itself; two streams on one repo collide |
| **Diff review** | Changes summary under the turn (files, +/-), multi-file review, accept/reject per hunk, comment on lines into the next prompt | Zed, T3, Conductor, Cursor | ◐ Changes panel (support card) reads the working tree; no per-turn scope, no accept/reject, no line comments |
| **Checkpoints / undo** | Per-turn snapshot, "edit from here" with revert files / keep files | T3, Codex | ◐ core takes a hidden-ref checkpoint per message and can restore; UI entry point is thin |
| **Ship** | Commit, push, open PR, watch checks, from the stream | Conductor, T3, Codex, Cursor | ◐ core Verify/Ship stage exists (deliver.rs) for familiars; streams have no Commit/PR button |
| **Queue and steer** | Messages typed mid-turn queue visibly; edit/remove queued; "send now" | Zed, T3 | ◐ `steer` queues and Plan edits use it; the queue is not shown as editable chips; queue is lost on restart (T3 keeps it) |
| **Approvals** | Inline Approve/Decline on the tool call, also from phone; permission modes on the composer | T3, Zed, Codex | ◐ Ask first / Act / Plan on the composer; approvals are decided up front per turn (one CLI process per turn), not mid-turn |
| **Model picker** | Searchable, favorites, provider rail, effort beside it, keyboard shortcut | T3, Zed, Zuse | ✅ Picker A/B with rail, search, favorites; ✗ no shortcut |
| **Command palette** | mod+K for actions, not only search | Conductor, T3, Zed | ◐ Cmd/Ctrl+K searches streams only |
| **Keyboard coverage** | New, switch, stop, picker, approve, review, all bound | T3, Zed | ✗ two bindings (New stream, Search) |
| **Context awareness** | Token use visible, warning near the limit, "new from summary" | Zed | ◐ context map exists in core; no near-limit banner |
| **Terminal** | Per-stream terminal drawer | T3, Conductor, Zed | ✅ terminal canvas |
| **Browser / preview** | Built-in browser with port forwarding | T3, Codex, Cursor | ◐ browser surface exists |
| **Error recovery** | Failure says why and offers the next step; auto-resume after quota reset; retry with backoff | T3 | ✅ failure card names the cause (rate limit) with Change model / Retry / Show output; ✗ no auto-resume at reset time |
| **Onboarding** | First run checks CLIs and logins and walks through what is missing | Conductor, Zed, T3 | ◐ picker shows not-installed/not-signed-in; no first-run checklist |
| **Empty states** | Every empty list says what to do, in the product's own words | all | ◐ "No threads yet… Start one with New thread" beside a "New stream" button (naming rule is "stream") |
| **Motion** | Short, purposeful: row status changes, tray open, streaming shimmer; respects reduce-motion | Zed, Codex | ✅ tray unfold, ledger sweep, wipe, reduce-motion read from the OS; ◐ no motion on status change in the sidebar |
| **Instances / channels** | One copy per channel; nightly beside stable | T3 (`T3CODE_CHANNEL`), Cursor (Nightly) | ◐ → ✅ in this PR (was Windows-only, no channel) |
| **Mobile / remote** | Approve and review from the phone | T3, Codex | ◐ companion API + iOS client exist; not on the current shell |
| **Automations** | Scheduled tasks inside a chat | Codex | ✅ familiars' routines |

## Where Cauldron is ahead

- **Side streams with a cheaper model** and `delegate` across CLIs (only T3 has delegation; nobody
  else sends the side work to a smaller model by default).
- **Multi-CLI, multi-account** in one picker (Claude, Codex, OpenCode, Gemini, Cursor, Grok).
- **Goals** (`/goal`) that keep a stream working, with a turn cap.
- **The stream ledger** (tool steps folded live) is closer to Zed's than to T3's raw log.
- **Visual identity**: frame, lifted cards, dither backgrounds. Nobody else has one this distinct;
  keep it, but it cannot stand in for the review loop.

## UX findings from the run (current main)

1. Finished stream filed under **Needs you** (flow-2). If "needs you" means unread, call it Unread;
   keep Needs you for approvals and failures.
2. Copy drift: "No threads yet / Start one with New thread" beside the **New stream** button.
3. On an empty composer the only accent button is the **mic**; once you type, Send appears beside
   it in white (flow-1). The strongest button on the screen is never Send.
4. The right rail has nine icons with no labels; Changes (±) is the most important and is the
   fourth one down.
5. The footer's stage words (Ask Plan Build Test Verify Ship Learn) look like links but read as
   decoration; "No usage data yet" sits under them on a fresh install.
6. The user row shows "Y …" when no name is set: the empty name is never asked for.
7. A good turn ends with only Copy and ⋯ under the reply: no "N files changed" line, so the
   result of an Act turn is invisible until you open the right rail.
8. The failure card is good (cause, plan B, output). Keep it as the pattern for every failure.

Screenshots: `docs/cauldron/parity/shots/` (main-0 = empty, flow-2 = good turn, fail-1 = failure).
