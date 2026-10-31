# M1 notes

> **Superseded in part by the hardening pass.** The gaps listed below for CLI workers, the advisor, cloud chat, the router and `backspace.toml` edited from bash were reworked afterwards; what is enforced now, and what is not, is in `M1-BUDGET-BOUNDARIES.md` and `M1-HARDENING-NOTES.md`. This page is kept as the M1 record.

## Design choices to review

- **No `[policy]` section, or no list inside it, means open.** Once a list is present (even empty) it is the whole allowlist. Exact string match only: no trim, case folding, prefix or glob. Chosen so existing projects keep working. If Charles wants default-deny, it is a one-line change in `Gateway::check_model` / `skill_allowed` / `check_plugin`, plus the default config.
- **Roles:** the main agent and any agent that can spawn (plans tickets) are Orchestration; workers, triage and scouts are Execution.
- **Token estimate is `ceil(chars / 3)`** of the whole prompt. It is a heuristic, deliberately high. The worst case can still overshoot by the estimate's error, and actual usage counts against the next call.
- **Zero-priced models skip USD checks** (nothing to spend) but still obey token budgets.
- **Policy is snapshotted at `Harness::open`;** the gateway has no mutator.

## Corrections to my own new tests (after the red commit `f653d98`)

All three are in `tests/gateway_e2e.rs`, which I wrote in M1. No pre-existing test was touched.

1. `a_skill_outside_the_list_is_refused...`: asserted `!result.contains("grill")`, but the refusal text itself names the skill (`grilling`), so it contradicted the line above it. Now asserts the skill's own body text (`design tree`) is absent.
2. `the_project_budget_is_enforced...`: assertion tightened from `"project"` to `"project budget"` (stricter).
3. `an_agent_cannot_rewrite_the_policy_file...`: asserted no `backspace.toml` exists in the agents' checkout, but the checkout holds a committed copy. Now asserts that copy, if present, is byte-identical to the original. This is weaker on "file absent" and equal on "file not changed"; the policy claim under test is the latter.

## Known gaps

- `bash` is unsandboxed until M6: an agent can edit `backspace.toml` on disk. It cannot change the running session's policy, but the next project open reads it.
- CLI-backed workers (`finish_cli`, `plan_on_cli`) get the model check only; they have no per-call token clamp.
- The advisor and the development cloud chat server (`cloud.rs`, a retained 5.11 capability with its own accounts) make provider calls that the task budget does not cover. `drive()` is the only call site agents use.
- Router: the allowlist filters the router's candidates; a pinned model outside the list is not rerouted, it is refused at call time (covered by the e2e tests).
- No plugin call site until M11; plugin refusal is proven at gateway level only.
