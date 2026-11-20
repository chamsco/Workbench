# What is left before Cauldron works as an app

Written 15 Nov 2026 after Charles approved the Tight tiles look (`design/cauldron-tight-tiles/`). Read from the repo and project notes, not from a run: nobody has seen the GPUI app on a display from the cloud, so every "built" below means "code exists and tests pass", not "verified on screen".

## Where things stand

- Core (`crates/core`): M0 to M4 signed off (gateway, routing, intake, planning). M5 onwards not started. The section 10 router eval is owed first; intake stays off until it runs.
- GPUI app (`crates/app`, about 11.7k lines with `crates/chrome`): shell, thread view, new-thread, panes, settings, terminal exist. Palette and geometry are the **old** v3/new-look ones, not Tight tiles.
- Tauri app: retired to `legacy/tauri-app`. The only build people can download that has working screens is the Tauri one (`windows-tauri` release); GPUI builds on `windows-preview` have been hollow in places.
- macOS GPUI CI: never proven (earlier runs cancelled with no steps).

## Needed to make the app work, in order

1. **Put the app on a display and screenshot it** (Mac or Windows). Everything below is blind without this. Remote Control on Charles's machine, or CI screenshots that can be downloaded.
2. **Retarget `crates/chrome` to Tight tiles**: new radii (8/6/4), 4px gap, no top bar, tabs in card header, support card replacing the 200px drawer, notch (Win 100x40) and Mac hover indent. Regenerate the golden from the new board (`tools/wireframe-golden/extract.js` still reads `cauldron-master-layout.html`; point it at `design/cauldron-tight-tiles/index.html`). Update the golden tests; this is a design change, read the diff as one.
3. **Theme layer**: painting behind the window, one frosted frame layer, glass cards. Needs pre-blurred cached crops (no backdrop blur in GPUI) and a Plain fallback.
4. **Sidebar and stream list from core**: grouped Needs you / Running / Done. Core gaps (`UI3-CORE-MAP.md`): no per-stream "Needs you", no hibernated or archived, no quota window (show tokens and cost instead).
5. **Stream view**: transcript, ask cards, composer with Plan toggle, scroll ticks, context bar. Text input and IME on both OSes is the first risk.
6. **New stream**: centred box, folder or "No folder", worktree, run location (this machine and connected machines exist; cloud sandbox does not), GitHub action (meaning to confirm).
7. **Support card**: tabs + the + menu. Browser, Changes, Files, Terminal, Git first; Simulator and Trace last. Each needs its own empty, loading, error and offline state. Terminal docks under the stream.
8. **Real agent execution**: M5 (Builder) and M6 (execution infra, local executors, herdr). Until then coding-CLI agents are refused and Bash is unsandboxed. A stream that edits code in a worktree and shows the diff is the minimum "working" loop.
9. **Settings, onboarding, loader, installers, update flow** (UI8). Onboarding replaces sign-in with Machines. Windows installer and a signed or at least notarisable macOS build.
10. **Parity and retirement** of the Tauri shell (UI9): Home, Memory, Agents, Apps, Schedule screens are not reachable in GPUI yet.

## Decisions waiting on Charles

- Card corner 8 or 6.
- Whether the Cauldron mark is a placeholder.
- GitHub button meaning.
- Router eval set review, then explicit go for M5.
- Whether the Tauri build stays the download until GPUI reaches parity (recommended: yes).

## Known limits to keep in mind

`start_task` lacks cross-project authorization (needed before shared workspaces). Filesystem checks are not race-proof. Per-subscription usage is not in core.
