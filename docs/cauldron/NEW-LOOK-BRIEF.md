# New look: build brief (2026-11-12)

Charles, 13:40Z: "From now on, this is the new look of the app." It replaces the round 21 layout. Top bar A stays.
Source: `design/new-look/directions-v2.html` (open it in a browser) and the `r3-dark-*.png` / `r3-light-*.png` beside it.
Screens: 0 new thread, 1 thread with drawer, 2 five drawer panels, 3 terminal beside thread, 4 settings, gal both sidebars folded.

## Order (do not start step 1 before the top bar A merge, `MERGE-NOTES.md`)

0. Merge gg3ogp into sa3a1p (Charles must type "merge"). Resolve `shell.rs` as in MERGE-NOTES.
1. Frame. Sidebars sit flat on the window frame; the main card is the only surface. Folded sidebar = 52px rail, 12px icon inset, both sides (measured: left 0-52, right 1148-1200 on a 1200px window).
2. Left sidebar (248px): apps row, New thread (Ctrl N), Search (Ctrl K), threads grouped by state (Needs you, Running, Done), grey repo tag per row ("no repo" when none), user row at the bottom.
3. Tabs: this thread's agents only (floating, from the card's left edge). A new thread has no tabs and a centred box.
4. New thread: no folder or branch. This machine, Worktree or Isolated environment, stage Execute.
5. Thread: breadcrumb header, one-line step ledger, question card, result card (hairline list, facts row, two buttons), composer, stages on the right under the box.
6. Right drawer (248px, mirrors the left): icon strip Highlights, Patches, Git, Agent trace (sub-agents nest like a git subtree), Dev (preview of the app in development). Folded = 52px rail.
7. Terminal: a second card, Warp-style blocks, takes the drawer's place.
8. Settings: swaps the sidebar list; the page takes the card.

## Core does not have these: draw nothing, no placeholder numbers
- Per-subscription usage badges (and "62% left this week" in the user row)
- Needs you count and the "1 needs you" top-bar badge (after UI5)
- Trace timings, dev server URL and status
- A reply path for the question card (needs Charles's OK to change core)

## Already decided elsewhere (keep)
- Avatars: flat figure, cog head for working, three small heads for typing, bust with outline in the chat status line (`design/cauldron-avatars-and-thread.html`, `crates/chrome/src/avatar.rs` on sa3a1p).
- The wireframe draws sphere icons on the tabs. Those are placeholders; use the approved figure, and ask Charles before busting rows and tabs.

## Tokens will change
The design system thread is moving tokens to a Tuff UI base (Geist, 22/16/12 radii, purple accent), not yet approved for the app. Do not edit `tokens.rs`; read every colour, radius and size from it so the switch is one place. Wireframe values (radii, accent) are layout intent, not final tokens.

## Check on screen
Each step: screenshot before and after, at 1200px and at the narrow width, against the matching `r3-dark-*.png`. Measure the 52px rails; do not assume.
