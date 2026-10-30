# M1 hardening notes

What the hardening pass changed, what it does not guarantee, and where it differs from `M1-HARDENING-PLAN.md`. Read with `M1-BUDGET-BOUNDARIES.md`. M1 stays pending Charles's approval.

## What is now enforced, and on what evidence

| Claim | Evidence |
|---|---|
| A new project (no `backspace.toml` anywhere) runs under an explicit, enforced policy | `policy_modes.rs`, `policy_cli.rs` (test) |
| A config with no `[policy]`, or with open lists, still runs as before and says so on every open | `policy_modes.rs` (test); the warning is in the Main log, `Harness::policy_notices()` and stderr in `backspace-cli` |
| `[]` denies everything; `mode = "enforced"` needs all four lists | `policy_modes.rs` (test) |
| A call that cannot fit a budget in the worst case is refused before it is sent | `budget_bound.rs`, `budget_reservation.rs`, `budget_e2e.rs` (test, against a mock provider) |
| Concurrent calls cannot jointly pass a budget | `budget_reservation.rs` (many threads) and `budget_e2e.rs` (four agents against a delayed mock) |
| An agent cannot weaken the policy for the next session | `policy_integrity.rs` (bash `sed -i`, redirect, `mv`, rewriting the lock) and `policy_integrity_cli.rs` (a CLI worker, a merged branch) (test) |
| A CLI worker cannot start a turn on a spent task budget, and is stopped when it reports usage past it | `cli_budget.rs` (test, against fake `claude` and `codex` scripts; no real CLI was run) |
| No model call leaves the process without an `Authorization` | `unmetered.rs` (source scan) |
| Plugin refusal | gateway-level unit tests only. No loader until M11; this is not end-to-end evidence |

## Where it is not a hard guarantee

1. **The input bound rests on one assumption: a token covers at least one input byte.** The reservation uses the exact request body in bytes, plus 1,024 for provider-added prompt, plus 8 per message and 16 per tool. No tokenizer was run, so no token count here is measured. For English prose the bound is roughly three to four times the real count; that costs earlier refusals under a tight budget and is the price of a bound. If a tokenizer ever emits more tokens than bytes, an overspend is possible; it would be logged as `overshoot` and counted in full.
2. **Prices are taken from the config.** A wrong price makes the reservation wrong.
3. **The advisor is bounded by assumption, not by the provider.** `advisor.max_output_tokens` (default 4,096) is what a reservation assumes; nothing enforces it.
4. **A Claude Code turn is not stopped within the turn**: it reports usage once, at the end. Codex reports per `turn.completed`, which is what the stop acts on. Within one process the largest figure reported is taken; I did not verify whether Codex's figures are per turn or cumulative, so the sum across processes (earlier turns plus this one's largest) may under- or over-count.
5. **Policy integrity is open until M6.** Nothing in user space makes the file tamper-proof while bash is unsandboxed. What holds: the policy in force comes from a lock outside the workspace (`<data dir>/policy-locks/…`), and a `backspace.toml` that loosens it is ignored with a warning until a person runs `backspace-cli policy adopt`. Around every tool batch and CLI turn the harness puts the file and the lock back if they changed, and says so in the log. The gap: a process an agent leaves running can write later, between actions, and is taken as a person's edit; and a person's own edit made during an agent's action is undone with the agent's. Only a sandbox or a separate OS user closes that.

## Behaviour changes for existing projects

- **Token budgets count raw provider tokens** (cache reads and writes at face value). Before, they counted billing-equivalent tokens (cache reads at 0.1x, writes at 1.25x). Token figures shown for an agent can rise accordingly. Dollar cost is unchanged.
- **Reservations are larger than the old estimate**, so a tight budget refuses earlier. A project with a budget and nothing else changed can see a refusal it did not see before.
- **A dollar limit now fails closed for a plan-billed CLI.** A project with `max_project_usd` (or `[policy.budget] max_usd`) used to run CLI workers and CLI planners unchecked, because their price is $0. They are now refused, with a message naming the model, unless a token limit sits on the same line. The project line carries a dollar limit only, so in practice a project that sets `max_project_usd` cannot run CLI workers. This is the approved F4 behaviour and the part most likely to surprise a project; if it is too strict, the alternative is to count a CLI turn at a configured API-equivalent price.
- **A delivery that edits `backspace.toml` waits for a person**, even with `auto_approve` and even where a manager would normally review it. The summary is prefixed `Changes project policy (backspace.toml):`.
- **A folder with no `backspace.toml` starts enforced.** Skills added to such a folder after its first open are refused with a warning that names them; `backspace-cli policy adopt` accepts them.

## Deviations from the approved plan

1. **New-project orchestration list holds every model, not only API models.** A CLI worker that can spawn tickets counts as an orchestrator, so a list of API models alone would refuse it. `init-config` writes every configured model into both role lists.
2. **No digest in the lock.** The plan said "with a digest". The lock is plain TOML; the restore guard puts it back if it is changed during an action, and an unreadable lock stops the open rather than falling back to the file.
3. **The restore guard runs after each tool batch (the parallel calls of one turn) and each CLI turn**, not after each individual call.
4. **`advisor.max_output_tokens` is a config field** (default 4,096), as planned; the constant behind its default is `gateway::ADVISOR_OUTPUT_TOKENS`.

## Corrections to my own new tests (after the red commit `418f5df`)

Two of the 63 new tests were wrong, not the code. No test that existed before this pass was touched. Both edits are in the green commit's diff.

1. `budget_bound.rs`, `the_bound_is_never_below_the_bytes_of_the_body_that_is_sent`: asserted a bound of at least two tokens per character. For combining marks (1.75 bytes a character) that is more than a token per byte, which is the most any tokenizer can emit under the bound's own assumption. The assertion is now `min(2 x characters, bytes)`: the same claim wherever it can hold, and the byte bound where it cannot. The strict byte assertion above it is unchanged.
2. `budget_reservation.rs`, `a_stopped_call_releases_its_reservation`: expected the second call to be refused with $0.04 left, but a call whose output is clamped to what is left is admitted by design (the stampede test above it relies on that). The limit is now $0.215, which leaves less than the prompt alone, so the second call is refused for the reason the test is about.

Also, not a test change: two F1 tests (`partial_policy_warns_for_each_open_list`, `a_policy_with_every_list_has_nothing_to_warn_about`) failed against my first implementation because it warned about the shipped skills a short list leaves out. The warning now names only skills the project or the user added (`skills_present_but_not_allowed_are_named` still passes with `house-style`). A shipped skill left off a list is silent.

## Not done, on purpose

- No tokenizer or `count_tokens` call (deferred, as proposed; no other provider has one).
- No OS sandbox (M6).
- No plugin end-to-end coverage (M11).
- Nothing from M2 or M3.

Tokens used: not measured. Frontier calls: 0 (the tests use mock providers and fake CLIs).
