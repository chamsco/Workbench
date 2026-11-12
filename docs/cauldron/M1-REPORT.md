# M1 report: policy gateway, allowlists and pre-call budgets

> **Superseded in part by the hardening pass.** The gaps listed below for CLI workers, the advisor, cloud chat, the router and `backspace.toml` edited from bash were reworked afterwards; what is enforced now, and what is not, is in `M1-BUDGET-BOUNDARIES.md` and `M1-HARDENING-NOTES.md`. This page is kept as the M1 record.

Branch `claude/project-thread-dpmr8e`. Red tests `f653d98`; green commit follows it. Full suite output: `docs/cauldron/M1-suite-output.txt`.

1. Suite (`cargo test --no-fail-fast`, default members): **237 passed, 0 failed**, 0 compiler warnings. Baseline was 204; +33 new (20 in `tests/gateway.rs`, 13 in `tests/gateway_e2e.rs`). No existing test was edited.
2. Models: a model outside `[policy] orchestration_models` / `execution_models` is refused in code before any request; the e2e mock server's request count stays 0 (both roles, goal text containing injection strings has no effect).
3. Skills: a skill outside `[policy] skills` is refused, left out of the skills index and not injected, for orchestration and execution agents.
4. Plugins: refused at gateway level in both roles (unit tests). **No plugin loader exists yet, so there is no call site to test end to end; that arrives with M11.**
5. Budgets: `[policy.budget] max_usd / max_tokens` (per task), agent `budget_usd` and `max_project_usd` are all checked before the call; the tightest one wins and is named. The `max_tokens` sent to the provider is clamped to what the budget can pay (e2e: $0.25 gives 250).
6. Injection: decisions use only typed values from the policy file read at `Harness::open`; no prompt, tool output or memory text is read. File tools refuse `backspace.toml`; widening the file after open has no effect on the running session.
7. Tokens used this milestone: **not measured** (no counter available to me). Frontier calls: **0** in the product tests (mock provider only); my own session's calls are not measured.
8. Left: bash can still edit `backspace.toml` on disk (effective on next open) until the M6 sandbox; CLI workers get the model check but no budget clamp; advisor and cloud-chat spend are not budgeted; plugin call site (M11).
9. Details, design choices and three corrections to my own new tests: `docs/cauldron/M1-NOTES.md`.
