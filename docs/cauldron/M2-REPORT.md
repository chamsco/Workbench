# M2 report: decision interface, routing policy, cost ladder, eval, metrics

Branch `claude/project-thread-dpmr8e`. Red tests `ead7ae0`; green commit follows. Output: `M2-red-output.txt` (300 tests, 59 red), `M2-suite-output.txt` (final).

1. **M2 acceptance is not fully met: the §10 eval report on real traces is missing.** The harness and `backspace-cli eval <traces.jsonl>` work and their arithmetic is tested, but no labeled Cauldron traces exist. I did not label my own or mock a report. Needs your decision.
2. Suite, final run: **302 passed, 0 failed**, 0 warnings (237 at M1, +65 new). An earlier full run failed `a_webhook_wakes_only_its_bot`: a flake that predates M2 (7 failures in 60 runs at the M1 commit, 10 in 60 now; two tests in that file race on the process-wide `BACKSPACE_DATA`). I did not touch it; both outputs attached.
3. Decision interface (`decision.rs`): typed questions in, one probability distribution per question out, checked before use (wrong options, NaN, sums not 1 are rejected). Swappable: `Router::with_decider`. Under the threshold the result is `Ask`, not a guess.
4. Baseline decider is keyword rules: no network, uncalibrated, unvalidated. Threshold 0.7 is provisional. No decision model is chosen.
5. **Frontier definition, proposed:** each model has a `class` (local, mid, frontier); unset, it follows the tier (0 local, 1–2 mid, 3–4 frontier). So opus and fable are frontier, and so are the coding CLIs even at $0. Needs your OK; M8 will rely on it.
6. Planner start: only a confident "long" task starts on a frontier model. One-shot, research and unsure tasks start mid. This replaces the forced Xhigh; your `main_min_effort` floor (High) is unchanged. A planner running in a coding CLI skips the router, so it is not covered.
7. Cost ladder (`ladder.rs`): moving up needs a failure with evidence, nothing is above frontier, and `escalate_sooner` compares expected cost to green. It is not wired into worker escalation; that is M8.
8. Metrics: all five §3.4 metrics go to `.backspace/metrics.jsonl` and `Harness::metrics()`. "Per merged" means per merged ticket and "green" means ticket accepted, until M7 and Gate A.
9. Tokens: not measured. Frontier calls: 0 real (mock providers only).
10. Left: the eval run, the CLI-planner path, wiring `escalate_sooner` (M8). Details, test changes and gaps: `M2-NOTES.md`.
