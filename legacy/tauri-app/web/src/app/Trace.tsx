// How a reply was made: its trace as a waterfall. One row per span (the
// reply, then each tool call), bars placed on the reply's own timeline,
// failures in the destructive colour. Click a row for its input and output.

import { useEffect, useState } from "react";
import { IconAlertTriangleFilled, IconChevronRight } from "@tabler/icons-react";

import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { getHost, openTrace, useChat, type Span, type Trace } from "../bridge";

function dur(ms: number) {
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  return `${Math.floor(ms / 60_000)} min ${Math.round((ms % 60_000) / 1000)} s`;
}

const num = (v: unknown) => (typeof v === "number" ? v : undefined);

/** A tool's input in one line: the command, path or query rather than raw JSON. */
function gist(input: string) {
  try {
    const v = JSON.parse(input);
    if (v && typeof v === "object") {
      for (const k of ["command", "file_path", "path", "pattern", "url", "query", "prompt"]) if (typeof v[k] === "string") return v[k];
    }
  } catch {
    /* not JSON */
  }
  return input;
}

export function TracePanel() {
  const { traceOpen } = useChat();
  const [trace, setTrace] = useState<Trace | null | undefined>(undefined);
  const [open, setOpen] = useState<string | null>(null);
  useEffect(() => {
    setTrace(undefined);
    setOpen(null);
    if (traceOpen) getHost().invoke<Trace | null>("trace_get", { id: traceOpen }).then(setTrace).catch(() => setTrace(null));
  }, [traceOpen]);
  if (!traceOpen) return null;
  const root = trace?.spans[0];
  const t0 = root?.start ?? 0;
  const total = Math.max(1, (root?.end ?? t0) - t0);
  const a = root?.attrs ?? {};
  const tin = num(a["gen_ai.usage.input_tokens"]);
  const tout = num(a["gen_ai.usage.output_tokens"]);
  const cost = num(a["cost.usd"]);
  const model = (a["gen_ai.response.model"] ?? a["gen_ai.request.model"]) as string | undefined;
  const tools = trace ? trace.spans.length - 1 : 0;
  return (
    <Dialog open onOpenChange={(o) => !o && openTrace(null)}>
      <DialogContent className="max-h-[86vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{root?.name ?? "Trace"}</DialogTitle>
          <DialogDescription>
            {trace === undefined
              ? "Loading…"
              : trace === null
                ? "This trace is gone (traces live in the data folder's traces/)."
                : [
                    dur(total),
                    model,
                    tools ? `${tools} tool call${tools === 1 ? "" : "s"}` : "no tool calls",
                    tin != null && tout != null ? `${tin.toLocaleString()} in · ${tout.toLocaleString()} out tokens` : null,
                    cost ? `$${cost.toFixed(4)}` : null,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
          </DialogDescription>
        </DialogHeader>
        {trace && (
          <div className="flex flex-col gap-px text-[12.5px]/5">
            {trace.spans.map((s) => (
              <Row key={s.span_id} s={s} t0={t0} total={total} open={open === s.span_id} toggle={() => setOpen(open === s.span_id ? null : s.span_id)} />
            ))}
            <p className="mt-3 text-[11.5px]/4 text-muted-foreground">
              Trace {trace.trace_id}. OpenTelemetry spans; set a collector in Settings → Tracing to send them on.
            </p>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function Row({ s, t0, total, open, toggle }: { s: Span; t0: number; total: number; open: boolean; toggle: () => void }) {
  const end = s.end ?? s.start;
  const left = ((s.start - t0) / total) * 100;
  const width = Math.max(0.8, ((end - s.start) / total) * 100);
  const tool = s.kind === "tool";
  return (
    <div className="rounded-lg hover:bg-muted/50">
      <button type="button" onClick={toggle} aria-expanded={open} className="grid w-full cursor-pointer grid-cols-[minmax(0,13rem)_1fr_4.5rem] items-center gap-3 px-2 py-1.5 text-left">
        <span className={`flex min-w-0 items-center gap-1.5 ${tool ? "pl-4" : "font-medium"}`}>
          <IconChevronRight size={13} className={`shrink-0 text-muted-foreground transition-transform ${open ? "rotate-90" : ""}`} />
          {s.error && <IconAlertTriangleFilled size={13} className="shrink-0 text-destructive" />}
          <span className={tool ? "shrink-0" : "truncate"}>{s.name}</span>
          {tool && s.input && <span className="min-w-0 truncate font-mono text-[11px] text-muted-foreground">{gist(s.input)}</span>}
        </span>
        <span className="relative h-2 rounded-full bg-muted">
          <span
            className={`absolute inset-y-0 rounded-full ${s.error ? "bg-destructive" : tool ? "bg-primary/70" : "bg-foreground/35"}`}
            style={{ left: `${left}%`, width: `${Math.min(width, 100 - left)}%` }}
          />
        </span>
        <span className="text-right font-mono text-[11.5px] tabular-nums text-muted-foreground">{dur(end - s.start)}</span>
      </button>
      {open && (
        <div className="flex flex-col gap-2 px-8 pb-3">
          {s.error && <p className="text-destructive">{s.error}</p>}
          {s.input && <Block label="Input" text={s.input} />}
          {s.output && <Block label={tool ? "Output" : "Answer"} text={s.output} />}
          {Object.keys(s.attrs).length > 0 && (
            <dl className="grid grid-cols-[max-content_1fr] gap-x-3 gap-y-0.5 font-mono text-[11px]/4 text-muted-foreground">
              {Object.entries(s.attrs).map(([k, v]) => (
                <div key={k} className="contents">
                  <dt>{k}</dt>
                  <dd className="truncate text-foreground/80">{typeof v === "string" ? v : JSON.stringify(v)}</dd>
                </div>
              ))}
            </dl>
          )}
        </div>
      )}
    </div>
  );
}

function Block({ label, text }: { label: string; text: string }) {
  return (
    <div>
      <div className="text-[11px]/4 font-medium text-muted-foreground">{label}</div>
      <pre className="mt-0.5 max-h-48 overflow-auto rounded-md bg-muted px-2.5 py-1.5 font-mono text-[11.5px]/4 whitespace-pre-wrap">{text}</pre>
    </div>
  );
}
