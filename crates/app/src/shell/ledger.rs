//! A reply's steps as a ledger (board A, Charles 21 Nov): each thing the reply did is its own line
//! in the transcript, between the paragraphs it fell between. A running step breathes with a
//! silver sweep and counts its seconds; a finished one is one grey line with its duration.
//! Reasoning and a command's output fold open under their line.

use super::*;
use backspace_core::chat::{Act, Msg};
use gpui::{HighlightStyle, StyledText};
use gpui_kit::prelude::FluentBuilder;

/// What the transcript draws for a reply, in order.
#[derive(Debug, PartialEq, Clone)]
pub(super) enum Part {
    Text(String),
    Step(Step),
}

#[derive(Debug, PartialEq, Clone)]
pub(super) struct Step {
    /// Unique in the message: its fold state is kept under it.
    pub key: String,
    /// While it runs: "Reading b.rs".
    pub live: String,
    /// Once it ended: "Read b.rs", or "Read 3 files" when several in a row were merged.
    pub done: String,
    pub start: u64,
    pub end: Option<u64>,
    pub error: bool,
    /// What opens under the line: the reasoning, or a command and what it printed.
    pub detail: Option<String>,
    /// A command step's command, drawn in mono after "Ran" (Tuff UI: tool name and its argument).
    pub cmd: Option<String>,
    /// A command step's whole command and what it printed, for its shell block.
    pub shell: Option<(String, String)>,
    /// The file a Read or Edit step touched, whole, for its link.
    pub file: Option<String>,
    /// A tool's arguments, as JSON text, for its block (not for files, commands or reasoning).
    pub args: Option<String>,
    /// How many times the call was made in a row after failing: 1 unless it was retried.
    pub tries: usize,
}

fn base(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn field(input: &str, names: &[&str]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(input).ok()?;
    names.iter().find_map(|n| v.get(*n).and_then(|x| x.as_str()).map(str::to_string))
}

/// A shell call's command. Core cuts long inputs, so a cut JSON object still gives its command
/// (unescaped up to the cut) instead of showing as raw JSON.
pub(super) fn command_of(input: &str) -> Option<String> {
    if let Some(c) = field(input, &["command"]) {
        return Some(c);
    }
    let rest = &input[input.find("\"command\"")? + 9..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut it = rest.chars();
    while let Some(ch) = it.next() {
        match ch {
            '"' => break,
            '\\' => match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => break,
            },
            o => out.push(o),
        }
    }
    Some(out)
}

/// The short hash `git commit` printed ("[main de0b59f] msg").
pub(super) fn commit_hash(cmd: &str, out: &str) -> Option<String> {
    if !cmd.contains("git commit") {
        return None;
    }
    let l = out.lines().find(|l| l.trim_start().starts_with('['))?;
    let inside = l.trim_start().strip_prefix('[')?.split(']').next()?;
    inside.split_whitespace().last().filter(|h| (7..=12).contains(&h.len()) && h.chars().all(|c| c.is_ascii_hexdigit())).map(str::to_string)
}

/// A tool call's input as readable JSON; text that is not JSON (cut short) as it is.
fn pretty(input: &str) -> Option<String> {
    let t = input.trim();
    if t.is_empty() || t == "{}" {
        return None;
    }
    Some(serde_json::from_str::<serde_json::Value>(t).ok().and_then(|v| serde_json::to_string_pretty(&v).ok()).unwrap_or_else(|| t.to_string()))
}

fn short(s: &str, n: usize) -> String {
    let one = s.lines().next().unwrap_or("").trim();
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one.to_string()
    }
}

/// The kind a step merges under, with its plural for "Read 3 files".
/// A tool's name as the CLIs that capitalise it call it: OpenCode says "bash", Claude Code "Bash".
fn canon(name: &str) -> &str {
    const NAMES: [&str; 14] = ["Read", "Edit", "MultiEdit", "Write", "NotebookEdit", "Grep", "Glob", "LS", "WebSearch", "WebFetch", "Bash", "PowerShell", "Task", "TodoWrite"];
    match name {
        "list" => "LS",
        "patch" => "Edit",
        _ => NAMES.iter().find(|n| n.eq_ignore_ascii_case(name)).copied().unwrap_or(name),
    }
}

fn kind(name: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match canon(name) {
        "Read" => ("read", "Read", "files"),
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => ("edit", "Edited", "files"),
        "Grep" | "Glob" | "LS" => ("search", "Searched", "times"),
        "WebSearch" | "WebFetch" => ("web", "Searched the web", "times"),
        _ => return None,
    })
}

fn describe(a: &Act, thinking: Option<&str>) -> (String, String, Option<String>) {
    // Core records the path before it cuts the input; older steps only have what the cut kept.
    let file = a.path.clone().or_else(|| field(&a.input, &["file_path", "filePath", "path", "notebook_path"])).map(|p| base(&p));
    match canon(&a.name) {
        // Its own text; a thread from before Thoughts kept theirs has only the reply's, for the first.
        "Thinking" => ("Thinking".into(), "Thought".into(), Some(a.output.as_str()).filter(|t| !t.trim().is_empty()).or(thinking).filter(|t| !t.trim().is_empty()).map(str::to_string)),
        "Read" => {
            let f = file.unwrap_or_else(|| "a file".into());
            (format!("Reading {f}"), format!("Read {f}"), None)
        }
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => {
            let f = file.unwrap_or_else(|| "a file".into());
            (format!("Editing {f}"), format!("Edited {f}"), None)
        }
        "Bash" | "PowerShell" => {
            let cmd = command_of(&a.input).unwrap_or_else(|| a.input.clone());
            let out = a.output.trim_end();
            let detail = if cmd.trim().is_empty() && out.is_empty() { None } else if out.is_empty() { Some(format!("$ {}", cmd.trim())) } else { Some(format!("$ {}\n{}", cmd.trim(), out)) };
            (format!("Running {}", short(&cmd, 56)), format!("Ran {}", short(&cmd, 56)), detail)
        }
        "Grep" | "Glob" | "LS" => ("Searching the code".into(), "Searched the code".into(), None),
        "WebSearch" | "WebFetch" => ("Searching the web".into(), "Searched the web".into(), None),
        "Task" | "Agent" => ("Running a subagent".into(), "Ran a subagent".into(), None),
        "TodoWrite" => ("Planning".into(), "Planned".into(), None),
        // MCP tools arrive as mcp__server__tool: "linear: get_issue", as the steps summary says.
        n => match n.strip_prefix("mcp__").and_then(|r| r.split_once("__")) {
            Some((server, tool)) => (format!("Calling {server}: {tool}"), format!("Used {server}: {tool}"), None),
            None => (format!("Running {n}"), format!("Ran {n}"), None),
        },
    }
}

/// The reply as text and steps. Several finished steps of one kind in a row, with no text between,
/// become one line ("Read 3 files"). `running` is whether this reply is still being written: a step
/// with no end in a reply that is not running ended with it.
pub(super) fn parts(m: &Msg, running: bool) -> Vec<Part> {
    // The steps only split the text where they truly fell: a rewritten text (Replace) can leave an
    // offset in the middle of a character, and then the text goes after the steps.
    let splits = m.acts.iter().all(|a| a.at <= m.text.len() && m.text.is_char_boundary(a.at)) && m.acts.windows(2).all(|w| w[0].at <= w[1].at);
    let mut out: Vec<Part> = Vec::new();
    let mut prev = 0usize;
    let mut first_think = true;
    let mut merged: Option<(&'static str, usize, u64, u64)> = None;
    let push_text = |out: &mut Vec<Part>, s: &str| {
        let t = s.trim();
        if !t.is_empty() {
            out.push(Part::Text(t.to_string()));
        }
    };
    for (i, a) in m.acts.iter().enumerate() {
        let at = if splits { a.at } else { 0 };
        if at > prev {
            push_text(&mut out, &m.text[prev..at]);
            prev = at;
            merged = None;
        }
        let end = a.end.or_else(|| (!running).then_some(a.start));
        let (live, done, detail) = describe(a, if a.name == "Thinking" && first_think { Some(m.thinking.as_str()) } else { None });
        if a.name == "Thinking" {
            first_think = false;
        }
        // Fold into the step before it when both are finished, of one kind, with no text between.
        if let (Some(e), Some((k, n, s0, _)), Some((k2, label, _))) = (end, merged, kind(&a.name)) {
            if k == k2 && !a.error {
                if let Some(Part::Step(prev_step)) = out.last_mut() {
                    let total = n + 1;
                    let noun = kind(&a.name).map_or("times", |x| x.2);
                    prev_step.done = if k == "web" { format!("Searched the web {total} times") } else { format!("{label} {total} {noun}") };
                    prev_step.end = Some(e);
                    prev_step.args = None;
                    prev_step.start = s0;
                    merged = Some((k, total, s0, e));
                    continue;
                }
            }
        }
        merged = match (end, kind(&a.name)) {
            (Some(e), Some((k, _, _))) if !a.error => Some((k, 1, a.start, e)),
            _ => None,
        };
        let file = matches!(canon(&a.name), "Read" | "Edit" | "MultiEdit" | "Write" | "NotebookEdit").then(|| a.path.clone().or_else(|| field(&a.input, &["file_path", "filePath", "path", "notebook_path"]))).flatten();
        let full = matches!(canon(&a.name), "Bash" | "PowerShell").then(|| command_of(&a.input)).flatten().filter(|c| !c.trim().is_empty());
        let args = if file.is_none() && full.is_none() && a.name != "Thinking" { pretty(&a.input) } else { None };
        let step = Step { key: format!("{}:{i}", m.id), live, done, start: a.start, end, error: a.error, detail, cmd: full.as_deref().map(|c| short(c, 56)), shell: full.map(|c| (c.trim().to_string(), a.output.trim_end().to_string())), args, file, tries: 1 };
        // A call made again right after it failed (the same tool on the same thing, nothing
        // between) is one line that counts its tries, not a line per attempt (Charles, 23 Nov).
        if let Some(Part::Step(p)) = out.last_mut() {
            if p.error && p.done == step.done && p.cmd == step.cmd {
                let (key, start, tries) = (p.key.clone(), p.start, p.tries + 1);
                *p = Step { key, start, tries, ..step };
                continue;
            }
        }
        out.push(Part::Step(step));
    }
    push_text(&mut out, &m.text[prev.min(m.text.len())..]);
    out
}

/// The live card's clock: "0:07", "1:04".
pub(super) fn clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// "3.2s" under ten seconds, "42s", "1m 04s".
pub(super) fn dur(ms: u64) -> String {
    let s = ms / 1000;
    if ms < 10_000 {
        format!("{:.1}s", ms as f32 / 1000.0)
    } else if s < 60 {
        format!("{s}s")
    } else {
        format!("{}m {:02}s", s / 60, s % 60)
    }
}

pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// `text` with a silver sweep running through it: each letter is mixed from the grey toward the
/// bright ink by how near the sweep is, 1.6 s a pass, on top of a slow pulse of the whole line
/// (2.4 s a breath) so it reads as alive even between sweeps. A still frame (reduced motion) is
/// plain grey.
pub(super) fn shimmer(text: &str, now: u64, grey: Hsla, ink: Hsla, still: bool) -> StyledText {
    let n = text.chars().count().max(1) as f32;
    let width = 5.0_f32;
    let phase = (now % 1600) as f32 / 1600.0;
    let centre = phase * (n + 2.0 * width) - width;
    let breath = 0.5 - 0.5 * ((now % 2400) as f32 / 2400.0 * std::f32::consts::TAU).cos();
    let mut hl = Vec::with_capacity(text.len());
    if !still {
        for (k, (i, ch)) in text.char_indices().enumerate() {
            let near = (1.0 - ((k as f32 - centre).abs() / width)).clamp(0.0, 1.0);
            let t = (near * near * (3.0 - 2.0 * near)).max(0.45 * breath);
            if t > 0.02 {
                let col = Hsla { h: grey.h, s: grey.s + (ink.s - grey.s) * t, l: grey.l + (ink.l - grey.l) * t, a: grey.a + (ink.a - grey.a) * t };
                hl.push((i..i + ch.len_utf8(), HighlightStyle { color: Some(col), ..Default::default() }));
            }
        }
    }
    StyledText::new(text.to_string()).with_highlights(hl)
}

impl Cauldron {
    /// One step's line, and under it, when open, its detail. `live` is whether it is running now.
    pub(super) fn step_el(&self, s: &Step, live: bool, now: u64, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        // A finished command is its terminal card, shut (a failure opens on what it printed).
        if let (false, Some((cmd, out))) = (live, &s.shell) {
            let open = self.act_open.get(&s.key).copied().unwrap_or(s.error);
            return self.shell_block(&s.key, cmd, out, false, s.error, s.start, s.end, Some(open), s.tries, pal, cx);
        }
        // A finished tool call is one card: its header is the line, the arguments open under it
        // (Charles, 23 Nov: "just the top of the card ... when you expand it, the arguments").
        if let (false, Some(args), None) = (live, &s.args, &s.detail) {
            let open = self.act_open.get(&s.key).copied().unwrap_or(false);
            return self.tool_block(&s.key, &s.done, args, false, s.error, s.start, s.end, s.tries, Some(open), pal, cx);
        }
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let still = cx.reduce_motion();
        let ended = s.end.filter(|_| !live);
        // A running command opens to nothing yet (its output comes when it ends), so it starts shut.
        let open = self.act_open.get(&s.key).copied().unwrap_or(live && s.shell.is_none());
        let foldable = s.detail.is_some() || s.args.is_some();
        let label = if live { s.live.clone() } else { s.done.clone() };
        let label = if s.tries > 1 { format!("{label} · {} tries", s.tries) } else { label };
        let time = match ended {
            Some(e) if e > s.start => Some(dur(e - s.start)),
            None if live => Some(dur(now.saturating_sub(s.start))),
            _ => None,
        };
        let key = s.key.clone();
        let is_cmd = s.shell.is_some();
        // The status stays on the left (a failed command keeps its X); a foldable step's chevron is
        // after its time, as on Tuff UI's tool rows.
        let glyph_el: AnyElement = if live {
            // A breathing dot, in step with the sweep.
            let breath = if still { 1.0 } else { 0.35 + 0.65 * (((now % 1100) as f32 / 1100.0) * std::f32::consts::TAU).sin().abs() };
            // fg2, not the accent: the accent means "this wants you" (design system).
            div().size(px(7.)).rounded_full().bg(c(pal.fg2)).opacity(breath).into_any_element()
        } else if s.error {
            glyph("x", 13., c(pal.bad)).into_any_element()
        } else {
            glyph("check", 13., c(pal.ok)).into_any_element()
        };
        let text_el: AnyElement = match &s.cmd {
            // A command: the verb, then the command itself in mono (Tuff UI: tool name and argument).
            Some(cmd) => {
                let verb = if live { "Running" } else { "Ran" };
                div()
                    .flex()
                    .gap(px(8.))
                    .child(div().flex_none().child(if live { shimmer(verb, now, fg3, fg, still).into_any_element() } else { verb.into_any_element() }))
                    .child(div().min_w_0().overflow_hidden().text_ellipsis().font_family(MONO).text_size(px(11.5)).text_color(if live { fg } else { fg3.opacity(0.85) }).child(cmd.clone()))
                    .into_any_element()
            }
            None if live => shimmer(&label, now, fg3, fg, still).into_any_element(),
            // A picture it made or touched reads as a link to its preview (Charles, 22 Nov).
            None if s.file.as_deref().is_some_and(|f| super::imageview::is_image(std::path::Path::new(f))) => {
                let name = base(s.file.as_deref().unwrap_or_default());
                let verb = label.strip_suffix(name.as_str()).unwrap_or(&label).trim_end().to_string();
                let acc = c(pal.accent);
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(verb)
                    .child(div().id(format!("step-image-{key}")).flex().items_center().gap(px(4.)).text_color(acc).hover(|d| d.underline()).aria_label(format!("Preview {name}")).child(glyph("image", 12., acc)).child(name).test_support())
                    .into_any_element()
            }
            None => div().child(label.clone()).into_any_element(),
        };
        let row = div()
            .id(format!("step-{key}"))
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(20.))
            .text_size(px(12.5))
            .text_color(fg3)
            .aria_label(match &time {
                Some(t) => format!("{label}, {t}"),
                None => label.clone(),
            })
            .child(div().size(px(16.)).flex_none().flex().items_center().justify_center().child(glyph_el))
            .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(text_el))
            .when_some(time.filter(|_| !live), |d, t| d.child(div().ml_auto().flex_none().font_family(MONO).font_features(tnum()).text_size(px(11.5)).child(t)))
            // The live card (board "Stream tool blocks" A): the step running now is a card with
            // its clock and a Stop for the stream.
            .when(live, |d| {
                d.h(px(40.))
                    .pl(px(8.))
                    .pr(px(6.))
                    .rounded(px(radius::INSET))
                    .bg(super::well_fill(false, pal, 1.0))
                    .border_1()
                    .border_color(c(pal.line_strong))
                    .shadow(super::lift(super::Lift::Well, pal))
                    .text_color(c(pal.fg2))
                    .child(div().ml_auto().flex_none().font_family(MONO).font_features(tnum()).text_size(px(11.5)).text_color(fg3).child(clock(now.saturating_sub(s.start))))
                    .child(
                        div()
                            .id(format!("step-stop-{key}"))
                            .flex_none()
                            .h(px(26.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .rounded(px(radius::INNER))
                            .border_1()
                            .border_color(c(pal.line_strong))
                            .bg(c(pal.panel))
                            .text_size(px(12.))
                            .text_color(fg)
                            .cursor_pointer()
                            .hover(|d| d.bg(c(pal.hover)))
                            .aria_label("Stop")
                            .child(div().size(px(8.)).rounded(px(2.)).bg(c(pal.fg2)))
                            .child("Stop")
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                if let (Some(id), Some((_, chats))) = (this.open.clone(), this.core.as_ref()) {
                                    chats.stop(&id);
                                }
                                this.refresh();
                                cx.notify();
                            }))
                            .test_support(),
                    )
            })
            .when_live(foldable, |d| d.child(div().w(px(14.)).flex_none().flex().items_center().justify_center().child(glyph(if open { "chevron-down" } else { "chevron-right" }, 10., fg3))))
            // A step that touched a file links to it (Charles, 22 Nov): a click shows it in the
            // card, a right click offers where.
            .when_some(s.file.clone().filter(|_| !foldable && !live), |d, f| {
                let p = std::path::PathBuf::from(&f);
                let file = if p.is_absolute() { p } else { self.thread.as_ref().and_then(|t| t.project.as_ref()).map_or(p.clone(), |d| std::path::Path::new(d).join(&p)) };
                let (a, b) = (file.clone(), file);
                d.cursor_pointer()
                    .hover(|d| d.text_color(fg))
                    .on_click(cx.listener(move |this, _, _, cx| this.open_link(super::links::Link::File(a.clone()), true, cx)))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.open_link_menu(super::links::Link::File(b.clone()), ev.position, cx)))
            })
            .when_live(foldable, |d| {
                let k = key.clone();
                d.cursor_pointer().hover(|d| d.text_color(fg)).on_click(cx.listener(move |this, _, _, cx| {
                    let cur = this.act_open.get(&k).copied();
                    // The first click flips what it showed: a running step was open, a finished one shut.
                    let was = cur.unwrap_or(live && is_cmd);
                    this.act_open.insert(k.clone(), !was);
                    cx.notify();
                }))
            });
        let mut col = div().flex().flex_col().child(row.test_support());
        if open {
            if let Some((cmd, out)) = &s.shell {
                col = col.child(self.shell_block(&key, cmd, out, live, s.error, s.start, s.end, None, s.tries, pal, cx));
            } else if let (Some(args), None) = (&s.args, &s.detail) {
                col = col.child(self.tool_block(&key, if live { &s.live } else { &s.done }, args, live, s.error, s.start, s.end, s.tries, None, pal, cx));
            } else if let Some(d) = &s.detail {
                let code = d.starts_with("$ ");
                col = col.child(
                    div()
                        .id(format!("step-detail-{key}"))
                        .ml(px(24.))
                        .my(px(4.))
                        .px(px(10.))
                        .py(px(6.))
                        .max_h(px(220.))
                        .overflow_y_scroll()
                        .rounded(px(radius::CONTROL))
                        .bg(c(pal.panel))
                        .border_1()
                        .border_color(c(pal.line_strong))
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .text_color(c(pal.fg2))
                        .relative()
                        .when(code, |d| d.font_family(MONO))
                        .child(div().pr(px(40.)).child(d.clone()))
                        .child(div().absolute().top(px(6.)).right(px(10.)).child(copy_button(format!("step-copy-{key}"), d.clone(), pal, cx)))
                        .test_support(),
                );
            }
        }
        col.into_any_element()
    }

    /// A tool call as one card (Tuff UI's tool call block, compact): a 30px header with the tool,
    /// its status and time, and a chevron; the arguments open under it. `fold` is Some(open) for a
    /// finished call, which stands as its own card and opens on a click; None under a running
    /// step's line, where it is always open. A step cannot be re-run on its own (the CLI owns the
    /// call), so there is no Retry here.
    #[allow(clippy::too_many_arguments)]
    fn tool_block(&self, key: &str, title: &str, args: &str, live: bool, error: bool, start: u64, end: Option<u64>, tries: usize, fold: Option<bool>, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let shown = fold != Some(false);
        let (word, icon, ink, fill) = if live {
            ("Running", None, c(pal.fg2), c(pal.selected))
        } else if error {
            ("Failed", Some("x"), c(pal.bad), ca(pal.bad, 0.12))
        } else {
            ("Done", Some("check"), c(pal.fg2), c(pal.selected))
        };
        let took = if live { Some(now_ms().saturating_sub(start)) } else { end.map(|e| e.saturating_sub(start)) };
        let mut word = match took {
            Some(ms) => format!("{word} · {}", dur(ms)),
            None => word.to_string(),
        };
        if tries > 1 {
            word = format!("{word} · {tries} tries");
        }
        // The arguments' first values, on the header, so a shut card still says what it was asked.
        let gist = serde_json::from_str::<serde_json::Value>(args).ok().and_then(|v| v.as_object().map(|o| o.values().filter_map(|x| x.as_str().map(str::to_string).or_else(|| (x.is_number() || x.is_boolean()).then(|| x.to_string()))).take(2).collect::<Vec<_>>().join(" · "))).filter(|g| !g.is_empty());
        let hair = c(pal.line);
        let k = key.to_string();
        super::surface::block(pal)
            .id(format!("tool-{key}"))
            .when(fold.is_none(), |d| d.ml(px(24.)))
            .my(px(4.))
            .overflow_hidden()
            .text_size(px(12.))
            .aria_label(format!("{title}, {word}"))
            .child(
                div()
                    .id(format!("tool-head-{key}"))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(30.))
                    .px(px(10.))
                    .when(shown, |d| d.border_b_1().border_color(hair))
                    .when_some(fold, |d, open| {
                        d.cursor_pointer().hover(|d| d.bg(c(pal.hover))).on_click(cx.listener(move |this, _, _, cx| {
                            this.act_open.insert(k.clone(), !open);
                            cx.notify();
                        }))
                    })
                    .child(glyph("wrench", 12., fg3))
                    .child(div().flex_none().text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(fg).child(title.to_string()))
                    .child(div().min_w_0().flex_1().overflow_hidden().whitespace_nowrap().text_ellipsis().font_family(MONO).text_size(px(11.)).text_color(fg3).children(gist))
                    .child(
                        div()
                            .flex_none()
                            .h(px(18.))
                            .px(px(7.))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .rounded(px(radius::TAG))
                            .bg(fill)
                            .text_size(px(10.5))
                            .font_weight(FontWeight::MEDIUM)
                            .font_features(tnum())
                            .text_color(ink)
                            .when_some(icon, |d, i| d.child(glyph(i, 10., ink)))
                            .child(word.clone()),
                    )
                    .when_some(fold, |d, open| d.child(div().flex_none().child(glyph(if open { "chevron-down" } else { "chevron-right" }, 10., fg3)))),
            )
            .when(shown, |d| {
                d.child(
                    div()
                        .px(px(10.))
                        .py(px(8.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(div().text_size(px(10.5)).text_color(fg3).child("Arguments"))
                        .child(
                            super::surface::inset(pal)
                                .id(format!("tool-args-{key}"))
                                .max_h(px(180.))
                                .overflow_y_scroll()
                                .font_family(MONO)
                                .text_size(px(11.5))
                                .line_height(px(17.))
                                .text_color(c(pal.fg2))
                                .child(args.to_string()),
                        )
                        .child(div().flex().justify_end().child(copy_button(format!("tool-copy-{key}"), args.to_string(), pal, cx))),
                )
            })
            .test_support()
            .into_any_element()
    }

    /// An opened command (Tuff UI's shell block): `$ command` and its state, what it printed, and
    /// a foot with the line count and Copy.
    /// A command as Tuff UI's terminal output: `$ cmd`, its status and time, then what it printed.
    /// `fold` is Some(open) for a finished command, which stands as its own card (no "Ran" line)
    /// and opens on a click; None under a running step's line.
    #[allow(clippy::too_many_arguments)]
    fn shell_block(&self, key: &str, cmd: &str, out: &str, live: bool, error: bool, start: u64, end: Option<u64>, fold: Option<bool>, tries: usize, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let shown = fold != Some(false);
        let hash = commit_hash(cmd, out);
        // Status is a glyph and a word; only a failure takes a colour (design system).
        let (word, icon, ink, fill) = if live {
            ("Running", None, c(pal.fg2), c(pal.selected))
        } else if error {
            ("Failed", Some("x"), c(pal.bad), ca(pal.bad, 0.12))
        } else {
            ("Done", Some("check"), c(pal.fg2), c(pal.selected))
        };
        // Tuff's header counts up while it runs and keeps the figure once it ends.
        let took = if live { Some(now_ms().saturating_sub(start)) } else { end.map(|e| e.saturating_sub(start)) };
        let word = match took {
            Some(ms) => format!("{word} · {}", dur(ms)),
            None => word.to_string(),
        };
        // A failed command says how it ended when its output does ("Exit code 1", "exit status 2").
        let word = match (error && !live).then(|| exit_code(out)).flatten() {
            Some(n) => format!("{word} · exit {n}"),
            None => word,
        };
        // Retried after failing: say how many times, in the same chip (one card, not one per try).
        let word = if tries > 1 { format!("{word} · {tries} tries") } else { word };
        let n = out.lines().count();
        let hair = c(pal.line);
        let k = key.to_string();
        super::surface::block(pal)
            .id(format!("shell-{key}"))
            .when(fold.is_none(), |d| d.ml(px(24.)))
            .my(px(4.))
            .overflow_hidden()
            .text_size(px(12.))
            .aria_label(format!("$ {cmd}, {word}"))
            .child(
                div()
                    .id(format!("shell-head-{key}"))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(30.))
                    .px(px(10.))
                    .when(shown, |d| d.border_b_1().border_color(hair))
                    .when_some(fold, |d, open| {
                        d.cursor_pointer().hover(|d| d.bg(c(pal.hover))).on_click(cx.listener(move |this, _, _, cx| {
                            this.act_open.insert(k.clone(), !open);
                            cx.notify();
                        }))
                    })
                    .child(glyph("terminal", 12., fg3))
                    .child(div().min_w_0().flex_1().overflow_hidden().whitespace_nowrap().text_ellipsis().font_family(MONO).text_size(px(11.5)).text_color(fg).child(format!("$ {}", cmd.lines().next().unwrap_or(""))))
                    // A commit says which (Tuff's Tag: mono, for hashes and branches).
                    .when_some(hash, |d, h| {
                        d.child(div().flex_none().h(px(18.)).px(px(6.)).flex().items_center().gap(px(4.)).rounded(px(radius::TAG)).border_1().border_color(hair).font_family(MONO).text_size(px(10.5)).text_color(c(pal.fg2)).child(glyph("git-commit-horizontal", 11., c(pal.fg2))).child(h))
                    })
                    .child(
                        div()
                            .flex_none()
                            .h(px(18.))
                            .px(px(7.))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .rounded(px(radius::TAG))
                            .bg(fill)
                            .text_size(px(10.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ink)
                            .font_features(tnum())
                            .when_some(icon, |d, i| d.child(glyph(i, 10., ink)))
                            .child(word),
                    )
                    // Stop, while it runs: it ends the whole turn, as the composer's Stop does.
                    .when_live(live, |d| {
                        d.child(
                            div()
                                .id(format!("shell-stop-{key}"))
                                .flex_none()
                                .h(px(18.))
                                .px(px(7.))
                                .flex()
                                .items_center()
                                .rounded(px(radius::TAG))
                                .text_size(px(10.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c(pal.fg2))
                                .bg(c(pal.selected))
                                .cursor_pointer()
                                .hover(move |d| d.text_color(fg))
                                .aria_label("Stop")
                                .child("Stop")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    if let (Some(id), Some((_, chats))) = (this.open.clone(), this.core.as_ref()) {
                                        chats.stop(&id);
                                    }
                                    this.refresh();
                                    cx.notify();
                                })),
                        )
                    })
                    .when_some(fold, |d, open| d.child(div().flex_none().child(glyph(if open { "chevron-down" } else { "chevron-right" }, 10., fg3)))),
            )
            .when(shown && !out.is_empty(), |d| {
                // The CLIs give one stream, so errors are told apart by how the line starts.
                let mut lines = div().id(format!("shell-out-{key}")).max_h(px(280.)).overflow_y_scroll().px(px(10.)).py(px(8.)).font_family(MONO).text_size(px(11.5)).line_height(px(17.)).text_color(fg3);
                for l in out.lines().take(1000) {
                    let ink = match line_kind(l) {
                        Some(true) => Some(c(pal.bad)),
                        Some(false) => Some(c(pal.warn)),
                        None => None,
                    };
                    lines = lines.child(div().min_h(px(17.)).whitespace_nowrap().when_some(ink, |d, k| d.text_color(k)).child(l.to_string()));
                }
                d.child(lines)
            })
            .when(shown, |d| d.child(
                div()
                    .flex()
                    .items_center()
                    .h(px(26.))
                    .px(px(10.))
                    .when(!out.is_empty(), |d| d.border_t_1().border_color(hair))
                    .text_size(px(11.))
                    .text_color(fg3)
                    .font_features(tnum())
                    .child(match (n, live) {
                        (0, true) => "Waiting for output…".to_string(),
                        (0, false) => "No output".to_string(),
                        (1, _) => "1 line".to_string(),
                        (n, _) => format!("{n} lines"),
                    })
                    .child(div().ml_auto().child(copy_button(format!("shell-copy-{key}"), if out.is_empty() { cmd.to_string() } else { out.to_string() }, pal, cx))),
            ))
            .test_support()
            .into_any_element()
    }
}

/// The exit code a command's output ends with, when it says one.
fn exit_code(out: &str) -> Option<i32> {
    out.lines().rev().find_map(|l| {
        let l = l.to_lowercase();
        let i = l.find("exit code").map(|i| i + 9).or_else(|| l.find("exit status").map(|i| i + 11))?;
        l[i..].trim_start_matches([' ', ':', '=']).split(|c: char| !c.is_ascii_digit() && c != '-').next()?.parse().ok()
    })
}

/// Whether an output line reads as an error (Some(true)) or a warning (Some(false)).
fn line_kind(l: &str) -> Option<bool> {
    let t = l.trim_start().to_lowercase();
    if ["error", "fatal", "panic", "npm err", "traceback", "e:", "failed", "✗"].iter().any(|p| t.starts_with(p)) {
        Some(true)
    } else if t.starts_with("warn") {
        Some(false)
    } else {
        None
    }
}

/// "Copy", which puts `text` on the clipboard.
fn copy_button(id: String, text: String, pal: &'static Palette, cx: &mut Context<Cauldron>) -> impl IntoElement {
    let fg = c(pal.fg);
    div()
        .id(id)
        .text_size(px(11.))
        .text_color(c(pal.fg2))
        .cursor_pointer()
        .hover(move |d| d.text_color(fg))
        .aria_label("Copy")
        .child("Copy")
        .on_click(cx.listener(move |_, _, _, cx| {
            cx.stop_propagation();
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use backspace_core::chat::{Act, Msg, Role, Status};

    fn msg(text: &str, acts: Vec<Act>) -> Msg {
        let mut m: Msg = serde_json::from_value(serde_json::json!({ "id": "m", "role": "assistant", "text": text, "at": 1, "status": "done" })).unwrap();
        m.acts = acts;
        m.role = Role::Assistant;
        m.status = Status::Done;
        m
    }
    fn act(name: &str, input: &str, at: usize, start: u64, end: Option<u64>) -> Act {
        Act { id: format!("{name}{start}"), name: name.into(), input: input.into(), start, end, at, ..Default::default() }
    }

    #[::core::prelude::v1::test]
    fn steps_sit_between_the_paragraphs_they_fell_between() {
        let m = msg("Looking.\n\nFound it.", vec![act("Read", r#"{"file_path":"/a/b.rs"}"#, 10, 1000, Some(1500))]);
        let p = parts(&m, false);
        assert_eq!(p.len(), 3);
        assert!(matches!(&p[0], Part::Text(t) if t == "Looking."));
        assert!(matches!(&p[1], Part::Step(s) if s.done == "Read b.rs" && s.live == "Reading b.rs"));
        assert!(matches!(&p[2], Part::Text(t) if t == "Found it."));
    }

    #[::core::prelude::v1::test]
    fn lower_case_tool_names_get_the_same_lines() {
        // OpenCode names its tools "bash", "write", "read".
        let m = msg("done", vec![act("bash", r#"{"command":"ls -la"}"#, 0, 0, Some(100)), act("write", r#"{"filePath":"C:/x/index.html"}"#, 0, 200, Some(300))]);
        let p = parts(&m, false);
        assert!(matches!(&p[0], Part::Step(s) if s.cmd.as_deref() == Some("ls -la") && s.done == "Ran ls -la"), "{p:?}");
        assert!(matches!(&p[1], Part::Step(s) if s.done == "Edited index.html"), "{p:?}");
    }

    #[::core::prelude::v1::test]
    fn a_write_names_its_file_from_the_recorded_path() {
        // The stored input was cut before the path; core kept the path on the step.
        let mut w = act("Write", r#"{"content":"<html><p>lorem"#, 0, 0, Some(100));
        w.path = Some("/w/landing/index.html".into());
        let p = parts(&msg("ok", vec![w]), false);
        assert!(matches!(&p[0], Part::Step(s) if s.done == "Edited index.html"), "{p:?}");
    }

    #[::core::prelude::v1::test]
    fn a_command_carries_its_whole_line_and_output_for_the_block() {
        let long = format!("cargo test --workspace {}", "-p x ".repeat(20));
        let mut b = act("Bash", &serde_json::json!({ "command": long }).to_string(), 0, 0, Some(100));
        b.output = "ok\nok\n".into();
        let p = parts(&msg("ok", vec![b]), false);
        let Part::Step(s) = &p[0] else { panic!() };
        assert!(s.cmd.as_deref().unwrap().ends_with('…'));
        assert_eq!(s.shell.as_ref().map(|(c, o)| (c.as_str(), o.as_str())), Some((long.trim(), "ok\nok")));
        assert_eq!(clock(64_000), "1:04");
    }

    #[::core::prelude::v1::test]
    fn finished_steps_of_one_kind_in_a_row_merge() {
        let r = |n: u64| act("Read", &format!(r#"{{"file_path":"f{n}.rs"}}"#), 0, n * 100, Some(n * 100 + 50));
        let m = msg("done", vec![r(1), r(2), r(3), act("Bash", r#"{"command":"cargo test"}"#, 0, 900, Some(1900))]);
        let p = parts(&m, false);
        assert!(matches!(&p[0], Part::Step(s) if s.done == "Read 3 files"), "{p:?}");
        assert!(matches!(&p[1], Part::Step(s) if s.done == "Ran cargo test"));
        // A running step is never merged, and a reply that stopped running ends its open steps.
        let live = msg("", vec![r(1), act("Read", "{}", 0, 400, None)]);
        assert_eq!(parts(&live, true).len(), 2);
        assert!(matches!(&parts(&live, false)[0], Part::Step(s) if s.end == Some(400) && s.done == "Read 2 files"));
    }

    /// Catches: a call retried after failing drawing one line per attempt (Charles, 23 Nov), or
    /// two different failed calls being merged as if they were retries of one.
    #[::core::prelude::v1::test]
    fn a_failed_call_tried_again_is_one_line_counting_its_tries() {
        let fail = |start: u64| Act { error: true, ..act("Bash", r#"{"command":"npm test"}"#, 0, start, Some(start + 50)) };
        let m = msg("done", vec![fail(0), fail(100), act("Bash", r#"{"command":"npm test"}"#, 0, 200, Some(250)), fail(300)]);
        let p = parts(&m, false);
        // Two failures then a success: one line, three tries, ended well; a failure after a
        // success is a new attempt of its own.
        assert_eq!(p.iter().filter(|x| matches!(x, Part::Step(_))).count(), 2, "{p:?}");
        assert!(matches!(&p[0], Part::Step(s) if s.tries == 3 && !s.error && s.start == 0 && s.end == Some(250)), "{p:?}");
        let other = Act { error: true, ..act("Bash", r#"{"command":"npm run lint"}"#, 0, 400, Some(450)) };
        assert_eq!(parts(&msg("done", vec![fail(0), other]), false).iter().filter(|x| matches!(x, Part::Step(_))).count(), 2, "different commands stay apart");
    }

    #[::core::prelude::v1::test]
    fn thinking_and_commands_carry_a_detail_to_fold() {
        let mut m = msg("ok", vec![act("Thinking", "", 0, 0, Some(3000)), act("Bash", r#"{"command":"ls"}"#, 0, 3000, Some(3500))]);
        m.thinking = "The cause is the order.".into();
        m.acts[1].output = "a\nb".into();
        let p = parts(&m, false);
        assert!(matches!(&p[0], Part::Step(s) if s.detail.as_deref() == Some("The cause is the order.")));
        assert!(matches!(&p[1], Part::Step(s) if s.detail.as_deref() == Some("$ ls\na\nb")));
    }

    #[::core::prelude::v1::test]
    fn every_thought_in_a_reply_opens() {
        let mut t1 = act("Thinking", "", 0, 0, Some(1000));
        t1.output = "First.".into();
        let mut t2 = act("Thinking", "", 0, 2000, Some(3000));
        t2.output = "Second.".into();
        let m = msg("ok", vec![t1, act("Bash", r#"{"command":"ls"}"#, 0, 1000, Some(2000)), t2]);
        let p = parts(&m, false);
        assert!(matches!(&p[0], Part::Step(s) if s.detail.as_deref() == Some("First.")));
        assert!(matches!(&p[2], Part::Step(s) if s.detail.as_deref() == Some("Second.")), "{p:?}");
    }

    #[::core::prelude::v1::test]
    fn exit_codes_and_error_lines_are_read_from_the_output() {
        assert_eq!(exit_code("a\nError: Exit code 2\n"), Some(2));
        assert_eq!(exit_code("exit status 127"), Some(127));
        assert_eq!(exit_code("fine"), None);
        assert_eq!(line_kind("  error[E0382]: moved"), Some(true));
        assert_eq!(line_kind("warning: unused"), Some(false));
        assert_eq!(line_kind("compiling"), None);
    }

    #[::core::prelude::v1::test]
    fn durations_read_like_a_stopwatch() {
        assert_eq!(dur(3200), "3.2s");
        assert_eq!(dur(42_000), "42s");
        assert_eq!(dur(64_000), "1m 04s");
    }
}
