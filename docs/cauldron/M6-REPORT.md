# M6 report: execution engine and herdr adapter

Branch `claude/project-thread-4kv2kv`, starting from `claude/project-thread-gg3ogp` at `12d9c47`. Red commit: `0be05b6`. Outputs: `M6-red-output.txt` and `M6-suite-output.txt` (run 3 of 3). Details are in `M6-NOTES.md`.

1. **Suite: 840 passed, 0 failed, in three runs** (814 + 25 new + 1 unit test; bubblewrap on, sandbox tests required).
2. **Red:** 25 of the 27 new tests failed against stubs. The other 2 were config guards and passed. The full suite at red was 814 passed, 25 failed.
3. **Sandboxed worktrees.**
   - Each agent's commands run in a sandbox: its bash, its checks, Rampart's vetting, and the sealed-test and mutation runs.
   - The backend is bubblewrap on Linux, Seatbelt on macOS, and a low-integrity process with mandatory labels on Windows.
   - Inside it, an agent writes only to its worktree, git's store and a private /tmp.
   - Backspace's folder, its data, and your SSH, cloud and CLI credentials are hidden.
   - The policy, git's hooks and config, and sealed tests are read-only.
4. **Executors respect allowlists.**
   - No API key or CLI login reaches an agent's shell, so it cannot call a model the gateway did not approve.
   - A coding-CLI worker runs in the same sandbox. It keeps only its own login, for the model it was launched with.
5. **Modes.**
   - `auto` (default): uses the sandbox where there is one, otherwise scrubs the environment and says so in the log.
   - `required`: refuses the command when no sandbox can isolate it.
   - `off`: behaves as before M6.
6. **herdr.**
   - It sits behind a `Supervisor` interface, with a native supervisor beside it.
   - It is pinned to 0.9.5. Any other version, or no answer from its socket, falls back to native.
   - Its agent states are signals only. Nothing gates on them.
   - It is off by default.
7. **M5 decisions built.**
   - Rampart prefers a model other than the lead's.
   - Rampart gets a shell inside the sandbox, with no network and its checkout read-only.
8. **CI.** The Linux job now installs bubblewrap and fails, rather than skips, the sandbox tests if bubblewrap can't run. New macOS and Windows jobs run the sandbox tests with the sandbox required.
9. **Limits.**
   - **Windows is weaker:** the network stays on, agents cannot commit there (Backspace commits for them), Rampart gets no shell, and the labels stay on disk.
   - The home folder stays writable.
   - The network is on by default.
   - herdr was tested against a stand-in, because its download is blocked here.
   - Nine older test files with fake CLIs run with the sandbox off. So do the policy-guard tests. Both kinds write where the sandbox now forbids.
10. Mock models only. Tokens: not measured. Real frontier calls: 0.
