# GPUI mapping

How these tokens land in `crates/chrome` (the GPUI token crate). The palette follows Tuff UI's neutral ramp and radii (22/16/12); the accent is purple; motion is Cauldron's. Regenerate `tests/golden/wireframe.json` after the change and read the diff as a design change.

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
| `accent` | `accent` | #6236d6 | #a08cff | purple; was ember. `on_accent` #ffffff / #12082e; hover #5229bd / #b4a4ff |
| `ok`, `warn`, `bad` | same | #23784f, #8a5a00, #b83232 | #30d158, #ff9f0a, #ff453a | dark values are Tuff's positive, warning, negative |
| `run`, `info`, `design`, `mem` | none | | | drop |

`radius::*` becomes SHELL 22, PANEL 16, INSET 12, CONTROL 10, INNER 8, TAG 6 (was R 6, R1 4, R2 3, WINDOW 10). Nested rule: outer = inner + 6px shell padding. This replaces the approved 6/4/3 family on Charles's request (2026-11-12).

`AGENT_DOTS` stays as `agent-1` to `agent-9`. Alpha overlays need compositing in `Rgb`; add an `Rgba` type and a `over(surface)` helper, and add contrast tests per surface. `shadow::MENU` and `TIP` gain the 1px ring from `shadow-menu` and `shadow-tip`.

## Fonts

The font files are in `fonts/` here (Geist Regular, Medium, SemiBold; Geist Mono Regular, Medium; static TTFs from the `geist` npm package 1.3.1, SIL OFL, licence in `fonts/LICENSE.txt`). Copy them into the app's assets and register them at startup; GPUI does not read fonts that are not installed. Family names: "Geist" and "Geist Mono". The `cv11` and `ss01` sets must be switched on in each text run. Check that GPUI's font features API accepts them before committing to this; if it does not, ship static instances with the alternates baked in.

## Text styles

Add a `type` module with one `TextStyle` constant per style (`CAPTION`, `META`, `LABEL`, `BODY`, `BODY_STRONG`, `READING`, `TITLE`, `HEADING`, `DISPLAY`, `MONO`, `MONO_TAG`) holding size, line height and weight. Tabular figures on META and any timer or count.

## Components

Each component page lists exact sizes. Screens: AgentBoard, AgentChat and ToolCallGraph are full-window specs (1280x760) in the Screens group. Build order: Surface, Button, IconButton, StatusLabel, Tag, ThreadRow, Composer, StepLedger, ToolCall, then ApprovalCard, ResultCard, Menu, AgentTabs, SegmentedSwitch, Toggle, ContextMeter, RunCost.

## Licence notice

Tuff UI is MIT ("Copyright (c) 2026 TUFFUI contributors"): colour values, radii, the Surface layering, the Coding Agent and Agent Console screen layouts and the motion curves are taken from it, so keep that notice in `THIRD_PARTY_NOTICES.md`. Geist and Geist Mono are SIL OFL 1.1: ship the licence text beside the font files. affectUI's free pieces are MIT. Components not taken from a free source are drawn for Cauldron.

## Generative UI

See `GENUI.md`. The renderer is a function from a validated `genui` block to the same GPUI components as the host screens. Build order after the first components exist: the envelope and validator (port `genui/validate.py` and its examples), then `ApprovalCard` and `AgentQuestion` wired to an `ActionRegistry`, then `ResultCard`, `AnswerCard` and `ThinkingTrace`.
