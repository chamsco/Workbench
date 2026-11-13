# Generative UI contract

How an agent shows interface in a thread or in the drawer, and how the host keeps that safe and on-brand. Machine-readable parts: `genui/catalog.json` (every component, when to use it, props), `genui/genui.schema.json` (JSON Schema for one block) and `genui/validate.py` (the host's checks, runnable). Worked messages are in `genui/examples/`, including five that must fail.

## Principles

1. **Blocks, not screens.** Agents emit blocks (an answer, an approval, a plan chart). The host owns the window, sidebar, tabs, composer and drawer. `AgentBoard` and `AgentChat` are host-owned and not in the catalog.
2. **Catalog only.** A block is a tree of catalog components. Unknown component or unknown prop is an error, not a guess. There is no escape hatch for HTML, CSS, SVG or script.
3. **Tokens, never values.** Props that carry colour, size, spacing or icons take token names or enums (`gap: 12`, `color: "agent-3"`, `icon: "check"`). Hex codes, pixel sizes and font names are invalid.
4. **The agent names actions, the host runs them.** Every button, toggle or card carries an `action` id from the host's registry for that block. The agent cannot run code, and an `ApprovalCard` can never approve itself: its approve and deny ids must differ and are executed only on a person's click or key press.
5. **Prose first.** If a sentence answers the question, send a sentence. Use a block when it carries structure a sentence cannot: sources, a decision, a plan, a comparison, state over time.
6. **One primary action per block.** An `ApprovalCard` counts as the primary. Two primaries in one block is invalid.

## Message envelope

```json
{ "v": 1, "op": "create", "at": "thread", "block": { "type": "...", "id": "...", "props": {}, "children": [] } }
{ "v": 1, "op": "patch",  "id": "ans1", "set": { "props.answer": "...", "props.streaming": true } }
{ "v": 1, "op": "remove", "id": "ans1" }
```

- `at` is `thread` (inline in the conversation) or `drawer` (right panel).
- Nodes have `type`, optional `id`, `props`, `children`. Ids are unique within a message and stable across patches; give an id to anything you will update.
- `patch` sets props by dotted path (`props.answer`, `props.steps.2.state`). Use it to stream: send the block with `streaming: true`, patch the text as it grows, then patch `streaming` to false. The host buffers partial markup until a word or a list line is complete.
- `remove` deletes a block the agent created. It cannot remove a block it did not create.

## Choosing a component

| The agent wants to... | Use |
| --- | --- |
| ask permission to do something | `ApprovalCard` (always for push, delete, spend, message people, write outside the worktree) |
| ask the person to choose | `AgentQuestion` |
| show finished work and next steps | `ResultCard` (files with +/-), optionally inside `Shell` > `Panel` |
| answer a research question with evidence | `AnswerCard` |
| show what it is doing now | `ThinkingTrace` while running; `StepLedger` after |
| show one tool call | `ToolCall` (group several in a `Stack`) |
| show plan against reality | `PlanVsActual` |
| show where cost or time went | `RunCost` |
| show how full context is | `ContextMeter` (narrow) or `ContextMap` (room) |
| show many agents and what needs the person | `AgentInbox` |
| show a fan-out of sub-agents | `OrchestrationGraph` |
| show which tools fail or are slow | `ToolCallGraph` |
| show several agents editing one document | `AgentCursors` |
| anything else | a sentence |

## Content rules

- Text is plain, sentence case, specific: real numbers with units ("3m 12s", "$0.84", "4 files"), no exclamation marks, no emoji.
- `title` is what happened or what is asked, in one line. `why` says the consequence ("Pushing starts the Windows build"), not the mechanism.
- Cap lists as the catalog says and say when a list is cut ("and 9 more").
- Never put secrets, tokens or full file contents in a block. `ToolCall.input` and `output` are truncated by the host at 2,000 and 4,000 characters and run through the secret filter.
- A block must read correctly without colour, motion or hover.

## Host rules (what `validate.py` enforces, and what the renderer must also do)

1. Validate the envelope and every node against `genui.schema.json`; reject unknown types and props.
2. At most 6 levels deep and 200 nodes per block; ids unique.
3. `Panel` only as a direct child of `Shell`; no `Shell` inside `Panel`.
4. At most one primary action per block; `ApprovalCard` approve and deny ids must differ.
5. Every action id must exist in the host's registry for this thread; otherwise render the control disabled with the reason, and log it.
6. On a failed validation show a one-line `Well` ("Block rejected: /props: color is not allowed") and keep the thread usable. Never crash, never render a partial block.
7. A block that the agent has been told to stop (thread stopped) freezes in place: controls disable, nothing animates.
8. Streaming blocks keep `aria-busy` until `streaming` is false. Announce new approvals and questions politely, once.
9. Render with the same components as the host's own screens. There is no second renderer.

## For the GPUI build

- Generate Rust types from `genui.schema.json` (one enum variant per component, `serde(tag = "type")`) and fail the build when `catalog.json` and the code disagree.
- A component's `props` define its builder's inputs; the builder looks up tokens by name from `crates/chrome`.
- Run the same checks as `validate.py` in Rust before rendering; keep the examples (valid and invalid) as test fixtures.
- Actions are an `ActionRegistry` per thread: `register(id, handler, needs_person: bool)`. Approvals and `AgentQuestion` answers are `needs_person: true`.

## Adding a component

1. Draw it in the system first (preview, README with sizes and states, light and dark).
2. Add it to the catalog with `useWhen` and bounded props (max lengths, max items, enums).
3. Add one valid and one invalid example. If it can change state outside the thread (spend, delete, send), it needs an `ApprovalCard` path.
