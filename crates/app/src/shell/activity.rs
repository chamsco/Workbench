//! Thinking / Acting (Charles, 27 Nov, after Tuff UI's coding agent and Claude): a reply's
//! reasoning, tool calls and the asides it wrote between them fold into one quiet line, "Ran 2
//! commands, read 3 files", that opens on a click. Only what the person needs comes out in full:
//! the text the reply ends on (a result, a question, a request), and a plan or a card it put down
//! on the way. While it works, the line is a Thinking Orb and what it is doing now.

use super::ledger::{Part, Step};
use super::orbs::Orb;
use super::*;
use gpui_kit::prelude::FluentBuilder;

/// What the transcript draws for a reply, by index into its parts.
#[derive(Debug, PartialEq)]
pub(super) enum Shown {
    /// A paragraph out in full.
    Text(usize),
    /// Steps, and the asides between them, folded under one line.
    Group(Vec<usize>),
}

/// Text that carries something to act on comes out even mid-reply: a plan, a proposed routine or
/// flow, the Done card's next steps.
fn surfaces(t: &str) -> bool {
    ["```todo", "```routine", "```flow", "```next"].iter().any(|f| t.contains(f))
}

/// The reply's parts as drawn. Text after the last step is the answer and shows; text before it
/// was the model talking to itself on the way, and folds with the steps around it, unless it
/// carries a card or comes right before a plan update (it is moving on to the next task).
pub(super) fn arrange(parts: &[Part]) -> Vec<Shown> {
    let last_step = parts.iter().rposition(|p| matches!(p, Part::Step(_)));
    let mut out = Vec::new();
    let mut group: Vec<usize> = Vec::new();
    let close = |group: &mut Vec<usize>, out: &mut Vec<Shown>| {
        if group.iter().any(|&j| matches!(parts[j], Part::Step(_))) {
            out.push(Shown::Group(std::mem::take(group)));
        } else {
            out.extend(group.drain(..).map(Shown::Text));
        }
    };
    for (j, p) in parts.iter().enumerate() {
        match p {
            Part::Step(_) => group.push(j),
            Part::Text(t) => {
                let next_is_plan = matches!(parts.get(j + 1), Some(Part::Step(s)) if s.sort == "plan");
                if last_step.map_or(true, |ls| j > ls) || surfaces(t) || next_is_plan {
                    close(&mut group, &mut out);
                    out.push(Shown::Text(j));
                } else {
                    group.push(j);
                }
            }
        }
    }
    close(&mut group, &mut out);
    out
}

/// The folded line for finished steps: what was done, most first-seen first ("Ran 2 commands,
/// read 3 files"); only reasoning reads "Thought for 12s".
pub(super) fn summary(steps: &[&Step]) -> String {
    let mut order: Vec<&'static str> = Vec::new();
    let mut n: std::collections::HashMap<&'static str, usize> = Default::default();
    for s in steps {
        if s.sort != "think" && !order.contains(&s.sort) {
            order.push(s.sort);
        }
        *n.entry(s.sort).or_default() += s.calls;
    }
    const RANK: [&str; 8] = ["edit", "run", "agent", "tool", "read", "search", "web", "plan"];
    order.sort_by_key(|k| RANK.iter().position(|r| r == k).unwrap_or(RANK.len()));
    if order.is_empty() {
        let span = span(steps);
        return match span {
            Some(ms) if ms >= 1000 => format!("Thought for {}", super::ledger::dur(ms)),
            _ => "Thought".into(),
        };
    }
    let count = |k: usize, one: &str, many: &str| if k == 1 { format!("a {one}") } else { format!("{k} {many}") };
    let mut said: Vec<String> = order
        .iter()
        .map(|&k| {
            let c = n[k];
            match k {
                "read" => format!("read {}", if c == 1 { "a file".into() } else { format!("{c} files") }),
                "edit" => format!("edited {}", if c == 1 { "a file".into() } else { format!("{c} files") }),
                "search" => "searched the code".into(),
                "web" => "searched the web".into(),
                "run" => format!("ran {}", if c == 1 { "a command".into() } else { format!("{c} commands") }),
                "agent" => format!("ran {}", count(c, "subagent", "subagents")),
                "plan" => "updated the plan".into(),
                _ => format!("used {}", if c == 1 { "a tool".into() } else { format!("{c} tools") }),
            }
        })
        .collect();
    // Three kinds is as much as one line holds: the ones that changed something first.
    said.truncate(3);
    let mut line = said.join(", ");
    if let Some(f) = line.get_mut(0..1) {
        f.make_ascii_uppercase();
    }
    let failed = steps.iter().filter(|s| s.error).count();
    if failed > 0 {
        line.push_str(&format!(", {failed} failed"));
    }
    line
}

/// From the first step's start to the last one's end.
fn span(steps: &[&Step]) -> Option<u64> {
    let a = steps.iter().map(|s| s.start).min()?;
    let b = steps.iter().filter_map(|s| s.end).max()?;
    Some(b.saturating_sub(a))
}

/// The orb for what a running step does; None is the model thinking between steps.
pub(super) fn orb_for(s: Option<&Step>) -> Orb {
    match s.map(|s| s.sort) {
        Some("read" | "search" | "web") => Orb::Searching,
        Some("edit") => Orb::Composing,
        Some("run" | "tool" | "agent") => Orb::Solving,
        Some("plan") => Orb::Shaping,
        _ => Orb::Working,
    }
}

/// Whether the palette is a dark one, for the orb's ink.
pub(super) fn dark(pal: &Palette) -> bool {
    let b = pal.bg;
    (b.0 as u32 + b.1 as u32 + b.2 as u32) < 3 * 128
}

/// Local servers a reply started or names (a dev server's "http://localhost:5173"), in the order
/// first seen: from what its commands printed, then its text.
pub(super) fn local_apps(parts: &[Part]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut take = |text: &str| {
        for (_, l) in super::links::find(text, &[], None) {
            if let super::links::Link::Url(u) = l {
                let h = backspace_core::unfurl::host(&u);
                let u = u.trim_end_matches('/').to_string();
                if (h.starts_with("localhost") || h.starts_with("127.0.0.1") || h.starts_with("0.0.0.0")) && !out.contains(&u) {
                    out.push(u);
                }
            }
        }
    };
    for p in parts {
        match p {
            Part::Step(s) => {
                if let Some((_, printed)) = &s.shell {
                    take(printed);
                }
            }
            Part::Text(t) => take(t),
        }
    }
    out.truncate(2);
    out
}

/// Pictures the reply made or looked at, as its steps name them, newest first, at most four.
pub(super) fn pictures(parts: &[Part]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in parts.iter().rev() {
        if let Part::Step(s) = p {
            if let Some(f) = s.file.as_ref().filter(|f| super::imageview::is_image(std::path::Path::new(f))) {
                if !out.contains(f) {
                    out.push(f.clone());
                }
            }
        }
    }
    out.truncate(4);
    out
}

impl Cauldron {
    /// One Thinking / Acting group: its line, and when open, the steps and asides it holds.
    /// `running` is whether the reply is still being written and this is its newest group; `wait`
    /// is why nothing arrives (a retry), said in place of "Thinking".
    #[inline(never)]
    pub(super) fn activity_el(&self, parts: &[&Part], running: bool, wait: Option<&str>, now: u64, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let steps: Vec<&Step> = parts.iter().filter_map(|p| if let Part::Step(s) = p { Some(s) } else { None }).collect();
        let key = format!("grp:{}", steps.first().map_or("", |s| s.key.as_str()));
        let open = self.act_open.get(&key).copied().unwrap_or(false);
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let still = cx.reduce_motion();
        let busy = steps.iter().rev().find(|s| s.end.is_none()).copied().filter(|_| running);
        let label = match (running, busy, wait) {
            (true, Some(s), _) => s.live.clone(),
            (true, None, Some(w)) => w.to_string(),
            (true, None, None) => "Thinking".to_string(),
            _ => summary(&steps),
        };
        // While it runs: the clock since the step began (or since the group did); after: how long it all took.
        let time = if running {
            let t0 = busy.map(|s| s.start).or_else(|| steps.iter().filter_map(|s| s.end).max()).unwrap_or(now);
            Some(super::ledger::dur(now.saturating_sub(t0)))
        } else {
            span(&steps).filter(|&ms| ms >= 1000 && steps.iter().any(|s| s.sort != "think")).map(super::ledger::dur)
        };
        let k = key.clone();
        let head = div()
            .id(SharedString::from(format!("{key}-head")))
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(24.))
            .text_size(px(13.))
            .text_color(fg3)
            .cursor_pointer()
            .hover(|d| d.text_color(fg))
            .aria_label(match &time {
                Some(t) => format!("{label}, {t}"),
                None => label.clone(),
            })
            .aria_expanded(open)
            // The orb says what kind of work it is; once done the line needs no mark (Claude).
            .when(running, |d| d.child(div().size(px(16.)).flex_none().child(super::orbs::orb(if busy.is_none() && wait.is_some() { Orb::Listening } else { orb_for(busy) }, 16., now, still, dark(pal)))))
            .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(if running { super::ledger::shimmer(&label, now, fg3, fg, still).into_any_element() } else { label.clone().into_any_element() }))
            .when_some(time, |d, t| d.child(div().flex_none().font_family(MONO).font_features(tnum()).text_size(px(11.5)).opacity(0.8).child(t)))
            .child(div().flex_none().child(glyph(if open { "chevron-down" } else { "chevron-right" }, 11., fg3)))
            .on_click(cx.listener(move |this, _, _, cx| {
                let was = this.act_open.get(&k).copied().unwrap_or(false);
                this.act_open.insert(k.clone(), !was);
                cx.notify();
            }));
        let mut col = div().id(SharedString::from(key.clone())).flex().flex_col().gap(px(6.)).child(head.test_support());
        if open {
            // Under a thin rule, the steps as the ledger draws them and the asides in small grey.
            let mut body = div().flex().flex_col().gap(px(6.)).ml(px(8.)).pl(px(14.)).border_l_1().border_color(c(pal.line));
            for (j, p) in parts.iter().enumerate() {
                body = match p {
                    Part::Step(s) => body.child(self.step_el(s, running && s.end.is_none(), now, pal, cx)),
                    Part::Text(t) => body.child(div().text_color(fg2).child(self.md_sized(t, &format!("{key}-aside-{j}"), None, (13., 20.), pal, cx))),
                };
            }
            col = col.child(body);
        }
        col.into_any_element()
    }

    /// Previews under a reply (Charles, 27 Nov): an app it has running, as a card that opens it
    /// beside the stream; the pictures it made, as thumbnails that open full size.
    #[inline(never)]
    pub(super) fn previews_el(&self, i: usize, parts: &[Part], pal: &'static Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let apps = local_apps(parts);
        let dir = self.thread.as_ref().and_then(|t| t.project.clone()).map(std::path::PathBuf::from);
        let pics: Vec<std::path::PathBuf> = pictures(parts)
            .into_iter()
            .map(|f| {
                let p = std::path::PathBuf::from(&f);
                if p.is_absolute() { p } else { dir.as_ref().map_or(p.clone(), |d| d.join(&p)) }
            })
            .filter(|p| p.exists())
            .collect();
        if apps.is_empty() && pics.is_empty() {
            return None;
        }
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let mut col = div().id(("previews", i)).flex().flex_col().gap(px(8.)).max_w(px(560.));
        for (k, u) in apps.into_iter().enumerate() {
            let open = u.clone();
            col = col.child(
                div()
                    .id(("app-preview", i * 10 + k))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(48.))
                    .px(px(12.))
                    .rounded(px(radius::CONTROL))
                    .border_1()
                    .border_color(c(pal.line))
                    .bg(c(pal.panel))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label(format!("App running at {u}"))
                    .child(div().size(px(28.)).flex_none().flex().items_center().justify_center().rounded(px(radius::TAG)).bg(c(pal.inset)).child(glyph("globe", 14., c(pal.fg2))))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(div().flex().items_center().gap(px(6.)).text_size(px(13.)).text_color(fg).child(div().size(px(6.)).rounded_full().bg(c(pal.ok))).child("App running"))
                            .child(div().text_size(px(11.5)).font_family(MONO).text_color(fg3).overflow_hidden().whitespace_nowrap().text_ellipsis().child(u.clone())),
                    )
                    .child(div().ml_auto().flex_none().flex().items_center().gap(px(5.)).h(px(26.)).px(px(10.)).rounded(px(radius::INNER)).border_1().border_color(c(pal.line_strong)).text_size(px(12.)).text_color(fg).child("Open preview").child(glyph("external-link", 11., fg3)))
                    .on_click(cx.listener(move |this, _, _, cx| this.open_link(super::links::Link::Url(open.clone()), true, cx)))
                    .test_support(),
            );
        }
        if !pics.is_empty() {
            let mut row = div().flex().flex_wrap().gap(px(8.));
            for (k, p) in pics.into_iter().enumerate() {
                let name = p.file_name().map_or_else(String::new, |n| n.to_string_lossy().to_string());
                let show = p.clone();
                row = row.child(
                    div()
                        .id(("pic-preview", i * 10 + k))
                        .w(px(168.))
                        .flex()
                        .flex_col()
                        .rounded(px(radius::CONTROL))
                        .border_1()
                        .border_color(c(pal.line))
                        .bg(c(pal.panel))
                        .overflow_hidden()
                        .cursor_pointer()
                        .hover(|d| d.border_color(c(pal.line_strong)))
                        .aria_label(format!("Preview {name}"))
                        .child(div().h(px(100.)).w_full().bg(c(pal.inset)).child(img(p).size_full().object_fit(ObjectFit::Cover)))
                        .child(div().h(px(26.)).px(px(8.)).flex().items_center().gap(px(5.)).text_size(px(11.5)).text_color(fg3).child(glyph("image", 11., fg3)).child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(name)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.image_view = Some(show.clone());
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            col = col.child(row);
        }
        Some(col.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(sort: &'static str, start: u64, end: Option<u64>) -> Part {
        Part::Step(Step { key: format!("m:{start}"), live: String::new(), done: String::new(), start, end, error: false, detail: None, cmd: None, shell: None, file: None, args: None, tries: 1, sort, calls: 1 })
    }

    fn text(s: &str) -> Part {
        Part::Text(s.into())
    }

    /// The point of the change: the model's asides between tool calls fold away, the answer it
    /// ends on does not.
    #[::core::prelude::v1::test]
    fn only_the_text_after_the_last_step_shows() {
        let parts = vec![text("Let me look."), step("read", 0, Some(1)), text("Now the tests."), step("run", 2, Some(3)), text("All green.")];
        assert_eq!(arrange(&parts), vec![Shown::Group(vec![0, 1, 2, 3]), Shown::Text(4)]);
    }

    /// Moving on to the next task, or putting down a plan, is said out loud.
    #[::core::prelude::v1::test]
    fn text_before_a_plan_update_or_carrying_a_card_shows() {
        let parts = vec![step("read", 0, Some(1)), text("Part one done."), step("plan", 2, Some(3)), step("edit", 4, Some(5))];
        assert_eq!(arrange(&parts), vec![Shown::Group(vec![0]), Shown::Text(1), Shown::Group(vec![2, 3])]);
        let parts = vec![step("read", 0, Some(1)), text("```todo\n[]\n```"), step("run", 2, Some(3))];
        assert_eq!(arrange(&parts), vec![Shown::Group(vec![0]), Shown::Text(1), Shown::Group(vec![2])]);
    }
}
