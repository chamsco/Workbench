# M3 notes

## Decisions that need you

- **Intake is off by default** (`[intake] enabled = false`). The classifier is the M2 keyword baseline: uncalibrated, and the §10 eval has not run. Turning it on means a wrong guess skips the interview (one-shot) or returns a report instead of building (research). It also keeps every existing e2e test as it was: their task texts ("flat build", "cli-project: say hello") score below the 0.7 threshold, so with intake on they would be asked about, not run. Recommendation: flip the default after the eval, not before.
- **One-shot cap of 5 acceptance lines** (`ONE_SHOT_MAX_ACCEPTANCE`) is what "lightweight" means in code. It is provisional, like the 0.7 threshold.
- **Research has no shell and no web.** `bash` can run `git push`, so it is not available on the research path, and the harness has no web tool at all. Research can `read` and `list` files in the project and answer from the model's own knowledge. A useful research path needs a read-only shell, which is M6's sandbox. I added `list` (read-only directory listing) because `read` alone cannot find a file.
- **Research is disabled when the planner runs in a coding CLI** (your follow-up instruction, 2026-11-05). See "Planner in a coding CLI".
- **File tools no longer follow a symlink out of the workspace** (same follow-up). See "File boundary".

## How it works

- `intake.rs` is pure: `first`/`answer` (classify, ask, parse the reply), `allows`/`refusal` (the tool gate per path), `OneShotSpec` and `Report` (validate and write files). The harness wires it in.
- Asking is code. A first message the classifier is unsure about (or cannot classify) posts the question as an assistant message and waits. No model call and no spend until the user answers (tested: ledger calls == 0, CLI launched 0 times). A reply that names `one-shot`, `long`, `research` or 1/2/3 starts that path on the original task, as a confident classification. Anything else is kept as context and asked once more; after `max_asks` (2) questions the task goes the long way and says so, and it is not treated as a confident long task, so it does not start on the frontier.
- The path and the waiting question are saved with the project, so both survive a restart.
- `intake::allows` gates every tool call (`run_tool`) and the list a model is offered (`tools_for_path`), so they cannot disagree. Research: `read`, `list`, `submit_report`. One-shot: `read`, `list`, `create_tickets` (one ticket), `work_tickets`, `submit_deliverable`. Anything not named is refused, including tools added later.
- Research ends in `submit_report`: a file under `.backspace/reports/` (self-ignored by git, never overwritten), recorded as a `document` artifact. No approval is asked, since a report changes nothing. Test: no ticket, approval, worker or new branch, and a model that tries `create_tickets`, `write` and `bash` is refused each time.
- One-shot: `create_tickets` is validated as a lightweight spec (exactly one ticket, 1 to 5 acceptance lines, no `blocked_by`, no `covers`). The ticket is created ready, with no plan gate and no SPEC.md coverage gate, because the spec stands in for the plan. The spec is also written to `.backspace/handoff/<key>/spec.md`. The worker still gets its brief from the ticket; the file is the record, not what the worker reads. Merge stays your call: deliverables are approved as before.
- A research or one-shot task ends when it is delivered, when you press Stop, or (a one-shot in a coding CLI) when its ticket settles. Its path and saved conversation are cleared first, and only then does the main agent read Approved or Idle, so anything that sees a task finished also sees its path released. The next message is classified afresh. A long task keeps its conversation, as before.

## Planner in a coding CLI

Intake (classify, ask) runs before the CLI is launched, as on the API path.

- **Research is refused before the CLI runs.** A CLI has its own file and shell tools and code cannot gate them, so "report only" cannot be kept. The refusal is an error line in the main agent's log (it names the cause and says to switch the planner to an API model or ask for a long task), the path and any waiting question are cleared, nothing is launched and nothing is spent. It applies whether the classifier chose research or you answered "research" to the question, and to a research path left in saved state by an earlier session. The earlier behaviour (the CLI's final answer filed as the report) is removed.
- **One-shot runs, with these bounds tested end to end** against a fake `claude`: the bridge refuses two tickets, files one validated ticket with no plan gate, refuses a second ("already has its ticket"), and dispatches it to a worker in its own worktree; you approve the worker's deliverable as usual and the work merges. When the ticket settles the path is released: done counts as a delivery (the worker's deliverable becomes the task's), failed or dropped releases the path and leaves the main agent idle. The next message is asked about afresh.
- **Not enforced, and this is the same as the long path:** what the one-shot planner does with the CLI's *own* file and shell tools. Its prompt says not to build; nothing stops it. That needs M6's sandbox.

## File boundary

`Workspace::resolve` (used by `read`, `list`, `write`, `edit`) used to check the path by name only, so a symlink inside the project pointing outside it was followed: `read` returned the target's text and `write` created files beside it. It now walks the path, and any component that exists and is a symlink must resolve inside the workspace (the workspace's own real path is used, so a workspace reached through a link works). A dangling link is refused as well, because writing through it would create a file at its target. Links that stay inside the workspace still work. `list` shows a symlink by name and never its target's contents. This applies to every agent, not only research.

Bounds: the check is made when the tool is called (a link planted between the check and the open is not caught); `bash` is not sandboxed until M6 and can still leave the workspace; the tests are Unix-only, and Windows junctions are not tested. The UI's file reads (`files.rs`) already canonicalized and are unchanged; its directory listing still descends into a symlinked folder to show names (it reads no content).

## Test changes you should know about

- No existing test was weakened. `diagram.rs`'s test-module `ProjectState` literal gained `intake: Default::default()` (compile-only).
- **Follow-up, owner-directed replacement:** `a_cli_research_task_cannot_plan_and_its_answer_is_filed_as_the_report` (intake_cli.rs) asserted the CLI research behaviour you then asked to disable. It is deleted, and two tests assert the refusal in its place (classified as research, and answered as research). Its plan-tool refusal for research is still covered on the API path (`research_returns_a_report_and_no_pr`).
- **Follow-up, a race in M3's own tests:** three of them (`research_returns_a_report_and_no_pr`, `one_shot_reaches_a_worker_…`, the CLI one-shot test) assert the path is cleared once the main agent reads Approved. The code set Approved first and cleared the path a step later, so they failed intermittently (they passed in my three M3 runs by luck). The assertions were right; I fixed the order in `main_loop`, not the tests. 40 consecutive runs each of the two files, no failure.
- **Follow-up, fixture:** the shared fake `claude` now tells planner from worker by directory and keeps its notes out of worker worktrees; written in the first version, they were committed by the worker and collided on merge.
- In `tests/intake.rs`, after the red commit: `intake_is_off_by_default_and_must_be_allowed_to_ask` appended a second `[intake]` table to `default-config.toml`, which now has one, so it failed on a duplicate table instead of on the check. I rebuilt its fixture from the test config and kept every assertion, plus one (a project file with no `[intake]` is off). Error text is read with `{:#}`.
- `stopping_a_restricted_task_releases_its_path` was written after the implementation. I showed it fails (40s timeout) with the stop-branch cleanup removed, and passes with it restored.
- Two tests pass in the red state on purpose (long is unrestricted, intake off changes nothing): they guard behaviour that must not move.

## Known gaps

- Everything ran against mock models and a fake `claude`. The research and one-shot prompts (`prompts/research.md`, `prompts/oneshot.md`) have not met a real model.
- No claim about classification quality: the §10 eval is still owed (needs your task texts and your OK on the frontier definition).
- `start_task` (a task you start in parallel from the shell) bypasses intake; it already names a ticket.
- The symlink check is per call, Unix-tested only (see "File boundary").
- A decider that fails is treated as "unsure" and asked about; it does not guess.
