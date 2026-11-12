# M1 hardening: gaps found and proposed fixes

Status: **proposal only. No code has changed.** M1 stays pending your approval. Every gap below was found by reading the code at `8230513`; line numbers are from that commit. Where I could not measure something I say so. I have not run a real tokenizer (none is available here), so nothing below claims a measured token count.

## What M1 guarantees today, and what it does not

| Claim | Status |
|---|---|
| A disallowed model, skill or plugin is refused before any request (API agents) | holds, proven by tests; plugin only at gateway level (no loader until M11) |
| One call cannot exceed a budget | **not established**: the input estimate is not an upper bound (G3) |
| Concurrent calls cannot jointly exceed a budget | **false** (G4) |
| An agent cannot change the policy | **false** for bash, CLI workers and merged work (G7, G8) |
| Every task-scoped model call goes through the gateway | **false** for CLI turns and, by omission, nothing stops a new call site (G9, G11) |

## Gaps

| # | Gap | Evidence | Why it matters |
|---|---|---|---|
| G1 | No `[policy]` means everything is allowed, silently | `gateway.rs` `check_model`/`skill_allowed`: `None` list = open | A project that never opted in has no allowlist and nothing says so |
| G2 | Host-injected local models are not in an explicit list | `local_subagent` (`harness.rs:204`) adds a tier-0 model after the policy is read; `drive()` then calls `check_model` | With `execution_models` set, the local triage/scout/worker jobs a user switched on in Settings are refused |
| G3 | The input estimate is not an upper bound | `prompt_chars` (`harness.rs:2027`) is `bytes / 3` of a *re-derived* prompt (messages JSON plus `name + description + schema` per tool), not the request body that is sent (`provider.rs:216-232`). Any text with more than one token per 3 bytes (hex, base64, minified code, some scripts) is under-counted. Anthropic's 1.25x cache-write price (`provider.rs:229`; the folding is at `:281-288`) is not applied to the worst case | "Hard budget" is not supportable with this estimator |
| G4 | No reservation: calls authorize against stale spend | `budget_lines` (`harness.rs:2036`) reads spent-so-far; `authorize_call` runs at `:2136`; the concurrency limiter is taken later at `:2155` and spend is recorded after the response | With `max_parallel_calls = 4`, four agents can each be told "you fit" against the same remaining budget |
| G5 | Advisor spend is neither bounded nor reserved | The advisor rides as a server tool with no limit (`provider.rs:222`); its tokens are only read afterwards (`:277-290`) and billed at its own rates | One call can cost more than its grant, by an amount nothing caps |
| G6 | Reconciliation hides overshoot, and token budgets use billing-equivalent tokens | `provider.rs:281-288` folds cache reads (x0.1) and writes (x1.25) into `input_tokens`; `a.input_tokens` then feeds the per-task token budget | A token budget is measured in something other than tokens; an overshoot is just added to spend with no event |
| G7 | Bash can rewrite `backspace.toml` | `Workspace::sh` runs `bash -lc` in the worktree with no restriction (`tools.rs:188`); `resolve` says it "guards the file tools only" (`:88`) | The next session loads the edited file |
| G8 | CLI workers and merged work can rewrite it too | A CLI worker edits files with its own tools; `submit` merges the worker branch with `git::merge` (`harness.rs:2642`), and with `auto_approve` no human sees the diff (`:2607`). The file is tracked, so the change lands on the branch | Same outcome as G7, through the normal delivery path |
| G9 | CLI workers get a model check and nothing else | `finish_cli` (`harness.rs:1652`) calls `check_model` once, then runs up to 4 rounds (`:1698`); no budget check before or during; CLI turns add $0 (plan-billed, price 0) | A task budget never binds a CLI worker |
| G10 | Same for a planner running in a CLI | `plan_on_cli` (`harness.rs:1474`): `check_model` only | Same |
| G11 | Other model calls sit outside the gateway with no declared boundary | `cloud.rs:668` (development cloud chat server, its own accounts); `chat.rs:1399` (agent chat, bots, routines, webhooks, over CLI, Ollama or a router); `router.rs:397` (Jev decision call, counted in total cost afterwards, never checked); `remote.rs` (a remote machine runs under its own harness). `provider::complete` is a public free function, so a new call site bypasses the gateway without anyone choosing to | "No silent bypass" cannot be shown, only hoped |
| G12 | Plugins | no loader | stays with M11; the evidence level stays "gateway unit tests" |

## Proposed fixes

**F1 (G1, G2): policy modes, migration-compatible.** I will not change behaviour for any existing config.

| Config state | Behaviour | Notice |
|---|---|---|
| No `[policy]` (every existing project) | open, as today | **warning on every open**, naming what is open and the command to fix it |
| `[policy]` without `mode` | exactly M1's rules (a missing list is open, `[]` denies all) | warning naming each open dimension |
| `[policy] mode = "legacy_open"` | same as above | one-line note (you chose it) |
| `[policy] mode = "enforced"` | every list must be present, else the project fails to open with a message; `[]` denies all | none |
| New project (`init-config`, built-in default) | `mode = "enforced"` with explicit lists: API models for orchestration, all models for execution, the shipped skills, no plugins | none |

Plus `backspace-cli policy init`, which prints a `[policy]` block that reproduces today's *effective* permissions (models usable per role, skills found), so moving to enforced changes nothing until you edit it. Skills found in the project but missing from an enforced list are named in a warning. Models the user switches on in Settings (host-controlled prefs, not agent-writable) are added to the execution list at injection. Warnings go to `Harness::warnings()`, the main agent's log, and stderr in the CLI. All 33 M1 tests keep passing because `mode` is optional and unset means M1's rules.

**F2 (G3, G4, G5, G6): reserve before the call, reconcile after.**
- *Input bound:* the UTF-8 byte length of the exact request body (built once in `provider.rs` and used both to send and to measure), plus a fixed allowance per message and per tool. Assumption, stated in the code: a token covers at least one input byte, and request blocks are text only (true today: `Block` has no image variant). This is about 3 to 4 times too high for English prose, which means earlier refusals under a tight budget; that is the price of a bound. Anthropic's free `count_tokens` endpoint could give an exact count and win the utilization back; I recommend deferring it until budgets prove too tight, since no other provider has an equivalent.
- *Price bound:* input at 1.25x for Anthropic (cache write), output at the granted `max_tokens` (the provider enforces it; thinking and reasoning tokens count inside it).
- *Reservation:* an atomic reservation table in the gateway keyed by scope (task, each budgeted ancestor, project). Check and reserve happen under one lock that also reads spent, so two callers cannot both see the same room. A guard releases on drop, so a stopped or failed call frees its hold. On completion the actual cost replaces the reservation; if it is higher, an `overshoot` event is logged and the extra counts as spent.
- *Token budgets* count raw tokens (a new field alongside the billing-equivalent one).
- *Advisor:* reserved at an assumed cap (`advisor.max_output_tokens`, new, default 4096) times the advisor's price, over the same input bound. I cannot make the provider enforce that cap, so advisor spend stays **bounded by assumption, not by the provider**, and the code and docs say so. If the reservation does not fit, the advisor is left off that call rather than the call being refused.
- *What remains after this:* overspend is still possible if the byte-per-token assumption fails, if a configured price is wrong, or by the advisor's real output exceeding its assumed cap. Each is named in the docs; none is silent.

**F3 (G7, G8): policy-file integrity, layered, with the limit stated.** No user-space mechanism makes the file tamper-proof while bash is unsandboxed: `sed -i` replaces the file by rename, so a read-only bit does not stop it, and the same user can undo `chmod`. What I can enforce:
1. *Adopted policy lock held outside the workspace* (`<data dir>/policy-locks/<hash of workspace path>.toml`, with a digest), written on first open or by the human-only `backspace-cli policy adopt`. On every open the policy comes from the lock. If the workspace file differs, a **tightening** (subsets of every list, lower budgets) is adopted automatically; **anything looser is ignored**, with a warning showing the diff. So an agent cannot weaken the policy for the next session by editing `backspace.toml`, whether by bash, a CLI worker's own tools, or a merged branch.
2. *Restore guard:* after every bash call, file-tool call and CLI turn, the harness checks the workspace policy file and the lock against the copies in memory and restores them, with a visible event.
3. *Merge gate:* a delivery whose diff touches `backspace.toml` is stopped before the merge, even under `auto_approve`, and shown to the human as a separate "changes project policy" approval.
- *Residual, stated plainly:* a process that knows the lock path and writes it after the harness's last check (a background job, or a crash between write and restore) is not stopped. Only the M6 sandbox, or running agents as a separate OS user, closes that. I will classify this as **open until M6** and not claim otherwise.
- Alternative if you prefer a real boundary now: run bash under an OS sandbox (bwrap on Linux, sandbox-exec on macOS) with the policy paths read-only. That is platform-specific, absent on Windows, and is M6's job; I recommend against pulling it into M1.

**F4 (G9, G10, G11): CLI budgets and a boundary for every call.**
- *CLI turns:* before each round, refuse if the task is already at its limit; during a turn, stop the process when the usage it reports (tokens) passes what is left; fail closed when the budget is in dollars and the CLI model has no API-equivalent price configured (a $0 price cannot be budgeted). Overshoot is bounded by one reported usage event, not zero; the per-event granularity depends on what each CLI prints, so the guarantee is "bounded", and I will label it that way. I did not verify any CLI-native spend flag and will not rely on one.
- *Structural guard:* `provider::complete` and `run_cli_turn` take an `Authorization` argument, either a gateway reservation or `Unmetered(reason)` from a closed list (`CloudChatServer`, `AgentChat`, `JevRouter`). A test scans the source and fails if `Unmetered` appears anywhere not on that list. A new call site then cannot skip the gateway by accident.
- *Boundaries, documented in code and in `docs/cauldron/M1-NOTES.md`:* cloud chat and agent chat are outside project budgets (no project, no policy); Jev decision spend counts toward project spend after the fact and is not checked; a remote machine enforces its own policy.

## Failing acceptance tests I will write first

Names are stable so you can check them against the red commit.
- F1: `no_policy_opens_with_a_visible_warning`, `partial_policy_warns_for_each_open_list`, `legacy_open_mode_is_a_note_not_a_warning`, `enforced_mode_needs_every_list`, `an_empty_list_still_denies_everything`, `new_project_default_is_enforced_with_explicit_lists`, `policy_init_reproduces_the_effective_permissions`, `user_enabled_local_models_stay_usable_under_an_execution_list`.
- F2: `the_estimate_is_the_request_body_in_bytes_not_chars`, unicode, source-code, long-tool-schema and base64 cases (each asserts estimate >= bytes), `anthropic_input_is_priced_at_cache_write`, `parallel_calls_cannot_jointly_exceed_the_budget` (unit, many threads, and e2e against the delayed mock with `max_parallel_calls = 4`: exactly the affordable number of requests arrive), `a_stopped_call_releases_its_reservation`, `overshoot_is_recorded_and_stops_the_next_call`, `token_budgets_count_raw_tokens`, `the_advisor_is_left_off_when_its_reserve_does_not_fit`.
- F3: `bash_cannot_weaken_the_policy_for_the_next_session` (sed -i, redirect, `mv`), `a_cli_worker_cannot_weaken_it`, `a_merged_branch_that_edits_the_policy_is_held_even_under_auto_approve`, `tightening_is_adopted_and_loosening_is_ignored_with_a_warning`, `the_lock_is_restored_after_a_command_that_rewrites_it`.
- F4: `a_cli_turn_is_refused_when_the_task_budget_is_spent`, `a_running_cli_turn_is_stopped_when_it_crosses_the_limit`, `a_dollar_budget_with_an_unpriced_cli_model_fails_closed`, `unmetered_is_only_used_where_declared`.

Existing tests are untouched. Where a fix changes what an existing assertion observes, I will add a new test and leave the old one as is; if an old assertion cannot hold, I will stop and ask before changing it.

## Decisions I need from you

1. **F1 as tabled**, in particular: `[policy]` without `mode` keeps M1's open-list rule (with a warning) so existing tests and configs are unaffected, and only new projects are enforced.
2. **F3 design:** lock outside the workspace with tighten-only auto-adoption, plus restore guard and merge gate, with the stated residual until M6. The alternative is to pull an OS sandbox into M1.
3. **F2 advisor:** reserve at an assumed cap and leave the advisor off when it does not fit (my recommendation), versus turning the advisor off whenever any hard budget is set.
4. **F4 CLI:** stop on reported usage, fail closed on unpriced dollar budgets.

Tokens used so far: not measured. Frontier calls: 0 (no model was called in this review).
