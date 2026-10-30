# M2 notes

## Test changes you should know about

- **Replaced assertion** in `router::tests::heuristic_scales_with_task` (as announced in the M0 audit): `main.effort >= Xhigh` for "make a todo app" is now two assertions, a small Main task stays below Xhigh and a long one is at least Xhigh. The other assertions are unchanged. End-to-end routing is in `tests/router_policy.rs`.
- `tests/gateway.rs` (M1): `class: None` added to its `ModelSpec` literal. Compile-only.
- `tests/e2e.rs`: one test appended, nothing existing edited.
- Two tests were added after the red commit, once the e2e run showed gaps: CLI turns were not counted (`tests/metrics_cli.rs`) and decision-model spend was not in the total (`record_decision` in `tests/metrics.rs`). Each was shown failing before the fix.
- `a_whole_project_reports_the_section_3_4_metrics` first failed on `human_tokens_read`: approval cards were not counted as read. I fixed the implementation (summaries and diff stats shown for approval now count), not the assertion.
- Five new tests pass on the skeleton by construction (three routing controls that guard existing behaviour, an unknown `class` being a config error, a missing CLI file being an error).

## Design choices

- **Class by tier** is a proposal. The alternative is explicit-only, which makes every existing config say nothing about frontier. Default-by-tier keeps the default config working.
- **Unsure starts mid, it does not go to the frontier.** The risk is that an unclear but hard task gets a weaker planner; the planner then climbs on measured failure. The `Ask` signal is on `Decision.triage` for M3's intake to act on; nothing asks the user yet.
- The class ceiling filters router candidates after the M1 allowlist. If the allowlist leaves no mid model, the ceiling is ignored rather than failing the route.
- A decider that errors leaves the task unclassified (starts mid, with a note on the decision).
- **Calls:** a CLI turn is one call of its model's class, with the tokens the CLI reports and $0. An advisor consultation is its own call, on the advisor's class. Decision-model (Jev) spend is cost but not a call. API model calls are one each.
- **Human tokens** are `ceil(chars / 3)` of what you typed (messages, rejection feedback) and what you were shown (main-agent replies, approval summaries and diff stats). A lower bound.
- `cost_to_green_usd` is total spend at the most recent green result, retries and escalations included.

## Known gaps

- No labeled traces, so no eval report, no accuracy or calibration number, and no basis yet for the threshold.
- The planner in a coding CLI (`plan_on_cli`) never calls the router, so it has no classification and no ceiling.
- Jev's own model choice is capped by class but untested here (needs a key).
- `cloud.rs` chat spend and anything outside `drive()` and CLI turns is not in the ledger.
- A CLI turn hides the model calls inside it, so frontier calls are undercounted for CLI workers.
- The webhook flake above.
