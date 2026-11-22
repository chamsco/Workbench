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
}

fn base(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn field(input: &str, names: &[&str]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(input).ok()?;
    names.iter().find_map(|n| v.get(*n).and_then(|x| x.as_str()).map(str::to_string))
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
            let cmd = field(&a.input, &["command"]).unwrap_or_else(|| a.input.clone());
            let out = a.output.trim_end();
            let detail = if cmd.trim().is_empty() && out.is_empty() { None } else if out.is_empty() { Some(format!("$ {}", cmd.trim())) } else { Some(format!("$ {}\n{}", cmd.trim(), out)) };
            (format!("Running {}", short(&cmd, 56)), format!("Ran {}", short(&cmd, 56)), detail)
        }
        "Grep" | "Glob" | "LS" => ("Searching the code".into(), "Searched the code".into(), None),
        "WebSearch" | "WebFetch" => ("Searching the web".into(), "Searched the web".into(), None),
        "Task" | "Agent" => ("Running a subagent".into(), "Ran a subagent".into(), None),
        "TodoWrite" => ("Planning".into(), "Planned".into(), None),
        n => (format!("Running {n}"), format!("Ran {n}"), None),
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
        let full = matches!(canon(&a.name), "Bash" | "PowerShell").then(|| field(&a.input, &["command"])).flatten().filter(|c| !c.trim().is_empty());
        out.push(Part::Step(Step { key: format!("{}:{i}", m.id), live, done, start: a.start, end, error: a.error, detail, cmd: full.as_deref().map(|c| short(c, 56)), shell: full.map(|c| (c.trim().to_string(), a.output.trim_end().to_string())), file }));
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
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let still = cx.reduce_motion();
        let ended = s.end.filter(|_| !live);
        // A running command opens to nothing yet (its output comes when it ends), so it starts shut.
        let open = self.act_open.get(&s.key).copied().unwrap_or(live && s.shell.is_none());
        let foldable = s.detail.is_some();
        let label = if live { s.live.clone() } else { s.done.clone() };
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
                    .bg(c(pal.inset))
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
                col = col.child(self.shell_block(&key, cmd, out, live, s.error, pal, cx));
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
                        .bg(c(pal.inset))
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .text_color(c(pal.fg3))
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

    /// An opened command (Tuff UI's shell block): `$ command` and its state, what it printed, and
    /// a foot with the line count and Copy.
    fn shell_block(&self, key: &str, cmd: &str, out: &str, live: bool, error: bool, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        // Status is a glyph and a word; only a failure takes a colour (design system).
        let (word, icon, ink, fill) = if live {
            ("Running", None, c(pal.fg2), c(pal.selected))
        } else if error {
            ("Failed", Some("x"), c(pal.bad), ca(pal.bad, 0.12))
        } else {
            ("Done", Some("check"), c(pal.fg2), c(pal.selected))
        };
        let n = out.lines().count();
        let hair = c(pal.line);
        div()
            .id(format!("shell-{key}"))
            .ml(px(24.))
            .my(px(4.))
            .rounded(px(radius::INSET))
            .bg(c(pal.inset))
            .overflow_hidden()
            .text_size(px(12.))
            .aria_label(format!("$ {cmd}, {word}"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(30.))
                    .px(px(10.))
                    .border_b_1()
                    .border_color(hair)
                    .child(div().min_w_0().flex_1().overflow_hidden().whitespace_nowrap().text_ellipsis().font_family(MONO).text_size(px(11.5)).text_color(fg).child(format!("$ {}", cmd.lines().next().unwrap_or(""))))
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
                            .when_some(icon, |d, i| d.child(glyph(i, 10., ink)))
                            .child(word),
                    ),
            )
            .when(!out.is_empty(), |d| {
                d.child(div().id(format!("shell-out-{key}")).max_h(px(220.)).overflow_y_scroll().px(px(10.)).py(px(8.)).font_family(MONO).text_size(px(11.5)).line_height(px(17.)).text_color(fg3).child(out.to_string()))
            })
            .child(
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
                        (0, true) => "No output yet".to_string(),
                        (0, false) => "No output".to_string(),
                        (1, _) => "1 line".to_string(),
                        (n, _) => format!("{n} lines"),
                    })
                    .child(div().ml_auto().child(copy_button(format!("shell-copy-{key}"), if out.is_empty() { cmd.to_string() } else { out.to_string() }, pal, cx))),
            )
            .test_support()
            .into_any_element()
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
    fn durations_read_like_a_stopwatch() {
        assert_eq!(dur(3200), "3.2s");
        assert_eq!(dur(42_000), "42s");
        assert_eq!(dur(64_000), "1m 04s");
    }
}
