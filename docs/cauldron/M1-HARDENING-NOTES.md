# M1 hardening notes

What the hardening passes changed, what they do not guarantee, and where they differ from `M1-HARDENING-PLAN.md`. Read with `M1-BUDGET-BOUNDARIES.md`. Revised after the third review. M1 stays pending Charles's approval; it is not marked approved here.

## Tests

| Set | Tests |
|---|---|
| Tests that existed before the hardening passes (untouched) | 302 |
| First hardening round (red `418f5df`, green `2264263`) | 63 |
| Second hardening round (red `5a57296`, green `2a47e37`) | 17 |
| Third hardening round (red `5cdd77d`) | 47 |
| **Branch suite** | **429** |

The third round's 47: `jev_settlement` 11, `cli_policy` 12, `max_project_tokens` 12, `policy_matrix` 9, `advisor_policy` 3. 38 of them existed at the red commit; 9 were added after it (listed under *Added after red*). Full-suite output: `M1-hardening-3-red-output.txt` (401 passed, 19 failed: 17 of the new tests plus two timing flakes in tests this round does not touch) and `M1-hardening-3-suite-output.txt`.

## What is enforced, and on what evidence

| Claim | Evidence |
|---|---|
| A new project (no `backspace.toml` anywhere) runs under an explicit, enforced policy | `policy_modes.rs`, `policy_cli.rs` (test) |
| A config with no `[policy]`, or with open lists, still runs as before and says so on every open | `policy_modes.rs` (test); the warning is in the Main log, `Harness::policy_notices()` and stderr in `backspace-cli` |
| `[]` denies everything; `mode = "enforced"` needs all four lists | `policy_modes.rs` (test) |
| An API call that cannot fit a budget in the worst case is refused before it is sent, under the assumptions in `M1-BUDGET-BOUNDARIES.md` | `budget_bound.rs`, `budget_reservation.rs`, `budget_e2e.rs` (test, against a mock provider) |
| Concurrent API calls do not jointly pass a budget, under the same assumptions | `budget_reservation.rs` (many threads) and `budget_e2e.rs` (four agents against a delayed mock) |
| Concurrent coding-CLI turns on one token budget are not given the same tokens: the second waits for the first | `cli_concurrency.rs` (unit: shared scope, 12 threads; end to end: three fake Claude workers on `max_project_tokens`, run one at a time, third refused when spent) |
| API calls and CLI turns started together on one project token scope never hold more than its limit | `max_project_tokens.rs` (20 rounds of 16 threads; fails if a CLI admission holds nothing) |
| A CLI worker cannot start a turn on a spent task budget, and Codex is stopped when it reports usage past it | `cli_budget.rs` (test, against fake `claude` and `codex` scripts; no real CLI was run) |
| A dollar limit never admits a CLI turn, priced model or not | `cli_budget.rs`, `cli_concurrency.rs` (test) |
| A Jev routing call is held against the project budget before it is sent; refused, it is not made and the heuristic decides | `router_budget.rs`, `jev_settlement.rs` (test, against a mock Jev) |
| A Jev call's real cost is recorded in the step that gives its hold back: nobody sees the hold gone and the spend missing, and a cost above the estimate is recorded before the room is released | `jev_settlement.rs` (three tests that fail when the recording is moved after the release) |
| Several simultaneous Jev calls cannot jointly pass the project budget | `jev_settlement.rs` (eight threads, room for two) |
| Claude Code's `--advisor` and `--agents` are left off when the project's lists do not name their models, each decided on its own, at the right role | `cli_policy.rs` (test, against a fake `claude` that records its arguments; it shows what Backspace passes, not what Claude Code does with it) |
| The CLI config entry that starts a worker or planner is refused before its turn when the lists do not name it | `cli_policy.rs` (test, with the check removed the tests fail) |
| The API advisor is left off when the lists do not name it for the agent's role | `advisor_policy.rs` (test, mock Anthropic API) |
| Each path states separately whether the budget and whether the policy covers it; every HTTP-sending file is accounted for | `policy_matrix.rs` (words checked against the document and a source scan) |
| `max_project_tokens` is optional, parsed, written back, and shared by API and CLI admissions | `max_project_tokens.rs` (test) |
| No model call leaves the process without an `Authorization` | `unmetered.rs` (source scan) |
| A rewritten `backspace.toml` does not become the next session's policy by itself, within the limits below | `policy_integrity.rs` and `policy_integrity_cli.rs` (test) |
| Each limit is written down | `budget_wording.rs` (the documents contain them; this checks words, not behaviour) |
| Plugin refusal | gateway-level unit tests only. No loader until M11; this is not end-to-end evidence |

## Where it is not a hard guarantee

The classification stays three-way: **reserved**, **admitted**, **unmetered** (`M1-BUDGET-BOUNDARIES.md`). Nothing is called a hard guarantee. Admission and post-call accounting are not enforcement of spend, and the document says which paths have only those.

1. **An API reservation rests on four assumptions:** a token covers at least one input byte (no tokenizer was run, so no token count here is measured), the provider adds no more than the 1,024-token allowance, the configured prices are accurate, and the provider enforces `max_tokens`. If one fails the call can cost more than was reserved; that is logged as `overshoot` and counted in full, not prevented.
2. **Advisor output is not provider-capped.** `advisor.max_output_tokens` (default 4,096) is what a reservation assumes.
3. **Claude Code may overshoot within one reported turn.** It reports usage once, at the end. The overshoot is bounded by that turn, not zero. Codex reports per `turn.completed` and is stopped there.
4. **Codex's cumulative versus per-turn usage semantics are not verified.** Within one process the largest figure reported is taken, so a Codex that reports per turn is undercounted when a process runs several. Only fake CLIs were run.
5. **The unmetered paths remain outside task-budget enforcement:** `CloudChatServer`, `AgentChat` (including its direct Backspace Cloud request), and `JevRouter` where no budget is around the router. A remote machine runs under its own policy and budgets.
6. **A Jev routing call is checked against the project budget only**, because the task has no agent yet. Its hold and its recorded cost now change hands in one step; the 512-token answer is an assumption and Jev's own model is not under the project's lists. A request that fails has an unknown cost and records none.
7. **The restore guard is not an OS-level security boundary.** The policy in force comes from a lock outside the workspace; a `backspace.toml` that loosens it is ignored with a warning until a person runs `backspace-cli policy adopt`; and around every tool batch and CLI turn the harness puts the file and the lock back if they changed. A background process an agent leaves running can write between monitored actions, and that write is taken as a person's edit; a person's own edit made during an agent's action is undone with the agent's. **Policy integrity is open until the M6 sandbox**, or a separate OS user, closes that.

## Third review: findings

Verified against the code before anything was changed.

1. **Jev: a real gap, now closed.** The reservation was held while the request was in flight and released when the decision returned; the cost was added afterwards in `announce_route`. A reservation checked in that interval saw neither the hold nor the spend, by at most one routing call's cost. A cost above the estimate was recorded after the room was released. The restart path for triage (`harness.rs`, the resume branch) routed without calling `announce_route` at all, so its Jev cost was never recorded; the recorder records it now.
2. **Claude Code's advisor and scouts bypassed the lists: a real bypass, now closed.** `run_cli_turn` passed `--advisor opus` and a scout on `haiku` to every Claude Code turn, with no policy check. A project whose lists named neither still ran both.
3. **Chat's Backspace Cloud route is a direct request** that the source scan for `provider::complete(` and friends cannot see. It is unmetered and outside the policy; it was undocumented in that form. It is now a named part of the `AgentChat` row and `policy_matrix.rs` keeps every HTTP-sending file accounted for.
4. **No defect found** in: the CLI config entry checks for workers and planners, Jev's candidate filtering, the API advisor check, or `max_project_tokens` (parsing, round trip, backward compatibility, shared scope). They are now covered by tests that fail when the check is removed.

### Decisions made without Charles

Charles is asleep and cannot accept choices; each of these is the call I would have recommended. Say if any should be reversed.

1. **`max_project_tokens` stays as a supported setting.** Removing it would leave CLI admission on a shared token scope unreachable end to end and would silently drop a limit a project may already have set. The costs of keeping it are in *`max_project_tokens`* in `M1-BUDGET-BOUNDARIES.md`. I did not add a per-turn allowance setting, as instructed.
2. **Claude Code's advisor and scouts are checked against the lists, as written, fail closed.** The advisor at the role of the agent that would consult it, scouts at execution. Both are model invocations Backspace chooses, so leaving them out of the policy was a bypass. The cost: a hand-written enforced list that does not carry `opus` and `haiku` now starts Claude Code without them.
3. **Generated policies carry the config's own aliases.** `effective_policy` (a new project, `policy init`) adds `advisor.cli` to both lists and `advisor.cli_scouts` to the execution list when it writes an open dimension out, so the new check does not switch the advisor and scouts off for every new project. A list a person wrote is never widened. This follows the existing rule that a generated policy allows what the config allows now.
4. **Jev's own model is not put under the lists.** It is router infrastructure configured under `[router]`, not an agent model; gating it would silently turn routing off for any project whose lists do not name `jev_model`. It is documented as *Not covered by the project's policy*, what it may pick is covered, and `backend = "heuristic"` is the supported way to remove it.
5. **No strict-budget mode was invented.** `backend = "heuristic"` exists, is project-wide, is tested, and is documented as the bypass. The documents say the software does not enforce its use for a strict task, and that a pin does not bypass Jev.
6. **API turns keep record-then-release.** It double-counts for an instant, which can only refuse a call, never admit one too many; making it atomic would not change what is enforced.
7. **A billed Jev answer that cannot be used is recorded too** (same synchronization), since the provider charged for it.
8. **Two timing flakes are not fixed.** `spec_gate::the_gate_refuses_twice_then_opens` failed 1 of 12 isolated runs and once in the full red run; `webhook::a_webhook_wakes_only_its_bot` is the known flake, whose fix still waits for Charles's permission. Neither touches code changed here.
9. **M1 stays pending. M2 and M3 were not started.**

### Behaviour changes for existing projects

- **Claude Code's advisor and scouts need the lists to name `opus` and `haiku`** in a hand-written enforced policy. Projects with open lists, and policies Backspace generated, keep both.
- **Every Jev cost is on the books once and in the step that releases its hold.** A project near its limit can refuse an API call slightly earlier than before, and not later.
- Earlier rounds' changes stand: raw-token budgets, larger reservations, a dollar limit refusing every coding-CLI turn, routing calls that can be refused, deliveries that edit `backspace.toml` waiting for a person, and a folder with no `backspace.toml` starting enforced.

## Added after red

Nine tests were written after the red commit. Where a protection can be removed, the test was run with it removed and failed (the billed Jev answer, the generated policy, the two CLI entry checks, the API advisor); the "not widened" and "may create tickets" tests are controls for the other direction and have no failing mutation:

- `jev_settlement`: a billed answer the router cannot use is recorded once.
- `cli_policy`: a generated policy names the aliases; a hand-written list is not widened; a planner and a worker CLI entry that the lists do not name are refused before their turn; a worker that may create tickets is checked as an orchestrator.
- `advisor_policy`: three tests for the API advisor.

## Corrections to my own tests

First round (after red `418f5df`): two of the 63 were wrong, not the code (`budget_bound.rs`, `budget_reservation.rs`); no test that existed before the pass was touched. Second round: none. Third round, after red `5cdd77d`, three of my own new tests were wrong and were fixed; no earlier test was touched:

1. `jev_settlement.rs`, `nobody_sees_the_hold_gone_and_the_spend_not_yet_recorded`: the final "nothing is held" check counted the racing API call's own live reservation. It now drops that reservation first.
2. `jev_settlement.rs`, `jev_calls_completing_among_api_reservations_never_leave_more_spent_than_the_limit`: the expected total included the sizing call made before the run. It now subtracts it.
3. `cli_policy.rs`, `a_workers_advisor_is_checked_at_the_workers_role_not_the_planners`: I assumed a ticket worker is an execution agent, but with the default depth it may create tickets and is checked as an orchestrator. The project now sets `max_depth = 1` for it, and a new test covers the spawning case.

## Deviations from the approved plan

1. **New-project orchestration list holds every model, not only API models.** A CLI worker that can spawn tickets counts as an orchestrator, so a list of API models alone would refuse it.
2. **No digest in the lock.** The lock is plain TOML; the restore guard puts it back if it is changed during an action, and an unreadable lock stops the open rather than falling back to the file.
3. **The restore guard runs after each tool batch (the parallel calls of one turn) and each CLI turn**, not after each individual call.
4. **New knob `max_project_tokens`** (second review), kept in the third.

## Not done, on purpose

- No tokenizer or `count_tokens` call.
- No OS sandbox (M6), no plugin loader (M11), no real provider or real CLI spend tests.
- No setting to switch the advisor, chat or the cloud server off for a strict task; the lists switch the advisor off, nothing else.
- No per-turn CLI allowance setting.
- Nothing from M2 or M3.

Tokens used: not measured. Frontier calls: 0 (the tests use mock providers, a mock Jev and fake CLIs).
