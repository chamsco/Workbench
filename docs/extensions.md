# Extensions

Backspace's window is a frame. The apps inside it — Chat and Code — are
extensions in `crates/tauri-app/ui/extensions/`. The frame knows them by their
manifest only: the switcher, the title-bar chip (name, On, New) and the window
classes all come from there.

## The list

`ui/extensions/index.json` is the order apps load in:

```json
{ "schema": 1, "extensions": ["chat", "code"] }
```

## The manifest

`ui/extensions/<id>/extension.json`:

```json
{
  "schema": 1,
  "id": "chat",
  "name": "Chat",
  "description": "Threads with a model behind each.",
  "icon": "bubble",
  "order": 10,
  "section": "chat",
  "className": "chat zone",
  "view": { "kind": "native", "script": "chat.js", "style": "chat.css" },
  "action": { "label": "New chat", "icon": "compose" }
}
```

- **id** is the mode: it is written into `prefs.mode`, so it is also the key
  the rest of the page has always used (`chat`, `code`).
- **order** places the app in the switcher. Frame pages sit at 0 (Apps) and 30
  (Memory); Chat is 10 and Code 20, the reading order the window had.
- **section** is the id of the `<section class="zone">` the frame makes for the
  app in `#body`; **className** its classes. The app builds inside it in
  `mount`.
- **view.kind: "native"** loads **style** first — inserted before `home.css`,
  where `chat.css` used to be, so the cascade is unchanged — then **script**.
  The script is a classic script: it shares the page's globals with the frame
  and with `app.js`.
- **view.kind: "web"** puts **entry** in an iframe inside the section instead
  of loading a script. The app SDK bridge (`docs/apps.md`) is not wired to it
  yet, so an app that wants agents, storage or tools still installs through
  Apps.
- **action** is the chip's New. `label` is the tooltip; `icon` is a name from
  `P` in `app.js`.

## The host contract

A native extension calls `Frame.register(spec)` when its script runs, at the
end of its file:

- `mount(el)` — build inside the section, once. Code puts its `#grid` there.
- `init()` — after the shell and its data are up.
- `paint(shown)` — the frame laid the app's section out; `shown` says whether
  it is visible now. Chat mounts lazily here.
- `show()` / `hide()` — the app became the active one, or stopped being it.
- `elFor()` — which section shows the app right now; the manifest's by
  default. Code returns `chat` in ADE pair mode and `canvas` in the IDE.
- `classes()` — window classes while the app is active (Code: `pair-on`).
- `newInstance(anchor)` — the chip's New; `anchor` is the button, for a
  popover.
- `head()` — HTML for the title bar's head while the app is active, or `null`
  when it has nothing to say. Code returns the project's folder (or
  "No folder") and wires it to its project menu in the shell.
- `windowTitle()` — the document title (taskbar, alt-tab), or `null` for just
  "Backspace". Code names the project it has open.

State lives in prefs, as it always has. `prefs.uses` is the On list: written by
`Frame.setOn`, by the chip, and by Settings → General. Off apps stay in the
switcher, dimmed; going to one turns it on. The last app cannot be turned off.

## What is where

Each app owns its surface. `chat.js` draws the conversation, its composer and
the thread; `extensions/code/code.js` is the whole workbench — the tab strip,
the per-project layout, the canvases and their panes (session, files,
terminal, browser, diagram, docs, options, trace), the wall, the tickets
drawer, the view menu and the project title. The project title is the app's
`head()`, which is why the shell no longer names Code.

`app.js` is the runtime both apps read: agents and approvals, projects and
machines, prefs, the snapshot, settings, updates, and the agent rows in the
sidebar. `shell.js` is the window: rail, switcher, title bar, split and
status. Neither names an app.

Two seams remain, both one-way and by design. The ADE sidebar (in `app.js`)
and Home read the open tab through `Code.tab()` and link into
`Code.openView()`, because the tab belongs to the workbench; and Chat's
`openCanvas` turns the app on and asks `Code` for the canvas. Agents are the
runtime's, so Home and the sidebar keep drawing their rows.
