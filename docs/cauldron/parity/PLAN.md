# PLAN: Cauldron to ADE parity

26 Nov 2026. Inputs: `PARITY.md` (gaps) and `STATES.md` (state audit). House rules apply to every
phase: study the product first; for UI, at least two directions on a board, pick one with a reason,
iterate, then build; before/after screenshots in the PR; tests per `sharp-testing`.

Order is by what blocks the core loop (**ask → watch → review → ship**), not by size.

## Phase 0: no undefined states (this PR, then the open rows)

- Done here: single instance per channel; stopped-turn handle race; goal halt and resume; OpenCode
  servers on quit.
- Next: STATES rows 6-9 (lost letters, automation re-fire, board send error, retry popping notes).
  Each is small and gets one regression test.
- Rule going forward: every new state machine lists its states and exits in its module doc, and
  every failure ends in something on screen with a next step (the failure card is the pattern).

## Phase 1: status and attention (design round first)

- One vocabulary everywhere: Working, Needs you (approval, question, failure), Done (unread),
  Idle. Sidebar groups by it; done no longer files under Needs you.
- OS notification + optional sound when a stream finishes or needs you while Cauldron is in the
  background (Zed, T3, Codex). Setting in General.
- Motion on status change in the sidebar row (short, reduce-motion aware).
- Acceptance: a finished background stream raises one OS notification and one row change; a
  failure raises Needs you; nothing raises twice.

## Phase 2: the review loop (largest gap)

- Per-turn changes line under every Act reply: "3 files changed +42 −7", click opens the review.
- Review surface: files list + diff, scopes This turn / Uncommitted / Branch (T3's four scopes,
  minus commits for now), accept or revert per file and per hunk via the existing checkpoints.
- Select lines → comment → attached to the next prompt.
- "Edit from here" on a user message: revert files too / keep files (T3).
- Acceptance: an Act turn that edits two files shows the line, opens the review, reverting one hunk
  leaves the other.

## Phase 3: isolation and ship

- New stream option: this folder / new worktree (default off until Charles picks), branch named
  from the stream; cleanup when archived.
- Ship button on a stream: commit, push, open PR (reuse deliver.rs Ship), then watch checks and
  wake the stream on a failure (T3 "watch a PR").
- Warning before committing straight to the default branch.

## Phase 4: steering and approvals

- Queued messages shown as chips above the box: edit, remove, send now. Persist the queue.
- Long-lived sessions (Claude stream-json input, `codex app-server`, `opencode serve` already) so
  approvals are inline Approve / Decline on the tool step instead of decided up front.
- Context near the limit: banner with "New stream from summary".

## Phase 5: speed of use

- Keyboard: mod+1..9 streams, mod+. stop, mod+shift+M picker, mod+shift+A access mode, mod+Enter
  send in background, Esc closes overlays. A shortcuts sheet in Settings.
- Command palette: mod+K becomes actions + streams.
- First-run checklist: CLIs found, signed in, a folder, your name (fixes the "Y …" row).
- Copy pass: "stream" everywhere; label the right rail or reduce it; Send is the accent button.

## Phase 6: channels

- A nightly workflow that writes `channel_stamp.rs` = "nightly", publishes `windows-nightly` /
  `mac-nightly`, and an updater that reads its own channel's release. This PR only makes the two
  able to run side by side safely.

## Not in this plan

Visual redesign. The look Charles approved stays; phases 1-5 add surfaces in it.
