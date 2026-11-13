# ToolCallGraph

Which tools an agent calls, how often, how slow and how often they fail, drawn as one agent node fanning out to its tools.

- One agent node on the left; one 54px tool node per tool on the right, in `inset` with `radius-inset`. Tool name in mono 500; under it calls, p50 and p95; at the right edge the error rate and the word "errors".
- Edge width is the call count (1.5 to 8.5px). Edges are `fg-4`; an edge with an error rate above 10% is `bad`. The selected tool's node takes the `hover` fill. The legend line says so.
- Switch metric (Calls, Latency, Errors) re-weights the edges; the layout never moves.
- Right panel: the selected edge's numbers as key/value rows in mono, one sentence of cause, and a primary action that opens the failed runs.
- Never colour by tool type; colour is for failure and selection only.
- Inspired by the public affectUI Tool Call Graph description; geometry, data and styling are Cauldron's.
