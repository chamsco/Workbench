# M1 budget boundaries

Which model calls a project's budget and policy cover, how, and which they do not. Written for the hardening pass; `crates/core/tests/unmetered.rs` fails if a model call is added that is not on this list.

A call is **reserved** (a gateway reservation is held against every budget it counts toward before anything is sent), **admitted** (CLI turns: checked at the start, stopped on reported usage), or **unmetered** for one of three named reasons. Nothing else leaves the process.

| Path | Class | What is enforced | What is not |
|---|---|---|---|
| API agent turn (`drive`: Main, orchestrators, workers, scouts, triage on an API model) | reserved | Model allowlist, then a worst-case reservation (request bytes as sent + fixed allowance + the whole granted output) against the task limit, every ticket budget above the agent, and the project limit. Refused before the request if it does not fit. The request is refused if it grew after the reservation. | The bound rests on one assumption: a token covers at least one input byte. See `M1-HARDENING-NOTES.md`. |
| The advisor on an API turn (Anthropic server tool) | reserved, **bounded by assumption** | A second reservation at the advisor's price over the same input bound plus `advisor.max_output_tokens` (default 4,096). If it does not fit, the advisor is left off that call and the log says so; the call itself still goes ahead. | The advisor tool takes no output cap, so what it writes is **not enforced** by the provider. Real use above the assumed cap is logged as an `overshoot` and counted in full. |
| CLI worker turn (`finish_cli`) and a planner running in a CLI (`plan_on_cli`) | admitted | Refused before the turn when any budget it counts toward is already spent. The turn gets a token ceiling (what the task has left); when its reported usage passes that, it is stopped. A dollar limit with a CLI model that has no price **fails closed** (refused, naming the model) unless a token limit sits beside it. | A CLI states its usage as it goes (Codex: at each `turn.completed`) or once, at the end (Claude Code: the `result`). A Claude Code turn is therefore **not enforced within the turn**: it can run on past the ceiling and is counted afterwards, and the next round is refused. Overshoot is bounded by one turn's report, not zero. |
| Claude Code's own advisor (`--advisor`) and scout subagent (`--agents`) | inside the CLI | Counted only as far as the CLI includes them in the usage it reports. | **Not enforced** by the gateway: they are not separate calls this process sees. |
| `CloudChatServer`: the development Backspace Cloud server answering chat with the models in its own config | unmetered | Nothing. It has no project, no task and no policy of its own. | **Not enforced.** Run it only against accounts you are prepared to spend on. |
| `AgentChat`: chat with an agent, a bot or a routine, over a CLI, a local model or a router | unmetered | Nothing task-scoped. It is not a task, so no task budget applies. | **Not enforced.** |
| `JevRouter`: the Jev router's own classification call | unmetered | It is added to the project's total cost afterwards (router cost). | It is never checked against a limit beforehand, so a project limit can be passed by the router's cost. Small by design, but **not enforced**. |
| A remote machine's harness | out of this process | Nothing here: it runs under the policy and budgets of its own `backspace.toml`. | This project's policy and budget do not follow a task to a **remote** machine. |
| Plugins | no call site yet | Gateway-level unit tests only (`check_plugin`). | There is no loader until M11. The evidence is gateway-level, not end to end. |

## Wording to use

- "Hard" is claimed only for API agent turns and only under the byte-per-token assumption. For anything else say **bounded** or **not enforced**, as in the table.
- A CLI turn is bounded by the usage it reports, not by a spend flag. No CLI-native spend flag was verified, so none is relied on.
- A dollar budget is a promise about priced models. It cannot bind a plan-billed CLI, which is why it refuses instead of passing.
