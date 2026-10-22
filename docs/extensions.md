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

A native extension calls `Frame.register(spec)` when its script runs (Chat does
it at the end of `chat.js`; `extensions/code/code.js` is only that call):

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

State lives in prefs, as it always has. `prefs.uses` is the On list: written by
`Frame.setOn`, by the chip, and by Settings → General. Off apps stay in the
switcher, dimmed; going to one turns it on. The last app cannot be turned off.

## What still lives in the shell

Stage 1 moved the apps out, not their engines. Chat's renderer is `chat.js`;
Code's tabs/canvases engine is still `app.js` (tabs, `renderGrid`, the drawer,
project switching), and `shell.js` still knows Code's project title. Moving the
canvas engine into `extensions/code/` and the project title into the
extension's head is the next step; after that `shell.js` no longer names an
app.
