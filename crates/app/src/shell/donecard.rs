//! The Done card (Charles, 22 Nov): when a stream's last reply finished a task, a card over the
//! box says so, with Archive, and "Or carry on" lists what might come next. Tick the ones you
//! want and Continue sends them; "?" on one shows why it helps. The suggestions are the reply's
//! fenced `next` block (core asks for one, chat.rs NEXT_BRIEF), so they cost no extra call.

use super::*;
use backspace_core::chat::{Role, Status, Thread};
use gpui_kit::prelude::FluentBuilder;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Next {
    pub(crate) text: String,
    pub(crate) why: String,
}

/// What is ticked and which "?" is open, for one reply.
#[derive(Default)]
pub(crate) struct DoneState {
    pub(crate) msg: String,
    pub(crate) picked: std::collections::BTreeSet<usize>,
    pub(crate) why: Option<usize>,
}

/// The items of the last `next` block in `text`, if it has one that reads.
pub(crate) fn parse(text: &str) -> Option<Vec<Next>> {
    let start = text.rfind("```next")?;
    let body = &text[start + 7..];
    let body = body.split_once("```").map_or(body, |(b, _)| b);
    let v: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    let items: Vec<Next> = v["items"]
        .as_array()?
        .iter()
        .filter_map(|i| {
            let text = i["text"].as_str()?.trim().to_string();
            let why = i["why"].as_str().unwrap_or("").trim().to_string();
            (!text.is_empty()).then_some(Next { text, why })
        })
        .take(5)
        .collect();
    (!items.is_empty()).then_some(items)
}

/// The reply the card is for: the thread's last message, a finished reply that carries a `next`
/// block. None while it runs, after a failure, or once you have written since.
pub(crate) fn card_for(t: &Thread) -> Option<(String, Vec<Next>)> {
    let m = t.messages.last()?;
    if m.role == Role::User || m.status != Status::Done || m.error.is_some() {
        return None;
    }
    parse(&m.text).map(|n| (m.id.clone(), n))
}

/// What Continue sends: the one picked as it is, or several as a short list.
pub(crate) fn message(picked: &[&Next]) -> String {
    match picked {
        [one] => one.text.clone(),
        many => {
            let lines: Vec<String> = many.iter().enumerate().map(|(i, n)| format!("{}. {}", i + 1, n.text)).collect();
            format!("Next, please:\n{}", lines.join("\n"))
        }
    }
}

impl Cauldron {
    fn done_state(&mut self, msg: &str) -> &mut DoneState {
        if self.done_card.msg != msg {
            self.done_card = DoneState { msg: msg.to_string(), ..Default::default() };
        }
        &mut self.done_card
    }

    fn done_continue(&mut self, msg: String, items: Vec<Next>, cx: &mut Context<Self>) {
        let (Some(id), Some((_, chats))) = (self.open.clone(), self.core.as_ref()) else { return };
        let chats = chats.clone();
        let st = self.done_state(&msg);
        let picked: Vec<&Next> = st.picked.iter().filter_map(|&i| items.get(i)).collect();
        if picked.is_empty() {
            return;
        }
        let text = message(&picked);
        self.session_pick = None;
        match chats.send(&id, &text, Vec::new(), None, self.mode_ctx()) {
            Ok(()) => self.send_error = None,
            Err(e) => self.send_error = Some(e.to_string()),
        }
        self.done_card = DoneState::default();
        self.refresh();
        cx.notify();
    }

    fn done_archive(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some((_, chats))) = (self.open.clone(), self.core.as_ref()) else { return };
        let _ = chats.archive(&id, true);
        self.close_open_stream();
        self.refresh();
        cx.notify();
    }

    /// The card over the box, when the open stream's last reply finished with suggestions.
    #[inline(never)]
    pub(super) fn done_el(&self, t: &Thread, pal: &'static Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (msg, items) = card_for(t)?;
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mine = self.done_card.msg == msg;
        let picked = |i: usize| mine && self.done_card.picked.contains(&i);
        let why_open = if mine { self.done_card.why } else { None };
        let n = items.iter().enumerate().filter(|(i, _)| picked(*i)).count();
        let ok = c(pal.ok);
        let head = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .pt(px(10.))
            .child(div().size(px(20.)).flex().items_center().justify_center().rounded_full().bg(ok.opacity(0.14)).child(glyph("check", 12., ok)))
            .child(div().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Done"))
            .child(div().flex_1())
            .child(
                div()
                    .id("done-archive")
                    .h(px(26.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(radius::CONTROL))
                    .border_1()
                    .border_color(c(pal.line))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .text_size(px(12.))
                    .text_color(fg2)
                    .aria_label("Archive")
                    .child(glyph("archive", 12., fg3))
                    .child("Archive")
                    .on_click(cx.listener(|this, _, _, cx| this.done_archive(cx)))
                    .test_support(),
            );
        let mut list = div().flex().flex_col().px(px(6.)).pb(px(6.));
        for (i, it) in items.iter().enumerate() {
            let on = picked(i);
            let open = why_open == Some(i);
            let (m1, m2) = (msg.clone(), msg.clone());
            let row = div()
                .id(("done-next", i))
                .min_h(px(30.))
                .px(px(6.))
                .flex()
                .items_center()
                .gap(px(9.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .aria_label(it.text.clone())
                .aria_selected(on)
                .child(
                    div()
                        .size(px(15.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .border_1()
                        .border_color(if on { c(pal.accent) } else { c(pal.line_strong) })
                        .when(on, |d| d.bg(c(pal.accent)).child(glyph("check", 11., c(pal.raised)))),
                )
                .child(div().flex_1().min_w_0().text_size(px(13.)).text_color(if on { fg } else { fg2 }).child(it.text.clone()))
                .when(!it.why.is_empty(), |d| {
                    d.child(
                        div()
                            .id(("done-why", i))
                            .size(px(20.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .border_1()
                            .border_color(if open { c(pal.accent) } else { c(pal.line) })
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if open { c(pal.accent) } else { fg3 })
                            .hover(|d| d.bg(c(pal.hover)))
                            .aria_label("What does this mean?")
                            .aria_expanded(open)
                            .child("?")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                let st = this.done_state(&m2);
                                st.why = if st.why == Some(i) { None } else { Some(i) };
                                cx.notify();
                            }))
                            .test_support(),
                    )
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    let st = this.done_state(&m1);
                    if !st.picked.remove(&i) {
                        st.picked.insert(i);
                    }
                    cx.notify();
                }))
                .test_support();
            list = list.child(row);
            if open {
                list = list.child(div().id(("done-why-text", i)).pl(px(30.)).pr(px(10.)).pb(px(6.)).text_size(px(12.)).line_height(px(17.)).text_color(fg3).aria_label(it.why.clone()).child(it.why.clone()).test_support());
            }
        }
        let foot = (n > 0).then(|| {
            let (m, its) = (msg.clone(), items.clone());
            div().flex().justify_end().px(px(12.)).pb(px(10.)).child(
                div()
                    .id("done-continue")
                    .h(px(28.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .bg(c(pal.accent))
                    .text_size(px(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(pal.raised))
                    .aria_label(if n == 1 { "Continue".to_string() } else { format!("Continue with {n}") })
                    .child(if n == 1 { "Continue".to_string() } else { format!("Continue with {n}") })
                    .child(glyph("arrow-up", 12., c(pal.raised)))
                    .on_click(cx.listener(move |this, _, _, cx| this.done_continue(m.clone(), its.clone(), cx)))
                    .test_support(),
            )
        });
        Some(
            div()
                .id("done-card")
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(6.))
                .rounded(px(radius::PANEL))
                .border_1()
                .border_color(c(pal.line))
                .bg(super::well_fill(self.painting, pal, super::backdrop::GLASS_COMPOSER))
                .shadow(super::lift(super::Lift::Well, pal))
                .child(head)
                .child(div().px(px(12.)).text_size(px(11.5)).text_color(fg3).child("Or carry on"))
                .child(list)
                .children(foot)
                .test_support()
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::thread::tests::{data_replying, open_data};
    use super::{message, parse, Next};
    use backspace_core::chat::Role;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};

    /// A finished reply with a `next` block: the card over the box, "?" shows the why, ticking
    /// two and Continue sends them as one message; Archive puts the stream away.
    #[gpui_kit::test]
    fn the_done_card_ticks_explains_and_sends(cx: &mut TestAppContext) {
        let reply = "All 591 pass.\n\n```next\n{\"items\": [{\"text\": \"Add a test\", \"why\": \"Pins the bug.\"}, {\"text\": \"Ship it\", \"why\": \"It is ready.\"}]}\n```";
        let (handle, chats) = open_data(cx, data_replying(reply));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
            window.render_frame(cx);
            assert!(window.try_find("done-card").is_some(), "the card is over the box");
            assert!(window.try_find("done-continue").is_none(), "nothing ticked, nothing to send");
            window.click(("done-why", 0usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find(("done-why-text", 0usize)).label(), Some("Pins the bug."));
            window.click(("done-next", 0usize), cx);
            window.click(("done-next", 1usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find("done-continue").label(), Some("Continue with 2"));
            window.click("done-continue", cx);
        })
        .unwrap();
        let t = chats.peek("t1").unwrap();
        let sent = t.messages.iter().rev().find(|m| m.role == Role::User).unwrap();
        assert_eq!(sent.text, "Next, please:\n1. Add a test\n2. Ship it");
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("done-card").is_none(), "gone once you have written");
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn archive_on_the_done_card_puts_the_stream_away(cx: &mut TestAppContext) {
        let (handle, chats) = open_data(cx, data_replying("Done.\n```next\n{\"items\": [{\"text\": \"Ship it\"}]}\n```"));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("thread", 0usize), cx);
            window.render_frame(cx);
            assert!(window.try_find(("done-why", 0usize)).is_none(), "no why, no ?");
            window.click("done-archive", cx);
        })
        .unwrap();
        assert!(chats.peek("t1").unwrap().archived);
    }

    #[::core::prelude::v1::test]
    fn the_next_block_reads_and_continue_words_the_picks() {
        let text = "Fixed the login.\n\n```next\n{\"items\": [{\"text\": \"Add a test\", \"why\": \"Pins the bug.\"}, {\"text\": \"Ship it\"}, {\"text\": \"  \"}]}\n```";
        let n = parse(text).unwrap();
        assert_eq!(n.len(), 2, "an empty item is dropped");
        assert_eq!(n[0], Next { text: "Add a test".into(), why: "Pins the bug.".into() });
        assert_eq!(n[1].why, "");
        assert_eq!(message(&[&n[0]]), "Add a test");
        assert_eq!(message(&[&n[0], &n[1]]), "Next, please:\n1. Add a test\n2. Ship it");
        assert!(parse("no block").is_none());
        assert!(parse("```next\nnot json\n```").is_none());
    }
}
