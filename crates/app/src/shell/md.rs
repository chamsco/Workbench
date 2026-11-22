//! A reply's markdown, drawn (Charles, 21 Nov: "To run it" showed as `**To run it:**`, a command
//! as three backticks). Only what replies use: headings, paragraphs, bullet and numbered lists,
//! fenced code, and `**bold**` / `` `code` `` inside a line. Half-arrived text is fine: an
//! unclosed fence is code to the end, an unclosed marker stays as typed.

use super::*;
use gpui::{FontWeight, HighlightStyle, StyledText};
use gpui_kit::prelude::FluentBuilder;
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
/// Which hover block each Markdown block belongs to: a heading joins the block after it, and
/// list items in a row are one block.
fn groups(blocks: &[Block]) -> Vec<usize> {
    let mut out = Vec::with_capacity(blocks.len());
    let mut g = 0usize;
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            let prev = &blocks[i - 1];
            let join = matches!(prev, Block::Head(_)) || (matches!(prev, Block::Item(..)) && matches!(b, Block::Item(..)));
            if !join {
                g += 1;
            }
        }
        out.push(g);
    }
    out
}

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
    let (text, hl, links) = styled_parts(s, tail, dir, pal);
    (StyledText::new(text).with_highlights(hl), links)
}

/// `styled` before it becomes a `StyledText`: the shown text, its runs and its links.
fn styled_parts(s: &str, tail: Option<(u64, Hsla)>, dir: Option<&std::path::Path>, pal: &'static Palette) -> (String, Vec<(Range<usize>, HighlightStyle)>, Vec<(Range<usize>, super::links::Link)>) {
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
    (text, hl, links)
}

impl Cauldron {
    /// `text` as blocks. `key` names it for ids; `tail` puts the live caret after the last block.
    /// A reply in the transcript: each block gets the hover bar (`block_bar`) of the message
    /// written at `at`.
    pub(super) fn md_el(&self, text: &str, key: &str, tail: Option<(u64, Hsla)>, at: u64, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        self.md_inner(text, key, tail, (14., 22.), Some(at), pal, cx)
    }

    /// `md_el` at another text size and line height, for the smaller panes (a familiar's plan,
    /// a subagent's ticket and notes).
    pub(super) fn md_sized(&self, text: &str, key: &str, tail: Option<(u64, Hsla)>, size: (f32, f32), pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        self.md_inner(text, key, tail, size, None, pal, cx)
    }

    fn md_inner(&self, text: &str, key: &str, tail: Option<(u64, Hsla)>, (size, line): (f32, f32), bar_at: Option<u64>, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let dir = self.thread.as_ref().and_then(|t| t.project.clone()).map(std::path::PathBuf::from);
        // A line with links: a click opens one in the card, a right click offers where.
        let selectable = bar_at.is_some();
        let link_line = |s: &str, t: Option<(u64, Hsla)>, n: usize, cx: &mut Context<Self>| -> AnyElement {
            // A finished reply line with no link can be selected (links keep their clicks).
            if selectable && t.is_none() {
                let (text, hl, links) = styled_parts(s, None, dir.as_deref(), pal);
                if links.is_empty() {
                    return div().child(super::seltext::SelText::new(SharedString::from(format!("sel-{key}-{n}")), text, hl, ca(pal.accent, 0.28))).into_any_element();
                }
            }
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
        let gap = px((size * 0.55).round());
        let mut col = div().flex().flex_col().gap(gap).text_size(px(size)).line_height(px(line)).text_color(fg);
        // The hover bar works per paragraph (Charles, 22 Nov: "by block, not line by line"): a
        // heading goes with what follows it and a list is one block.
        let groups = groups(&blocks);
        let mut group: Option<(usize, Vec<AnyElement>, Vec<String>)> = None;
        let bar = bar_at.filter(|_| tail.is_none());
        // Each block as wide as its text, so its bar lands right after the text.
        if bar.is_some() {
            col = col.items_start();
        }
        for (i, b) in blocks.into_iter().enumerate() {
            let t = tail.filter(|_| i == last);
            let raw = match &b {
                Block::Para(s) | Block::Head(s) => s.clone(),
                Block::Item(None, s) => format!("- {s}"),
                Block::Item(Some(n), s) => format!("{n}. {s}"),
                Block::Code(lang, body) => format!("```{lang}\n{body}\n```"),
            };
            let el: AnyElement = match b {
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
            };
            let Some(at) = bar else {
                col = col.child(el);
                continue;
            };
            let g = groups[i];
            if group.as_ref().is_some_and(|(k, _, _)| *k != g) {
                let (k, els, raws) = group.take().unwrap();
                col = col.child(self.block_with_bar(div().flex().flex_col().gap(gap).children(els).into_any_element(), &format!("{key}#{k}"), raws.join("\n\n"), at, pal, cx));
            }
            let entry = group.get_or_insert_with(|| (g, Vec::new(), Vec::new()));
            entry.1.push(el);
            entry.2.push(raw);
        }
        if let (Some((k, els, raws)), Some(at)) = (group, bar) {
            col = col.child(self.block_with_bar(div().flex().flex_col().gap(gap).children(els).into_any_element(), &format!("{key}#{k}"), raws.join("\n\n"), at, pal, cx));
        }
        col.into_any_element()
    }
}

/// Room kept at the right of each reply block for its hover bar, so the bar sits on the block's
/// first line, right after the text, without covering it.
const BAR_W: f32 = 132.;

impl Cauldron {
    /// A reply block that, under the pointer, lights up and shows its bar (Capy's, Charles 22 Nov):
    /// the time it was written, Reply (quotes it in the composer), Copy, and ⋯ (fork into a side
    /// thread, copy the whole reply).
    fn block_with_bar(&self, el: AnyElement, bk: &str, raw: String, at: u64, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let on = self.block_hover.as_deref() == Some(bk) || self.block_menu.as_deref() == Some(bk);
        let menu = self.block_menu.as_deref() == Some(bk);
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let msg_id = bk.split([':', '#']).next().unwrap_or("").to_string();
        let k1 = bk.to_string();
        let mut wrap = div()
            .id(SharedString::from(format!("blk-{bk}")))
            .relative()
            .pr(px(BAR_W))
            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                if *h {
                    this.block_hover = Some(k1.clone());
                } else if !this.bar_hover && this.block_hover.as_deref() == Some(k1.as_str()) {
                    this.block_hover = None;
                }
                cx.notify();
            }))
            .child(el);
        if !on {
            return wrap.into_any_element();
        }
        let time = chrono::DateTime::from_timestamp_millis(at as i64).map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string()).unwrap_or_default();
        let btn = |id: &str, icon: &'static str, label: &'static str| {
            div().id(SharedString::from(format!("{id}-{bk}"))).size(px(24.)).flex().items_center().justify_center().rounded(px(radius::TAG)).cursor_pointer().hover(|d| d.bg(c(pal.hover))).aria_label(label).child(glyph(icon, 13., fg2))
        };
        let (q, cp, k2, k3) = (raw.clone(), raw.clone(), bk.to_string(), bk.to_string());
        let bar = div()
            .id(SharedString::from(format!("blk-bar-{bk}")))
            .absolute()
            .top(px(-3.))
            .right_0()
            .h(px(28.))
            .px(px(4.))
            .flex()
            .items_center()
            .gap(px(2.))
            .rounded(px(radius::CONTROL))
            .bg(c(pal.panel))
            .border_1()
            .border_color(c(pal.line))
            .shadow(super::lift(super::Lift::Button, pal))
            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                this.bar_hover = *h;
                this.block_hover = if *h { Some(k2.clone()) } else { None };
                cx.notify();
            }))
            .child(div().id(SharedString::from(format!("blk-time-{bk}"))).px(px(6.)).text_size(px(11.5)).font_features(tnum()).text_color(fg3).aria_label(format!("Written at {time}")).child(time))
            .child(btn("blk-reply", "reply", "Reply to this").on_click(cx.listener(move |this, _, window, cx| {
                this.quote_in_composer(&q, window, cx);
            })))
            .child(btn("blk-copy", "copy", "Copy").on_click(cx.listener(move |_, _, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(cp.clone())))))
            .child(btn("blk-more", "ellipsis", "More").on_click(cx.listener(move |this, _, _, cx| {
                this.block_menu = if this.block_menu.as_deref() == Some(k3.as_str()) { None } else { Some(k3.clone()) };
                cx.notify();
            })));
        wrap = wrap.child(bar);
        if menu {
            let item = |id: &str, icon: &'static str, label: &'static str| {
                div().id(SharedString::from(format!("{id}-{bk}"))).h(px(28.)).px(px(8.)).flex().items_center().gap(px(8.)).rounded(px(radius::TAG)).text_size(px(12.5)).text_color(fg).cursor_pointer().hover(|d| d.bg(c(pal.hover))).aria_label(label).child(glyph(icon, 13., fg2)).child(label)
            };
            let (fq, mid, mid2) = (raw.clone(), msg_id.clone(), msg_id);
            wrap = wrap.child(
                div()
                    .id(SharedString::from(format!("blk-menu-{bk}")))
                    .absolute()
                    .top(px(28.))
                    .right(px(4.))
                    .w(px(220.))
                    .p(px(4.))
                    .flex()
                    .flex_col()
                    .rounded(px(radius::CONTROL))
                    .bg(c(pal.panel))
                    .border_1()
                    .border_color(c(pal.line))
                    .shadow(super::lift(super::Lift::Card, pal))
                    .occlude()
                    .child(item("blk-fork", "git-fork", "Fork into a side thread").on_click(cx.listener(move |this, _, window, cx| {
                        this.block_menu = None;
                        this.fork_block(&mid, &fq, window, cx);
                    })))
                    .child(item("blk-copy-all", "copy", "Copy the whole reply").on_click(cx.listener(move |this, _, _, cx| {
                        this.block_menu = None;
                        if let Some(m) = this.thread.as_ref().and_then(|t| t.messages.iter().find(|m| m.id == mid2)) {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(m.text.clone()));
                        }
                        cx.notify();
                    }))),
            );
        }
        wrap.into_any_element()
    }

    /// Puts `text` in the composer as a quote, with the caret after it.
    pub(super) fn quote_in_composer(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some((st, _)) = self.composer.as_ref() else { return };
        let quoted: String = text.lines().map(|l| format!("> {l}\n")).collect();
        let st = st.clone();
        st.update(cx, |s, cx| {
            let had = s.value().to_string();
            let v = if had.trim().is_empty() { format!("{quoted}\n") } else { format!("{quoted}\n{had}") };
            s.set_value(v, window, cx);
            s.focus(window, cx);
        });
        cx.notify();
    }

    /// A side thread off the open stream that starts with the whole conversation, opened with
    /// this block quoted in its composer.
    fn fork_block(&mut self, _msg: &str, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(id), Some((_, chats))) = (self.open.clone(), self.core.as_ref()) else { return };
        let title: String = text.trim_start_matches(['#', '-', '>', ' ', '*']).chars().take(40).collect();
        match chats.side(&id, &title, backspace_core::chat::SideCtx::Whole) {
            Ok(t) => {
                self.selected = Some(t.id.clone());
                self.open_thread(t.id);
                self.refresh();
                self.quote_in_composer(text, window, cx);
            }
            Err(e) => self.send_error = Some(e.to_string()),
        }
        cx.notify();
    }

    /// The pointer let go over the transcript: keep what is selected, and where, for the bar.
    pub(super) fn note_selection(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let t = gpui_kit::base::TextSelection::selected_text(window, cx);
        self.sel = (!t.trim().is_empty()).then_some((t, at));
        cx.notify();
    }

    /// The small bar over a selection (Charles, 22 Nov): Reply with it quoted, Copy, and fork it
    /// into a side thread.
    pub(super) fn sel_bar_el(&self, pal: &'static Palette, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (text, at) = self.sel.clone()?;
        if !gpui_kit::base::TextSelection::has_selection(window, cx) {
            return None;
        }
        let vp = window.viewport_size();
        let x = (f32::from(at.x) - 50.).min(f32::from(vp.width) - 120.).max(8.);
        let y = (f32::from(at.y) - 40.).max(8.);
        let fg2 = c(pal.fg2);
        let btn = |id: &'static str, icon: &'static str, label: &'static str| {
            div().id(id).size(px(26.)).flex().items_center().justify_center().rounded(px(radius::TAG)).cursor_pointer().hover(|d| d.bg(c(pal.hover))).aria_label(label).child(glyph(icon, 14., fg2))
        };
        let (t1, t2, t3) = (text.clone(), text.clone(), text);
        Some(
            div()
                .id("sel-bar")
                .absolute()
                .left(px(x))
                .top(px(y))
                .h(px(32.))
                .px(px(3.))
                .flex()
                .items_center()
                .gap(px(2.))
                .rounded(px(radius::CONTROL))
                .bg(c(pal.panel))
                .border_1()
                .border_color(c(pal.line))
                .shadow(super::lift(super::Lift::Card, pal))
                .occlude()
                // Down, not click: the selection layer clears the selection on the way up.
                .child(btn("sel-reply", "message-square-quote", "Reply with this quoted").on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.sel = None;
                    gpui_kit::base::TextSelection::clear(window, cx);
                    this.quote_in_composer(&t1, window, cx);
                })))
                .child(btn("sel-copy", "copy", "Copy").on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(t2.clone()));
                    this.sel = None;
                    gpui_kit::base::TextSelection::clear(window, cx);
                    cx.notify();
                })))
                .child(btn("sel-fork", "git-fork", "Fork into a side thread").on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.sel = None;
                    gpui_kit::base::TextSelection::clear(window, cx);
                    this.fork_block("", &t3, window, cx);
                })))
                .test_support()
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{groups, inline, parse, settle, Block};

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
    fn a_heading_joins_its_paragraph_and_a_list_is_one_block() {
        let b = parse("Intro.\n\n## Steps\nDo this.\n\n- one\n- two\n\nAfter.");
        assert_eq!(groups(&b), vec![0, 1, 1, 2, 2, 3]);
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
