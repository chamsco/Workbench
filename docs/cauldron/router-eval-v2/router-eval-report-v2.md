# Router evaluation report v2 (§10 diagnostic)

**Historical-prompt replay evaluation — provisional pre-runtime baseline.** Not runtime traces, not the §10 shipping evaluation. Intake stays off; no model or provider was called. Everything here is a *measured fact* (counts produced by the code on the frozen files), a *provisional label* (mine, pending Charles) or an *inference* (marked as such). v1 is preserved unchanged and superseded by this report for the corrected sets.

- Code HEAD: `af31eb33d59613acad831eb9e90dc939224f416a` (branch `claude/project-thread-xkq6x0`; the commit that adds these artefacts follows it).
- Corpus manifest: `581fbbb8c3a97b341185fe3f87d69fde72e61ee15e0977226099e587cab134f6`; per-file hashes in `router-eval-config-v2.md`.
- Cost: **Not computed — insufficient measured token and execution data.**

## 1. Groups (measured)

| group | file | v1 | v2 |
|---|---|---|---|
| primary benchmark candidate | `router-eval-v2.jsonl` | 31 | 31 (8 one_shot, 17 long, 6 research) |
| owner-review pool | `router-eval-v2-owner-review-pool.jsonl` | 13 | 17 (8 context-dependent, 2 ambiguous, 6 low-confidence, 1 control-disputed) |
| negative controls | `router-eval-v2-excluded.jsonl` | 10 | 6 |
| supplemental stress | `router-eval-v2-stress.jsonl` | 1 | 1 (same message and text hash as v1; the file differs only by the added v2 metadata fields) |

Majority-class baseline on the corrected primary set: always `long` = 17/31 = 54.8%. Small sample (31 items from one author): one item moves a class by 4–17 points. Treat all numbers as indicative.

## 2. Primary candidate, threshold 0.7, shipped keyword baseline (length rule on)

| metric | value |
|---|---|
| items | 31 |
| overall top-class accuracy (includes items the system would ask about) | 24/31 = 77.4% |
| one_shot accuracy | 7/8 = 87.5% |
| long accuracy | 17/17 = 100.0% |
| research accuracy | 0/6 = 0.0% |
| confident decisions (p ≥ 0.7) | 5 |
| coverage | 5/31 = 16.1% |
| selective accuracy (confident only, denominator 5) | 4/5 = 80.0% |
| abstentions (asked) | 26 |
| wrong predictions caught by asking | 6 |
| correct predictions escalated unnecessarily (asked but right) | 20 |
| confident-wrong | 1 (of 5 confident) |
| ECE | 0.260 |
| Brier | 0.461 |

Confusion matrix (rows = provisional label, columns = predicted):

| true \ predicted | one_shot | long | research | n |
|---|---|---|---|---|
| **one_shot** | 7 | 1 | 0 | 8 |
| **long** | 0 | 17 | 0 | 17 |
| **research** | 2 | 4 | 0 | 6 |

Calibration:

| bin | n | mean confidence | accuracy |
|---|---|---|---|
| 0.3–0.4 | 8 | 0.333 | 0.750 |
| 0.4–0.5 | 1 | 0.429 | 1.000 |
| 0.5–0.6 | 3 | 0.528 | 0.667 |
| 0.6–0.7 | 14 | 0.647 | 0.786 |
| 0.7–0.8 | 4 | 0.763 | 1.000 |
| 0.8–0.9 | 1 | 0.840 | 0.000 |

Confident decisions (5): F4LqFDKhEV, 3vfSNZZAcn, NsYAnzNThU, aH7hF8sRkG, ZyixXbQTvw. Four are correct; **`3vfSNZZAcn` is confident-wrong** (labelled one_shot, predicted long at 0.84).  Facts about that item: its label is provisional (medium confidence; a reader could call it long, in which case it would be correct and confident-wrong would be 0); it is a 2,044-character message, so the length rule is why it is confident (without the rule it is 0.73, still above 0.7, from build/system/integrat keywords). Coverage is 16.1%: one wrong confident answer out of five is no evidence about the confident-error rate in either direction.

With the `/goal` stress case added (32 items): 25/32 = 78.1%, confident 5 (4 correct), asked 27, ECE 0.263, Brier 0.454. The stress case is classified long (correct) with the length rule and correct without it. It does not change any conclusion.

## 3. Length-rule ablation (separate diagnostic)

`keyword_scores` adds 2.0 to `long` above 400 characters and 3.0 more above 1,500. `KeywordDeciderNoLength` removes only that; same keyword lists, same threshold, nothing tuned on this corpus. Reproduce with `--no-length`.

Length versus label in this corpus (measured):

| label | n | over 400 chars | over 1,500 |
|---|---|---|---|
| one_shot | 8 | 1 | 1 |
| long | 17 | 16 | 5 |
| research | 6 | 4 | 2 |

Inference (not a measurement): length tracks label here because of how the messages happen to be written (short follow-ups, long briefs). It is not shown to measure task size and need not hold at runtime.

| metric (primary, 0.7) | length rule on | length rule off |
|---|---|---|
| overall top-class accuracy | 24/31 = 77.4% | 19/31 = 61.3% |
| one_shot | 7/8 | 7/8 |
| long | 17/17 | 11/17 |
| research | 0/6 | 1/6 |
| confident decisions | 5 | 2 |
| selective accuracy (denominator = confident) | 4/5 | 1/2 |
| wrong caught by asking | 6 | 11 |
| asked but right | 20 | 18 |
| confident-wrong | 1 | 1 |
| ECE | 0.260 | 0.193 |
| Brier | 0.461 | 0.554 |

Without the rule, confusion matrix:

| true \ predicted | one_shot | long | research | n |
|---|---|---|---|---|
| **one_shot** | 7 | 1 | 0 | 8 |
| **long** | 4 | 11 | 2 | 17 |
| **research** | 2 | 3 | 1 | 6 |

Calibration without the rule:

| bin | n | mean confidence | accuracy |
|---|---|---|---|
| 0.3–0.4 | 12 | 0.333 | 0.500 |
| 0.4–0.5 | 1 | 0.462 | 0.000 |
| 0.5–0.6 | 13 | 0.544 | 0.769 |
| 0.6–0.7 | 3 | 0.622 | 0.667 |
| 0.7–0.8 | 2 | 0.733 | 0.500 |

Items whose predicted class changes when the rule is removed: XvJZK8Koo9 (long, 898 chars: long→research); QeQzLM8MFd (long, 677 chars: long→research); 9VgNem1Ftm (long, 761 chars: long→one_shot); a4NXvQCcJx (long, 626 chars: long→one_shot); dNEK4oF7yV (research, 1482 chars: long→research); pEjbaBnj1M (long, 1177 chars: long→one_shot); K1mHACVkPQ (long, 590 chars: long→one_shot).

Arithmetic (rechecked): the rule is worth 24-19 = 5 correct answers. Without it the baseline gets 19/31 = 61.3%; always-`long` gets 17/31 = 54.8%; the difference is 2 items. (The v1 report said "three"; v1's figures were 20/31 vs 18/31, a difference of two. Corrected here.) With the rule the baseline is 7 items above majority. ECE is lower without the rule only because the baseline rarely exceeds 0.5 confidence; Brier is worse, so I do not read it as better calibration.

## 4. Threshold sensitivity (exploratory, not pre-registered)

Primary set only. Only 0.7 was fixed in advance; do not read a "best" threshold from this table.

| threshold | accuracy (all, unchanged) | confident, rule on | correct | confident, rule off | correct |
|---|---|---|---|---|---|
| 0.5 | 24/31 | 22 | 17 | 18 | 13 |
| 0.6 | 24/31 | 19 | 15 | 5 | 3 |
| 0.7 | 24/31 | 5 | 4 | 2 | 1 |
| 0.8 | 24/31 | 1 | 0 | 0 | 0 |
| 0.9 | 24/31 | 0 | 0 | 0 | 0 |

## 5. Negative controls (reported separately; not in any accuracy denominator)

Six messages that genuinely are not new tasks. The expected behaviour is to ask, not act. The previous control set's one confident item, `3vfSNZZAcn`, is no longer a control: it authorises and requests work (see label review), so v1's "one of ten non-tasks gets a confident answer" is withdrawn.

| id | chars | why it is not a task | length rule on | length rule off |
|---|---|---|---|---|
| `fBvHnrzpqd` | 5 | approval ("DO IT") | one_shot 0.33 ask | one_shot 0.33 ask |
| `X5c2rXL5Pf` | 8 | bare continuation | one_shot 0.33 ask | one_shot 0.33 ask |
| `d6YKMH8sFa` | 13 | bare continuation | one_shot 0.33 ask | one_shot 0.33 ask |
| `NEDNECoMJD` | 6217 | standing rule (how to work), no deliverable | long 0.46 ask | research 0.53 ask |
| `a3TMGNAEHP` | 29 | acknowledgement | one_shot 0.33 ask | one_shot 0.33 ask |
| `7ktDWmLsxc` | 610 | approval / sign-off statement | long 0.60 ask | one_shot 0.33 ask |

Result (measured): 0 of 6 controls are confident with the length rule, 0 of 6 without it. The remaining four controls are 5–29 characters with no keywords, so they sit at the uniform floor (0.333) and are asked about. That is not evidence the baseline recognises non-tasks: it asks because it has no evidence, and any longer non-task with task-like keywords gets a confident answer (the v1 counter-example, `3vfSNZZAcn`, scored 0.84 as a task prediction; as a *task* it is now confident-wrong, section 2). Limits: six items; four are trivial; two of the six (`NEDNECoMJD`, a working-method prompt, and `7ktDWmLsxc`, an approval that states next steps) are the only non-trivial ones and rest on my reading that they request nothing. One control moved out: `BUau1Gzo6o` ("make a version on the mac and continue where we left off") requests work and needs prior context, so it sits in the owner-review pool as `control_disputed`.

Smallest defensible next step (owner decision): build a larger control set that includes *near-miss* messages (long answers, sign-offs with next steps, standing rules) from real traces, and test a minimal not-a-task guard against it. No guard was built or tuned here.

## 6. Owner-review pool (outside the primary denominator)

17 items; 16 have a proposed label. Reasons are kept apart: context-dependent, ambiguous, low-confidence, control-disputed. Reference only.

| metric | value |
|---|---|
| items | 16 |
| overall top-class accuracy (includes items the system would ask about) | 7/16 = 43.8% |
| one_shot accuracy | 4/9 = 44.4% |
| long accuracy | 3/5 = 60.0% |
| research accuracy | 0/2 = 0.0% |
| confident decisions (p ≥ 0.7) | 0 |
| coverage | 0/16 = 0.0% |
| selective accuracy (confident only, denominator 0) | 0/0 = n/a |
| abstentions (asked) | 16 |
| wrong predictions caught by asking | 9 |
| correct predictions escalated unnecessarily (asked but right) | 7 |
| confident-wrong | 0 (of 0 confident) |
| ECE | 0.246 |
| Brier | 0.700 |

Without the length rule: 6/16 correct, 1 confident. These items are unresolved by construction; their accuracy says nothing about the router.

## 7. What this establishes, and what it does not

Measured: the corrected sets, the numbers above, and that the length rule accounts for 5 of 24 correct answers.
Provisional: every label, and the primary/pool split. None is approved.
Inference: the baseline is weak on research (0/6 correct), its `long` accuracy is largely the length rule, and its abstention (83.9%) is mostly lack of evidence rather than calibrated doubt.
Not established: runtime behaviour; cost or savings (**Not computed — insufficient measured token and execution data**; nothing was estimated, no provider price was used, no escalation rate was measured); representativeness (single author, one product, UI-heavy, pre-M-series); whether the tier policy is right (no provider execution happened).

Reproduce: `cargo run -p backspace-core --example router_eval_report -- docs/cauldron/router-eval-v2/router-eval-v2.jsonl --threshold 0.7 [--no-length]`; the CLI (`backspace-cli eval <file> --threshold 0.7`) printed the same accuracy, per-class, calibration, ECE and Brier numbers (checked: 24/31, 7/8, 17/17, 0/6, ECE 0.260, Brier 0.461).
