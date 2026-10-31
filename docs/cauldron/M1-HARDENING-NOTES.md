# M1 hardening notes

What the hardening passes changed, what is claimed and on what evidence, what is only assumed, what is known not to be done, and where they differ from `M1-HARDENING-PLAN.md`. Read with `M1-BUDGET-BOUNDARIES.md`. Revised in the final hardening pass. **M1 is not approved; it stays pending the owner's decision.** Nothing here marks it approved.

The five sections that matter are kept apart on purpose: *Implemented and tested controls*, *Operational recommendations*, *Assumptions*, *Known limitations* and *Future work*. A line in one is not evidence for a line in another.

## Tests

| Set | Tests |
|---|---|
| Tests that existed before the hardening passes (untouched) | 302 |
| First hardening round (red `418f5df`, green `2264263`) | 63 |
| Second hardening round (red `5a57296`, green `2a47e37`) | 17 |
| Third hardening round (red `5cdd77d`, green `b2a73af`) | 47 |
| Final hardening pass (red `52922d0`) | 62 |
| **Branch suite** | **491** |

The final pass's 62: `jev_unknown_cost` 15, `routing_policy` 18, `cli_model_policy` 23, `policy_matrix` 5 more (9 to 14), `policy_integrity_cli` 1 more (4 to 5). 60 of them existed at the red commit; 2 (the harness-level unknown-cost tests) were added after it. Full-suite outputs: `M1-final-red-output.txt` (436 passed, 53 failed: 20 `cli_model_policy`, 11 `jev_unknown_cost`, 11 `policy_matrix`, 11 `routing_policy`), `M1-final-suite-output.txt` (the green runs), `M1-final-flaky-evidence.txt`. Earlier rounds' outputs: `M1-hardening-3-*.txt`.

## Implemented and tested controls

Each row has a test that fails when the control is removed unless the row says otherwise. All are against mock providers, a mock Jev and fake `claude` / `codex` / `cursor-agent` scripts: they show what Backspace sends, records and refuses, not what a real provider, Jev or CLI then does.

| Claim | Evidence |
|---|---|
| A new project (no `backspace.toml` anywhere) runs under an explicit, enforced policy that names its router model and its unpinned CLI entries | `policy_modes.rs`, `routing_policy.rs`, `cli_model_policy.rs` |
| A config with no `[policy]`, or with open lists, still runs as before and says so on every open | `policy_modes.rs`; the warning is in the Main log, `Harness::policy_notices()` and stderr in `backspace-cli` |
| `[]` denies everything; `mode = "enforced"` needs the four original lists; a missing `routing_models` or `unpinned_cli_models` in an enforced policy allows nothing | `policy_modes.rs`, `routing_policy.rs`, `cli_model_policy.rs` |
| An API call that cannot fit a budget in the worst case is refused before it is sent, under the assumptions below | `budget_bound.rs`, `budget_reservation.rs`, `budget_e2e.rs` |
| Concurrent API calls do not jointly pass a budget, under the same assumptions | `budget_reservation.rs`, `budget_e2e.rs` |
| Concurrent coding-CLI turns on one token budget are not given the same tokens: the second waits for the first | `cli_concurrency.rs` |
| API calls and CLI turns started together on one project token scope never hold more than its limit | `max_project_tokens.rs` (20 rounds of 16 threads; fails if a CLI admission holds nothing) |
| A CLI worker cannot start a turn on a spent task budget, and Codex is stopped when it reports usage past it | `cli_budget.rs` |
| A dollar limit never admits a CLI turn, priced model or not | `cli_budget.rs`, `cli_concurrency.rs` |
| A Jev routing call is held against the project budget before it is sent; refused, it is not made and the heuristic decides | `router_budget.rs`, `jev_settlement.rs` |
| A Jev call's real cost is recorded in the step that gives its hold back, including a cost above the estimate | `jev_settlement.rs`; with the order reversed (release, then record) 3 of its tests and 2 of `jev_unknown_cost.rs` fail |
| Several simultaneous Jev calls cannot jointly pass the project budget | `jev_settlement.rs` (eight threads, room for two) |
| A Jev request that never left (refused connection, malformed endpoint) releases its hold and records nothing | `jev_unknown_cost.rs` (router and project level) |
| A Jev request that was sent and failed with no reported cost (timeout, cut connection, error status, non-JSON body, or a good answer with no usage) is counted as spent at its reservation, marked unknown, in the step that gives the hold back; it is never freed as zero, a later request cannot use the room, a retry neither frees nor double-counts it | `jev_unknown_cost.rs` (13 router-level, 2 project-level) |
| A failed Jev request that does report its cost is recorded at that cost, in full even above its reservation | `jev_unknown_cost.rs` |
| A mixture of successful, failed and unknown Jev calls run concurrently with API reservations adds up exactly and never passes the limit | `jev_unknown_cost.rs` (eight Jev calls in different outcomes against eight API calls) |
| Jev's own model must be named in `routing_models`; otherwise it is not called, nothing is reserved, and the heuristic decides with a note; naming it in the role lists is not a permission; the list does not widen Jev's candidates | `routing_policy.rs` |
| `backend = "heuristic"` keeps Jev off a project; a pin does not | `jev_settlement.rs` (both) |
| A coding-CLI entry under an enforced policy either pins a model (Claude Code `--model`, Codex `-m`), which is checked against the role's list and put on the command line, or is accepted unpinned by name in `unpinned_cli_models`, or is refused before the CLI starts; planners and workers are checked against their own lists; a CLI whose command-line precedence is not verified counts as unpinned | `cli_model_policy.rs` |
| The planner's policy check is made against the entry it runs as, not the first entry on the same CLI | `cli_model_policy.rs` (this was a defect, fixed) |
| The environment and a later edit of `backspace.toml` do not change the model after it was checked | `cli_model_policy.rs` (fake CLI sees `ANTHROPIC_MODEL`, and the argument is still the pinned model) |
| Claude Code's `--advisor` and `--agents` are left off when the project's lists do not name their models, each decided on its own, at the right role | `cli_policy.rs` |
| A generated policy names the advisor and scout aliases, the configured routing model, the unpinned CLI entries and the pinned CLI models; a list a person wrote is never widened | `cli_policy.rs`, `routing_policy.rs`, `cli_model_policy.rs` |
| Adding to `routing_models` or `unpinned_cli_models` is a loosening: ignored with a warning until a person runs `policy adopt`; removing is adopted by itself | `routing_policy.rs`, `cli_model_policy.rs` |
| The CLI config entry that starts a worker or planner is refused before its turn when the lists do not name it | `cli_policy.rs` |
| The API advisor is left off when the lists do not name it for the agent's role | `advisor_policy.rs` |
| Each path states separately whether the budget and whether the policy covers it; every HTTP-sending file is accounted for; the strict-budget section and the Jev rows say the same things | `policy_matrix.rs` (words checked against the document and a source scan) |
| `max_project_tokens` is optional, parsed, written back, shared by API and CLI admissions, and does not count Jev's tokens | `max_project_tokens.rs`, `jev_settlement.rs` |
| No model call leaves the process without an `Authorization` | `unmetered.rs` (source scan) |
| The policy lock is authoritative; a lock that cannot be read stops the open; a rewritten `backspace.toml` does not become the next session's policy by itself, within the limits below; a delivery that edits it waits for a person even under auto-approve; the restore guard covers API tool batches, and CLI worker and planner turns | `policy_integrity.rs`, `policy_integrity_cli.rs` (the planner test is new in this pass) |
| Each limit is written down | `budget_wording.rs`, `policy_matrix.rs` (the documents contain them; this checks words, not behaviour) |
| Plugin refusal | gateway-level unit tests only. No loader until M11; this is not end-to-end evidence |

## Operational recommendations

Things a person does; the software does not enforce them.

1. For a strict-budget project set `[router] backend = "heuristic"`. It applies to the whole project; there is no per-task strict mode and nothing switches it on for a task. A pin is not a bypass.
2. Keep plan-billed CLIs (Claude Code, Codex and the rest) off strict-budget work. Their spend is observed as the CLI reports it, not bounded.
3. Pin a full model id on every coding-CLI entry (`model = "claude-…"`), and keep the policy lists naming it. An alias such as `sonnet` is resolved by the CLI, whose own settings can remap it.
4. After upgrading, regenerate an older enforced policy (`backspace-cli policy init`) and adopt it: an enforced policy written before this pass has no `routing_models` or `unpinned_cli_models`, so Jev is not called and unpinned CLI entries are refused until it does.
5. Keep `CloudChatServer` and `AgentChat` off the project's accounts; no setting disables them. Do not send strict work to a remote machine unless its own `backspace.toml` carries the budget.
6. Set `max_project_usd` together with `max_project_tokens` if CLI workers are used; a dollar limit alone refuses every CLI turn.

## Assumptions

These are what the controls above rest on. None is measured here.

1. **Bytes per token.** A token covers at least one input byte. No tokenizer was run.
2. **Provider overhead.** The provider adds no more than the 1,024-token allowance.
3. **Pricing accuracy.** The configured per-million-token prices are right, and Jev's own rate `jev_usd_per_mtok` too.
4. **Enforcement of `max_tokens`.** The provider honours the output cap that is sent.
5. **Advisor output** is not provider-capped; `advisor.max_output_tokens` is only what a reservation assumes.
6. **Jev's answer is 512 tokens or less.** An assumed reservation, not a cap; a larger real cost is recorded in full and named.
7. **Claude Code reports usage once, at the end of a turn,** and Codex per `turn.completed`.
8. **Whether Codex's usage figures are cumulative within a process or per turn is not known;** Backspace takes the largest figure reported.
9. **`--model` / `-m` win over the CLI's environment and settings,** as the vendors document them (Claude Code: `--model` over `ANTHROPIC_MODEL` and the `model` setting; Codex: command-line arguments over a profile and `config.toml`). Taken from documentation, not tested against the real tools.
10. **A conservative count is not a measurement.** An unknown-cost Jev call is counted at its reservation, which may be more or less than it cost.

## Known limitations

1. **Not a hard budget guarantee anywhere.** Reserved, admitted and unmetered are classifications; admission and post-call accounting are not enforcement of spend.
2. **Claude Code may overshoot within one reported turn,** bounded by that turn, not zero; the next is refused.
3. **The unmetered paths** remain outside task-budget enforcement and outside the project's policy: `CloudChatServer`, `AgentChat` (including its direct Backspace Cloud request), and `JevRouter` where no budget is around the router.
4. **A remote machine's harness** runs under its own policy and budgets; neither follows a task there.
5. **CLI policy versus the CLI's own model selection.** A pinned model is put on the command line and checked, but the real CLI's resolution of an alias, an organization setting that replaces a model, or a CLI that ignores the argument are outside what Backspace can see. A pin on Cursor, Grok or OpenCode counts as unpinned because their precedence is not verified. The fake-CLI tests show what Backspace passes, not what a real CLI does with it. Claude Code's advisor and scout are checked as written; anything the CLI's own settings add is not.
6. **`max_project_tokens` costs:** CLI turns on a shared token budget run one at a time; an API call is refused, not queued, while a CLI turn holds the tokens; a project that does not set it is not affected; Jev's tokens are not counted toward `max_project_tokens`; a dollar limit alone still refuses every CLI turn. `cli_turn_tokens` was not introduced.
7. **Jev:** its output size is not provider-capped; its cost, when nobody reports it, is counted at the reservation; a router with no budget around it is unmetered; what the Jev service does with a request is outside Backspace.
8. **The restore guard is not an OS-level security boundary.** The policy in force comes from a lock outside the workspace; a `backspace.toml` that loosens it is ignored with a warning until a person runs `backspace-cli policy adopt`; and around every tool batch and CLI turn the harness puts the file and the lock back if they changed. A background process an agent leaves running can write between monitored actions, and that write is taken as a person's edit; a person's own edit made during an agent's action is undone with the agent's. Deleting or emptying the lock re-adopts the file as a first open; that needs write access outside the workspace and nothing prevents it. **Policy integrity is open until the M6 sandbox,** or a separate OS user, closes that.
9. **No plugin loader until M11,** so plugin refusal has gateway-level evidence only.
10. **Real services were never used.** No real provider, no real CLI and no real Jev was called; tokens used: not measured.

## Future work

Not done in M1, on purpose and in writing:

- The M6 sandbox (or a separate OS user) for policy integrity; plugin loader and integrity check (M11).
- Validation against the real tools: Claude Code and Codex usage semantics, `--model` precedence, token-per-byte measurement, a tokenizer or `count_tokens` check.
- A smaller per-turn CLI allowance so that CLI turns could overlap (`cli_turn_tokens`), if the one-at-a-time cost is not acceptable.
- A per-task strict mode, and a way to switch the advisor, chat or the cloud server off for it. Not planned in M1.
- Owner's call, outside M1: `chat.rs` `spawn_reply` clears the busy flag before the reply's verdict is attached and sets it after spawning (see `M1-final-flaky-evidence.txt`).
- Nothing from M2 or M3.

## Final hardening pass

### Findings

Verified against the code before anything was changed.

1. **Jev unknown cost: a real gap, now closed.** A request that failed after it was sent (a timeout, a cut connection, an error status, a body that is not JSON) released its hold and recorded nothing, so a call that may have been billed disappeared from the books. A good answer with no usage in it was recorded as free. Now: a request that never left releases; a known cost is recorded in full; an unknown one is counted at its reservation, marked, in the same locked step; the person is told in the decision's note.
2. **CLI model selection: a real gap, now closed for Claude Code and Codex.** The policy checked the CLI's config entry, but `model` on the entry was never passed to the CLI, so nothing said which model ran, and the lists said nothing about it. Now an entry's pinned model is checked and passed; an unpinned entry needs a written exception. The fake-CLI limitation applies.
3. **The planner was checked as the first entry on its CLI,** not the one it ran as. With two entries on Claude Code the wrong one was looked up. Fixed (`planner_entry`).
4. **Jev's own model was outside the policy.** Previously a documented decision; now a routing-model policy of its own (owner's decision C).
5. **No defect found** in: the Claude Code advisor and scout gating (retained, tests fail when the check is removed), `max_project_tokens` (parse, round trip, shared scope, API refused not queued, Jev excluded), the policy-integrity guard (audit below).
6. **Two flaky tests classified,** both test-side: `spec_gate` waited on a flag that clears before the verdict is attached (2 of 60 runs failed); the two `webhook` tests shared a process-wide `BACKSPACE_DATA` (8 of 80 failed together, 0 of 60 alone). Test-only fixes: 120 of 120 and 150 of 150 after. No webhook or chat product code was touched. Evidence: `M1-final-flaky-evidence.txt`.

### Owner decisions applied

- **A.** `max_project_tokens` kept, with its trade-offs written down; `cli_turn_tokens` not introduced.
- **B.** Advisor and scout gating stays fail closed (Claude Code `--advisor` only if allowed for the agent's role; the scout only if allowed for execution; otherwise off with a log line). Generated policies name `opus` and `haiku`; a hand-written enforced policy is never silently widened. The tests that remove the check fail.
- **C.** Jev's model is governed by `routing_models`, separate from the role lists. Enforced policies must name it; generated policies name the configured model; legacy keeps working with the open-time warning; if not allowed, Jev is not called and the heuristic decides; the router's classification model and its candidates are separate.
- **D.** The restore guard is a limited M1 protection, not an OS-level boundary (limitation 8).

### Behaviour changes for existing projects

- **Enforced policies written before this pass stop calling Jev and refuse unpinned CLI entries** until they name `routing_models` and `unpinned_cli_models` (`backspace-cli policy init` prints both). The project says so when it opens. This is deliberate: the new dimensions are closed under `enforced`, like the others. Legacy and open policies are unchanged.
- **Coding-CLI turns under an enforced policy pass `--model` / `-m`** when the entry pins a model. Without an enforced policy nothing is passed, as before.
- A Jev call that fails after dispatch now counts against the project dollar budget at its reservation. A project near its limit can refuse a call sooner, never later.
- A Jev call's cost now reaches the metrics ledger with the books (a billed but unusable answer too), not only through the decision.

### Changes to existing tests

Each is a consequence of the new behaviour above. None weakens an assertion; no test was deleted or skipped.

1. `cli_policy.rs`: `NARROW`, `WITH_BOTH`, `SPLIT`, `WITH_OPUS` add `unpinned_cli_models = ["cli-model"]`: the entry pins no model and the policy has to say it may run that way. The advisor and scout assertions are unchanged.
2. `jev_settlement.rs`: `ONLY_SMALL` adds `routing_models = ["jev-test"]` so Jev may be called at all. The candidate-filtering assertions are unchanged.
3. `policy_integrity_cli.rs`: `POLICY` adds the same `unpinned_cli_models`; `project` became `project_with` (a wrapper keeps the old signature).
4. `spec_gate.rs`, `webhook.rs`: the flaky-test fixes above.
5. `policy_matrix.rs`: reworked for the four-column table, the two Jev rows and the strict-section agreement.

### Policy-integrity audit

| Requirement | Evidence | Gap |
|---|---|---|
| Loads the authoritative lock | `policy_integrity.rs` (first open adopts; tightening adopted, loosening ignored with a warning; each workspace has its own lock) | none |
| Refuses unreadable or invalid state | `a_lock_that_cannot_be_read_stops_the_project_opening…` (a corrupt lock stops the open and is not overwritten) | a missing lock is a first open (limitation 8) |
| Detects and restores covered modifications after tool batches | `bash_cannot_weaken_the_policy…`, `the_lock_is_restored_after_a_command_that_rewrites_it`, `a_command_that_leaves_the_policy_alone_is_not_reported` | between monitored actions (limitation 8) |
| …and after CLI turns | `a_cli_worker_cannot_weaken_the_policy`; **new** `a_cli_planner_cannot_weaken_the_policy_either` (passed at once: the guard sits inside `run_cli_turn`, so this fills a gap in evidence, not in code) | same |
| Prevents unapproved adoption | `a_merged_branch_that_edits_the_policy_is_held_even_under_auto_approve`; `every_way_of_loosening_is_ignored`; new lists in `routing_policy.rs`, `cli_model_policy.rs` | none |
| Human approval for policy changes | `a_person_adopting_a_loosening_makes_it_take_effect_and_stay`, `ignoring_a_loosening_does_not_wear_off` | none |

### Added after red

Two tests were written after the red commit (`jev_unknown_cost.rs`: the project counts an unknown cost once, and marks it; a failure before dispatch has nothing to count in a project). The first was run red before the implementation (the unknown amount stayed 0); the second is a control that passes before and after. `planner_cli_cannot_weaken…` was written with the red set and also passed at once, as above.

### Mutation checks

Each protection was removed and the named tests failed, then the source was restored (checked byte for byte):

| Protection removed | Tests that fail |
|---|---|
| `Reservation::settle` releases before it records | `jev_settlement` 3, `jev_unknown_cost` 2 |
| Router does not check `routing_models` | `routing_policy` 3 |
| An unknown-cost call releases its hold instead of counting it | `jev_unknown_cost` 10 |
| A connect/builder failure is treated as dispatched | `jev_unknown_cost` 2 |
| Unpinned CLI check removed | `cli_model_policy` 4 |
| Pinned model not checked against the role's list | `cli_model_policy` 4 |
| Pinned model not passed to the CLI | `cli_model_policy` 5 |
| Legacy also pins | `cli_model_policy` 1 |
| Cursor treated as pinnable | `cli_model_policy` 1 |
| Planner checked as the first entry on its CLI | `cli_model_policy` 1 |
| `routing_models` / `unpinned_cli_models` not compared when adopting | `routing_policy` 1, `cli_model_policy` 1 |
| Generated policy widens an enforced file | `routing_policy` 1 |

### Corrections to my own tests

`cli_model_policy.rs`: the fake Cursor binary was named `cursor`, but Backspace looks for `cursor-agent`, so `a_pin_on_a_cli_whose_command_line_precedence_is_not_verified_counts_as_unpinned` failed at red for the wrong reason ("no usable models"); fixed, and the mutation above shows it now fails for the right one. No earlier test was touched except as listed under *Changes to existing tests*.

## History: earlier rounds

### Third review

1. **Jev: hold released before its cost was recorded.** Closed: `Reservation::settle` records under the table's lock and releases in the same hold. The triage restart path never recorded its Jev cost; it does now.
2. **Claude Code's advisor and scouts bypassed the lists.** Closed with fail-closed gating; generated policies carry the aliases.
3. **Chat's Backspace Cloud route is a direct request** the name scan cannot see; now a named part of the `AgentChat` row, and a source scan accounts for every HTTP-sending file.
4. No defect in the CLI entry checks, Jev's candidate filtering, the API advisor check or `max_project_tokens`.

Decisions made while the owner was asleep (kept, or reversed by the final decisions A to D above): keep `max_project_tokens` (A); gate the advisor and scouts, fail closed (B); Jev's model outside the lists (reversed by C); no strict-budget mode; API turns keep record-then-release (it double-counts for an instant, which can only refuse, never over-admit); a billed Jev answer that cannot be used is recorded too.

Earlier rounds' changes stand: raw-token budgets, larger reservations, a dollar limit refusing every coding-CLI turn, routing calls that can be refused, deliveries that edit `backspace.toml` waiting for a person, and a folder with no `backspace.toml` starting enforced.

### Corrections to my own tests, earlier rounds

First round (after red `418f5df`): two of the 63 were wrong, not the code (`budget_bound.rs`, `budget_reservation.rs`). Second round: none. Third round (after red `5cdd77d`): three new tests were wrong and fixed: `nobody_sees_the_hold_gone…` counted the racer's own live reservation; `jev_calls_completing_among_api_reservations…` included the sizing call in its expected total; `a_workers_advisor_is_checked_at_the_workers_role…` assumed a ticket worker is an execution agent, but at the default depth it may create tickets and is checked as an orchestrator.

### Deviations from the approved plan

1. **New-project orchestration list holds every model, not only API models.** A CLI worker that can spawn tickets counts as an orchestrator, so a list of API models alone would refuse it.
2. **No digest in the lock.** The lock is plain TOML; the restore guard puts it back if it is changed during an action, and an unreadable lock stops the open rather than falling back to the file.
3. **The restore guard runs after each tool batch (the parallel calls of one turn) and each CLI turn,** not after each individual call.
4. **New knob `max_project_tokens`** (second review), kept.

### Not done, on purpose

- No tokenizer or `count_tokens` call; no OS sandbox (M6); no plugin loader (M11); no real provider, CLI or Jev spend tests.
- No setting to switch the advisor, chat or the cloud server off for a strict task; the lists switch the advisor off, nothing else.
- No per-turn CLI allowance setting.
- Nothing from M2 or M3.

Tokens used: not measured. Frontier calls: 0.
