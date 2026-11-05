# M3 report: intake, research path, one-shot path

Branch `claude/project-thread-dpmr8e`. Red `4698546`, green `0bceea6953d088a8374730d0411c6c3e7692b34b`. Outputs: `M3-red-output.txt`, `M3-suite-output.txt` (run 3 of 3). Details and test changes: `M3-NOTES.md`.

1. **Intake is off by default (`[intake] enabled = false`). Your call:** the classifier is the uncalibrated M2 baseline, and with intake on the existing e2e task texts would be asked about, not run. I recommend turning it on after the §10 eval.
2. Suite: **540 passed, 0 failed, 0 warnings, in three consecutive runs** (512 at M1 + 28 new). Red: 21 of 23 new acceptance tests failed on missing behaviour; 2 pass by design as guards.
3. **Ask:** an unsure (or unclassifiable) task gets the question in code: no model call, no spend, no CLI launch (tested). A label or 1/2/3 starts that path; two unclear replies and it goes long, keeping what you said. Survives a restart.
4. **Research returns a report with no PR:** `read`, `list`, `submit_report` only, enforced on every call. The report is a file under `.backspace/reports/`. Tested: no ticket, approval, worker or branch, and `create_tickets`, `write` and `bash` are refused.
5. **One-shot reaches execution:** exactly one ticket, 1–5 acceptance lines (5 is provisional), no plan gate, handed off as `.backspace/handoff/<key>/spec.md`, then a worker builds and merges it as usual. Main cannot write or run commands itself.
6. **Research cannot grep, run code or browse.** `bash` could push, so it is off until M6's sandbox, and the harness has no web tool. I added a read-only `list`.
7. **Coding-CLI planner:** intake and the plan-tool rules apply, and a CLI research answer is filed as the report. **Not enforced:** the CLI's own file and shell tools. A CLI one-shot's path ends only on Stop; a worker dispatch from it is untested.
8. Mock models and a fake `claude` only; the two new prompts have not met a real model.
9. Tokens: not measured. Frontier calls: 0 real. Intake's own calls: 0.
10. Still owed from M2: the §10 eval needs your task texts and your OK on the frontier definition. Not started: M4, M5.
