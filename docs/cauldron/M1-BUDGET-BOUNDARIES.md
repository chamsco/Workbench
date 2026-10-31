# M1 budget and policy boundaries

Which model calls a project's budget and model policy cover, how, and which they do not. Written for the M1 review rounds and revised after the third. `crates/core/tests/unmetered.rs` fails if a model call is added that is not on this list, `crates/core/tests/budget_wording.rs` fails if a limit below is dropped from this page, and `crates/core/tests/policy_matrix.rs` fails if a path loses its policy column or a place that sends an HTTP request is not accounted for.

## Two questions per path

For every path the table below answers two questions, in two separate places:

1. **Does the budget cover it?** The *Budget class* column and the two budget columns after it.
2. **Does the model policy cover it?** The *Project policy check* column: which list, at which role.

Neither is inferred from the other. A path can be reserved against the budget and sit outside the policy (the Jev router's own model), or be checked against the policy and be unmetered, or be neither. Read the two columns separately.

**Budget classes.** Every model call is **reserved** (a gateway reservation holds its worst case against each budget it counts toward before anything is sent), **admitted** (coding-CLI turns: checked at the start, holding an allowance, stopped on reported usage) or **unmetered** for one of three named reasons. Nothing else leaves the process. None of the three is a hard budget guarantee in general; where a row says *not enforced*, no budget limits that path.

**Policy checks.** The policy column uses these phrases and no others:

- *Checked against the relevant project orchestration/execution allowlist.* The model is looked up, as written, in `orchestration_models` or `execution_models`, whichever is the role of the agent. Orchestration is the main agent, a planner running in a CLI, and any agent that may create tickets; execution is every other worker, triage and scouts. The row names the role.
- *Checked by the CLI's own configuration, not independently by the gateway.* Backspace does not choose the model; the CLI runs whichever one its own settings name.
- *Not covered by the project's policy.* No list is consulted.
- *Runs under the remote machine's own policy.*
- *Gateway-level check only; no end-to-end loader exists yet.*

These describe an enforced policy. A project with no `[policy]` section, or with `mode = "legacy_open"`, has open lists, so every check passes; that is announced when the project opens.

## What a reservation rests on

An API reservation is a bound only under all of these:

1. **The byte-per-token assumption.** The input bound is the exact request body in bytes plus 1,024 (provider-added prompt) plus 8 per message and 16 per tool. It holds if a token covers at least one input byte. No tokenizer was run, so no figure here is a measured token count.
2. **Provider overhead.** The 1,024 allowance is justified by Anthropic's documented tool-use system prompt (up to 804 tokens). Overhead beyond it, or a provider that adds more, is not in the bound.
3. **Accurate configured pricing.** Dollars are computed from `input_usd_per_mtok` and `output_usd_per_mtok` in the config (Anthropic input at 1.25x for cache writes). A wrong price makes the reservation wrong.
4. **The provider enforces `max_tokens`.** The granted output is sent as the output cap.

If any of these fails, a call can cost more than was reserved. That is logged as an `overshoot`, counted in full, and checked against by the next call. The overspend itself is not prevented.

## The paths

| Path | Budget class | Budget: what is enforced | Budget: what is not | Project policy check |
|---|---|---|---|---|
| API agent turn (`drive`: Main, orchestrators, workers, scouts, triage on an API model) | reserved | Worst-case reservation (request bytes as sent + allowances + the whole granted output) against the task limit, every ticket budget above the agent, and the project limits. Refused before the request if it does not fit; refused if the request grew after the reservation. The spend is recorded first and the hold released after, so for an instant both count: that can only refuse, never over-admit. | Everything under *What a reservation rests on*. While a coding-CLI turn holds a shared token budget, API calls on that budget are refused, not queued. | Checked against the relevant project orchestration/execution allowlist: the model the router picked (or a pin chose) is looked up for the agent's role before the request is built, and the router's candidates are filtered the same way. Orchestration: the main agent and any agent that may create tickets. Execution: the other workers, triage and scouts. |
| The advisor on an API turn (Anthropic server tool) | reserved, assumption only | A second reservation at the advisor's price over the same input bound plus `advisor.max_output_tokens` (default 4,096). If it does not fit, the advisor is left off that call and the log says so; the call itself still goes ahead. | Advisor output is **not provider-capped**: the tool takes no output cap, so what it writes is not enforced. Real use above the assumed cap is logged as an `overshoot` and counted in full. | Checked against the relevant project orchestration/execution allowlist: the advisor model is looked up at the role of the agent that calls it. If the lists do not name it, the advisor is left off that call and the call goes ahead (`advisor_policy.rs`). |
| CLI worker turn (`finish_cli`) | admitted | Refused before the turn when a budget it counts toward is spent. Otherwise it is given everything the tightest token limit has left, and **that allowance is held against every scope it counts toward until the turn ends**, so two turns are never given the same tokens; a second turn on the same budget waits for the first and then asks again. A dollar limit with no token limit beside it is refused, naming the model, whether or not the CLI model has a configured price. Task and project token limits bind it. | **Claude Code may overshoot within one reported turn**: it reports usage once, at the end, so it is not stopped inside the turn; the overshoot is bounded by that one turn, not zero, and the next turn is refused. Codex reports per `turn.completed` and is stopped when that passes the allowance. Within one process the largest figure reported is taken, and **Codex's cumulative versus per-turn usage semantics are not verified**: if its figures are per turn, the largest undercounts a process that runs several. Only fake CLIs were run. | Config entry: the entry that starts the CLI is checked against the execution allowlist (the orchestration allowlist when the worker may create tickets) before the turn, and refused if the list does not name it (`cli_policy.rs`). Model the CLI runs: Checked by the CLI's own configuration, not independently by the gateway. Backspace passes no `--model`, so the lists never see which model it is. |
| CLI planner (`plan_on_cli`: the main agent running in a coding CLI) | admitted | The same as a CLI worker turn. | The same as a CLI worker turn. | Config entry: checked against the orchestration allowlist before each turn, and refused if the list does not name it (`cli_policy.rs`). Model the CLI runs: Checked by the CLI's own configuration, not independently by the gateway. |
| Claude Code's own advisor (`--advisor`) | inside the CLI, not separately metered | Counted only as far as the CLI includes it in the usage it reports. | Not enforced by the gateway: it is not a separate call this process sees. | Checked against the relevant project orchestration/execution allowlist: `[advisor] cli` (default `opus`) is looked up as written, at the role of the agent that would consult it, before the turn. If the lists do not name it, `--advisor` is left off and the agent's log says so (`cli_policy.rs`). Only the advisor Backspace passes is checked; whatever the CLI's own settings add is not. |
| Claude Code's scout subagent (`--agents`) | inside the CLI, not separately metered | Counted only as far as the CLI includes it in the usage it reports. | Not enforced by the gateway: it is not a separate call this process sees. | Checked against the relevant project orchestration/execution allowlist: `[advisor] cli_scouts` (default `haiku`) is looked up as written, at the execution role. If the lists do not name it, the scout is left off and the log says so (`cli_policy.rs`). Only the scout Backspace passes is checked. |
| `JevRouter`: the Jev router's own classification call | reserved with a project budget; else unmetered | Inside a harness, the call's worst case (its body in bytes bounded as above, plus an assumed 512-token answer, at Jev's own rate `jev_usd_per_mtok`) is held against the **project** budget from before the request is sent until its real cost is on the books; the two happen in one step (see *JevRouter*). If it does not fit, Jev is not called and the heuristic decides. | A task-scoped routing call is checked against the project budget only: the task has no agent yet, so there is no per-task budget to check. Jev's answer size is an assumption, not a cap. A router with no budget around it (a script, the eval) declares `Unmetered(JevRouter)`: **no budget** applies there. A request that fails, or answers with a non-2xx status or non-JSON, has an unknown cost and records none. | Not covered by the project's policy for Jev's own model (`[router] jev_model` at `jev_endpoint`): no list is consulted for it. Covered for what it may pick: it is offered only the candidates the lists allow for the role being routed, and a name outside them is not used (`jev_settlement.rs`). `backend = "heuristic"` removes the Jev call. |
| `CloudChatServer`: the development Backspace Cloud server answering chat with the models in its own config | unmetered | Nothing. It has no project, no task and no policy of its own. | **Not enforced.** It is outside task-budget enforcement. | Not covered by the project's policy: the server has no project and reads no `[policy]`. |
| `AgentChat`: chat with an agent, a bot or a routine, over a CLI, a local model, a router or Backspace Cloud | unmetered | Nothing task-scoped. It is not a task, so no task budget applies. | **Not enforced.** It is outside task-budget enforcement. The Backspace Cloud route is a direct request (`/v1/chat`, with the user's cloud token) that the source scan in `unmetered.rs` cannot see by name; `policy_matrix.rs` keeps it listed. | Not covered by the project's policy: a chat over a CLI, a local model, a router or Backspace Cloud takes its model from the chat's own route, not from the project's lists. |
| A remote machine's harness | out of this process | Nothing here: it runs under the budgets of its own `backspace.toml`. | This project's budget does not follow a task to a **remote** machine. Not enforced from here. | Runs under the remote machine's own policy: this project's lists do not follow a task there. |
| Plugins | no call site yet | Gateway-level unit tests only (`check_plugin`). | There is no loader until M11. The evidence is gateway-level, not end to end. | Gateway-level check only; no end-to-end loader exists yet: `check_plugin` refuses a plugin the `plugins` list does not name, in unit tests, and nothing calls it until the loader exists. |

The unmetered paths, `CloudChatServer`, `AgentChat` and, where no budget is around it, `JevRouter`, remain **outside task-budget enforcement**.

How the two Claude Code rows are matched: the alias in `[advisor] cli` and `cli_scouts` (`opus`, `haiku`) is compared as written with the names in the lists, since that is the name Backspace passes to the CLI. A list the person wrote must carry the alias to keep the advisor or scout. A policy that Backspace writes out (a new project, `backspace-cli policy init`) names the aliases the config already uses, so those projects keep what they had; a list written by hand is never widened by them.

## JevRouter

What the third review asked, answered from the code and `jev_settlement.rs`.

- **When the hold is held.** From before the request is sent until the call's real cost is on the project's books. Earlier, the hold was released when the decision returned and the cost was added afterwards; another reservation could be checked in between and see neither. That window is closed.
- **How.** `Reservation::settle` runs the recording inside the reservation table's own lock and takes the hold out under the same lock hold. Every other reservation (an API call, a CLI admission, another Jev call) reads the holds and, through its `lines` closure, the spend under that lock, so none of them can see the hold gone and the spend not recorded, nor a cost above the estimate not yet recorded. The harness supplies the recorder that adds the cost to the router and project totals; the routing code then skips adding it again.
- **A cost above the estimate.** Recorded in full before the room is given back, and named in the decision's note (`jev overshoot`). The next call is checked against it. The overspend itself is not prevented: the 512-token answer is an estimate of Jev's output, not a cap the provider enforces.
- **A billed answer that cannot be used** (a model that was not offered, a missing field) is recorded too, once; the heuristic's decision carries no router cost, so nothing is added twice.
- **A call whose cost is unknown** (the request failed, a non-2xx status, a body that is not JSON) records nothing and releases the hold. That is a gap in the accounting that the software cannot close.
- **Dollars and tokens.** The reservation is priced in project dollars at Jev's own rate and is also held in tokens against `max_project_tokens` while the call is in flight. Once it has finished, its cost is on the project's dollars only: router tokens are not added to the agents' token count that `max_project_tokens` is measured against.
- **API calls** still record first and release after, which counts a call twice for an instant. That can only refuse a call, never admit one too many, so it was left alone.

### Strict-budget status

Jev is not a fully bounded path, and this page does not describe it as one. The existing configuration does provide one project-wide bypass: `[router] backend = "heuristic"`. With it, no request is made to Jev (`jev_settlement.rs`: not one request reaches a mock Jev). It is not per task, nothing switches it on for a strict task, and the software does not enforce that a strict task uses it. A pinned model does not bypass Jev: Jev is asked first and the pin overrides its answer afterwards (also tested), so a pin saves no routing spend. No implicit strict mode was added.

## `max_project_tokens`

`[orchestrator] max_project_tokens` is an optional whole number in the project's `backspace.toml`, next to `max_project_usd`. It was added in the second review round so that a token scope shared by more than one agent exists.

- **Parsed, written back, optional.** Read when the project opens; a config without it loads as before with no project token limit; the shipped example documents it commented out (`max_project_tokens.rs`).
- **Scope.** The `project` scope: one line shared by every API reservation, every CLI admission and every Jev hold in the project, measured against the agents' own raw provider tokens (input plus output, cache reads and writes at face value). Router tokens are not in that count.
- **API calls** reserve their worst case (input bound plus granted output) against it and are refused before the request if it does not fit, with a refusal that names the project budget. The tokens a finished call used count against the next one.
- **Coding-CLI turns** are held the whole of what is left. That is what makes two turns unable to share the same allowance, and it is also why turns on this budget run **one at a time**: a second turn waits for the first to end and then asks again, and when the budget is spent it is refused.
- **While a CLI turn runs, API calls on that budget are refused, not queued.** They get a budget refusal straight away; they do not wait for the turn to end.
- **Mixed load.** Started together, API calls and CLI turns never hold more than the limit between them (`max_project_tokens.rs`).

Keeping it as a supported setting means accepting these:

1. CLI workers on the shared token budget are serialized, which can make a project with several of them slow.
2. An API agent can be refused in the middle of a CLI turn instead of waiting, so a project that mixes both on one token budget will see refusals it can retry later.
3. A project that does not set it is not affected: nothing is serialized by it, and a per-agent token limit still binds on its own.
4. Claude Code reports usage once per turn, so one turn can pass the limit; the next is refused. Total spend can therefore pass the limit by up to one turn.
5. The router's tokens are never counted toward it.
6. A dollar limit without it still refuses every coding-CLI turn.

A smaller per-turn allowance, so that turns could overlap, is a design change this pass does not make.

## Strict budgets

There is no strict-budget mode. Nothing in the software puts a task or a project into one, and no path above is a fully bounded spend. Three different kinds of statement follow, kept apart on purpose.

### Operational recommendation

Things a person does. The software does not enforce them.

1. **Set `[router] backend = "heuristic"`** for a project with a strict requirement, so Jev is never asked. A pin is not a substitute.
2. **Do not use plan-billed CLI workers** (Claude Code, Codex and the rest). Their spend is observed only as the CLI reports it. Route strict tasks to API models.
3. **Switch the advisor off**, on API turns and in Claude Code: leave its model, and the aliases `opus` and `haiku`, out of the lists (see below).
4. **Keep `CloudChatServer` and `AgentChat` off the project's accounts.** No setting disables them.
5. **Do not send strict tasks to a remote machine** unless its own `backspace.toml` carries the budget.

### Enforced by configuration

Settings that restrict something, each with a test. They restrict that thing and no more.

- `max_project_usd` (or `[policy.budget] max_usd`) with no `max_project_tokens` beside it refuses every coding-CLI turn before it starts (`cli_budget.rs`, `cli_concurrency.rs`).
- `[router] backend = "heuristic"`: no request goes to Jev (`jev_settlement.rs`).
- `orchestration_models` and `execution_models` that do not name a model, a coding-CLI entry, the API advisor or the aliases `opus` and `haiku`: it is not used, and Claude Code is started without them (`advisor_policy.rs`, `cli_policy.rs`).
- `max_project_tokens` and `[policy.budget] max_tokens` bound API reservations and CLI admissions, under the assumptions above.

### Limitation the software cannot enforce

- Plan-billed CLI spend is whatever the CLI reports; Claude Code can overshoot within one reported turn. Codex's cumulative versus per-turn semantics are not verified.
- Advisor output is not provider-capped, and Jev's answer size is an assumption.
- `CloudChatServer` and `AgentChat` are unmetered and outside the project's policy. Not enforced.
- A remote machine has its own policy and budgets.
- A strict task cannot be told apart from any other: nothing makes it bypass Jev or the advisor on its own.
- Policy integrity is not an OS-level boundary until the M6 sandbox.

What this leaves for a strict project: API turns on reserved models, with the heuristic backend and the other paths unused or switched off, under the byte-per-token and pricing assumptions. That is the closest M1 gets, and it still rests on those assumptions.

## Wording to use

- Say **reserved**, **admitted** or **unmetered** and give the limit; do not say a project has a hard budget guarantee. A reservation is a bound only under its assumptions.
- Say which list a model was checked against, or that none was. Do not say a path is covered by the policy because it is covered by the budget, or the other way round.
- A CLI turn is bounded by the usage it reports, not by a spend flag. No CLI-native spend flag was verified, so none is relied on.
- A dollar budget is a promise about priced API models only. It cannot bind a plan-billed CLI, which is why it refuses instead of passing.
