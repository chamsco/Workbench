# Workbench v2 — the current build prototype

File: `design/workbench-minimal-v2.html` (the original
`design/workbench-minimal.html` is untouched).

v2 keeps the original Direction 04 stage: warm paper, the floating rounded
window on the ambient field, the slim project rail, the warm sidebar, the quiet
type and the notes under the stage. It updates the inside to the current app:

- **Header:** the four mode icons — Code, Work, Canvas, Crew — centered in the
  top bar.
- **Top of the sidebar:** the five attention/work icons — Needs you, Agent
  board, Code review, Automations, Limits.
- **Footer:** Today, Machines and Team centered; machine state on the left;
  Settings, Night mode, Glass, Help and the account on the right.
- **Code:** token/cost usage centered directly under the chat box.
- **New views:** Today, Machines and Team, with the current content and the
  sidebar/day/machine/bot lists.
- **Existing views:** Home, Code, Crew and Limits remain from the original
  prototype, so the v2 is still the full design surface, not a single screen.

Use the page chips at the top to switch screens (`? #today`, `#machines`,
`#team`, `#code`). The window can still be switched between Mac/Win/Linux and
light/dark from the original controls.

## Direction

- **Product:** a local AI-agent workspace.
- **Character:** calm, exact, a little warm.
- **World:** paper desk, terminal, workshop bench.
- **Moments:** the day timeline and the Code transcript.
- **Signature:** three rails and one live status dot.

## What is deliberately kept

- The floating app window and ambient backdrop.
- The 64px rail / 264px sidebar proportions.
- The white cards, warm paper ground, near-black ink and pastel people.
- The original card, side list, code thread and session footer patterns.

## What changed

- The old page-head text chips now switch the v2 screens.
- The current three-rail chrome replaces the single top pill row and the
  rail-bottom user cluster.
- The new views use the same tokens and component styles as the original; no
  new palette or font.
- The Code view gets a centered usage line under the composer.

Screenshots: `docs/screenshots/2026-11-05/v2-home.png`, `v2-today.png`,
`v2-machines.png`, `v2-team.png`, `v2-code.png`.
