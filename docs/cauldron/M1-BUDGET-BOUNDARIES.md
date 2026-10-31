# M1 budget and policy boundaries

Which model calls a project's budget and model policy cover, how, and which they do not. Written for the M1 review rounds and revised in the final hardening pass. `crates/core/tests/unmetered.rs` fails if a model call is added that is not on this list, `crates/core/tests/budget_wording.rs` fails if a limit below is dropped from this page, and `crates/core/tests/policy_matrix.rs` fails if a path loses its policy column, if a place that sends an HTTP request is not accounted for, or if the strict-budget section and the router's row disagree.

## Two questions per path

For every path the table below answers two questions, in two separate places:

1. **Does the budget cover it?** The *Budget classification and enforcement* column.
2. **Does the model policy cover it?** The *Project policy check* column: which list, at which role, or which policy.

A third column, *Remaining limitation*, says what is still not done for that path. Neither is inferred from the other. A path can be reserved against the budget and sit outside the policy, or be checked against the policy and be unmetered, or be neither. Read the columns separately.

**Budget classification.** Every model call is **reserved** (a gateway reservation holds its worst case against each budget it counts toward before anything is sent), **admitted** (coding-CLI turns: checked at the start, holding an allowance, stopped on reported usage), **unmetered** (it passes no budget; declared on a closed list), or **out of this process** (it runs on another machine). Nothing else leaves the process. None of these is a hard budget guarantee in general; where a row says *not enforced*, no budget limits that path.

**Policy checks.** The policy column uses these phrases and no others:

- *Checked against the relevant project orchestration/execution allowlist.* The model is looked up, as written, in `orchestration_models` or `execution_models`, whichever is the role of the agent. Orchestration is the main agent, a planner running in a CLI, and any agent that may create tickets; execution is every other worker, triage and scouts. The row names the role.
- *Checked against the project's routing-model policy.* The router's own model is looked up in `routing_models`, a role of its own. It is not an orchestration or execution model, and naming it in those lists does not permit it.
- *Pinned model checked against the relevant project orchestration/execution allowlist.* A coding-CLI entry that pins a model has that model put on the command line, and it is looked up in the list for the agent's role.
- *Checked by the CLI's own configuration, not independently by the gateway.* Backspace does not choose the model; the CLI runs whichever one its own settings, environment or defaults name. This is what is left when a CLI entry pins nothing and the policy accepts that (`unpinned_cli_models`), and for every CLI under `legacy_open`.
- *Not covered by the project's policy.* No list is consulted.
- *Runs under the remote machine's own policy.*
- *Gateway-level check only; no end-to-end loader exists yet.*

These describe an enforced policy. A project with no `[policy]` section, or with `mode = "legacy_open"`, has open lists, so every check passes; that is announced when the project opens. Under `mode = "enforced"`, `routing_models` and `unpinned_cli_models` follow the same rule as the other lists: a missing list allows nothing, and `[]` allows nothing.

## What a reservation rests on

An API reservation is a bound only under all of these:

1. **The byte-per-token assumption.** The input bound is the exact request body in bytes plus 1,024 (provider-added prompt) plus 8 per message and 16 per tool. It holds if a token covers at least one input byte, that is, if there is at most one token per byte: a measured figure would have to show more than one byte per token. No tokenizer was run, so no figure here is a measured count of bytes per token or of tokens.
2. **Provider overhead.** The 1,024 allowance is justified by Anthropic's documented tool-use system prompt (up to 804 tokens). Overhead beyond it, or a provider that adds more, is not in the bound.
3. **Accurate configured pricing** (pricing accuracy). Dollars are computed from `input_usd_per_mtok` and `output_usd_per_mtok` in the config (Anthropic input at 1.25x for cache writes). A wrong price makes the reservation wrong.
4. **The provider's enforcement of `max_tokens`.** The granted output is sent as the output cap.

If any of these fails, a call can cost more than was reserved. That is logged as an `overshoot`, counted in full, and checked against by the next call. The overspend itself is not prevented.

## The paths

| Path | Budget classification and enforcement | Project policy check | Remaining limitation |
|---|---|---|---|
| API agent turn (`drive`: Main, orchestrators, workers, scouts, triage on an API model) | reserved. Worst-case reservation (request bytes as sent + allowances + the whole granted output) against the task limit, every ticket budget above the agent, and the project limits. Refused before the request if it does not fit; refused if the request grew after the reservation. The spend is recorded first and the hold released after, so for an instant both count: that can only refuse, never over-admit. | Checked against the relevant project orchestration/execution allowlist: the model the router picked (or a pin chose) is looked up for the agent's role before the request is built. Orchestration: the main agent and any agent that may create tickets. Execution: the other workers, triage and scouts. | Everything under *What a reservation rests on*. While a coding-CLI turn holds a shared token budget, API calls on that budget are refused, not queued. |
| The advisor on an API turn (Anthropic server tool) | reserved, assumption only. A second reservation at the advisor's price over the same input bound plus `advisor.max_output_tokens` (default 4,096). If it does not fit, the advisor is left off that call and the log says so; the call itself still goes ahead. | Checked against the relevant project orchestration/execution allowlist: the advisor model is looked up at the role of the agent that calls it. If the lists do not name it, the advisor is left off that call and the call goes ahead (`advisor_policy.rs`). | Advisor output is **not provider-capped**: the tool takes no output cap, so what it writes is not enforced. Real use above the assumed cap is logged as an `overshoot` and counted in full. |
| CLI worker turn (`finish_cli`) | admitted. Refused before the turn when a budget it counts toward is spent. Otherwise it is given everything the tightest token limit has left, and **that allowance is held against every scope it counts toward until the turn ends**, so two turns are never given the same tokens; a second turn on the same budget waits for the first and then asks again. A dollar limit with no token limit beside it is refused, naming the model, whether or not the CLI model has a configured price. | Pinned model checked against the relevant project orchestration/execution allowlist, execution (orchestration when the worker may create tickets), under an enforced policy: the entry is looked up before the turn; if it pins a model (`model = "…"` on a Claude Code or Codex entry) that model is looked up too and put on the command line (`--model` / `-m`); if it pins none, the turn is refused unless `unpinned_cli_models` names the entry, and then the CLI runs the model its own configuration selects. Checked by the CLI's own configuration, not independently by the gateway, for that unpinned case, for every entry under `legacy_open`, for a pin on a CLI whose command-line precedence is not verified (treated as unpinned), and for how the CLI resolves an alias. | **Claude Code may overshoot within one reported turn**: it reports usage once, at the end, so it is not stopped inside the turn; the overshoot is bounded by that one turn, not zero, and the next turn is refused. Codex reports per `turn.completed` and is stopped when that passes the allowance. Within one process the largest figure reported is taken, and **Codex's cumulative versus per-turn usage semantics are not verified**: if its figures are per turn, the largest undercounts a process that runs several. The tests use fake CLIs, so they show what Backspace passes, not how the real CLI interprets it; the precedence of `--model` / `-m` over the CLI's environment and settings is taken from the vendors' documentation, not tested. A managed or organization setting inside the CLI can still replace a model, and an alias such as `sonnet` is resolved by the CLI (its own `ANTHROPIC_DEFAULT_*_MODEL` settings can remap it): pin a full model id. |
| CLI planner (`plan_on_cli`: the main agent running in a coding CLI) | admitted. The same as a CLI worker turn. | Pinned model checked against the relevant project orchestration/execution allowlist, orchestration, exactly as for a worker (Claude Code and Codex entries pin; the others count as unpinned): the entry the planner runs as (the settings pick, then `pin_main`) is the one looked up, its pinned model goes on the command line, and an entry that pins none needs `unpinned_cli_models`. Checked by the CLI's own configuration, not independently by the gateway, in the same unpinned, `legacy_open` and unverified-CLI cases. | The same as a CLI worker turn, including fake CLIs, managed settings and aliases. |
| Claude Code's own advisor (`--advisor`) | admitted, inside the CLI turn and not separately metered. Counted only as far as the CLI includes it in the usage it reports. | Checked against the relevant project orchestration/execution allowlist: `[advisor] cli` (default `opus`) is looked up as written, at the role of the agent that would consult it, before the turn. If the lists do not name it, `--advisor` is left off and the agent's log says so (`cli_policy.rs`). Only the advisor Backspace passes is checked; whatever the CLI's own settings add is not. | Not enforced by the gateway: it is not a separate call this process sees. |
| Claude Code's scout subagent (`--agents`) | admitted, inside the CLI turn and not separately metered. Counted only as far as the CLI includes it in the usage it reports. | Checked against the relevant project orchestration/execution allowlist: `[advisor] cli_scouts` (default `haiku`) is looked up as written, at the execution role. If the lists do not name it, the scout is left off and the log says so (`cli_policy.rs`). Only the scout Backspace passes is checked. | Not enforced by the gateway: it is not a separate call this process sees. |
| JevRouter's own model: the router's classification call (`[router] jev_model` at `jev_endpoint`) | reserved when a project budget is around it; a router with no budget is unmetered (a script, the eval declare `Unmetered(JevRouter)`). Inside a harness the call's worst case (its body in bytes bounded as above, plus an assumed 512-token answer, at Jev's own rate `jev_usd_per_mtok`) is held before the request is sent and counts against the project dollar budget until its real cost is on the books; the two happen in one step (see *JevRouter*). If it does not fit, Jev is not called and the heuristic decides. Its tokens are held while in flight but afterwards it does not count toward `max_project_tokens`. | Checked against the project's routing-model policy: `jev_model` must be named in `routing_models`, a list separate from the orchestration or execution lists; naming Jev in those is not a permission, and `routing_models` is not a way to make Jev a candidate for either role. Under an enforced policy a missing or empty list means Jev is not called and the heuristic decides, with a note and an open-time warning (`routing_policy.rs`). Generated policies name the configured model. The heuristic backend (`backend = "heuristic"`) is the project-wide bypass. | A task-scoped routing call is checked against the project budget only: the task has no agent yet, so there is no per-task budget to check. Jev's answer size is an assumption, not a cap. **Unknown cost:** a request that was sent and then timed out, was cut off, got a non-2xx status or a body that is not JSON, or answered without usage, has an unknown cost; the amount held for it (its reservation) is counted as spent, marked unknown, and never freed as zero. A request that never left (refused connection, malformed endpoint) releases its hold and records nothing. A router with no budget around it is unmetered: nothing limits it. Jev's tokens are not counted toward `max_project_tokens`. |
| JevRouter's candidate models: the models it may pick | reserved or admitted, on the path of the model it picks (the pick then runs on the API-turn or CLI row). The routing call itself is the row above. | Checked against the relevant project orchestration/execution allowlist: Jev is offered only the candidates the lists allow for the role being routed, and a name outside them is not used (`jev_settlement.rs`). The candidate set is never widened by `routing_models`. | The router's classification model and the models it chooses between are different things, with different lists. |
| `CloudChatServer`: the development Backspace Cloud server answering chat with the models in its own config | unmetered. Nothing. It has no project, no task and no policy of its own. | Not covered by the project's policy: the server has no project and reads no `[policy]`. | **Not enforced.** It is outside task-budget enforcement. |
| `AgentChat`: chat with an agent, a bot or a routine, over a CLI, a local model, a router or Backspace Cloud | unmetered. Nothing task-scoped. It is not a task, so no task budget applies. | Not covered by the project's policy: a chat over a CLI, a local model, a router or Backspace Cloud takes its model from the chat's own route, not from the project's lists. | **Not enforced.** It is outside task-budget enforcement. The Backspace Cloud route is a direct request (`/v1/chat`, with the user's cloud token) that the source scan in `unmetered.rs` cannot see by name; `policy_matrix.rs` keeps it listed. |
| A remote machine's harness | out of this process. Nothing here: it runs under the budgets of its own `backspace.toml`. | Runs under the remote machine's own policy: this project's lists do not follow a task there. | This project's budget does not follow a task to a **remote** machine. Not enforced from here. |
| Plugins | no call site yet. Gateway-level unit tests only (`check_plugin`). | Gateway-level check only; no end-to-end loader exists yet: `check_plugin` refuses a plugin the `plugins` list does not name, in unit tests, and nothing calls it until the loader exists. | There is no loader until M11. The evidence is gateway-level, not end to end. |

The unmetered paths, `CloudChatServer`, `AgentChat` and, where no budget is around it, `JevRouter`, remain **outside task-budget enforcement**.

How the two Claude Code helper rows are matched: the alias in `[advisor] cli` and `cli_scouts` (`opus`, `haiku`) is compared as written with the names in the lists, since that is the name Backspace passes to the CLI. A list the person wrote must carry the alias to keep the advisor or scout. A policy that Backspace writes out (a new project, `backspace-cli policy init`) names the aliases the config already uses, so those projects keep what they had; a list written by hand is never widened by them.

How a coding-CLI model is matched: a Claude Code or Codex entry that sets `model` pins it. Backspace puts that model on the command line, where the vendors document it as taking precedence over the CLI's environment variables and settings files (Claude Code: `--model` over `ANTHROPIC_MODEL` and the `model` setting; Codex: command-line arguments over a profile and `config.toml`). That is what keeps the environment, a project's own CLI settings or the text of a task from changing the model after it was checked. Cursor, Grok and OpenCode have no verified precedence, so their entries count as unpinned. The default config's CLI entries pin nothing, so a generated policy lists them in `unpinned_cli_models`; a person who wants the model checked sets `model` on the entry.

## JevRouter

- **When the hold is held.** From before the request is sent until the call's real cost is on the project's books. `Reservation::settle` runs the recording inside the reservation table's own lock and takes the hold out under the same lock hold, so no other reservation (an API call, a CLI admission, another Jev call) can see the hold gone and the spend not recorded (`jev_settlement.rs`).
- **A cost above the estimate.** Recorded in full before the room is given back, and named in the decision's note (`jev overshoot`). The next call is checked against it. The overspend itself is not prevented: the 512-token answer is an estimate of Jev's output, not a cap the provider enforces.
- **A billed answer that cannot be used** (a model that was not offered, a missing field) is recorded too, once; the heuristic's decision carries no router cost, so nothing is added twice.
- **Four outcomes for a call, decided by what is known** (`jev_unknown_cost.rs`):
  1. *Failed before dispatch* (the connection was refused, the endpoint is not a URL): nothing was sent, the hold is released and nothing is recorded.
  2. *Succeeded with a known cost:* the actual cost is recorded, including above the reservation, in the step that releases the hold.
  3. *Failed after dispatch with an unknown cost* (a timeout, a connection cut after the request was written, a non-2xx status or a body that is not JSON with no usage in it, or a successful answer that reports none): the reservation is **not** freed as zero. The amount held is recorded as spent, marked as unknown (`router_unknown_cost_usd`, part of `router_cost_usd` and the project total, persisted with them), in the same locked step that gives the hold back. It is neither forgotten, nor made available again, nor counted twice: a retry is a new call with its own hold. A successful answer with no usage is used, and counted at its hold. This is conservative accounting, not a measured cost, and the person is told in the decision's note.
  4. *Failed after dispatch with a known cost* (a non-2xx whose body reports usage, an answer that cannot be used): the reported cost is recorded and the hold released in the same step.
- **Where the unknown cost is counted.** With a project budget around the router (a harness), in that project's state, which is saved with the rest of it. A router without one has no hold to count and says so in the note; nothing is persisted for it.
- **Dollars and tokens.** The reservation is priced in project dollars at Jev's own rate and is also held in tokens against `max_project_tokens` while the call is in flight. Once it has finished, its cost is on the project's dollars only: router tokens are not added to the agents' token count that `max_project_tokens` is measured against.
- **API calls** still record first and release after, which counts a call twice for an instant. That can only refuse a call, never admit one too many, so it was left alone.

### Jev's own model and the policy

`jev_model` is a model call with a role of its own. Under an enforced policy `routing_models` must name it; if it does not, no request is made, no key is read, nothing is reserved, and the heuristic decides, with a note in the decision and a warning when the project opens. Nothing widens the orchestration or execution lists to let it through, and naming it there is not a permission. A legacy or open policy behaves as before. A list that adds a routing model is a loosening and waits for a person (`backspace-cli policy adopt`). The tests are in `routing_policy.rs`.

## `max_project_tokens`

`[orchestrator] max_project_tokens` is an optional whole number in the project's `backspace.toml`, next to `max_project_usd`.

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

A smaller per-turn allowance, so that turns could overlap, is a design change this pass does not make; `cli_turn_tokens` was not introduced.

## Strict budgets

There is no strict-budget mode. Nothing in the software puts a task or a project into one, and no path above is a fully bounded spend. Three different kinds of statement follow, kept apart on purpose.

### Operational recommendation

Things a person does. The software does not enforce them.

1. **Set `[router] backend = "heuristic"`** for a project with a strict requirement, so Jev is never asked. It applies to the whole project; the software does not enforce it for an individual strict task, and nothing switches it on for one. A pin is not a bypass: a pinned model still asks Jev first and overrides the answer afterwards, so a pin saves no routing spend (`jev_settlement.rs`).
2. **Do not use plan-billed CLI workers** (Claude Code, Codex and the rest). Their spend is observed only as the CLI reports it. Route strict tasks to API models.
3. **Switch the advisor off**, on API turns and in Claude Code: leave its model, and the aliases `opus` and `haiku`, out of the lists (see below).
4. **Keep `CloudChatServer` and `AgentChat` off the project's accounts.** No setting disables them.
5. **Do not send strict tasks to a remote machine** unless its own `backspace.toml` carries the budget.
6. **Pin full model ids on coding-CLI entries** (`model = "…"`), so the model the policy checked is the one on the command line.

### Enforced by configuration

Settings that restrict something, each with a test. They restrict that thing and no more.

- `max_project_usd` (or `[policy.budget] max_usd`) with no `max_project_tokens` beside it refuses every coding-CLI turn before it starts (`cli_budget.rs`, `cli_concurrency.rs`).
- `[router] backend = "heuristic"`: no request goes to Jev (`jev_settlement.rs`). It is the project-wide setting; the software does not enforce it for an individual strict task. A router with no budget is unmetered, so a strict project should not run one.
- `orchestration_models` and `execution_models` that do not name a model, a coding-CLI entry or its pinned model, the API advisor or the aliases `opus` and `haiku`: it is not used, and Claude Code is started without them (`advisor_policy.rs`, `cli_policy.rs`, `cli_model_policy.rs`).
- `routing_models` that does not name `jev_model`: Jev is not called and the heuristic decides (`routing_policy.rs`).
- `unpinned_cli_models` that does not name a CLI entry that pins no model: the entry is refused before its turn (`cli_model_policy.rs`).
- `max_project_tokens` and `[policy.budget] max_tokens` bound API reservations and CLI admissions, under the assumptions above.

### Limitation the software cannot enforce

- Plan-billed CLI spend is whatever the CLI reports; Claude Code can overshoot within one reported turn. Codex's cumulative versus per-turn semantics are not verified.
- Advisor output is not provider-capped, and Jev's output size is not provider-capped either: the 512-token answer is an assumption. A Jev call's reservation counts against the project dollar budget, and Jev's usage does not count toward `max_project_tokens`. A Jev call whose cost nobody reports is counted at its reservation, which may be more or less than it cost.
- A router with no budget is unmetered: `Unmetered(JevRouter)` applies to scripts and the eval, and no limit binds it.
- The model Jev itself runs is governed by `routing_models`, not by the orchestration or execution lists. That governs whether Backspace calls it, not what the Jev service then does.
- `CloudChatServer` and `AgentChat` are unmetered and outside the project's policy. Not enforced.
- A remote machine has its own policy and budgets.
- A strict task cannot be told apart from any other: nothing makes it bypass Jev or the advisor on its own.
- Policy integrity is not an OS-level boundary until the M6 sandbox.

What this leaves for a strict project: API turns on reserved models, with the heuristic backend and the other paths unused or switched off, under the byte-per-token and pricing assumptions. That is the closest M1 gets, and it still rests on those assumptions.

## Wording to use

- Say **reserved**, **admitted**, **unmetered** or **out of this process** and give the limit; do not say a project has a hard budget guarantee. A reservation is a bound only under its assumptions.
- Say which list a model was checked against, or that none was. Do not say a path is covered by the policy because it is covered by the budget, or the other way round.
- A CLI turn is bounded by the usage it reports, not by a spend flag. No CLI-native spend flag was verified, so none is relied on.
- A CLI test with a fake `claude` or `codex` shows what Backspace passes; it does not show how the real CLI interprets it. Model selection by the real CLI is not tested here.
- A dollar budget is a promise about priced API models only. It cannot bind a plan-billed CLI, which is why it refuses instead of passing.
