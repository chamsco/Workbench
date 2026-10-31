# M1 budget boundaries

Which model calls a project's budget and policy cover, how, and which they do not. Written for the hardening pass and revised after the second review; `crates/core/tests/unmetered.rs` fails if a model call is added that is not on this list, and `crates/core/tests/budget_wording.rs` fails if a limit below is dropped from this page.

Every model call is **reserved** (a gateway reservation holds its worst case against each budget it counts toward before anything is sent), **admitted** (coding-CLI turns: checked at the start, holding an allowance, stopped on reported usage) or **unmetered** for one of three named reasons. Nothing else leaves the process.

None of the three is a hard budget guarantee in general. What each does and does not give is below; where a row says *not enforced*, no budget limits that path.

## What a reservation rests on

An API reservation is a bound only under all of these:

1. **The byte-per-token assumption.** The input bound is the exact request body in bytes plus 1,024 (provider-added prompt) plus 8 per message and 16 per tool. It holds if a token covers at least one input byte. No tokenizer was run, so no figure here is a measured token count.
2. **Provider overhead.** The 1,024 allowance is justified by Anthropic's documented tool-use system prompt (up to 804 tokens). Overhead beyond it, or a provider that adds more, is not in the bound.
3. **Accurate configured pricing.** Dollars are computed from `input_usd_per_mtok` and `output_usd_per_mtok` in the config (Anthropic input at 1.25x for cache writes). A wrong price makes the reservation wrong.
4. **The provider enforces `max_tokens`.** The granted output is sent as the output cap.

If any of these fails, a call can cost more than was reserved. That is logged as an `overshoot`, counted in full, and checked against by the next call. The overspend itself is not prevented.

## The paths

| Path | Class | What is enforced | What is not |
|---|---|---|---|
| API agent turn (`drive`: Main, orchestrators, workers, scouts, triage on an API model) | reserved | Model allowlist, then a worst-case reservation (request bytes as sent + allowances + the whole granted output) against the task limit, every ticket budget above the agent, and the project limits. Refused before the request if it does not fit; refused if the request grew after the reservation. | Everything under *What a reservation rests on*. While a coding-CLI turn holds a shared token budget, API calls on that budget are refused (not queued). |
| The advisor on an API turn (Anthropic server tool) | reserved, **assumption only** | A second reservation at the advisor's price over the same input bound plus `advisor.max_output_tokens` (default 4,096). If it does not fit, the advisor is left off that call and the log says so; the call itself still goes ahead. | Advisor output is **not provider-capped**: the tool takes no output cap, so what it writes is not enforced. Real use above the assumed cap is logged as an `overshoot` and counted in full. |
| CLI worker turn (`finish_cli`) and a planner running in a CLI (`plan_on_cli`) | admitted | Refused before the turn when a budget it counts toward is spent. Otherwise it is given everything the tightest token limit has left, and **that allowance is held against every scope it counts toward until the turn ends**, so two turns are never given the same tokens; a second turn on the same budget waits for the first (`is_held`) and then asks again. A dollar limit with no token limit beside it is refused, naming the model, whether or not the CLI model has a configured price (a CLI turn adds no dollars to any scope). Task and project token limits bind it. | **Claude Code may overshoot within one reported turn**: it reports usage once, at the end of the turn, so it is not stopped inside the turn; the overshoot is bounded by that one turn, not zero, and the next turn is refused. Codex reports per `turn.completed` and is stopped when that passes the allowance. Within one process the largest figure reported is taken, and **Codex's cumulative versus per-turn usage semantics are not verified**: if its figures are per turn, the largest undercounts a process that runs several. Only fake CLIs were run. |
| Claude Code's own advisor (`--advisor`) and scout subagent (`--agents`) | inside the CLI | Counted only as far as the CLI includes them in the usage it reports. | Not enforced by the gateway: they are not separate calls this process sees. |
| `JevRouter`: the Jev router's own classification call | reserved where a project budget is around the router; unmetered where there is no budget | Inside a harness, the call's worst case (its body in bytes bounded as above, plus an assumed 512-token answer, at Jev's own rate `jev_usd_per_mtok`) is held against the **project** budget until the response is in. If it does not fit, Jev is not called and the heuristic decides. Its real cost is added to project spend. | A task-scoped routing call is checked against the project budget only: the task has no agent yet, so there is no per-task budget to check. Router tokens are not counted in `max_project_tokens`. The hold is released when the decision returns and the cost is recorded just after, so a reservation checked in between can miss it, by at most one routing call's cost. Jev's answer size is an assumption, not a cap. A router with no budget around it (a script, the eval) declares `Unmetered(JevRouter)`: **no budget** applies there. |
| `CloudChatServer`: the development Backspace Cloud server answering chat with the models in its own config | unmetered | Nothing. It has no project, no task and no policy of its own. | **Not enforced.** It is outside task-budget enforcement. |
| `AgentChat`: chat with an agent, a bot or a routine, over a CLI, a local model or a router | unmetered | Nothing task-scoped. It is not a task, so no task budget applies. | **Not enforced.** It is outside task-budget enforcement. |
| A remote machine's harness | out of this process | Nothing here: it runs under the policy and budgets of its own `backspace.toml`. | This project's policy and budget do not follow a task to a **remote** machine. Not enforced from here. |
| Plugins | no call site yet | Gateway-level unit tests only (`check_plugin`). | There is no loader until M11. The evidence is gateway-level, not end to end. |

The unmetered paths, `CloudChatServer`, `AgentChat` and, where no budget is around it, `JevRouter`, remain **outside task-budget enforcement**.

## Strict budgets

A task with a strict budget requirement cannot get one from every path above. Until a path's spend can be bounded, **disable it** for that project. This pass proposes the list; it builds no switch for it.

1. **Do not use plan-billed CLI workers** (Claude Code, Codex and the rest). Their spend is observed only as the CLI reports it. A dollar limit already refuses them; a token limit only admits and stops them, and Claude Code can overshoot by one turn. Route strict tasks to API models.
2. **Switch the advisor off** for strict tasks. It is the one API cost the provider does not cap. Today it is left off only when its worst case does not fit; there is no config setting that turns it off outright (a proposal for a later pass).
3. **Keep `CloudChatServer` and `AgentChat` off the project's accounts.** They are not task-scoped and cannot be limited by a task or project budget. There is no setting for this today either.
4. **Do not send strict tasks to a remote machine** unless its own `backspace.toml` carries the budget.
5. **Set the limits that do bind:** `max_project_usd` and `[policy.budget]` for API turns, `max_project_tokens` for a token ceiling that API reservations and CLI admissions share. A routing call's worst case is held against the project budget, with the limits above.

What this leaves for a strict task: API turns on reserved models, under the byte-per-token and pricing assumptions, with every other path off. That is the closest M1 gets to a hard budget, and it is still an assumption-bound one.

## Wording to use

- Say **reserved**, **admitted** or **unmetered** and give the limit; do not say a project has a hard budget guarantee. Hard is claimed for no path, and a reservation is a bound only under its assumptions.
- A CLI turn is bounded by the usage it reports, not by a spend flag. No CLI-native spend flag was verified, so none is relied on.
- A dollar budget is a promise about priced API models only. It cannot bind a plan-billed CLI, which is why it refuses instead of passing.
