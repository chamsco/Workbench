# Phase 0 audit: Cauldron vs the Platform / Code / Decision Engine PRD

Audited 2026-11-17 on `claude/project-thread-gg3ogp` at `c7caea5` (the master line; PR #47 / M5 merged into it by Charles at 08:01Z today). `main` (0e2518a) is behind and is NOT an ancestor of master. Test count: 839 `#[test]` functions in the workspace; the suite was not re-run for this audit (last recorded run: M5 green `b8fb7a2`, 809 pass x3).

Status labels (PRD §8): **IT** implemented and tested · **P** partial · **I** interface defined, impl pending · **N** not implemented · **B** blocked externally.

## 1. What the repo actually is

One Rust workspace, no platform/app boundary.

| Crate | Lines | Role today | PRD home |
|---|---|---|---|
| `crates/core` (backspace-core) | 51k | Everything: harness, tickets, git, gates, models, memory, prefs, remote, cloud, usage, update | Must be split |
| `crates/runner` | 1.3k | One turn on any CLI / API / A2A agent as an event stream | Platform model runtime (provider side) |
| `crates/colony` | 1.5k | Durable per-agent workspaces | Code (or platform isolation later) |
| `crates/chrome` | 1.9k | Tight Tiles tokens + layout solver | Platform UI kit |
| `crates/app` (GPUI) | 14.6k | New shell; old palette, never verified on a display | Platform shell + Code views |
| `crates/cli` | 0.5k | `backspace-cli` | Code CLI |
| `legacy/tauri-app` | n/a | Retired shell, but the only download with working screens (`windows-tauri`) | Retire after parity |

Dependency direction: app/cli -> core -> {runner, colony}. Because "platform" services (model_server, provider, memory, prefs, update) live in the same crate as the Code harness, **PRD rule 2.2.1 is violated by construction**: there is nothing separate to keep from depending on Code.

## 2. Capability map

### Platform
| Capability | Where | Status | Note |
|---|---|---|---|
| App runtime, manifests, permissions | `core/apps.rs` (`backspace-app.json`, `permissions: Vec<String>`, `has()` check) | P | Manifest + permission list exist for "apps" (downloadable views). No install/update/rollback, no isolation, no enforcement outside the view bridge. Code itself is not an app. |
| App Store | `apps/prompt-lab`, `apps/sdk.js` | P | One sample app; no store. |
| Model provider abstraction | `core/provider.rs` (Anthropic + OpenAI wire formats), `runner` | IT | Neutral conversation shape, tested. |
| Local inference / small models | `core/model_server.rs` | P | Picks a small local model for small tasks. No ONNX anywhere. |
| **Model server :7422 + Brainpowr engine (llama.cpp manager, LRU, unload, pinning)** | `model_gateway.rs`, `crates/engine` | **N on master** | Exist ONLY on unmerged branch `s1gyfk` (PR #27 line). Never merged into master. This is the closest thing to the PRD's shared model runtime. |
| Model registry / lifecycle / RAM-VRAM / scheduling between apps | — | N | Engine on s1gyfk has LRU + idle unload only. |
| Policy gateway, allowlists, budgets | `core/gateway.rs`, `policy_guard.rs` | IT | M1. Per-agent, not per-app. |
| Cost / usage tracking | `metrics.rs`, `usage.rs`, `cloud.rs` | P | cloud.rs spend not in the ledger (M2 gap). |
| Library (knowledge, memories, skills) | `memory.rs`, `memory_graph.rs` ("Library graph"), `skills.rs`, `folders.rs` | P | Scopes global / project / agent exist. No per-app permissions, no semantic search, no subscriptions, no versioning beyond memory commits. |
| Event bus | harness inbox `mpsc` (harness.rs:96, 592), fleet change channel | N | Ad-hoc channels inside Code; no bus, no typed events. |
| Notifications (inbox, desktop, prefs, quiet hours) | — | N | Nothing in core or GPUI shell. |
| Storage namespaces, secrets | `prefs.rs` (one JSON), `accounts.rs` (CLI profiles), `config.rs` | P | No per-app namespace; secrets are env/config. |
| Remote machines / companion | `remote.rs`, `fleet.rs`, `companion.rs` | IT | Platform-level by nature. |
| Observability / traces | `trace.rs` (OTel-shaped), `metrics.rs` | P | Per agent; not cross-app. |
| Update flow | `update.rs` | IT | Against `mac-latest` releases. |

### Code app
| Capability | Where | Status | Note |
|---|---|---|---|
| Agent orchestration, tickets, worktrees | `harness.rs` (5.3k), `ticket.rs`, `git.rs`, `delegate.rs`, `board.rs`, `relay.rs` | IT | M0-M5. |
| Intake (one-shot vs ask) | `intake.rs`, `decision.rs` | IT | Off by default until the router eval runs. |
| **Plan gate** (grill-me, typed checklist, spec hash) | `planning.rs`, `spec.rs` | IT | M4. Output is `spec.md` + checklist, not `PLAN.md`; confirmed/assumed/unresolved split partly present. |
| **Execute** + escalation ladder + failure digest | `ladder.rs`, `router.rs`, `rampart.rs` §7.4 | P | Cheapest rung first, escalate on failure, one frontier call with digest exists for sealed work. No `EXECUTE.md`. Bash unsandboxed (M6). Coding-CLI one-shot refused until M6. |
| **Verify** | `rampart.rs` (sealed tests, 3x flake, mutation score), stage review model | P | Tests-first and anti-weakening enforced. No plan-vs-diff comparison, no CI/security inputs, no unresolved-issues report. |
| **Ship** | `stages.rs` Ship = "only a person" | N | No PR creation (M7), no delivery record, no return-to-stage. |
| **Learn** | `memory_learn.rs`, `leave_note`, Commit stage | P | Today Learn IS the last stage ("Commit: the Librarian files what agents learned"). No provenance/confidence/scope schema per PRD §4.6. |
| **Librarian** | Commit stage in `stages.rs`, `memory::propose/drain`, `folders.rs` | P | Proposes and writes memory commits; no dedup/contradiction checks. |
| **Judge** | stage review model (`stages.rs` `Review::Block`) | P | Can only block a gate; no cross-gate outcome evaluation. |
| Hands (browser/desktop) | `hands/` | IT | 1 known red: `hands/decide.rs` model call outside gateway (policy_matrix). |
| MCP server for CLIs | `mcp.rs` | IT | |
| Chat, agents, projects | `chat.rs`, `agents.rs`, `projects.rs` | IT | Arguably platform-level (Chat is not Code). |

### Decision Engine
| Item | Status | Evidence |
|---|---|---|
| Separate repository | **N** | None of the 39 repos this account can reach is a decision engine. |
| Typed decisions + calibrated output | IT | `decision.rs` (252 lines) depends only on anyhow/serde: extractable today. |
| Cost ladder / escalation | IT | `ladder.rs` depends on `config::ModelSpec` + decision; small adapter needed. |
| Eval harness | IT | `eval.rs`, `backspace-cli eval`. |
| Decision ledger / replay vs rerun | P | `phase.rs` gate records + `metrics.jsonl`; no replay. |
| Policy evaluation | IT but Cauldron-bound | `gateway.rs` couples policy to harness config. |

## 3. Conflicts needing Charles's call

1. **Review cadence**: resolved by Charles at 08:02Z ("no mandatory review per gate M1-M13, I test the finished result"). PLAN.md §12 still says stop after M2/M5/M8/M10; PLAN needs an amendment line.
2. **Ship replaces Learn**: today the last stage is Commit (Librarian). PRD order Plan/Execute/Verify/Ship + async Learn means renaming Commit -> Learn, adding Verify as a stage, and making Ship real (PR/merge). PLAN.md §5.7 says "human approval mandatory for merge"; the PRD's autonomy policy agrees for irreversible actions, so merge stays human, PR creation goes autonomous.
3. **`PLAN.md` vs `spec.md`**: M4 emits a hashed `spec.md`. Renaming to PLAN.md collides with the repo's own PLAN.md. Recommend keep `spec.md`.
4. **Decision Engine as separate repo**: the PRD requires it; nothing exists. Recommend a `crates/decision` crate in this workspace first (independently buildable, no core dep), split to its own repo only when a second consumer exists.
5. **Model runtime is off master**: the :7422 server and Brainpowr engine live only on `s1gyfk`. Merging that is the cheapest route to PRD §3.2.
6. **Which shell ships**: GPUI is approved (Tight Tiles) but has never been seen on a display and Home/Memory/Agents/Apps are unreachable in it; Tauri works but is retired.
7. **Unsandboxed Bash (M6)**: an autonomous Execute on his machine runs agent Bash with his user rights. PRD autonomy does not override that.

## 4. Migration plan (keeps M1-M5)

No rewrite. Move code behind seams in the same workspace; behaviour and tests stay green at each step.

1. **Code loop first (Phase 4 before 1-3)**, because the deliverable is a working app: add a Verify stage (spec checklist vs diff + sealed suite + test result), make Ship open a PR (M7) and write a delivery record, rename Commit -> Learn and run it after Ship without blocking. End-to-end test: request -> plan -> execute -> verify -> PR.
2. **Extract `crates/decision`** from decision.rs + ladder.rs + eval.rs with a `ModelSpec` trait adapter. Zero behaviour change.
3. **Merge the model server + engine from `s1gyfk`** into a `crates/platform-models` crate; core calls it through `provider`.
4. **Carve `crates/platform`**: prefs, update, remote/fleet/companion, accounts, usage, memory+skills (Library), plus a minimal typed event bus replacing the harness inbox for user-facing events, and an in-app notification inbox. Core (renamed `code`) depends on it, never the reverse; enforce with a CI check.
5. **Manifest**: extend `apps.rs` manifest to describe Code itself; enforce permissions at the platform service boundary.
6. **Last**: App Store, multi-app scheduler, ONNX, VRAM monitoring, quiet hours.
