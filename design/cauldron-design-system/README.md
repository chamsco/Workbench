Cauldron is a desktop app for running coding agents. The interface is a quiet instrument: neutral surfaces stacked in tone steps, one purple accent that only ever means "this wants you", and facts written as numbers and words. Build every screen from these tokens; a value that is not here is a question for the design thread, not a new hex code.

## Voice

- Write what happened and what will happen, with the real numbers: "Pushing starts the Windows build", "11 passed", "118k of 200k".
- Sentence case everywhere. No exclamation marks, no emoji, no "Oops".
- Name actions by verb and object: "Create PR", "Delete thread", "Open diff". Never "Confirm" or "OK" on a destructive action.
- Agents speak in the first person in the thread; the chrome speaks about them in the third ("Builder needs you").
- Units once and compact: "3m 12s", "$0.84", "41k tokens", "5 in 90 d". Write "7 of 11 shown" when a list is cut.

## Surfaces and depth

- Stack surfaces in tone, not lines, as Tuff UI's Surface does: `bg`/`frame` (window, sidebar, top bar) holds a Shell (`card`), which holds a Panel (`panel`), which holds Insets (`inset`: tool rows, code, tags, switch tracks). In light the panel is white on a grey shell; in dark it is darker than the shell.
- Shells and panels take no shadow; a panel gets a 1px `line` ring. Only overlays lift: menus and popovers use `raised` with `shadow-menu`, tooltips `shadow-tip`, the composer `shadow-box`.
- Hairlines (`line`) separate rows inside a card. `line-strong` is for the composer edge, menu rings and Kbd. `line-control` (3:1) is for edges that carry meaning, such as an off toggle.
- Hover is a fill (`hover`), selection a deeper fill (`selected`). Never show hover or selection with a border or an accent bar.

## Colour

- One accent. `accent` marks the primary button (one per view), the focus ring, the needs-you dot and the current stage. If two things on screen are accent and only one needs Charles, remove the other.
- Status is a glyph plus a word. Done is an `fg-2` check, not green. Running is an `fg-2` spinner and a timer. Only failure (`bad`), warnings (`warn`) and needs-you (`accent`) add colour.
- `ok` is for passed checks and the + side of a diff only.
- Agents keep colour: `agent-1` to `agent-9` are for avatar figures and tab dots, never for UI state.
- Text: `fg` primary, `fg-2` secondary, `fg-3` metadata and placeholders (4.5:1 on every surface in both themes), `fg-4` disabled only.
- On accent fills use `on-accent`: white in light, near-black in dark.

## Type

- UI text is `body` (13/18). Metadata is `meta` (12/16). Agent prose in the thread is `reading` (14/22, max 72ch). Card titles are `title` (15/20); page headings `heading`; `display` only on first run and empty states.
- The face is Geist, the same as Tuff UI (SIL Open Font License, so it may be bundled), with Geist Mono for code. Switch on the stylistic sets `cv11` and `ss01` everywhere, as Tuff does. Bundle the Geist and Geist Mono font files with the app: Windows and macOS do not ship them. Fall back to Segoe UI Variable on Windows and SF Pro on macOS.
- Identifiers are mono (`mono-tag` in tags): models, branches, files, tools, commands. Prose is never mono.
- Turn on tabular numerals for every number that changes or lines up.
- Two weights: 400 and 600 for text, 500 for controls and labels. No 700, no italics in chrome.

## Space, size and radius

- Spacing steps are `space-2` to `space-48`; window margins and card padding are `space-12`, rows pad `space-8`.
- Controls are 28 (`size-control`), 24 in dense places, 32 for tabs and tool rows. Sidebar rows are 28, two-line rows 44.
- Radii are Tuff's and nest by the 6px shell padding: `radius-shell` 22 (shell, window, composer shell), `radius-panel` 16 (panel, cards, dialogs), `radius-inset` 12 (insets, tool rows, menus), `radius-control` 10 (buttons, rows), `radius-inner` 8, `radius-tag` 6. `radius-round` is for circles only.

## Motion

- Layout moves use `ease-settle` over `dur-layout` (300ms). Panel content switches over `dur-panel`. Menus arrive over `dur-menu` and leave faster over `dur-exit`. Colour changes over `dur-swap`.
- Nothing bounces, nothing overshoots. Exits are faster than entrances.
- Running work shows a spinner, a live timer or a slow shimmer on one label, never all three on one row.
- When the OS asks for less motion, every move becomes a `dur-reduced` cross-fade and shimmers stop.

## Focus and keyboard

- Every focusable control shows a 2px `focus` ring at 2px offset, solid, at least 3:1 on frame, card and raised.
- Primary actions in a focused card show their shortcut as a `Kbd`: Ctrl ↵ approve, Esc deny on Windows; ⌘↵ on macOS.
- Lists move with arrow keys, Home and End; Esc closes the topmost overlay.

## Iconography

- Lucide icons, outline, 16px at stroke 1.75 in controls and 14px at stroke 2 inside rows and labels. Colour follows the text they sit with (`fg-2` at rest).
- One icon per control. No filled icons, no duotone, no emoji.
- Avatars are imagery, not icons: the flat two-circle agent figure in the agent's colour.

## What not to do

- No glow, no gradient fills, no glass or blur on panels, no dot-texture behind text.
- No coloured pills for status, no coloured left-border cards, no hero numbers.
- No centred wordmark in the chrome; the logo lives only on first run.
