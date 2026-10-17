ADE and IDE: two surfaces over one machine

Decision taken 16 Oct 2026, from the live run and from T3 Code's architecture.
This records what the split is, why "Chat / Code" was the wrong axis, and what
is left to build.

## Why not "Chat and Code", and not "Chat and Agents"

Today the rail has Chat, Code, Memory, Apps. The names suggest Chat is talk and
Code is work, but that is not what the zones do:

- Chat runs coding CLIs. `crates/tauri-app/ui/chat.js` builds its route list from
  `harnesses.rs` `Kind::Cli` entries — Claude Code, Codex, Cursor, Grok,
  OpenCode — alongside Ollama and routers. A "chat" in Backspace is frequently a
  coding session.
- The Code zone's main agent *is* a conversation (`AgentRecord.log`, the chat
  face in Pair mode). The planner talks, the workers talk, tickets have threads.

So "Chat vs Code" splits by a difference that does not exist. T3 Code's
`docs/internals/overview.md` makes the distinction that does: execution belongs
to the environment that owns the workspace, and clients are thin views over it.
One machine, several windows onto it. Their clients are desktop/mobile/web; ours
are two surfaces inside one window.

## The split

**One ADE (agent development environment).** The thread is the unit: a session
with a thread, its **artifacts**, and a composer. This is where you talk to the
main agent, read what it produced, and approve or reject. Everything a session
produces attaches to the thread that produced it.

**One IDE-ish workspace view.** Canvases, worktrees, tickets: the things that
are true of the *project* rather than of one conversation. A canvas holds no
chat. Home starts a project by a task (a session) or by a view (the workspace).

The pair maps to what already exists:

| Surface | Existing pieces |
|---|---|
| ADE | `Chats` + `chat.rs`, the Whirl chat face, `trace.rs` timelines, `options.rs` picks, `board.rs` |
| Workspace | `layout.rs` + `prefs.tabs_by_project`, `git.rs` worktrees, `ticket.rs`, the diagram canvas |

The rail becomes: **Agents** (ADE) · **Workspace** · **Memory** · **Apps**, with
**Apps first**, since a plugin is a place you go rather than a mode you enter.

## Artifacts: what T3 gets right and what we should copy

T3 keeps execution server-side and treats a thread's output as structured objects
(its event log is the source of truth; checkpoints are hidden Git refs, not
commits). The transferable part is not their stack, it is the shape:

1. **One thread, many artifact kinds.** We already have `options.rs` (a set of
   variants the user picks from), `trace.rs` (a timeline), `diagram.rs` (a
   review graph), `files.rs` (an image as a data URL), and tickets. Each is an
   artifact with no common type. A reply should be able to carry artifacts, not
   only text and tool calls.
2. **Artifacts are addressable.** An artifact has an id, an owner (the agent that
   made it), and a thread. Then "show me the diff from ticket 3" is a lookup, not
   a re-render.
3. **Checkpoints before review.** T3 snapshots workspace state with hidden refs
   so a revert coordinates with the conversation. We use a real commit plus a
   worktree; that is the same idea with more visible scars.

Concrete first slice (not built): an `Artifact` enum on `ProjectState`
(`Diff { ticket }`, `Image { path }`, `Options { set }`, `Trace { id }`,
`Document { path }`, `Table { rows }`), produced by the existing tools, and one
renderer per variant inline in the thread. That is where "enriched chat" comes
from — the answer stays prose, and the thing it is about is a real object.

## Shipped 17 Oct 2026: the two names inside Code

The rail rename is still open, but Code's own switch now says what it is. It read
"Chat | Agents" — Chat's axis, borrowed — and now reads **ADE | IDE**:

- **ADE** (`prefs.code_view: "pair"`) is the agent development environment: the
  chat face scoped to the project, as before, but it lands on a coding home
  rather than the agent grid. `web/src/app/CodeHome.tsx` shows the folder, its
  branch, the sessions you have already had in it, what is uncommitted, the
  tickets and the worktrees, and the composer under it starts the next session.
  The data comes from `Host.codeHome()` in `chat.js`, so the face never reaches
  into the shell's internals for `snap`.
- **IDE** (`"agents"`) is the project's workspace: canvases, worktrees, tickets.
  It opens on **one full-window empty canvas** — a project's default tab is
  `TabSpec::defaults()`, one empty canvas, whose whole face is the chooser — so
  the IDE asks what you want open instead of opening a chat. The canvas pane
  keeps every option it had (Main agent, Browser, Diagram review, Worktree,
  PLAN.md, Team chat, Conversation, Terminal, Visual options). Its view menu is
  **Canvases** and **Wall**: Start is the ADE's landing, and a canvas holds no
  chat, so it is not an IDE view. A stored `prefs.wb_view: "home"` therefore
  lands on Canvases rather than opening the IDE on the home screen.
- The IDE's session canvas carries the chrome an IDE is read through: a **session
  card** at the top of the transcript (`sessionCard()`) naming the CLI, its
  version, the model and effort it is on, how it is signed in, and the folder —
  and a **status line** (`statusHTML()`) reading `<mode> on (shift+tab to cycle)
  · N agents`. Shift+Tab really does cycle the CLI's permission mode in the IDE
  (`cyclePerm()`), which is why the line can say so.

Stored values are still `"pair"` and `"agents"`, so no prefs migration was
needed. Both surfaces draw the same sidebar (`#chatList`, the project tree):
`chat.css` forces it visible wherever Code is showing, and the vanilla
Projects/Workers list stands aside for it. The tree's root is the **machine**
— expanding it shows the folder it is running right now, with that folder's
canvases and threads under it, and the ⋯ beside the row is the machine picker
(`EnvironmentRow` in `web/src/app/ThreadList.tsx`). The card stays glass in
both, and it shows the painting at one strength in either surface: the ADE's
backdrop is vivid, so there the card carries its own layer of page colour and
comes out at the muted strength the rest of the app paints (`shell.css`,
`.win.m-code.art-vivid .side`). What the zone paints is untouched.

## What is left

1. **The rail rename and its order** — `shell.js` `ZONES` is
   `["chat","bubble","Chat"], ["code","codei","Code"], …`; the labels and the
   order change, and `prefs.mode` values need a migration (`"chat"` → `"ade"`,
   `"code"` → `"workspace"`) so an existing prefs file keeps working. Only the
   labels *inside* Code have moved so far.
2. **The workspace view inside the ADE** — the IDE is a first-class surface now,
   but the ADE's own home is read-only: it shows tickets and worktrees, it does
   not yet open one in place.
3. **The artifact type**, per above.
4. **The old Chat design** (asked for on 16 Oct): brought back as the ADE's own
   look — the opaque layer rather than the frosted one, with the agent/project
   rail and the git history beside the thread. The Whirl face stays as the
   thread renderer; the frame around it is ours again.

## Open questions

- Whether a session can span more than one project. Today a thread has one
  `project`, and the workspace view is per project. Keeping it that way is
  simpler and matches "the workspace owns execution".
- Whether Cloud lives in the ADE only (it is nightly-only either way; see the
  `nightly()` gate in `app.js`).
