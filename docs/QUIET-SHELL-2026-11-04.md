# Direction 04 in the app

> The shell was tightened again in `docs/UNSLOP-2026-11-05.md`: one framed
> sheet, flat surfaces, and no floating command/dock pills.

The quiet shell is now the default surface of `crates/tauri-app/ui`. The
engine underneath is unchanged; this is the frame, the navigation and the
pages around it.

## What the shell owns

| Prototype page | App page | What draws it |
| --- | --- | --- |
| Today (per project) | Today | `quiet.js`: the day header, date navigation, status/project grouping, chat list, worked/messages/to-dos/decisions/chats cards, machines and team dock |
| Machines + Harnesses | Machines | `quiet.js`: every paired host with OS, state, paths and tags; a per-host harness version table with updates |
| Meet your team | Team | `quiet.js`: bot cards, the Doctor banner, and a full agent sheet (instructions, runs-on, provider/model, fallbacks, intelligence, kind) |
| Home / HQ | Home | `quiet.js`, live snapshot, bots, chats and routines |
| Work | Work | approvals, tickets and routines from the snapshot |
| Needs / Board / Review / Automate | same | approvals, ticket states, routines |
| Inbox / Calendar / Tasks | same | chats, routine wakes, tickets |
| Canvas | Canvas | `prefs.tabs` and the existing canvas layouts |
| Code | Code | the session transcript (default) and the existing ADE/IDE panes behind Session / Panes, plus the live team wall (`#qWall`) |
| Crew | Crew | the existing chat face in agents mode (`#chat`, `#chatList`) |
| Limits | Limits | snapshot cost, tokens and per-agent decisions |
| Settings | Settings | the existing settings sheet, opened on a section |
| Plugins / Skills / Apps / Memory | same | the existing zone renderers |

The top bar's **?** opens the Helper Agent: a paper panel with suggested
questions and short answers about Needs you, canvases, the team wall,
approvals, Crew, Limits and themes. It is a guide only; it does not call the
engine.

Bot faces in Crew, the bot pages and the creation sheet use the same
`open-bot-avatar` package as the shell. The old orb stays as the fallback if
the bundle is not present.

The left rail is projects; the sidebar is page-specific; the top bar is a
crumb, the command row, search and overflow. Rail, sidebar and top bar share
one tinted frame; the page is a white sheet inset 8px from the right and bottom
edges, with a 1px edge and a 12px radius. The rail is a flush 48px column, not
a floating card. The project rail's buttons open a project's **Today** page. It keeps the day
the app is on in `todayDate`, lists the last seven days in the sidebar, and
reaches any other day through the date picker, the previous/next arrows or the
**Today** button. Chats are grouped by status (Working / Needs you / Later) or
by project, and the right column stays honest to the data: active span, chats
and replies, finished/review tickets, approvals, places, and the team on duty.

**Machines** is a first-class page, not a settings section: each host shows its
OS, link state, running/waiting counts, paths and tags (local, Tailscale), and
the harness table carries a version per host with Update actions. **Team** is
the bot directory with a real sheet: name, role, instructions, runs-on host,
provider, model, fallback chain, intelligence, fast mode, quick rotation and
Agent/Doctor kind. The sheet saves through `agent_save`; the fallback chain is
part of the `Agent` and the core retries it when the primary route fails. The
sheet's effort, fast/rotation and kind switches live in `bs.q_agent_extra`.
The Doctor banner scans failed chats, retries them with `chat_retry`, purges
ghosts with `chat_purge_ghosts`, and runs automatically when Team is opened.

Above every page, chrome is split into three rails. The top bar keeps the
**Code / Work / Canvas / Crew** mode switch, centered. The top of the sidebar is
the **Needs you / Agent board / Code review / Automations / Limits** icon row.
The bottom status strip is the third place: **Today / Machines / Team** centered,
with machine state on the left and Settings, Light/Night, Glass, Help and the
account on the right. All are icons, with `aria-label`/`title` tooltips, dots
for waiting work, and a soft fill for the active place.

The Code session is the one place that reads as a session rather than a page:
its sidebar is the Sessions list (New session, This Mac, recent sessions,
Cloud when present, Connect machine), its agent rows and transcript use the
model provider's mark (Claude, Codex/OpenAI, Cursor, Grok, OpenCode), and its
footer is the full-width strip of agent rings over project/branch, changed
files, spend, Jump and Environment. The pane grid is one switch away under
Panes.

## Files

- `ui/quiet.css` — the Direction 04 palette, frame, sidebar, pages and the
  light/dark/glass skins. Loaded last.
- `ui/quiet.js` — the page router, data adapters, sidebar and page renderers.
  It calls the existing `setMode`, `setSide`, `openProject`, `openSettings`,
  `setDrawer`, `addCanvas` and `Chat`/`WhirlChat` APIs.
- `ui/shell.js` — knows `crew` as a zone and hands Crew to the agents face.
- `ui/vendor/open-bot-avatar.js` — the exact `open-bot-avatar` package bundle
  (same one the prototype uses), loaded for the shell's team-wall faces.
- `ui/app.js` — routes the old `setMode`/`setChatView` callers into the quiet
  page when one is active; boots `Quiet` after the first refresh.
- `web/src/bridge.ts` — tolerates a null `agent_projects` reply (the harness
  can answer `null`; a missing list is not a crash).
- `web/src/app/bot-face.tsx` — the React wrapper for the same package face;
  `avatar.tsx` and `orb.tsx` fall back to the orb if the bundle is absent.
- `ui/chat-web/` — rebuilt from `web/` after the bridge and bot-face changes.

## Defaults and skins

The first run resolves the old `system` theme to **light**, so the app opens on
warm paper. The sidebar's sun/moon/drop buttons choose Light, Dark or Glass;
those choices persist in `prefs.theme` and `localStorage`.

## Verification

```sh
node tools/uiharness/run.mjs --chrome <chrome> \
  --eval-file tools/uiharness/evals/quiet-demo.js \
  --eval-file tools/uiharness/evals/quiet-nav.js
```

`quiet-demo.js` installs a realistic project, bots, groups, tickets and a
routine; `quiet-nav.js` walks Home → Board → Needs → Work → Canvas → Limits →
Settings (Appearance) → Crew → Home. `quiet-global-nav.js` checks the same
**Needs you / Agent board / Code review / Automations / Limits** links and all
four mode switches from the shared header, including from Crew. The Code frame
is the app's `shot.js` fixture with `Quiet.go("code")`; its session transcript
is the default and `quiet-session.js` checks Session → Panes → Session, the
provider marks, the full-width footer, the filter and sending.
`quiet-actions.js` adds a canvas, opens a ticket and an approval, starts a
session, and opens/answers/closes the Helper Agent. `quiet-today.js` checks
the slim flush rail, the Today day navigation and regrouping, the machines
and harness cards, and the team sheet: fallback add, save, and the doctor.
`quiet-chrome.js` checks the centered top command row and the bottom utility
rail, including night mode and the glass scene. The screenshots in
`docs/screenshots/2026-11-05/` are `home`, `work`, `today`, `machines`, `team`,
`agent`, `code`, `today-dark`, `today-narrow`, `glass-today` and `glass-code`;
the earlier `docs/screenshots/2026-11-04/` set covers Board, Crew, Helper,
Settings and the three skins.

`npx tsc --noEmit` and `npm run build` in `crates/tauri-app/web` are green.

## Not in this pass

- The prototype's second-pane switcher (left / right / bottom placement) is not
  rebuilt. Code keeps the app's canvases, tabs, terminal, browser and drawer.
- `tools/verify/agents-projects.js` is written against an older Agents
  sidebar (Bots / Chats / Projects). The current chat face uses Bots / Group
  chats / Artifacts, so that suite's section expectations are stale.
