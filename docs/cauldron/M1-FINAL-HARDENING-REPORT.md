# M1 final hardening: report

Branch `claude/project-thread-dpmr8e`. Red `52922d0f03a2764048b0ac2172d68d2795d2df7d`, green `b36ad3ecd0e3166b3941ee012d5e796a3302fbaa` (both from `git rev-parse`). **M1 is not approved; I did not approve it.** The recommendation is in section K. Nothing from M2 or M3 was started.

The objective was to make the implementation, the policy, the budget accounting, the tests and the documents tell the same truth. Where they could not, the document now says so.

## A. Baseline

- Start: branch HEAD `331e9a1` (docs-only commit on top of green `b2a73af`), clean tree, 429 tests passing per the previous report.
- Confirmed again by the red run: 436 passed = those 429 plus 7 of the new tests that pass on arrival (controls). Nothing older changed state.
- Not built here, as before: the GPUI and Tauri app crates.

## B. Findings

Checked against the code before any change.

| # | Finding | Verdict |
|---|---|---|
| 1 | A Jev request sent and then failed with no reported cost released its hold and recorded nothing; a good answer with no usage was recorded as free | **Real gap.** Closed |
| 2 | Nothing said which model a coding CLI ran: a CLI entry's `model` was never passed to the CLI and the lists never saw it | **Real gap.** Closed for Claude Code and Codex; others count as unpinned |
| 3 | The planner in a CLI was policy-checked as the first config entry on that CLI, not the one it ran as | **Real defect** found while auditing. Fixed |
| 4 | Jev's own model had no policy of its own (owner decision C) | Closed |
| 5 | Advisor and scout gating, `max_project_tokens`, policy-integrity guard | No defect. Re-verified with tests that fail when the check is removed |
| 6 | `spec_gate` and `webhook` flakes | Both test-side; fixed test-only (section H) |

## C. Changes

- `gateway.rs`: `routing_models` and `unpinned_cli_models` in `PolicyConfig` (missing list = closed under `enforced`, open otherwise); `check_routing_model`, `check_cli_model`, `enforced()`; two new `Refusal` variants; both lists compared when a policy file is adopted (adding is a loosening); open-time notices.
- `router.rs`: routing-model check before the key and the reservation; the call is split into *never left* (release), *known cost* (record, release) and *unknown cost* (count at the reservation, marked, release), all in `Reservation::settle`'s locked step; `with_unknown_cost_recorder`, `with_jev_timeout`.
- `harness.rs`: unknown-cost recorder (project total, router total, `router_unknown_cost_usd`, metrics ledger); `cli_model` pin/exception check for workers and planners; `planner_entry`; the pinned model goes on the command line only under an enforced policy.
- `config.rs`: `cli_pin`, `PINNABLE_CLIS` (`claude`, `codex`), generated policies name the router model, the unpinned CLI entries and pinned models; `policy init` prints both lists.
- `project.rs`: `router_unknown_cost_usd` (persisted with the other spend, `serde(default)`).
- Docs: `M1-BUDGET-BOUNDARIES.md` rewritten (four-column matrix, 12 rows, Jev outcomes, routing policy, CLI model selection); `M1-HARDENING-NOTES.md` restructured into controls, recommendations, assumptions, limitations, future work.
- Tests: new `jev_unknown_cost` 15, `routing_policy` 18, `cli_model_policy` 23; `policy_matrix` +5; `policy_integrity_cli` +1. Existing files changed only as listed under *Changes to existing tests* in the notes.

## D. JevRouter

**Invariant.** From before the request is sent until its real cost, or its reservation counted as an unknown cost, is on the project's books, the hold and the spend are never both missing and never inconsistent.

| Outcome | What happens | Test |
|---|---|---|
| Failed before dispatch (connection refused, not a URL) | hold released, nothing recorded, no "unknown" note | `a_request_that_never_left_…`, project-level control |
| Succeeded, cost known | recorded in full (also above the reservation), hold released in the same step | `jev_settlement.rs` |
| Failed after dispatch, cost unknown (timeout, cut connection, 5xx, non-JSON) | **not freed as zero**: the reservation is recorded as spent, marked unknown, in the same step; never counted twice | `a_timeout_…`, `a_connection_cut_…`, `an_error_status_…`, `a_body_that_is_not_json_…` |
| Succeeded, no usage reported | answer used, counted at the reservation | `a_usable_answer_with_no_usage_…` |
| Failed after dispatch, cost known | recorded at that cost, hold released in the same step | `a_failure_that_reports_its_cost_…`, `…above_the_hold_…` |

Also tested: a later request cannot use the retained reservation; a retry neither frees nor double-counts; eight Jev calls in different outcomes against eight API reservations never pass the limit and add up exactly; an API reservation made the instant an unknown cost is being recorded sees the recorded spend; the project's state shows `router_unknown_cost_usd` once. Release-then-record mutation: 5 tests fail (3 + 2).

Routing-model policy (decision C): `routing_models` is separate from the role lists; under `enforced` it must name `jev_model` or no request is made, no key is read and nothing is reserved, and the heuristic decides with a note and an open-time warning. Naming Jev in the role lists is not a permission and `routing_models` does not widen Jev's candidates. Generated policies name the configured model. Legacy and open are unchanged.

Strict-budget semantics: `backend = "heuristic"` is the project-wide bypass (not one request reaches Jev, tested); a pin is not a bypass (tested); no per-task strict mode; Jev output size is not provider-capped; Jev reservations count against the project dollar budget; Jev's tokens do not count toward `max_project_tokens`; a router with no budget is unmetered. The budget table's Jev rows and the strict section are checked against each other by `policy_matrix.rs`.

## E. CLI policy

Audit of every place Backspace starts a coding CLI or chooses a model for one:

| Path | Before | Now (enforced policy) |
|---|---|---|
| Worker (`finish_cli`) | entry id checked; model unconstrained | entry checked; pinned model checked and passed (`--model` / `-m`); unpinned needs `unpinned_cli_models`; checked at the worker's role |
| Planner (`plan_on_cli`) | entry checked, but the wrong entry when two share a CLI | the entry it runs as; same pin rules; orchestration role |
| Claude Code `--advisor` | gated (third round) | unchanged, retained |
| Claude Code scout (`--agents`) | gated | unchanged, retained |
| Env (`ANTHROPIC_MODEL`), project/personal CLI config | not looked at | the pinned model is on the command line, which Claude Code documents as taking precedence; tested that the argument is the pinned model with the variable set |
| Later edit of `backspace.toml` | n/a | the model comes from the config in memory; an edit after open is not used (tested) |
| Legacy / open | no flags | unchanged (tested) |

What I verified in the vendors' documentation: Claude Code: `--model` is above `ANTHROPIC_MODEL` and the settings `model` field. Codex: command-line arguments are above a profile and `config.toml`. What I could not verify: the documentation I read names `--model` for Codex, while the runner has always passed `-m`; I did not check the short flag against a real Codex. Cursor, Grok and OpenCode have no verified precedence, so a pin on them counts as unpinned and needs the exception.

State plainly: the fake-CLI tests prove what Backspace passes, not how the real CLI interprets it. An alias such as `sonnet` is resolved by the CLI and its settings can remap it; a managed or organization setting can replace a model; pin full model ids.

## F. Policy and budget matrix

Full table, four columns, in `M1-BUDGET-BOUNDARIES.md`. Condensed:

| Path | Budget | Policy | Main remaining limitation |
|---|---|---|---|
| API turns (`drive`) | reserved | orchestration/execution allowlist | the four reservation assumptions |
| API advisors | reserved, assumption only | allowlist at the caller's role, else left off | output not provider-capped |
| CLI workers | admitted | entry + pinned model against the allowlist; unpinned only by exception | overshoot within a Claude Code turn; Codex semantics; fake CLIs |
| CLI planners | admitted | same, orchestration | same |
| Claude Code advisors | admitted, inside the CLI | allowlist, else `--advisor` left off | not a call this process sees |
| Claude Code scouts | admitted, inside the CLI | execution allowlist, else left off | same |
| JevRouter's own model | reserved with a budget; unmetered without | `routing_models` | unknown cost counted at the reservation; tokens not counted toward `max_project_tokens` |
| JevRouter's candidates | the pick's own path | role lists; never widened | none beyond the above |
| CloudChatServer | unmetered | not covered | not enforced |
| AgentChat (CLI, local model, router, Backspace Cloud) | unmetered | not covered | not enforced; the cloud route is a direct request the name scan cannot see |
| Remote harnesses | out of this process | the remote machine's own policy | not enforced from here |
| Plugins | no call site yet | gateway-level only | no loader until M11 |

## G. `max_project_tokens`

Reconfirmed by tests that run unchanged: optional whole number under `[orchestrator]`; configs without it load; it is written back; the shipped example documents it commented out; one `project` scope shared by API reservations, CLI admissions and Jev holds; 20 rounds of 16 threads never hold more than the limit; an API call is refused, not queued, while a CLI turn holds the tokens; Jev's tokens are not counted toward it (`jev_settlement.rs`); a dollar limit alone still refuses every CLI turn (fail closed); a project that does not set it is unaffected. The trade-offs (CLI turns one at a time, API refused rather than queued, Claude Code overshoot, router excluded) are in the boundaries document. `cli_turn_tokens` was not introduced.

## H. Flaky tests

| Test | Before | Classification | Fix | After |
|---|---|---|---|---|
| `spec_gate::the_gate_refuses_twice_then_opens` | 2 failures in 60 isolated runs; both `a Plan reply with a spec gets a verdict` | timing: `is_busy` clears before the verdict is attached (`chat.rs` removes the `running` entry before `finish` and `file_spec`) | test waits for the verdict itself, same 30 s limit | 120 of 120 under four parallel loops |
| `webhook::a_webhook_wakes_only_its_bot` | 0 of 60 failed alone; 8 of 80 failed with the file's other test | shared state: both tests set the process-wide `BACKSPACE_DATA` and the fleet reads it again | a static mutex serialises the two tests | 150 of 150 |

No webhook behaviour and no `chat.rs` code was changed. For the owner: `spawn_reply` should clear the busy flag after the verdict is attached, and it sets the flag after spawning, so a reply that finishes first leaves it stuck. Not changed; outside M1.

## I. Test results

Commands (repo root): `cargo test --no-fail-fast`; per file, `cargo test -p backspace-core --test <name>`.

| Run | Commit | Exit | Passed | Failed |
|---|---|---|---|---|
| Red | `52922d0` | 101 | 436 | 53 |
| Green 1 | `b36ad3e` | 0 | 491 | 0 |
| Green 2 | `b36ad3e` | 0 | 491 | 0 |
| Green 3 | `b36ad3e` | 0 | 491 | 0 |
| Green 4 | `b36ad3e` | 0 | 491 | 0 |
| Green 5 | `b36ad3e` | 0 | 491 | 0 |

Five consecutive green runs on an unmodified tree, with identical per-binary result lines in all five (`M1-final-suite-output.txt`). Both flaky tests were in those runs and passed every time.

- New tests this pass: **62** (`jev_unknown_cost` 15, `routing_policy` 18, `cli_model_policy` 23, `policy_matrix` +5, `policy_integrity_cli` +1). Branch total 302 + 63 + 17 + 47 + 62 = **491**.
- Red `52922d0`: 436 passed, 53 failed (all in the four new or reworked files, each for its own reason; one, the Cursor test, failed for a fixture reason first and is listed under corrections). Green: see the table. The two harness-level unknown-cost tests were added after red; one was run red first.
- Preserved: every older test is unchanged except the three enforced policies that now name `unpinned_cli_models` or `routing_models`, and the two flaky-test fixes. No assertion was weakened; none deleted or skipped. Listed with reasons in the notes.
- Corrected: my own Cursor fixture (binary is `cursor-agent`).
- Mutation checks: 13, each fails at least one test (table in the notes).
- Skipped / ignored: none. Not built: the GPUI and Tauri app crates.
- **Real provider, CLI or Jev usage: none.** Mock providers, a mock Jev, fake `claude`, `codex` and `cursor-agent` scripts. Tokens: not measured. Frontier calls: 0.
- What the tests cannot tell you: that a real provider enforces `max_tokens`; that real Claude Code or Codex honour the model argument over their settings; Codex's usage semantics; Claude Code's end-of-turn report; a real Jev's usage reporting and billing for a failed request; the token-per-byte bound; the price tables.

## J. Remaining risks

1. Everything in *Assumptions* and *Known limitations* in the notes; the weakest are the unverified `-m` for Codex and the unmeasured token-per-byte bound.
2. Existing enforced policies stop calling Jev and refuse unpinned CLI entries until regenerated; the project says so when it opens, but a person who does not read it will find Jev quietly off.
3. An unknown-cost Jev call is counted at its reservation, which may exceed or fall short of the truth; a flaky network near a project limit can therefore use up room.
4. Policy integrity is limited: a background process, or deleting the lock, is not prevented (decision D; M6).
5. A pin on a CLI the real tool resolves differently (an alias, a managed setting) is not caught.
6. `chat.rs` busy-flag ordering (not M1).

## K. M1 recommendation

**Recommend approval, with these accepted by the owner, not by me:** the ten assumptions and ten limitations in `M1-HARDENING-NOTES.md`, in particular that the policy and budget claims are tested against mocks and fake CLIs only, that policy integrity is not an OS-level boundary until M6, and that older enforced policies need regenerating. I know of no code blocker. If the owner wants any of the following before approving, M1 should stay pending: a run against real Claude Code and Codex to confirm `--model` / `-m` precedence, or a per-turn CLI allowance so CLI turns can overlap.
