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

## What is left

1. **The rename and the rail order** — `shell.js` `ZONES` is
   `["chat","bubble","Chat"], ["code","codei","Code"], …`; the labels and the
   order change, and `prefs.mode` values need a migration (`"chat"` → `"ade"`,
   `"code"` → `"workspace"`) so an existing prefs file keeps working.
2. **The workspace view inside the ADE** — today Pair mode already scopes the
   chat face to a project (`inScope` in `web/src/bridge.ts`), which is most of
   the way. It needs to become a first-class surface rather than a scoped chat.
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
