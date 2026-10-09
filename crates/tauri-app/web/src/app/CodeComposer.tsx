// Code chat's composer: not Chat's capsule but the agents' composer card
// (ui/home.js, shell.css .hc), after Cursor's agent window. Project, branch
// and machine sit above it; the text spans the card; under it, attach, Plan
// mode (read and propose, change nothing; Shift+Tab), the CLI with its
// effort, and send. The CLI and effort picker (ui/picker.js) unfolds from
// the card itself, the same one the agents use.

import { useEffect, useRef, useState, type RefObject } from "react";
import {
  IconArrowUp,
  IconChevronDown,
  IconDeviceDesktop,
  IconFolder,
  IconGitBranch,
  IconListCheck,
  IconPaperclip,
  IconPlayerStopFilled,
  IconX,
} from "@tabler/icons-react";

import { useAttachments } from "@/lib/use-attachments";
import { ComposerAttachments } from "@/components/composer-attachments";
import { getHost, type NewFile, type Route } from "../bridge";
import { Logo } from "./RoutePicker";

const baseName = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;

export function CodeComposer({
  value,
  onValueChange,
  route,
  onRouteChange,
  onSubmit,
  onStop,
  generating,
  inThread,
  scope,
  textareaRef,
}: {
  value: string;
  onValueChange: (v: string) => void;
  route: Route | null;
  onRouteChange: (r: Route) => void;
  onSubmit: (text: string, files: NewFile[], plan: boolean) => Promise<void>;
  onStop?: () => void;
  generating: boolean;
  inThread: boolean;
  scope: string;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
}) {
  const host = getHost();
  const attachments = useAttachments();
  const [plan, setPlan] = useState(false);
  const [picking, setPicking] = useState(false);
  const [sending, setSending] = useState(false);
  const [run, setRun] = useState(host.codeRun);
  const [branch, setBranch] = useState("");
  const panel = useRef<HTMLDivElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const cli = route?.kind === "cli" ? route.provider : null;
  // The picker reports its whole choice each time; compare with what is current now.
  const live = useRef({ cli, run });
  live.current = { cli, run };

  useEffect(() => {
    host.invoke<{ branch: string }>("git_changes", { path: scope }).then((r) => setBranch(r.branch), () => setBranch(""));
  }, [scope, inThread, host]);
  useEffect(() => () => host.picker.close(), [host]);

  const togglePicker = () => {
    if (picking) return host.picker.close();
    setPicking(true);
    requestAnimationFrame(() => {
      if (!panel.current) return;
      void host.picker.mount(
        panel.current,
        { model: cli ? host.picker.model(cli) : "", ...run },
        (c) => {
          const next = host.picker.cli(c.model);
          if (next !== "auto" && next !== live.current.cli) onRouteChange({ kind: "cli", provider: next, model: null });
          const was = live.current.run;
          if (c.permission !== was.permission || c.effort !== was.effort) {
            setRun({ permission: c.permission, effort: c.effort });
            void host.setCodeRun(c.permission, c.effort);
          }
        },
        () => setPicking(false),
      );
    });
  };

  const ready = attachments.drafts.every((d) => d.status === "ready" || d.status === "error");
  const files = attachments.drafts.filter((d) => d.status === "ready" && d.data);
  const canSend = !sending && ready && (value.trim().length > 0 || files.length > 0);
  const submit = async () => {
    if (!canSend) return;
    setSending(true);
    const text = value;
    onValueChange("");
    attachments.clear();
    try {
      await onSubmit(text, files.map((d) => ({ name: d.name, mime: d.type, data: d.data! })), plan);
    } catch {
      onValueChange(text);
    } finally {
      setSending(false);
    }
  };

  const name = !route ? "Pick a CLI" : cli === "claude" ? "Claude Code" : cli ? host.providerName(route) : host.routeName(route);
  const effort = cli ? host.picker.effortName(cli, run.effort) : "";
  const stop = generating && onStop;

  return (
    <div className="code-composer">
      {!inThread && (
        <div className="hc-above">
          <button type="button" className="hc-chip" data-popper onClick={(e) => host.projectMenu(e.currentTarget)}>
            <IconFolder size={13} />
            <span className="v">{baseName(scope)}</span>
            <IconChevronDown size={12} />
          </button>
          {branch && (
            <span className="hc-chip quiet" title="The branch the project folder is on">
              <IconGitBranch size={13} />
              <span className="br">{branch}</span>
            </span>
          )}
          <span className="hc-chip quiet">
            <IconDeviceDesktop size={13} />
            {host.machineName()}
          </span>
        </div>
      )}
      <div
        className={`hc${plan ? " plan" : ""}`}
        onDragOver={(e) => e.dataTransfer.types.includes("Files") && e.preventDefault()}
        onDrop={(e) => {
          e.preventDefault();
          if (e.dataTransfer.files.length) attachments.addFiles(e.dataTransfer.files);
        }}
      >
        {attachments.drafts.length > 0 && (
          <div className="px-2 pt-2">
            <ComposerAttachments
              drafts={attachments.drafts}
              rejectionFor={(d) => (d.status === "error" ? (d.error ?? "Couldn't read it.") : null)}
              onRemove={attachments.remove}
            />
          </div>
        )}
        <textarea
          ref={textareaRef}
          rows={inThread ? 1 : 2}
          value={value}
          autoFocus
          onChange={(e) => {
            onValueChange(e.target.value);
            e.target.style.height = "auto";
            e.target.style.height = `${Math.min(200, e.target.scrollHeight)}px`;
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
              e.preventDefault();
              void submit();
            } else if (e.key === "Tab" && e.shiftKey) {
              e.preventDefault();
              setPlan((p) => !p);
            }
          }}
          onPaste={(e) => {
            const pasted = e.clipboardData?.files;
            if (pasted && pasted.length > 0) {
              e.preventDefault();
              attachments.addFiles(pasted);
            }
          }}
          placeholder={plan ? "Plan changes" : `Ask ${name} to change ${baseName(scope)}`}
          aria-label="Message"
        />
        <div className="hc-bar">
          <input
            ref={fileRef}
            type="file"
            multiple
            hidden
            onChange={(e) => {
              if (e.target.files) attachments.addFiles(e.target.files);
              e.target.value = "";
            }}
          />
          <button type="button" className="hc-ic" aria-label="Attach files" title="Attach files" onClick={() => fileRef.current?.click()}>
            <IconPaperclip size={15} />
          </button>
          <button
            type="button"
            className="hc-plan"
            aria-pressed={plan}
            title="Plan mode: read the project and propose, change nothing (Shift+Tab)"
            onClick={() => setPlan((p) => !p)}
          >
            <IconListCheck size={14} />
            Plan
            {plan && <IconX size={12} className="x" />}
          </button>
          <span className="sp" />
          <button type="button" className="hc-model" aria-expanded={picking} onClick={togglePicker}>
            {route && <Logo id={cli ?? route.provider} size={14} />}
            <span className="v">{name}</span>
            {effort && <span className="eff">{effort}</span>}
            <IconChevronDown size={12} className="ic" />
          </button>
          <button
            type="button"
            className="hc-send"
            aria-label={stop ? "Stop" : "Send"}
            disabled={!stop && !canSend}
            onClick={() => (stop ? onStop!() : void submit())}
          >
            {stop ? <IconPlayerStopFilled size={13} /> : <IconArrowUp size={16} stroke={2.4} />}
          </button>
        </div>
        <div ref={panel} className="hc-panel" hidden={!picking} />
      </div>
    </div>
  );
}

/* Beside a Code chat: what the project has to show, each opening the side panel. */
export function CodeSide({ scope }: { scope: string }) {
  const host = getHost();
  const items: [string, string][] = [
    ["changes", "Changes"],
    ["history", "History"],
    ["files", "Files"],
    ["sessions", "Sessions"],
  ];
  return (
    <nav className="code-side" aria-label={`On ${baseName(scope)}`}>
      <div className="cs-h">On {baseName(scope)}</div>
      {items.map(([k, l]) => (
        <button key={k} type="button" onClick={() => host.openPanel(k)}>
          {l}
        </button>
      ))}
    </nav>
  );
}
