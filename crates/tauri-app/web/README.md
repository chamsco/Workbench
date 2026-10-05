# Chat UI (Whirl components on Backspace)

The Tauri app's chat face. It is [Whirl](https://github.com/whirlchat/whirl)'s
chat components (MIT, see `src/whirl/LICENSE`) — the thread view, Markdown
rendering, composer, sidebar rows, dialogs and design tokens — running on
Backspace's own data: `src/bridge.ts` talks to the Tauri commands that the
vanilla shell (`../ui/chat.js`) hands over, in place of Whirl's Convex and
Clerk backend.

- `src/whirl/` — files taken from Whirl's `apps/v2`, mostly unchanged.
  Changed: `lib/attachments.ts` (pure helpers only), `lib/use-attachments.ts`
  (files read locally), `lib/messages.ts` (types only),
  `components/thread/markdown.tsx` (no Mermaid), `styles/globals.css`
  (paths; Backspace ships Inter itself). Whirl's logo and name are not used.
- `src/app/` — the containers rewritten for Backspace: `ChatApp`,
  `ThreadView`, `AssistantMessage`, `Composer`, `RoutePicker`, `ThreadList`.
- `src/shiki-slim.ts` — 27 code languages instead of all of shiki's.

Build (writes `../ui/chat-web/`, which is committed so the Rust build never
needs Node):

    npm install
    npm run build
