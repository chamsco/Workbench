# M3 final closure report: CLI one-shot, symlink UI, intake boundaries

Branch `claude/project-thread-dpmr8e`. Red `40ecfec0e2a264254437a3a76c51c705e994b9fa`, green `ad9124b6169229b4384cb9348bb1e375975db6a6`. Outputs: `M3-closure-red-output.txt`, `M3-closure-suite-output.txt` (run 3 of 3). Details: `M3-NOTES.md`. **M3 is not approved; that is your call.**

1. Suite: **565 passed, 0 failed, 0 warnings, three consecutive runs** (553 before: +9 listing, +1 API one-shot, +2 net in `intake_cli` after removing 3 and adding 5). Red first: 7 listing and 3 CLI tests failed on missing behaviour.
2. **CLI one-shot is refused until M6**, before the CLI launches, whether the classifier chose it, you answered it, or a saved session carried it. No launch, no model call, no ticket, spec or report; the path, question and ticket key are cleared; the error says why and offers the API planner or a long task; it is not turned into a long task. API one-shot and API research are unchanged. A CLI long task launches as before.
3. **UI listing fixed** (`files::scan`): a symlink is listed as a link and an external or dangling one is never entered, so no outside names appear, however deep or chained. Internal links keep their behaviour; a loop into an ancestor is not walked. `FileEntry.link` is new (additive).
4. **Test changes, disclosed:** three CLI one-shot tests deleted on your instruction (lifecycle coverage stays on the API path, plus a new API second-ticket test); one listing test of mine over-specified and became two. The new API test passes in the red state on purpose (guard).
5. **Not covered:** Windows junctions untested (no Windows runner); symlink check is per call; `bash` unsandboxed until M6; prompts have only met mock models.
6. **Open, found not fixed:** with a CLI planner and intake on, the question still offers one-shot and research, then refuses them; an API one-shot whose ticket fails is not tested for path release (Stop works).
7. Tokens: not measured. Frontier calls: 0 real (mock providers and fake CLIs).
