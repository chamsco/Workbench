# Router eval: label and group review v2

Taxonomy unchanged: one_shot (one bounded, clearly specified change), long (meaningful planning, a new system/feature/milestone, or 2+ independent build asks), research (the deliverable is an answer, report, investigation, comparison or design proposal, no implementation PR), excluded (not a new task). Classified by the dominant requested deliverable, not by phrasing, length or numbered points.

**Every label in this document is a proposal.** `owner_approved` is false on every row. Nothing is ground truth until Charles approves it.

## A. Changes from v1 (group counts: primary 31→31, pool 13→17, controls 10→6, stress 1→1)

| id | v1 group / label | v2 group / label | confidence | reason |
|---|---|---|---|---|
| `VTsRosZxiV` | control / None | primary / research | med | Reclassified from control. Asks for confirmation of a technical point ("we can queue messages, right?"): an answer is a research deliverable. Short and interrogative is not a reason to exclude. Weak point: "a harness" refers to an earlier statement, and the intended answer may be a design decision rather than a fact. |
| `d4MEWNBX8V` | control / None | primary / research | med | Reclassified from control. Requests a deliverable: map every requirement in §§3–12 to a §5 entry and flag unmappable ones for the owner. A mapping/traceability report with no implementation PR: research. Also confirms continuous-chat scope (a decision, not a task). Could be one_shot if the mapping is written into PLAN.md. |
| `3vfSNZZAcn` | control / None | primary / one_shot | med | Reclassified from control. Authorises and requests work: replace the PLAN.md §5 TODO with the eleven-entry inventory, record five decisions in the audit, remove resolved flags, update the M0 blocker section, return the section, audit and diff. Documentation only ("Do not ... modify product code"): one bounded change, so one_shot; a reader could call it long (it touches PLAN.md and the audit). |
| `CK3sKxD5Yq` | primary / one_shot | pool (low_confidence) / one_shot | low | Moved out of primary on recheck: "do this version" points at an attachment that is not recorded, and the message mixes a standing rule (before/after pictures) with one design-replication change. one_shot is the draft-v1 reading; unresolved. |
| `mr7DwLWcwE` | primary / long | pool (ambiguous) / long | low | Moved out of primary on recheck: a new feature ("context sharing like Muse") and a feasibility question ("would websockets work here?") with no stated dominant path; long or research. |
| `MHgLPqGHzr` | pool / research | pool (ambiguous) / research | low | Ambiguous, not context-dependent: asks for a critique against named skills and which branch is next; "fix whats left" could read as build (one_shot/long) or as a review report (research). |
| `bf7eL9rznC` | primary / one_shot | pool (context_dependent) / one_shot | low | Moved out of primary: "the numbers arent centered for this one" has no recorded referent. |
| `BUau1Gzo6o` | control / None | pool (control_disputed) / one_shot | low | Moved out of controls: "make a version on the mac" requests work, and "continue where we left off" needs prior context; a task requiring clarification is not a non-task. |

## B. The three controls that were tasks

- **`3vfSNZZAcn`** (2,044 chars): not only answers to five agent questions. It says "you now have authorization" to replace the §5 TODO with the eleven-entry inventory, record five decisions in the audit, remove resolved flags, update the M0 blocker section, and return the section, audit and diff, with "Do not ... modify product code". Dominant deliverable: one bounded documentation change, so **one_shot**, medium confidence. Alternative reading: long (several documents). Owner review required.
- **`d4MEWNBX8V`** (707 chars): the §5 list is pasted, but the message asks to "map every requirement in sections 3–12 to one of these §5 entries" and to flag unmappable ones for the owner. Deliverable: a mapping report, no PR: **research**, medium. Alternative: one_shot if the mapping is written into PLAN.md. "The continuous-chat scope is a yes" is a decision, not a task.
- **`VTsRosZxiV`** (97 chars): "we can queue messages right ?" asks for an answer to a technical question; an answer is a research deliverable. **research**, medium. Weak point: "a harness" refers to a statement outside the message, and the answer may call for a design decision rather than a fact.

## C. Context-dependent set (exactly eight) and the other pool reasons

| id | reason | proposed label | note |
|---|---|---|---|
| `MHgLPqGHzr` | ambiguous | research | Asks for a critique against named skills and which branch is next; "fix whats left" could also read as build. |
| `mr7DwLWcwE` | ambiguous | long | Moved out of primary on recheck: a new feature ("context sharing like Muse") and a feasibility question ("would websockets work here?") with no stated dominant path; long or research. |
| `GtKvSDHfQB` | context_dependent | long | Directed rework of rail, tabs and navigation (several independent changes). Context-dependent. Draft said one_shot. |
| `Ke2VfK4qip` | context_dependent | one_shot | One more wireframe variation with stated changes. |
| `SrJFhTmoT9` | context_dependent | one_shot | "do it" refers to pushing a folder as a branch; plus a walkthrough wireframe. |
| `YdeMLtxPLj` | context_dependent | one_shot | Push and update the README. |
| `at7XCYUxdG` | context_dependent | None | Design feedback whose referents ("the sidebar", "the new thread tab") are not visible; label unresolved (draft one_shot). |
| `bf7eL9rznC` | context_dependent | one_shot | Moved out of primary: "the numbers arent centered for this one" has no recorded referent. |
| `txK4nC7PZi` | context_dependent | one_shot | Compare then archive, name the master "cauldron". Meaningless without the thread. |
| `x7JWmQtsnD` | context_dependent | one_shot | Layout reversal with stated target; moderate size. |
| `BUau1Gzo6o` | control_disputed | one_shot | Moved out of controls: "make a version on the mac" requests work, and "continue where we left off" needs prior context; a task requiring clarification is not a non-task. |
| `9o9f3tCf4u` | low_confidence | research | A design deliverable (wireframe suggestions) that fits none of the three classes cleanly. |
| `CK3sKxD5Yq` | low_confidence | one_shot | Moved out of primary on recheck: "do this version" points at an attachment that is not recorded, and the message mixes a standing rule (before/after pictures) with one design-replication change. one_shot is the draft-v1 reading; unresolved. |
| `UANSZCrSdP` | low_confidence | long | Look up references then finalise a system; part research, part build. |
| `ijp71Z5AZt` | low_confidence | long | Investigation of Codex model pinning, then a bounded code/test/docs change: one_shot or long. |
| `izLnjF4TnL` | low_confidence | long | Compound with no clear dominant path. |
| `umnF2qkesp` | low_confidence | one_shot | Bounded corrections to an existing audit doc; one_shot or research (report only). |

No missing context was reconstructed. `bf7eL9rznC` ("the numbers arent centered for this one") left the primary set. `MHgLPqGHzr` is **ambiguous**, not context-dependent, and is represented that way (flag `ambiguous`, pool reason `ambiguous`). Pool reasons and flags agree row by row (a test enforces this).

## D. Rechecked labels that stay in primary

| id | label | confidence | check |
|---|---|---|---|
| `34gjvHeyGQ` | long | high | four independent fixes; "like shown here" points at an image but the class does not depend on it |
| `Dix3H6Fyph` | long | medium | rail fix plus a new-tab page build: two builds; opens with "NO", a correction of earlier work |
| `dNEK4oF7yV` | research | medium | "Do M0 only: the audit table ... Stop and wait": a report; also the kickoff of a milestone programme, which a reader could call long |
| `yrQdLMvh4s` | long | medium | wireframe + revised plan.md + measured analysis with proposed fixes: three deliverables |
| `xhMcczbtWd` | research | medium | restored in full; a pasted article then "how do i integrate this into our workflow ?": a design proposal |

`CK3sKxD5Yq` and `mr7DwLWcwE` were rechecked and moved to the pool (section A). `izLnjF4TnL`, `ijp71Z5AZt`, `9o9f3tCf4u`, `UANSZCrSdP` stay in the pool as low-confidence; `Ke2VfK4qip` stays context-dependent. Owner-review status for all of them: **required, not given**.

## E. Flags and provenance fields

- `truncated-8kb` is removed from `flags` on all rows. The historical fact is kept as `historically_truncated_in_draft: true` on `wYCfpHgci1`, `BjxMTw3GNE`, `ijp71Z5AZt`, `xhMcczbtWd` (current full texts: 9,460 / 19,567 / 9,126 / 8,267 chars). `Ke2VfK4qip` is 477 chars, context-dependent.
- Added: `pool_reason`, `v1_group`, `v1_label`, `label_changed_from_v1`, `group_changed_from_v1`, `v2_movement_reason`, `owner_approved`, `proposed_label`. Kept: `draft_label`, `source_message_id`, `created_at`, `text_sha256`, `chars`, `source_fetch`.
- `chars`, `text_sha256`, `source_message_id`, `created_at` and `text` are unchanged from v1 on every row (checked by script and by tests).

## F. Primary candidate (v2)

| id | label | confidence | owner review | chars | text (start) |
|---|---|---|---|---|---|
| `XvJZK8Koo9` | long | high | required | 898 | here is the updating flow from T3 code that i would like to for us to use, and for some reason the a… |
| `QeQzLM8MFd` | long | med | required | 677 | ok i think there is a misunderstanding or you fundamentally DO NOT see what you are doing, APPS SHOU… |
| `9VgNem1Ftm` | long | high | required | 761 | ok is it possible to have a flow with : for Code : the ADE, it should be like Claude projects just w… |
| `VTsRosZxiV` | research | med | required | 97 | a harness can’t receive a message in the middle of a run. is wrong we caqn queue messages right ? |
| `SpHCk8xVs9` | one_shot | high | required | 125 | Comment on `home-target-before-after.png` (pinned at 81% from the left, 13% from the top):  add a se… |
| `DpQd1dfZwF` | one_shot | high | required | 163 | the right side chat should only be for the HOME Page, everything else shoudl be full taking the spac… |
| `4QtyBgvypt` | one_shot | high | required | 88 | leave some space for the window utility buttons (-,. square,X0 and the mac os equivalent |
| `pzG2mTog1s` | long | high | required | 627 | ok now for the Agents side of the chat interface, show me if its updated, latest brain agent design,… |
| `a4NXvQCcJx` | long | high | required | 626 | geenrate a real windows exe so i can try out the newest App, and also the abrain mascot shouldnt be … |
| `F4LqFDKhEV` | long | med | required | 3232 | ok, design wise , HOME , app should open with the left side bar opened not the right one , put recen… |
| `aeykVkqsM1` | long | high | required | 2503 | Build me a Langfuse-style observability dashboard from my own AI coding sessions for the last 7 days… |
| `W7gZj66JGc` | one_shot | high | required | 85 | I GOT BUGS ON THE RIGHT SIDE BAR WHHEN I TRY TO CLOSE IT DOWN OR OPEN UP  LETS FIX IT |
| `34gjvHeyGQ` | long | high | required | 776 | start to examine and here are the dfew pooints we need to check , 1- we still, have soime issues wit… |
| `Dix3H6Fyph` | long | med | required | 398 | NO the pill app rail should be the compacted / sidebar hidden mode, if not it should put all the ite… |
| `RjVowB8ciP` | one_shot | high | required | 187 | the new window on the right side need to follow the theme of the app, too,  once the sideabr is depl… |
| `c4dBkhh14Z` | one_shot | med | required | 214 | the readme is a little too charged and not up to date with the latest vision of the project, make a … |
| `dNEK4oF7yV` | research | med | required | 1482 | You are lead engineer for Cauldron (also called Workbench). PLAN.md in the repo root is the single s… |
| `VQWiPDEgbm` | research | med | required | 5328 | You are the lead engineer for Cauldron (also called Workbench). I'm attaching Cauldron_M0_Revised_Au… |
| `d4MEWNBX8V` | research | med | required | 707 | §5 — Complete feature inventory  5.1 Conversational intake and classification  5.2 Specification and… |
| `3vfSNZZAcn` | one_shot | med | required | 2044 | My decisions on the five flags:  1. Memory auto-capture → 5.8, in scope. It must comply with the §8 … |
| `NsYAnzNThU` | long | high | required | 2601 | M1 review: the implementation and tests establish a good foundation, but I don't want to close M1 un… |
| `wYCfpHgci1` | long | high | required | 9460 | ok this is my last instruction im going to slep and wont be able to accept choices unfortunately, ma… |
| `BjxMTw3GNE` | long | high | required | 19567 | # MASTER PROMPT — M1 FINAL HARDENING AND SIGN-OFF  You are continuing the M1 hardening work on Cauld… |
| `xhMcczbtWd` | research | med | required | 8267 | Subagents are the biggest cause of AI failures  In most harnesses: 1. Main agent writes a summary of… |
| `yrQdLMvh4s` | long | med | required | 1149 | https://github.com/mikecann/threadbear this screensho and plus this repo is another direction , I wo… |
| `pEjbaBnj1M` | long | high | required | 1177 | here is how Threadbar looks , i want to fuse thiss look , with what we already have, put Everything … |
| `3rC3iSEhvD` | research | high | required | 291 | J'aimerais continuer ici le travail de la session session_01Q4CcB9WmEtqqHJrMx6vZZC. Merci de lire ce… |
| `aH7hF8sRkG` | long | high | required | 994 | https://www.affectui.com/  https://tuffui.com/components/clause-playbook  https://tuffui.com/compone… |
| `K1mHACVkPQ` | long | med | required | 590 | use the tough card like 22, 16, 12 pixels. Accent let's use purple. Like tough is blue, and let's go… |
| `ZyixXbQTvw` | long | high | required | 892 | I would require these closure items:  1. Add pre-execution human approval for baseline commands. 2. … |
| `MGRARd1VKL` | one_shot | high | required | 135 | add it to the plan in one of the gate , once our gates ge good enough with the plan->execute)>ship->… |
