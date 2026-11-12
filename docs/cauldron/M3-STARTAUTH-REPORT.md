# M3 start_task authorization evidence

Branch `claude/project-thread-dpmr8e`. Red `2dc435cb09065a237f9c1d45b257f93f131cf515`, green `5aa0c44b3f671890c33999215d36c64218e06bb4`. Outputs: `M3-startauth-red-output.txt`, `M3-startauth-suite-output.txt` (run 3 of 3). Details: `M3-NOTES.md` ("start_task"). **M3 is still not approved; that is your call.**

1. Suite: **591 passed, 0 failed, 0 warnings, three consecutive runs** (586 before, +5). One of the five failed once in my first set of three runs: a wrong wait in the test, not the code. Fixed, 112 runs of it with 0 failures, whole suite rerun.
2. **Project access:** a token is good only on its own project's server (A's token on B is 401, nothing happens in B). With A's token, a body naming B's folder, a key, a state or an agent is ignored: the key is the harness's, the ticket is A's, and B's tickets, agents, worktrees and files are unchanged. `start_task` has no field that can name another project.
3. **Execution permission:** it only ever creates a new ticket. Existing tickets (proposed, needs-info, wontfix, done, in review) keep their state and get no worker, and no plan is approved. Its worker runs under the project's own limits: a policy that does not list its model, or a task budget smaller than its first call, refuses it before the provider is called; the agent cap holds (the second task is created, reported, left ready).
4. **One gap found and fixed:** with the project folder deleted, `start_task` still created a ticket. It now refuses ("the project folder is not accessible") and does not recreate the folder. This was the only red test; the other four pass on the existing code as guards (I broke two on purpose and they failed).
5. **Not covered:** a folder that exists but is unreadable (I run as root, where permissions do not bite). A started task's worker counts as an orchestration agent in the policy, so `orchestration_models` is the list that governs it.
6. Tokens: not measured. Frontier calls: 0 real.
