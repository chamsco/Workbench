# GPUI mapping

How these tokens land in `crates/chrome` (the GPUI token crate). The palette now follows Tuff UI's neutral ramp; the accent, radii and motion are Cauldron's. Regenerate `tests/golden/wireframe.json` after the change and read the diff as a design change.

## Palette fields

| `tokens.rs` field | Token | Light | Dark | Change from today |
| --- | --- | --- | --- | --- |
| `bg`, `frame` | `bg`, `frame` | #f7f7f8 | #111111 | the same value now; was two values |
| `card` | `card` (Shell) | #eeeef0 | #1d1d1d | light is now a grey tray; dark is lighter than the window |
| (new) | `panel` (Panel) | #ffffff | #151515 | content surface inside the shell |
| `raised` | `raised` | #e4e4e7 | #282828 | menus, tooltips |
| `well` | `inset` | fg at 4% | fg at 5% | now an alpha overlay, not a solid |
| (new) | `hover`, `selected` | fg at 5%, 9% | fg at 5%, 9% | alpha overlays |
| `line` | `line` | fg at 7% | white at 6% | Tuff border |
| `strip` | `line-strong` | fg at 14% | white at 12% | the Hoplite band becomes a ring |
| (new) | `line-control` | #7e7e86 | #7c7c82 | 3:1 control edges |
| `fg`, `fg2`, `fg3` | `fg`, `fg-2`, `fg-3` | #1c1c1f, #45454c, #62626a | #f5f5f5, #c9c9cd, #9e9ea3 | Tuff foreground and muted-foreground; light fg-3 darkened to pass 4.5:1 on hover and raised |
| (new) | `fg-4` | #a6a6ad | #5e5e64 | disabled only |
| `accent` | `accent` | #bf2f0c | #ff4d2d | light darkened a step to hold 4.5:1 on the grey shell |
| `ok`, `warn`, `bad` | same | #23784f, #8a5a00, #b83232 | #30d158, #ff9f0a, #ff453a | dark values are Tuff's positive, warning, negative |
| `run`, `info`, `design`, `mem` | none | | | drop |

`AGENT_DOTS` stays as `agent-1` to `agent-9`. Alpha overlays need compositing in `Rgb`; add an `Rgba` type and a `over(surface)` helper, and add contrast tests per surface. `shadow::MENU` and `TIP` gain the 1px ring from `shadow-menu` and `shadow-tip`.

## Fonts

Bundle Geist (variable, 100 to 900) and Geist Mono with the app and register them at startup; GPUI does not read system fonts that are not installed. Family names: "Geist" and "Geist Mono". The `cv11` and `ss01` sets must be switched on in each text run. Check that GPUI's font features API accepts them before committing to this; if it does not, ship static instances with the alternates baked in.

## Text styles

Add a `type` module with one `TextStyle` constant per style (`CAPTION`, `META`, `LABEL`, `BODY`, `BODY_STRONG`, `READING`, `TITLE`, `HEADING`, `DISPLAY`, `MONO`, `MONO_TAG`) holding size, line height and weight. Tabular figures on META and any timer or count.

## Components

Each component page lists exact sizes. Build order: Surface, Button, IconButton, StatusLabel, Tag, ThreadRow, Composer, StepLedger, ToolCall, then ApprovalCard, ResultCard, Menu, AgentTabs, SegmentedSwitch, Toggle, ContextMeter, RunCost.

## Licence notice

Tuff UI is MIT ("Copyright (c) 2026 TUFFUI contributors"): colour values, the Surface layering and the motion curves are taken from it, so keep that notice in `THIRD_PARTY_NOTICES.md`. Geist and Geist Mono are SIL OFL 1.1: ship the licence text beside the font files. affectUI's free pieces are MIT. Components not taken from a free source are drawn for Cauldron.
