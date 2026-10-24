# Research

These are the references behind the Brainwave plan, and what each one means for
us. Each entry says how it was checked:

- **[checked]** was read on 2026-10-24.
- **[repo]** comes from this repository's own docs or code.
- **[from memory]** is not verified this session. Treat it as a lead, not a fact.

## Where the ideas came from

| Reference | What we took | Source |
|---|---|---|
| Whirl (whirlchat/whirl, MIT) | The React chat face: threads, composer, tapbacks | [repo] README credits |
| Atomic Chat | Frame layout; the local model gateway on a fixed port (`:1337` there, `:7422` here) | [repo] PR #27 notes |
| orbs (nikuscs/orbs, MIT) | The shared animation clock and the hover cheer for faces | [repo] `orb.tsx` attribution |
| The owner's Brain Mascot | The brain's shape, folds, eyes, smile, cursor look and blink | [repo] ported into `orb.tsx` |
| Context map by @flohoeller (via componest) | A treemap of the context window with tinted tiles by kind, Free hatched, and a hover card with kind, tokens and % | Screenshots from the owner, 2026-10-24 |
| Agent Memory Repo | Memory as a git repo of Markdown, with metadata per bullet | [repo] README |
| mattpocock/skills, ponytail (MIT) | Vendored skills | [repo] README |
| Pele (pele.run, MIT, by kepano) | Mermaid to SVG, themed with CSS variables, lazy-loaded | [repo] pinned 1.9.0 |

## Agent memory

Our model is a **file store and a librarian**:

1. Apps propose notes into an inbox.
2. One process files them: it keeps, merges or drops each one, and writes a
   receipt.
3. Ranking by importance, use and age decides what goes into a prompt.

| System | Shape | Licence | Relevance |
|---|---|---|---|
| **Mem0** | A memory layer: `add()` and `search()` inside any agent framework | Apache-2.0 [checked] | The closest to ours in spirit. Our ranking and merge are hand-written; Mem0 does them with embeddings and an LLM. |
| **Letta** (formerly MemGPT) | A full agent runtime with core, recall and archival memory tiers | Apache-2.0 [checked] | Adopting it would mean moving our agents into Letta's runtime. Not worth it, since we already have one. |
| Zep, Cloudflare agents memory | Hosted or graph memory | [from memory] | Hosted options conflict with local first |

**What this means for us:**

- **Keep the git repo.** Its advantages are real: it is human-editable, the
  history comes free, and Undo is a revert.
- **What we lack is retrieval quality.** Notes are ranked by importance, use
  and age, not by relevance to the message.
- **The cheapest upgrade is local embeddings.** Use the model gateway: an
  embedding model through Ollama or Brainpowr. Re-rank the scoped notes by
  similarity to the current message before applying the budget.
- **It doesn't need a vector database** at our size (hundreds to a few thousand
  notes). A flat file of vectors next to the repo is enough.

## Voice

**Today.** The webview's `SpeechRecognition` listens, and the system
`speechSynthesis` voice speaks.

**The catch.**

- WebKitGTK (Linux) has no `SpeechRecognition`, so voice can't hear you there.
- On macOS, WebKit's recognition goes through Apple's servers unless on-device
  dictation is on [from memory].
- Behaviour differs per OS, and none of it has been tested on hardware.

| Option | Notes |
|---|---|
| **Moonshine** (moonshine-ai) | Built for live streaming. Tiny model is 34M parameters (26 MB), Small 123M, Medium 245M. Packages exist for Python, Swift, Android, Windows, and a native C++ library on Linux. Licence not stated on the docs page; check the repo. [checked] |
| **whisper.cpp** | MIT, runs everywhere, Rust bindings exist. Not built for streaming: it transcribes chunks, so there is a lag. [from memory] |
| Local TTS (Piper, Kokoro) | Better voices than the system one, fully offline [from memory] |

**Recommendation.** Keep the webview path as the default, since it costs
nothing. Add a core-side speech-to-text through the model gateway (Moonshine
first, whisper.cpp as a fallback) so Linux and offline work. The orb's
`voiceLevel` already takes any level source, so the face won't change.

## Diagrams

**Pele** renders Mermaid to SVG with no browser dependencies beyond the DOM, and
it themes through `.pele` CSS variables mapped to our tokens. It costs about
9.5 MB of lazy chunks, because all the Mermaid diagram types ship [repo]. We use
it for chat replies and the organigram. If the size matters later, the fix is to
ship only the flowchart and sequence diagram types.

## Design app

| Option | Licence | Fit |
|---|---|---|
| **tldraw SDK** | Free for commercial use, but shows a watermark. Removing the watermark needs a paid Business licence. [checked] | The best canvas SDK. The watermark rules it out for a polished product unless we pay. |
| **Excalidraw** | MIT [from memory] | A hand-drawn look. It is open, but a sketch tool, not a design tool. |
| **Our own options board** (`show_options`) | Ours | Already returns a pick to the agent. **Start here.** |

**Recommendation.** Design v0 is a board of options, not a canvas. Agents
propose two to six directions as live HTML. You pick, mix ("this layout, that
palette") and refine. The picks go to memory under `design`. Only add a canvas
SDK if picking turns out not to be enough.

## Clip Studio

| Option | Licence | Fit |
|---|---|---|
| **Remotion** | Free for individuals and teams of up to 3. Larger teams need a Company licence ($25 per developer per month), and automated rendering is priced per render ($0.01, $100 per month minimum). End users can't upload their own Remotion code. [checked] | Video as React. A great fit technically. The licence is a cost from the day the team reaches 4 or the product renders for users. |
| **OpenCut** | MIT. Next.js web app plus a Rust core compiled to WASM; a GPUI desktop app is in progress. About 45.8k stars. Export is being reworked. [checked, via a third-party write-up] | A full editor to learn from or embed later. Its Rust core and GPUI are close to our stack. |
| **ffmpeg** | LGPL or GPL, depending on the build [from memory] | Enough for v0: concatenate, mux voice-over, burn captions, stills to video. |

**Recommendation.** Clip Studio v0 is a pipeline, not an editor:

1. A four-agent group writes the script (Scripts), plans the shots (Director),
   picks or makes the stills (Visuals) and voices it (Voice).
2. Core assembles it with ffmpeg.

That needs no new licence and reuses groups, routines and traces. Look at
OpenCut's core before writing any timeline code.

## Context window display

The reference context map draws the window as a treemap:

- **Tiles by kind:** system prompt, tool definitions, project memory, chat, tool
  results, files.
- **Free space** as a hatched block.
- **A model chip** with the window size, plus used / total and a percentage.
- **A hover card** with the kind, tokens and share of the window.

**What we have.** We draw the same thing vertically in the agent panel, from
`chat_context`. Two gaps remain:

- **Counts are estimates** (four characters per token). Exact counts need each
  provider's tokenizer, or the input-token count from the last trace (which we
  show).
- **Tool definitions and tool results aren't counted yet.** The runner knows the
  tool list, and the trace knows the results. Adding both would make the map
  match what the model actually reads.

## Sources

- Remotion licence FAQ: <https://www.remotion.dev/docs/license/faq>
- tldraw licence: <https://tldraw.dev/legal/tldraw-license>
- OpenCut overview: <https://themenonlab.blog/blog/opencut-open-source-capcut-alternative-video-editor>
- Moonshine docs: <https://mintlify.wiki/moonshine-ai/moonshine/introduction.md>
- Mem0 vs Letta: <https://vectorize.io/articles/mem0-vs-letta>
- Pele: <https://pele.run/>
