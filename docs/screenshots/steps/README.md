# Step lines under a reply

Captured on Charles's Windows 11 PC, 12 Nov 2026, 1240x702, dark theme, debug build, seeded sample data folder with a made-up trace (12 reads, 4 commands, 3 edits). No model was contacted.

| File | State |
|---|---|
| `01-steps-sample.png` | A finished reply with its steps above the text: Thought, Read 12 files, Ran 4 commands, Made 3 edits |

- Steps come from the reply's trace in core (`Msg.trace`, one span per tool call): one line per kind of tool, in the order each kind first happened; files are counted once; failures say "N failed"; unknown tools are named ("Used mcp__linear__get_issue"). Tested in `steps_group_a_replys_tool_calls` and on screen in `a_row_opens_its_thread`.
- "Thought" has no duration: core does not time the thinking.
- Read once per thread load, never while drawing.

## Not done

- Live steps while a reply runs: core keeps the open trace inside the run and saves it at the end, so a running reply still says "Writing". Showing tools as they happen needs core to publish the open trace.
- The ask card with answer chips: core has no question event and the harness runs without input; it needs a reply path into the run (asked).
