# The four phases: specification, execution, testing, deployment

Plan written 17 Oct 2026, at `790b733`, after parking pane stacks (`pane-stacks`
branch) and taking Team chat out of the picker. Nothing in this document is
built yet. It is the build order for turning Workbench from "a chat window with
a coding harness behind it" into a pipeline you can trust end to end, with one
decision model over all four phases.

Three references shape it, all read this session:

- [whirlchat/whirl](https://github.com/whirlchat/whirl) for the chat surface and
  its incognito behaviour, already vendored and now matched.
- [cursor/plugins/pstack](https://github.com/cursor/plugins/tree/main/pstack) for
  the execution discipline: playbooks, subagent roles, `/swarm`, comment
  auditing, poteto-mode.
- [longsurf-ai/endo](https://github.com/longsurf-ai/endo), the Higher Order
  Harness, for the harness mechanics (durable runs, typed errors, permissions,
  dig-in). Our own reading of it is [ENDO-NOTES.md](ENDO-NOTES.md).
- [mattpocock/skills](https://github.com/mattpocock/skills) for specification:
  `grilling`, `to-spec`, `to-tickets`, `wayfinder`, `tdd`, `triage`.

## What is already here (do not rebuild it)

A plan that ignores this will duplicate half the repo. The four phases are
mostly wiring and gates around pieces that exist:

| Piece | Where | State |
|---|---|---|
| Skills, vendored with licence | `crates/core/skills/` (grilling, to-tickets, triage, tdd, code-review, diagnosing-bugs, ponytail) | Present. `WorkflowConfig.main_skills` already injects `grilling` + `to-tickets` |
| The grilling round | `config.rs:128` (`WorkflowConfig.grill`, default **true**), `harness.rs:959` | On, but only inside the harness, and it tells the model the opposite of what we want: "Do not interview the user; make reasonable decisions and state them in PLAN.md" |
| PLAN.md as an artifact | `app.js:1943`, GPUI `canvas.rs:852` | Written by the main agent, rendered on a docs canvas |
| Typed tickets with blocking edges | `project.rs`, `ticket.rs`, harness scheduler | Ticketing exists; `to-tickets` semantics are close |
| Tiers, effort and escalation | `config.rs:105` (`EscalationConfig`: start_max_effort, start_max_tier, max_steps, loop_repeats) | Real. Climbs on failed checks, rejections, loops |
| Roles | `config.rs:40` (`AdvisorConfig`), `harness.rs:100` (`planner`, `worker`) | Model per role exists, no table for the newer roles |
| Big/small routing | `router.rs`, `model_server.rs` (`local_subagent`, need→family table), `prefs.local_tasks` | Real, and the local model is chosen per job kind already |
| Account switching | `accounts.rs` (profiles per CLI, health, cooldown, Automatic/Manual/Off) | Wired into chat turns; worker spend does not reach `total_cost_usd` yet (HANDOFF P1-8) |
| Swarm | `fleet.rs`, `harness.rs` tickets, `max_subagents`, `max_parallel_calls` | Sequential-ish; no ready-frontier concurrency |
| Traces | `trace.rs`, `Trace.tsx`, OTLP export | Real, and the substrate the decision model should log into |
| E2E harness | `tools/uiharness/`, `tools/verify/*.js` | Real, zero-dependency, headless Chromium |
| CI | `.github/workflows/macos.yml` | One workflow, **never seen green** |
| Mutation testing | none | Missing entirely |
| Release engineering | one rolling `mac-latest` pre-release | No versions, checksums or changelog |

Two things follow from that table. First, phase 1 is mostly about turning an
existing config flag and skill into a real mode with a gate, not about inventing
an interview. Second, phases 3 and 4 are where the repo is genuinely thin.

## The spine: one decision model, five gates, one markdown chain

### The decision model

One interface, three implementations, and a hard rule about which is allowed to
answer.

```
Decider::ask(&self, q: Decision) -> Answer { answer, confidence, evidence, cost, tier }
```

- `Decision` is a typed enum, one variant per decision the system actually
  makes: `Route { role, ticket }`, `Escalate { agent, reason }`, `SpecComplete
  { spec }`, `TestsEnough { diff, plan }`, `Ship { reports }`, `PickModel { job }`,
  `ReviewVerdict { diff, spec }`.
- `Rules` answers without a model wherever a rule can. Routing inside one tier,
  budget arithmetic, "is the check green" are rules. A model must never be the
  thing that decides something deterministic.
- `SmallModel` (the scout, via `model_server` + `router.route_for`) answers
  classification questions: which playbook, which files matter, which tests are
  affected.
- `StrongModel` (the advisor, `AdvisorConfig.model`) answers exactly the gate
  questions that need judgment: is this spec complete, are these tests enough,
  do we ship.

Properties every implementation must have, because the whole design leans on
them:

1. **Row-per-decision in the trace.** A decision is a span with the question,
   the answer, the evidence it cited, the tier that answered, and the cost. The
   waterfall in `Trace.tsx` already renders spans; this makes "why did it do
   that" answerable after the fact.
2. **Idempotent and cacheable.** Answer is keyed by a hash of the question plus
   its inputs, so a retry reuses it instead of re-asking. (Endo's
   "decode-once, the caller decides whether to repair" rule.)
3. **Escalates, never silently guesses.** Low confidence moves one rung up the
   existing ladder (effort, then tier, then human). The `EscalationConfig` knobs
   are the same ones workers use.
4. **The human can always override**, and the override is recorded next to the
   machine answer. A gate the human passes by hand is not a failure; a gate
   nobody can see is.
5. **Deterministic fallback.** If no model is reachable (offline, no key), every
   gate falls back to a defined rule, and says so. A pipeline that deadlocks
   because a local model is down is worse than one that asks.

Cost ceiling: gate decisions default to `Rules` then `SmallModel`; a
`StrongModel` call at a gate needs a reason recorded in the trace. The budget
accounting is `max_project_usd` per project and a per-phase sub-budget.

### The five gates and the markdown chain

Every phase ends in an artifact and a gate. The artifact is markdown because it
is readable by the human, diffable by git, and quotable by the next phase's
prompt. One writer per file, one gate per file.

| # | Phase | Artifact (in the project folder) | Written by | Gate question |
|---|---|---|---|---|
| 1 | Specification | `SPEC.md` | Plan mode (you answer, the model composes) | Can this be built without guessing? |
| 2 | Execution | `PLAN.md` + typed tickets | Main agent | Does the plan cover the spec, one ticket per claim? |
| 3 | Testing | `TEST-PLAN.md` | Decision model + tester agent, **from the spec** | Does every acceptance criterion have a test? |
| 4 | Testing | `TEST-REPORT.md` | Test runner + decision model | Is the evidence real, and is it enough? |
| 5 | Deployment | `RELEASE.md` | Decision model + human | Is everything green, and can we undo it? |

Files live in the project folder so they travel with the work and show in the
docs canvas. `SPEC.md`, `PLAN.md`, `TEST-PLAN.md` are committed; `TEST-REPORT.md`
and `RELEASE.md` are per-run and may be gitignored if they get noisy.

Each file has a fixed skeleton. `SPEC.md`:

```
# <name>
## Goal              one paragraph, no "and also"
## Non-goals         what this explicitly will not do
## Users and when    who hits it, in what moment
## Acceptance        numbered, each observable by a command or a click
## Interfaces        data shapes, commands, files touched
## Constraints       platform, budget, deadline, things we cannot change
## Decisions         every question asked, the answer, and the date
## Open questions    must be empty for gate 1 to pass
```

The `Decisions` section is the point. It is the interrogator's transcript, so a
later reader sees what was settled and what was assumed. Plan mode appends to
it; the gate refuses while `Open questions` is non-empty.

### Phase state

A typed phase lives on the project, not in a prompt: `phase.rs`, with
`Phase::Specify | Plan | Execute | Test | Ship | Done`, `Gate { phase, passed,
by: Rules|Model|Human, at, artifact, note }`. It is persisted with the project
state, shown in the UI (a five-dot strip in the Code header), and every harness
action checks it. That is what stops last week's "just make reasonable
decisions" from leaking back in: outside `Plan`, the main agent may not start
tickets.

## Phase 1: Specification, and the Plan/Fast switch in the composer

**What you get.** Two modes in the chat box at the bottom, next to the model
chip. **Fast** is today's behaviour. **Plan** turns the send into an interview:
the model asks, in rounds, until `SPEC.md` has no open questions, then offers to
start execution. The mode is visible on the thread and on the project's phase.

**Why a mode, not a setting.** The grill already exists as
`WorkflowConfig.grill` and is already on, but it is invisible and it competes
with the instruction at `harness.rs:959` that tells the main agent not to
interview. The switch makes it a thing you choose per conversation, and the
first change in this phase is deleting that instruction and making the plan-mode
prompt replace it.

**How the interview works.** `grilling` is the skill (vendored already). Plan
mode sends the reply through a loop:

1. The model restates the ask in one line and names what it cannot decide.
2. It asks at most three questions, each with a recommended answer and the cost
   of getting it wrong. No open-ended "tell me more".
3. Your answers are appended to `SPEC.md` under `Decisions`.
4. The decision model scores completeness against the rubric (goal, non-goals,
   acceptance, interfaces, constraints, failure modes). Below the threshold it
   asks the next round; at the threshold it drafts the file.
5. The gate passes only when the file has zero open questions and every
   acceptance line is observable.

**Rules the interview must follow** (these are the "don't leave it to the LLM"
part, and they belong in the plan-mode prompt and in the gate rubrics):

- Every feature must name the problem it solves. A feature with no problem in
  the spec is flagged as scope, not silently built.
- Every requirement must be falsifiable. "Fast" is not a requirement; "list
  renders under 100 ms at 500 threads" is.
- Any word the codebase already names differently gets the codebase's word
  (the `domain-modeling` / `grill-with-docs` discipline, and GLOSSARY.md).
- Nothing is decided "as needed later" unless it is written as an open question.
- When you cannot answer, the model picks the default, writes it down as an
  assumption, and says what would change it.

**Skills to vendor** (all MIT, from `mattpocock/skills`): `to-spec`,
`wayfinder` (for work too large for one session), `domain-modeling`,
`codebase-design`, `implement-spec`, `prototype`, `research`. `grilling`,
`to-tickets`, `triage`, `tdd`, `code-review`, `diagnosing-bugs` are already
there; check them against upstream while adding the rest.

**Fix first: the skill parser.** `skills.rs:207` reads the first line starting
`description:` and takes the rest of that line. Upstream skills write
`description: >` with the text on following indented lines, so those skills
resolve to `>`. The vendored ponytail skills hit this today; the six new ones
would too. Parse the YAML block properly (or take the indented continuation),
and make the test assert a real sentence, not "non-empty". This is a bug fix,
not a feature, and it gates the whole phase.

**Data and commands.** `Spec` struct in core (goal, non_goals, acceptance,
interfaces, constraints, decisions, open_questions, versioned). Commands:
`spec_get`, `spec_append_decision`, `spec_set_open`, `spec_gate`. The chat send
carries the mode: `chat_send { mode: "plan" | "fast" }`, persisted on the
thread so reopening shows where it stood.

**What proves it.** A spec for a real small change, written in the app, whose
`SPEC.md` builds and whose gate refused twice before passing. That is an eval in
`crates/core/tests/` (fake model, real gate) plus a UI harness check for the
switch.

## Phase 2: Execution

**What you get.** Plan mode hands off to execution. Tickets come out of the
spec, workers run them in worktrees, the decision model picks who runs what and
when to escalate, and the swarm runs the ready frontier at once instead of one
at a time.

**The pstack shape, mapped.** pstack's playbooks become a `Playbook` enum on the
ticket instead of a skill the model has to remember: `Feature`, `Refactor`,
`Bugfix`, `Perf`, `Eval`, `VisualParity`, `Migration`. Each has a step list, a
required artifact set, and the commands that verify it. `poteto-mode`'s
non-negotiables become the worker's system prompt (they are already close to
`ponytail`). `/no-comments` becomes a `comment-sicko` reviewer role, and we now
have the audit (`docs/COMMENT-PRUNE-2026-10-17.md`) to prove the role finds real
things. `/swarm` becomes the ready-frontier scheduler.

**Roles and routing.** Extend `AdvisorConfig` into one table, one line per role,
because "big model for judgment, small for looking" is the actual policy:

| Role | Job | Default tier |
|---|---|---|
| decider | gate questions, playbook choice | small, escalate to strong |
| planner | main agent: spec → plan → tickets | strongest available |
| worker | one ticket in one worktree | medium, climbs on failure |
| reviewer | diff against the spec; Standards + Spec axes | strong, second opinion |
| scout | find files, classify, summarize | local tiny model if one is up |
| tester | write and run tests from the spec | medium |
| adversary | comment audit, adversarial review | strong, different family |

Routing is `Rules` first: a role maps to a tier, a tier ranks usable routes
(`prefs.default_route`, `harnesses`, accounts health, local model liveness), and
the cheapest that satisfies the tier wins. `EscalationConfig` already climbs;
this phase makes it climb *across accounts too*, which is where
`accounts.rs` becomes load-bearing. Fix the two known holes first: worker spend
must reach `total_cost_usd`, and a Codex worker's thread id must be captured so
a resumed worker is not cold (HANDOFF P1-8).

**Swarm.** Ready-frontier concurrency: tickets whose `blocked_by` are done run at
once, up to `max_subagents`, each in its own worktree, each with a budget. The
decision model chooses the fan-out width from the spec size and the budget, not
from a constant. A ticket that fails its check twice parks and asks, rather than
spending the ceiling. This is `implement-spec`'s shape with our tickets.

**Endo's mechanics, taken in this order** (each with the reason it earns its
place now):

1. **Durable run queue: admission separate from execution.** Today a send is a
   spawn; a restart mid-run is handled ad hoc. A persisted run row with a state
   machine, serial within a session and concurrent across sessions, is what
   makes the swarm observable and restartable. Do not retry orphaned runs.
2. **Snapshot-on-connect, committed-state-only events.** We ship the whole
   `ProjectState` over IPC on every event (HANDOFF P1-5). Fix it before the phase
   adds more events, not after.
3. **Typed tool errors end to end.** A failure becomes a typed field on the
   record, rendered from data, not parsed out of prose. This is what lets the
   decision model see *why* a step failed.
4. **Permissions rule algebra** (deny > ask > allow, last match wins, saved
   grants cannot bypass a hard denial). We have no sandbox, so this is not
   safety; it is a predictable contract for what an agent may touch, which the
   gates need.
5. **Dig In** and **turn-paginated transcripts.** Only after the above; they are
   UX, not pipeline.

**Gate 2.** The plan is approved when every `SPEC.md` acceptance line maps to at
least one ticket, every ticket declares its blocking edges and its check
command, and no ticket is larger than one session. The decision model checks the
mapping and names the gaps; the human approves.

## Phase 3: Testing

**What you get.** Tests are planned from the spec before they are written, run
by a scheduler that knows what changed, and judged by the decision model. Two
new kinds of evidence enter the repo: mutation score and an eval suite that
tests the *behaviour* of the pipeline itself.

**The test plan comes from the spec, not the code.** `TEST-PLAN.md` is written
before execution ends: one row per acceptance criterion, the cheapest test that
would prove it (unit, integration, e2e, manual), and what would make it flake.
Writing it from the spec is the whole point; writing it from the diff only tests
what was built, never what was promised.

**The ladder of evidence**, cheapest first, each mapped to what already exists:

1. **Unit** in the crate, `cargo test --workspace --exclude backspace`.
2. **Integration** in `crates/core/tests/` (already seven suites, several with
   fake CLIs and stand-in servers, so they need no network or key).
3. **E2E on the real UI**: `tools/uiharness/` plus the `tools/verify/*.js`
   scripts, which drive the real face. Formalise them as a named suite with a
   manifest so "run the e2e" is one command with a countable result.
4. **Mutation testing**, new: `cargo-mutants`, scoped with `--in-diff` in CI so a
   pull request only pays for mutants it touched, full run nightly. A surviving
   mutant in changed code fails the gate. Start on `crates/core` and
   `crates/runner`; do not attempt the UI yet.
5. **Pipeline evals**, new: fake models and fake harnesses that drive a spec
   through all five gates end to end, failing exactly where they should. Without
   these, the four phases are untestable and every gate is vibes.

**The decision model here has three jobs.** It picks the tests affected by a
diff (small model over the import graph and the test manifest, not over the
whole repo). It writes `TEST-PLAN.md` rows for criteria the code does not yet
cover. And it judges gate 4 with a rubric: every acceptance criterion has
passing evidence, the evidence is behavioural rather than structural, mutation
in the diff is at threshold, nothing is skipped without a recorded reason, and
the e2e run produced artifacts. It may not judge its own test-writing; a
different family reviews the tests it wrote (the two-axis `code-review` shape).

**Gate 3** is the plan (coverage of the spec). **Gate 4** is the report
(evidence, mutation score, flakes). `TEST-REPORT.md` records, per criterion: the
test name, the command, the last result, and the trace span.

**Deployment-adjacent checks move here**, not later: the Windows and Linux build
jobs belong in phase 3 so a platform break is caught when it lands, not at
release.

## Phase 4: Deployment

**What you get.** A release is not a push. It is a go/no-go decision over the
evidence the earlier phases produced, an artifact set that can be reinstalled and
verified, and a way back.

**First, fix what is broken.** `.github/workflows/macos.yml` has never been seen
to pass. A deployment phase on top of a red workflow is decoration. Fix the build
(tauri CLI feature checks, icon generation, `bundle.active` override), add a
Windows job (the owner runs Windows) and an Ubuntu job (headless, for the core),
and make all three required.

**Endo's release engineering, adopted wholesale** because it is cheap and right:
`SHA256SUMS` per artifact, a `SOURCE` file naming the exact commit, one version
per release with published versions immutable (a correction ships a higher
version, never a rebuild), and `changelog.json` surfaced in Settings →
Changelog, where the newest entry must match the package version. The current
rolling `mac-latest` pre-release is replaced by versioned releases; keep a
`latest` alias for convenience.

**The decision model's last job** is the one place it must be most conservative.
It does not decide to ship; it decides whether the evidence says a human may
ship, and it writes the case:

- all five reports present and green, with the commit hash they describe;
- the changelog entry matches the version and the diff;
- blast radius classified (one-way or two-way door, and what it touches: data
  format, prefs schema, accounts, wire protocol);
- rollback is written down and has been exercised once;
- every open question from any gate is either closed or explicitly deferred with
  a named owner.

Confidence below the bar means "not yet, here is what is missing", never a
silent pass. The human's approval is the only thing that starts a release, and
the artifact records both answers.

**Rollback.** Keep the previous installer and the previous data-format reader for
one release. A prefs or thread schema change ships with a downgrade path, or it
does not ship. That rule is cheap now and priceless after the first bad release.

## Build order

Each milestone ends in something visible or measurable. Do not start the next
until the previous one's proof exists.

| # | Milestone | Proof |
|---|---|---|
| 1 | Foundations: skill parser fixed; missing mattpocock skills vendored; `phase.rs` state + gate records; `Decider` interface with a `Rules` implementation and trace spans | Unit tests for the parser and the decider; a gate record visible on the project |
| 2 | Plan/Fast in the composer; the grilling loop; `SPEC.md` written and gated; the "do not interview" instruction gone | A spec for a real small change, whose gate refused at least once before passing |
| 3 | Playbooks per ticket; role→tier table; account/cost truth fixed; gate 2 | A spec driven to an approved plan: every acceptance line mapped, every ticket with a check |
| 4 | Ready-frontier swarm with budgets; durable run queue | Two tickets running at once in separate worktrees, one failing and parking without spending the ceiling |
| 5 | `TEST-PLAN.md` from the spec; named e2e suite; `cargo-mutants` in CI with `--in-diff`; Windows and Ubuntu jobs | A red build caused by a surviving mutant, then green |
| 6 | Decision-model gates 3 and 4; `TEST-REPORT.md`; pipeline evals with fake models | The eval suite drives a spec through gates 1–4 and fails exactly where expected |
| 7 | Versioned releases, checksums, `SOURCE`, changelog, rollback exercise; gate 5 | One real release with its `RELEASE.md`, installable, and a downgrade that works |
| 8 | Endo hardening as it becomes the bottleneck: snapshot-on-connect, typed tool errors, permission rules, dig-in | The IPC payload stops carrying the whole project state; a tool failure renders from data |

Milestones 1–2 are the ones that make the rest possible, and they are small.
Milestones 5–7 are where the repo is thinnest, so expect them to take the
longest.

## Non-goals, for this pass

- No new crates. `crates/core`, `crates/runner` and `crates/tauri-app` carry it.
- No work on the GPUI shell (`crates/app`), Cloud, or the phone link.
- No pane stacks (parked on `pane-stacks`) and no Team chat (hidden, not
  deleted). They come back after the pipeline is real.
- No new model providers. The decision model uses routes that already exist.
- The decision model never becomes the only gate for something destructive.
  Human approval stays in front of anything that publishes or deletes.

## Open questions for you

These are the ones I would answer before milestone 2, because they change the
shape rather than the details:

1. **Where do the artifacts live?** I propose the project folder (`SPEC.md`,
   `PLAN.md`, `TEST-PLAN.md` at the root, next to each other), committed. The
   alternative is a `.workbench/` folder, which keeps the repo root clean but
   hides the artifacts from the docs canvas.
2. **Does Plan mode replace the grill or wrap it?** Today `grill` runs inside the
   harness. I propose the composer switch chooses, and the harness grill stays
   only for Code's "start from a goal" path.
3. **Is the decision model allowed to spend money on gates?** My default: rules
   and small models free, strong-model gate calls capped per project (roughly
   the price of one worker turn each) and always visible in the trace.
4. **Mutation testing on the UI?** I propose Rust only for now. The TS surface
   has no coverage today, so mutation there would just measure nothing.
