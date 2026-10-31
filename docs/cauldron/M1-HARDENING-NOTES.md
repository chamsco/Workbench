# M1 hardening notes

What the hardening pass changed, what it does not guarantee, and where it differs from `M1-HARDENING-PLAN.md`. Read with `M1-BUDGET-BOUNDARIES.md`. Revised after the second review. M1 stays pending Charles's approval.

## Tests

| Set | Tests |
|---|---|
| Tests that existed before the hardening pass (untouched) | 302 |
| First hardening round (red `418f5df`, green `2264263`) | 63 |
| Second hardening round (red `5a57296`) | 17 |
| **Branch suite** | **382** |

## What is enforced, and on what evidence

| Claim | Evidence |
|---|---|
| A new project (no `backspace.toml` anywhere) runs under an explicit, enforced policy | `policy_modes.rs`, `policy_cli.rs` (test) |
| A config with no `[policy]`, or with open lists, still runs as before and says so on every open | `policy_modes.rs` (test); the warning is in the Main log, `Harness::policy_notices()` and stderr in `backspace-cli` |
| `[]` denies everything; `mode = "enforced"` needs all four lists | `policy_modes.rs` (test) |
| An API call that cannot fit a budget in the worst case is refused before it is sent, under the assumptions in `M1-BUDGET-BOUNDARIES.md` | `budget_bound.rs`, `budget_reservation.rs`, `budget_e2e.rs` (test, against a mock provider) |
| Concurrent API calls do not jointly pass a budget, under the same assumptions | `budget_reservation.rs` (many threads) and `budget_e2e.rs` (four agents against a delayed mock) |
| Concurrent coding-CLI turns on one token budget are not given the same tokens: the second waits for the first | `cli_concurrency.rs` (unit: shared scope, 12 threads; end to end: three fake Claude workers on `max_project_tokens`, run one at a time, third refused when spent) |
| A CLI worker cannot start a turn on a spent task budget, and Codex is stopped when it reports usage past it | `cli_budget.rs` (test, against fake `claude` and `codex` scripts; no real CLI was run) |
| A dollar limit never admits a CLI turn, priced model or not | `cli_budget.rs`, `cli_concurrency.rs` (test) |
| A Jev routing call is held against the project budget before it is sent; refused, it is not made and the heuristic decides | `router_budget.rs` (test, against a mock Jev) |
| No model call leaves the process without an `Authorization` | `unmetered.rs` (source scan) |
| A rewritten `backspace.toml` does not become the next session's policy by itself, within the limits below | `policy_integrity.rs` (bash `sed -i`, redirect, `mv`, rewriting the lock) and `policy_integrity_cli.rs` (a CLI worker, a merged branch) (test) |
| Each limit is written down | `budget_wording.rs` (the documents contain them; this checks words, not behaviour) |
| Plugin refusal | gateway-level unit tests only. No loader until M11; this is not end-to-end evidence |

## Where it is not a hard guarantee

The classification stays three-way: **reserved**, **admitted**, **unmetered** (`M1-BUDGET-BOUNDARIES.md`). Nothing is called a hard guarantee. Admission and post-call accounting are not enforcement of spend, and the document says which paths have only those.

1. **An API reservation rests on four assumptions:** a token covers at least one input byte (no tokenizer was run, so no token count here is measured), the provider adds no more than the 1,024-token allowance, the configured prices are accurate, and the provider enforces `max_tokens`. If one fails the call can cost more than was reserved; that is logged as `overshoot` and counted in full, not prevented.
2. **Advisor output is not provider-capped.** `advisor.max_output_tokens` (default 4,096) is what a reservation assumes.
3. **Claude Code may overshoot within one reported turn.** It reports usage once, at the end. The overshoot is bounded by that turn, not zero. Codex reports per `turn.completed` and is stopped there.
4. **Codex's cumulative versus per-turn usage semantics are not verified.** Within one process the largest figure reported is taken, so a Codex that reports per turn is undercounted when a process runs several. Only fake CLIs were run.
5. **The unmetered paths remain outside task-budget enforcement:** `CloudChatServer`, `AgentChat`, and `JevRouter` where no budget is around the router. A remote machine runs under its own policy.
6. **A Jev routing call is checked against the project budget only**, because the task has no agent yet. Its hold is released when the decision returns and its cost is recorded just after, so another reservation can miss it by at most one routing call's cost. Router tokens are not counted in `max_project_tokens`.
7. **The restore guard is not an OS-level security boundary.** The policy in force comes from a lock outside the workspace (`<data dir>/policy-locks/…`); a `backspace.toml` that loosens it is ignored with a warning until a person runs `backspace-cli policy adopt`; and around every tool batch and CLI turn the harness puts the file and the lock back if they changed. A background process an agent leaves running can write between monitored actions, and that write is taken as a person's edit; a person's own edit made during an agent's action is undone with the agent's. **Policy integrity is open until the M6 sandbox**, or a separate OS user, closes that.

## What the second review found and changed

1. **Concurrent CLI turns got the same allowance.** `admit_cli` computed a token ceiling and held nothing. It now holds the ceiling against every scope the turn counts toward until the turn ends, and a second turn on the same budget waits for the release, then asks again.
2. **There was no token scope two CLI workers could share.** The per-task limit is per agent and the project limit was dollars only, so the hole could not be reached end to end. To test and close it I added `[orchestrator] max_project_tokens`, next to `max_project_usd`. This is a new config knob; if you do not want a project token limit it can come out, and the unit tests on the shared scope key stay.
3. **The cost of that fix, stated plainly.** A turn holds *everything* the tightest limit has left. So on a shared token budget coding-CLI turns run one at a time, and an API call on that budget is refused (not queued) while a CLI turn runs. The remedy I would propose, not built: an allowance per turn (a `cli_turn_tokens` setting), so turns can overlap and API calls keep room. It needs your decision because a turn that outgrows its allowance is stopped (Codex) or overshoots (Claude Code).
4. **A configured price did not make a dollar limit bind a CLI.** A priced CLI model with `max_project_usd` was admitted and its turns added no dollars, so the limit was never reached. A dollar limit now refuses every CLI turn that has no token limit on the same line.
5. **Jev was counted after the fact only.** Now held before it is sent (above). A router with no budget around it still declares `Unmetered(JevRouter)`, so the three-way list is unchanged.

## Behaviour changes for existing projects

- **Token budgets count raw provider tokens** (cache reads and writes at face value). Before, they counted billing-equivalent tokens (cache reads at 0.1x, writes at 1.25x). Token figures shown for an agent can rise accordingly. Dollar cost is unchanged.
- **Reservations are larger than the old estimate**, so a tight budget refuses earlier.
- **A dollar limit fails closed for every coding CLI.** A project with `max_project_usd` (or `[policy.budget] max_usd`) used to run CLI workers and CLI planners unchecked. They are now refused, with a message naming the model, unless a token limit sits on the same line. The project line carries a dollar limit unless `max_project_tokens` is set beside it, so in practice a project that sets only `max_project_usd` cannot run CLI workers. The alternative, if that is too strict, is to count a CLI turn at a configured API-equivalent price after the fact; that would be post-call accounting only.
- **Routing calls can be refused.** With a project budget, a Jev call that does not fit is not made and the heuristic decides, with a note in the log.
- **A delivery that edits `backspace.toml` waits for a person**, even with `auto_approve` and even where a manager would normally review it. The summary is prefixed `Changes project policy (backspace.toml):`.
- **A folder with no `backspace.toml` starts enforced.** Skills added to such a folder after its first open are refused with a warning that names them; `backspace-cli policy adopt` accepts them.

## Deviations from the approved plan

1. **New-project orchestration list holds every model, not only API models.** A CLI worker that can spawn tickets counts as an orchestrator, so a list of API models alone would refuse it.
2. **No digest in the lock.** The lock is plain TOML; the restore guard puts it back if it is changed during an action, and an unreadable lock stops the open rather than falling back to the file.
3. **The restore guard runs after each tool batch (the parallel calls of one turn) and each CLI turn**, not after each individual call.
4. **New knob `max_project_tokens`** (second review, above).

## Corrections to my own new tests

First round (after red `418f5df`): two of the 63 were wrong, not the code. No test that existed before the pass was touched.

1. `budget_bound.rs`, `the_bound_is_never_below_the_bytes_of_the_body_that_is_sent`: asserted at least two tokens per character. For combining marks (1.75 bytes a character) that is more than a token per byte, the most any tokenizer can emit under the bound's own assumption. It is now `min(2 x characters, bytes)`; the strict byte assertion above it is unchanged.
2. `budget_reservation.rs`, `a_stopped_call_releases_its_reservation`: expected a refusal with $0.04 left, but a call clamped to what is left is admitted by design. The limit is now $0.215, which leaves less than the prompt alone.

Also, not a test change: the F1 warning now names only skills the project or the user added, not the shipped ones a short list leaves out.

Second round: none yet. The 17 tests are as committed in red `5a57296`.

## Not done, on purpose

- No tokenizer or `count_tokens` call (deferred; no other provider has one).
- No OS sandbox (M6).
- No plugin end-to-end coverage (M11).
- No setting to switch the advisor, chat or the cloud server off for a strict task: proposed in `M1-BUDGET-BOUNDARIES.md`, not built.
- Nothing from M2 or M3.

Tokens used: not measured. Frontier calls: 0 (the tests use mock providers, a mock Jev and fake CLIs).
