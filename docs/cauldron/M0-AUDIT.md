# M0 audit (final for review): Cauldron against PLAN.md

Date: 2026-10-30 (revision 5; change summaries at the end). Status: **M0 not approved. M1 not started.**

Inputs: `PLAN.md` (repo root, committed in `e236259`), the requested
corrections in `Cauldron_M0_Revised_Audit.md`, and the repository as it is now.
Where those two documents disagree, PLAN.md wins. Every row below was checked
against source and tests in this pass. Nothing is carried over from the first
audit without being checked again.

## 0. Baseline

| Item | Value |
|---|---|
| Audited code | `origin/main` = `1d508e6` (PR #25). Branch `claude/project-thread-dpmr8e` = `e236259`, which adds only `PLAN.md` and this doc, so the product code is identical. |
| Not audited | Unmerged PRs #27–#37 (model server, harness relay, agent hands, shell rail). |
| Toolchain | rustc 1.97.0, cargo 1.97.0 |
| Command | `cargo test --no-fail-fast` from the repo root. This runs the workspace default members (runner, core, cli, colony), which is the same set `.github/workflows/test.yml` runs. |
| Result (rerun 2026-10-30 on this commit) | **204 passed, 0 failed, 0 ignored**, exit 0. Breakdown: colony 25 unit; core 140 unit; core integration: agents 1, bya 1, e2e 7, features 17, interrupted 2, resume 1, spec_gate 1, webhook 2; runner 3 unit + 3 integration (`tests/http.rs`); colony 1 doc-test; cli 0. Full output is in Appendix A. |
| Passing but not exercised | `agents::tests::docker_computer` reports `ok` but returns early when Docker is absent (`crates/core/src/agents.rs:991-996`). Docker is absent here (`docker info` fails), so its body did not run. |
| Excluded: not run | `crates/tauri-app` (4 `#[test]` in `src/main.rs`; not a default member; not attempted). `capsule/` (Go `agentd` + Rust `capsule/core`: 0 tests found in either). `tools/uiharness/evals` (UI evals, not part of any suite). |
| Excluded: does not build here | `crates/app` (GPUI): `cargo test --workspace` fails to link, `rust-lld: unable to find library -lxkbcommon` (seen in the first-pass run). |

Test types used below: **U** = unit test in `src/` (`cargo test -p backspace-core --lib <name>`). **I** = integration test in `crates/core/tests/*.rs` (`cargo test -p backspace-core --test <file> <name>`). **E2E** = `tests/e2e.rs`, which drives the whole harness against a scripted fake model server, a real git repo and real worktrees.

## 1. Status rules

- **works**: a passing test exercises the PLAN.md behaviour itself.
- **partial**: tested code covers part of the requirement. The row says what is proven and what is not.
- **unknown**: code exists, but no test exercises the required behaviour.
- **missing**: no implementation found, or the code does something different from what PLAN.md requires.

Every row also states its **evidence level**:

- **I = implementation evidence**: the code exists, cited by file and line.
- **B = behavioural evidence**: a passing test runs that code. The test is named in the row.
- **A = acceptance evidence**: a passing test proves the PLAN.md requirement itself.
- **—** means none of the three: nothing relevant was found.

A qualifier in brackets says what the evidence does *not* cover, for example "B (cap untested)" or "B (asserts the opposite)".

**Result: no row reaches A, so no PLAN.md requirement is "works".** Existing tests demonstrate meaningful unit, integration and end-to-end behaviour, but none currently demonstrates an entire PLAN.md requirement at its required acceptance scope.

## 1a. Optimization objective

PLAN.md §3.4 says total cost-to-green is the real target. This audit uses that as the objective everywhere: **minimize the total expected cost of reaching a verified successful result** (Gate A passed). It is not the cost of any one call, and not the number of frontier calls.

Total cost-to-green counts every metered cost on the way to green, not only inference:

- model inference at every rung, including the router and decision-model calls;
- retries, escalations and send-back loops;
- execution: container or VM time, sandbox and worktree setup, remote runners;
- tool and third-party API usage, including paid MCP tools;
- test runs, lint and type checks, mutation testing and flake re-runs;
- CI minutes and preview deployments, and gate execution (Gates A, B and C);
- human time, measured as human tokens typed or read (§3.4).

Routing and escalation decisions must therefore weigh the **probability that each rung succeeds**, not only the price of its next call. A cheap rung with a low success rate can cost more in expectation (retries, extra test and CI runs, then an escalation anyway) than a stronger rung that usually succeeds first time. Frontier calls per PR and per-call price stay as diagnostics under this objective. They are not targets.

Current state against this objective: the code records per-agent inference cost (`harness.rs:2061-2073`) and per-ticket cost in `outcomes.jsonl` (`harness.rs:2824-2846`). It does not meter execution, tool, test, CI or gate costs, and it has no per-rung success-probability estimate. Evidence: I, B for inference cost only; — for the rest.

## 1b. Milestone ownership used in this audit

| Milestone | Owns |
|---|---|
| M1 | Policy gateway, allowlists, per-task budgets |
| M2 | Decision interface, routing policy (including the planner's starting rung and the definition of "frontier"), cost ladder, router eval, §3.4 metrics emitted |
| M3 / M4 | Intake paths / planning checklist and baselines |
| M5 | Builder/Rampart, sealed tests, rampart validation |
| M6 | Execution infrastructure: sandboxed worktrees, local executors and their bounded jobs, herdr adapter |
| M7 / M9 | PR + Gate A / CI/CD + Gate C |
| M8 | Review loops and escalation control: Gate B, loop cap, §7.4 triggers, the single frontier escalation with a digest, cost-aware escalation, human handoff |
| M10 | Memory |

## 2. Audit table: section 5 features, broken down by capability

| Feature / capability | Status | Evidence | Evidence level |
|---|---|---|---|
| Chat intake starts a project | partial | E2E `flat_project_with_gates_escalation_and_merges` (`tests/e2e.rs:509`) calls `Harness::send` and the project runs to merged tickets. Not proven: any intake step that classifies the task first. | I, B |
| Intake triage returns `one-shot / long / research` | missing | `router.rs` `Decision` (`:78`) holds model, effort, source, confidence and cost, but no task type. The ticket triage agent (`prompts/triage.md`) classifies *filed tickets* as ready_for_agent / ready_for_human / needs_info / wontfix, using a generative agent rather than a decision model. | — |
| Triage confidence threshold + ask-the-user path | missing | `Decision.confidence` is the router's confidence in a model pick. `heuristic()` always returns `confidence: 0.0` (`router.rs:463`). Nothing compares it to a threshold. | I (confidence field only) |
| Decision-model interface (swappable) | partial | `RouterBackend` switches between Jev (HTTP API) and the keyword heuristic (`router.rs:276-280`). Tested: U `heuristic_scales_with_task`, U `cli_pin_routes_workers_not_the_planner`. Not proven: a typed interface covering the four uses in section 10 (triage, checklist, readiness, memory); a local decision model; any test with Jev. | I, B (Jev path untested) |
| Planning interview (grill-me) | partial | Grilling prompt `harness.rs:962`; rules gate `spec::check` (`spec.rs:43`, `SECTIONS` at `:17-18`). Tested: U `a_complete_spec_passes`, `each_rule_names_what_is_missing`, `the_skeleton_alone_is_refused_for_every_empty_part`; I `spec_gate::the_gate_refuses_twice_then_opens`. See row-level checklist in §3. | I, B |
| `spec.md` handoff | partial | The app writes `SPEC.md` (`spec.rs:14`, `file()` `:158`). U `extract_takes_the_last_spec_block_only`. Tickets are mirrored to `.backspace/tickets/*.md` (U `ticket::parses_and_renders`; E2E flat asserts 2 ticket files). Not proven: that execution reads its context from these files rather than from chat history. | I, B |
| `tests.md` handoff | missing | No code writes or reads one. | — |
| Failure digest file | missing | A failed check returns raw output, truncated to 3000 chars, into the agent's chat (`harness.rs:2447-2452`). No digest file exists. | — |
| Builder and Rampart as separate orchestrators | missing | No code. | — |
| Rampart blind to Builder code | missing | No code. | — |
| Sealed, hash-locked tests | missing | No code. | — |
| Ramparts fail on baseline / 3-run flake check / mutation score / traceability | missing | No code. (`spec::coverage` maps acceptance lines to *tickets*, not to tests. See §3.) | — |
| Cheap-first worker start | partial | `cap_start` (`harness.rs:1031-1060`) caps workers at `start_max_tier: 2`, `start_max_effort: Medium` (`config.rs:117-125`). E2E flat asserts ticket `a` started on `mock-small` (`tests/e2e.rs:557-567`). Caveat: within the cap it picks the **highest** tier ≤ 2, not the cheapest rung (`max_by` tier, `harness.rs:1041-1046`). | I, B |
| Local model for selected jobs | partial | `Router::local_pick` / `route_for` (`router.rs:131-215`). Jobs: scouts, triage, workers. I `features::a_worker_routes_to_the_local_model_only_while_it_is_listening`, `a_delegated_lookup_reaches_the_chosen_local_model`, `triage_can_run_on_the_local_model_with_no_api_key`, `local_jobs_defaults_to_scouts_and_refuses_unknown_words`. It is **opt-in**: with no local model picked, "the big model does everything, which is the default" (`router.rs:93-94`). | I, B |
| Bounded local executor jobs (tests, lint, failure triage, diff summary, file search) | partial | Scouts (lookups) and ticket triage can run locally (tests above). The "workers" job sends whole code-writing tickets to a local model, which PLAN §10 says to avoid. Not proven: lint, test runs, failure triage or diff summaries as local jobs. | I, B |
| Main orchestrator on a frontier model only when needed | **missing (code works against it)** | The planner is held to a high starting point on two axes. **Effort:** `main_min_effort = "high"` (`default-config.toml:14`) is applied to every Main decision on every router backend (`router.rs:301-303`). Under the heuristic backend, Main is raised to at least level 3 = Xhigh (`router.rs:440-442`). **Tier:** Xhigh needs tier ≥ 3 (`router.rs:445-450`), and the heuristic picks the cheapest model at or above it. In the default config the API models at tier ≥ 3 are `claude-opus-5-5` (3) and `claude-fable-5-1` (4) (`default-config.toml:119-132`). **Frontier:** the code has no frontier classification. `tier` is a 1–4 integer (`config.rs:208`), set per model by config, so whether tier ≥ 3 is a frontier model depends on the user's model list. It is not established by the code. Also: `local_pick` returns `None` for Main (`router.rs:137`); `escalate` refuses non-workers (`harness.rs:1517-1519`), so the planner never moves down. U `heuristic_scales_with_task` asserts only `main.effort >= Effort::Xhigh` (`router.rs:529-530`), not the model. | I, B (asserts high effort; frontier not established) |
| Adjacent mechanism: generic worker escalation (not a §7.4 trigger) | partial | `escalate` (`harness.rs:1516`) moves a worker one rung up on: a failed ticket check or a user rejection (E2E flat: 2 escalations, `check failed` then `rejected by the user`, ends on `mock-big`); an exhausted turn budget (`harness.rs:1985-1988`, untested); a repeated identical tool call (`harness.rs:2191-2209`, untested). It is listed for completeness. **None of these satisfies a §7.4 trigger** (rows below). | I, B |
| §7.4 trigger: Builder fails the sealed tests N times (default 3) | missing | There are no sealed tests, and no failure counter is compared with N. Adjacent: a failed ticket check escalates on *every* failure (`harness.rs:2438-2456` → `escalate`), which is not this trigger. | — |
| §7.4 trigger: same failure signature twice in a row | missing | No failure signature is computed or compared. Adjacent: the loop detector flags N identical *tool calls* (`name+input`, `harness.rs:2189-2203`, untested). Repeated tool calls are not repeated failure signatures. | — |
| §7.4 trigger: per-task budget on cheap rungs exhausted | missing | No per-rung cost budget exists. Adjacent: a generic turn budget per effort level (`harness.rs:1984-1996`, untested) and a per-agent/project dollar budget checked after spend (`harness.rs:1960-1976`). Neither is a per-rung cost budget. | — |
| §7.4 trigger: expected cost of another cheap retry exceeds a frontier call | missing | No expected-cost estimate and no success-probability estimate exist. Adjacent: `escalate` uses price only as a tiebreak when choosing the next model (`harness.rs:1538-1546`). | — |
| Exactly one frontier escalation | missing | Up to `max_steps: 3` climbs (`config.rs:122`), counting both effort raises and model raises. "Frontier" is not a concept in the code. | I, B (3 climbs, not one) |
| Escalation carries a compact failure digest | missing | The escalated agent keeps its full conversation (`c.messages`). No digest is built. | — |
| Human handoff after failed escalation | unknown | At the limit, `escalate` logs and returns `false` (`harness.rs:1521-1527`). The agent then hits the turn budget and bails (`harness.rs:1994-1996`). `settle` marks it `Failed` (`harness.rs:2807-2822`). No test covers this, and no digest goes to the human. | I |
| Automatic PR creation | missing | Accepted work is merged locally (`git::merge`, `harness.rs:2489-2500`; U `git::branch_merge_and_conflict`). No PR API call exists anywhere in the crates. | — |
| Readiness score | missing | No code. | — |
| Deterministic ticket check before review | partial | `harness.rs:2438-2456`: a non-zero exit refuses review. Covered by E2E flat (`check failed` escalation, `check_passed` asserted). This is a single shell command, not Gate A. | I, B |
| Send-back on rejection | partial | Human rejection reopens the ticket (`harness.rs:2474-2486`; E2E flat). Manager review of deep work: E2E `nested_leads_review_and_revise_their_reports`. | I, B |
| Independent reviewer model (≠ writer) | missing | The reviewer is the human or the parent agent (`harness.rs:2459-2461`). No rule makes it a different model. | — |
| Max 3 Gate B send-backs, then human | missing | API workers are bounded only by the turn budget. CLI workers stop after 4 rounds (`harness.rs:1685`, untested). No 3-loop cap. | I (CLI 4-round cap, untested) |
| CI/CD step in the lifecycle | missing | No code. (`.github/workflows/test.yml` is this repo's own CI, not a product feature.) | — |
| Before/after at Gate A and Gate C | missing | No code. `show_options` (E2E `visual_options_wait_for_the_users_pick`) is a user picker, not a comparison. | — |
| Model allowlists (orchestration vs execution) | missing | `usable_models` filters by provider and key only (`config.rs:270-282`). No per-project list, no split by role. | — |
| Skill allowlist | missing | `skills.rs` has overrides but no allow/deny. | — |
| Plugin allowlist | missing | No plugin system. | — |
| Single policy gateway | missing | Tool deny rules exist in two places: `mcp.rs:82` `denied()` (U `rules_keep_tools_away`) and `agents.rs` (U `rules_become_denied_tools`). They cover tools and apps only. | — |
| Per-task budget recorded | partial | `budget_usd` per agent and project; `over_budget` (`harness.rs:1960-1976`). The E2E only asserts the budget text appears in the brief (`tests/e2e.rs:222`). | I, B |
| Budget enforced before the call | missing | The check runs after spend (`spent >= budget`). It warns, then bails 2 turns later (`harness.rs:1997-2008`), so overspend is possible by design. Not tested. Not a gateway. | I (after-spend only, untested) |
| Memory: versioned, scoped, revertible notes | partial | Git-backed (`memory.rs`). U `scopes_and_persistence`, `repo_format_hand_edits_and_history`, `the_librarian_files_a_batch_as_one_commit`. `revert` (`:931`) has no test. | I, B (revert untested) |
| Memory proposal intake | partial | `propose()` queues candidates (`memory.rs:160`). `drain()` (`:804`) drops empty, multi-line, secret-looking (`:835`) and previously rejected (`:837`) notes, and merges near-duplicates (`similar`, `:848`). U `the_librarian_files_a_batch_as_one_commit` asserts those exact drop reasons and the merge. Not proven: the §8.1 schema (type, evidence, source, ttl). | I, B |
| Memory capture model | partial | Logistic model; U `memory_learn::proposes_and_skips`, `learns_from_rejections`. This is a capture filter, not the §8.2 decision agent. | I, B |
| Evidence verification | missing | No evidence field. | — |
| Contradiction check | missing | Duplicates are merged. Contradictions are not detected. | — |
| Secret / personal-data check | partial | `looks_secret` is a fixed list of 10 substrings (`memory.rs:189-194`), tested via the librarian test. Personal data is not checked. | I, B |
| Entries are data, never instructions | **missing (code does the opposite)** | `memory_learn.rs:164` labels "remember / from now on / always" notes as kind `instruction`, and U `third_person_rewrites` asserts it (`:339`). `context()` injects notes into chat and agent system prompts (`chat.rs:1326`, `fleet.rs:628`), framed as "context, not commands" (`memory.rs:1030`). Framing in a prompt is exactly the "enforced in prompts" approach §8.3 and §10 rule out, and the kind is never rejected. | I, B (asserts the opposite) |
| Entries cannot alter permissions, gates or budgets | unknown | No code path from memory to config was found, but no test proves there isn't one. | — |
| Per-entry provenance | partial | Notes store `source` and thread (`add_from`, `memory.rs:651`). No task id, no evidence link. | I |
| Global promotion needs human approval | missing (approval flow); qualified | **No promotion-with-approval flow exists.** Paths that write a global note (`project: None`), traced from every caller in the audited crates and `crates/tauri-app`: (1) **Agents: no path found.** The agent tool `memory_save` (`mcp.rs:54-69`) always proposes with a non-empty scope from `BACKSPACE_MEMORY` (`mcp.rs:30-31`), which `chat.rs:1575-1579` sets to the project path or `agent:<id>`, never empty. `drain()` files it under that scope (`memory.rs:846`). So an agent cannot write global memory, with or without approval. (2) **Explicit human writes:** the Memory page (`ui/memory.js:125` → `memory_add`, `tauri-app/src/main.rs:1278`) and the paired phone (`POST /v1/memory`, `companion.rs:202`, token-guarded) call `Memory::add` with the scope the person chose. These are human actions, not bypasses. (3) **Automatic capture, reachable without prior approval:** `chat_send` (`tauri-app/src/main.rs:1507`) calls `Memory::capture` on the user's own message when `memory_on && memory_auto` (`main.rs:1525`; `memory_auto` defaults to `true`, `prefs.rs:121`). A thread with no project and no agent gives scope `None` (`main.rs:1532`), so the logistic model writes a **global** note with `on: true` (`memory.rs:704-744`). The person is told after the write and can undo it (comment at `main.rs:1522-1523`). The text is the user's own words, not an agent's, so this is not the §8 agent-promotion path; but it is a model-decided global write with no approval before it. Evidence: U `memory::tests` call `capture(.., None, ..)` and get a global note (`memory.rs:1063`); the `chat_send` path is untested (`crates/tauri-app` is not in the suite). | I, B (capture writes global; chat_send path untested) |
| Rejections logged with reasons | partial | Receipts keep drop reasons (librarian test). No eval set built from them. | I, B |
| Selective reads under a token cap | partial | `context()` is capped at `BUDGET = 6000` **characters** (`memory.rs:86`, `:1016`) and ranked by importance, use and age. It is not ranked by relevance to the task. The cap has no test. | I, B (cap untested) |
| Memory accept/reject eval | missing | No code. | — |
| herdr adapter, version pin | missing | No herdr code. Latest upstream release noted in §7 item 4 (v0.9.5, checked 2026-10-30). | — |
| Plugin API + audit hook | missing | "plugin" appears only as Tauri plugins and a runner flag (`runner/src/lib.rs:97`). `ui/extensions` are UI apps. | — |
| On-demand skills | partial | `skills.rs`; U `defaults_load_with_descriptions`, `search_ranks_a_name_hit_above_a_body_hit`, `a_workspace_folder_overrides_a_default_and_shows_as_ours`. No allowlist, no per-project enable list. | I, B |
| Test-on-edit, affected-test selection | missing | No code. | — |
| Isolated test run (worktree + container) | partial | Worktree per ticket: E2E flat asserts `.backspace/worktrees/a/.git` and the branches. No container. Docker is used only for agents' computer use, and that test did not run here. | I, B (worktree only) |
| Regression blocks change; ADE/IDE parity | missing | No code. | — |
| Continuous chat threads | partial | `chat.rs`, 12 U tests (e.g. `threads_persist_react_branch_delete`). Scope confirmed in by Charles on 2026-10-30 (§2a). | I, B |
| Shared personal context | partial | Memory `context()` is injected into chat and agent prompts (`chat.rs:1326`, `fleet.rs:628`). The project/global split is tested in U `scopes_and_persistence`. The injection itself is not tested. | I, B (scoping only) |
| Cloud execution | partial | U `cloud::dev_server_end_to_end`, E2E `a_remote_machine_is_followed_and_driven`. Not tied to the Cauldron lifecycle. | I, B |
| Local execution | partial | E2E `a_ticket_runs_on_a_cli_worker`, I `interrupted::a_restart_carries_a_cli_worker_on_in_its_own_session`. | I, B |

## 2a. §5 feature inventory and requirement mapping

On 2026-10-30 Charles set the §5 inventory as the eleven entries below and confirmed that continuous chat is in scope. As instructed, PLAN.md has not been edited: its §5 still holds the old list and the TODO. Every requirement row of the §3 matrix now carries a **§5 entry** column. A row that spans entries names its primary entry first and the others in brackets.

Rows counted by primary entry (from the §3 matrix; no row reaches A):

| §5 entry | Matrix rows |
|---|---|
| 5.1 Conversational intake and classification | 2 missing |
| 5.2 Specification and planning | 5 partial, 4 missing |
| 5.3 Intelligent routing and cost-aware orchestration | 2 partial, 2 unknown, 6 missing |
| 5.4 Policy, permissions and budgets | 3 missing |
| 5.5 Build and execution infrastructure | 1 partial, 2 missing |
| 5.6 Independent testing and verification | 1 partial, 1 unknown, 5 missing |
| 5.7 Review, gates and delivery | 3 partial, 4 missing |
| 5.8 Persistent memory and context | 4 partial, 5 missing |
| 5.9 Skills and plugins | 1 partial, 1 missing |
| 5.10 Developer experience and observability | 4 partial, 5 missing |
| 5.11 Existing capabilities retained / outside current implementation scope | 0 rows |

5.11 has no PLAN.md requirements by design. It holds what the repo already does outside the inventory, left untouched: routines, the delegate board, apps, voice, the iOS companion, colony, A2A agents, and the "Backspace" naming.

**The old §5 items, mapped to the new entries**

| Old §5 item | New entry |
|---|---|
| Chat box intake that starts a project or task | 5.1 |
| Local decision model that triages tasks | 5.1 (classification), 5.3 (decision model) |
| Planning mode (grill-me) | 5.2 |
| Markdown handoff files (spec.md, tests.md) | 5.2 |
| Builder and Rampart | 5.6 |
| Frontier orchestrator only when needed, small/local executors | 5.3 (executors run on 5.5) |
| Automatic PR creation | 5.7 |
| Readiness check, send-back loop, CI/CD, before/after judging | 5.7 |
| Per-project allowlists for models, skills, plugins | 5.4 |
| Master memory with a memory decision agent | 5.8 |
| herdr adapter | 5.5 |
| Plugin and skill system | 5.9 |
| In-app auto-testing and regression, ADE and IDE | 5.6 (surfaces in 5.10) |
| Continuous chat with threads, shared personal context, cloud and local execution | **In scope (confirmed 2026-10-30).** Threads: 5.1. Shared personal context: 5.8. Cloud and local execution: 5.5. |

**Flagged for your decision: could not be mapped without a choice**

1. **Memory auto-capture of your own chat messages** (existing; writes global notes with no prior approval, see §2). It is either 5.8, and so bound by the §8 rules (schema, evidence, global approval), or 5.11, kept as-is outside Cauldron's memory rules. Which one?
2. **Pi as an embeddable agent base (§9.1)**, including the "spike pi-durable first" note. PLAN describes it as usable, not required. Is adopting Pi a requirement (5.5), or reference only like jessfraz/agent? This interacts with the open language decision.
3. **jessfraz/agent "useful ideas" (§9.1)**: one named session per task and guarded cleanup of merged work. PLAN marks the project as reference only. Are these ideas requirements (5.5), or context only?
4. **Removing human approval (Q5)** for merge and global memory. PLAN keeps approval until evals prove reliable but sets no threshold, so it is not yet a requirement under 5.7/5.8. Leave it as an open question?
5. **Security and supply-chain items stated only in §14 Risks** (containerized execution, test isolation, plugin signing on the roadmap). Sections 3–12 already cover containerization (§11 → 5.5) and plugin audit (§3.2 → 5.9). Plugin signing is roadmap only (§13). No action unless you want signing in 5.9 now.

Not mapped because they are not requirements: §1 Goal and §4 Pain points (motivation, covered by the rows they motivate), §2 Non-goals (constraints on scope), §13 Roadmap and §15 Open questions.

## 3. Acceptance coverage matrix, PLAN.md sections 3–12

| § | Requirement | Status | Evidence / gap | Evidence level | §5 entry |
|---|---|---|---|---|---|
| 3.1 | Cost ladder, escalation only on measured failure | partial | Workers only (see §2). The planner starts high by design. | I, B | 5.3 |
| 3.2 | Skills on demand | partial | `skills.rs` tests. | I, B | 5.9 |
| 3.2 | Plugins via a stable API, audited | missing | | — | 5.9 |
| 3.2 | Skills and plugins per-project and allowlisted | missing | | — | 5.4 (lists for 5.9) |
| 3.3 | File handoffs, never chat history | partial | SPEC.md and ticket files exist. Workers still receive the brief in chat; retries keep full history. | I, B | 5.2 (consumed by 5.3/5.5) |
| 3.3 | Diff information in deliverables | partial | `diff_stat` and `numstat` are attached to each deliverable (`harness.rs:2429-2435`). E2E `flat_project_with_gates_escalation_and_merges` asserts `changes` = `b.txt +1 -0` and that `diff_stat` names `b.txt`. Not proven: that diffs replace whole files in what agents are sent, or a diff-based handoff between stages. | I, B | 5.7 |
| 3.3 | Symbol-level context | missing | Nothing found in the audited crates (searched for tree-sitter, LSP, ctags, symbol and outline code). `lsp-types` appears in `Cargo.lock` only as a transitive dependency of the GPUI UI kit used by the excluded `crates/app`; no Cauldron code uses it. | — | 5.8 |
| 3.3 | Prompt caching for stable prefixes | unknown | Anthropic requests set `cache_control: ephemeral` on the tools+system prefix (`provider.rs:228-229`). No test asserts it; spec, skills and memory are not separately cached. | I | 5.3 |
| 3.3 | Summaries at stage boundaries | missing | | — | 5.3 |
| 3.3 | Escalate with a failure digest | missing | | — | 5.3 |
| 3.3 | Hard per-task budgets at the gateway | missing | Enforcement is after spend and is not a gateway. | I (after-spend only) | 5.4 |
| 3.4 | Frontier calls per merged PR | missing | | — | 5.10 |
| 3.4 | Tokens and cost per merged PR | partial | Per-agent `cost_usd`/tokens (`harness.rs:2061-2073`); `outcomes.jsonl` has cost and escalations per ticket (`harness.rs:2824-2846`; E2E flat asserts 2 lines). OTel spans: U `trace::records_tools_and_exports_otlp`. No per-PR aggregate. | I, B | 5.10 |
| 3.4 | Cost-to-green | missing | | — | 5.10 (objective owned by 5.3) |
| 3.4 | Escalation rate / success rate | partial | Raw data only (`outcomes.jsonl` escalations + outcome). No metric. | I, B (raw data only) | 5.10 |
| 3.4 | Human tokens per merged PR | missing | | — | 5.10 |
| 6 | Intake → triage → research / one-shot / long | missing | No typed triage, no research path, no one-shot spec path. | — | 5.1 (research report delivered via 5.7) |
| 6 | Low confidence asks the user | missing | | — | 5.1 |
| 6 | Checklist: goal | partial | `Problem` + `Solution` required (`spec.rs:57`). | I, B | 5.2 |
| 6 | Checklist: acceptance criteria | partial | `Acceptance` lines must each name a command or click (`spec.rs` rules; U `each_rule_names_what_is_missing`). | I, B | 5.2 |
| 6 | Checklist: one test per criterion | missing | `coverage()` maps criteria to tickets (E2E `a_plan_must_cover_the_spec`), not to tests. | — | 5.2 |
| 6 | Checklist: non-goals | partial | `Out of scope` must be non-empty (`spec.rs:57`); "None" is accepted. **Correction to the revision**, which said non-goals were missing. | I, B | 5.2 |
| 6 | Checklist: constraints, dependencies | missing | Not in `SECTIONS`. | — | 5.2 |
| 6 | Checklist: "before" baseline captured | missing | | — | 5.2 |
| 6 | Human "yes" on the plan | partial | The plan (ticket list) needs human approval: E2E flat asserts 2 plan approvals, the first rejected. The *spec* gate opens on rules alone (`phase.rs` `record_spec` → `GateBy::Rules`). | I, B | 5.2 |
| 6 | Baselines per task type (bug/feature/UI/refactor/research) | missing | No code for any of the five. The revision marked these unknown; no code exists, so they are missing. | — | 5.2 (compared in 5.7) |
| 6 | Gate A deterministic, no model override | partial | Primitive: `PhaseState::record` drops a model's pass (`phase.rs`, U `a_model_can_block_but_never_open`). Gate A itself does not exist. | I, B (primitive only) | 5.7 |
| 6 | Gate A content (sealed tests, lint, types, before/after) | missing | The ticket `check` is one command chosen by the planner. | — | 5.7 |
| 6 | Gate B different model, readiness score, 3 loops | missing | | — | 5.7 (readiness scoring via 5.3) |
| 6 | CI/CD, Gate C preview before/after | missing | | — | 5.7 |
| 6 | Human approval before merge | partial | Deliverables at depth ≤ `human_review_depth` wait for the human (`harness.rs:2459-2486`; E2E flat rejects then approves ticket `a`). Deeper work is merged before the manager reviews it (`harness.rs:2505-2506`). `auto_approve` (`config.rs:79`) skips the human entirely. | I, B | 5.7 |
| 7.1–7.3 | Builder/Rampart separation, sealing, hash, fail-on-baseline, flake, mutation, traceability, spot-check, arbitration | missing | No code for any of these. The revision marked spot-check and arbitration unknown; no code exists, so they are missing. | — | 5.6 |
| 7.4 | Four escalation triggers | missing | **None of the four triggers is implemented as specified** (§2: N sealed-test failures, same failure signature twice, cheap-rung budget, expected-cost comparison). Generic worker escalation exists as an adjacent mechanism only. | — | 5.3 |
| 7.4 | One frontier call with a digest | missing | Up to 3 climbs with the full transcript (`config.rs:122`; E2E flat asserts 2 climbs). | I, B (3 climbs, not one) | 5.3 |
| 7.4 | Human handoff after the failed escalation | unknown | `settle` marks a worker `Failed` after the turn-budget bail (`harness.rs:1994-1996`, `:2807-2822`). No test, no digest to the human. Stays unknown until a test verifies it. | I | 5.3 |
| 8.1 | Proposal schema | missing | `NoteCandidate` (`memory.rs:132`) lacks evidence, ttl and a typed scope check. | — | 5.8 |
| 8.2 | Decision agent checks | partial | Duplicates and secrets tested; the durability check is a heuristic capture model. Evidence, contradiction, scope and instruction-phrasing checks missing. | I, B | 5.8 (decision model via 5.3) |
| 8.3 | Data, never instructions | missing | Code labels and injects `instruction` notes (see §2). | I, B (asserts the opposite) | 5.8 (enforced at 5.4 gateway) |
| 8.3 | Provenance, versioning, revert | partial | Git versioning tested; `revert` untested; provenance incomplete. | I, B (revert untested) | 5.8 |
| 8.3 | Global needs human approval | missing (qualified) | No approval flow. No agent path to global memory was found. One reachable path writes global without prior approval: auto-capture of the user's own chat messages (see §2). | I, B (capture only) | 5.8 |
| 8.3 | Token-capped selective reads | partial | 6000-char cap, untested, not relevance-ranked. | I, B (cap untested) | 5.8 |
| 8.4 | Eval: precision, recall, dup rate, secret-catch rate, bloat | missing | | — | 5.8 |
| 9 | herdr behind adapter, pinned, state never a gate | missing | | — | 5.5 |
| 10 | Decision tier for triage, checklist, readiness, memory | missing | | — | 5.3 |
| 10 | Router eval from real traces | missing | `outcomes.jsonl` is raw material only. | — | 5.3 |
| 10 | Allowlists in code at one gateway | missing | | — | 5.4 |
| 10 | Bounded executor jobs | partial | See §2. | I, B | 5.5 (routing in 5.3) |
| 11 | Affected tests, sandbox, streamed results, block on regression, same in ADE/IDE | missing | Worktrees only. | — | 5.6 (sandbox 5.5; streaming and ADE/IDE surface 5.10) |
| 11 | Unit + E2E layers | partial | 204 tests exist (Appendix A). | I, B | 5.6 |
| 11 | Sealed rampart suites as permanent regression | missing | | — | 5.6 |
| 11 | Recorded-trace replay, stub models, frozen router | missing | E2E uses a hand-scripted fake server (`tests/e2e.rs:59` `respond`), not recorded traces. The revision marked this partial; it has no recorded traces, so it is missing. | I, B (scripted fake, not recorded) | 5.6 |
| 3.3/6 | Automatic PR creation | missing | No PR API call in the crates. Adjacent: local merge into the parent branch (`harness.rs:2489-2500`; U `git::branch_merge_and_conflict`). (Added: it was in §2 but had no matrix row.) | — | 5.7 |
| 7.3 | Property and edge-case tests derived from acceptance criteria | missing | No code. | — | 5.6 |
| 8.3 | Rejections logged with reasons, used as eval data | partial | Librarian receipts keep drop reasons (U `the_librarian_files_a_batch_as_one_commit`). Not used as eval data. | I, B | 5.8 |
| 9.2 | Builder and Rampart in role panes | missing | No herdr or pane code. | — | 5.5 |
| 9.2 | IDE mode: the human watches and attaches to the same panes agents use | missing | No shared-pane mechanism found. The IDE terminal canvases are UI code in `crates/tauri-app`, which is not in the suite. | — | 5.10 |
| 10 | Swappable decision-model interface; candidates chosen on license, size, locality, then own eval | partial | `RouterBackend` Jev / heuristic switch (`router.rs:276-280`); U `heuristic_scales_with_task`. No license/locality record, no eval. | I, B (Jev path untested) | 5.3 |
| 12 (M13) | Audit log | partial | Gate decisions are recorded with who answered (`phase.rs` `Gate`; U `a_refusal_is_kept_and_a_pass_moves_on`); per-ticket outcomes in `outcomes.jsonl` (E2E flat). No unified audit log of agent actions, policy refusals or approvals. | I, B | 5.10 |
| 12 (M13) | Observability | partial | OTel traces with token counts, exportable (U `trace::records_tools_and_exports_otlp`, `exports_to_a_collector`). Not linked to tasks/PRs or §3.4 metrics. | I, B | 5.10 |
| 12 (M13) | Failure-mode tests | unknown | PLAN does not define the failure modes. Some error-path tests exist (e.g. U `backend::tests::a_missing_cli_is_an_error_not_a_panic`, I `features::one_limited_login_does_not_stop_the_project`), but there is no failure-mode suite to judge against. | I, B (scope undefined) | 5.6 |
| 12 (M13) | Metrics dashboard (§3.4 metrics) | missing | No §3.4 metrics are computed, so none are shown. The Home dashboard in `crates/tauri-app/ui` shows other data. | — | 5.10 |
| 12 | M0: feature list complete | **pending your confirmation** | Charles gave the §5 inventory as 11 entries on 2026-10-30 (§2a). Every row of this matrix is mapped to one. PLAN.md itself still holds the TODO (not edited, per instruction). Open flags in §2a. | n/a | n/a (this inventory) |

## 4. Frontier-efficient orchestration

This is the core gap, measured against the objective in §1a (cheapest successful path, total cost-to-green). The code has cheap-first primitives for **workers**. Its **planner always starts at high effort, and under the heuristic at Xhigh on a tier ≥ 3 model**, and never moves down. Whether that means a frontier model cannot be established from the code: there is no frontier classification, only a config-set tier integer. In the default config tier ≥ 3 resolves to `claude-opus-5-5` or above. Nothing measures whether the planner's starting point is the cheapest one that succeeds.

| Capability | Status | What exists | What is missing | Evidence level |
|---|---|---|---|---|
| Cheap-first execution routing | partial | Worker cap at tier ≤ 2 / Medium, tested (E2E flat). | Starts at the top of the cap, not the cheapest rung. Applies to workers only. | I, B |
| Local decision-model selection | missing | Jev over HTTP or a keyword heuristic. | A local decision model behind a typed interface; an eval (§10) before choosing one. | I, B (heuristic only; Jev untested) |
| Local task classification + confidence | missing | Ticket triage runs locally (generative, 4 states). | one-shot/long/research classification; a calibrated confidence and threshold. | I, B (ticket triage only) |
| Frontier only when needed (planner) | missing, code works against it | Effort floor High always (`router.rs:301-303`); Xhigh + tier ≥ 3 under the heuristic (`router.rs:440-451`); no escalation for Main. | A frontier classification for models; a planner that starts at the cheapest rung expected to succeed and escalates on measured failure; a test asserting the chosen *model*, not just effort. | I, B (asserts high effort) |
| Cost-aware escalation | missing | Next model chosen by tier, then price. | Expected retry cost vs frontier cost. | I (price tiebreak only) |
| Single frontier escalation + compact digest | missing | Up to 3 climbs with the full transcript. | One call; a digest of failing tests, signatures and last diff. | I, B (3 climbs) |
| Bounded local executors | partial | Local scouts and triage, tested. | Lint, test runs, failure triage and diff-summary jobs. Keep code generation off the local tier by default. | I, B |
| Human handoff after failed escalation | unknown | Status `Failed` via the turn-budget bail. | A test, a digest, and an explicit stop. | I |
| Cost-to-green measured | missing | Per-ticket cost in `outcomes.jsonl`. | Aggregation through Gate A across retries. | I, B (per-ticket cost only) |

Consequence for M2: the last assertion of `heuristic_scales_with_task` (`main.effort >= Effort::Xhigh`) encodes the old planner policy. In M2 that assertion is replaced by acceptance tests for the new routing policy: the planner's starting rung, the frontier classification, and cost-to-green accounting. It is not deleted outright, and the other assertions in the test stay. I will name the change in the M2 report.

## 5. Verified existing capabilities and their limits

Every row here has implementation and behavioural evidence (I, B). None has acceptance evidence (A).

| Capability | Proven by | Limitation against PLAN.md |
|---|---|---|
| Spec rules gate (7 sections, placeholders, acceptance needs a command) | U spec ×6, I `spec_gate`, E2E `a_plan_must_cover_the_spec` | No constraints, dependencies, baseline or criterion→test mapping |
| Models may block a gate, never open it | U `phase::a_model_can_block_but_never_open` | A primitive for Gate A, not Gate A |
| Plan approval by human | E2E flat (2 plan approvals) | On tickets, not on the spec |
| Worktree + branch per ticket, merge into parent | E2E flat, U `git::branch_merge_and_conflict` | Local merge, no PR; no container |
| Check command gates review | E2E flat | One command, planner-authored, not sealed |
| Worker escalation ladder | E2E flat (2 steps), E2E nested (1 step) | Up to 3 climbs, not cost-aware, no digest |
| Local routing for scouts, triage, workers | I features ×4 | Opt-in; "workers" includes code generation |
| Per-ticket cost log | E2E flat (`outcomes.jsonl`) | No per-PR metrics |
| Memory librarian (secret, rejected, duplicate) | U `the_librarian_files_a_batch_as_one_commit` | Substring secret list; no evidence or contradiction checks; instruction notes allowed |
| Tool and app deny rules | U `rules_keep_tools_away`, `rules_become_denied_tools` | Tools only; deny-list, not allowlist; not a gateway |
| On-demand skills | U skills ×5 | No allowlist |
| OTel tracing with token counts | U trace ×2 | Per span, not per PR |

## 6. Missing critical capabilities (by impact)

Ordered by impact on the objective (cheapest successful path, total cost-to-green).

1. **M2 — routing policy and cost ladder:** a frontier classification for models, the planner's starting rung, a decision interface, the router eval, and cost-to-green plus the other §3.4 metrics emitted per run.
2. **M8 — escalation control and review loops:** the §7.4 triggers, cost-aware escalation, exactly one frontier escalation with a compact digest, human handoff, Gate B with a different reviewer, and the 3-loop cap.
3. **M6 — execution infrastructure:** sandboxed worktrees plus containers, and local executors limited to bounded jobs (tests, lint, failure triage, diff summary, search) with code generation kept off the local tier by default. Also the herdr adapter.
4. **M1 — gateway and budgets:** model (by role), skill and plugin allowlists at one gateway, and budgets checked before the call.
5. **M5 — Builder/Rampart:** separation, sealed and hash-locked tests, fail-on-baseline, flake check, mutation score, traceability.
6. **M3/M4 — intake and planning:** typed triage with threshold and ask path; checklist items for constraints, dependencies, criterion→test, and baselines per task type.
7. **M7/M9 — PR and gates:** automatic PR, Gate A, CI/CD, Gate C.
8. **M10 — memory:** schema, evidence, global approval, eval, and the data-not-instructions rule (see §7).
9. **M11 / M12 / M13:** plugin API and audit hook; test-on-edit regression in ADE and IDE plus recorded-trace replay; dashboard.

## 7. Decisions and TODOs, by what they block

**Blocks M0 approval**

1. **§5 feature list.** You set the inventory (11 entries) and confirmed continuous chat on 2026-10-30. Every matrix row is mapped (§2a). M0 now needs only: your answers to the five flags in §2a, and whether to write the inventory into PLAN.md §5 (I have not edited PLAN.md).

**Blocks M1 (decide before I write code)**

2. **Implementation language (Q1).** PLAN.md offers TypeScript or Go. The repo is Rust. I recommend Rust; it is your decision.
   - Rust: extends the existing harness, router, spec gate, memory and git worktree code, plus 204 passing tests. One binary with the Tauri app. Cost: slower iteration, and Pi (§9.1) is TypeScript, so using it means an RPC boundary.
   - TypeScript: native to Pi and its SDK, faster iteration, a large agent-tooling ecosystem. Cost: a rewrite of the orchestration core, and today's tests do not carry over.
   - Go: easy concurrency and single binaries; `capsule/agentd` is already Go. Cost: a rewrite of the core too, a thinner LLM-tooling ecosystem, and no fit with Pi.

**Blocks later milestones only**

3. M2: decision model. Per §10, I will build the swappable interface and the eval harness first, run the eval on Cauldron traces, then bring you the comparison. No candidate is picked before that. Prerequisite: there are no labeled traces yet (`outcomes.jsonl` is unlabeled), so M2 needs a trace set. I will ask for it then.
4. M6: herdr release. "v2" is still not identified. Correction: the "0.8.x" in earlier revisions came from PLAN.md §9.2 itself, not from my own check, and it is out of date. Checked 2026-10-30 against the GitHub releases page of `herdrdev/herdr` (https://github.com/herdrdev/herdr/releases): the newest stable is **v0.9.5**, marked "Latest", released 14 Oct (2026 per the adjacent preview-build tag names), a hotfix for v0.9.4. Preview builds are published alongside. No v2 or 2.x appeared on the first page of releases (10 pages total, only the first was read). The upstream repo was identified through a secondary article (developersdigest.tech, "Herdr Deep Dive"). The page was read through a web-fetch tool that summarizes, not by cloning or calling the GitHub API. Treat v0.9.5 as **verified as of 2026-10-30 at that URL, subject to re-check at M6**, since releases ship weekly. The version pin is still an M6 decision.
5. M2: what counts as "frontier" and "mid-tier" in the model config (today `tier` is a config-set integer 1–4). I will propose a definition as part of the M2 routing policy. M8 then uses it for the single-escalation rule.
6. Q5 (when human approval is removed): not needed until after the evals; PLAN keeps approval for now.

**Already decided by PLAN.md, applied without asking**

- Features that exist outside §5 (routines, delegate board, apps, voice, iOS companion, colony, A2A) stay untouched.
- Memory entries are data, never instructions (§8.3). In M10 I will not just delete the `assert_eq!(p.kind, "instruction")` assertion in `memory_learn::third_person_rewrites` (`memory_learn.rs:339`). Failing acceptance tests come first, then that one obsolete assertion is replaced by tests proving that: (a) an instruction-phrased proposal is rejected or stored as inert data, never labelled as an instruction; (b) memory reaching a prompt is quoted data and cannot change tool, model, skill or plugin permissions at the M1 gateway; (c) memory cannot pass, skip or reopen a gate; (d) memory cannot change a budget. The rest of `third_person_rewrites` stays.
- herdr state is never a gate. The code keeps its "Backspace" names; no rename.
- Human approval stays on merge and global memory.

## 8. Token use and frontier calls (M0)

- Product code: 0 model calls. Only tests ran, and those use local fake servers.
- This audit session runs on a frontier model. I cannot see its token count or call count, so both are **not measured**. I am not substituting an estimate.
- Raw logs were kept out of the doc except the suite output.

## Appendix A: full suite output (rerun 2026-10-30, `e236259`, `cargo test --no-fail-fast`, compile lines removed)

```
Running unittests src/main.rs (target/debug/deps/backspace_cli-e14ffda91c75b4ab)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/backspace_colony-0abb1bf7f3d72512)

running 25 tests
test backend::tests::cli_states_fold_into_five ... ok
test backend::tests::create_args_are_the_cli_flags ... ok
test backend::tests::error_message_comes_out_verbatim ... ok
test backend::tests::exec_output_reads_the_cli_exit_code ... ok
test backend::tests::cli_workspace_json_maps_to_ours ... ok
test backend::tests::capabilities_print_providers ... ok
test backend::tests::a_missing_cli_is_an_error_not_a_panic ... ok
test backend::tests::parse_error_ignores_noise_and_junk ... ok
test colony::tests::a_missing_state_file_is_an_empty_colony ... ok
test colony::tests::colony_json_round_trips ... ok
test colony::tests::duplicate_name_is_refused_before_the_backend ... ok
test colony::tests::a_corrupt_state_file_is_an_error ... ok
test colony::tests::exec_records_argv_and_returns_the_output ... ok
test colony::tests::list_adopts_what_the_backend_reports ... ok
test colony::tests::unknown_workspace_is_an_error_not_a_panic ... ok
test fake::tests::the_fake_refuses_a_duplicate_like_the_cli ... ok
test colony::tests::stop_force_reaches_the_backend ... ok
test fake::tests::the_fake_reports_running_then_stopped ... ok
test prompt::tests::chat_prompt_copes_without_a_task ... ok
test prompt::tests::chat_prompt_names_the_workspace ... ok
test colony::tests::task_and_created_survive_a_backend_round_trip ... ok
test tools::tests::every_tool_carries_a_description_and_an_object_schema ... ok
test tools::tests::five_tools_with_the_right_names ... ok
test tools::tests::required_arguments_are_the_ones_the_registry_needs ... ok
test colony::tests::lifecycle_create_list_status_stop_start_delete ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running unittests src/lib.rs (target/debug/deps/backspace_core-506ff0b8f481c1a9)

running 140 tests
test accounts::tests::default_profile_dir_sits_under_the_data_accounts_folder ... ok
test accounts::tests::best_prefers_healthy_over_warning_at_equal_failures ... ok
test accounts::tests::note_limit_cools_down_and_note_fail_does_not ... ok
test accounts::tests::order_drops_disabled_and_cooled_down_accounts ... ok
test accounts::tests::order_ranks_state_then_failures_then_priority_then_last_ok ... ok
test accounts::tests::policies_decide_who_switches ... ok
test accounts::tests::env_for_uses_the_providers_own_variable ... ok
test accounts::tests::fails_decay_after_a_day_and_the_ring_stays_small ... ok
test agents::tests::next_run_maths ... ok
test agents::tests::next_run_round_trips_a_negative_offset ... ok
test agents::tests::quoting ... ok
test accounts::tests::round_trips_through_json_and_decays_on_load ... ok
test agents::tests::brief_in_names_the_project_personality_and_routines ... ok
test agents::tests::rules_become_denied_tools ... ok
test agents::tests::routine_summaries ... ok
test agents::tests::save_cleans_routines_and_refreshes_a_passed_wake ... ok
test agents::tests::routine_save_validates_and_persists ... ok
test agents::tests::save_list_brief_delete ... ok
test apps::tests::manifest_rules ... ok
test artifact::tests::a_table_row_must_match_the_header ... ok
test artifact::tests::an_old_state_file_without_artifacts_still_loads ... ok
test artifact::tests::a_body_cannot_be_unbounded_and_a_summary_says_what_it_is ... ok
test artifact::tests::ids_are_unique_and_addressable ... ok
test artifact::tests::each_kind_is_validated_on_its_own_field ... ok
test artifact::tests::options_must_refer_to_a_set_that_exists ... ok
test artifact::tests::kinds_this_build_does_not_know_are_kept_not_dropped ... ok
test board::tests::a_thread_is_the_pair_whichever_way_it_went ... ok
test board::tests::conversations_group_replies_and_broadcasts_separately ... ok
test board::tests::post_and_read ... ok
test artifact::tests::the_list_is_bounded_and_drops_the_oldest ... ok
test apps::tests::install_serve_store_remove ... ok
test chat::tests::a_thread_file_without_a_life_loads_as_a_normal_chat ... ok
test chat::tests::ansi_is_stripped ... ok
test chat::tests::base64_roundtrip ... ok
test chat::tests::link_meta ... ok
test chat::tests::making_a_thread_a_ghost_deletes_the_file_it_had ... ok
test chat::tests::mentions_pick_a_cli ... ok
test chat::tests::a_temporal_thread_forgets_itself_after_its_deadline ... ok
test chat::tests::a_ghost_thread_is_never_written_and_never_listed ... ok
test chat::tests::purging_ghosts_removes_only_ghosts ... ok
test chat::tests::titles_are_short ... ok
test chat::tests::talking_to_a_temporal_thread_moves_its_deadline ... ok
test cloud::tests::plan_table ... ok
test companion::tests::pair_link_escapes ... ok
test config::tests::default_config_parses ... ok
test delegate::tests::a_task_whose_bot_is_gone_comes_back_unassigned ... ok
test delegate::tests::adds_assigns_and_survives_a_reopen ... ok
test delegate::tests::an_empty_title_is_refused ... ok
test delegate::tests::auto_off_leaves_work_unassigned ... ok
test delegate::tests::auto_picks_the_shortest_queue_then_the_cheaper_minute ... ok
test delegate::tests::importing_tickets_twice_keeps_one_task_each ... ok
test delegate::tests::the_snapshot_prices_a_queue_and_a_cli_is_free ... ok
test diagram::tests::plan_layers_by_blockers_and_flags_missing_checks ... ok
test effort::tests::clamps_up_then_down ... ok
test chat::tests::threads_persist_react_branch_delete ... ok
test git::tests::a_new_repo_lists_folders_and_says_so ... ok
test git::tests::day_numbers ... ok
test git::tests::parse_changes_reads_status_and_counts ... ok
test harnesses::tests::codex_plan_from_id_token ... ok
test harnesses::tests::toggles_override_scan ... ok
test harnesses::tests::versions ... ok
test layout::tests::a_card_can_come_out_and_a_split_still_works ... ok
test layout::tests::a_full_stack_never_loses_a_canvas ... ok
test layout::tests::a_stack_survives_json_and_older_shapes_still_load ... ok
test layout::tests::centre_swaps_and_self_is_a_no_op ... ok
test layout::tests::closing_a_card_keeps_the_stack_valid_and_renumbers_it ... ok
test layout::tests::defaults_and_presets_cover_every_pane ... ok
test layout::tests::drag_to_an_edge_splits_there_and_the_old_spot_closes ... ok
test layout::tests::dropping_on_a_cards_middle_stacks_instead_of_swapping ... ok
test layout::tests::json_shape ... ok
test layout::tests::one_over_two_is_a_long_top_and_two_squares ... ok
test layout::tests::remove_and_renumber_keep_it_valid ... ok
test layout::tests::stacking_two_keeps_both_and_raising_swaps_which_is_in_front ... ok
test layout::tests::zones ... ok
test mcp::tests::rules_keep_tools_away ... ok
test memory::tests::dates_and_meta ... ok
test memory::tests::moves_old_json_in ... ok
test cloud::tests::dev_server_end_to_end ... ok
test memory::tests::repo_format_hand_edits_and_history ... ok
test git::tests::branch_merge_and_conflict ... ok
test memory_dream::tests::plan_applies_conservatively ... ok
test memory_dream::tests::prompt_lists_notes_and_messages ... ok
test memory::tests::scopes_and_persistence ... ok
test memory_learn::tests::learns_from_rejections ... ok
test memory_learn::tests::third_person_rewrites ... ok
test model_server::tests::human_size_reads_in_decimal_units ... ok
test model_server::tests::is_loopback_only_for_this_machine ... ok
test memory_learn::tests::proposes_and_skips ... ok
test model_server::tests::openai_base_adds_v1_for_ollama ... ok
test model_server::tests::order_breaks_ties_by_size_then_id ... ok
test model_server::tests::order_puts_the_smallest_tiny_first ... ok
test model_server::tests::parse_openai_models_fixture ... ok
test model_server::tests::parse_ollama_tags_fixture ... ok
test model_server::tests::pick_falls_back_and_handles_empty ... ok
test model_server::tests::pick_maps_the_task_kind ... ok
test model_server::tests::pick_prefers_the_named_family ... ok
test model_server::tests::recommend_needs_a_running_runtime ... ok
test model_server::tests::recommend_prefers_the_tiny_model_and_stays_stable ... ok
test model_server::tests::tiny_by_parameter_count ... ok
test model_server::tests::tiny_from_the_file_size_only_when_nothing_else_says ... ok
test model_server::tests::tiny_from_the_name_when_the_count_is_missing ... ok
test options::tests::add_names_variants_and_pick_records_the_choice ... ok
test options::tests::bad_sets_are_refused ... ok
test phase::tests::a_model_can_block_but_never_open ... ok
test phase::tests::a_refusal_is_kept_and_a_pass_moves_on ... ok
test phase::tests::a_refused_respec_reopens_specify_and_a_pass_does_not_skip_ahead ... ok
test phase::tests::round_trips_and_old_state_loads_without_it ... ok
test phase::tests::loads_and_saves_beside_the_project ... ok
test provider::tests::anthropic_body_sets_effort_only_when_supported ... ok
test provider::tests::openai_roundtrips_tool_calls ... ok
test router::liveness_tests::a_listening_port_is_up_and_a_closed_one_is_not ... ok
test projects::tests::save_list_rename_assign_delete ... ok
test router::tests::cli_pin_routes_workers_not_the_planner ... ok
test router::tests::heuristic_scales_with_task ... ok
test skills::tests::defaults_load_with_descriptions ... ok
test skills::tests::description_reads_plain_folded_and_literal ... ok
test skills::tests::found_says_where_a_skill_came_from ... ok
test skills::tests::search_ranks_a_name_hit_above_a_body_hit ... ok
test spec::tests::a_complete_spec_passes ... ok
test spec::tests::coverage_names_every_hole ... ok
test spec::tests::each_rule_names_what_is_missing ... ok
test spec::tests::extract_takes_the_last_spec_block_only ... ok
test spec::tests::the_prompt_carries_the_grilling_skill_the_rules_and_the_last_refusal ... ok
test spec::tests::the_skeleton_alone_is_refused_for_every_empty_part ... ok
test ticket::tests::parses_and_renders ... ok
test tools::tests::resolve_blocks_escape ... ok
test skills::tests::a_workspace_folder_overrides_a_default_and_shows_as_ours ... ok
test tools::tests::write_edit_read_bash ... ok
test trace::tests::records_tools_and_exports_otlp ... ok
test update::tests::a_merged_pull_request_reads_as_its_title ... ok
test update::tests::another_commit_is_newer_but_the_same_one_is_not ... ok
test update::tests::falls_back_to_the_notes ... ok
test update::tests::notes_without_a_stamp_say_nothing ... ok
test update::tests::only_a_hash_counts_as_a_commit ... ok
test update::tests::reads_the_commit_the_workflow_passed ... ok
test update::tests::the_changelog_is_pull_requests_newest_first ... ok
test update::tests::the_stamp_is_the_commit_the_build_was_given ... ok
test trace::tests::exports_to_a_collector ... ok
test memory::tests::the_librarian_files_a_batch_as_one_commit ... ok
test agents::tests::docker_computer ... ok

test result: ok. 140 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.24s

     Running tests/agents.rs (target/debug/deps/agents-3e3bf92970288c0d)

running 1 test
test groups_take_turns_and_agents_work_in_their_folder ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s

     Running tests/bya.rs (target/debug/deps/bya-e3043d92b9939e25)

running 1 test
test chat_with_a_remote_a2a_agent ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s

     Running tests/e2e.rs (target/debug/deps/e2e-36d83ee279d41137)

running 7 tests
test filed_tickets_get_triaged ... ok
test a_plan_must_cover_the_spec ... ok
test a_remote_machine_is_followed_and_driven ... ok
test a_ticket_runs_on_a_cli_worker ... ok
test visual_options_wait_for_the_users_pick ... ok
test flat_project_with_gates_escalation_and_merges ... ok
test nested_leads_review_and_revise_their_reports ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.53s

     Running tests/features.rs (target/debug/deps/features-f89239a6df61383c)

running 17 tests
test a_limit_is_recognised_from_the_clis_own_words_only ... ok
test a_local_model_on_a_custom_router_is_found_by_id ... ok
test a_chosen_local_model_becomes_the_scout_model ... ok
test a_direct_message_between_two_agents_is_its_own_conversation ... ok
test a_reply_to_a_message_that_is_not_there_is_refused ... ok
test accounts_round_trip_and_keep_their_env ... ok
test each_project_remembers_its_own_canvases ... ok
test local_jobs_defaults_to_scouts_and_refuses_unknown_words ... ok
test no_choice_or_an_unknown_runtime_leaves_the_config_alone ... ok
test one_limited_login_does_not_stop_the_project ... ok
test read_messages_hands_back_the_id_a_reply_can_name ... ok
test the_board_message_shape_survives_a_round_trip ... ok
test the_tick_gates_the_local_model_and_scouts_only_follow_their_own ... ok
test a_worker_routes_to_the_local_model_only_while_it_is_listening ... ok
test a_produced_artifact_reaches_the_snapshot_and_is_addressable ... ok
test a_delegated_lookup_reaches_the_chosen_local_model ... ok
test triage_can_run_on_the_local_model_with_no_api_key ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s

     Running tests/interrupted.rs (target/debug/deps/interrupted-f51eb1a30baed879)

running 2 tests
test only_an_interrupted_agent_can_carry_on ... ok
test a_restart_carries_a_cli_worker_on_in_its_own_session ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.15s

     Running tests/resume.rs (target/debug/deps/resume-c3b5cae2affceb3b)

running 1 test
test interrupted_replies_resume ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s

     Running tests/spec_gate.rs (target/debug/deps/spec_gate-94498de447b56d3b)

running 1 test
test the_gate_refuses_twice_then_opens ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s

     Running tests/webhook.rs (target/debug/deps/webhook-7da8c2b7530ee62e)

running 2 tests
test the_phone_cannot_read_agent_or_group_threads ... ok
test a_webhook_wakes_only_its_bot ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s

     Running unittests src/lib.rs (target/debug/deps/backspace_runner-7f3ebfbbcbbefa7b)

running 3 tests
test tests::transcripts_and_base64 ... ok
test tests::codex_stream ... ok
test tests::claude_stream_with_a_tool_call ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/http.rs (target/debug/deps/http-425a39f3299f7e6e)

running 3 tests
test openai_compatible_stream ... ok
test bad_agent_addresses_say_so ... ok
test a2a_card_and_task ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.73s

   Doc-tests backspace_colony

running 1 test
test crates/colony/src/lib.rs - (line 15) - compile ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.34s

   Doc-tests backspace_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests backspace_runner

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

EXIT 0
```

## Change summary (revision 4)

Audit document only. No product code, tests, configuration or PLAN.md changed. The test suite was **not** rerun for this revision, because no code changed since the rerun recorded in §0 and Appendix A.

1. **§7.4 triggers.** All four triggers are now marked missing, and §3 says explicitly that none is implemented as specified. Generic worker escalation, the tool-call loop detector, the turn budget and the after-spend dollar budget are listed separately as adjacent mechanisms. Human handoff stays unknown. The cheap-rung budget row moved from unknown to missing.
2. **Diffs.** The §3.3 row is split. Diff information in deliverables is partial (I, B), backed by the E2E assertions. Symbol-level context is missing; no implementation was found.
3. **Cost-to-green (§1a).** It now covers execution, tool/API, test, CI, gate, retry, escalation and human costs. Routing must weigh the probability that each rung succeeds. The objective is stated as minimizing the total expected cost of a verified result.
4. **Global memory.** Every caller was traced. Agents have no path to global memory. Explicit human writes come from the Memory page and the paired phone. Auto-capture of the user's own chat messages writes global notes before any approval, and that path is reachable (`chat_send` → `capture`, `memory_auto` default on). The finding stays missing for the approval flow, qualified with this evidence.
5. **Acceptance wording.** The §1 result sentence is replaced with the requested wording.
6. **herdr version.** The 0.8.x figure came from PLAN.md, not from a check, and is out of date. The GitHub releases page checked on 2026-10-30 shows v0.9.5 as latest, with the source and its limits stated.

## Change summary (revision 5)

Audit document only. Product code, tests, configuration and PLAN.md are unchanged. The suite was not rerun, because no code changed.

1. New §2a records your §5 inventory (11 entries) and the continuous-chat decision, and counts the matrix rows per entry.
2. The §3 matrix gains a "§5 entry" column. All 52 existing rows are mapped.
3. Ten rows were added for requirements in sections 3–12 that had no matrix row: automatic PR creation; property and edge-case tests; rejection logging; role panes; IDE pane attach; the swappable decision-model interface; and the four M13 items (audit log, observability, failure-mode tests, metrics dashboard). Each new row was checked against the code with the same status rules. None is "works".
4. Five items are flagged for your decision instead of being mapped silently (§2a).
5. The §2 continuous-chat row and the M0 blocker text are updated. M0 is still not approved and M1 has not started.

