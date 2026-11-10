# UI plan: the wireframe as a working GPUI app on macOS and Windows

Requested by Charles on 10 Nov 2026. The spec is the published wireframe (`threadbear/master-layout.html`, artifact W1yiVkytvdBLVdCNJJ9vt4). The audit that motivates the order is `UI-AUDIT-2026-11-10.md`.

## Goal

A working Cauldron desktop app, built in GPUI, on macOS and Windows, that looks like the wireframe and runs the existing core. Every milestone ends with automated tests, red first, in line with PLAN.md section 12.

## The conflict to settle first

`README.md` and `HANDOFF.md` call `crates/app` the older GPUI shell and suggest freezing it. The shipped builds (`macos.yml`, `windows.yml`) are the Tauri app. This plan follows Charles's instruction that GPUI is the app. So:

- GPUI becomes the primary shell. Tauri keeps building until UI9 and gets no new screens.
- Logic stays in `crates/core`. The GPUI shell calls it directly, with no command layer in between. The 177 Tauri commands are not ported one for one; a screen gets the calls it needs.
- If Charles wants both shells maintained, say so; the cost of that is not in this plan.

## How exactness is enforced

1. **Tokens and layout as code.** `crates/chrome` holds the palette, radii and shell geometry. Its tests compare against numbers measured from the wireframe, so the Rust cannot drift from the page. UI0 is done.
2. **Rect tests on live elements.** UI1 adds a test that renders the GPUI shell headless and compares each pane's bounds with `solve()` for every shell state. Needs `gpui` test support; to verify at the start of UI1.
3. **Screenshot parity.** Playwright renders the wireframe in dark and light. The CI runners (`macos-14`, `windows-latest`) capture the GPUI window at the same size. A per-region diff with a tolerance set after the first run. Text will not match pixel for pixel (CoreText against DirectWrite), so regions are compared by layout and colour, not glyphs.
4. **Skills as lints.** From UI2 a test fails the build on: a radius outside 6/4/3 and circles, a `px()` off the 4px scale, an em-dash in a UI string, a second icon family, a number without tabular figures, a colour outside the palette. The contrast rule is already a test.

## Two versions, one design

Same pane geometry on both. Only these differ (tested in `platforms_differ_only_in_window_controls`):

| | macOS | Windows |
|---|---|---|
| Window controls | Traffic lights in the notch | Caption buttons top right (`WindowControlArea`) |
| Notch | 126px: lights and sidebar button | 62px: sidebar button only (proposed, not yet reviewed) |
| Terminal and Split | Right edge | Left of the caption buttons |
| Shortcuts | Cmd | Ctrl |
| System font | `.SystemUIFont` | Segoe UI Variable, falls back to Segoe UI |
| Menu | Application menu bar | In-window menu button |
| Backdrop | Opaque | Opaque; Mica is an option after UI8 |
| Build and signing | Ad-hoc today; notarization needs a paid developer account | Unsigned today; SmartScreen warns |

## Milestones

| # | Milestone | Exit tests |
|---|---|---|
| UI0 | Tokens and layout contract: `crates/chrome` | Done. 11 tests green: two window sizes, five shell states, both themes, contrast, radii, platform differences. |
| UI1 | Shell skeleton in GPUI: window, notch, sidebar card, main card with tabs, right rail, drawer, split, both platforms | `cargo check -p backspace` on all three OSes in CI. Live-element rects equal `solve()` in all states. First screenshot diff recorded as the baseline. |
| UI2 | Swap the old glass palette and radii for the tokens in the existing coding workbench; delete blobs, translucent panes, violet accent | The lints in "How exactness is enforced" pass on `crates/app`. No regression in the existing 6.2k lines of behaviour. |
| UI3 | Sidebar: function icon row, search, glowing New thread button, thread list by state, usage row, user row | Thread rows come from core. State order and counts match `THREAD-STATE.md`. Keyboard and icon labels covered. |
| UI4 | Thread view: transcript, ask cards, composer (Coordinator, Auto and Medium, Send), stage bar with usage on the left | Text input and IME work on both OSes. Stage click shows detail. Empty, loading and error states drawn. |
| UI5 | New thread screen: Code and Design tabs, Hoplite shell box, machine switcher | Start a thread on this machine and on a connected one. Focus ring and tab join match the wireframe. |
| UI6 | Right rail and drawer: Highlights, Usage, patches (Koi preview, Code review), Patches list with trust | A patch asks for trust before it runs. Drawer width 200. Split chooser opens Files, Changes, Terminal, Browser. |
| UI7 | Projects, Agents, Schedule, Memory, Apps on the drawer | One screen at a time, each with its states. Nothing outside PLAN section 5. |
| UI8 | Settings, onboarding, update flow, installers | Windows installer and macOS dmg from the GPUI build. Update flow tested. |
| UI9 | Parity and retirement of the Tauri shell | Same behaviours covered by tests on GPUI. Release workflows switch to GPUI. Tauri UI deleted. |

## State matrix (before UI3)

Every screen draws: normal, empty, loading (skeleton shaped like the final layout), error (inline), offline or machine unreachable, and for threads hibernated. The wireframe draws only the normal state, so these are designed in UI3 to UI7 as each screen lands, and added to the wireframe only if Charles wants to review them first.

## Decisions for Charles

1. GPUI replaces Tauri, which retires at UI9. Assumed above.
2. Windows notch of 62px with only the sidebar button. Assumed.
3. Minimum thread column of 360px: Split closes first, then the drawer. Assumed. The wireframe does not draw it.
4. The left usage meter. Core reads token counts from session logs; a "% left" figure needs quota data that may not be in those logs. Until checked, the meter shows tokens and cost.

## Risks

- **Accessibility.** `accesskit` is a GPUI dependency; whether screen readers work on either OS is unverified. UI1 adds one test that reads the tree.
- **Text input.** GPUI text editing is custom. `gpui-component` (pulled in by `gpui-kit`) has an input; the plan is to use it for editing and style it with tokens.
- **Pinned pre-release GPUI.** `gpui-pre 0.3.6` through `gpui-kit 0.6.6`. An upgrade can break drawing code; upgrade only between milestones.
- **No local display.** The cloud container cannot open a window. Visual checks run on CI runners or on Charles's machines; layout tests run anywhere.
- **Scope.** The Tauri UI is about 7.1k lines of JS and 4k of CSS across many screens. Retiring it is the long part; UI7 is where that cost lands.
