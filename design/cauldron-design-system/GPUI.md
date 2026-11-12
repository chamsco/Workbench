# GPUI mapping

How these tokens land in `crates/chrome` (the GPUI token crate). Values here win over `tokens.rs` today; regenerate `tests/golden/wireframe.json` after the change and read the diff as a design change.

## Palette fields

| `tokens.rs` field | Token | Light | Dark | Change from today |
| --- | --- | --- | --- | --- |
| `bg` | `bg` | #ecebee | #08080a | was #e9e8e5 / #09090a |
| `frame` | `frame` | #f3f2f4 | #0e0e10 | was #e4e3df / #0c0c0d |
| `card` | `card` | #ffffff | #161618 | light was #f7f7f5 |
| `raised` | `raised` | #ffffff | #1d1d20 | dark was #1f1f22 |
| `well` | `inset` | #f5f4f6 | #121214 | renamed; dark now sits below the card |
| `line` | `line` | #e3e1e6 | #26262a | was #dcdbd7 / #2e2e32 |
| `strip` | `line-strong` | #d6d3db | #323237 | the Hoplite band becomes a ring |
| (new) | `line-control` | #8a8592 | #77717f | add |
| (new) | `hover`, `selected` | #ebeaed, #e6e4e9 | #232327, #2a2a2f | add |
| `fg`, `fg2`, `fg3` | `fg`, `fg-2`, `fg-3` | #18171b, #3f3c45, #68636f | #f2f1f4, #cfcad4, #8e8896 | retuned for 4.5:1 on hover |
| (new) | `fg-4` | #a7a2ae | #5f5a66 | disabled only |
| `accent` | `accent` | #c9330f | #ff4d2d | unchanged |
| `on_accent` | `on-accent` | #ffffff | #1a0400 | unchanged |
| `ok`, `warn`, `bad` | same | #23784f, #8a5a00, #b83232 | #3fb27f, #e0a32a, #ef6a6a | light darkened to pass 4.5:1 |
| `run` | none | | | drop: running is a spinner in `fg-2` |
| `info`, `design`, `mem` | none | | | drop: Code/Design/Memory are told apart by icon and word |

`AGENT_DOTS` stays as `agent-1` to `agent-9`. `shadow::MENU`, `TIP`, `BOX` keep their geometry; MENU and TIP gain the 1px ring shown in `shadow-menu` and `shadow-tip` (draw it as a 1px border in GPUI). `radius::*` and `motion::time::*` are unchanged; `dur-exit` (140ms) is new.

## Text styles

GPUI has no CSS classes: add a `type` module with one `TextStyle` constant per style (`CAPTION`, `META`, `LABEL`, `BODY`, `BODY_STRONG`, `READING`, `TITLE`, `HEADING`, `DISPLAY`, `MONO`, `MONO_TAG`) holding size, line height and weight. Font family: "Segoe UI Variable Text" on Windows, ".SystemUIFont" on macOS; mono "Cascadia Mono" / "SF Mono". Enable tabular figures (`tnum`) on META and on any element that shows a timer or a count.

## Components

Each component page lists exact sizes. Build order that unblocks the most screens: Button, IconButton, StatusLabel, Tag, ThreadRow, Composer, StepLedger, ToolCall, then ApprovalCard, ResultCard, Menu, AgentTabs, SegmentedSwitch, Toggle, ContextMeter, RunCost.

## Licence notice

ToolCall's layout follows the affectUI Tool Call Card, which is MIT licensed. No code was copied; if any is ported, keep its MIT notice in `THIRD_PARTY_NOTICES.md`. Tuff UI (MIT) informed the tone-step surfaces. affectUI Pro and Profound's Palermo were studied only from their public pages; nothing from them is reproduced.
