# Hands: browser use and desktop use for agents

Agents get them as `backspace mcp` tools (`crates/core/src/hands/`). Each
agent has two switches in its side panel's **Screen** tab: **Browser use**
(on by default; off whenever its Web rule is off) and **Desktop use** (off
until the user turns it on). The same tab shows what it last saw, its last
steps, and **Take control**, which stops its hands until handed back.

## Browser use

The agents share one Chrome (or Edge, Brave, Chromium) with its own profile
in `<data>/hands/chrome`. It is started with a DevTools port and left
running, so sign-ins made there last. Each agent works in its own tab.

The page is read the way [Jev Ultrafast](https://github.com/browser-use/jev-ultrafast)
(MIT) reads it: one atomic DOM snapshot (`snapshot.js`, vendored unchanged)
turned into a table of controls with ids (`e7 type textbox "Where to?"`).
Actions are only ever taken on an observed node. Just before the input, the
node is checked again for freshness, visibility and occlusion. A model never
writes a selector, a coordinate or script.

| Tool | What it does |
| --- | --- |
| `browser_open {url}` | Navigates its tab and returns the table |
| `browser_look {screenshot?}` | Reads the page again |
| `browser_act {id, text?}` | Clicks, selects, types, scrolls or waits; returns the new table |
| `browser_key {key}` | Enter, Escape, Tab, Backspace, Up or Down |
| `browser_task {goal, url?}` | The fast loop: a decision model runs the whole goal (below) |

Without a fast model, the agent's own model (Claude Code, Codex...) does the
deciding, one `browser_act` per step. That is the most reliable path, but each
step is a full agent turn.

### The fast loop (`browser_task`)

Pick a decider in the Screen tab under **Fast browser model** (`prefs.hands.decider`):

- **Jev**: TypeSafe's hosted Jev (`POST api.typesafe.ai/v1/systemone`) with
  `TYPESAFE_API_KEY`. The request is Jev Ultrafast's: one operation head plus a
  target head per operation. Jev never writes text, so text comes from the
  local model if one is set, otherwise from `inception/mercury-2.5` on
  OpenRouter (`OPENROUTER_API_KEY`).
- **On-device**: a small model on this machine (`ollama:qwen3:4b`; empty uses
  the small local model from Settings → Local models). It goes through the
  runtime's OpenAI API. A JSON-schema enum holds the answer to the ids on the
  page, and one call both picks the action and writes the text.

## Can the decision model run on the device? Yes, but not "ultrafast"

- **Jev itself can't run locally.** Jev Ultrafast is MIT, but it is only the
  loop and the DOM reader. The decisions come from TypeSafe's hosted API.
  Their docs give no weights, size or self-hosting option.
- **The speed comes from TypeSafe's servers.** Their Flights run took 17
  decisions with a median of 178 ms each, at about 5.3k input tokens per
  decision (90,558 in total).
- **On-device, prefill sets the cost of each step.** At llama.cpp-class speeds,
  a 1–4B model on an M-series Mac prefills about 600–2,500 tokens a second.
  Those figures are estimates; nothing was measured here. To cut the prompt,
  the local path sends at most 80 controls and 1,500 characters of text, about
  1.5–2k tokens. That makes a step about 1–3 s on a Mac (faster on a desktop
  GPU), so a 17-step task takes about 20–50 s instead of 7 s. That is still
  quicker than a big agent doing every step itself, and it is free and private.
- **Accuracy is the real unknown.** Jev is trained for exactly this choice.
  A general 1–4B instruct model should manage simple pages (open a link, fill
  two fields, submit), but autocompletes, date pickers and multi-filter forms
  are where it will slip. No real small model was run in this build:
  huggingface.co is blocked from the build container. The loop was tested
  against a stand-in.
- **What would close the gap:** fine-tune a 1–3B model on traces of this exact
  table → id format. Every `browser_task` run on Jev or on the agent's own
  model is a labelled example. Add KV-cache reuse for the fixed prefix
  (rules and goal).

## Desktop use

Tools: `desktop_screenshot`, `desktop_click {x, y, button?, double?}`,
`desktop_type {text}`, `desktop_key {key}` (`cmd+c`, `enter`...),
`desktop_scroll {dy}`. Each one returns a fresh screenshot. Screenshots are
scaled to 1280 wide. The agent clicks in that picture's pixels, and
`screen.json` maps them back to the display. Nothing decides here: the
agent's own vision model looks and says where. Jev has no desktop
counterpart. A local desktop decider would need an OS accessibility-tree
reader (AX on macOS, UI Automation on Windows) to get the same "choose from
a table" shape, which this build does not have.

- macOS: screenshots via `screencapture`; the app needs **Screen Recording**
  and **Accessibility** (System Settings → Privacy & Security).
- Windows: `xcap` for the screenshot, `enigo` for input.
- Linux: `grim` or ImageMagick `import`, and X11 input.

## Files

`<data>/hands/agents/<id>/`: `tab` (its Chrome target), `page.json` (the last
table, so `e7` means the same thing across calls), `frame.jpg`, `steps.jsonl`,
`live.json`, `screen.json`, `paused`.
