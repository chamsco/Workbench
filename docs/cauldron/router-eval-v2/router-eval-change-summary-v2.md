# Router eval v1 → v2: change summary

v1 files are untouched (`docs/cauldron/router-eval-v1/`, primary SHA-256 `80c47060…ef9d2`). v2 lives in `docs/cauldron/router-eval-v2/`. Label status everywhere: provisional, pending owner review, `owner_approved = false`.

## Group counts

| group | v1 | v2 |
|---|---|---|
| primary candidate | 31 (9 one_shot, 18 long, 4 research) | 31 (8 one_shot, 17 long, 6 research) |
| owner-review pool | 13 | 17 (8 context-dependent, 2 ambiguous, 6 low-confidence, 1 control-disputed) |
| negative controls | 10 | 6 |
| stress | 1 | 1 |

## Every group or label change

| id | v1 | v2 | why |
|---|---|---|---|
| `3vfSNZZAcn` | control | primary, one_shot (med) | requests PLAN.md §5 edit, audit record, flag removal: a task |
| `d4MEWNBX8V` | control | primary, research (med) | requests a §§3–12 → §5 mapping report |
| `VTsRosZxiV` | control (was research) | primary, research (med) | a question asking for an answer is a research deliverable |
| `bf7eL9rznC` | primary, one_shot | pool, context_dependent | "this one" has no recorded referent |
| `CK3sKxD5Yq` | primary, one_shot | pool, low_confidence | "this version" points at an unrecorded attachment; rule + build mixed |
| `mr7DwLWcwE` | primary, long | pool, ambiguous | feature request and feasibility question, no dominant path |
| `BUau1Gzo6o` | control | pool, control_disputed (proposed one_shot) | "make a version on the mac" requests work, and needs prior context |
| `MHgLPqGHzr` | pool (listed as context-dependent in the v1 review) | pool, ambiguous | the v1 review document and JSONL disagreed; ambiguity is not missing context |

All other rows keep group and label. Controls after the change (6): `fBvHnrzpqd`, `X5c2rXL5Pf`, `d6YKMH8sFa`, `NEDNECoMJD`, `a3TMGNAEHP`, `7ktDWmLsxc`.

Flag changes on all rows: `truncated-8kb` removed from `flags`; `historically_truncated_in_draft: true` added on `wYCfpHgci1`, `BjxMTw3GNE`, `ijp71Z5AZt`, `xhMcczbtWd`. `context-dependent`, `ambiguous`, `low-confidence`, `control-disputed` flags now equal the pool reason, and none appears on a primary row. New fields on every row: `pool_reason`, `v1_group`, `v1_label`, `label_changed_from_v1`, `group_changed_from_v1`, `v2_movement_reason`, `owner_approved`, `proposed_label`.

## Metric changes (keyword, threshold 0.7, primary)

| | v1 | v2 | cause |
|---|---|---|---|
| overall | 25/31 (80.6%) | 24/31 (77.4%) | 3 removed (1 correct: `bf7eL9rznC`), 3 added (0 correct) |
| one_shot | 8/9 | 7/8 | `bf7eL9rznC` and `CK3sKxD5Yq` out, `3vfSNZZAcn` in (wrong) |
| long | 17/18 | 17/17 | `mr7DwLWcwE` (wrong) out |
| research | 0/4 | 0/6 | `VTsRosZxiV`, `d4MEWNBX8V` in (wrong) |
| confident | 4 (4 correct) | 5 (4 correct, 1 wrong) | `3vfSNZZAcn` is confident (0.84) and labelled one_shot |
| ECE / Brier | 0.268 / 0.424 | 0.260 / 0.461 | |
| without length rule | 20/31 | 19/31 | |
| majority baseline | 18/31 (58.1%) | 17/31 (54.8%) | class mix changed |
| controls confident | 1 of 10 | 0 of 6 | the one confident control was a task |

Corrected arithmetic from v1: no-length 20 vs always-long 18 was a difference of two, not three. v2: 19 vs 17, again two.

No label was chosen with reference to the router's output. `3vfSNZZAcn` was labelled from its text before its score was read; its label is the one that makes the router wrong, and the report says so.

## Code and tests (this change)

- `eval::extended` (confusion matrix, confident/asked split, per-trace) added with tests; the example uses it. Red `1788d6b`, green `af31eb3`; v1 numbers reproduce exactly through the refactored example (25/31, 4 confident, ECE 0.268).
- `crates/core/tests/router_eval_v2.rs` (10 tests): row hashes and char counts, no cross-group overlap and group counts, reclassified controls are primary tasks, exactly eight context-dependent plus `MHgLPqGHzr` ambiguous with flags matching reasons, restored messages complete and truncation historical-only, nothing owner-approved, primary/stress parse as traces and controls have no label, extended-metric values on a stub model, agreement with `evaluate`, actual majority class.
- Commands run: example (shipped and `--no-length`) on primary, primary+stress, pool, stress, controls and at five thresholds; `backspace-cli eval` on v2 primary (same 24/31, per-class, calibration, ECE, Brier); `cargo test --workspace --exclude backspace --exclude backspace-tauri` five times (see below). `backspace` (GUI) and `backspace-tauri` do not link in this container (missing libxkbcommon).
- Suite (5 runs, identical tree): runs 2, 3 and 4 green (739 pass, 0 fail, 2 ignored); runs 1 and 5 each had one failure (738 pass, 1 fail). Both failures are in the existing intake tests (`stop_releases_a_failed_one_shot_even_though_the_main_agent_is_idle` in run 1; `a_one_shot_worker_the_user_stops_fails_its_ticket_and_can_be_dispatched_again` in run 5). `intake.rs` is unchanged by this work; the test failed 1 of 25 isolated reruns here, and the M5 branch carries a race fix for it (`1396db0`). So there are **not** three consecutive green runs on this branch; the flake predates this change.
