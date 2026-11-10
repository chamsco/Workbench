# Architecture

This is the one architecture for Workbench, the Brainwave project. It describes
what is there today and the few rules new work follows. Where today's code breaks
a rule, the break is listed under *Debts* with how to pay it off.

## The layers

```
┌──────────────────────────── Tauri window (one per app instance) ───────────────────────────┐
│  Frame  (vanilla JS: ui/shell.js, frame.js, dash.js, ai.js, memory.js, app.js)              │
│    sidebar · app tiles · project tree · page card · Home · Ask column · Settings           │
│                                                                                             │
│  Apps   (ui/extensions/<id>/extension.json)                                                 │
│    chat  → mounts the React face  (web/ → ui/chat-web/whirl.js)                             │
│    code  → vanilla canvases       (ui/extensions/code/code.js)                              │
│    third-party → sandboxed iframe + apps/sdk.js                                              │
└─────────────────────────────────────────────┬───────────────────────────────────────────────┘
                                              │ invoke(cmd, args)   ·   events ("state", …)
┌─────────────────────────────────────────────▼───────────────────────────────────────────────┐
│  crates/tauri-app/src/main.rs   ~160 #[tauri::command]s, thin: parse, call core, return     │
└─────────────────────────────────────────────┬───────────────────────────────────────────────┘
┌─────────────────────────────────────────────▼───────────────────────────────────────────────┐
│  crates/core      the harness: chat, agents, routines, memory, traces, projects, tickets,   │
│                   worktrees, router, providers, tools, skills, accounts, apps, relay,       │
│                   model gateway (:7422), companion (phone), remote (machines), MCP          │
│  crates/runner    one turn on any CLI / Ollama / OpenAI-compatible / A2A → Events           │
│  crates/colony    disposable WSL2 / Hyper-V workspaces                                       │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
     other front doors on the same core:  crates/cli (headless, MCP server)  ·  crates/app (GPUI, frozen)
```

## Rules

1. **Core owns state. The UI owns nothing it can't rebuild.** Everything that has
   to survive a restart lives in core: threads, agents, memory, traces, routines,
   prefs. The UI may cache in `localStorage`, but only for conveniences such as
   the last tab or a panel width.
2. **Commands are thin.** A Tauri command parses its arguments, calls one core
   function and returns its result. Logic in `main.rs` can't be tested and can't
   be reached from the CLI or the phone. `chat_context` is the pattern:
   `Chats::context_map` lives in core, has a test, and the command is one line.
3. **The frame knows apps only through their manifest.** An app draws only inside
   its own section. It reaches the frame through `Frame.register` (native apps) or
   the SDK (web apps), never through another app's globals.
4. **One agent model, one memory, one trace format, shared by every app.** A new
   app adds a view, never a parallel store. Design and Clip Studio use `Agent`,
   `Routine`, `memory::propose` and traces as they are.
5. **Every model call goes through the runner.** A turn is a `Request` that
   produces `Event`s. That is what gives a call its trace, its cost and Carry on.
   Code paths that spawn CLIs directly are debts.
6. **Files over databases.** Data is stored as JSON and Markdown under `<data>/`,
   with memory as a git repo. That keeps it readable, diffable and easy to sync.
   Add a database only once a measured query needs one.
7. **Local first, loopback first.** Servers bind `127.0.0.1` with a token. LAN
   exposure is opt-in. Credentials stay with the CLI or provider that owns them.

## Data on disk

| Path | Owner | What |
|---|---|---|
| `~/.config/backspace/prefs.json` | `prefs.rs` | Settings, appearance, machines, model gateway |
| `<data>/threads/` | `chat.rs` | Threads and their messages, one JSON each |
| `<data>/agents/<id>/` | `agents.rs` | An agent's definition and its workspace |
| `<data>/memory/` | `memory.rs` | The memory git repo (Agent Memory Repo format) |
| `<data>/memory-inbox/` | `memory.rs` | Proposed notes waiting for the Librarian |
| `<data>/traces/` | `trace.rs` | One trace per reply, OpenTelemetry shape |
| `<data>/apps/<id>/` | `apps.rs` | Installed third-party apps |
| `<data>/relay/` | `relay.rs` | Harness tiles and their outbox |
| `<workspace>/.backspace/` | `harness.rs` | A Code run's state, tickets and worktrees |

## Key flows

**A message to an agent.**
1. The composer calls `send`, which goes to core `Chats`.
2. Core builds the system prompt from the instructions, personality, memory for
   the scope and the app's brief.
3. The runner runs the turn on the agent's route.
4. Events stream back. Spans are written to the trace.
5. Progress is saved before and after each tool call, so Carry on can resume.

**Memory in.** Any app or agent calls `memory::propose`, which writes to
`memory-inbox/`. The Librarian's `drain` keeps, merges or drops each note, makes
one commit and writes a receipt. The UI reads `memory_list`.

**Memory out.** `Memory::context(project)` picks notes by scope, ranks them by
importance, use and age, and fits them to a budget. `context_notes` returns the
same set without counting a use, for the context map.

**A routine.** Core keeps `next_run`, and a ticker in `fleet.rs` fires due
routines into the agent's thread, one per agent per tick, never while it is
answering. The ticker lives in the app process, so nothing fires while the app is
closed (see *Debts*).

## How apps plug in

| Kind | Where | Can do |
|---|---|---|
| First-party native | `ui/extensions/<id>/` and its `extension.json` | Anything in the page's JS, with care: it shares globals with the frame |
| First-party React | A native extension that mounts a bundle built from `web/` | The `Host` bridge in `web/src/bridge.ts` |
| Third-party | `<data>/apps/<id>/` and its `backspace-app.json` | Only what `permissions` grants, through `apps/sdk.js`, inside a sandboxed iframe |

**Design** and **Clip Studio** should be first-party React apps, with each one's
logic in core. Clip Studio's render is a core job that shells out to `ffmpeg`
through the runner's process handling, so it gets a trace and can be cancelled.

## Debts

| Debt | Why it matters | Pay it off by |
|---|---|---|
| Two UI stacks: vanilla frame and React chat face | Two token systems and two component sets. The same control (a toggle, a pill) gets built twice. | New surfaces go in React. Move the frame pages (Home, Settings) one at a time. Share tokens first (DESIGN.md). |
| `main.rs` is ~2.3k lines | Logic is creeping into commands | Move logic into core modules. Keep commands to one call each. |
| `chat.rs` is ~2.3k lines | Hard to change safely | Split it into thread store, turn, context and groups |
| Code workers (`harness.rs`, `fleet.rs`) skip the runner | They get no traces, and they don't resume after a restart | Port them onto `backspace-runner` |
| Routines need the window open | A routine "on a clock" silently misses runs | A background agent: a launchd or systemd user unit, or a tray process |
| The phone link binds `0.0.0.0` with a long-lived token over HTTP | It is a remote path to agents that run commands | Short-lived pairing, binding only while pairing is on, and TLS or Tailscale |
| The GPUI shell (`crates/app`) | A second shell to keep compiling | Freeze it, and stop adding to it |
| Pele adds about 9.5 MB of lazy chunks | Bundle size | Accept it for now. Revisit if startup or update size hurts. |

## Decisions to agree on

1. **React for every new surface.** Yes or no? The recommendation is yes.
2. **Freeze GPUI.** The recommendation is yes.
3. **A background routine runner.** It is needed before routines are a promise.
4. **Pending notes** from apps that infer: shown on Home, one tap to keep.
