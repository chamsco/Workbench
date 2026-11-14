# M4 notes: the planning stage

PLAN §12: "Checklist enforced. spec.md generated. Baseline captured per task type." Sources: §5.2, §6 (planning stage, baseline table), §3.3 (handoff by file), §10 (decision tier).

## What was built

- `crates/core/src/planning.rs` (new). A typed checklist (`PlanSpec`) with the seven §6 items in order: goal, acceptance criteria, one test per criterion, non-goals, constraints, dependencies, baseline. `check` judges it by rules only. A missing or blank list, a placeholder (TBD, TODO, ???, FIXME), a criterion with no test, or two criteria sharing one test is refused, and each refusal names the item. "None" is accepted as a statement for non-goals, constraints and dependencies, but not for the goal.
- Baseline capture (`capture`) uses deterministic tools only (zero tokens) and runs in the planner's worktree. It only runs once the other six items pass.
  - bug_fix: the repro must fail today.
  - feature: every acceptance test must fail today.
  - ui_change: an optional capture command, then each screenshot must be a PNG or JPEG inside the workspace (symlinks that leave it are refused). Copies go to `.backspace/planning/baseline/`, with SHA-256 and size.
  - refactor: the suite must pass. Its time in ms is recorded, plus an optional perf command's output.
  - research: refused, because it is a report and not a PR.
  - The record is `.backspace/planning/baseline.json`.
- `spec.md` (`.backspace/planning/spec.md`) is rendered by code from the checklist, never by the model. It contains Goal, Acceptance (numbered A1… the way `spec::coverage` reads them), a criterion-to-test table, Non-goals, Constraints, Dependencies, Baseline (commands, exits, ms, screenshot hashes) and the checklist itself.
- The human yes. A passing `submit_spec` writes spec.md, saves its SHA-256 and asks the user in the thread. The next message is read in code (`parse_yes`): only a plain yes counts ("yes but…" does not). A yes is bound to the hash, so a spec.md edited before or after the yes is not approved. Anything other than a yes means the planner must submit again.
- Enforcement is in `create_batch`, so it covers the API planner and the CLI bridge alike. On the long path, Main gets no ticket until the checklist passed and the approved hash equals spec.md on disk. `start_task` is untouched (not planned, as in M3).
- Phase record: every checklist answer is kept as a rules gate on Specify, but a rules pass does not advance the phase. The yes is recorded as a Human gate and moves Specify to Plan.
- Wiring:
  - The long-path API planner gets `submit_spec` and `prompts/planning.md`.
  - A CLI planner gets the MCP tool `submit_spec` through `/v1/plan/spec`.
  - Research and one-shot are refused by `intake::allows`, because the tool is not on their lists.
- Decision model: `questions()` asks one typed met/missing question per item. `score()` can only refuse a checklist the rules passed. A checklist the rules refused is never put to the model. An unusable answer is an error, never a pass. It is not wired to any model (see below).

## M2 eval dependency

The §10 eval report is still owed. No labelled traces exist: it needs Charles's task texts and his OK on the frontier definition. Checklist enforcement does not depend on it, because enforcement is rules plus the human yes. Model scoring is the only part that would, and it stays unwired, per the decision "no decision model before the §10 eval".

## Limits and open choices

1. Enforced only when intake routes a task to the long path. Intake is still off by default (M3), and with it off the planner works as before, with no checklist (tested).
2. ~~Baseline commands run before the user's yes.~~ Closed (14 Nov): they are put to the user as an exact list first and run only after a plain yes to that list (hash-bound). They still run unsandboxed, in the planner's worktree, under `bash_timeout_secs` (accepted until M6).
3. Two spec files exist. `docs/cauldron/PLANNING-CONTRACT.md` makes `.backspace/planning/spec.md` canonical for M5, and Plan mode's root `SPEC.md` is not an input to M5. Merging them is a later decision.
4. `tests.md` is not generated. §5.2 lists it, but M4's acceptance names spec.md only, and Rampart writes the tests in M5.
5. The yes is a chat reply. There is no approval card or button (UI is the other thread).
6. As in M3, a delivered long task keeps the long path, so a later task in the same project plans under the already approved spec until a new `submit_spec`.
7. The CLI test is Unix only (fake `claude` + curl). Windows ran no test here. Prompts have not met a real model.

## Closure (14 Nov 2026, Charles's items)

1. **Baseline commands need approval before they run.** `planning::commands` lists what a capture would run. `submit_spec` puts the list to the user and runs nothing. A plain yes approves that list (`commands_approved_sha256`), and the next submission runs it. A changed command is a new list and needs a new yes. Screenshots alone run nothing and are not asked about.
2. **`hands/decide.rs` is in the policy audit.**
   - It is listed in `policy_matrix.rs` (`POSTS`) and has a row in `M1-BUDGET-BOUNDARIES.md`.
   - Inside a project, `browser_task` asks the bridge (`/v1/policy/model`) before a browser or model is touched. Every decider model must be in the list for the agent's role, and a refusal runs nothing.
   - Outside a project (no bridge) it is not covered.
   - Its spend is unmetered.
   - A worker that may create tickets is checked at the execution role.
3. **Canonical spec path:** `docs/cauldron/PLANNING-CONTRACT.md`.
4. **Intake stays off by default** until the §10 evaluation, as stated in the contract and tested.
