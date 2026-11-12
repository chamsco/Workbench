# Wireframe to core: what exists for UI3, UI4 and UI5

Read from `crates/core` on 11 Nov 2026. "Exists" means a call or type is there today. "Derive" means the screen can compute it from what exists. "Missing" means no core support: the wireframe shows a thing the core cannot yet produce. Missing items are not mocked in the app; the row is left out until core supports it.

The model behind most of the "missing" rows is `THREAD-STATE.md` (design only, no Rust written, blocked on decision D1: amend Cauldron section 5 to add thread tickets and filing).

## Sidebar (UI3)

| Wireframe | Source | Status |
|---|---|---|
| Function icons: Threads, Projects, Schedule, Agents, Usage, Memory, Apps | Screens in UI7 | Icons now, destinations later |
| Badge counts on Threads and Projects | Needs-you count (below); `Projects::list()` with `ProjectState::pending_approvals()` | Derive, project level only |
| Search threads | Filter `ThreadInfo.title` and `.preview` | Exists |
| New thread button | `Chats::create_in(route, Scope)`, then `Chats::send` | Exists |
| Thread row: title, repo, age, pin | `Chats::list()` gives `ThreadInfo { title, updated, pinned, project, preview, busy, unread, failed }` | Exists |
| Group Running | `ThreadInfo.busy` | Exists |
| Group Failed (red label) | `ThreadInfo.failed.is_some()` | Exists |
| Group Done | not busy, not failed | Derive |
| Group Needs you (red dot, "2 asks") | `ThreadInfo.unread` means an unopened reply, not a question to you. Real asks live in `intake::PendingAsk` and `ProjectState::pending_approvals()`, per project, not per thread | Missing per thread (needs Run in `THREAD-STATE.md`). First cut: unread replies, labelled "Unread" until then |
| "3 agents" on a row, "3 agents running" with a graph | `AgentRecord.status.is_active()` per project; no history for the graph | Count derive; graph missing |
| Hibernating 1, Archived 23 | No such concept anywhere in core | Missing |
| Filed by librarian | `stages::librarian_prompt`, `rule_fates` exist as logic; nothing ties them to a thread row | Missing in the UI path |
| Usage row: "62% left" with a meter | `usage::summarize()` returns token counts and `Tokens::total()` only; no quota window | Missing. Show tokens and cost (UI-PLAN decision 4) |
| User row | No profile name in prefs | Use the OS user name |

## Thread view (UI4)

| Wireframe | Source | Status |
|---|---|---|
| Transcript | `Chats::thread(id)` gives `Msg { role, text, at, status, cost_usd, thinking, trace, ... }`; `Chats::open` takes a notify channel that fires as replies stream | Exists |
| Typing box, Send, Stop | `Chats::send(id, text, attachments, reply_to, Ctx)` and `Chats::stop(id)` | Exists |
| Coordinator selector | `Scope.agent`, `Route`, `agents::Agents` | Exists |
| "Auto" and "Medium" | `effort.rs`, `ladder.rs` | Check that both map to a setting before wiring |
| Header chips (Code, project pinned) | `Thread.project`, `intake::label(TaskType)` | Derive |
| People avatars and teammate messages | `board.rs` is agent to agent only. Teammates are in `THREAD-STATE.md` rule 8 | Missing; leave out |
| Ask cards with option buttons | `intake::PendingAsk` and `options::OptionSet` with `options::pick` | Exists per project; per thread missing |
| Stage bar Plan, Execute, Ship, Commit | `phase::Stage::ALL`, `PhaseState` | Exists per project; per thread missing (`THREAD-STATE.md` step 6) |
| Usage left on the stage bar | As the usage row above | Tokens and cost |

## New thread (UI5)

| Wireframe | Source | Status |
|---|---|---|
| Code and Design tabs | `intake::TaskType` and `intake::first` triage | Exists |
| Machine switcher: This machine, plus | `fleet::Fleet`, `fleet::MachineInfo`, `remote::Remote::connect` | Exists |
| Start the thread | `Chats::create_in` then `Chats::send` | Exists |

## What this means

The first working app can start threads, stream replies, show a project's stages and approvals, and list threads as Running, Failed, Done and Unread. It cannot honestly show Needs you per thread, Hibernating, Archived, teammate messages, quota percentages or the agent graph. Those need new core work first. Decision for Charles: build that core (D1 and hibernation) after UI5, or sooner.
