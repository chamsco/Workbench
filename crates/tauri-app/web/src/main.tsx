// Mounts the Whirl-based chat into the Tauri window. ui/chat.js calls
// `window.WhirlChat.mount(...)` with the host API once its own data is
// ready, and `refresh()` on every core change.

import { StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";

import "./styles.css";
import { ChatApp } from "./app/ChatApp";
import { goHome, openThread, refresh, setHost, setScope, type Host } from "./bridge";

let root: Root | null = null;

/* Whirl themes through a `.dark` class; Backspace through data-theme or
   the system. Keep the class in step. */
function syncDark() {
  const html = document.documentElement;
  const t = html.dataset.theme;
  const dark = t === "dark" || (t !== "light" && matchMedia("(prefers-color-scheme: dark)").matches);
  html.classList.toggle("dark", dark);
}

declare global {
  interface Window {
    WhirlChat: typeof api;
  }
}

const api = {
  mount(el: HTMLElement, sideEl: HTMLElement, host: Host, user: string) {
    setHost(host);
    syncDark();
    matchMedia("(prefers-color-scheme: dark)").addEventListener("change", syncDark);
    new MutationObserver(syncDark).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    setScope(host.scope());
    root?.unmount();
    root = createRoot(el);
    root.render(
      <StrictMode>
        <ChatApp sideEl={sideEl} user={user} />
      </StrictMode>,
    );
    void refresh();
  },
  refresh: () => refresh(),
  open: (id: string) => openThread(id),
  home: () => goHome(),
  setScope: (s: string | null) => setScope(s),
};

window.WhirlChat = api;
