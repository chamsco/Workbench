# M3 follow-up 3: CLI question, one-shot lifecycle, link UI, start_task

> Item 5 is extended by `M3-STARTAUTH-REPORT.md` (project access and execution permission, requested before sign-off).

Branch `claude/project-thread-dpmr8e`. Red `063debee6710f418401a4b7083a6624e1c48c55b`, green `6c95634239678bf22198b1c7caaaa94bd6fb597f`. Outputs: `M3-followup3-red-output.txt`, `M3-followup3-suite-output.txt` (run 3 of 3), `M3-followup3-ui-red/green-output.txt`. Details: `M3-NOTES.md`. **M3 is still not approved; that is your call.**

1. Suite: **586 passed, 0 failed, 0 warnings, three consecutive runs** (565 before, +21). Red first: 12 of the 21 new tests failed and the UI eval failed on every link row; 9 pass in red as guards (two broken on purpose to see them fail).
2. **Question:** under a CLI planner it offers only `long` and names one-shot and research as unavailable, with why. API planner unchanged. An answer naming one anyway is still refused.
3. **Lifecycle tests found a real trap, now fixed:** Stop did nothing when the main agent was idle, so a one-shot whose worker failed kept its path across restarts. Stop now closes it. `work_tickets` with no keys retries the failed ticket (it said "no tickets ready"). A dropped ticket (gone or wontfix) frees the one-ticket slot. Tested: failure, user-stopped worker, rejection, retry, Stop, dropped.
4. **Links in the UI:** italic name, arrow marker, "Symbolic link" tooltip and accessible name, in the right panel and the Worktree canvas. Before/after at 2x: `screenshots/m3-links/`.
5. **start_task, my reading of "authorization boundary":** needs the token (401 otherwise, nothing created); skips plan approval and the question, not your review; no agent can call it. **Behaviour change for you to confirm:** it is now refused while a research or one-shot task is open (a one-shot used to absorb the ticket).
6. **Not done:** a failed one-shot can be retried or stopped, not re-specified; the GPUI shell does not draw links and the UI cannot tell internal from external file links; Windows junctions untested; `bash` unsandboxed until M6; prompts only met mock models.
7. Tokens: not measured. Frontier calls: 0 real (mock providers, fake CLIs).
