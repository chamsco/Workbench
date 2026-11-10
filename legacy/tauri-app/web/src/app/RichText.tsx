// Rich answers: an assistant reply is Markdown, except ```openui blocks,
// which are OpenUI Lang (thesysdev/openui, MIT) rendered as real UI: cards,
// tables, charts, steps, follow-ups. The model learns the language from a
// prompt built from a small subset of OpenUI's chat library (setRich), so the
// cost per turn stays a couple of thousand tokens.

import { Component, type ReactNode, useEffect, useRef, useState } from "react";
import { Renderer, createLibrary, type OpenUIError } from "@openuidev/react-lang";
import { ThemeProvider, openuiChatLibrary } from "@openuidev/react-ui";
import "@openuidev/react-ui/layered/styles/index.css";

import { Markdown } from "@/components/thread/markdown";
import { AskCard, parseAsk, parseRoutine, RoutineProposalCard, splitFences } from "./ask";

// What the model is taught.
//
// The renderer takes the whole chat library, so anything it writes still shows —
// but the parser validates every call against that library, and silently drops
// what is not in it. Card's signature (which the prompt prints) lists Form among
// its children, so a model shown this list without Form writes one anyway: it
// invents FormInput, FormSelect, TextAreaMulti and named arguments, and then the
// whole block parses to nothing. The Forms group is the shape a real answer
// takes when it asks the reader something, so it is taught with the rest.
const TAUGHT = [
  "Card", "CardHeader", "TextContent", "Callout", "Table", "Col", "BarChart", "LineChart", "PieChart",
  "Series", "Slice", "Steps", "StepsItem", "FollowUpBlock", "FollowUpItem", "Tag", "TagBlock",
  "CodeBlock", "Separator", "ListBlock", "ListItem",
  // Forms — these names exactly. There is no FormInput, FormSelect or FormField.
  "Form", "FormControl", "Label", "Input", "TextArea", "Select", "SelectItem", "DatePicker", "Slider",
  "CheckBoxGroup", "CheckBoxItem", "RadioGroup", "RadioItem", "SwitchGroup", "SwitchItem",
  "Chips", "ChipItem", "OptionCards", "OptionCard", "Button", "Buttons",
];

/**
 * The mistakes that actually made blocks draw nothing.
 *
 * `createLibrary(...).prompt()` writes its own §Forms section and its own rules
 * — but only from what the library spec is given, and this build hands it
 * `{ preamble: "", additionalRules: [] }`. So the language's own guidance
 * ("arguments are positional", "define one FormControl per field") never
 * reaches the model, while the leaky Card signature above does.
 */
const RULES = [
  "Write the block as statements, one per line, and start it with `root = `.",
  "Arguments are POSITIONAL, in the order the signature lists them. `Card([TextContent(\"hi\")])` is right; `Card(children=[TextContent(\"hi\")])` parses to nothing and draws nothing. Named arguments are never valid.",
  "Every statement except root must be referenced by another statement, or it is silently dropped.",
  "A form is three kinds of statement: the input, its FormControl label, and the Form that lists the controls. `who = Input(\"who\", \"Your name\", \"text\")`, `f1 = FormControl(\"Name\", who)`, `form = Form(\"myform\", Button(\"Send\"), [f1])`, then put `form` in the Card's children.",
  "Never invent a component name. The field parts are FormControl, Label, Input, TextArea, Select, SelectItem, DatePicker, Slider, CheckBoxGroup, CheckBoxItem, RadioGroup, RadioItem, SwitchGroup, SwitchItem, Chips, ChipItem, OptionCards, OptionCard, Button and Buttons — there is no FormInput, FormTextarea, TextAreaMulti or FormSelect.",
  "There are no methods on a statement. `Form.getFieldValue(\"x\")` does not exist. A field's value reaches an action through a `$binding` passed as the input's last argument, used inside `Action([@ToAssistant(\"...\")])`.",
  "Submitting is an action: `Button(\"Break it down\", Action([@ToAssistant(\"Help me break down this task\")]))`.",
];

export function richPrompt(): string {
  const comps = TAUGHT.map((k) => (openuiChatLibrary.components as Record<string, any>)[k]).filter(Boolean);
  return createLibrary({ components: comps, root: "Card" }).prompt({ preamble: "", additionalRules: RULES });
}

/**
 * What the parser said, in a sentence a reader can act on.
 *
 * The renderer reaches `if (!result?.root) return null` for every kind of
 * invalid block, so "the renderer produced nothing" was the only thing the
 * notice could say — true, and useless. The parser knows exactly what is wrong;
 * it was being thrown away into console.warn.
 */
function explain(errs: OpenUIError[]): string {
  // The renderer prepends a generic "produced no renderable root" whenever the
  // root is missing, so the first error is the least useful one. The specific
  // one is further down: pick it out by code.
  const find = (...codes: string[]) => errs.find((e) => codes.includes(e.code));
  const unknown = find("unknown-component");
  if (unknown) return `it used ${unknown.component ?? "a part"} — not a part this build has`;
  if (find("null-required", "excess-args")) return "it was written with named arguments, which this language does not take";
  if (errs.length === 0) return "the renderer produced nothing";
  const e = errs[errs.length - 1];
  if (e.code === "parse-failed" || e.code === "parse-exception") return "it is not valid openui-lang";
  return e.message;
}

/**
 * A ````openui` block the renderer cannot turn into UI.
 *
 * No source, ever. It is not prose and it is not the answer: showing it gave the
 * reader a wall of `Card(children: [...])` in the middle of a reply, and the
 * "Show source" toggle only made that opt-in rather than impossible. What is
 * left is one quiet line, and the reason is logged for whoever is teaching the
 * model the language.
 */
function FailedBlock({ source, why }: { source: string; why: string }) {
  useEffect(() => {
    console.warn("[openui] block not drawn:", why, "|", source.slice(0, 400));
  }, [source, why]);
  return (
    <div className="openui-failed my-3 rounded-lg border border-amber-500/25 bg-amber-500/[0.05] px-3 py-2 text-[13px] text-amber-700 dark:text-amber-400">
      <span aria-hidden>⚠</span> That answer had a diagram this build could not draw ({why}).
    </div>
  );
}

/** Catches a renderer that throws, so one bad block cannot take the reply down. */
class CatchRender extends Component<{ fallback: ReactNode; children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}

function useDark() {
  const get = () => document.documentElement.classList.contains("dark");
  const [dark, setDark] = useState(get);
  useEffect(() => {
    const o = new MutationObserver(() => setDark(get()));
    o.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => o.disconnect();
  }, []);
  return dark;
}

/**
 * One drawn block, with the two ways it can fail handled: it throws (caught
 * above) or it draws nothing at all (measured after paint). Neither is left to
 * leak the raw language into the answer.
 */
function OpenUIBlock({ source, dark, streaming }: { source: string; dark: boolean; streaming: boolean }) {
  const host = useRef<HTMLDivElement>(null);
  // The renderer's own errors, tagged with the source they belong to. Reset by
  // replacing the record, never by clearing it in an effect: the Renderer's
  // effects run before this component's, so a reset here would land after the
  // errors arrived and wipe them — which is exactly what it did.
  const errs = useRef<{ src: string; list: OpenUIError[] }>({ src: "", list: [] });
  const [why, setWhy] = useState<string | null>(null);
  useEffect(() => {
    setWhy(null);
    if (streaming) return;
    const el = host.current;
    if (!el) return;
    // One frame for the renderer to mount, then ask whether anything is there.
    const id = requestAnimationFrame(() => {
      if (el.childElementCount === 0) setWhy(explain(errs.current.src === source ? errs.current.list : []));
    });
    return () => cancelAnimationFrame(id);
  }, [source, streaming]);

  if (why) return <FailedBlock source={source} why={why} />;
  return (
    <div ref={host} className="openui-block my-3">
      <ThemeProvider mode={dark ? "dark" : "light"}>
        <Renderer
          response={source}
          library={openuiChatLibrary}
          isStreaming={streaming}
          onError={(e) => {
            errs.current = { src: source, list: e };
          }}
        />
      </ThemeProvider>
    </div>
  );
}

/**
 * OpenUI Lang written without the fence.
 *
 * The fenced case is handled below, but a small local model will often emit the
 * language bare — `root = Card(children: [...])` — and then the Markdown
 * renderer had no idea it was source, so the reader got a paragraph of
 * Python-looking noise in the middle of the reply.
 *
 * The answer is not to delete it: the model did say something, and it is
 * expressible as real UI. So the block gets the fence it was missing and the
 * normal OpenUI path draws it. Balanced-paren walk, so a multi-line call goes as
 * a unit; an expression that never closes is left alone rather than half-fenced.
 */
export function fenceBareOpenUI(text: string): string {
  if (!/^\s*root\s*=\s*[A-Za-z_]\w*\s*\(/m.test(text)) return text;
  const lines = text.split("\n");
  const out: string[] = [];
  // A block the model already fenced is left exactly as it is. Re-fencing it —
  // which is what this did — puts a second ```openui directly after the first,
  // so FENCE below closes on that line: the block parses as empty ("the
  // renderer produced nothing") and the rest of the program is then printed as
  // Markdown prose, which is the one outcome this file exists to prevent. It
  // fired on any correctly fenced block, which is to say on the common case.
  let fenced = false;
  for (let i = 0; i < lines.length; i++) {
    if (/^\s*```/.test(lines[i])) {
      fenced = !fenced;
      out.push(lines[i]);
      continue;
    }
    if (fenced || !/^\s*root\s*=\s*[A-Za-z_]\w*\s*\(/.test(lines[i])) {
      out.push(lines[i]);
      continue;
    }
    let depth = 0;
    let j = i;
    let closed = false;
    for (; j < lines.length; j++) {
      for (const ch of lines[j]) {
        if (ch === "(") depth++;
        else if (ch === ")") depth--;
      }
      if (depth <= 0) {
        closed = true;
        break;
      }
    }
    if (!closed) {
      out.push(lines[i]);
      continue;
    }
    out.push("```openui", ...lines.slice(i, j + 1), "```");
    i = j;
  }
  return out.join("\n");
}

export function RichText({ text, streaming, answered = false }: { text: string; streaming: boolean; answered?: boolean }) {
  const dark = useDark();
  const clean = fenceBareOpenUI(text);
  const fenced = clean.includes("```openui") || clean.includes("```ask") || clean.includes("```routine") || /\{\s*"(?:name|question)"\s*:/.test(clean);
  if (!fenced) return <Markdown streaming={streaming}>{clean}</Markdown>;
  const parts = splitFences(clean);
  return (
    <>
      {parts.map((p, i) => {
        if (p.kind === "md") {
          return p.text.trim() ? (
            <Markdown key={i} streaming={streaming && i === parts.length - 1}>
              {p.text}
            </Markdown>
          ) : null;
        }
        // A question or a proposal is only a control once its closing fence has
        // arrived; half a JSON object is not something to draw.
        if (p.open && p.tag !== "openui") return null;
        if (p.tag === "openui") {
          return (
            <CatchRender key={i} fallback={<FailedBlock source={p.body} why="the renderer rejected it" />}>
              <OpenUIBlock source={p.body} dark={dark} streaming={streaming && p.open} />
            </CatchRender>
          );
        }
        if (p.tag === "ask") {
          const spec = parseAsk(p.body);
          return spec ? <AskCard key={i} spec={spec} answered={answered} /> : <UnreadableBlock key={i} what="question" />;
        }
        const spec = parseRoutine(p.body);
        return spec ? <RoutineProposalCard key={i} spec={spec} /> : <UnreadableBlock key={i} what="routine" />;
      })}
    </>
  );
}

/** A fence that is not the JSON we expect: say so quietly, never show source. */
function UnreadableBlock({ what }: { what: string }) {
  return (
    <div className="my-2 text-[12px]/4 text-muted-foreground">
      That reply had a {what} this build could not read.
    </div>
  );
}

