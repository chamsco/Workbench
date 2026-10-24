# Design system

The goal is one look across the frame, every app and every agent. This file sets
the tokens, type, shapes, components and the mascot. New code follows it. Old
code moves to it when it is touched.

## What is wrong today

Measured on 2026-10-24:

- **Four token sets.**
  - `ui/style.css`: `--fg`, `--pane`, `--glass`, `--pill`, …
  - `ui/shell.css`: the frame's `--frame-bg`, `--dcard`, `--tile-on`, …
  - `web/src/styles.css`: the React face's `--surface`, `--well`, `--border`,
    `--foreground`, …
  - Inline hex values in components (the context map, folder cards, toggles).
- **Ten font sizes** in the React face alone (10.5 to 24px). The most used are
  12.5, 11.5, 12, 13 and 13.5px, which are too close to tell apart.
- **Twelve corner radii** in the frame CSS (2 to 14px, plus 50% and 999px).
- **The same control built twice.** The toggle, the pill and the segmented
  control exist in the vanilla frame and again in React.

None of this shows in one screenshot, but it shows in all of them side by side.
The fix is the tokens below. Restyling every page by hand is not.

## Tokens

There is one set, defined once, in `ui/tokens.css` (not created yet; it is the
first task in TODO.md). The vanilla frame and the
React face (`web/src/styles.css`) both import it. Names are semantic. Each one
has a light and a dark value, and the glass modes only change the surfaces.

### Surfaces

| Token | Use | Light | Dark |
|---|---|---|---|
| `--bg` | The window behind everything | `#f3f3f1` | `#0d0d0e` |
| `--surface` | The page card, panels | `#fbfbfa` | `#121213` |
| `--raised` | Cards on a page, popovers | `#ffffff` | `#18181a` |
| `--well` | Inputs, inactive pills, tracks | `#f0efed` | `#1d1d20` |
| `--line` | 1px borders and separators | `#e3e3e0` | `#26262a` |
| `--hover` | Hover fill | `rgb(0 0 0 / .05)` | `rgb(255 255 255 / .06)` |
| `--selected` | Selected row or tile | `rgb(0 0 0 / .08)` | `rgb(255 255 255 / .1)` |

In **Frosted** mode, `--surface` and `--raised` become `color-mix()` blends of
themselves with transparent, and the panes get `backdrop-filter: blur(24px)
saturate(1.4)`. **See-through** works the same way, but blends more, so the
desktop shows through. **Solid** uses the values in the table as they are.

### Text

| Token | Use | Light | Dark |
|---|---|---|---|
| `--fg` | Body and titles | `#1c1a1f` | `#f4f3f6` |
| `--fg-2` | Secondary text, labels | `#55505b` | `#c9c4cf` |
| `--fg-3` | Hints, timestamps, placeholders | `#86808d` | `#8d8794` |

### Colour

| Token | Use |
|---|---|
| `--accent` `#cf5a24` / dark `#f08a5d` | The one brand accent: primary buttons, focus ring, links |
| `--ok` `#2f9e6e` | On, success, toggles that are on |
| `--warn` `#c98a12` | Needs you, waiting |
| `--bad` `#d64545` | Failed, destructive |
| `--info` `#3a72e8` | Neutral notices, links inside data |

**Agent colours** are the mascot palette, which you pick per agent:
`#ff7f95` (the default pink), `#f97316`, `#e11d48`, `#8b5cf6`, `#2563eb`,
`#0891b2`, `#16a34a`, `#ca8a04` and `#64748b`. An agent's colour is used only for
its brain, its folder card and its accent in its own panel. It is never used for
the app chrome.

**Data colours** are for charts, the context map and the memory folders. The six
hues below are fixed, and each hue always means the same kind of thing, in every
app:

| Kind | Hue |
|---|---|
| Instructions, system | `#7c8594` |
| Memory | `#7a5cf0` |
| Chat, conversation | `#d4568c` |
| Files | `#3a72e8` |
| Tool results | `#e08a2c` |
| Free or empty | Hatched `--fg-3` at 9% with a dashed 30% border |

A tile is its hue at 13% fill, with a 30% inset border and text in the hue mixed
82% with `--fg`. This is the context-map look. Reuse it for any categorical
block.

## Type

The fonts are the system UI face (SF Pro, Segoe UI Variable), with Inter as the
fallback. The mono face is the system mono, with JetBrains Mono as the fallback.

| Step | Size / line | Weight | Use |
|---|---|---|---|
| `title` | 22 / 28 | 600 | Page titles (Home, an agent's name on its page) |
| `heading` | 15 / 20 | 600 | Section titles, panel headers |
| `body` | 13.5 / 20 | 400 | Messages, paragraphs, inputs |
| `ui` | 12.5 / 18 | 500 | Buttons, tabs, list rows, labels |
| `meta` | 11.5 / 16 | 400 | Secondary lines, timestamps, captions |
| `micro` | 10.5 / 14 | 500 | Badges and tile captions only |

That is six steps instead of ten. Map the old sizes as follows:

- 11px becomes meta.
- 12px becomes ui.
- 13px and 14px become body.
- 19px and 24px become title.

Numbers use `tabular-nums` wherever they change or line up.

## Space and shape

- **Spacing:** a 4px grid. Use 4, 8, 12, 16, 24 and 32. Pages pad 24px, panels
  12px, rows 8px vertically.
- **Radii:** use four, plus round.
  - `--r-sm` 6px: tiles, chips inside cards, small buttons.
  - `--r-md` 10px: inputs, list rows, menus, popovers.
  - `--r-lg` 14px: cards, panels, the page card.
  - `--r-xl` 20px: the composer, sheets, modals.
  - `999px`: pills, toggles, avatars.
- **Borders:** 1px `--line`, drawn as an inset shadow so they don't shift the
  layout.
- **Shadows:** two only. `--shadow-pop` for menus and popovers, `--shadow-sheet`
  for modals. Cards on a page have no shadow.
- **The frame:** the page card sits 8px in from the window, with `--r-lg`
  corners. On Windows and Linux, the card starts 40px down wherever it would
  reach the window buttons.

## Components

Build each of these once, in React (`web/src/components/ui/`). The vanilla frame
uses the same CSS classes until it moves to React.

| Component | Spec |
|---|---|
| **Toggle** | 32×20 track, 16px white knob. Off: `--fg-3` at 35%. On: `--ok`. `role="switch"`. |
| **Pill / chip** | 24 to 28px tall, `ui` type. Inactive: `--well`. Active: `--fg` text on `--raised` with a `--line` border. |
| **Tabs** | Text tabs 28px tall. The active tab gets `--well` fill. No underline. |
| **Button** | 32px tall (28px when compact). Primary: `--accent`. Secondary: `--well`. Ghost: hover fill only. |
| **Input / textarea** | `--well` fill, `--r-md`, inset `--line` border that turns `--accent` on focus. |
| **Card** | `--raised`, `--r-lg`, 16px padding, `heading` title, `meta` sub-line. |
| **Folder card** | A back panel with a tab, two tilted sheets of paper, a frosted front in the folder's colour, source faces and an "Updated" pill (Home, `ui/dash.css`). |
| **List row** | 32 to 40px, icon 16px, `ui` title, optional `meta` line, hover `--hover`. |
| **Tooltip** | `--raised`, `--r-md`, `--shadow-pop`, `ui` and `meta` lines, a 10px swatch when it labels data. |
| **Composer** | `--r-xl`. Left: attach. Right: model picker, then the mic, then Send, in that order. |

## The mascot

Every agent is a brain. The drawing and its motion come from the owner's
**Brain Mascot**, ported in `web/src/app/orb.tsx`. Do not redraw it per screen.

- **Shape:**
  - Ten lobes (circles) around a wide ellipse.
  - Seven fold strokes, 34 units wide with round caps, in the body colour minus
    7 points of lightness.
  - Two white pill eyes (98×194, fully rounded).
  - A small white smile.
- **Colour:** the agent's colour (pink `#ff7f95` by default). Eyes and smile are
  always white.
- **Motion:**
  - It looks toward the pointer with tanh easing over 420px. The head leans and
    rotates up to 5°, the folds trail, the face slides further than the head, and
    the eyes squash and tilt as they turn.
  - It blinks every 2.2 to 5.8s, sometimes twice.
  - Every face on screen shares one animation clock. With reduced motion it
    holds a still pose.
- **States:**

  | State | What it looks like |
  |---|---|
  | idle | Slow wander |
  | working | Quicker breath, a bob, eyes 82% tall |
  | needs you | Looks up at a blue badge |
  | hover 1.2s | Eyes close into smiles |
  | voice | The eyes become an equalizer (dot, bar, dot) and the head sways and bobs with the voice level |

- **Sizes:** 18 to 22px in lists, 32px in headers, 104px at the top of the agent
  panel, 200px in voice mode.
- **The crew** (Clip Studio): the cloud mascots (Director, Voice, Visuals,
  Scripts) are a separate family for that app's roles. They should share the
  brain's rules: white pill eyes, one body colour, the same motion clock.

## Motion

- Durations: 120ms for hover, 180ms for press or toggle, 280ms for a panel or
  page, all on `cubic-bezier(.2,.8,.2,1)`.
- Nothing animates for longer than 400ms, except the mascots and the painting.
- Honour `prefers-reduced-motion` everywhere. The Settings → Appearance → Motion
  switch overrides it.

## Writing in the UI

- Use sentence case, and say what something does: "Talk", not "Voice mode
  activation".
- Give the number before the word: "7.8k of 200k tokens".
- An empty state says what will appear there and how to get it. Don't use
  marketing copy.
- If a value is an estimate, the label says so.
