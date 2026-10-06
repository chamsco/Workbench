# Backspace apps

An app is a view with its own agent that anyone can make and share: a folder
with a `backspace-app.json` and a web page. Backspace shows the page in a
sandboxed frame (in the rail, in Apps, or beside a chat in a split) and lets
it ask models the user has switched on, keep a little data, and offer tools
to coding CLIs.

The example in `apps/prompt-lab/` asks two models the same prompt and shows
the answers side by side. It ships in the desktop app under Apps → Examples.

## Install

- **From a URL**: paste the link to a `backspace-app.json` in Apps. Backspace
  downloads the manifest, then every path in `files` (and `view.entry`,
  `icon.file`) relative to it.
- **From a folder**: Apps → From folder… copies the folder (skipping dot
  files, `node_modules` and `target`). Install again to pick up changes.

Installed apps live in `<data>/apps/<id>/` (`BACKSPACE_DATA` overrides the
data dir). Reinstalling keeps the app's stored values.

## backspace-app.json

```json
{
  "schema": 1,
  "id": "prompt-lab",
  "name": "Prompt Lab",
  "version": "0.1.0",
  "description": "Ask two models the same thing and read the answers side by side.",
  "author": "Backspace",
  "homepage": "https://example.com",
  "icon": { "glyph": "PL", "color": "#2563eb" },
  "view": { "entry": "index.html" },
  "agent": {
    "name": "Prompt Lab",
    "instructions": "Answer directly and concisely; no preamble.",
    "route": { "kind": "cli", "provider": "claude", "model": null },
    "suggestions": ["Compare two answers to a tricky question"]
  },
  "permissions": ["agent", "routes", "storage", "notify"],
  "tools": [
    {
      "name": "render",
      "description": "Render the current piece to MP4.",
      "input_schema": { "type": "object", "properties": { "format": { "type": "string" } } },
      "command": ["node", "tools/render.js"]
    }
  ],
  "files": ["index.html", "app.js", "style.css"]
}
```

| Field | Meaning |
|---|---|
| `schema` | Manifest version. This build reads `1`. |
| `id` | 1-48 lowercase letters, digits and dashes. The folder name; unique. |
| `name`, `version`, `description`, `author`, `homepage` | Shown in Apps. |
| `icon` | `glyph` (one or two characters) on `color`, or `file` (a png/svg in the app). |
| `view.entry` | The page, a path inside the app folder. Or `view.url`: an http(s) page (a dev server, a hosted app). One is required. |
| `agent.instructions` | The system prompt for every reply the app asks for. |
| `agent.route` | Who answers unless the page passes another. Default: the user's default chat model. |
| `permissions` | What the page may ask for (below). Anything not listed is refused. |
| `tools` | Commands coding CLIs can call (below). |
| `files` | Paths to download on a URL install. |

All paths are relative, use forward slashes, and may not leave the app folder.

## The page

The page runs in a frame with `sandbox="allow-scripts allow-forms
allow-popups allow-modals allow-downloads"`: no access to Backspace, its
cookies, or the user's files. It talks to the host through the SDK:

```html
<script src="__backspace/sdk.js"></script>
```

(served by Backspace for every installed app; `apps/sdk.js` is the same file
if you would rather ship a copy, e.g. for a `view.url` app).

```js
const { app, user, dark } = await backspace.ready();

// permission "agent": ask the app's agent. Resolves when the reply is done.
const r = await backspace.agent.ask("Write a haiku about rain", {
  route,                          // optional: one of backspace.routes()
  thread: previous?.thread,       // optional: continue a conversation
  onText: text => (out.textContent = text),   // streamed so far
});
r.text; r.thread; r.message;
await backspace.agent.stop(r.thread);
await backspace.threads();        // this app's threads
await backspace.thread(id);       // one, with its messages

// permission "routes": the models the user has switched on
const routes = await backspace.routes();   // [{ label, sub, route }]

// permission "storage": up to 1 MB of JSON values per app
await backspace.storage.set("draft", { text: "..." });
await backspace.storage.get("draft");
await backspace.storage.set("draft", null);   // remove

backspace.notify("Rendered", "ok");   // permission "notify": a toast
backspace.openUrl("https://...");     // permission "open_url": the browser

backspace.on("theme", ({ dark }) => {});   // light/dark changed
backspace.on("thread", thread => {});      // one of your threads changed
```

The SDK sets `data-theme="dark|light"` and `color-scheme` on `<html>` when
the theme changes, so `:root[data-theme="dark"]` styles just work.

Threads an app's agent writes are kept with the user's chats but only shown
to the app.

## Tools for coding CLIs

Each tool is a command run in the app's folder. Its input arrives as JSON on
stdin; whatever it prints is the result; a non-zero exit is an error. Coding
CLIs get every installed app's tools through `backspace mcp`, named
`<app id with _ for ->__<tool>` (e.g. `prompt_lab__render`): Claude Code in
chats and Pair threads, and CLI workers in projects.

## Security

- The frame has no IPC; the host checks every call against `permissions`.
- Apps' threads and storage are separate per app.
- Tools run as the user, like any program they install. Only install apps
  you trust; Apps shows what each one asks for before you install it.
