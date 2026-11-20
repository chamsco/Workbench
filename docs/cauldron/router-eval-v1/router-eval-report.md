# Router evaluation report (§10)

**Historical-prompt replay evaluation — provisional pre-runtime baseline.** Not runtime traces. Every item is a message Charles wrote to the project, replayed through the keyword baseline (`KeywordDecider`, decision-model name `keyword`) at confidence threshold 0.7. Labels are provisional and pending owner review; nothing here is ground truth, and this report is not an approval.

- Corpus: `router-eval-v1.jsonl`, frozen. SHA-256 `80c470606f3d3ad90c2cc2cae12a2133d41c2129ee3d6e7516060263801ef9d2`.
- Corpus manifest (all four files, see `router-eval-config.md`): `beb71f3bead5b037997af3184a51197af5e5848a5449ca89d0750b800f234a13`.
- Code: branch `claude/project-thread-xkq6x0`, HEAD at generation `a6145099664fa3ce24341e0548e36eac335b1562` (before the commit that adds these files).
- Cost: **Not computed — insufficient measured token and execution data.** No `input_tokens`/`output_tokens` exist for any item and none were estimated.
- Intake stays disabled by default. No decision model was selected or integrated. No model or provider was called.

## 1. What is scored

| group | file | n | role |
|---|---|---|---|
| primary benchmark | `router-eval-v1.jsonl` | 31 | the 3-class denominator (9 one_shot, 18 long, 4 research) |
| supplemental stress | `router-eval-stress.jsonl` | 1 | the `/goal` brief (26,183 chars), long; reported with and without |
| owner-review pool | `router-eval-owner-review-pool.jsonl` | 13 | 5 low-confidence + 8 context-dependent; **not** in the primary denominator |
| control set | `router-eval-excluded.jsonl` | 10 | non-tasks; separate negative controls |

The primary set is small (31) and skewed to `long`. Always-answer-`long` scores 18/31 = 58.1%; the keyword baseline has to be read against that, not against 33%. With 31 items, one item moves a class by 5–25 points: treat every number below as indicative.

## 2. Primary benchmark, threshold 0.7, keyword baseline (length rule on, as shipped)

| metric | value |
|---|---|
| items | 31 |
| overall accuracy | 25/31 = 80.6% |
| one_shot accuracy | 8/9 = 88.9% |
| long accuracy | 17/18 = 94.4% |
| research accuracy | 0/4 = 0.0% |
| confident decisions (p ≥ 0.7) | 4 |
| coverage | 12.9% |
| selective accuracy (confident only) | 4/4 = 100.0% |
| abstentions (asked) | 27 |
| wrong, caught by asking | 6 |
| asked but would have been right | 21 |
| confident-wrong | 0 |
| ECE | 0.268 |
| Brier | 0.424 |

Confusion matrix (rows = owner-review label, columns = predicted):

| true \ predicted | one_shot | long | research | n |
|---|---|---|---|---|
| **one_shot** | 8 | 1 | 0 | 9 |
| **long** | 1 | 17 | 0 | 18 |
| **research** | 1 | 3 | 0 | 4 |

Calibration:

| confidence bin | n | mean confidence | accuracy |
|---|---|---|---|
| 0.3–0.4 | 9 | 0.333 | 0.778 |
| 0.4–0.5 | 1 | 0.429 | 1.000 |
| 0.5–0.6 | 4 | 0.535 | 0.500 |
| 0.6–0.7 | 13 | 0.643 | 0.846 |
| 0.7–0.8 | 4 | 0.763 | 1.000 |

The four confident decisions (all long, all correct): F4LqFDKhEV, NsYAnzNThU, aH7hF8sRkG, ZyixXbQTvw. All four are over 800 characters, and three of them drop below 0.7 once the length rule is removed (section 4). With 4 confident decisions, "0 confident-wrong" is weak evidence: it is a result on 4 items.

## 3. With the `/goal` stress case added (n = 32)

| metric | value |
|---|---|
| items | 32 |
| overall accuracy | 26/32 = 81.2% |
| one_shot accuracy | 8/9 = 88.9% |
| long accuracy | 18/19 = 94.7% |
| research accuracy | 0/4 = 0.0% |
| confident decisions (p ≥ 0.7) | 4 |
| coverage | 12.5% |
| selective accuracy (confident only) | 4/4 = 100.0% |
| abstentions (asked) | 28 |
| wrong, caught by asking | 6 |
| asked but would have been right | 22 |
| confident-wrong | 0 |
| ECE | 0.271 |
| Brier | 0.418 |

The stress case is classified long at the shipped settings and adds one correct item; it changes no conclusion. With the length rule removed it is still classified long (long 12/19 correct, so the stress case stays right), but its confidence is not the point: it is a 26k-character brief, and the length rule has nothing to say about it that the keywords do not. See section 4.

## 4. The 400-character length bias

`keyword_scores` adds 2.0 to `long` above 400 characters and 3.0 more above 1,500. In this corpus the rule is nearly a label:

| label | n | over 400 chars |
|---|---|---|
| one_shot | 9 | 0 |
| long | 18 | 16 |
| research | 4 | 3 |

Every one_shot is ≤400 characters; 16 of 18 long items exceed 400. That is a property of how the messages were written (short follow-ups versus long briefs), not evidence that length measures task size, and it will not hold in runtime use. So the as-shipped accuracy on `long` mostly measures the length rule.

Ablation: identical baseline with only the length rule removed (`KeywordDeciderNoLength`, evaluation-only; tests prove it equals the shipped baseline at ≤400 characters and differs above). Heuristic and keyword lists untouched, nothing tuned on this corpus.

| metric (primary, 0.7) | length rule on | length rule off |
|---|---|---|
| overall accuracy | 25/31 = 80.6% | 20/31 = 64.5% |
| one_shot | 8/9 | 8/9 |
| long | 17/18 = 94.4% | 11/18 = 61.1% |
| research | 0/4 | 1/4 |
| confident decisions | 4 | 1 |
| selective accuracy | 4/4 | 1/1 |
| wrong caught by asking | 6 | 11 |
| confident-wrong | 0 | 0 |
| ECE | 0.268 | 0.208 |
| Brier | 0.424 | 0.534 |

Confusion matrix, length rule off:

| true \ predicted | one_shot | long | research | n |
|---|---|---|---|---|
| **one_shot** | 8 | 1 | 0 | 9 |
| **long** | 5 | 11 | 2 | 18 |
| **research** | 1 | 2 | 1 | 4 |

Calibration, length rule off:

| confidence bin | n | mean confidence | accuracy |
|---|---|---|---|
| 0.3–0.4 | 13 | 0.333 | 0.538 |
| 0.4–0.5 | 1 | 0.462 | 0.000 |
| 0.5–0.6 | 13 | 0.544 | 0.769 |
| 0.6–0.7 | 3 | 0.622 | 0.667 |
| 0.7–0.8 | 1 | 0.733 | 1.000 |

Seven items change prediction when the length rule is removed (all `long` or the one research prompt that the rule had pulled to long): XvJZK8Koo9 (long, 898 chars: long→research); QeQzLM8MFd (long, 677 chars: long→research); 9VgNem1Ftm (long, 761 chars: long→one_shot); a4NXvQCcJx (long, 626 chars: long→one_shot); dNEK4oF7yV (research, 1482 chars: long→research); pEjbaBnj1M (long, 1177 chars: long→one_shot); K1mHACVkPQ (long, 590 chars: long→one_shot).

Reading: the length rule is worth 5 correct answers of 31 and 3 of 4 confident decisions. Without it the baseline is 64.5% against a 58.1% majority-class rate: it is barely better than always answering `long` (+3 items of 31). ECE is lower without the rule only because the baseline is then almost never above 0.5 confidence, which is not better calibration; Brier is worse. Selective accuracy is not informative either way (4/4 and 1/1).

## 5. Threshold sensitivity (separate; not the headline)

Primary set only. Thresholds other than 0.7 were not pre-registered; do not read a "best" threshold from this.

| threshold | confident (rule on) | correct | confident (rule off) | correct |
|---|---|---|---|---|
| 0.5 | 21 | 17 | 17 | 13 |
| 0.6 | 17 | 15 | 4 | 3 |
| 0.7 | 4 | 4 | 1 | 1 |
| 0.8 | 0 | 0 | 0 | 0 |
| 0.9 | 0 | 0 | 0 | 0 |

Even at 0.5 the baseline is not accurate enough to trust, and at 0.8 it never decides.

## 6. Control set (non-tasks; negative controls, not scored for accuracy)

Ten rows: the nine non-tasks Charles named (approval, answers to the agent's questions, a pasted list, two bare continuations, an acknowledgement, a continuation without a task, an M4 sign-off, the "Design before implementation" standing rule) plus VTsRosZxiV, moved out of the research class (a question with no deliverable). Expected behaviour: ask, not act.

| | length rule on | length rule off |
|---|---|---|
| rows | 10 | 10 |
| confident (would be acted on) | 1 | 1 |
| asked | 9 | 9 |

The confident one is `3vfSNZZAcn` (long, p=0.84), a 2,044-character list of answers to the agent's questions. It stays confident without the length rule (0.73) because it is full of build/system/integrat keywords. Nothing in the baseline can tell a non-task from a task; with intake off this has no effect today, and it must be solved before intake is switched on (a "not a task" class or a guard on short/non-imperative messages).

## 7. Owner-review pool (not in the primary denominator)

13 items, 12 with a label: for completeness only.

| metric | value |
|---|---|
| items | 12 |
| overall accuracy | 5/12 = 41.7% |
| one_shot accuracy | 2/6 = 33.3% |
| long accuracy | 3/4 = 75.0% |
| research accuracy | 0/2 = 0.0% |
| confident decisions (p ≥ 0.7) | 0 |
| coverage | 0.0% |
| selective accuracy (confident only) | 0/0 = n/a |
| abstentions (asked) | 12 |
| wrong, caught by asking | 7 |
| asked but would have been right | 5 |
| confident-wrong | 0 |
| ECE | 0.198 |
| Brier | 0.686 |

Without the length rule: 4/12 correct, 1 confident (0 correct). The pool is context-dependent or unresolved by construction; its accuracy says nothing about the router.

## 8. What this does and does not show

- It shows the keyword baseline is not usable as a router on its own: weak on research (0/4 with the rule, 1/4 without), long accuracy mostly the length rule, and 87% of decisions abstain at 0.7. Abstaining is the safe failure mode, and it is what happened: 0 confident-wrong in 4 confident decisions.
- It does not show how a decision model would do, how the router behaves on runtime traces, or anything about cost. The cost-versus-always-frontier comparison is Not computed — insufficient measured token and execution data.
- The corpus is Charles's own messages to the project, one author, one product, mostly UI direction. It is not a sample of tasks the router will see.
- Labels are mine, applied from the written definitions, and every changed label is pending Charles's review (`router-eval-label-review.md`).

Reproduce: `cargo run -p backspace-core --example router_eval_report -- router-eval-v1.jsonl --threshold 0.7 [--no-length]`; `backspace-cli eval router-eval-v1.jsonl` gives the same accuracy/calibration tables (the example's `evaluate` call is the CLI's code path).
