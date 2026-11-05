# M3 follow-up report: symlink boundary, CLI research, CLI one-shot

Branch `claude/project-thread-dpmr8e`. Red `824c65be1fbded4d717fd33db60a07c4e73277b1`, green `5b1d50103f4bf31f0d1ed52d18c33fd06ec6357b`. Outputs: `M3-followup-red-output.txt`, `M3-followup-suite-output.txt` (run 3 of 3). Details: `M3-NOTES.md`.

1. Suite: **553 passed, 0 failed, 0 warnings, three consecutive runs** (540 before, +13 new, 1 replaced). Red first: 6 of 9 boundary tests, the research symlink test and 4 CLI tests failed on missing behaviour.
2. **Symlink boundary fixed.** `read`, `list`, `write` and `edit` now refuse any path through a symlink that leaves the workspace, a dangling link and a chain included. Links that stay inside still work. Applies to every agent. Tested directly and end to end: a research agent cannot read a linked secret, and it never reaches the model, the report or the log.
3. **CLI research is disabled.** It is refused before the CLI runs: an error that says why, no launch, no spend, nothing half-started. Same when you answer "research" to the question. The old CLI-research test is deleted and replaced by two refusal tests, per your instruction.
4. **CLI one-shot boundary tested end to end** (fake `claude`): two tickets refused, one filed with no plan gate, a second refused, dispatched to a worker in its own worktree, you approve, it merges. **Fixed a gap I had reported:** the path now ends when the ticket settles (done is a delivery; failed releases it and stays idle). Both outcomes tested.
5. **A race in my own M3 tests:** the path was cleared one step after the main agent read Approved, so three tests failed intermittently (they passed my three M3 runs by luck). I fixed the order in code, not the tests: 40 runs of each file, 0 failures.
6. **Not changed:** the CLI's own file and shell tools stay ungated on a one-shot (as on the long path; needs M6). The symlink check is per call, Unix-tested only, and `bash` is still unsandboxed. The UI file listing still shows names under a symlinked folder (no content).
7. Tokens: not measured. Frontier calls: 0 real (mock providers and a fake CLI).
