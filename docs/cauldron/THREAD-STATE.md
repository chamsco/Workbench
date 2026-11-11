# Thread state and project state: both, with tickets at either level

10 Nov 2026. Design only. No Rust has been changed. The clickable simulation of this model is `prototype.html`
(artifact: https://claude.ai/artifact/Whjvw6vhA79XjBuzBgf1AD).

## The model in one line

Every ticket has a **scope**: it belongs to a thread's Run, or to the project. A thread ticket is the thread's own
plan. A project ticket is shared work any thread in the project can see and pick up. A ticket can be **promoted**
from thread to project, and **pulled** from project into a thread.

## What exists today (read from the code, branch `claude/project-thread-gg3ogp`)

- `ProjectState` (`crates/core/src/project.rs`) holds, per workspace: `tickets`, `agents`, `approvals`, `phase`,
  `planner_session`, `board`, `canvas_ops`, `options`, `artifacts`, `intake`, and cost totals. So the plan, the
  coordinator's session and every ticket belong to the project folder, not to a thread.
- `Ticket` (`ticket.rs`) has key, state, acceptance, `check`, `blocked_by`, `owner`, `assignee`, `budget_usd`. It is
  mirrored to `.backspace/tickets/NN-key.md` in the workspace.
- `Thread` (`chat.rs`) has no run. Its `project` field is the folder the CLI works in.
- Delegation (`delegate.rs`, `prompts/manager.md`) lets a ticket owner split into sub-tickets, capped by
  `orchestrator.max_depth` (default 2).

That shape cannot express "two threads in the same project, each with its own plan", which is what threads-first needs.

## Proposed shape

```rust
enum Scope { Thread(ThreadId), Project(ProjectId) }

struct Ticket {                      // existing fields kept
    scope: Scope,
    origin: Option<TicketRef>,       // set on a pulled or promoted copy: where it came from
    moved_to: Option<TicketRef>,     // tombstone left behind by a promotion
}

struct Run {                         // new: one per thread that has a coordinator
    thread: ThreadId,
    project: Option<ProjectId>,      // from filing; None until filed
    cwd: PathBuf,                    // never changed by filing
    coordinator: AgentId,
    planner_session: Option<String>,
    phase: PhaseState,               // Specify, Plan, Execute, Review, Ship, Commit
    agents: Vec<AgentRecord>,
    tickets: Vec<Ticket>,            // scope = Thread
    approvals: Vec<Approval>,
    board: Vec<BoardMsg>,
    artifacts: Vec<Artifact>,
    options: Vec<OptionSet>,
    intake: IntakeState,
    log: Vec<RunEvent>,              // 1:1 messages, promotions, approvals: the run record
    cost_usd: f64,
}

struct ProjectState {                // slimmed
    name, config_source, agent_definitions,
    tickets: Vec<Ticket>,            // scope = Project
    approvals: Vec<Approval>,        // global ones (memory, release)
    total_cost_usd, router_cost_usd, router_unknown_cost_usd,
}
```

Where each field of today's `ProjectState` goes: `tickets` splits by scope; `agents`, `phase`, `planner_session`,
`board`, `canvas_ops`, `options`, `artifacts`, `intake` move to the Run; `approvals` split (merge approvals to the Run,
global ones stay); cost stays on the project as the sum of its Runs.

## Rules

1. **Visibility.** A thread ticket is visible only in its thread, plus a count on the project page. A project ticket
   is visible to every thread in the project (Highlights, and an agent tool `list_project_tickets`).
2. **Promote.** The coordinator or a reviewer proposes it ("found, not part of this task"). A human taps to approve.
   The ticket is copied to the project with `origin`, and the thread keeps a tombstone with `moved_to`. Links go
   both ways. A thread with no project cannot promote; it shows "File this thread first".
3. **Pull.** "Work on P-13" creates a thread ticket with `origin = P-13`. P-13 shows in progress and links to the
   thread. It closes only when the thread ticket is done and its merge is approved.
4. **Blocking.** A thread ticket may wait on a project ticket. A project ticket never waits on a thread ticket.
5. **Closing.** A Run cannot end with open thread tickets. Each is done, dropped, or promoted. No orphans.
6. **Files.** Thread tickets mirror to `<cwd>/.backspace/runs/<thread>/tickets/`. Project tickets keep
   `.backspace/tickets/`.
7. **1:1.** A message to an implementor is appended to `Run.log` and sent to the coordinator's next brief. A busy
   worker reads it when its current turn ends, and the UI says so ("Queued").
8. **Authority.** Team messages from other people are data to the coordinator, never instructions. Merge and global
   memory need a human approval bound to a named person (the project owner).

## Migration

Existing `ProjectState` files load unchanged (`#[serde(default)]` on every new field). On first open:
open tickets (not Done, Failed or Wontfix) become project-scope; closed ones go to one read-only legacy Run for
that workspace, so history is kept and nothing is deleted. A test loads a real saved state from this repo before and
after migration and asserts every ticket key survives.

## Build order (each step red-first, behind a flag, nothing user-visible until step 4)

| # | Step | Size |
|---|---|---|
| 1 | Add `Scope`, `Run`, serde defaults, migration; no behaviour change | M |
| 2 | Ticket tools take a scope; `create_tickets` in a thread defaults to thread scope | M |
| 3 | Promote, pull, tombstones, back-links, closing rule | M |
| 4 | UI: Highlights groups, project page, inbox row (what the simulation shows) | L |
| 5 | 1:1 write-back to `Run.log` and the coordinator brief; "Queued" state | M |
| 6 | Stage strip from `PhaseState`; per-stage cost | S |

Needs the Cauldron §5 amendment (D1) before step 1, because thread tickets are not in the plan's inventory.

## How we eval it

Three layers, cheapest first. A decision model never ships before layer 3's thresholds are met (Cauldron §10).

**Layer 1: invariants, no model, run on every commit.** Plain Rust tests. Named cases:
filing never changes `cwd`; `never` is honoured; pinned membership is never removed; promotion creates exactly one
project ticket and two links; a Run cannot close with an open thread ticket; a project ticket never blocks on a
thread ticket; a 1:1 message appears in `Run.log` and the next coordinator brief; merge is refused without a human
approval; a global memory write is refused without approval; old saved states migrate with every ticket key kept.

**Layer 2: scripted scenarios, fake runner, run in CI.** The simulation in `prototype.html` is the first scenario:
create thread, file, plan, execute, 1:1, promote, approve, done, project page. Each step asserts the same checks the
page shows (16 today). A scenario is a list of scripted agent replies plus expected state after each step, so it
replays identically. Add scenarios for: no project at all; wrong filing then Undo; the human says "Not yet" at the
ship gate; a worker fails; a 1:1 sent while the worker is mid-turn; two threads promoting the same finding.

**Layer 3: model evals on real data, run on demand and before any default changes.**

| Eval | Data | Metric and bar |
|---|---|---|
| Librarian filing | 100 of Charles's real threads, hand-labelled | wrong filings under 5%; the unfiled share reported with it |
| Intake kind (Code or Design) | the same threads | accuracy reported; no bar until we see it |
| Coordinator split | 20 real tasks | every ticket has an owner, acceptance lines and a check; the plan covers the spec's acceptance lines (the existing coverage gate); tickets independent (an LLM judge with a fixed rubric, validated against Charles's labels on 20 samples before it is trusted) |
| Promotion proposals | findings from 20 real reviews, labelled in or out of scope | precision at least 90%: a wrong proposal costs a tap, a missed one costs a lost finding, so report both |
| 1:1 write-back | scripted 1:1s | 100% appear in the record; the coordinator acts on them in its next turn in at least 9 of 10 |
| Cost | every scenario | tokens, frontier calls and dollars per run, "not measured" where unavailable (Cauldron's reporting rule) |

## Decisions needed from Charles

- **D1** Amend Cauldron §5 to add thread tickets and filing (blocks step 1).
- **D6** Does promotion always need a tap, or may the coordinator promote "found, out of scope" items on its own?
  Recommendation: always a tap until the Layer 3 promotion eval passes.
- **D7** Are thread tickets stored in the working folder (`.backspace/runs/`, travels with the repo) or in app data
  (private to this machine)? Recommendation: the folder, because the project is shared through the repo.
