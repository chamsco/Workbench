# M3 notes

## Decisions that need you

- **Intake is off by default** (`[intake] enabled = false`). The classifier is the M2 keyword baseline: uncalibrated, and the §10 eval has not run. Turning it on means a wrong guess skips the interview (one-shot) or returns a report instead of building (research). It also keeps every existing e2e test as it was: their task texts ("flat build", "cli-project: say hello") score below the 0.7 threshold, so with intake on they would be asked about, not run. Recommendation: flip the default after the eval, not before.
- **One-shot cap of 5 acceptance lines** (`ONE_SHOT_MAX_ACCEPTANCE`) is what "lightweight" means in code. It is provisional, like the 0.7 threshold.
- **Research has no shell and no web.** `bash` can run `git push`, so it is not available on the research path, and the harness has no web tool at all. Research can `read` and `list` files in the project and answer from the model's own knowledge. A useful research path needs a read-only shell, which is M6's sandbox. I added `list` (read-only directory listing) because `read` alone cannot find a file.

## How it works

- `intake.rs` is pure: `first`/`answer` (classify, ask, parse the reply), `allows`/`refusal` (the tool gate per path), `OneShotSpec` and `Report` (validate and write files). The harness wires it in.
- Asking is code. A first message the classifier is unsure about (or cannot classify) posts the question as an assistant message and waits. No model call and no spend until the user answers (tested: ledger calls == 0, CLI launched 0 times). A reply that names `one-shot`, `long`, `research` or 1/2/3 starts that path on the original task, as a confident classification. Anything else is kept as context and asked once more; after `max_asks` (2) questions the task goes the long way and says so, and it is not treated as a confident long task, so it does not start on the frontier.
- The path and the waiting question are saved with the project, so both survive a restart.
- `intake::allows` gates every tool call (`run_tool`) and the list a model is offered (`tools_for_path`), so they cannot disagree. Research: `read`, `list`, `submit_report`. One-shot: `read`, `list`, `create_tickets` (one ticket), `work_tickets`, `submit_deliverable`. Anything not named is refused, including tools added later.
- Research ends in `submit_report`: a file under `.backspace/reports/` (self-ignored by git, never overwritten), recorded as a `document` artifact. No approval is asked, since a report changes nothing. Test: no ticket, approval, worker or new branch, and a model that tries `create_tickets`, `write` and `bash` is refused each time.
- One-shot: `create_tickets` is validated as a lightweight spec (exactly one ticket, 1 to 5 acceptance lines, no `blocked_by`, no `covers`). The ticket is created ready, with no plan gate and no SPEC.md coverage gate, because the spec stands in for the plan. The spec is also written to `.backspace/handoff/<key>/spec.md`. The worker still gets its brief from the ticket; the file is the record, not what the worker reads. Merge stays your call: deliverables are approved as before.
- When a research or one-shot task is delivered, or you press Stop, its path and saved conversation are cleared and the next message is classified afresh. A long task keeps its conversation, as before.

## Planner in a coding CLI

Intake (classify, ask) runs before the CLI is launched. Backspace's own planning tools obey the path, because they go through the same `create_batch` and `/v1/plan/*`: research is refused, a one-shot gets the same validation and no plan gate. A CLI research task's final message is filed as the report.

**What is not enforced:** the CLI's own file and shell tools. A research task in Claude Code can still write files or run git through the CLI itself; only its system prompt says not to. That needs M6's sandbox. A CLI one-shot has no delivery signal, so its path ends on Stop, not on its own, and until then a second ticket is refused ("already has its ticket"). Not tested: a CLI one-shot dispatching a worker.

## Test changes you should know about

- No existing test was edited. `diagram.rs`'s test-module `ProjectState` literal gained `intake: Default::default()` (compile-only).
- In `tests/intake.rs`, after the red commit: `intake_is_off_by_default_and_must_be_allowed_to_ask` appended a second `[intake]` table to `default-config.toml`, which now has one, so it failed on a duplicate table instead of on the check. I rebuilt its fixture from the test config and kept every assertion, plus one (a project file with no `[intake]` is off). Error text is read with `{:#}`.
- `stopping_a_restricted_task_releases_its_path` was written after the implementation. I showed it fails (40s timeout) with the stop-branch cleanup removed, and passes with it restored.
- Two tests pass in the red state on purpose (long is unrestricted, intake off changes nothing): they guard behaviour that must not move.

## Known gaps

- Everything ran against mock models and a fake `claude`. The research and one-shot prompts (`prompts/research.md`, `prompts/oneshot.md`) have not met a real model.
- No claim about classification quality: the §10 eval is still owed (needs your task texts and your OK on the frontier definition).
- `start_task` (a task you start in parallel from the shell) bypasses intake; it already names a ticket.
- `read` resolves paths lexically and follows symlinks, so a symlink inside the project can point outside it. This predates M3 and applies to every agent, but research makes `read` its main tool.
- A decider that fails is treated as "unsure" and asked about; it does not guess.
