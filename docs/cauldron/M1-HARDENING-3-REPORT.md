# M1 hardening, third review: final report

Branch `claude/project-thread-dpmr8e`. Red `5cdd77ddeb4c80ed31e289126f799d90f4ffe4a8`, green `b2a73af0c8110225032fdb4b5402804a0cbdba75` (both checked with `git rev-parse`). **M1 is not approved; it stays pending.** Nothing from M2 or M3 was started.

Charles was asleep and could not accept choices, so each decision below is the call I would have recommended. The reasoning is in `M1-HARDENING-NOTES.md` under *Decisions made without Charles*.

## 1. Findings

Every finding was checked against the code first.

| # | Finding | Verdict |
|---|---|---|
| 1 | Jev: hold released when the decision returned, cost added later in `announce_route` | **Real gap.** A reservation checked in between saw neither hold nor spend (up to one routing call's cost); a cost above the estimate was recorded after the room was released. Closed. |
| 1b | Triage routing on the restart path never called `announce_route` | **Real, small.** Its Jev cost was never recorded. Recorded now (side effect of the fix). |
| 2 | Claude Code `--advisor opus` and the `haiku` scout passed to every turn with no policy check | **Real bypass.** Closed: looked up in the lists, left off with a log line when not named. |
| 3 | Chat's Backspace Cloud route is a direct request the `unmetered.rs` name scan cannot see | **Real documentation/audit gap.** Now a named part of the `AgentChat` row, and a source scan accounts for every HTTP-sending file. |
| 4 | CLI worker/planner entry checks, Jev candidate filtering, API advisor check, `max_project_tokens` | **No defect.** Verified, now covered by tests that fail when the check is removed. |

## 2. Changes

- `gateway.rs`: `Reservation::settle(record)` runs the recording under the reservation table's lock and removes the hold in the same lock hold. `Drop` shares the release code.
- `router.rs`: `Router::with_recorder`, `Decision::cost_settled`. `jev()` settles right after the response is parsed, so a billed but unusable answer is recorded too. A cost above the hold is named in the decision note (`jev overshoot`).
- `harness.rs`: the router's recorder adds the cost to the router and project totals; `announce_route` skips adding it again. `run_cli_turn` takes the agent's role and passes `--advisor` / `--agents` only if `check_model` allows them.
- `config.rs`: `effective_policy` (new projects, `policy init`) names `advisor.cli` in both lists and `advisor.cli_scouts` in the execution list. Hand-written lists are never widened.
- Docs: `M1-BUDGET-BOUNDARIES.md` rewritten (separate *Project policy check* column, Jev section, `max_project_tokens` section, strict budgets split in three); `M1-HARDENING-NOTES.md` updated.
- Tests: `jev_settlement.rs`, `cli_policy.rs`, `max_project_tokens.rs`, `policy_matrix.rs`, `advisor_policy.rs`. No earlier test was touched.

## 3. JevRouter

**Invariant.** From before the request is sent until the real cost is on the project's books, the hold and the spend are never both missing and never inconsistent: recording and release happen under one lock that every other reservation also takes.

Answers to the review's questions:

| Question | Before | Now |
|---|---|---|
| Is the reservation held for the whole period Jev can incur cost? | No: released at return | Yes, until recorded. Not for a failed request, whose cost is unknown and records none |
| A window between release and recording? | Yes | No |
| Could concurrent completions let another API reservation pass first? | Yes | No |
| Could a reservation observe held and spent inconsistently? | Yes | No |
| Is a cost above the reservation recorded before the room is released? | No | Yes, in full, and noted |

**Concurrency tests** (`jev_settlement.rs`, 11 tests; each drives a real `Router` and a real reservation table against a mock Jev):

- eight simultaneous calls, room for two: exactly two requests sent, six refused to the heuristic, spend recorded once each;
- a racing API reservation made the instant recording starts must see the recorded spend (recording slowed to 300 ms);
- a cost at 90% of its hold, then 95% in a mixed run of eight Jev and eight API calls: total spent never passes the limit, nothing lost, nothing counted twice;
- a billed answer the router cannot use is recorded once.

Mutation check: with `settle` changed to release then record, three of the four settlement tests fail. The fourth, the eight-simultaneous test, still passes: it guards the pre-call hold and exactly-once recording, not the gap.

**Strict-budget status.** `[router] backend = "heuristic"` is a supported project-wide bypass; with it no request reaches Jev (tested). It is not per task and nothing enforces its use. A pin does not bypass Jev (Jev is asked first, the pin overrides its answer; tested). No implicit strict mode was added. Jev is **not** described as a fully bounded path: its 512-token answer is an assumption, and Jev's own model is outside the project's lists.

## 4. Policy matrix

Two independent questions per path. The full table is in `M1-BUDGET-BOUNDARIES.md`.

| Path | Budget | Project policy check |
|---|---|---|
| API agent turn (`drive`) | reserved | allowlist for the agent's role (orchestration: main, planners, ticket-creating agents; execution: other workers, triage, scouts) |
| Advisor on an API turn | reserved, assumption only (output not provider-capped) | allowlist at the calling agent's role; left off if not named |
| CLI worker turn (`finish_cli`) | admitted | config entry checked against execution (orchestration if it may create tickets); the model the CLI runs: *the CLI's own configuration, not independently by the gateway* |
| CLI planner (`plan_on_cli`) | admitted | config entry against orchestration; model by the CLI's own configuration |
| Claude Code's `--advisor` | inside the CLI | **now** allowlist at the agent's role, as written (`opus`); left off if not named |
| Claude Code's scout (`--agents`) | inside the CLI | **now** allowlist at execution (`haiku`); left off if not named |
| JevRouter | reserved with a project budget, else unmetered | Jev's own model: not covered by the project's policy; what it may pick: candidates the lists allow |
| CloudChatServer | unmetered, not enforced | not covered by the project's policy |
| AgentChat (CLI, local model, router, Backspace Cloud) | unmetered, not enforced | not covered by the project's policy |
| Remote machine's harness | out of this process | runs under the remote machine's own policy |
| Plugins | no call site yet | gateway-level check only; no end-to-end loader exists yet |

Bypass check result: CLI workers and planners could not choose a model past the lists by their config entry (verified); Claude Code's advisor and scouts could, and no longer can (fixed with a regression test); Jev could pick only offered candidates (verified).

## 5. `max_project_tokens`

Confirmed by tests: optional whole number under `[orchestrator]`; configs without it load unchanged; survives a write-back; the shipped example documents it commented out; it is the `project` scope that API reservations, CLI admissions and Jev holds share; spent tokens of finished calls count against the next; started together, API calls and CLI turns never hold more than the limit (20 rounds of 16 threads; fails if a CLI admission holds nothing); an API call is refused, not queued, while a CLI turn holds the tokens.

Trade-offs of keeping it, all in the docs: CLI turns on the shared token budget run one at a time; API agents are refused mid-turn instead of waiting; a project that does not set it is unaffected; Claude Code can pass the limit by one turn; router tokens are never counted; a dollar limit alone still refuses every CLI turn. No `cli_turn_tokens` was introduced.

## 6. Tests

Commands (repo root): `cargo test --no-fail-fast`; per file, `cargo test -p backspace-core --test <name> --no-fail-fast`.

| | Pass | Fail |
|---|---|---|
| Red `5cdd77d` | 401 | 19 (17 new tests failing for their reason; `spec_gate::the_gate_refuses_twice_then_opens` and `webhook::a_webhook_wakes_only_its_bot`, timing flakes in untouched code) |
| Green `b2a73af` | **429** | **0** |

Counts: 302 older + 63 + 17 + **47 new this round** = 429. The 47: `jev_settlement` 11, `cli_policy` 12, `max_project_tokens` 12, `policy_matrix` 9, `advisor_policy` 3. 38 existed at red; 9 were added after (each run with the protection removed where one can be). Of the 38 at red, 17 failed (cli_policy 4, jev_settlement 4, policy_matrix 9); the other 21 passed because they verify behaviour that already worked (`max_project_tokens` 12, controls). Three of my own new tests were wrong and were fixed after red (listed in the notes); no earlier test was edited.

Skipped / not run: none skipped. The GPUI and Tauri app crates are not built in this environment. **No real provider, no real CLI and no real Jev was called:** everything uses mock providers, a mock Jev and fake `claude`/`codex` scripts, so these tests show what Backspace sends and records, not what Claude Code, Codex or a provider then does. Tokens used: not measured. Frontier calls: 0.

Gap between the documented guarantee and what tests establish: byte-per-token and the 1,024 overhead are unmeasured; Codex usage semantics and Claude Code's end-of-turn reporting are unverified against the real tools; provider enforcement of `max_tokens` is assumed; the advisor and Jev answer sizes are assumptions; the restore guard is tested against shell edits, not against a determined background process.

## 7. Remaining risks

- A hand-written enforced policy now starts Claude Code without its advisor and scout unless it lists `opus` and `haiku`.
- Jev's own model, `AgentChat`, `CloudChatServer`, remote machines: outside the project's policy; the first three also outside the budget.
- Plan-billed CLI spend is observed, not bounded; Claude Code can overshoot by one turn.
- A failed Jev request has an unknown cost and records none.
- Policy integrity is open until the M6 sandbox.
- Two timing flakes remain in untouched tests; the webhook fix still needs Charles's permission.

## 8. M1 recommendation

**Keep M1 pending.** Blockers for approval:

1. Charles to confirm three calls made for him: keep `max_project_tokens`; fail-closed advisor/scout gating (with generated policies naming the aliases); leave Jev's own model outside the lists.
2. Policy integrity is not an OS-level boundary and stays open until M6; if that must be closed before approval, M1 cannot be approved.
3. No evidence from a real CLI or provider (Codex usage semantics, Claude Code overshoot, token-per-byte); approval means accepting the assumptions in `M1-BUDGET-BOUNDARIES.md`.
4. The two flaky tests, and Charles's answer on fixing the webhook one.

Parked from M2, still waiting on Charles: the task texts for the §10 eval traces and his answers on the frontier-definition proposal.
