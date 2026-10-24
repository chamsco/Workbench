# Workbench, project Brainwave: the plan

Brainwave is the name for what Workbench is turning into. Workbench used to be a
coding harness with a chat beside it. It is becoming a desktop home for your
agents. One window holds them: a brain-faced agent per job, one memory that
every app reads and writes, and a small set of apps (Code, Chat, and next Design
and Clip Studio) that each use agents differently.

The task list lives in [TODO.md](TODO.md). The structure is in
[ARCHITECTURE.md](ARCHITECTURE.md), the visual system in [DESIGN.md](DESIGN.md),
and the references in [RESEARCH.md](RESEARCH.md).

> **Naming.** The product is *Workbench*. *Brainwave* is this project, the
> redesign. *backspace* is still the name in the code (crates, config folder),
> and *Brainpowr* is the local model engine and the coding TUI. Four names is
> too many for one user to meet. The proposal: the user only ever sees
> **Workbench**, and the other three stay internal.

## What it is

You install one desktop app (Tauri: macOS first, then Windows and Linux). It
finds the AI you already pay for or run: Claude Code, Codex and the other coding
CLIs, Ollama, LM Studio, any OpenAI-compatible router, or an A2A agent. Workbench
never holds those credentials. On top of them it gives you:

- **Agents with a face.** An agent has a name, a pink brain (in its own colour),
  instructions, a personality, a model, a folder or computer, rules, memory and
  routines. You talk to one directly, put several in a group, or let them run on
  a clock.
- **One memory.** Notes that outlive a chat. They are global, per project, or per
  agent, stored as a git repo of Markdown you can edit by hand.
- **Apps.** Each app is a view on top of the same agents, memory and projects.
- **Traces.** Every reply is recorded: each tool call, its tokens and cost, and
  what filled the context window.

**Why not just use the CLI or a chat app?** Because those give you one
conversation. Workbench gives you the parts around the model: agents that keep a
job, memory that crosses apps, traces, carry-on after a crash, and several agents
at once without them stepping on each other. That is the pitch. Anything that
doesn't serve it is a candidate to cut (see *Scope* below).

## The window

```
┌──────────────┬──────────────────────────────────────┬────────────┐
│ ← → ▢  ⌕ ⚙   │  Page header: icon · name · last ask │            │
│ ⌂ 💬 </> +    │──────────────────────────────────────│   Ask      │
│ (app tiles)  │                                      │ (Home only)│
│              │            The page card             │            │
│ Projects     │    (Home, Chat, Code, an app …)      │            │
│  └ threads   │                                      │            │
│              │                                      │            │
│ This machine │                                      │ composer   │
└──────────────┴──────────────────────────────────────┴────────────┘
```

- **Sidebar:** history buttons, search and settings. Then the app tiles, the
  project tree with its threads, and the machine switcher at the bottom.
- **Page card:** whichever app is open, 8px in from the window, with rounded
  corners. It takes the full width everywhere except Home.
- **Ask column:** a docked chat, on Home only.
- **Window material:** Frosted (the default), See-through, or Solid.

## Home, with memory folded in

Home is the landing page and the one place you see everything at once:

1. **Memory folders.** One folder card each for Everywhere, every project and
   every agent, with who wrote into it and when. Under them, *Recent activity*
   lists what was remembered, by whom, and in which folder. Opening a folder
   shows its notes as Home › Folder. There is no separate Memory app any more.
2. **Activity:** the last 30 days, plan usage, recent threads and projects.
3. **Ask:** a chat column that can reach every app.

Memory is **on by default** and is **scoped by default**. A note made inside a
project goes to that project's folder, and a note an agent learns goes to that
agent's folder. Only what you mark global goes to Everywhere.

### How each app writes to memory

Each app writes notes of its own kind, tagged with its source. That way Home can
show who remembered what, and a tidy can treat them differently.

| App | What it remembers | Source tag | Status |
|---|---|---|---|
| Chat | Your preferences ("from now on…"), facts you state, Remember on a message | `chat`, `you` | Built |
| Agents | What an agent learned doing its job; routine outcomes | `agent` | Partly built (the tag exists, nothing writes routine outcomes yet) |
| Code | Repo conventions, commands that work, decisions made in review | `code`, `file` | Tag only |
| Design | Brand tokens, chosen variants, rejected directions | `design` | Planned |
| Clip Studio | Voice, pacing, intro and outro, music you keep picking | `clip` | Planned |
| Phone | Notes said or typed on the phone | `phone` | API only |
| Tidy | Merges and rewrites from the daily tidy | `dream` | Built, off by default |

The way in already exists, and every app should use it:

1. An app calls `memory::propose` with a `NoteCandidate` that carries its text,
   its scope (everywhere, a project, or `agent:<id>`) and its source.
2. The candidate lands in `<data>/memory-inbox/`.
3. The Librarian drains the inbox. It keeps the candidate, merges it into an
   existing note, or drops it, and it writes a receipt saying which.

Agents already write this way through MCP. So adding an app to memory is a
matter of choosing its source tag and deciding when it proposes, not building a
new store.

The open question is how much an app may write without asking. The proposal:
anything an app infers goes in as **pending** and shows on Home for one tap;
anything you said outright is saved. Today Chat saves inferred notes above a
score threshold without asking. That is fine for one app, but it gets noisy
with five.

## The apps

An app is an extension in `crates/tauri-app/ui/extensions/<id>/` with a manifest.
The frame knows an app only through that manifest. Third-party apps are sandboxed
web pages with a `backspace-app.json` (see `docs/apps.md`). The first-party apps:

### Code

Say what you want built. A planner asks numbered questions and writes tickets.
One agent per ticket works in parallel, each in its own git worktree. A check
command has to pass, and a diff reaches you to approve.

- **Built:** tickets, worktrees, checks, review that follows the agent org chart,
  canvases (sessions, terminal, browser, diagram, `PLAN.md`, options), harness
  tiles that message each other, Brainpowr Code as the first terminal.
- **Next:** workers on the shared runner, so they get traces and Carry on.

### Chat

Threads with any model or CLI, plus the **Agents** side:

- **Built:** named agents, groups, ghost chats, routines, computer use,
  artifacts, delegation.
- **New in Brainwave:**
  - **The agent panel.** On the right of an agent's thread. The agent's name and
    brain sit on top, and four tabs sit under them:
    - **Settings:** model, instructions, personality and switches.
    - **Context:** a map of what fills the context window.
    - **Traces.**
    - **Routines.**
  - **Voice mode.** It starts from the mic next to Send. While it listens or
    talks, the brain's eyes turn into an equalizer.
  - **An organigram** of who works on what, drawn by Pele.

### Design (planned)

A canvas where agents propose visual directions, and you pick, combine and
refine them. It builds on what exists:

- `show_options` already puts two to six variants on screen and returns your
  pick.
- Brand decisions go to memory under the `design` tag, so Code and Clip Studio
  reuse the same colours and type.

The smallest first version: a board of options for one brief, with a pick, a
"more like this" and an export to HTML and PNG. A free-form drawing canvas comes
later, and only if picking turns out not to be enough. See RESEARCH.md for the
licence trade-off between tldraw and Excalidraw.

### Clip Studio (planned)

Short videos made by agents: a script, a voice, visuals and the cut. The four
cloud mascots (Director, Voice, Visuals, Scripts) are the crew, one agent per
role, working as a group thread with a timeline as its artifact.

The smallest first version is a script-to-video pipeline: script → voice-over
(the system voice, or a local TTS) → slides or stock clips → render with ffmpeg.
Do this before building any editor. An editor is a large project; OpenCut (MIT)
shows how large. Remotion is attractive but has a paid licence for teams of four
or more and for automated rendering (see RESEARCH.md).

## Agents, the shared layer

All apps share one agent model (`crates/core/src/agents.rs`, the `Agent` type in
`web/src/bridge.ts`):

| Field | What it is |
|---|---|
| name, avatar.color | The brain's name and colour. The emoji only phases the blink. |
| job | Its instructions. The first line is its job, and the rest is its backstory. |
| personality | How it talks, added to its system prompt |
| route | Its model: a CLI, a local model, a router, cloud or A2A |
| computer | A folder, a Docker container or an SSH box |
| memory, off, apps_off | Memory on or off, rules turned off, apps it may not use |
| routines | Jobs on a clock: daily, weekdays, weekly, every N minutes |
| shared | Folders shared with it |

## Scope: what to stop doing

The app already covers coding, chat, agents, memory, apps, cloud, a phone link,
Colony VMs, a GPUI shell and a local model engine. Adding Design and Clip Studio
on top of all that is the biggest risk to this plan. The proposal:

- **Freeze the GPUI shell** (`crates/app`). It duplicates the Tauri shell and only
  does coding.
- **Park Cloud and its ads** until there are users who ask for them.
- **Park Colony** until a user needs a disposable VM rather than Docker.
- **Ship Design and Clip Studio as thin first versions** built on existing pieces
  (options, groups, artifacts, routines). Not as new engines.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| 1 | The frame | PR #29 is merged; the Mac build passes; it has been seen on a real Mac and a real Windows box |
| 2 | Agents you can live with | The agent panel, voice and mascot are tested on hardware; the phone link is secured |
| 3 | Memory across apps | Source tags and pending notes from every app; Home's review queue |
| 4 | Design v0 | A board of options for one brief, picks saved to memory |
| 5 | Clip Studio v0 | Script → voice → slides → MP4 by a four-agent group |
| 6 | Public beta | Signed builds on all three OSes, a crash reporter, docs |
