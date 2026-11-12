# OrchestrationGraph

A multi-agent run as a tree on a dotted canvas, with its trace underneath. For orchestrator threads that fan out to specialists and sub-agents.

- Canvas: `panel` with a 16px dot grid in `line`. Nodes are 190px wide `card` surfaces with a `line-strong` ring: agent name, status, one clipped task line, and mono model · cost · time. A caption above each column names the lane (RESEARCH, ANALYSIS, WRITING, REVIEW) in 10px `fg-3`.
- Connectors: right-angle 1.25px lines in `line-strong`; lines on the path that is currently running are `accent`.
- A node that needs Charles gets a 1.5px `accent` ring; failed nodes show the `bad` status word; queued nodes are dashed-dot, no cost.
- Header: run title, run id `Tag`, live status, Pause run (secondary) and Stop (`danger`).
- Trace: rows "AGENT · CALL" with a time axis, bars in `fg-3`, running ones in `accent`, durations right-aligned in mono; sub-agents indent two spaces. Selecting a node selects its trace row and the reverse.
- Props for generation: `nodes[{id,parent,lane,name,task,status,model,cost,elapsed}]`, `run:{id,state}`. Layout is computed, never given.
- The affectUI-style orchestration board is drawn from public screenshots of the shape only.
