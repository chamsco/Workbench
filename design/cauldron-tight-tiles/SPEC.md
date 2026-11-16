# Cauldron UI: Tight tiles (approved)

Approved by Charles on 15 Nov 2026 (20:25Z) as the current UI of Cauldron. `index.html` is the board (open it in a browser; it has dark and light, Painting and Plain, and the Mac and Windows frames). It supersedes everything in `design/legacy-ui/`. The design system in `design/cauldron-design-system/` stays: Geist, purple accent, tone steps, components. Only the geometry below changes.

Wording: the UI says **stream**, not thread. The board and older docs still say "thread" in places; code and copy follow this rule from now on.

## Rules

1. The right split is always a **support panel for the current stream** (Browser, Changes, Git, Files, Simulator, Terminal, Trace, Highlights). Never a second stream; a second stream is another tab.
2. Open state is tight tiles: stream card and support card, 4px gap, no tile shadows. The support card has its own tabs (3 visible, the rest under "+N") and a + menu (Browser, Terminal Ctrl+J, Files, Changes, Simulator, History).
3. Folded right side is one card with a 44px quick-button rail inside its right edge. Open and folded use the same rail.
4. Window buttons live in a notch cut into the top-right of the card (Windows, 100x40, one svg mask, outline drawn once at window level). macOS has no notch; lights sit top-left. Never half on the frame and half on a card.
5. Terminal docks under the stream (Ctrl+J). It lives in one place only.
6. Folded left sidebar shows a hover indent, not a floating card: the same frosted frame layer with the card clipped back (72px Mac, 48px Windows), full height, rounded-end hairline, no shadow, drawn over the card with no reflow.
7. Theme: Harbour painting behind the whole window (dusk dark, day light, from `legacy/tauri-app/ui/art`). One frosted frame layer (28px blur, tint 52% light, 70% dark). Cards are glass (10px blur, about 58% tint). Reading surfaces are more opaque (composer 82%, terminal and diff 70%). Motion (shader or GIF) only on the new-stream screen. Plain (the solid frame) is the default; Painting is a choice in Settings > Appearance (Charles, 16 Nov).
8. GPUI has no backdrop blur: use pre-blurred, cached crops of the painting per region.

## Screens

- **Sidebar** (236px): apps row (Streams, Folders, Automations, Agents, Usage, Library), search, New stream, streams grouped Needs you / Running / Done, user row with version and cog, usage badges under the box. Flat on the frame.
- **Stream**: header with open-in split button, list, dock toggle; ledger; scroll ticks on the right edge (one per response, accent = current, click jumps); composer with context bar.
- **New stream**: centred. Box with a Plan toggle next to "Ask first" (accent when on; nothing is edited until the plan is approved). Bar under the box: folder (including "No folder") and worktree on the left; GitHub and run location on the right. Run menu: This machine, other machines, Cloud sandbox, Add a machine. GitHub reading: link or create a PR for the repo (to confirm).
- **Onboarding** (6 steps): Welcome, Agents, Machines (instead of sign-in), Import, Make it yours (theme and Ask first / Approve routine / Full access), Ready.
- **Loader**: Cauldron outline mark with a thin light-thread travelling around it (white in dark, ink in light); reduced motion shows the static mark. The mark is a placeholder until Charles confirms a logo.

## Token deltas (applied in `tokens.json`, `tokens.css` and `crates/chrome`)

| Token | Now | New |
|---|---|---|
| Card radius | 16 (panel), 22 (shell) | 6 (Charles, 16 Nov) |
| Control radius | 10 | 6 |
| Tag radius | 6 | 4 |
| Gap between cards | 6 | 4 |
| Window corner | 22 | the OS's |
| Tile shadows | some | none |
| Top bar | 44px | removed; tabs live in the 38px card header |
| Support card | 200px drawer | 470px (360 to 50%) |
| Sidebar | 236 | 236 (unchanged); fold rail 52px unchanged |

## Not copied from Zuse

Logo, space-invaders backdrop, dithered green buttons, copy.

## Open questions

Whether the mark is a placeholder. What "GitHub" in the new-stream bar must do. The rest of the cut-off message ("And for the...").
