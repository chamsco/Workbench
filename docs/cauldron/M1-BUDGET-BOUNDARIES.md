# M1 budget and policy boundaries

Which model calls a project's budget and model policy cover, how, and which they do not. Written for the M1 review rounds and revised in the final hardening pass. `crates/core/tests/unmetered.rs` fails if a model call is added that is not on this list, `crates/core/tests/budget_wording.rs` fails if a limit below is dropped from this page, and `crates/core/tests/policy_matrix.rs` fails if a path loses its policy column, if a place that sends an HTTP request is not accounted for, if the strict-budget section and the router's row disagree, or if the *Coding-CLI models* section stops matching the CLIs and aliases the code knows.

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
| CLI worker turn (`finish_cli`) | admitted. Refused before the turn when a budget it counts toward is spent. Otherwise it is given everything the tightest token limit has left, and **that allowance is held against every scope it counts toward until the turn ends**, so two turns are never given the same tokens; a second turn on the same budget waits for the first and then asks again. A dollar limit with no token limit beside it is refused, naming the model, whether or not the CLI model has a configured price. | Pinned model checked against the relevant project orchestration/execution allowlist, execution (orchestration when the worker may create tickets), under an enforced policy: the entry is looked up before the turn; if it pins a model (`model = "…"` on a Claude Code or Codex entry) that model is looked up too and put on the command line as `--model` (both CLIs); if it pins none, the turn is refused unless `unpinned_cli_models` names the entry, and then the CLI runs the model its own configuration selects. Checked by the CLI's own configuration, not independently by the gateway, for that unpinned case, for every entry under `legacy_open`, for a pin on a CLI whose command-line precedence is not verified (treated as unpinned), and for how the CLI resolves an alias. | **Claude Code may overshoot within one reported turn**: it reports usage once, at the end, so it is not stopped inside the turn; the overshoot is bounded by that one turn, not zero, and the next turn is refused. Codex reports per `turn.completed` and is stopped when that passes the allowance. Within one process the largest figure reported is taken, and **Codex's cumulative versus per-turn usage semantics are not verified**: if its figures are per turn, the largest undercounts a process that runs several. The tests use fake CLIs, so they show what Backspace passes, not how the real CLI interprets it; the precedence of `--model` over the CLI's environment and settings is taken from the vendors' documentation and, for Codex, its source, not tested against the real CLI. A managed or organization setting inside the CLI can still replace a model, and an alias such as `sonnet` is resolved by the CLI (its own `ANTHROPIC_DEFAULT_*_MODEL` settings can remap it): pin a full model id. What is verified, what is not, and what is excepted is in *Coding-CLI models* below. |
| CLI planner (`plan_on_cli`: the main agent running in a coding CLI) | admitted. The same as a CLI worker turn. | Pinned model checked against the relevant project orchestration/execution allowlist, orchestration, exactly as for a worker (Claude Code and Codex entries pin; the others count as unpinned): the entry the planner runs as (the settings pick, then `pin_main`) is the one looked up, its pinned model goes on the command line, and an entry that pins none needs `unpinned_cli_models`. Checked by the CLI's own configuration, not independently by the gateway, in the same unpinned, `legacy_open` and unverified-CLI cases. | The same as a CLI worker turn, including fake CLIs, managed settings and aliases. |
| Claude Code's own advisor (`--advisor`) | admitted, inside the CLI turn and not separately metered. Counted only as far as the CLI includes it in the usage it reports. | Checked against the relevant project orchestration/execution allowlist: `[advisor] cli` (default `opus`) is looked up as written, at the role of the agent that would consult it, before the turn. If the lists do not name it, `--advisor` is left off and the agent's log says so (`cli_policy.rs`). Only the advisor Backspace passes is checked; whatever the CLI's own settings add is not. | Not enforced by the gateway: it is not a separate call this process sees. |
| Claude Code's scout subagent (`--agents`) | admitted, inside the CLI turn and not separately metered. Counted only as far as the CLI includes it in the usage it reports. | Checked against the relevant project orchestration/execution allowlist: `[advisor] cli_scouts` (default `haiku`) is looked up as written, at the execution role. If the lists do not name it, the scout is left off and the log says so (`cli_policy.rs`). Only the scout Backspace passes is checked; under an enforced policy `CLAUDE_CODE_SUBAGENT_MODEL` is not handed to Claude Code, so it cannot replace that model (`cli_model_policy.rs`). | Not enforced by the gateway: it is not a separate call this process sees. **A subagent the running Claude Code model starts with a model of its own (the per-invocation `model` parameter takes `sonnet`, `opus`, `haiku` or `fable`), and Claude Code's built-in subagents, are not checked by Backspace**: it never sees that choice. |
| JevRouter's own model: the router's classification call (`[router] jev_model` at `jev_endpoint`) | reserved when a project budget is around it; a router with no budget is unmetered (a script, the eval declare `Unmetered(JevRouter)`). Inside a harness the call's worst case (its body in bytes bounded as above, plus an assumed 512-token answer, at Jev's own rate `jev_usd_per_mtok`) is held before the request is sent and counts against the project dollar budget until its real cost is on the books; the two happen in one step (see *JevRouter*). If it does not fit, Jev is not called and the heuristic decides. Its tokens are held while in flight but afterwards it does not count toward `max_project_tokens`. | Checked against the project's routing-model policy: `jev_model` must be named in `routing_models`, a list separate from the orchestration or execution lists; naming Jev in those is not a permission, and `routing_models` is not a way to make Jev a candidate for either role. Under an enforced policy a missing or empty list means Jev is not called and the heuristic decides, with a note and an open-time warning (`routing_policy.rs`). Generated policies name the configured model. The heuristic backend (`backend = "heuristic"`) is the project-wide bypass. | A task-scoped routing call is checked against the project budget only: the task has no agent yet, so there is no per-task budget to check. Jev's answer size is an assumption, not a cap. **Unknown cost:** a request that was sent and then timed out, was cut off, got a non-2xx status or a body that is not JSON, or answered without usage, has an unknown cost; the amount held for it (its reservation) is counted as spent, marked unknown, and never freed as zero. A request that never left (refused connection, malformed endpoint) releases its hold and records nothing. A router with no budget around it is unmetered: nothing limits it. Jev's tokens are not counted toward `max_project_tokens`. |
| JevRouter's candidate models: the models it may pick | reserved or admitted, on the path of the model it picks (the pick then runs on the API-turn or CLI row). The routing call itself is the row above. | Checked against the relevant project orchestration/execution allowlist: Jev is offered only the candidates the lists allow for the role being routed, and a name outside them is not used (`jev_settlement.rs`). The candidate set is never widened by `routing_models`. | The router's classification model and the models it chooses between are different things, with different lists. |
| `CloudChatServer`: the development Backspace Cloud server answering chat with the models in its own config | unmetered. Nothing. It has no project, no task and no policy of its own. | Not covered by the project's policy: the server has no project and reads no `[policy]`. | **Not enforced.** It is outside task-budget enforcement. |
| `AgentChat`: chat with an agent, a bot or a routine, over a CLI, a local model, a router or Backspace Cloud | unmetered. Nothing task-scoped. It is not a task, so no task budget applies. | Not covered by the project's policy: a chat over a CLI, a local model, a router or Backspace Cloud takes its model from the chat's own route, not from the project's lists. | **Not enforced.** It is outside task-budget enforcement. The Backspace Cloud route is a direct request (`/v1/chat`, with the user's cloud token) that the source scan in `unmetered.rs` cannot see by name; `policy_matrix.rs` keeps it listed. |
| A remote machine's harness | out of this process. Nothing here: it runs under the budgets of its own `backspace.toml`. | Runs under the remote machine's own policy: this project's lists do not follow a task there. | This project's budget does not follow a task to a **remote** machine. Not enforced from here. |
| Plugins | no call site yet. Gateway-level unit tests only (`check_plugin`). | Gateway-level check only; no end-to-end loader exists yet: `check_plugin` refuses a plugin the `plugins` list does not name, in unit tests, and nothing calls it until the loader exists. | There is no loader until M11. The evidence is gateway-level, not end to end. |
| browser_task's decision model (`hands/decide.rs`: Jev, its text model, or a local model, from the user's Browser settings) | unmetered. Nothing task-scoped: the fast loop's requests are not reserved or counted against any budget. | Checked against the relevant project orchestration/execution allowlist inside a project: before a browser or a model is touched, the agent's `backspace mcp` asks the bridge (`/v1/policy/model`), and every model the decider calls (Jev and its text model, or the local model) must be in the list for the agent's role (orchestration for the main agent, execution for the others); a refusal runs nothing (`hands_policy.rs`, `hands_policy_bridge.rs`). Not covered by the project's policy outside a project (a chat agent with hands has no bridge and no project lists). | **Not enforced:** its spend is not budgeted. A worker that may create tickets is checked at the execution role here, not orchestration. The models come from the user's settings, not from `[[models]]`, so a list must name them as the settings spell them (`jev-latest`, `inception/mercury-2.5`, the local model id). |

The unmetered paths, `CloudChatServer`, `AgentChat` and, where no budget is around it, `JevRouter`, remain **outside task-budget enforcement**.

How the two Claude Code helper rows are matched: the alias in `[advisor] cli` and `cli_scouts` (`opus`, `haiku`) is compared as written with the names in the lists, since that is the name Backspace passes to the CLI. A list the person wrote must carry the alias to keep the advisor or scout. A policy that Backspace writes out (a new project, `backspace-cli policy init`) names the aliases the config already uses, so those projects keep what they had; a list written by hand is never widened by them.

How a coding-CLI model is matched: a Claude Code or Codex entry that sets `model` pins it. Backspace puts that model on the command line as `--model`, where the vendors document it as taking precedence over the CLI's environment variables and settings files (Claude Code: `--model` over `ANTHROPIC_MODEL` and the `model` setting; Codex: CLI flags and `--config` overrides are the top layer, above project, profile and user `config.toml`). That is what keeps the environment, a project's own CLI settings or the text of a task from changing the model after it was checked. Cursor, Grok and OpenCode have no verified precedence, so their entries count as unpinned. The default config's CLI entries pin nothing, so a generated policy lists them in `unpinned_cli_models`; a person who wants the model checked sets `model` on the entry.

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

## Coding-CLI models: what is enforced, and where

A coding CLI is one entry in `[[models]]`, but the model it runs is decided partly outside Backspace. Five kinds of statement follow, kept apart, each with its evidence. **"Verified" always means verified on Backspace's side**: what it checks and what it passes, shown by tests with fake CLIs and by argument tests in `crates/runner`. It never means that a real CLI was run and seen to use the model.

### Verified model-level enforcement

Under `mode = "enforced"`, for a Claude Code or Codex entry that sets `model` (`PINNABLE_CLIS` in `config.rs`; `cli_model_policy.rs` and the runner's argument tests).

- The entry's `model` is read from the config the project opened with. Editing `backspace.toml` afterwards changes nothing (tested for Claude Code and for Codex).
- It is looked up, as written, in the list for the agent's role: `execution_models` for a worker, `orchestration_models` for a planner and for a worker that may create tickets. A planner is checked as the entry it runs as. A model the list does not name is refused before the CLI starts, naming the model (Claude Code and Codex, planner and worker, each tested; the two lists are tested to be independent).
- The same string is passed as `--model <model>`, once, for both CLIs. For Codex `--model` is the spelling its documentation names, so the runner uses it (before this closure it passed `-m`, the same argument; see below). A resumed Codex session gets it ahead of `resume`, where it is an option of `codex exec`.
- Nothing else Backspace passes can name a model: the other `-c key=value` overrides it builds are the reasoning effort and MCP servers, and it passes no `--profile`, `-p` or `--oss`.
- Under an enforced policy a pinned Claude Code, or one given a scout, is not handed `CLAUDE_CODE_SUBAGENT_MODEL`, and the agent's log says when it was set. The main model's `ANTHROPIC_MODEL` is left alone: `--model` is documented to rank above it.
- The environment of the process or the profile (`CODEX_HOME`, `CLAUDE_CONFIG_DIR`) does not change what Backspace passes (tested with a hostile `ANTHROPIC_MODEL` and `CODEX_MODEL`; the latter is a stand-in, since no Codex model variable is documented).
- A project with no `[policy]`, or with `legacy_open`, passes no model, as before.

### CLI-entry-only enforcement

Only the entry's name is checked against the lists, and the model is whatever the CLI selects:

- an entry that sets no `model` (the default config's CLI entries are like this);
- an entry on Cursor, Grok or OpenCode, even if it sets `model`: their command-line precedence is not verified here, so they count as unpinned and nothing is passed;
- every entry under `legacy_open`, or with no `[policy]`.

Claude Code's advisor and scouts are not in this group: Backspace names those models, so they are checked as models (their rows above).

### Explicitly permitted unpinned CLI execution

`unpinned_cli_models` names, entry by entry, the CLI entries that may run without a pinned model. A missing list allows none, and so does `[]`. The turn's log says the CLI runs the model its own configuration selects. The project opens with a warning for each entry that would be refused; adding an entry is a loosening that waits for `policy adopt`; a policy Backspace generates names the unpinned entries of the config it was generated from, and a list a person wrote is never widened by that.

### Unverified external CLI behaviour

What Backspace relies on and has not seen happen. No real model request was made for any of it.

- **Codex flags.** `--model` and `-m` are the same argument: `#[arg(long, short = 'm')] pub model` in Codex's shared options (`codex-rs/utils/cli/src/shared_options.rs`, main branch), and the official command reference lists `--model, -m` for `codex` and for `codex exec`. The npm package `@openai/codex` 0.166.0 (the latest on 2026-10-31) was downloaded into a scratch directory and **not run**: the safety check refused to execute a downloaded binary, and no Codex is installed here. A string search of that binary finds the same help text ("Model the agent should use"). So the equivalence rests on source and documentation, not on `codex exec --help`; the source read is the main branch, not the 0.166.0 tag. To close it: `codex --version` and `codex exec --help | grep -i model`.
- **Codex precedence.** The official configuration page orders CLI flags and `--config` overrides above project `.codex/config.toml`, profile files, user `config.toml`, cloud-managed defaults and system config. In the source the model flag is its own override, separate from `-c`. The page read names no environment variable that selects a model (the environment-variables page was not read).
- **Codex managed layers.** The managed-configuration page says `managed_config.toml` and macOS managed preferences override the user's `config.toml` and `--config` overrides; `requirements.toml` lists approval, sandbox and similar keys and no model key. Whether a managed layer overrides `--model` is not documented. On a machine with managed Codex configuration the model may differ from the approved one.
- **Codex resume.** A resumed session is sent the command-line model in the resume request (Codex source, `exec/src/lib.rs`); whether the server then keeps the saved session's model or this one is decided in code that was not read.
- **The endpoint a model name is served from.** Both CLIs take a model name, not a place; Claude Code's documentation says `ANTHROPIC_BASE_URL` changes where requests are sent, not which model answers. Backspace does not check or set where a CLI sends its requests, so an approved name may be answered by something else.
- **Claude Code flags and precedence.** The installed Claude Code 2.1.316 lists `--model <model>`, `--agents` and `--disallowedTools` in `--help` (run here, no model request). Its `--help` does not list `--advisor`; the model-config page names the flag and it may be unlisted, so it was not exercised. The model-config page orders: `/model` during a session, `--model`, `ANTHROPIC_MODEL`, the `model` setting, then `ANTHROPIC_DEFAULT_MODEL`. Backspace never sends `/model`.
- **Claude Code managed settings.** With `availableModels` in managed settings, Claude Code replaces a `--model` value it does not allow, at startup, with a warning, and starts on the default model; a blocked `--advisor` makes it exit with an error. An organization setting can therefore replace the approved model, and Backspace does not read the warning.
- **A Claude Code alias.** `default`, `best`, `fable`, `sonnet`, `opus`, `haiku` and `opusplan` are resolved by Claude Code to the newest version its provider has (an earlier one on some providers), and `ANTHROPIC_DEFAULT_*_MODEL` remaps them. A list that names `opus` approves a family, not a model. Codex documents no aliases; whether a service re-points a Codex model name to a newer snapshot is not known to Backspace, so list a dated identifier where the vendor offers one.
- **A Claude Code subagent's model.** Documented order since v2.1.251: the per-invocation `model` parameter, then the definition's `model`, then `CLAUDE_CODE_SUBAGENT_MODEL`, then the main model; before v2.1.251 the variable came first. The per-invocation parameter is chosen by the running model when it calls the Agent tool, and Backspace never sees it.
- **Fake CLIs.** The tests use fake `claude`, `codex` and `cursor-agent` scripts that record what they were started with. They show what Backspace passes, not how the real CLI reads it.

### Known policy exceptions

Each is accepted in M1 and written down here, not closed.

- A Claude Code entry may pin an alias. It is allowed and noted when the project opens (an alias names a family, not a model); a generated policy names the aliases `opus` (advisor) and `haiku` (scouts) because the config uses them.
- An unpinned CLI entry the policy names runs a model Backspace did not check.
- A subagent Claude Code starts with a model of its own, and its built-in subagents, are outside the lists. Closing it would mean switching the Agent tool off, which also removes the scouts; that was not done.
- A managed or organization setting inside a CLI can replace the model (Claude Code `availableModels`; Codex managed layers, undocumented for `--model`). Backspace does not detect it.
- The policy restore guard is not an OS-level boundary; M6 is still needed. `AgentChat` and `CloudChatServer` are outside the project's policy (their rows).

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
- A pinned Claude Code, or one given a scout, under an enforced policy is not handed `CLAUDE_CODE_SUBAGENT_MODEL` (`cli_model_policy.rs`).
- `max_project_tokens` and `[policy.budget] max_tokens` bound API reservations and CLI admissions, under the assumptions above.

### Limitation the software cannot enforce

- Plan-billed CLI spend is whatever the CLI reports; Claude Code can overshoot within one reported turn. Codex's cumulative versus per-turn semantics are not verified.
- Advisor output is not provider-capped, and Jev's output size is not provider-capped either: the 512-token answer is an assumption. A Jev call's reservation counts against the project dollar budget, and Jev's usage does not count toward `max_project_tokens`. A Jev call whose cost nobody reports is counted at its reservation, which may be more or less than it cost.
- A router with no budget is unmetered: `Unmetered(JevRouter)` applies to scripts and the eval, and no limit binds it.
- The model Jev itself runs is governed by `routing_models`, not by the orchestration or execution lists. That governs whether Backspace calls it, not what the Jev service then does.
- `CloudChatServer` and `AgentChat` are unmetered and outside the project's policy. Not enforced.
- A remote machine has its own policy and budgets.
- A strict task cannot be told apart from any other: nothing makes it bypass Jev or the advisor on its own.
- A subagent that Claude Code's running model starts with a model of its own is outside the lists; a managed setting inside a CLI can replace the approved model; the endpoint a CLI sends to is not checked (see *Coding-CLI models*).
- Policy integrity is not an OS-level boundary until the M6 sandbox.

What this leaves for a strict project: API turns on reserved models, with the heuristic backend and the other paths unused or switched off, under the byte-per-token and pricing assumptions. That is the closest M1 gets, and it still rests on those assumptions.

## Wording to use

- Say **reserved**, **admitted**, **unmetered** or **out of this process** and give the limit; do not say a project has a hard budget guarantee. A reservation is a bound only under its assumptions.
- Say which list a model was checked against, or that none was. Do not say a path is covered by the policy because it is covered by the budget, or the other way round.
- A CLI turn is bounded by the usage it reports, not by a spend flag. No CLI-native spend flag was verified, so none is relied on.
- A CLI test with a fake `claude` or `codex` shows what Backspace passes; it does not show how the real CLI interprets it. Model selection by the real CLI is not tested here.
- A dollar budget is a promise about priced API models only. It cannot bind a plan-billed CLI, which is why it refuses instead of passing.
