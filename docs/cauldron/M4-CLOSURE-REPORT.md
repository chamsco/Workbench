# M4 closure report

Branch `claude/project-thread-nxzxke`, PR #45. Red `e708ac76dc963db206967c93324c99c737f04ece`, green `7595cae454869d0239db2d95972c9fb10de697c5`. Outputs: `M4-closure-red-output.txt`, `M4-closure-suite-output.txt` (run 3 of 3). Details: `M4-NOTES.md` ("Closure"), `PLANNING-CONTRACT.md`.

1. Suite: **718 passed, 0 failed, three consecutive runs** (709 + 1 before, +8 new). The one remaining warning is the pre-existing `usage.rs` one.
2. **Baseline commands wait for approval.** Nothing runs until you say yes to the exact command list. A changed command needs a new yes, and anything other than a yes runs nothing.
   - Tested: marker files stay absent before the yes, appear after it, and stay absent for a changed list.
   - Five existing harness tests now include this approval step. I only inserted that step: no assertion was removed or weakened.
3. **`hands/decide.rs` is in the audit.**
   - It is listed in `policy_matrix` with a row in `M1-BUDGET-BOUNDARIES.md`.
   - Inside a project, `browser_task` asks the project's model policy via the bridge before any browser or model call. A refusal runs nothing.
   - Its spend is still unmetered.
   - It is not covered outside a project (no bridge there).
   - A worker that may create tickets is checked at the execution role.
4. **Canonical spec for M5:** `.backspace/planning/spec.md`, valid only when its hash equals `approved_sha256`. Plan mode's root `SPEC.md` is not an M5 input.
5. **Intake stays off by default** until the §10 eval, as stated in the contract and tested.
6. **Not started:** M5. The §10 eval needs your task texts (or traces) to label.
7. Mock models and fake CLIs only. Tokens: not measured. Frontier calls: 0 real.
