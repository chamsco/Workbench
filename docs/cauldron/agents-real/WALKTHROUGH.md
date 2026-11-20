# Click-through of the latest GPUI app (20 Nov 2026)

Real window (Linux, Xvfb), fresh data folder, branch `claude/project-thread-0cb445` (on top of the integration branch c71dbfe). Every button below was clicked.

## Works
- New stream sends to Claude and answers; the stream opens in the sidebar under Needs you / Done.
- Sidebar app icons switch pages: Streams, Agents (Familiars), Usage (empty state).
- Right rail opens Highlights, Changes, Git, Files, Browser (each says "Open a stream to see…" until a stream is open) and the Terminal dock.
- Window buttons, sidebar fold, tabs, Settings back row, Backdrop / Frost / Stream background in Appearance.

## Placeholders (the "shell" feeling)
| Where | What you see |
|---|---|
| Sidebar > Folders | "No folders yet" |
| Sidebar > Automations | "arrive in a later build" |
| Sidebar > Library | "arrives in a later build" |
| Sidebar > Usage | "No usage data yet" |
| Right rail > Trace, Simulator | marked Placeholder |
| Settings > General, Machines, Shortcuts | "arrive in a later milestone" |
| New stream > GitHub, Worktree chips | dimmed, do nothing |
| + menu > Spawn a subagent | shown, not live (the `delegate` tool is live) |

## Missing, found by you
- No way to pick a folder (fixed on this branch: "Choose folder…", saved to the recent list).
- Only Claude could answer; no model or account picker (fixed).
- No sign-in or second account (fixed: Subscriptions page, Add account).

## Not testable here
Codex, Cursor, OpenCode, Grok and Gemini are not installed in the container, so their sign-in and answers were not run. Windows and macOS were not run.
