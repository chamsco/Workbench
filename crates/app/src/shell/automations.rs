//! Automations (Charles, 22 Nov: "automation and the agent side"): jobs a familiar does on a
//! clock. Each is a routine on the familiar (core agents.rs) and runs in a side thread of its own
//! under the familiar's stream (core automations.rs). The window checks what is due every 30 s
//! while it is open (thread.rs `pump`); nothing runs while Cauldron is closed, and the page says so.

use super::*;
use backspace_core::agents::{Agent, Routine};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder;

/// When it runs: (kind, label).
pub(crate) const KINDS: [(&str, &str); 4] = [("daily", "Every day"), ("weekdays", "Weekdays"), ("weekly", "Some days"), ("every", "Every few hours")];
/// The gaps offered for "every", in minutes.
pub(crate) const EVERY: [u32; 6] = [15, 30, 60, 180, 360, 720];
const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// The New / Edit automation form.
pub(crate) struct Form {
    /// (agent, routine) when editing one.
    pub(crate) editing: Option<(String, String)>,
    pub(crate) name: (Entity<InputState>, Subscription),
    pub(crate) what: (Entity<TextareaState>, Subscription),
    pub(crate) time: (Entity<InputState>, Subscription),
    /// Into the familiars (`Start::captains`).
    pub(crate) familiar: usize,
    pub(crate) kind: usize,
    pub(crate) days: [bool; 7],
    pub(crate) every: usize,
    pub(crate) error: Option<String>,
}

#[derive(Default)]
pub(crate) struct State {
    /// Every automation with its familiar, soonest first (core `automations::all`).
    pub(crate) list: Vec<(Agent, Routine)>,
    pub(crate) form: Option<Form>,
    /// The routine id whose Delete was pressed once: the second press deletes.
    pub(crate) confirm: Option<String>,
    pub(crate) error: Option<String>,
    /// Agents read for drawing (reply headers, proposed-automation cards), so a frame never reads
    /// the disk twice for one. Cleared whenever automations reload.
    pub(crate) cache: std::cell::RefCell<std::collections::HashMap<String, Option<Agent>>>,
}

/// "Mon 08:00", "Today 08:00", "Tomorrow 08:00", in local time.
pub(crate) fn when(ms: u64) -> String {
    let Some(t) = chrono::DateTime::from_timestamp_millis(ms as i64).map(|t| t.with_timezone(&chrono::Local)) else { return String::new() };
    let today = chrono::Local::now().date_naive();
    let day = match (t.date_naive() - today).num_days() {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        _ => t.format("%a %-d %b").to_string(),
    };
    format!("{day} {}", t.format("%H:%M"))
}

/// "just now", "5 min ago", "3 h ago", "2 d ago".
pub(crate) fn ago(ms: u64, now: u64) -> String {
    let m = now.saturating_sub(ms) / 60_000;
    match m {
        0 => "just now".into(),
        1..=59 => format!("{m} min ago"),
        60..=2879 => format!("{} h ago", m / 60),
        _ => format!("{} d ago", m / 1440),
    }
}

/// Minutes east of UTC here, for the routine's wake (core computes it, the UI sends the offset).
fn offset() -> i32 {
    use chrono::Offset;
    chrono::Local::now().offset().fix().local_minus_utc() / 60
}

/// The short label the sidebar shows beside a name: "08:00", "Weekdays", "1 h".
pub(crate) fn short(r: &Routine) -> String {
    if !r.enabled {
        return "Paused".into();
    }
    match r.kind.as_str() {
        "every" if r.every_minutes >= 60 => format!("{} h", r.every_minutes / 60),
        "every" => format!("{} min", r.every_minutes),
        _ => r.time.clone(),
    }
}

impl Cauldron {
    /// Re-reads every automation from disk (cheap: a few small files).
    pub(super) fn load_automations(&mut self) {
        self.autos.cache.borrow_mut().clear();
        self.autos.list = backspace_core::automations::all(&super::thread::agents()).into_iter().filter(|(a, _)| a.lead.is_some()).collect();
    }

    /// New automation (or Edit, for `edit`): the form at the top of the Automations page.
    pub(super) fn open_automation_form(&mut self, edit: Option<(Agent, Routine)>, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = None;
        self.start.captains = super::captain::load_captains();
        self.open_automations_page(cx);
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name it, e.g. Morning PR triage"));
        let name_sub = cx.subscribe_in(&name, window, |_, _, _: &InputEvent, _, cx| cx.notify());
        let what = cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 10).placeholder("What it does each time, as you'd ask it in a thread: \"Read the open PRs and list what blocks each.\""));
        let what_sub = cx.subscribe_in(&what, window, |_, _, _: &InputEvent, _, cx| cx.notify());
        let time = cx.new(|cx| InputState::new(window, cx).placeholder("08:00"));
        let time_sub = cx.subscribe_in(&time, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.save_automation(window, cx);
            }
            cx.notify();
        });
        let pre = self.agent_open.as_deref().and_then(|id| self.start.captains.iter().position(|c| c.id == id)).unwrap_or(0);
        let mut f = Form { editing: None, name: (name, name_sub), what: (what, what_sub), time: (time, time_sub), familiar: pre, kind: 0, days: [true, false, false, false, false, false, false], every: 2, error: None };
        let mut time_text = "08:00".to_string();
        if let Some((a, r)) = edit {
            f.editing = Some((a.id.clone(), r.id.clone()));
            f.familiar = self.start.captains.iter().position(|c| c.id == a.id).unwrap_or(0);
            f.kind = KINDS.iter().position(|(k, _)| *k == r.kind).unwrap_or(0);
            f.days = std::array::from_fn(|d| r.days.contains(&(d as u8 + 1)));
            f.every = EVERY.iter().position(|m| *m == r.every_minutes).unwrap_or(2);
            f.name.0.update(cx, |s, cx| s.set_value(r.name.clone(), window, cx));
            f.what.0.update(cx, |s, cx| s.set_value(r.instructions.clone(), window, cx));
            if !r.time.is_empty() {
                time_text = r.time.clone();
            }
        }
        f.time.0.update(cx, |s, cx| s.set_value(time_text, window, cx));
        let focus = f.name.0.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        self.autos.form = Some(f);
        cx.notify();
    }

    fn automation_error(&mut self, e: impl Into<String>, cx: &mut Context<Self>) {
        if let Some(f) = self.autos.form.as_mut() {
            f.error = Some(e.into());
        }
        cx.notify();
    }

    /// Save: validated by core (a name, a real time, a day for "Some days"), which also works out
    /// the next wake.
    pub(super) fn save_automation(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = self.autos.form.as_ref() else { return };
        let name = f.name.0.read(cx).value().trim().to_string();
        let what = f.what.0.read(cx).value().trim().to_string();
        let time = f.time.0.read(cx).value().trim().to_string();
        let Some(cap) = self.start.captains.get(f.familiar).cloned() else {
            return self.automation_error("An automation belongs to a familiar: make one first.", cx);
        };
        if what.is_empty() {
            return self.automation_error("Say what it should do each time.", cx);
        }
        let agents = super::thread::agents();
        let mut r = match &f.editing {
            Some((a, id)) => agents.get(a).and_then(|a| a.routines.into_iter().find(|r| &r.id == id)).unwrap_or_default(),
            None => Routine { enabled: true, ..Default::default() },
        };
        let moved = f.editing.as_ref().is_some_and(|(a, _)| *a != cap.id);
        r.name = name;
        r.instructions = what;
        r.kind = KINDS[f.kind].0.to_string();
        r.time = time;
        r.days = (0..7u8).filter(|d| f.days[*d as usize]).map(|d| d + 1).collect();
        r.every_minutes = EVERY[f.every];
        r.utc_offset_minutes = offset();
        if moved {
            // Another familiar runs it now: a new routine there, in a new thread.
            r.id = String::new();
            r.thread = String::new();
        }
        match agents.routine_save(&cap.id, r) {
            Ok(_) => {
                if moved {
                    if let Some((a, id)) = &f.editing {
                        let _ = agents.routine_delete(a, id);
                    }
                }
                self.autos.form = None;
                self.load_automations();
            }
            Err(e) => self.automation_error(capitalise(&e.to_string()), cx),
        }
        cx.notify();
    }

    /// Run now: posts it into its thread and opens that thread.
    fn run_automation(&mut self, agent: &str, routine: &str, cx: &mut Context<Self>) {
        let Some((_, chats)) = self.core.as_ref() else { return };
        let chats = chats.clone();
        let agents = super::thread::agents();
        let found = agents.get(agent).and_then(|a| a.routines.iter().find(|r| r.id == routine).cloned().map(|r| (a, r)));
        let Some((a, r)) = found else { return };
        match backspace_core::automations::run(&chats, &agents, &a, &r, false, super::thread::ctx(false)) {
            Ok(id) => {
                self.autos.error = None;
                self.load_automations();
                self.open_agent_thread(&a.id, Some(id), cx);
            }
            Err(e) => self.autos.error = Some(format!("{} didn't start: {e}", r.name)),
        }
        cx.notify();
    }

    fn pause_automation(&mut self, agent: &str, routine: &str, cx: &mut Context<Self>) {
        let agents = super::thread::agents();
        if let Some(mut r) = agents.get(agent).and_then(|a| a.routines.into_iter().find(|r| r.id == routine)) {
            r.enabled = !r.enabled;
            r.utc_offset_minutes = offset();
            if let Err(e) = agents.routine_save(agent, r) {
                self.autos.error = Some(capitalise(&e.to_string()));
            }
        }
        self.load_automations();
        cx.notify();
    }

    fn delete_automation(&mut self, agent: &str, routine: &str, cx: &mut Context<Self>) {
        if self.autos.confirm.as_deref() != Some(routine) {
            self.autos.confirm = Some(routine.to_string());
            return cx.notify();
        }
        self.autos.confirm = None;
        if let Err(e) = super::thread::agents().routine_delete(agent, routine) {
            self.autos.error = Some(capitalise(&e.to_string()));
        }
        self.load_automations();
        cx.notify();
    }

    /// Opens the thread an automation runs in, when it has run at least once.
    pub(super) fn open_automation(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((a, r)) = self.autos.list.get(i).cloned() else { return };
        let alive = self.core.as_ref().is_some_and(|(_, c)| !r.thread.is_empty() && c.thread(&r.thread).is_some());
        if alive {
            self.load_automations();
            self.open_agent_thread(&a.id, Some(r.thread.clone()), cx);
        } else {
            self.open_automation_form(Some((a, r)), window, cx);
        }
    }

    /// The Automations page: the form when open, then one card per automation.
    #[inline(never)]
    pub(super) fn automations_page(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mut kids: Vec<AnyElement> = Vec::new();
        kids.push(
            div()
                .mb(px(14.))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(div().flex_1().text_size(px(12.)).text_color(fg3).child("They run while Cauldron is open. One that comes due while it is closed runs at the next check after it opens."))
                .when(self.autos.form.is_none() && !self.start.captains.is_empty(), |d| {
                    d.child(
                        div()
                            .id("new-automation")
                            .flex_none()
                            .px(px(10.))
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .rounded(px(radius::CONTROL))
                            .border_1()
                            .border_color(c(pal.line))
                            .cursor_pointer()
                            .hover(|d| d.bg(c(pal.hover)))
                            .text_size(px(12.5))
                            .text_color(fg2)
                            .aria_label("New automation")
                            .child(glyph("plus", 13., fg3))
                            .child("New automation")
                            .on_click(cx.listener(|this, _, window, cx| this.open_automation_form(None, window, cx)))
                            .test_support(),
                    )
                })
                .into_any_element(),
        );
        if let Some(e) = &self.autos.error {
            kids.push(div().id("auto-error").mb(px(10.)).text_size(px(12.5)).text_color(c(pal.bad)).aria_label(e.clone()).child(e.clone()).test_support().into_any_element());
        }
        if self.autos.form.is_some() {
            kids.push(self.automation_form(pal, cx));
        }
        if self.autos.list.is_empty() && self.autos.form.is_none() {
            let (title, body) = if self.start.captains.is_empty() {
                ("No familiars yet", "An automation is a job a familiar does on a clock. Make a familiar first, from Agents.")
            } else {
                ("No automations yet", "Give a familiar a job on a clock: a morning PR triage, a nightly test run, a weekly dependency check.")
            };
            kids.push(div().id("auto-empty").flex().flex_col().gap(px(4.)).p(px(18.)).rounded(px(radius::PANEL)).border_1().border_color(c(pal.line)).aria_label(format!("{title}. {body}")).child(div().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg2).child(title)).child(div().text_size(px(12.5)).text_color(fg3).child(body)).test_support().into_any_element());
        }
        let now = backspace_core::chat::now_ms();
        let busy: Vec<String> = match &self.threads {
            Threads::Ready(v) => v.iter().filter(|t| t.busy).map(|t| t.id.clone()).collect(),
            _ => Vec::new(),
        };
        let mut list = div().flex().flex_col().gap(px(10.)).pb(px(24.));
        for (i, (a, r)) in self.autos.list.iter().enumerate() {
            let running = !r.thread.is_empty() && busy.contains(&r.thread);
            let status = if running {
                "Running now".to_string()
            } else if !r.enabled {
                "Paused".to_string()
            } else if r.next_run > 0 {
                format!("Next {}", when(r.next_run))
            } else {
                String::new()
            };
            let last = if r.last_run > 0 { format!("Last ran {} · {} run{}", ago(r.last_run, now), r.runs, if r.runs == 1 { "" } else { "s" }) } else { "Not run yet".into() };
            let btn = |id: (&'static str, usize), icon: &'static str, label: String| {
                div().id(id).h(px(26.)).px(px(9.)).flex().items_center().gap(px(5.)).rounded(px(radius::CONTROL)).cursor_pointer().hover(|d| d.bg(c(pal.hover))).text_size(px(12.)).text_color(fg2).aria_label(label.clone()).child(glyph(icon, 12., fg3)).child(label)
            };
            let (aid, rid) = (a.id.clone(), r.id.clone());
            let (aid2, rid2) = (a.id.clone(), r.id.clone());
            let (aid3, rid3) = (a.id.clone(), r.id.clone());
            let edit = (a.clone(), r.clone());
            let confirm = self.autos.confirm.as_deref() == Some(r.id.as_str());
            let dot = if running { c(pal.accent) } else if r.enabled { c(pal.ok) } else { c(pal.fg4) };
            list = list.child(
                div()
                    .id(("auto-card", i))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .p(px(14.))
                    .rounded(px(radius::PANEL))
                    .border_1()
                    .border_color(c(pal.line))
                    .bg(c(pal.raised))
                    .when(!r.enabled, |d| d.opacity(0.75))
                    .aria_label(format!("{}, {}, {}", r.name, r.summary(), a.name))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(super::captain::face(&a.avatar, 28., pal))
                            .child(div().flex_1().min_w_0().flex().flex_col().child(div().truncate().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(r.name.clone())).child(div().truncate().text_size(px(11.5)).text_color(fg3).child(format!("{} · {}", a.name, r.summary()))))
                            .child(div().flex().items_center().gap(px(6.)).flex_none().text_size(px(11.5)).font_features(tnum()).text_color(fg2).child(div().size(px(7.)).rounded_full().bg(dot)).child(status)),
                    )
                    .child(div().text_size(px(12.5)).line_height(px(18.)).text_color(fg2).line_clamp(2).child(r.instructions.clone()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .child(div().flex_1().text_size(px(11.5)).text_color(fg3).child(last))
                            .when(!r.thread.is_empty(), |d| d.child(btn(("auto-open", i), "message-square", "Open".into()).on_click(cx.listener(move |this, _, window, cx| this.open_automation(i, window, cx))).test_support()))
                            .child(btn(("auto-run", i), "play", "Run now".into()).when(running, |d| d.opacity(0.5)).on_click(cx.listener(move |this, _, _, cx| this.run_automation(&aid, &rid, cx))).test_support())
                            .child(btn(("auto-pause", i), if r.enabled { "pause" } else { "play" }, if r.enabled { "Pause".into() } else { "Resume".into() }).on_click(cx.listener(move |this, _, _, cx| this.pause_automation(&aid2, &rid2, cx))).test_support())
                            .child(btn(("auto-edit", i), "pencil", "Edit".into()).on_click(cx.listener(move |this, _, window, cx| this.open_automation_form(Some(edit.clone()), window, cx))).test_support())
                            .child(btn(("auto-delete", i), "trash", if confirm { "Delete it?".into() } else { "Delete".into() }).when(confirm, |d| d.text_color(c(pal.bad))).on_click(cx.listener(move |this, _, _, cx| this.delete_automation(&aid3, &rid3, cx))).test_support()),
                    )
                    .test_support(),
            );
        }
        kids.push(list.into_any_element());
        Self::page_shell("automations-page", "Automations", "Jobs your familiars do on a clock, each in a thread of its own under the familiar's stream.", pal, kids)
    }

    #[inline(never)]
    fn automation_form(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(f) = self.autos.form.as_ref() else { return div().into_any_element() };
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let label = |t: &'static str| div().w(px(92.)).flex_none().pt(px(5.)).text_size(px(12.)).text_color(fg3).child(t);
        let row = |t: &'static str, body: AnyElement| div().flex().gap(px(12.)).child(label(t)).child(div().flex_1().min_w_0().child(body));
        let pill = |id: (&'static str, usize), on: bool, text: String| {
            div()
                .id(id)
                .h(px(28.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(6.))
                .rounded(px(radius::CONTROL))
                .border_1()
                .border_color(if on { ca(pal.accent, 0.6) } else { c(pal.line) })
                .when(on, |d| d.bg(ca(pal.accent, 0.14)))
                .text_size(px(12.5))
                .text_color(if on { fg } else { fg2 })
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .aria_label(text.clone())
                .aria_selected(on)
                .child(text)
        };
        let wrap = || div().flex().flex_wrap().gap(px(5.));
        let field = |id: &'static str, h: f32| div().id(id).min_h(px(h)).px(px(10.)).flex().items_center().rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line)).bg(c(pal.panel)).text_size(px(13.)).text_color(fg);

        let mut who = wrap();
        if self.start.captains.is_empty() {
            who = who.child(div().text_size(px(12.5)).text_color(fg3).child("No familiars yet. Make one from Agents first."));
        }
        for (i, a) in self.start.captains.iter().enumerate() {
            who = who.child(
                div()
                    .id(("auto-who", i))
                    .h(px(28.))
                    .pl(px(4.))
                    .pr(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(radius::CONTROL))
                    .border_1()
                    .border_color(if f.familiar == i { ca(pal.accent, 0.6) } else { c(pal.line) })
                    .when(f.familiar == i, |d| d.bg(ca(pal.accent, 0.14)))
                    .text_size(px(12.5))
                    .text_color(if f.familiar == i { fg } else { fg2 })
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label(a.name.clone())
                    .aria_selected(f.familiar == i)
                    .child(super::captain::face(&a.avatar, 20., pal))
                    .child(a.name.clone())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(f) = this.autos.form.as_mut() {
                            f.familiar = i;
                        }
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        let mut kinds = wrap();
        for (i, (_, l)) in KINDS.iter().enumerate() {
            kinds = kinds.child(
                pill(("auto-kind", i), f.kind == i, (*l).into())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(f) = this.autos.form.as_mut() {
                            f.kind = i;
                        }
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        let every = KINDS[f.kind].0 == "every";
        let mut sub = div().flex().flex_col().gap(px(8.)).mt(px(8.));
        if every {
            let mut gaps = wrap();
            for (i, m) in EVERY.iter().enumerate() {
                let t = if *m >= 60 { format!("{} h", m / 60) } else { format!("{m} min") };
                gaps = gaps.child(
                    pill(("auto-every", i), f.every == i, t)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(f) = this.autos.form.as_mut() {
                                f.every = i;
                            }
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            sub = sub.child(gaps);
        } else {
            if KINDS[f.kind].0 == "weekly" {
                let mut days = wrap();
                for (d, n) in DAYS.iter().enumerate() {
                    days = days.child(
                        pill(("auto-day", d), f.days[d], (*n).into())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(f) = this.autos.form.as_mut() {
                                    f.days[d] = !f.days[d];
                                }
                                cx.notify();
                            }))
                            .test_support(),
                    );
                }
                sub = sub.child(days);
            }
            sub = sub.child(div().flex().items_center().gap(px(8.)).child(div().text_size(px(12.)).text_color(fg3).child("at")).child(field("auto-time", 28.).w(px(76.)).font_features(tnum()).child(Input::new(&f.time.0).appearance(false).text_size(px(13.)).p_0()).test_support()).child(div().text_size(px(11.5)).text_color(fg3).child("your time, 24 h")));
        }

        let save_label = if f.editing.is_some() { "Save" } else { "Create automation" };
        let save = div()
            .id("auto-save")
            .h(px(32.))
            .px(px(14.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(radius::CONTROL))
            .primary_face(pal)
            .pressed()
            .cursor_pointer()
            .text_size(px(13.))
            .text_color(c(Rgb::hex(0x111111)))
            .aria_label(save_label)
            .child(glyph("calendar", 14., c(Rgb::hex(0x111111))))
            .child(save_label)
            .on_click(cx.listener(|this, _, window, cx| this.save_automation(window, cx)))
            .test_support();
        let cancel = div()
            .id("auto-cancel")
            .h(px(32.))
            .px(px(12.))
            .flex()
            .items_center()
            .rounded(px(radius::CONTROL))
            .cursor_pointer()
            .hover(|d| d.bg(c(pal.hover)))
            .text_size(px(13.))
            .text_color(fg2)
            .aria_label("Cancel")
            .child("Cancel")
            .on_click(cx.listener(|this, _, _, cx| {
                this.autos.form = None;
                cx.notify();
            }))
            .test_support();

        div()
            .id("auto-form")
            .w_full()
            .flex()
            .flex_col()
            .gap(px(14.))
            .p(px(20.))
            .mb(px(14.))
            .rounded(px(radius::SHELL))
            .border_1()
            .border_color(c(pal.line))
            .bg(super::well_fill(self.painting, pal, super::backdrop::GLASS_COMPOSER))
            .shadow(super::lift(super::Lift::Well, pal))
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(if f.editing.is_some() { "Edit automation" } else { "New automation" }))
            .child(row("Name", field("auto-name", 32.).child(Input::new(&f.name.0).appearance(false).text_size(px(13.)).p_0()).test_support().into_any_element()))
            .child(row("What it does", field("auto-what", 64.).py(px(8.)).items_start().child(Textarea::new(&f.what.0).appearance(false).text_size(px(13.)).p_0()).test_support().into_any_element()))
            .child(row("Familiar", who.into_any_element()))
            .child(row("When", div().flex().flex_col().child(kinds).child(sub).into_any_element()))
            .when_some(f.error.clone(), |d, e| d.child(div().id("auto-form-error").text_size(px(12.5)).text_color(c(pal.bad)).aria_label(e.clone()).child(e).test_support()))
            .child(div().flex().items_center().gap(px(6.)).child(div().flex_1().text_size(px(11.5)).text_color(fg3).child("Each run posts these words into the automation's own thread, where the familiar answers as usual.")).child(cancel).child(save))
            .test_support()
            .into_any_element()
    }

    /// Agent `id`, read once per reload (see `State::cache`).
    pub(super) fn agent_cached(&self, id: &str) -> Option<Agent> {
        if let Some(a) = self.autos.cache.borrow().get(id) {
            return a.clone();
        }
        let a = super::thread::agents().get(id);
        self.autos.cache.borrow_mut().insert(id.to_string(), a.clone());
        a
    }

    /// The familiar the open thread belongs to: its own agent, or its stream's when it is a side.
    fn thread_familiar(&self) -> Option<Agent> {
        let t = self.thread.as_ref()?;
        let own = t.agent.as_deref().and_then(|a| self.agent_cached(a)).filter(|a| a.lead.is_some());
        own.or_else(|| {
            let main = t.side_of.as_deref().and_then(|m| self.core.as_ref().and_then(|(_, c)| c.thread(m)))?;
            main.agent.as_deref().and_then(|a| self.agent_cached(a)).filter(|a| a.lead.is_some())
        })
    }

    /// Above a reply in an agent's thread: its face and name (Charles wanted the familiar, not
    /// only the model, to say who answered).
    #[inline(never)]
    pub(super) fn reply_who(&self, agent: Option<&str>, pal: &'static Palette) -> Option<Div> {
        let a = self.agent_cached(agent?)?;
        Some(div().flex().items_center().gap(px(6.)).child(super::captain::face(&a.avatar, 18., pal)).child(div().text_size(px(12.5)).font_weight(FontWeight::MEDIUM).text_color(c(pal.fg)).child(a.name)))
    }

    /// A ```routine block in a reply: what the familiar proposes, and Add. Adding saves it as one of
    /// the familiar's automations; nothing is scheduled until then (core's brief says so too).
    #[inline(never)]
    pub(super) fn routine_card(&self, body: &str, i: usize, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let Ok(mut r) = serde_json::from_str::<Routine>(body.trim()) else {
            return div().text_size(px(12.)).text_color(fg3).child("A proposed automation that couldn't be read.").into_any_element();
        };
        r.enabled = true;
        let fam = self.thread_familiar();
        let added = fam.as_ref().is_some_and(|a| a.routines.iter().any(|x| x.name.trim() == r.name.trim()));
        let action: AnyElement = match (&fam, added) {
            (Some(_), true) => div()
                .id(("routine-added", i))
                .h(px(28.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(5.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .text_size(px(12.5))
                .text_color(fg2)
                .aria_label("Added. Open Automations")
                .child(glyph("check", 13., c(pal.ok)))
                .child("Added")
                .on_click(cx.listener(|this, _, _, cx| this.open_automations_page(cx)))
                .test_support()
                .into_any_element(),
            (Some(a), false) => {
                let (aid, prop) = (a.id.clone(), r.clone());
                div()
                    .id(("routine-add", i))
                    .h(px(28.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .rounded(px(radius::CONTROL))
                    .primary_face(pal)
                    .pressed()
                    .cursor_pointer()
                    .text_size(px(12.5))
                    .text_color(c(Rgb::hex(0x111111)))
                    .aria_label(format!("Add automation {}", r.name))
                    .child(glyph("plus", 13., c(Rgb::hex(0x111111))))
                    .child("Add")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let mut r = prop.clone();
                        r.id = String::new();
                        r.thread = String::new();
                        r.utc_offset_minutes = offset();
                        if let Err(e) = super::thread::agents().routine_save(&aid, r) {
                            this.send_error = Some(capitalise(&e.to_string()));
                        }
                        this.load_automations();
                        cx.notify();
                    }))
                    .test_support()
                    .into_any_element()
            }
            (None, _) => div().text_size(px(11.5)).text_color(fg3).child("Only a familiar runs automations").into_any_element(),
        };
        div()
            .id(("routine-card", i))
            .w_full()
            .max_w(px(520.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(12.))
            .rounded(px(radius::PANEL))
            .border_1()
            .border_color(c(pal.line))
            .bg(c(pal.inset))
            .aria_label(format!("Proposed automation: {}, {}", r.name, r.summary()))
            .child(div().flex().items_center().gap(px(6.)).text_size(px(11.5)).text_color(fg3).child(glyph("calendar", 13., fg3)).child("Proposed automation"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(div().flex_1().min_w_0().flex().flex_col().child(div().truncate().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(r.name.clone())).child(div().text_size(px(12.)).text_color(fg2).child(r.summary())))
                    .child(action),
            )
            .when(!r.instructions.trim().is_empty(), |d| d.child(div().text_size(px(12.5)).line_height(px(18.)).text_color(fg2).child(r.instructions.clone())))
            .test_support()
            .into_any_element()
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
