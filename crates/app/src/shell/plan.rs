//! A coordinator's plan above its thread (Charles, 22 Nov): what it will do, is doing and has
//! done, folded or open. The plan is the newest fenced `todo` block in the thread (core asks a
//! coordinator to keep one, chat.rs TODO_BRIEF). Each step, under the pointer, offers Reply
//! (quote it in the box), Edit and Remove; "Add a step" adds one. A change goes to the agent as
//! a `Plan edit:` message carrying the whole new plan: a change to the step it is on stops the
//! turn and goes now, any other waits for the turn to end so the work is not interrupted.

use super::*;
use backspace_core::chat::Thread;
use gpui_kit::prelude::FluentBuilder;
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Step {
    pub(crate) text: String,
    /// "todo", "doing" or "done".
    pub(crate) state: String,
}

/// The steps of the last `todo` block in `text`, if it has one that reads.
pub(crate) fn parse(text: &str) -> Option<Vec<Step>> {
    let start = text.rfind("```todo")?;
    let body = &text[start + 7..];
    let body = body.split_once("```").map_or(body, |(b, _)| b);
    let v: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    let steps: Vec<Step> = v["items"]
        .as_array()?
        .iter()
        .filter_map(|i| {
            let text = i["text"].as_str()?.trim().to_string();
            let state = match i["state"].as_str().unwrap_or("todo") {
                s @ ("doing" | "done") => s.to_string(),
                _ => "todo".to_string(),
            };
            (!text.is_empty()).then_some(Step { text, state })
        })
        .collect();
    (!steps.is_empty()).then_some(steps)
}

/// The thread's plan now: the newest message that carries one.
pub(crate) fn current(t: &Thread) -> Option<Vec<Step>> {
    t.messages.iter().rev().find_map(|m| parse(&m.text))
}

/// The text of a message without its `todo` block (the plan shows above the thread instead).
pub(crate) fn strip(text: &str) -> String {
    let Some(start) = text.find("```todo") else { return text.to_string() };
    let rest = &text[start + 7..];
    let after = rest.split_once("```").map_or("", |(_, a)| a);
    format!("{}{}", text[..start].trim_end(), if after.trim().is_empty() { String::new() } else { format!("\n\n{}", after.trim_start()) })
}

/// The message that changes the plan: what changed in words, then the whole plan.
pub(crate) fn edit_message(what: &str, steps: &[Step]) -> String {
    let items: Vec<serde_json::Value> = steps.iter().map(|s| serde_json::json!({ "text": s.text, "state": s.state })).collect();
    let json = serde_json::to_string(&serde_json::json!({ "items": items })).unwrap_or_default();
    format!("Plan edit: {what}\n\n```todo\n{json}\n```")
}

impl Cauldron {
    /// Applies a change and sends it: now when it touches the step in progress, else after the turn.
    fn plan_change(&mut self, steps: Vec<Step>, what: String, now: bool, cx: &mut Context<Self>) {
        let (Some(id), Some((_, chats))) = (self.open.clone(), self.core.as_ref()) else { return };
        let chats = chats.clone();
        if let Err(e) = chats.steer(&id, &edit_message(&what, &steps), now, self.mode_ctx()) {
            self.send_error = Some(e.to_string());
        }
        self.plan_edit = None;
        self.refresh();
        cx.notify();
    }

    fn plan_start_edit(&mut self, i: usize, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx));
        st.update(cx, |s, cx| s.set_value(text, window, cx));
        let sub = cx.subscribe_in(&st, window, move |this, st, ev: &InputEvent, _, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let new = st.read(cx).value().trim().to_string();
                this.plan_commit(i, new, cx);
            }
        });
        let f = st.read(cx).focus_handle(cx);
        window.focus(&f, cx);
        self.plan_edit = Some((i, st, sub));
        cx.notify();
    }

    /// Enter in a step's editor: changed, added (i past the end), or removed when emptied.
    fn plan_commit(&mut self, i: usize, new: String, cx: &mut Context<Self>) {
        let Some(mut steps) = self.thread.as_ref().and_then(current).or_else(|| Some(Vec::new())) else { return };
        let (what, now) = if i >= steps.len() {
            if new.is_empty() {
                self.plan_edit = None;
                cx.notify();
                return;
            }
            steps.push(Step { text: new.clone(), state: "todo".into() });
            (format!("add a step at the end: \"{new}\""), false)
        } else if new.is_empty() {
            let old = steps.remove(i);
            (format!("remove step {}: \"{}\"", i + 1, old.text), old.state == "doing")
        } else if new == steps[i].text {
            self.plan_edit = None;
            cx.notify();
            return;
        } else {
            let now = steps[i].state == "doing";
            let old = std::mem::replace(&mut steps[i].text, new.clone());
            (format!("step {} \"{old}\" is now \"{new}\"", i + 1), now)
        };
        self.plan_change(steps, what, now, cx);
    }

    /// The plan panel, when the open thread has a plan.
    pub(super) fn plan_el(&self, t: &Thread, pal: &'static Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let steps = current(t)?;
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let done = steps.iter().filter(|s| s.state == "done").count();
        let doing = steps.iter().find(|s| s.state == "doing").map(|s| s.text.clone());
        let open = !self.plan_folded;
        let head = div()
            .id("plan-head")
            .h(px(32.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(8.))
            .cursor_pointer()
            .text_size(px(12.5))
            .aria_label(format!("Plan, {done} of {} done", steps.len()))
            .aria_expanded(open)
            .child(glyph("list-checks", 14., fg3))
            .child(div().font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Plan"))
            .child(div().text_color(fg3).font_features(tnum()).child(format!("{done} of {}", steps.len())))
            .when(!open, |d| d.when_some(doing.clone(), |d, s| d.child(div().min_w_0().flex_1().truncate().text_color(fg2).child(format!("· {s}")))))
            .child(div().ml_auto().child(glyph(if open { "chevron-up" } else { "chevron-down" }, 14., fg3)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.plan_folded = !this.plan_folded;
                cx.notify();
            }))
            .test_support();
        let mut panel = div()
            .id("plan")
            .flex_none()
            .flex()
            .flex_col()
            .rounded(px(radius::PANEL))
            .border_1()
            .border_color(c(pal.line))
            .bg(super::backdrop::surface(self.painting, pal.raised, super::backdrop::GLASS_COMPOSER))
            .child(head);
        if !open {
            return Some(panel.test_support().into_any_element());
        }
        let mut list = div().id("plan-steps").max_h(px(220.)).overflow_y_scroll().px(px(6.)).pb(px(6.)).flex().flex_col();
        for (i, s) in steps.iter().enumerate() {
            let editing = self.plan_edit.as_ref().filter(|(k, _, _)| *k == i).map(|(_, st, _)| st.clone());
            let hover = self.plan_hover == Some(i) && editing.is_none();
            let (icon, col) = match s.state.as_str() {
                "done" => ("circle-check", c(pal.ok)),
                "doing" => ("circle-dot", c(pal.accent)),
                _ => ("circle", fg3),
            };
            let btn = |id: &'static str, icon: &'static str, label: &'static str| {
                div().id((id, i)).size(px(22.)).flex().items_center().justify_center().rounded(px(radius::TAG)).cursor_pointer().hover(|d| d.bg(c(pal.hover))).aria_label(label).child(glyph(icon, 12., fg2))
            };
            let (quote, text) = (s.text.clone(), s.text.clone());
            let row = div()
                .id(("plan-step", i))
                .relative()
                .min_h(px(28.))
                .px(px(6.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(radius::CONTROL))
                .when(hover, |d| d.bg(c(pal.hover)))
                .text_size(px(13.))
                .text_color(if s.state == "done" { fg3 } else { fg })
                .aria_label(format!("{}: {}", s.state, s.text))
                .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                    if *h {
                        this.plan_hover = Some(i);
                    } else if this.plan_hover == Some(i) {
                        this.plan_hover = None;
                    }
                    cx.notify();
                }))
                .child(glyph(icon, 14., col))
                .child(match editing {
                    Some(st) => div().flex_1().min_w_0().child(Input::new(&st).appearance(false).text_size(px(13.)).p_0()).into_any_element(),
                    // As wide as its text, so the step's buttons sit right after it.
                    None => div().min_w_0().when(s.state == "done", |d| d.line_through()).child(s.text.clone()).into_any_element(),
                })
                .when(hover, |d| {
                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .child(btn("plan-reply", "reply", "Reply to this step").on_click(cx.listener(move |this, _, window, cx| this.quote_in_composer(&quote, window, cx))))
                            .child(btn("plan-edit", "pencil", "Edit this step").on_click(cx.listener(move |this, _, window, cx| this.plan_start_edit(i, text.clone(), window, cx))))
                            .child(btn("plan-remove", "x", "Remove this step").on_click(cx.listener(move |this, _, _, cx| this.plan_commit(i, String::new(), cx)))),
                    )
                })
                .test_support();
            list = list.child(row);
        }
        // A new step, typed in place.
        let adding = self.plan_edit.as_ref().filter(|(k, _, _)| *k >= steps.len()).map(|(_, st, _)| st.clone());
        let n = steps.len();
        list = list.child(match adding {
            Some(st) => div().id("plan-new").min_h(px(28.)).px(px(6.)).flex().items_center().gap(px(8.)).child(glyph("plus", 14., fg3)).child(div().flex_1().child(Input::new(&st).appearance(false).text_size(px(13.)).p_0())).test_support().into_any_element(),
            None => div()
                .id("plan-add")
                .h(px(28.))
                .px(px(6.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_size(px(12.5))
                .text_color(fg3)
                .hover(|d| d.bg(c(pal.hover)).text_color(fg))
                .aria_label("Add a step")
                .child(glyph("plus", 14., fg3))
                .child("Add a step")
                .on_click(cx.listener(move |this, _, window, cx| this.plan_start_edit(n, String::new(), window, cx)))
                .test_support()
                .into_any_element(),
        });
        panel = panel.child(list.test_support());
        let note = if self.busy() { "A change to the step in progress goes now; any other waits for this turn to end." } else { "Edits go to the agent as your next message." };
        panel = panel.child(div().px(px(12.)).pb(px(8.)).text_size(px(11.)).text_color(fg3).child(note));
        Some(panel.test_support().into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn the_last_todo_block_is_the_plan_and_an_edit_carries_the_whole_plan() {
        let text = "Starting.\n\n```todo\n{\"items\": [{\"text\": \"Tests\", \"state\": \"done\"}, {\"text\": \"Login\", \"state\": \"doing\"}, {\"text\": \"Docs\"}]}\n```\n\nOn it.";
        let p = parse(text).unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!((p[1].text.as_str(), p[1].state.as_str(), p[2].state.as_str()), ("Login", "doing", "todo"));
        assert_eq!(strip(text), "Starting.\n\nOn it.");
        let m = edit_message("add \"Ship\"", &p);
        assert!(m.starts_with("Plan edit: add \"Ship\""));
        assert_eq!(parse(&m).unwrap(), p, "the edit's own block becomes the plan");
        assert!(parse("no plan here").is_none());
    }
}
