# tauri-shell-rs

I opened all three files the audit names: `crates/tauri-app/src/main.rs` (2143 lines, every cited region read at ±15 lines), `crates/cli/src/main.rs` (cited arms at L45 and the `models()` doc at L85–87), and `crates/tauri-app/build.rs` (3 lines, no comments, no findings to rule on). Every rule-of-line and quote checked out except one: the finding at `main.rs:527` quoted the comment as a single spliced line that merged L527–531 and dropped the words "bar for the whole scan"; I re-anchored it to the real block (L527–531) and quote the exact lines. I also confirmed the audit's own arithmetic claim with a grep: the file contains exactly 16 `// ---` section banners, at the 16 lines listed — no missed banner and no phantom one. Nothing else was dropped, so `rejected` is 0. One verdict was overruled: the finding at L527 arrived as `refactor`, but under this audit's rules `refactor` is reserved for comments that are load-bearing *because the code is a workaround* and whose `why` must name a root-cause fix — here the code is the framework's correct shape (`async` command + `blocking()`), and the load-bearing claim is a platform rule (Tauri v2 runs a non-async command on the UI thread) plus its Windows symptom, neither of which this repo can change. That is `keep`, so `Refactor first` is empty.

## Delete (21)

| file:line | kind | the comment | why it goes |
| --- | --- | --- | --- |
| crates/tauri-app/src/main.rs:116 | tombstone | `/// Commits a day for the last two weeks and lines changed, for Home.` | Sits on `git_changes` (uncommitted files + branch; its real doc is L117) while describing `git_activity` at L137: `activity(path, 14)` and the days/added/removed JSON carry that shape themselves. Wrong contract where it sits. |
| crates/tauri-app/src/main.rs:195 | section-banner | `// ---------------------------------------------------------------- machines` | Divider over `machines`, `select_machine`, `add_machine`, `remove_machine`; the names are the heading. |
| crates/tauri-app/src/main.rs:250 | section-banner | `// ---------------------------------------------------------------- prefs, updates` | `prefs`, `set_tabs`, `set_theme` below already group the block. |
| crates/tauri-app/src/main.rs:273 | history-narration | `` `/// open), which is what the old `set_tabs` did for everyone.` `` | Changelog prose about a removed API inside the new command's doc; L271–272 already state the invariant (empty `workspace` writes the global layout). |
| crates/tauri-app/src/main.rs:486 | section-banner | `// ---------------------------------------------------------------- projects` | `pick_folder`, `open_project`, `close_project`, `forget_project` follow; nothing added. |
| crates/tauri-app/src/main.rs:523 | section-banner | `// ---------------------------------------------------------------- CLIs, models, routers` | Restates the command names beneath it (`harnesses`, `cli_login`, model and route commands). |
| crates/tauri-app/src/main.rs:777 | section-banner | `// ---------------------------------------------------------------- chat` | Every item below is `chat_*`; the heading is the prefix. |
| crates/tauri-app/src/main.rs:827 | section-banner | `// ------------------------------------------------------- local tiny models` | `local_models`, `set_subagent_model`, `set_local_tasks` and their own docs say it. |
| crates/tauri-app/src/main.rs:865 | section-banner | `// ---------------------------------------------------- skills and worktrees` | The two functions below are `skills_catalog` and `worktrees`. |
| crates/tauri-app/src/main.rs:1020 | obvious | `/// Show a file or folder in the OS file manager.` | `reveal_path` plus the cfg-gated `explorer`, `open -R`, `xdg-open` calls state the whole contract; signature and body suffice. |
| crates/tauri-app/src/main.rs:1062 | section-banner | `// ---------------------------------------------------------------- accounts` | Duplicates the `accounts`/`account_*` names below. |
| crates/tauri-app/src/main.rs:1130 | section-banner | `// ---------------------------------------------------------------- terminal` | Duplicates the `terminal`/`term_*` names below. |
| crates/tauri-app/src/main.rs:1252 | section-banner | `// ---------------------------------------------------------------- memory` | Duplicates the `memory_*` names below. |
| crates/tauri-app/src/main.rs:1324 | section-banner | `// ---------------------------------------------------------------- tracing` | `trace_get`, `set_tracing`, `trace_test` carry the grouping. |
| crates/tauri-app/src/main.rs:1342 | section-banner | `// ---------------------------------------------------------------- companion` | `companion_status`, `set_companion`, `qr_svg` state it. |
| crates/tauri-app/src/main.rs:1376 | section-banner | `// ---------------------------------------------------------------- apps` | Duplicates the `apps_list`/`app_*` names below. |
| crates/tauri-app/src/main.rs:1535 | section-banner | `// ---------------------------------------------------------------- agents` | Duplicates the `agents_list`/`agent_*` names below. |
| crates/tauri-app/src/main.rs:1643 | section-banner | `// ---------------------------------------------------------------- cloud` | Duplicates the `cloud_*` and `plans` names below. |
| crates/tauri-app/src/main.rs:1675 | section-banner | `// ---------------------------------------------------------------- boot` | Two-function block (`boot`, `ready`); the names are the heading. |
| crates/tauri-app/src/main.rs:2066 | restates-code | `// Nothing said: framing is allowed.` | The next line is `assert_eq!(frameable_from(None, None), Some(true))`; the doc at L2062–2063 already explains the header decision. |
| crates/cli/src/main.rs:45 | restates-code | `// The tiny local models, and which one small delegated jobs would use.` | Labels the arm `[cmd] if cmd == "models" => models()` and repeats the fuller doc on `models()` at L85–87. |

## Refactor first (0)

| file:line | the comment | what the code should become |
| --- | --- | --- |

## Keep (1)

| file:line | the comment | what cannot change |
| --- | --- | --- |
| crates/tauri-app/src/main.rs:527-531 | `// Off the main thread. This scans the machine — spawning codex, claude,` … `// synchronous command runs on the UI thread in Tauri v2` | Tauri v2 runs a non-async command on the UI thread — a platform rule this repo cannot change — so the machine scan (spawning codex, claude, cursor-agent, opencode to read versions and sign-in state) must go through `blocking()`, and `rescan` at L542 has the same shape. Nothing in the code says "must be off the UI thread", so deleting it invites a simplification straight back into a frozen window. |
