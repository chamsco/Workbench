# Router eval: label review

Definitions applied (Charles, 2026-11-14): **one_shot** = one bounded change. **long** = meaningful planning, a new system/feature/milestone, or 2+ independent build asks. **research** = a deliverable of answer, report, investigation, comparison or design proposal with no PR. **Excluded** = approvals, acknowledgements, continuation without a task, answers to agent questions, pasted data, non-deliverable questions. No fourth production class.

Every label here is provisional. "Owner review" means Charles must confirm; until then nothing is ground truth. An item is in the primary benchmark only if its label confidence is medium or high and it is not context-dependent.

## A. Label changes (ten proposed corrections)

| id | draft label | revised | confidence | owner review | group | reason |
|---|---|---|---|---|---|---|
| `CK3sKxD5Yq` | long | one_shot | med | yes | primary | One bounded design-replication change plus working rules (before/after pics, study structure first). Draft said long. |
| `VTsRosZxiV` | research | excluded (control) | high | yes | control | A question about harness behaviour; no deliverable (no answer document, no PR). Draft said research. |
| `mr7DwLWcwE` | research | long | med | yes | primary | A new feature (context sharing for project/message) with a feasibility question attached; the feature dominates. Draft said research. |
| `34gjvHeyGQ` | one_shot | long | high | yes | primary | Four independent fixes (sidebar fusion, browser/desktop-use failure, scrollbar, loading dots): 2+ independent asks. Draft said one_shot. |
| `Dix3H6Fyph` | one_shot | long | med | yes | primary | Rail behaviour change plus building a new-tab start page: 2 independent builds. Draft said one_shot. |
| `dNEK4oF7yV` | long | research | med | yes | primary | The router-visible task is "do M0 only: the audit table, then stop"; a report with no PR. It is also the kickoff of a milestone programme, which a reader could call long. Draft said long. |
| `yrQdLMvh4s` | research | long | med | yes | primary | Three deliverables (wireframe, revised plan.md, measured time-waste analysis with proposed and measured fixes). Draft said research. |
| `GtKvSDHfQB` | one_shot | long | low | yes | context_dependent | Directed rework of rail, tabs and navigation (several independent changes). Context-dependent. Draft said one_shot. |
| `at7XCYUxdG` | one_shot | unresolved | low | yes | context_dependent | Design feedback whose referents ("the sidebar", "the new thread tab") are not visible; label unresolved (draft one_shot). |
| `xhMcczbtWd` | research | research (confirmed after restoration) | med | yes | primary | Restored in full: a pasted article then "how do i integrate this into our workflow ?". A design proposal, no PR. Draft label unchanged. |

`at7XCYUxdG` stays unresolved: it is design feedback on screens the router would not see, and the draft label (one_shot) is not defensible without them.

## B. Compound messages (13): dominant path

Rule: the dominant path is the deliverable that decides the class. Count of numbered points and length were not used.

| id | final | group | dominant path |
|---|---|---|---|
| `XvJZK8Koo9` | long | primary | 5+ independent UI/product changes plus a new IDE multiplexer: long. |
| `QeQzLM8MFd` | long | primary | Replace docked chat with a floating Ask, IDE proposal, chat fixes: several independent builds, long. |
| `9VgNem1Ftm` | long | primary | Orchestrator/executor flow, harness-to-harness messaging, per-app settings, UI cleanup: long. |
| `pzG2mTog1s` | long | primary | Five separate features (agents sidebar, memory in Home, folders, voice mascot, org charts): long. |
| `a4NXvQCcJx` | long | primary | Windows build + mascot colours + restoring the Plan/Execute/Ship/Commit pipeline; the pipeline restore is feature work: long (medium). |
| `F4LqFDKhEV` | long | primary | Many layout changes, a freeze bug and update-button states in one message: long. |
| `34gjvHeyGQ` | long | primary | Four independent fixes: 2+ independent asks, long (changed from one_shot). |
| `Dix3H6Fyph` | long | primary | Rail fix + new-tab page build: 2 independent builds, long (changed from one_shot). |
| `izLnjF4TnL` | long | owner_review_pool | Verify git state, close M0, start M1: the dominant path is unclear (a verification step then a milestone). Unresolved. |
| `yrQdLMvh4s` | long | primary | Wireframe + plan.md + measured analysis: three deliverables, long (changed from research). |
| `pEjbaBnj1M` | long | primary | Fuse a look, merge two codebases into a master, adopt features: long. |
| `GtKvSDHfQB` | long | context_dependent | Several independent rail/tab/navigation changes; context-dependent: moved to the context group. |
| `K1mHACVkPQ` | long | primary | Token changes, a font download, then a new pass of agent screens: long. |

## C. Context-dependent messages (8): not reconstructed

Reconstructing router-visible context would mean choosing which earlier messages and agent replies the router "saw". That choice would be mine, not recorded history, so none was reconstructed. All eight are in the owner-review pool, outside the primary denominator. Each is shown with the proposed label for review.

| id | chars | proposed label | what it depends on |
|---|---|---|---|
| `x7JWmQtsnD` | 341 | one_shot | an earlier layout ("it", the layout being reversed) |
| `YdeMLtxPLj` | 39 | one_shot | which README and which branch |
| `MHgLPqGHzr` | 389 | research | named skills and "which branch is next" |
| `SrJFhTmoT9` | 85 | one_shot | "do it" = pushing a folder as a branch; wireframe walkthrough |
| `txK4nC7PZi` | 65 | one_shot | "compare then archive": which two things |
| `GtKvSDHfQB` | 667 | long | the rail/tab design being discussed |
| `at7XCYUxdG` | 1311 | unresolved | the on-screen sidebar and tabs |
| `Ke2VfK4qip` | 477 | one_shot | the existing wireframe being varied |

## D. Low-confidence items moved to the owner-review pool

| id | chars | proposed | why unresolved |
|---|---|---|---|
| `umnF2qkesp` | 1416 | one_shot | Bounded corrections to an existing audit doc; one_shot or research (report only). |
| `izLnjF4TnL` | 429 | long | Compound with no clear dominant path. |
| `ijp71Z5AZt` | 9126 | long | Investigation of Codex model pinning, then a bounded code/test/docs change: one_shot or long. |
| `9o9f3tCf4u` | 161 | research | A design deliverable (wireframe suggestions) that fits none of the three classes cleanly. |
| `UANSZCrSdP` | 321 | long | Look up references then finalise a system; part research, part build. |

## E. Restored and re-checked messages

`wYCfpHgci1` (long), `BjxMTw3GNE` (long), `ijp71Z5AZt` (pool: one_shot or long), `xhMcczbtWd` (research) were truncated at ~8 KB in the draft; full texts were fetched by message id (9,460 / 19,567 / 9,126 / 8,267 chars) and verified against the API body. `Ke2VfK4qip` was restored (477 chars). The draft's label for each of the four large ones was re-read against the full text: only `xhMcczbtWd` needed a look (kept research, owner review yes), and `ijp71Z5AZt` moved to the pool because its front half is an investigation and its back half a bounded change.

## F. Full primary table

| id | label | conf | owner review | chars | text (start) |
|---|---|---|---|---|---|
| `XvJZK8Koo9` | long | high | no | 898 | here is the updating flow from T3 code that i would like to for us to use, and for some reason the ascii shark… |
| `QeQzLM8MFd` | long | med | no | 677 | ok i think there is a misunderstanding or you fundamentally DO NOT see what you are doing, APPS SHOULD BE CONT… |
| `9VgNem1Ftm` | long | high | no | 761 | ok is it possible to have a flow with : for Code : the ADE, it should be like Claude projects just we should h… |
| `CK3sKxD5Yq` | one_shot * | med | yes | 226 | 1- show me what UI changes you did, when you do a PR ALWAYS put a before after pic, for the design of this bra… |
| `mr7DwLWcwE` | long * | med | yes | 130 | i would also like if you could add some context sharing also for a project or message like Muse does, would we… |
| `SpHCk8xVs9` | one_shot | high | no | 125 | Comment on `home-target-before-after.png` (pinned at 81% from the left, 13% from the top):  add a separator fo… |
| `DpQd1dfZwF` | one_shot | high | no | 163 | the right side chat should only be for the HOME Page, everything else shoudl be full taking the space, wiith t… |
| `4QtyBgvypt` | one_shot | high | no | 88 | leave some space for the window utility buttons (-,. square,X0 and the mac os equivalent |
| `pzG2mTog1s` | long | high | no | 627 | ok now for the Agents side of the chat interface, show me if its updated, latest brain agent design,... and fo… |
| `a4NXvQCcJx` | long | high | no | 626 | geenrate a real windows exe so i can try out the newest App, and also the abrain mascot shouldnt be 100% pink … |
| `F4LqFDKhEV` | long | med | no | 3232 | ok, design wise , HOME , app should open with the left side bar opened not the right one , put recent activity… |
| `aeykVkqsM1` | long | high | no | 2503 | Build me a Langfuse-style observability dashboard from my own AI coding sessions for the last 7 days.  SOURCES… |
| `W7gZj66JGc` | one_shot | high | no | 85 | I GOT BUGS ON THE RIGHT SIDE BAR WHHEN I TRY TO CLOSE IT DOWN OR OPEN UP  LETS FIX IT |
| `34gjvHeyGQ` | long * | high | yes | 776 | start to examine and here are the dfew pooints we need to check , 1- we still, have soime issues with the left… |
| `Dix3H6Fyph` | long * | med | yes | 398 | NO the pill app rail should be the compacted / sidebar hidden mode, if not it should put all the items on top … |
| `RjVowB8ciP` | one_shot | high | no | 187 | the new window on the right side need to follow the theme of the app, too,  once the sideabr is deployed , put… |
| `c4dBkhh14Z` | one_shot | med | no | 214 | the readme is a little too charged and not up to date with the latest vision of the project, make a For Humans… |
| `dNEK4oF7yV` | research * | med | yes | 1482 | You are lead engineer for Cauldron (also called Workbench). PLAN.md in the repo root is the single source of t… |
| `VQWiPDEgbm` | research | med | no | 5328 | You are the lead engineer for Cauldron (also called Workbench). I'm attaching Cauldron_M0_Revised_Audit.md. Th… |
| `NsYAnzNThU` | long | high | no | 2601 | M1 review: the implementation and tests establish a good foundation, but I don't want to close M1 until we res… |
| `wYCfpHgci1` | long | high | no | 9460 | ok this is my last instruction im going to slep and wont be able to accept choices unfortunately, make an agen… |
| `BjxMTw3GNE` | long | high | no | 19567 | # MASTER PROMPT — M1 FINAL HARDENING AND SIGN-OFF  You are continuing the M1 hardening work on Cauldron / Work… |
| `xhMcczbtWd` | research | med | yes | 8267 | Subagents are the biggest cause of AI failures  In most harnesses: 1. Main agent writes a summary of what it k… |
| `yrQdLMvh4s` | long * | med | yes | 1149 | https://github.com/mikecann/threadbear this screensho and plus this repo is another direction , I would like u… |
| `pEjbaBnj1M` | long | high | no | 1177 | here is how Threadbar looks , i want to fuse thiss look , with what we already have, put Everything from brain… |
| `3rC3iSEhvD` | research | high | no | 291 | J'aimerais continuer ici le travail de la session session_01Q4CcB9WmEtqqHJrMx6vZZC. Merci de lire cette sessio… |
| `aH7hF8sRkG` | long | high | no | 994 | https://www.affectui.com/  https://tuffui.com/components/clause-playbook  https://tuffui.com/components/renewa… |
| `K1mHACVkPQ` | long | med | no | 590 | use the tough card like 22, 16, 12 pixels. Accent let's use purple. Like tough is blue, and let's go from this… |
| `bf7eL9rznC` | one_shot | high | no | 63 | the numbers arent centered for this one but the rest look fine, |
| `ZyixXbQTvw` | long | high | no | 892 | I would require these closure items:  1. Add pre-execution human approval for baseline commands. 2. Resolve th… |
| `MGRARd1VKL` | one_shot | high | no | 135 | add it to the plan in one of the gate , once our gates ge good enough with the plan->execute)>ship->deploy we … |

`*` = label changed from the draft.
