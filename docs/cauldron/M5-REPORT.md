# M5 report: Builder and Rampart

Branch `claude/project-thread-4kv2kv` (from `claude/project-thread-gg3ogp` with the §10 eval branch `xkq6x0` merged in). Red `f2333ca0e24d051dbf1b06adf13f9d5000c55126`, green `b8fb7a2f2d15b06539173b92d0348884a1ff5b8d`. Outputs: `M5-red-output.txt`, `M5-scope-red-output.txt`, `M5-suite-output.txt` (run 3 of 3). Details: `M5-NOTES.md`.

1. **Suite: 809 passed, 0 failed, in three runs.** That is 779 before M5 plus 30 new. Warnings: 1, the same `usage.rs` one as before.
2. **Red:** 27 of the 28 new tests failed. One passed as a guard (the shipped config). Two scoping tests were added during green; the end-to-end one was shown red with scoping turned off.
3. **Two separate orchestrators.** On the long path, the lead's first dispatch starts Rampart and waits for it.
   - Rampart's own agent gets the approved spec.md and the baseline, and nothing of the lead's.
   - In code, it can only read its own checkout of the baseline and submit tests.
   - It starts below the frontier.
4. **Tests checked, then sealed:**
   - Every acceptance criterion needs a test.
   - Each test runs 3 times on the baseline and must fail every time. A test that passes, or flips between runs, is sent back.
   - Your yes is needed for the first 3 suites.
   - Sealed means a SHA-256 per file, committed on the lead's branch.
5. **Hash-locked:**
   - Any write to a sealed test is refused.
   - A delivery whose sealed files changed is refused, naming them.
   - Workers run the tests for the criteria their ticket covers, 3 times each.
6. **Mutation score:** after a pass, the changed lines are mutated (up to 12 mutants) and the share the sealed tests catch is recorded. It is not a gate yet: gates are M7/M8.
7. **§7.4 limits:** the same failure twice, 3 failures, the cheap budget, or a frontier call expected to be cheaper triggers ONE frontier call with a failure digest. If that fails too, the ticket goes to you with the digest. There are no frontier loops.
8. **Limits:**
   - It only runs on the long path, and intake is still off.
   - Workers' bash is unsandboxed until M6, so it can still read `.backspace/`.
   - Rampart runs no commands itself.
   - Nothing runs the whole suite across tickets until Gate A (M7).
   - CLI workers go straight to you on a trigger.
   - The §7.3 dispute path is not built.
9. **Open for you:**
   - Must Rampart use a different model from Builder? Today it is only a separate agent.
   - Should Rampart get a sandboxed shell in M6?
   - Should the mutation score have a threshold?
10. Mock models only. Tokens: not measured. Real frontier calls: 0. PLAN stops here for your review.
