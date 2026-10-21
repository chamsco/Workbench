# OptMem: Exploration for Agent Memory Compaction

Branch: `nightly/feature/delegation`
Date: 21 Oct 2026
Source: [github.com/VictorTaelin/OptMem](https://github.com/VictorTaelin/OptMem)

---

## What OptMem is

OptMem is a **permanent, append-only memory for AI agents**, built by Victor
Taelin (the author of HVM and Bend). It is one Python file (`memo`, no
dependencies), a 426-token prompt pasted into the agent's instruction file,
and a fixed-width file format. That is the whole system.

The pitch: the agent wakes, reads its memory context, works, notes what
mattered, and when the tool asks it to compress two memories into one summary,
it does so before its next action. Old memories stay verbatim; ancient ones
collapse into tree-structured summaries that can be navigated back to the raw
record. Nothing runs in the background. The agent is the scheduler.

### The files

```
~/.optmem/
  memo          the tool: one file of Python 3, no dependencies
  memory/
    LOG.txt     every memory, one per line, append-only, never edited
    TREE/       the summaries: a cache, rebuildable from the log alone
    config      the sizes, written by `memo config`
```

### The commands

| Command | What it does |
|---|---|
| `memo wake [part [T]]` | Read the memory context. First call of every session. Paginates output to fit harness caps. |
| `memo note "<one line>"` | Record one memory (max 280 bytes). Triggers a compression if one is due. |
| `memo nap <lo>-<hi> "<line>"` | Compress a block of memories into one summary line. |
| `memo nap` | Ask for the next pending compression. |
| `memo recall <regex>` | Search every memory ever recorded, streaming, bounded by output cap. |
| `memo zoom <lo>-<hi>` | Open a tree node into its two halves, down to raw memories. |
| `memo forget <lo>-<hi>` | Drop a bad summary; the next nap rebuilds it. Log is never touched. |
| `memo config [NAME=VALUE]` | Show or change the sizes (WAKE_LINES, ENTRY_CHARS, PART_CHARS, PART_LINES). |
| `memo import <file>` | Bulk-load dated memories (bootstrap only). |

---

## The core idea: detail decays with age

The central data structure is a **binary merge tree** over an append-only
log. Every memory is one line in `LOG.txt`, numbered `#0`, `#1`, `#2`, ...
Memories are always appended — never edited, never deleted. On top of this
log sits a tree of one-line summaries:

```
#0-1   summary of memories #0 and #1
#2-3   summary of memories #2 and #3
#0-3   summary of #0-1 and #2-3
#4-7   summary of memories #4 through #7
#0-7   summary of #0-3 and #4-7
...
```

Each level halves the resolution. The agent sees the tree's top — the
coarsest summaries — and can `zoom` into any node to read its two halves,
repeating until it reaches raw memories.

### The cover function

`wake` does not print the whole tree. It prints at most `WAKE_LINES` (default
96, ~8k tokens) lines, tiled as **aligned power-of-two blocks**. The key
insight is the `_cover(T, alpha)` function: it tiles `[0, T)` with blocks
whose size is at most `alpha` times their age. Bigger `alpha` = coarser =
fewer lines. A binary search over `alpha` finds the value that hits the
budget exactly.

The result: **recent memories stay verbatim, ancient ones collapse**. If
everything fits in the budget, nothing is compressed at all — the tree is
empty, and `wake` just prints the log.

### Fixed-width records

Records are padded to a fixed width (`LOG_REC = 320` bytes for log entries,
`TREE_REC = 288` for summaries). This means:

- **Position is identity.** Memory `i` lives at offset `i * LOG_REC`.
  Block `[lo, hi)` lives at offset `(lo // size) * TREE_REC` in the level
  file for that size. Every lookup is one `seek`, no scan, no index.
- **No database.** No SQLite, no LMDB, no B-tree. Just files.
- **Crash-safe.** A partial trailing record (from a crash mid-write) is
  truncated on the next append, so records stay aligned.
- **Costs ~2x on disk** (padding), buys O(1) everywhere.

At a million memories (608 MB), `wake` takes 0.03s.

### The compression loop

Compressions happen **one at a time, in the output of `note`** — never in the
background. When a new memory creates a pair that can be merged, `note`
prints a prompt:

```
Compress memories #0-1 into one line of at most 280 bytes.
Keep what has lasting effect, drop what does not. Invent nothing.

  #0 2026-10-21 the first thing that happened
  #1 2026-10-21 the second thing that happened

Run: ~/.optmem/memo nap 0-1 "<your line>"
```

The agent does the compression before its next action. Blocks are built in
order (smallest first), so there is never a spike — one new memory creates
at most `log2(N)` pending compressions.

For blocks larger than `RAW_MAX` (16), the prompt shows the two child
summaries instead of raw memories — compressing summaries of summaries.

### Pagination

Every harness truncates differently: Claude Code cuts the middle at 30,000
chars, pi cuts the head at 50 KB, Codex budgets 10,000 tokens. So `wake`
hands the memory over in **parts** that fit all of them. Each part ends with
either "Not awake yet. Run: `memo wake 2 <T>`" or "You are awake."

Parts are rendered **as of a fixed `T`** (the log length at the first call),
so a note landing between two parts cannot shift a boundary and drop a line.

### Subagent safety

The prompt explicitly tells the agent: parallel sessions on the same machine
are all the same identity and may all write memories. A **subagent** is not —
it must never run `memo`, because it cannot judge what is already known, and
its notes would arrive duplicated and incorrectly. When spawning one, write:
"You are a subagent. Don't run memo."

---

## What Workbench's memory does today

Workbench already has a memory system (`crates/core/src/memory.rs`,
`memory_dream.rs`, `memory_learn.rs`). It stores notes as bullets in a git
repo of Markdown, following the [Agent Memory Repo](https://github.com/AgentMemoryRepo/agentmemoryrepo)
format. Each note is scoped (global, per-project, per-agent), has metadata
(id, date, kind, importance, source, uses), and is committed as one change.

**What it does well:**
- Human-readable, hand-editable, git-versioned
- Scoped notes (global, project, agent)
- Auto-capture from chat messages (the learner scores messages locally)
- Dreaming (a model tidies memory now and then: merge, edit, delete, add)
- The Librarian pattern: agents propose notes to an inbox; the owning process
  files them as one commit (`drain`)
- Ranking by importance × use × age, with a 6,000-char budget per prompt

**What it does not do:**
- **No structural compaction.** Notes accumulate. Dreaming merges duplicates
  and removes stale ones, but it does not build a hierarchical summary tree.
  After 500 notes, the context is either truncated or incomplete.
- **No temporal decay.** A note from October is as present as one from today.
  The ranking formula (`importance + ln(uses) * 0.1 - age/365`) barely
  decays — a year-old note loses 0.5, which is less than the noise floor.
- **No navigation.** You can search (`memory_search`) or read the context
  block, but you cannot zoom from a summary into its constituents.
- **No per-agent episodic memory.** Notes are scoped by project or agent, but
  they are flat — there is no notion of "what happened in this session,
  compressed over time."
- **The budget is character-counted, not token-aware.** `BUDGET = 6000`
  chars is a rough proxy, and it truncates by ranking, not by building a
  compact representation.

---

## What OptMem adds: the compaction gap

The delegation branch adds `crates/core/src/delegate.rs` — a board of tasks
dispatched to agents. Each delegated task produces an outcome. Today those
outcomes land in `.backspace/outcomes.jsonl` and are never read back unless
an agent searches for them. There is no mechanism to compress a long
session's worth of outcomes into a summary that the next session can wake to.

This is exactly the gap OptMem fills. Its model is:

1. **Every session wakes to a context that fits in the prompt budget.**
   `wake` prints 96 lines (~8k tokens) of summaries and raw memories,
   tiled so the present is detailed and the past is coarse.

2. **The agent is the compactor.** When `note` says "compress #0-1," the
   agent produces a one-line summary that keeps what has lasting effect and
   drops what does not. This is a judgment call — exactly what a model is
   good at — and it happens inline, not in a background batch.

3. **Detail is recoverable.** A summary at level N is the compression of two
   summaries at level N-1. `zoom` opens any node into its halves, all the
   way down to raw memories. `recall` searches the raw log. Nothing is ever
   lost; the tree is a cache rebuildable from the log alone.

4. **The work never spikes.** One new memory creates at most `log2(N)` pending
   compressions. At 1,000 memories, that is 10. At a million, 20. The
   compressions are served one at a time, so the agent never stalls.

### Where this matters for Workbench

- **Delegated agents** (the new `delegate.rs`): each task outcome is a "memory."
  Over a long project, hundreds of outcomes accumulate. Without compaction,
  the next agent either reads all of them (expensive, blows the context
  window) or reads none (loses institutional knowledge). With OptMem's tree,
  the agent wakes to a compact summary and zooms in where it needs to.

- **The Librarian** (planned in `PLAN-PHASES-2026-10-17.md`): the phase plan
  names the Librarian as "the single writer to the memory repo: subagents
  propose, the main agent compiles, the Librarian decides, places, supersedes
  and commits once per batch." OptMem's `note` → `nap` loop is a simpler
  version of the same idea: the agent notes what happened, and when a pair is
  due, it compresses. The Librarian could use OptMem's tree as the episodic
  store underneath the curated notes.

- **Cross-session continuity**: Workbench's Carry On resumes a cut-off reply,
  but it does not resume the *context* — the agent re-reads its system prompt
  and the conversation, but not what it learned last session. OptMem's `wake`
  is the first command of every session: the agent reads its memory before
  doing anything else.

---

## Architecture comparison

| Aspect | Workbench memory (`memory.rs`) | OptMem (`memo`) |
|---|---|---|
| **Storage** | Git repo of Markdown files | Fixed-width flat files (LOG.txt + TREE/) |
| **Unit** | A bullet with metadata (id, date, kind, importance, source) | One line of text (max 280 bytes) with id and date |
| **Scope** | Global, per-project, per-agent | One identity (the machine), but `$MEMORY_DIR` can point elsewhere |
| **Compaction** | Dreaming: a model merges/edits/deletes notes in a batch | Binary merge tree: pairs → summaries → summaries of summaries |
| **Context** | Top-N notes by ranking, 6,000-char budget | ~96 lines (~8k tokens), tiled by temporal decay |
| **Navigation** | `memory_search` (full-text) | `recall` (regex on raw log) + `zoom` (tree navigation) |
| **Recovery** | Git history (revert) | `forget` drops a summary; `nap` rebuilds it from the log |
| **Concurrency** | Inbox + `drain` (single writer) | File locking (fcntl on Unix, msvcrt on Windows) |
| **Dependencies** | Git, serde, the AMR format | Python 3, nothing else |
| **Agent integration** | System prompt injection (notes as context) | 426-token prompt pasted into AGENTS.md |
| **Who compacts** | A model, in a background dream | The agent itself, inline, on demand |

The two systems solve **different problems**: Workbench's memory is a
curated knowledge base (preferences, facts, standing instructions), while
OptMem is an episodic log with structural compaction. They are complementary,
not competing.

---

## How OptMem could be integrated

### Option A: Use OptMem as-is, as an external tool

The simplest path: install OptMem alongside Workbench, and let each agent that
has its own folder (Chat agents, delegated workers) wake to its own OptMem
store. The agent's `AGENTS.md` gets the 426-token prompt block, and the
`$MEMORY_DIR` environment variable points each agent at its own memory.

This requires no Rust changes. The agent's CLI (Claude Code, Codex) already
runs shell commands. The `memo` script is one Python file with no
dependencies.

**What this gives:** every agent gets permanent, compacting episodic memory
across sessions. The delegation board's outcomes become memories the next
agent can wake to.

**What it does not give:** integration with Workbench's curated memory (the
AMR notes). The two stores would be separate — AMR for preferences and
facts, OptMem for what happened and what was tried.

### Option B: Port the binary merge tree into Workbench's memory

Port the `cover`, `pending`, `nap`, `zoom`, and `recall` logic from Python
to Rust, and add an episodic store alongside the existing AMR notes. The
store would live in `<data>/memory/episodic/` with the same fixed-width
format.

This gives a unified memory: curated notes (AMR) for the "what to always
know" and an episodic tree for "what happened, compressed over time." The
Librarian could write to both: proposals go to the AMR inbox, and session
summaries go to the episodic log.

**Effort:** moderate. The Python is ~500 lines of clean, single-file logic.
The tree math (`_cover`, `cover`, `pending`) is the hard part — about 80
lines — and the rest is file I/O and command dispatch. A Rust port would
be ~800 lines including tests, and would live in a new `memory_tree.rs`.

### Option C: Hybrid — OptMem for agents, AMR for the system

Keep Workbench's AMR memory as the system's curated knowledge base. Add
OptMem as an **opt-in episodic store** per agent, triggered by a new memory
kind or a per-agent flag. When an agent's session ends, its outcomes are
`note`d into its OptMem store; when it next wakes, it reads the context.

This is the lightest integration: no Rust changes for the core, just a new
tool (`memory_wake`, `memory_note`) that shells out to `memo`, and an
agent-level config flag to turn it on.

---

## Key design decisions worth stealing

Regardless of integration depth, several ideas from OptMem are worth
adopting in Workbench's memory:

1. **Detail decays with age, structurally.** Instead of a ranking formula
   that barely decays, build a tree where old memories are compressed and
   new ones are verbatim. The `cover(T, budget)` function is the right
   abstraction: tile the log into blocks whose size is proportional to age,
   so the present is always detailed.

2. **The agent is the compactor.** Workbench's dreaming uses a model in a
   background batch. OptMem asks the agent to compress inline, as part of
   its normal turn. This is cheaper (no separate API call), more contextual
   (the agent knows what it just did), and more reliable (the compression
   happens before the next action, not "now and then").

3. **Fixed-width records make position identity.** Workbench's AMR notes
   are variable-length bullets in Markdown. This is fine for human
   readability but means every read scans. For the episodic store,
   fixed-width records would make "memory #1234" a single seek.

4. **Nothing is ever lost.** OptMem's log is append-only and never edited.
   The tree is a cache. `forget` drops a bad summary; `nap` rebuilds it.
   Workbench's AMR has git history for this, but git revert is a blunt
   instrument. A rebuildable cache is cleaner.

5. **Pagination that survives every harness.** OptMem's parts are sized to
   fit the strictest harness cap (Claude Code: 30k chars, pi: 50k, Codex:
   10k tokens). Workbench's `BUDGET = 6000` chars is less than all of them,
   but it truncates by ranking rather than building a compact representation.
   The paginated, as-of-T approach is better.

6. **The "You are awake" terminator.** Every `wake` ends with "You are
   awake." This is a contract: the agent runs parts until one says it is
   done. Workbench has no such contract — the context block is injected
   silently into the system prompt.

---

## The prompt (verbatim)

This is what the installer prints, and the whole of the integration. It is
426 tokens.

```markdown
## Memory

Your memory is OptMem:
- The tool is `~/.optmem/memo`
- Your memories are in `~/.optmem/memory`

OptMem outlives every session, compaction, model and vendor change.
Without it you do not know who you are, or what was decided and tried.

### At startup: activating OptMem (mandatory)

Run `~/.optmem/memo wake` before any other tool call, in every session, and
then do exactly what it prints, to the end of its output.

### While working: register memories (mandatory)

Call `~/.optmem/memo note "<1 line, max 280 bytes>"` whenever you learn
something new, or something worth keeping happens. That covers a task
worth real effort, a fact or insight the user teaches you, anything you
learn about their life (even indirectly), any event of lasting effect.

Do not register redundant memories.

If `~/.optmem/memo note` asks a compression: do it before your next action.

Never edit or delete anything under `~/.optmem/memory`: the tool manages it.

### When you need an old memory: search, or navigate

`~/.optmem/memo recall <regex>` searches every memory, word for word.

Your memories also form a binary tree: #0-1, #2-3 ... exist as one-line
summaries, pairs of those as #0-3, and so on -- every `#a-b` line wake
prints is one node of it. `~/.optmem/memo zoom <a-b>` opens a node into its
two halves, down to the raw memories.

### If you're a subagent: skip everything above

Parallel sessions on this machine are all you, and may all write memories.
A subagent is not: it must never run `memo`, because it cannot judge what
is already known, and its notes would arrive duplicated and incorrectly.
When you spawn one, write: `You are a subagent. Don't run memo.`
```

---

## Test coverage

OptMem ships with `test.py` — a synthetic life of 2,000 memories, checked
against an oracle for the block math, and against the real CLI for
concurrency, crash recovery, UTF-8, pagination, and every command. The test
suite is ~600 lines and covers:

- **Block math:** `cover(T, budget)` for T from 1 to 100,003 — no gaps, no
  overlaps, blocks are aligned powers of two, detail increases toward the
  present.
- **The CLI:** init, wake, note, nap, recall, zoom, forget, config, import —
  every command, every error path, every edge case.
- **Concurrency:** 16 parallel processes writing notes — all 16 survive,
  no id collision.
- **Crash recovery:** a torn write (half a record) is truncated, and the
  next append lands at the right offset.
- **UTF-8:** multi-byte characters do not shift record boundaries or dodge
  the byte limit. The store is UTF-8 regardless of locale.
- **Pagination:** every part fits every harness's cap (30k chars, 2k lines).
  A note between parts does not shift a boundary.
- **The "You are awake" contract:** every wake, even a one-part one, ends
  with "You are awake."
- **Corruption recovery:** a blank summary record points at `forget`, not
  a traceback or an infinite refusal.

The test is run with `python3 test.py` and passes 109,099 checks.

---

## Open questions

1. **Per-agent vs. per-project.** OptMem assumes one identity (the machine).
   Workbench has agents with their own folders and projects with their own
   scopes. Should each agent get its own OptMem store? Each project? Or one
   store per machine, with agent-id tags in each memory?

2. **The 280-byte limit.** OptMem caps memories at 280 bytes (to fit
   fixed-width records). Workbench's AMR notes can be up to 2,000 chars.
   For session outcomes, 280 bytes is tight — a one-line summary of what
   was tried, what worked, what did not, and why, is often 300–400 bytes.

3. **Integration with the Librarian.** The phase plan's Librarian is "the
   single writer to the memory repo." OptMem's model has every session
   writing directly. These are different write models. If the Librarian owns
   the AMR notes, could it also own the episodic log — writing session
   summaries as `note`s and compressing them on demand?

4. **The compression prompt.** OptMem asks the agent to "Keep what has
   lasting effect, drop what does not. Invent nothing." This is a good
   prompt for episodic memory. For Workbench's use case (outcomes, decisions,
   technical debt), the prompt might need domain-specific guidance: keep
   the decision and its rationale, drop the implementation steps.

5. **Cross-agent memory.** If agent A delegates to agent B, and B produces
   an outcome, should A's memory include B's outcome? OptMem says
   "subagents must never run memo." But if B's outcome is `note`d by A
   (after B reports back), the tree compresses it naturally.

---

## Verdict

OptMem solves a problem Workbench does not yet solve: **structural,
lossless compaction of an agent's episodic memory over time, with a bounded
context window and recoverable detail.** The implementation is remarkably
clean — one Python file, no dependencies, fixed-width files, O(1) lookups,
and a binary search over a single parameter to fit any budget.

The most valuable ideas to steal, in priority order:

1. The `cover(T, budget)` tiling — temporal decay as a data structure.
2. The inline compression loop — the agent compacts its own memory.
3. The `zoom` navigation — summaries down to raw records.
4. The "You are awake" contract — a terminal the agent waits for.
5. The subagent safety rule — one writer per identity.

The lightest integration is Option A (install OptMem as-is, per-agent).
The most valuable is Option B (port the tree to Rust, alongside the AMR
notes). Either way, the binary merge tree is the right data structure for
"what happened, compressed over time" — and Workbench does not have it yet.
