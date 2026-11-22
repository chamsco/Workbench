//! Settings > Flow: the steps a coordinator works through (core flow.rs), each one editable,
//! movable, able to be turned off, removed or added, and a card in the stream for a change the
//! Learn step proposes. The flow is the user's (Charles, 22 Nov): it grows with them.

use backspace_core::flow::{Change, Flow, Step};
use gpui_kit::component::input::{Textarea, TextareaState};

use super::*;
use gpui_kit::prelude::FluentBuilder;

/// The step being edited: its index and its three fields.
pub(crate) struct FlowEdit {
    pub i: usize,
    pub name: (Entity<InputState>, Subscription),
    pub does: (Entity<TextareaState>, Subscription),
    pub skill: (Entity<InputState>, Subscription),
}

impl Cauldron {
    fn flow_data(&self) -> std::path::PathBuf {
        self.core.as_ref().map_or_else(Prefs::data_dir, |(_, ch)| ch.dir().to_path_buf())
    }

    /// The flow this machine uses (Settings edits it; a project's own file wins in that project).
    pub(super) fn flow(&self) -> Flow {
        Flow::load(&self.flow_data(), None)
    }

    fn flow_save(&mut self, f: &Flow, cx: &mut Context<Self>) {
        if let Err(e) = f.save(&Flow::user_file(&self.flow_data())) {
            self.send_error = Some(e.to_string());
        }
        cx.notify();
    }

    fn flow_change(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut Flow)) {
        let mut f = self.flow();
        edit(&mut f);
        self.flow_save(&f, cx);
    }

    fn flow_edit_start(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.flow().steps.get(i).cloned() else { return };
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Step name"));
        name.update(cx, |st, cx| st.set_value(s.name.clone(), window, cx));
        let does = cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 8).placeholder("What the agent does in this step"));
        does.update(cx, |st, cx| st.set_value(s.does.clone(), window, cx));
        let skill = cx.new(|cx| InputState::new(window, cx).placeholder("Skill to load (optional), e.g. sharp-testing"));
        skill.update(cx, |st, cx| st.set_value(s.skill.clone().unwrap_or_default(), window, cx));
        let notify = |cx: &mut Context<Self>, e: &Entity<InputState>, window: &mut Window| cx.subscribe_in(e, window, |_, _, _: &InputEvent, _, cx| cx.notify());
        let (ns, ks) = (notify(cx, &name, window), notify(cx, &skill, window));
        let ds = cx.subscribe_in(&does, window, |_, _, _: &InputEvent, _, cx| cx.notify());
        self.flow_edit = Some(FlowEdit { i, name: (name, ns), does: (does, ds), skill: (skill, ks) });
        cx.notify();
    }

    fn flow_edit_save(&mut self, cx: &mut Context<Self>) {
        let Some(e) = self.flow_edit.take() else { return };
        let name = e.name.0.read(cx).value().trim().to_string();
        let does = e.does.0.read(cx).value().trim().to_string();
        let skill = e.skill.0.read(cx).value().trim().to_string();
        self.flow_change(cx, |f| {
            if let Some(s) = f.steps.get_mut(e.i) {
                if !name.is_empty() {
                    s.name = name;
                }
                if !does.is_empty() {
                    s.does = does;
                }
                s.skill = Some(skill).filter(|k| !k.is_empty());
            }
        });
    }

    /// Settings > Flow.
    #[inline(never)]
    pub(super) fn flow_page(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let flow = self.flow();
        let n = flow.steps.len();
        let small = |id: (&'static str, usize), label: &'static str| {
            div().id(id).h(px(26.)).px(px(8.)).flex().items_center().rounded(px(radius::CONTROL)).text_size(px(12.)).text_color(fg2).cursor_pointer().hover(|d| d.bg(c(pal.hover)).text_color(fg)).aria_label(label).child(label)
        };
        let mut col = div().w_full().max_w(px(720.)).flex().flex_col().gap(px(8.)).px(px(24.)).py(px(8.));
        col = col
            .child(div().text_size(px(20.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).mt(px(12.)).child("Flow"))
            .child(div().text_size(px(12.5)).line_height(px(18.)).text_color(fg3).mb(px(8.)).child("The steps every coordinator works through, in order. Change what a step does, the skill it loads, or turn it off. A project can keep its own flow in .backspace/flow.json; there, that one is used."));
        for (i, s) in flow.steps.iter().enumerate() {
            let editing = self.flow_edit.as_ref().filter(|e| e.i == i);
            let mut card = div().id(("flow-step", i)).flex().flex_col().gap(px(6.)).p(px(12.)).rounded(px(radius::INSET)).bg(c(pal.inset)).aria_label(s.name.clone()).when(!s.on, |d| d.opacity(0.55));
            let head = div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().w(px(20.)).text_size(px(12.)).font_family(MONO).text_color(fg3).child(format!("{}", i + 1)))
                .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(s.name.clone()))
                .when_some(s.skill.clone(), |d, k| d.child(div().h(px(20.)).px(px(7.)).flex().items_center().gap(px(4.)).rounded(px(radius::TAG)).border_1().border_color(c(pal.line)).font_family(MONO).text_size(px(11.)).text_color(fg2).child(glyph("book-open", 11., fg3)).child(k)))
                .child(div().flex_1())
                .child(small(("flow-on", i), if s.on { "On" } else { "Off" }).on_click(cx.listener(move |this, _, _, cx| this.flow_change(cx, |f| f.steps[i].on = !f.steps[i].on))).test_support())
                .when(i > 0, |d| d.child(small(("flow-up", i), "Up").on_click(cx.listener(move |this, _, _, cx| this.flow_change(cx, |f| f.steps.swap(i, i - 1)))).test_support()))
                .when(i + 1 < n, |d| d.child(small(("flow-down", i), "Down").on_click(cx.listener(move |this, _, _, cx| this.flow_change(cx, |f| f.steps.swap(i, i + 1)))).test_support()))
                .child(small(("flow-edit", i), "Edit").on_click(cx.listener(move |this, _, window, cx| this.flow_edit_start(i, window, cx))).test_support())
                .when(n > 1, |d| {
                    d.child(small(("flow-remove", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                        this.flow_edit = None;
                        this.flow_change(cx, |f| {
                            f.steps.remove(i);
                        })
                    })).test_support())
                });
            card = card.child(head);
            match editing {
                Some(e) => {
                    let field = |id: &'static str| div().id(id).min_h(px(32.)).px(px(10.)).flex().items_center().rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line)).bg(c(pal.panel)).text_size(px(13.)).text_color(fg);
                    card = card
                        .child(field("flow-name").child(Input::new(&e.name.0).appearance(false).text_size(px(13.)).p_0()).test_support())
                        .child(field("flow-does").py(px(8.)).items_start().child(Textarea::new(&e.does.0).appearance(false).text_size(px(13.)).p_0()).test_support())
                        .child(field("flow-skill").child(Input::new(&e.skill.0).appearance(false).text_size(px(13.)).p_0()).test_support())
                        .child(
                            div()
                                .flex()
                                .gap(px(6.))
                                .child(div().id("flow-save").h(px(28.)).px(px(12.)).flex().items_center().rounded(px(radius::CONTROL)).bg(c(pal.accent)).text_color(c(pal.on_accent)).text_size(px(12.5)).cursor_pointer().aria_label("Save").child("Save").on_click(cx.listener(|this, _, _, cx| this.flow_edit_save(cx))).test_support())
                                .child(small(("flow-cancel", i), "Cancel").on_click(cx.listener(|this, _, _, cx| {
                                    this.flow_edit = None;
                                    cx.notify();
                                }))),
                        );
                }
                None => card = card.child(div().pl(px(28.)).text_size(px(13.)).line_height(px(19.)).text_color(fg2).child(s.does.clone())),
            }
            col = col.child(card.test_support());
        }
        col = col.child(
            div()
                .flex()
                .gap(px(8.))
                .mt(px(4.))
                .child(small(("flow-add", 0), "Add a step").border_1().border_color(c(pal.line)).on_click(cx.listener(move |this, _, window, cx| {
                    this.flow_change(cx, |f| f.steps.push(Step { key: format!("step{}", f.steps.len() + 1), name: "New step".into(), does: "Say what the agent does here.".into(), skill: None, on: true }));
                    let last = this.flow().steps.len() - 1;
                    this.flow_edit_start(last, window, cx);
                })).test_support())
                .child(small(("flow-reset", 0), "Reset to the defaults").on_click(cx.listener(|this, _, _, cx| {
                    this.flow_edit = None;
                    this.flow_save(&Flow::default(), cx);
                })).test_support()),
        );
        div().id("flow-page").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element()
    }

    /// A change to the flow an agent proposed (a fenced `flow` block): what changes, why, and
    /// Apply, which writes it to the flow in use (the project's when it has its own).
    pub(super) fn flow_card(&self, body: &str, i: usize, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let Some(ch) = Change::parse(body) else {
            return div().text_size(px(12.)).text_color(fg3).child("A proposed change to the flow that couldn't be read.").into_any_element();
        };
        let project = self.thread.as_ref().and_then(|t| t.project.clone()).map(std::path::PathBuf::from);
        let data = self.flow_data();
        let flow = Flow::load(&data, project.as_deref());
        let step = flow.steps.iter().find(|s| s.key.eq_ignore_ascii_case(ch.step.trim())).cloned();
        let applied = step.as_ref().is_some_and(|s| ch.does.as_ref().is_none_or(|d| d.trim() == s.does) && ch.skill.as_ref().is_none_or(|k| Some(k.trim()) == s.skill.as_deref()));
        let title = format!("Change the {} step", step.as_ref().map_or(ch.step.clone(), |s| s.name.clone()));
        let what = match (&ch.does, &ch.skill) {
            (Some(d), _) => d.clone(),
            (None, Some(k)) => format!("Load the {k} skill."),
            _ => String::new(),
        };
        let action: AnyElement = if step.is_none() {
            div().text_size(px(12.)).text_color(fg3).child("Your flow has no such step.").into_any_element()
        } else if applied {
            div().id(("flow-applied", i)).flex().items_center().gap(px(5.)).text_size(px(12.5)).text_color(fg2).aria_label("Applied").child(glyph("check", 13., c(pal.ok))).child("Applied").test_support().into_any_element()
        } else {
            let c2 = ch.clone();
            div()
                .id(("flow-apply", i))
                .h(px(28.))
                .px(px(12.))
                .flex()
                .items_center()
                .rounded(px(radius::CONTROL))
                .bg(c(pal.accent))
                .text_color(c(pal.on_accent))
                .text_size(px(12.5))
                .cursor_pointer()
                .aria_label("Apply")
                .child("Apply")
                .on_click(cx.listener(move |this, _, _, cx| {
                    let project = this.thread.as_ref().and_then(|t| t.project.clone()).map(std::path::PathBuf::from);
                    let data = this.flow_data();
                    // Where the flow in use came from: the project's own file, else this machine's.
                    let file = project.as_ref().map(|p| Flow::project_file(p)).filter(|f| f.exists()).unwrap_or_else(|| Flow::user_file(&data));
                    let mut f = Flow::load(&data, project.as_deref());
                    if f.apply(&c2) {
                        if let Err(e) = f.save(&file) {
                            this.send_error = Some(e.to_string());
                        }
                    }
                    cx.notify();
                }))
                .test_support()
                .into_any_element()
        };
        div()
            .id(("flow-card", i))
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(12.))
            .rounded(px(radius::INSET))
            .bg(c(pal.inset))
            .aria_label(title.clone())
            .child(div().flex().items_center().gap(px(6.)).text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child(glyph("workflow", 13., fg3)).child(title))
            .child(div().text_size(px(13.5)).line_height(px(20.)).text_color(fg).child(what))
            .when(!ch.why.trim().is_empty(), |d| d.child(div().text_size(px(12.5)).line_height(px(18.)).text_color(fg3).child(ch.why.clone())))
            .child(div().flex().child(action))
            .test_support()
            .into_any_element()
    }
}
