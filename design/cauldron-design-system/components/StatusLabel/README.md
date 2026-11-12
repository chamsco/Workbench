# StatusLabel

The state of a run, thread or tool call, written as a glyph and a word; colour is added only when the state asks for Charles.

| State | Glyph | Text colour |
| --- | --- | --- |
| Running | spinner in `fg-2` plus a live timer in `fg-3` | `fg-2` |
| Done | thin check | `fg-2` (not green) |
| Needs you | `accent` dot | `fg` |
| Failed | x | `bad` |
| Stopped | square | `fg-2` |
| Idle | `fg-4` dot | `fg-2` |
| Expiring | clock | `warn` |

- Text is `meta` (12/16). Timers use tabular numerals and only the digits change.
- Never a coloured pill background for state: in a list of 30 threads, colour must point at the two that need someone.
