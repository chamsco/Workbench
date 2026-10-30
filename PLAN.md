# Cauldron: PLAN.md

Cauldron (also called Workbench) is an agentic development environment: an ADE plus a new orchestrator that takes a task from a chat box to a merged, verified PR with minimal human involvement and minimal frontier-model spend.

Cauldron is a separate product from Meadow. Meadow is for everyday people, Cauldron is for development, they serve different users and have no connection. Nothing in this plan builds, depends on, or exposes Meadow features.

---

## 1. Goal

A working product where every feature is verified by automated tests, and where auto-testing and regression checking of a project run inside the app, in both modes:

- **ADE mode**: agents drive, the human supervises.
- **IDE mode**: the human drives, agents assist.

The same test pipeline serves both modes.

## 2. Non-goals

- Anything Meadow related.
- Features not listed in section 5. Do not invent features.
- Fully unattended merging on day one. Human approval stays until the router eval (section 10) and the memory eval (section 8) prove reliable.

## 3. Design philosophy

Vibe: **plugins, skills, token minimizing, frontier models called as little as possible.**

### 3.1 The cost ladder

Every step starts at the cheapest rung that can do the job and moves up only on measured failure, never by default.

1. Deterministic tools: grep, tests, linters, type checkers, scripts. Zero tokens.
2. Decision model (local): classification, scoring, gating.
3. Small local models: bounded executor jobs.
4. Mid-tier models.
5. Frontier model: orchestration of long tasks, and escalation after the limit is hit (section 7.4).

### 3.2 Plugins and skills

- **Skills** are loaded on demand (name and one-line description always visible, full instructions only when triggered), so they cost tokens only when used.
- **Plugins** add capabilities (runners, integrations, review tools) through a stable plugin API instead of being baked into prompts. Third-party plugins are audited before use (supply chain risk).
- Skills and plugins are per-project and allowlisted like models.

### 3.3 Token-saving mechanics

- Hand off between stages with files (spec.md, tests.md, failure digests), never with chat history.
- Send diffs and symbol-level context, not whole files.
- Prompt caching for stable prefixes (spec, skills, project memory).
- Summarize at stage boundaries and drop raw logs.
- Escalate with a compact failure digest (failing test names, error signatures, last diff), not the full transcript.
- Hard per-task token and cost budgets enforced at the gateway.

### 3.4 Metrics (reported on every run)

| Metric | Why |
|---|---|
| Frontier calls per merged PR | The headline number to drive down |
| Total tokens and cost per merged PR | Cheap retries can cost more than one frontier call, so total cost is the real target |
| Cost-to-green | Cost including all retries until Gate A passes |
| Escalation rate and escalation success rate | Shows whether the ladder is tuned |
| Human tokens per merged PR | Tokens the user types or reads, the quantity to minimize |

Escalation policy must be cost-aware: if the expected cost of another cheap retry exceeds a frontier call, escalate sooner.

## 4. Pain points this solves

1. Frontier models are expensive and are used for trivial work.
2. Spawning small local agents on local infra is manual today.
3. No single flow from "I have a task" to "it is merged and verified".
4. Agent output is trusted without independent verification.
5. Agents forget lessons between tasks and projects.

## 5. Feature list

The complete feature inventory, set 2026-10-30. Each entry gives a short description and its key capabilities. Sections 3 and 6–12 are the authoritative specifications and acceptance criteria. This section indexes them and does not restate them.

### 5.1 Conversational intake and classification

A chat box starts a project or task. Each task is classified before any work begins.

- Chat box intake that starts a project or task.
- Triage by a local decision model into one-shot, long or research, with a confidence score. Below the threshold, Cauldron asks the user (section 6).
- Continuous chat with threads (scope confirmed).

### 5.2 Specification and planning

Long tasks are planned until a typed checklist passes. Then they are handed off as files.

- Planning mode: a grill-me style interview that ends on the typed checklist and a human "yes" (section 6).
- A "before" baseline captured per task type (section 6).
- Markdown handoff files (spec.md, tests.md) into execution mode (section 3.3).

### 5.3 Intelligent routing and cost-aware orchestration

Every step runs on the cheapest rung that is expected to succeed. The objective is the lowest total cost-to-green.

- The cost ladder (section 3.1) and a swappable decision-model interface used for triage, checklist scoring, readiness and memory decisions (section 10).
- The orchestrator uses a frontier model only when needed and spawns small or local executors (sections 3.1, 10).
- Escalation triggers, cost-aware escalation, and one frontier escalation with a failure digest, then the human (section 7.4).
- Token-saving mechanics (section 3.3) and the router eval from real traces (section 10).

### 5.4 Policy, permissions and budgets

One gateway in code decides what may run and how much it may spend.

- Per-project allowlists for models, skills and plugins, with separate model lists for orchestration and execution (section 10).
- Hard per-task token and cost budgets enforced at the gateway (section 3.3).
- No prompt or memory entry can bypass the gateway (sections 8.3, 10).

### 5.5 Build and execution infrastructure

Where executors run, and how that is isolated.

- Sandboxed worktrees plus containers for execution and test runs (section 11).
- Local executors limited to bounded jobs (section 10).
- herdr adapter for pane and worktree supervision: version pinned, swappable, never a gate (section 9).
- Cloud and local execution (scope confirmed).

### 5.6 Independent testing and verification

Verification that does not trust the agent's own output.

- Two orchestrators at planning time: Builder and Rampart, with sealed, hash-locked and validated tests (section 7).
- In-app auto-testing and regression in ADE and IDE modes (section 11).
- Recorded-trace replay with stubbed models and a frozen router (section 11).

### 5.7 Review, gates and delivery

From a passing build to a merged, verified change.

- Automatic PR creation.
- Gate A (deterministic), Gate B (independent reviewer and readiness check) with the send-back loop, CI/CD, and Gate C (before/after on preview) (section 6).
- Human approval before merge until the evals prove reliable (section 2).

### 5.8 Persistent memory and context

What agents and the user carry between tasks and projects.

- Master and per-project memory with a memory decision agent and the hard rules (section 8).
- Shared personal context (scope confirmed), subject to section 8.
- Automatic capture from chat may only create proposals. It never writes global memory without explicit human approval.

### 5.9 Skills and plugins

Capabilities added on demand rather than baked into prompts.

- Skills loaded on demand; plugins through a stable plugin API (section 3.2).
- Plugins are allowlisted, audited and permission-controlled. Arbitrary untrusted third-party plugins are not installed until signing or an equivalent integrity check exists (signing is on the roadmap, section 13).

### 5.10 Developer experience and observability

How people supervise, drive and measure the system.

- ADE mode (agents drive) and IDE mode (the human drives) on the same pipeline, with test results streamed into the thread (sections 1, 11).
- The section 3.4 metrics, reported on every run.
- Hardening: audit log, observability, failure-mode tests and a metrics dashboard (section 12, M13).

### 5.11 Existing capabilities retained / outside current implementation scope

These exist in the repository today and stay as they are. They are **not** a delivery commitment of this plan, and they are not changed unless they are explicitly added to an entry above.

- Routines, delegate board, apps, voice, iOS companion, colony, A2A agents.
- Internal "Backspace" naming in code and configuration.

## 6. Lifecycle

```
Intake (chat box)
  -> Triage (decision model: one-shot | long | research, + confidence)
       low confidence -> ask the user
  -> research  : report only, no PR
  -> one-shot  : lightweight spec -> Execution
  -> long      : Grill-me interview -> checklist passes -> spec.md
                 -> spawn Builder + Rampart
                 -> Rampart writes and seals tests -> Execution
Execution (Builder: small local executors, sandboxed worktrees)
  -> pass sealed tests?  no -> retry (cheap rungs)
       limit hit (7.4) -> ONE frontier escalation with failure digest -> human if still failing
  -> PR created
  -> Gate A (deterministic): sealed tests, lint, types, before/after vs baseline
  -> Gate B: independent reviewer + decision model readiness score
       fail -> back to Execution with failures attached (max 3 loops, then human)
  -> CI/CD
  -> Gate C: before/after in preview
  -> merge (human approval until trusted)
  -> memory proposals submitted (section 8)
```

### Gate rules

- **Gate A is deterministic and cannot be overridden by any model.** Sealed tests, lint, type checks, and the before/after comparison against the baseline captured at planning time.
- **Gate B uses a different model than the one that wrote the code.** A writer judging its own work is a rubber stamp. The decision model only weighs what the deterministic gate cannot (diff size, scope creep, reviewer findings).
- **Gate C** repeats before/after on the preview deployment.
- **Loop cap**: 3 send-backs, then escalate to a human. No uncapped self-correction.

### Planning stage (grill-me style)

The interview ends when a typed checklist passes, not when the model feels done:

- Goal stated
- Acceptance criteria present
- One test per acceptance criterion
- Non-goals stated
- Constraints stated
- Dependencies named
- "Before" baseline captured

A decision model can score this checklist. It cannot detect unknown unknowns, so a human "yes" is required until measured data shows planned tasks rarely fail later.

### The "before" baseline

| Task type | Baseline |
|---|---|
| Bug fix | A failing test that reproduces the bug |
| Feature | Acceptance tests that fail today |
| UI change | Screenshots of the current state |
| Refactor | Existing test suite plus performance numbers |
| Research | None. No PR. Output is a report. |

## 7. Adversarial testing: Builder and Rampart

At planning time, two orchestrators are spawned with separated roles:

- **Builder** executes the task.
- **Rampart** writes the hard tests (the ramparts) that Builder must pass.

### 7.1 Separation rules

- Rampart sees spec.md and the baseline, never Builder's code or plans.
- Builder never sees how Rampart derived its tests, only the tests and their failures.
- Once Rampart seals the tests they are hash-locked. Builder cannot edit, skip or delete them. Gate A re-verifies the hash.
- Both start on cheap rungs of the ladder. Rampart is not allowed to be the same model instance as Builder.

### 7.2 Validating the ramparts

Tests written by an LLM can be too weak or simply wrong. Before sealing, check:

- Every sealed test **fails on the baseline** (otherwise it proves nothing).
- Run each test 3 times to catch flakiness.
- Mutation testing on the changed area to measure test strength (surviving mutants lower the rampart score).
- Coverage of every acceptance criterion in spec.md (traceability table: criterion to test).
- Human spot-check of the first N rampart suites until the score correlates with real defects.

### 7.3 Known weakness

Two small models can share blind spots, and a wrong spec yields wrong tests that pass wrong code. Mitigations: the spec checklist (section 6), property and edge-case tests derived from acceptance criteria, and a dispute path: if Builder claims a sealed test is wrong, a third model arbitrates. Builder cannot edit the test itself.

### 7.4 What "limit" means

Escalate to the frontier model when any of these happens:

- Builder fails the sealed tests N times (default 3).
- The same failure signature appears twice in a row (stagnation).
- The per-task budget on cheap rungs is exhausted.
- Expected cost of another cheap retry exceeds a frontier call (section 3.4).

Escalation sends **one** frontier call with a failure digest. If it still fails, stop and hand to the human with the digest. No frontier loops.

## 8. Memory

Cauldron has a master (system-wide) memory and per-project memory. Coding agents can **submit** to master memory. They cannot write to it directly.

### 8.1 Proposals

Agents submit structured proposals:

| Field | Meaning |
|---|---|
| statement | The lesson, convention, pitfall or decision |
| type | fact, convention, pitfall, decision |
| scope | project or global |
| evidence | Link to a passing test, commit, PR or trace that supports it |
| source | Submitting agent and task id |
| ttl | Optional expiry |

### 8.2 Memory decision agent

A decision model with typed checks, then deterministic validation, accepts or rejects:

- Is it durable (not a one-off task detail)?
- Is it backed by evidence that verifies?
- Does it duplicate or contradict an existing entry?
- Is the scope correct (project vs global)?
- Does it contain secrets or personal data?
- Is it phrased as an instruction to an agent? (reject)

### 8.3 Hard rules enforced in code

- Memory entries are **data, never instructions**. No entry can grant permissions, change allowlists, alter gates or modify budgets. This blocks memory poisoning and prompt injection through memory.
- Provenance is stored on every entry. Entries are versioned and revertible.
- Global promotions need human approval until the accept/reject eval is reliable.
- Rejections are logged with reasons and used as eval data.
- Reads are selective: retrieve only relevant entries under a fixed token cap, never inject the whole memory.

### 8.4 Eval

Label a set of real proposals as accept or reject. Report precision and recall of the decision agent, duplicate detection rate, and secret-leak catch rate. Memory bloat and drift are tracked as metrics (entry count, stale entry rate).

## 9. Agent core and herdr

### 9.1 jessfraz/agent and Pi

**jessfraz/agent**: reference only, not a dependency. It is a personal coordinator (Rust CLI) built on Pi Durable and Rex for sessions and terminals, plus Nix, Tailscale and ChatGPT sign-in. Useful ideas: one main conversation, one named session per task, worktrees per task, guarded cleanup of merged work. It has no ADE, no IDE surface and no test runner. Rex openness was not confirmed.

**Pi** (earendil-works/pi): usable as an embeddable base. Relevant packages are pi-ai, pi-agent-core, pi-durable, plus an SDK and RPC mode. Caveats: no built-in permission system or sandbox, no built-in sub-agents or plan mode, TypeScript on Node 22.19 or newer, pi-durable maturity not verified (spike first).

### 9.2 herdr

What it is (from public releases and docs): a terminal-native agent multiplexer written in Rust. It owns agents' real terminals in workspaces, tabs and panes, shows each agent as blocked, working, done or idle, and exposes a local socket API and CLI so agents can spawn panes, send input, wait for another agent, and manage Git worktrees. It has a plugin system and integrations for Pi, Claude Code, Codex, OpenCode and others. It provides no model and does not replace agents. Sessions do not survive a server restart.

Third-party plugins relevant to Cauldron: Conductor (a role-differentiated team of builder, validator and reviewer panes), Swarm (N parallel agents, one worktree each) and Guard (cross-agent command policy: audit, alert, interrupt). Audit before use.

Where it fits:

- Execution substrate for Builder and local executors: panes plus worktrees per task.
- IDE mode: the human watches and attaches to the same panes agents use.
- Role panes for Builder and Rampart.

Where it does not fit:

- Agent state (done, blocked) comes partly from terminal-screen heuristics. Treat it as a signal, never as a gate. Gates stay deterministic (section 6).
- It is pre-1.0 with frequent releases and protocol changes, so pin the version and wrap it behind an adapter so it is replaceable.
- Cauldron owns routing, memory, budgets and gates. herdr does not.

**Open item**: I could not find a release named "herdr v2". The newest stable I found was 0.8.x with a preview channel. Confirm which release or link you mean before M6.

## 10. Routing and model policy

### Decision model tier

Decision models (Liquid AI d1, TypeSafe Jev, Cloudflare Clef) take a state plus a schema of typed questions and return calibrated probabilities in one forward pass, with no text generated. They are good for classification, routing, scoring and gating. They are not orchestrators: something generative still plans, decomposes and writes code.

Used at: triage, plan-checklist scoring, readiness weighing, memory accept/reject.

Candidate notes (verify before committing):

- Clef-flash (9B) and Clef (27B): reported as open weights under Apache 2.0. The 27B model needs a real GPU.
- d1-3B: open weights under Liquid's LFM Open License. Check commercial terms. d1.1 and d1-ultra were reported as API-only.
- Jev: local availability of weights not confirmed.
- All benchmark numbers found were vendor-reported and near-identical across candidates, so choose on license, size and locality, then on your own eval.

All candidates sit behind one decision interface so they are swappable.

### Router eval (required before shipping)

Build an eval set from real Cauldron traces labeled with the correct task type. Report accuracy per type, calibration, cost versus always-frontier, and escalation rate below the confidence threshold. A router can be confidently wrong, so low confidence must escalate.

### Allowlists

- Per-project config with separate lists for models used for **orchestration** and **execution**, plus lists of allowed skills and plugins.
- Enforced in code at a single gateway. Never enforced in prompts.
- A disallowed model, skill or plugin is refused, and no prompt can bypass it.

### Executors

Small local models are weak at long tool chains. Give them bounded jobs: run tests, lint, triage failures, summarize diffs, search files. Keep code generation on a stronger rung unless the eval shows otherwise.

## 11. Test loop and regression (core product feature)

On any edit, by an agent or a human:

1. Select affected tests.
2. Run them in an isolated sandbox (worktree plus container).
3. Stream results into the thread.
4. Block merge on regression.

The same pipeline runs in ADE and IDE modes.

Regression suite layers:

- Unit tests
- End-to-end tests
- Sealed rampart suites kept as permanent regression tests after merge
- Recorded agent traces replayed with stubbed models and a frozen router

Any previously passing test that fails blocks the change.

## 12. Milestones

Each milestone ends with: full suite run, pass/fail counts reported, commit. If blocked, stop and ask. Never mock around a blocker. Write failing acceptance tests first, then implement.

| # | Milestone | Acceptance |
|---|---|---|
| M0 | Audit and spec baseline | Table of feature / status / evidence. "Works" requires a passing test. Feature list complete. |
| M1 | Gateway, allowlists, budgets | Disallowed model, skill or plugin refused. Per-task budget enforced. Injection cannot bypass. |
| M2 | Triage router, cost ladder, eval | Swappable decision interface. Eval report per section 10. Metrics from section 3.4 emitted. |
| M3 | Intake, research path, one-shot path | Research returns a report with no PR. One-shot reaches execution with a lightweight spec. |
| M4 | Planning stage | Checklist enforced. spec.md generated. Baseline captured per task type. |
| M5 | Builder and Rampart | Two separated orchestrators. Tests sealed and hash-locked. Ramparts fail on baseline, flake check, mutation score. Escalation triggers per 7.4. |
| M6 | Execution engine and herdr adapter | Sandboxed worktrees. Executors respect allowlists. herdr behind a swappable adapter, version pinned. |
| M7 | PR creation and Gate A | PR opened automatically. Deterministic gate blocks on any failing check or changed sealed tests. |
| M8 | Gate B and loop control | Different reviewer model than writer. Max 3 loops, then human. One frontier escalation max. |
| M9 | CI/CD and Gate C | Before/after on preview. Human merge approval in place. |
| M10 | Memory | Proposal flow, decision agent, hard rules, versioning, accept/reject eval. |
| M11 | Plugins and skills | On-demand skill loading, plugin API, allowlists, audit hook. |
| M12 | ADE and IDE parity | Same test loop works in both modes. Regression suite green. |
| M13 | Hardening | Audit log, observability, failure-mode tests, metrics dashboard. |

## 13. Roadmap (future tasks)

### Now
M0 to M13 above.

### Next
- Fine-tune or calibrate the decision model on Cauldron's own traces, if the chosen model's license allows.
- Auto-generate skills from successful tasks, gated by the memory decision agent.
- Cost dashboard per project and per task type, with trend lines for the section 3.4 metrics.
- Remove human merge approval for low-risk task classes once measured thresholds are met.
- Multi-repo tasks.
- Remote runners for heavier executors.

### Later
- Plugin marketplace with signed plugins and a review process.
- Shared skill registry across projects.
- Cross-project memory promotion rules.
- Replay-driven regression for router and memory decisions (re-run old traces against new model versions before upgrading).

### Explicitly not planned
- Any integration with Meadow.

## 14. Risks

| Risk | Mitigation |
|---|---|
| Decision model confidently wrong | Eval from real traces, calibration check, escalation below threshold |
| "No gap" check misses unknown unknowns | Human sign-off until measured reliable |
| Model judging its own output | Deterministic Gate A, different reviewer model at Gate B |
| Rampart tests weak or wrong | Fail-on-baseline, flake check, mutation score, traceability, human spot-check, arbitration |
| Builder games the tests | Hash-locked sealed tests, Gate A re-verifies hash, Rampart blind to Builder code |
| Before/after has no "before" | Baseline captured at planning, per task type |
| Cheap retries cost more than escalating | Cost-aware escalation policy, total cost metric |
| Memory poisoning or drift | Entries are data not instructions, evidence required, provenance, versioning, human approval for global |
| Memory bloat | Token-capped reads, TTL, staleness metric |
| herdr is pre-1.0 and protocol changes | Version pin, adapter, never use its state as a gate |
| Third-party plugins | Audit, allowlist, signing on the roadmap |
| No sandbox in agent core | Containerize execution, test isolation |
| Small local models fail long tasks | Restrict to bounded jobs, verify with eval |
| Scope creep from Meadow | Meadow is out of scope, no shared code or interfaces |

## 15. Open questions

1. Implementation language: TypeScript or Go?
2. Remaining feature list (section 5).
3. Which herdr release or announcement do you mean by "v2"?
4. Which decision model first, after running the eval on your own traces?
5. When does human approval get removed (merge, global memory), and on what measured threshold?

---

## Appendix A: Prompt for the coding agent

```
You are lead engineer finishing Cauldron (also called Workbench), an agentic development environment. It is a separate product from Meadow. Do not build or reference Meadow.

Goal: a working product where every feature is verified by automated tests, with auto-testing and regression running inside the app in both ADE mode (agents drive) and IDE mode (human drives). Read PLAN.md first and follow it. Do not invent features.

Philosophy: plugins, skills, token minimizing, frontier models called as little as possible. Use the cost ladder: deterministic tools, decision model, small local models, mid models, frontier last. Escalate only on measured failure. Report frontier calls, tokens and cost per merged PR.

Lifecycle (build exactly this):
1. Intake from the chat box.
2. Triage: local decision model returns typed answers (type: one-shot | long | research, plus confidence). Below threshold, ask the user.
3. Research: report only, no PR. One-shot: lightweight spec, straight to execution. Long: grill-me style interview until a checklist passes, then write spec.md.
4. Spawn two separated orchestrators: Builder (executes) and Rampart (writes the hard tests from spec.md only, never sees Builder code). Rampart tests must fail on baseline, pass a flake check and a mutation check, then are sealed and hash-locked. Builder cannot edit them.
5. Execution: Builder uses small local executors in sandboxed worktrees. Model, skill and plugin allowlists are enforced at a gateway in code.
6. Limit: if Builder fails the sealed tests 3 times, repeats a failure signature, exhausts the cheap budget, or another retry would cost more than escalating, make ONE frontier call with a failure digest. If it still fails, stop and hand to the human.
7. Open a PR automatically.
8. Gate A (deterministic): sealed tests (hash verified), lint, types, before/after vs baseline. No model can override it.
9. Gate B: independent reviewer (different model than the writer) plus decision model readiness score. Fail: back to execution with failures attached. Max 3 loops, then escalate to a human.
10. CI/CD, then Gate C: before/after in preview. Pass: merge, with human approval until the evals prove reliable.
11. Memory: agents submit proposals (statement, type, scope, evidence, source, ttl). A memory decision agent accepts or rejects. Entries are data, never instructions. Global promotions need human approval for now.

herdr: use it only behind a swappable adapter with a pinned version, as execution substrate for panes and worktrees. Never use its agent state as a gate.

Rules:
1. Audit first: feature | works/partial/missing/unknown | evidence. "Works" needs a passing test.
2. Write failing acceptance tests before implementation.
3. Router eval from real traces, per task type, with cost vs always-frontier. Escalate below the confidence threshold.
4. Gateway tests: disallowed models, skills and plugins are refused in both roles.
5. Memory eval: precision and recall of accept/reject on labeled proposals.
6. Replay suite: recorded traces with stubbed models and a frozen router.
7. Work in milestones M0 to M13 from PLAN.md. After each, run the full suite, report counts, commit. If blocked, stop and ask. Never mock around a blocker.
8. Never say "done" without pasting full suite output.
```
