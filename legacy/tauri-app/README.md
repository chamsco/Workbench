# Retired: the Tauri shell

Retired on 10 Nov 2026 at Charles's request. The app is now the GPUI shell in `crates/app`
(docs/cauldron/UI-PLAN.md). Nothing here is built, tested or shipped.

It stays in the tree for one reason: it is the working reference for the screens the GPUI
shell has not rebuilt yet (UI3 to UI7: Home, Memory, Agents, Apps, Schedule, onboarding,
update flow, voice). Read a screen here, rebuild it in GPUI against the tokens, then delete
the old one. UI9 deletes this directory.

The last build of it is the `windows-preview` and `mac-latest` pre-releases that were already
published. The source at the last commit that built it is `e3d472a`.
