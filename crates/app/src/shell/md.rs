//! A reply's markdown, drawn (Charles, 21 Nov: "To run it" showed as `**To run it:**`, a command
//! as three backticks). Only what replies use: headings, paragraphs, bullet and numbered lists,
//! fenced code, and `**bold**` / `` `code` `` inside a line. Half-arrived text is fine: an
//! unclosed fence is code to the end, an unclosed marker stays as typed.

use super::*;
use gpui::{FontWeight, HighlightStyle, StyledText};
use std::ops::Range;

#[derive(Debug, PartialEq, Clone)]
pub(super) enum Block {
    Para(String),
    Head(String),
    /// A list item; the number, or `None` for a bullet.
    Item(Option<u32>, String),
    /// Language, body.
    Code(String, String),
}

pub(super) fn parse(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut para: Vec<&str> = Vec::new();
    let mut code: Option<(String, Vec<&str>)> = None;
    fn flush(para: &mut Vec<&str>, out: &mut Vec<Block>) {
        if !para.is_empty() {
            out.push(Block::Para(para.join(" ")));
            para.clear();
        }
    }
    for line in text.lines() {
        let t = line.trim_start();
        if let Some((lang, body)) = code.as_mut() {
            if t.starts_with("```") {
                out.push(Block::Code(std::mem::take(lang), body.join("\n")));
                code = None;
            } else {
                body.push(line);
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("```") {
            flush(&mut para, &mut out);
            code = Some((rest.trim().to_string(), Vec::new()));
        } else if t.is_empty() {
            flush(&mut para, &mut out);
        } else if let Some(h) = t.trim_start_matches('#').strip_prefix(' ').filter(|_| t.starts_with('#')) {
            flush(&mut para, &mut out);
            out.push(Block::Head(h.trim().to_string()));
        } else if let Some(i) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush(&mut para, &mut out);
            out.push(Block::Item(None, i.trim().to_string()));
        } else if let Some((n, i)) = t.split_once(". ").and_then(|(n, i)| Some((n.parse::<u32>().ok()?, i))) {
            flush(&mut para, &mut out);
            out.push(Block::Item(Some(n), i.trim().to_string()));
        } else {
            para.push(t.trim_end());
        }
    }
    flush(&mut para, &mut out);
    if let Some((lang, body)) = code {
        out.push(Block::Code(lang, body.join("\n")));
    }
    out
}

/// A line's text without its markers, and where the bold (false) and the code (true) sit in it.
fn inline(s: &str) -> (String, Vec<(Range<usize>, bool)>) {
    let (mut out, mut spans) = (String::new(), Vec::new());
    let mut rest = s;
    while !rest.is_empty() {
        let (open, code) = if rest.starts_with("**") {
            ("**", false)
        } else if rest.starts_with('`') {
            ("`", true)
        } else {
            ("", false)
        };
        if let Some(end) = (!open.is_empty()).then(|| rest[open.len()..].find(open)).flatten().filter(|&e| e > 0) {
            let at = out.len();
            out.push_str(&rest[open.len()..open.len() + end]);
            spans.push((at..out.len(), code));
            rest = &rest[open.len() * 2 + end..];
        } else {
            let n = rest.chars().next().map_or(0, char::len_utf8);
            out.push_str(&rest[..n]);
            rest = &rest[n..];
        }
    }
    (out, spans)
}

/// A line still arriving, without a marker that has not closed yet: no flash of `**` or a lone
/// backtick before the bold or the code appears.
fn settle(s: &str) -> String {
    let mut s = s.to_string();
    if s.matches("**").count() % 2 == 1 {
        if let Some(i) = s.rfind("**") {
            s.replace_range(i..i + 2, "");
        }
    }
    if s.matches('`').count() % 2 == 1 {
        if let Some(i) = s.rfind('`') {
            s.remove(i);
        }
    }
    s
}

/// A line drawn, with its links: URLs, and code spans naming a file in `dir`.
fn styled(s: &str, tail: Option<(u64, Hsla)>, dir: Option<&std::path::Path>, pal: &'static Palette) -> (StyledText, Vec<(Range<usize>, super::links::Link)>) {
    let (mut text, spans) = inline(&if tail.is_some() { settle(s) } else { s.to_string() });
    let code: Vec<Range<usize>> = spans.iter().filter(|(_, c)| *c).map(|(r, _)| r.clone()).collect();
    let links = super::links::find(&text, &code, dir);
    let mut hl: Vec<(Range<usize>, HighlightStyle)> = spans
        .into_iter()
        .filter(|(r, _)| !links.iter().any(|(l, _)| l.start < r.end && r.start < l.end))
        .map(|(r, code)| {
            let style = if code {
                HighlightStyle { color: Some(c(pal.accent)), background_color: Some(c(pal.inset)), ..Default::default() }
            } else {
                HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() }
            };
            (r, style)
        })
        .collect();
    // A link: the accent, underlined; in mono when it was a code span (a file).
    for (r, l) in &links {
        let file = matches!(l, super::links::Link::File(_));
        hl.push((r.clone(), HighlightStyle { color: Some(c(pal.accent)), background_color: file.then(|| c(pal.inset)), underline: Some(gpui::UnderlineStyle { thickness: px(1.), color: Some(ca(pal.accent, 0.4)), wavy: false }), ..Default::default() }));
    }
    hl.sort_by_key(|(r, _)| r.start);
    // The live edge: a caret in the accent that blinks.
    if let Some((now, accent)) = tail {
        let at = text.len();
        text.push('\u{258d}');
        hl.push((at..text.len(), HighlightStyle { color: Some(accent.opacity(if (now / 530) % 2 == 0 { 1.0 } else { 0.0 })), ..Default::default() }));
    }
    (StyledText::new(text).with_highlights(hl), links)
}

impl Cauldron {
    /// `text` as blocks. `key` names it for ids; `tail` puts the live caret after the last block.
    pub(super) fn md_el(&self, text: &str, key: &str, tail: Option<(u64, Hsla)>, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        self.md_sized(text, key, tail, (14., 22.), pal, cx)
    }

    /// `md_el` at another text size and line height, for the smaller panes (a familiar's plan,
    /// a subagent's ticket and notes).
    pub(super) fn md_sized(&self, text: &str, key: &str, tail: Option<(u64, Hsla)>, (size, line): (f32, f32), pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let dir = self.thread.as_ref().and_then(|t| t.project.clone()).map(std::path::PathBuf::from);
        // A line with links: a click opens one in the card, a right click offers where.
        let link_line = |s: &str, t: Option<(u64, Hsla)>, n: usize, cx: &mut Context<Self>| -> AnyElement {
            let (st, links) = styled(s, t, dir.as_deref(), pal);
            if links.is_empty() {
                return div().child(st).into_any_element();
            }
            let id = format!("md-{key}-{n}");
            let (ranges, targets): (Vec<_>, Vec<_>) = links.into_iter().unzip();
            let (on_click, on_hover) = (cx.entity().downgrade(), self.link_hover.clone());
            let hov = targets.clone();
            let hover_ranges = ranges.clone();
            let menu = self.link_hover.clone();
            div()
                .id(format!("{id}-wrap"))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    if let Some(l) = menu.borrow().clone() {
                        cx.stop_propagation();
                        this.open_link_menu(l, ev.position, cx);
                    }
                }))
                .child(
                    gpui::InteractiveText::new(gpui::ElementId::Name(id.into()), st)
                        .on_click(ranges, move |i, _, cx| {
                            if let Some(l) = targets.get(i).cloned() {
                                let _ = on_click.update(cx, |this, cx| this.open_link(l, true, cx));
                            }
                        })
                        .on_hover(move |at, _, _, _| {
                            *on_hover.borrow_mut() = at.and_then(|at| hover_ranges.iter().position(|r| r.contains(&at))).map(|k| hov[k].clone());
                        }),
                )
                .into_any_element()
        };
        let blocks = parse(text);
        let last = blocks.len().saturating_sub(1);
        let mut col = div().flex().flex_col().gap(px((size * 0.55).round())).text_size(px(size)).line_height(px(line)).text_color(fg);
        for (i, b) in blocks.into_iter().enumerate() {
            let t = tail.filter(|_| i == last);
            col = col.child(match b {
                Block::Para(s) => link_line(&s, t, i, cx),
                Block::Head(s) => div().mt(px(4.)).font_weight(FontWeight::SEMIBOLD).child(link_line(&s, t, i, cx)).into_any_element(),
                Block::Item(n, s) => div()
                    .flex()
                    .gap(px(8.))
                    .child(div().w(px(18.)).flex_none().text_color(fg3).child(n.map_or("\u{2022}".to_string(), |n| format!("{n}."))))
                    .child(div().min_w_0().flex_1().child(link_line(&s, t, i, cx)))
                    .into_any_element(),
                Block::Code(lang, body) => {
                    let copy = body.clone();
                    let id = format!("code-{key}-{i}");
                    div()
                        .id(id.clone())
                        .rounded(px(radius::CONTROL))
                        .bg(c(pal.inset))
                        .overflow_hidden()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .px(px(12.))
                                .h(px(28.))
                                .text_size(px(11.5))
                                .text_color(fg3)
                                .font_family(MONO)
                                .child(div().child(if lang.is_empty() { "code".to_string() } else { lang }))
                                .child(
                                    div()
                                        .id(format!("{id}-copy"))
                                        .ml_auto()
                                        .cursor_pointer()
                                        .hover(|d| d.text_color(fg))
                                        .aria_label("Copy code")
                                        .child("Copy")
                                        .on_click(cx.listener(move |_, _, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.clone())))),
                                ),
                        )
                        .child(div().px(px(12.)).pb(px(10.)).text_size(px(12.5)).line_height(px(19.)).font_family(MONO).child(body))
                        .into_any_element()
                }
            });
        }
        col.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{inline, parse, settle, Block};

    #[test]
    fn replies_split_into_blocks() {
        let b = parse("Done.\n\n**To run it:** open it\n\n```bash\nstart index.html\n```\n\n- one\n- two\n1. first");
        assert_eq!(b[0], Block::Para("Done.".into()));
        assert_eq!(b[1], Block::Para("**To run it:** open it".into()));
        assert_eq!(b[2], Block::Code("bash".into(), "start index.html".into()));
        assert_eq!(b[3], Block::Item(None, "one".into()));
        assert_eq!(b[5], Block::Item(Some(1), "first".into()));
        // Half-arrived: the fence is still open.
        assert_eq!(parse("```sh\nls"), vec![Block::Code("sh".into(), "ls".into())]);
    }

    #[test]
    fn a_marker_still_open_is_held_back() {
        assert_eq!(settle("run **To"), "run To");
        assert_eq!(settle("run `ls"), "run ls");
        assert_eq!(settle("a **b** `c`"), "a **b** `c`");
    }

    #[test]
    fn markers_leave_the_text_and_mark_a_range() {
        assert_eq!(inline("a **b** `c` d"), ("a b c d".to_string(), vec![(2..3, false), (4..5, true)]));
        assert_eq!(inline("2 ** 3 and `open"), ("2 ** 3 and `open".to_string(), vec![]));
        assert_eq!(inline("é**ü**"), ("éü".to_string(), vec![(2..4, false)]));
    }
}
