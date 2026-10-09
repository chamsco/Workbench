// Mounts the Whirl-based chat into the Tauri window. ui/chat.js calls
// `window.WhirlChat.mount(...)` with the host API once its own data is
// ready, and `refresh()` on every core change.

import { Component, StrictMode, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";

import "./styles.css";
import { ChatApp } from "./app/ChatApp";
import { richPrompt } from "./app/RichText";
import { goHome, openThread, refresh, setHost, setScope, type Host } from "./bridge";

let root: Root | null = null;

/* A render error used to unmount the whole chat face, leaving an empty
   window; now it says what broke and offers to draw it again. */
class Guard extends Component<{ children: ReactNode }, { err: Error | null }> {
  state = { err: null as Error | null };
  static getDerivedStateFromError(err: Error) {
    return { err };
  }
  componentDidCatch(err: Error) {
    console.error("chat view crashed", err);
  }
  render() {
    if (!this.state.err) return this.props.children;
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center text-[13px] text-muted-foreground">
        <b className="text-[15px] text-foreground">The chat view hit an error</b>
        <code className="max-w-xl break-words">{this.state.err.message}</code>
        <button type="button" className="rounded-full bg-well px-4 py-2 text-foreground hover:bg-accent" onClick={() => this.setState({ err: null })}>
          Draw it again
        </button>
      </div>
    );
  }
}

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
    setRich: (on: boolean) => void;
    __TAURI__?: { core: { invoke: (cmd: string, args?: unknown) => Promise<unknown> } };
  }
}

// Rich answers: teach chat models OpenUI Lang (Settings → Chat → Rich answers).
window.setRich = (on) => {
  void window.__TAURI__?.core.invoke("set_genui", { prompt: on ? richPrompt() : null }).catch(() => {});
};
let richOn = true;
try { richOn = localStorage.getItem("bs.rich") !== "0"; } catch {}
window.setRich(richOn);

const api = {
  mount(el: HTMLElement, sideEl: HTMLElement, host: Host, user: string) {
    setHost(host);
    syncDark();
    matchMedia("(prefers-color-scheme: dark)").addEventListener("change", syncDark);
    new MutationObserver(syncDark).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    setScope(host.scope(), host.agentsMode());
    root?.unmount();
    root = createRoot(el);
    root.render(
      <StrictMode>
        <Guard>
          <ChatApp sideEl={sideEl} user={user} />
        </Guard>
      </StrictMode>,
    );
    void refresh();
  },
  refresh: () => refresh(),
  open: (id: string) => openThread(id),
  home: () => goHome(),
  setScope: (s: string | null, agents = false) => setScope(s, agents),
};

window.WhirlChat = api;
