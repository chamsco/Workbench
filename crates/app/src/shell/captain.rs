//! Familiars as captains (Charles, 22 Nov, after Capy's captains): a long-lived agent that knows
//! one project end to end and leads a standing crew of agents. New familiar opens this form:
//! the avatar (the app's figure or an emoji, on a colour), the name, the project, the machine,
//! the model, what it may do, and its crew. Create saves the crew and the captain as agents
//! (core agents.rs), then opens the captain's stream, which starts with its intro. The older
//! goal pipeline (plan, Rampart, builders, ship) stays as a mission, one link away.

use super::*;
use backspace_core::agents::{Agent, Avatar, Lead};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder;

pub(crate) const EMOJIS: [&str; 10] = ["🦊", "🦉", "🐙", "🐝", "🐺", "🐢", "🤖", "🧙", "⚓", "🔥"];

/// Crew roles offered at creation: (name, job).
pub(crate) const ROLES: [(&str, &str); 3] = [
    ("Tester", "Writes and runs the tests for each change, and says plainly what fails and why."),
    ("Writer", "Writes the docs, release notes and user-facing text, in the project's voice."),
    ("Reviewer", "Reviews each change for bugs, risks and missing tests before it reaches the user."),
];

/// What a familiar may do: (rule, label). Each is on unless turned off.
pub(crate) const ACCESS: [(&str, &str); 3] = [("edit", "Edit files"), ("run", "Run commands"), ("web", "Use the web")];

/// The New familiar form.
pub(crate) struct Captain {
    pub(crate) name: Option<(Entity<InputState>, Subscription)>,
    /// Its first task, sent as the thread's first message (Charles, 22 Nov: a familiar starts
    /// like any thread, from the chat box, with no greeting).
    pub(crate) task: Option<(Entity<TextareaState>, Subscription)>,
    /// None is the app's figure; Some(i) is `EMOJIS[i]`.
    pub(crate) emoji: Option<usize>,
    /// Into `AGENT_DOTS`.
    pub(crate) color: usize,
    /// Its own container instead of this computer's folder.
    pub(crate) container: bool,
    /// Turned off, by `ACCESS` index.
    pub(crate) off: [bool; 3],
    /// Picked, by `ROLES` index.
    pub(crate) crew: [bool; 3],
    pub(crate) error: Option<String>,
}

impl Default for Captain {
    fn default() -> Self {
        Captain { name: None, task: None, emoji: None, color: 0, container: false, off: [false; 3], crew: [true, false, true], error: None }
    }
}

fn dot_hex(i: usize) -> String {
    let d = super::tokens::AGENT_DOTS;
    d[i % d.len()].to_hex()
}

/// An agent's face: its emoji on a disc of its colour, or the app's figure in that colour.
pub(crate) fn face(a: &Avatar, size: f32, pal: &'static Palette) -> Div {
    let col = super::theme::parse_hex(&a.color).unwrap_or(super::tokens::AGENT_DOTS[0]);
    let disc = div().size(px(size)).flex_none().flex().items_center().justify_center().rounded_full().bg(ca(col, 0.22)).border_1().border_color(ca(col, 0.55));
    if a.emoji.is_empty() {
        let fig = avatar::svg(0, Mood::Idle, Look { px: size * 0.78, outline: false, ms: None, bust: true, ink: Some(col) });
        disc.overflow_hidden().child(img(Arc::new(Image::from_bytes(ImageFormat::Svg, fig.into_bytes()))).size(px(size * 0.78)))
    } else {
        let _ = pal;
        disc.text_size(px(size * 0.5)).child(a.emoji.clone())
    }
}

/// The familiars on disk: agents that lead a project, oldest first.
pub(crate) fn load_captains() -> Vec<Agent> {
    super::thread::agents().list().into_iter().filter(|a| a.lead.is_some()).collect()
}

impl Cauldron {
    fn captain_avatar(&self) -> Avatar {
        let f = self.start.captain.as_ref();
        Avatar { emoji: f.and_then(|f| f.emoji).map(|i| EMOJIS[i].to_string()).unwrap_or_default(), color: dot_hex(f.map_or(0, |f| f.color)) }
    }

    /// New familiar: the form, in the card where the new-stream box sits.
    pub(super) fn open_captain_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = None;
        self.new_thread(window, cx);
        let st = cx.new(|cx| InputState::new(window, cx).placeholder("Name it, e.g. Ez"));
        let sub = cx.subscribe_in(&st, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.create_captain(window, cx);
            }
            cx.notify();
        });
        let f = st.read(cx).focus_handle(cx);
        window.focus(&f, cx);
        let task = cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 8).submit_on_enter(true).placeholder("What should it start on? It works like any thread: this is its first message."));
        let task_sub = cx.subscribe_in(&task, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { shift: false, .. } = ev {
                this.create_captain(window, cx);
            }
            cx.notify();
        });
        // A new colour each time, so two familiars rarely look alike.
        let color = self.start.captains.len() % super::tokens::AGENT_DOTS.len();
        self.start.captain = Some(Captain { name: Some((st, sub)), task: Some((task, task_sub)), color, ..Captain::default() });
        cx.notify();
    }

    fn captain_error(&mut self, e: impl Into<String>, cx: &mut Context<Self>) {
        if let Some(f) = self.start.captain.as_mut() {
            f.error = Some(e.into());
        }
        cx.notify();
    }

    /// Create: the crew, then the captain, then its stream with the intro, opened.
    pub(super) fn create_captain(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = self.start.captain.as_ref() else { return };
        let name = f.name.as_ref().map(|(st, _)| st.read(cx).value().trim().to_string()).unwrap_or_default();
        let task = f.task.as_ref().map(|(st, _)| st.read(cx).value().trim().to_string()).unwrap_or_default();
        if name.is_empty() {
            return self.captain_error("Give your familiar a name.", cx);
        }
        let Some(project) = self.start.folder.clone() else {
            return self.captain_error("A familiar learns one project: pick its folder.", cx);
        };
        let Some(route) = self.start.route.clone() else {
            return self.captain_error("Pick the model it runs on.", cx);
        };
        let Some((_, chats)) = self.core.as_ref() else {
            return self.captain_error("Your threads are still opening.", cx);
        };
        let chats = chats.clone();
        let (off, crew_on, container) = (f.off, f.crew, f.container);
        let avatar = self.captain_avatar();
        let agents = super::thread::agents();
        let kit = |n: &str, job: &str, avatar: Avatar| {
            let mut a = Agent::new(n, route.clone());
            a.job = job.into();
            a.avatar = avatar;
            a.shared = vec![project.clone()];
            a.computer.kind = if container { "docker".into() } else { "folder".into() };
            a.off = ACCESS.iter().zip(off).filter(|(_, o)| *o).map(|((k, _), _)| k.to_string()).collect();
            a
        };
        let made = (|| -> anyhow::Result<String> {
            let mut crew = Vec::new();
            for (i, (role, job)) in ROLES.iter().enumerate().filter(|(i, _)| crew_on[*i]) {
                let face = Avatar { emoji: String::new(), color: dot_hex(i + 3) };
                crew.push(agents.save(kit(role, job, face))?.id);
            }
            let mut cap = kit(&name, "", avatar.clone());
            cap.lead = Some(Lead { project: project.clone(), crew });
            let cap = agents.save(cap)?;
            let id = chats.start_familiar(&cap)?;
            if !task.is_empty() {
                chats.send(&id, &task, Vec::new(), None, self.mode_ctx())?;
            }
            Ok(id)
        })();
        match made {
            Ok(id) => {
                self.start.captain = None;
                self.start.captains = load_captains();
                self.refresh();
                self.open_thread(id);
            }
            Err(e) => self.captain_error(format!("Couldn't create it: {e}"), cx),
        }
        cx.notify();
    }

    /// Open a familiar's stream (its newest), or start a new one when it has none left.
    pub(super) fn open_captain(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some((_, chats)) = self.core.as_ref() else { return };
        let chats = chats.clone();
        let tid = chats.list().into_iter().filter(|t| t.agent.as_deref() == Some(id) && t.side_of.is_none() && !t.archived).max_by_key(|t| t.updated).map(|t| t.id);
        let tid = match tid {
            Some(t) => Some(t),
            None => super::thread::agents().get(id).and_then(|a| chats.start_familiar(&a).ok()),
        };
        if let Some(t) = tid {
            self.select_section(0, cx);
            self.refresh();
            self.open_thread(t);
        }
        cx.notify();
    }

    pub(super) fn captain_view(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(f) = self.start.captain.as_ref() else { return div().into_any_element() };
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

        // Avatar: the figure or an emoji, then the colour.
        let mut kinds = wrap().child(pill(("cap-face", 0), f.emoji.is_none(), "Figure".into()).on_click(cx.listener(|this, _, _, cx| {
            if let Some(f) = this.start.captain.as_mut() {
                f.emoji = None;
            }
            cx.notify();
        })));
        for (i, e) in EMOJIS.iter().enumerate() {
            kinds = kinds.child(
                pill(("cap-emoji", i), f.emoji == Some(i), (*e).to_string())
                    .w(px(28.))
                    .px_0()
                    .justify_center()
                    .text_size(px(15.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(f) = this.start.captain.as_mut() {
                            f.emoji = Some(i);
                        }
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        let mut colours = wrap().gap(px(8.)).mt(px(8.));
        for (i, d) in super::tokens::AGENT_DOTS.iter().enumerate() {
            let on = f.color == i;
            colours = colours.child(
                div()
                    .id(("cap-colour", i))
                    .size(px(20.))
                    .rounded_full()
                    .bg(c(*d))
                    .border_2()
                    .border_color(if on { fg } else { gpui::transparent_black() })
                    .cursor_pointer()
                    .aria_label(format!("Colour {}", i + 1))
                    .aria_selected(on)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(f) = this.start.captain.as_mut() {
                            f.color = i;
                        }
                        cx.notify();
                    }))
                    .test_support(),
            );
        }

        let name = f.name.as_ref().map(|(st, _)| st.clone());
        let head = div()
            .flex()
            .items_center()
            .gap(px(14.))
            .child(face(&self.captain_avatar(), 56., pal))
            .child(
                div()
                    .id("cap-name")
                    .flex_1()
                    .h(px(38.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .rounded(px(radius::CONTROL))
                    .border_1()
                    .border_color(c(pal.line))
                    .bg(c(pal.raised))
                    .text_size(px(15.))
                    .text_color(fg)
                    .when_some(name, |d, st| d.child(Input::new(&st).appearance(false).text_size(px(15.)).p_0()))
                    .test_support(),
            );

        // Project: the new-stream box's folder menu.
        let folder_name = self.start.folder.as_deref().map(|p| p.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(p).to_string());
        let project = div()
            .id("cap-project")
            .relative()
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(radius::CONTROL))
            .border_1()
            .border_color(c(pal.line))
            .text_size(px(12.5))
            .text_color(if folder_name.is_some() { fg } else { fg3 })
            .cursor_pointer()
            .hover(|d| d.bg(c(pal.hover)))
            .aria_label(folder_name.clone().unwrap_or_else(|| "Pick its project".into()))
            .child(glyph("folder", 13., fg3))
            .child(folder_name.unwrap_or_else(|| "Pick its project".into()))
            .child(glyph("chevron-down", 12., fg3))
            .on_click(cx.listener(|this, _, _, cx| {
                this.start.folder_menu = !this.start.folder_menu;
                cx.notify();
            }))
            .when(self.start.folder_menu, |d| d.child(deferred(div().absolute().top(px(32.)).left_0().child(self.folder_menu(pal, cx))).with_priority(2)))
            .test_support();

        let machine = wrap()
            .child(pill(("cap-machine", 0), !f.container, "This computer".into()).on_click(cx.listener(|this, _, _, cx| {
                if let Some(f) = this.start.captain.as_mut() {
                    f.container = false;
                }
                cx.notify();
            })))
            .child(pill(("cap-machine", 1), f.container, "Its own container".into()).on_click(cx.listener(|this, _, _, cx| {
                if let Some(f) = this.start.captain.as_mut() {
                    f.container = true;
                }
                cx.notify();
            })));

        let model_open = self.start.route_menu == Some(super::subs::PickFor::New);
        let model = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().flex().child(div().border_1().border_color(c(pal.line)).rounded(px(radius::CONTROL)).p(px(2.)).child(self.route_chip("cap-model", super::subs::PickFor::New, self.start.route.as_ref(), pal, cx))))
            .when(model_open, |d| d.child(div().rounded(px(radius::PANEL)).border_1().border_color(c(pal.line)).bg(c(pal.raised)).child(self.route_tray(pal, cx))));

        let mut access = wrap();
        for (i, (_, l)) in ACCESS.iter().enumerate() {
            access = access.child(
                pill(("cap-access", i), !f.off[i], (*l).into())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(f) = this.start.captain.as_mut() {
                            f.off[i] = !f.off[i];
                        }
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        let mut crew = wrap();
        for (i, (r, _)) in ROLES.iter().enumerate() {
            crew = crew.child(
                pill(("cap-crew", i), f.crew[i], (*r).into())
                    .child(div().size(px(8.)).rounded_full().bg(c(super::tokens::AGENT_DOTS[(i + 3) % super::tokens::AGENT_DOTS.len()])))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(f) = this.start.captain.as_mut() {
                            f.crew[i] = !f.crew[i];
                        }
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        let crew = div().flex().flex_col().gap(px(6.)).child(crew).child(div().text_size(px(11.5)).text_color(fg3).child("Each crew member is an agent of its own. It keeps its thread under the captain's between jobs."));

        let ready = self.start.folder.is_some() && self.start.route.is_some();
        let create = div()
            .id("cap-create")
            .h(px(32.))
            .px(px(14.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(radius::CONTROL))
            .when(ready, |d| d.primary_face(pal))
            .when(!ready, |d| d.button_face(pal).opacity(0.6))
            .pressed()
            .cursor_pointer()
            .text_size(px(13.))
            .text_color(if ready { c(Rgb::hex(0x111111)) } else { fg2 })
            .aria_label("Create familiar")
            .child(glyph("sparkles", 14., if ready { c(Rgb::hex(0x111111)) } else { fg2 }))
            .child("Create familiar")
            .on_click(cx.listener(|this, _, window, cx| this.create_captain(window, cx)))
            .test_support();
        let mission = div()
            .id("cap-mission")
            .text_size(px(12.))
            .text_color(fg3)
            .cursor_pointer()
            .hover(|d| d.text_color(fg))
            .pt(px(4.))
            .aria_label("Hand a goal to a mission instead")
            .child("Or a mission")
            .on_click(cx.listener(|this, _, window, cx| this.new_mission(window, cx)))
            .test_support();

        let card = div()
            .id("cap-form")
            .w_full()
            .max_w(px(560.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(22.))
            .rounded(px(radius::SHELL))
            .border_1()
            .border_color(c(pal.line))
            .bg(super::backdrop::surface(self.painting, pal.raised, super::backdrop::GLASS_COMPOSER))
            .shadow(super::lift(super::Lift::Box, pal))
            .child(div().flex().items_start().gap(px(12.)).child(div().flex_1().flex().flex_col().gap(px(3.)).child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("New familiar")).child(div().text_size(px(12.5)).text_color(fg3).child("A captain that learns one project end to end and leads a standing crew."))).child(mission))
            .child(head)
            .child(row("Avatar", div().flex().flex_col().child(kinds).child(colours).into_any_element()))
            .child(row("Project", div().flex().child(project).into_any_element()))
            .child(row("Machine", machine.into_any_element()))
            .child(row("Model", model.into_any_element()))
            .child(row("Access", access.into_any_element()))
            .child(row("Crew", crew.into_any_element()))
            .when_some(f.task.as_ref().map(|(st, _)| st.clone()), |d, st| {
                d.child(
                    div()
                        .id("cap-task")
                        .min_h(px(64.))
                        .px(px(12.))
                        .py(px(8.))
                        .rounded(px(radius::CONTROL))
                        .border_1()
                        .border_color(c(pal.line))
                        .bg(c(pal.raised))
                        .text_size(px(13.))
                        .text_color(fg)
                        .child(Textarea::new(&st).appearance(false))
                        .test_support(),
                )
            })
            .when_some(f.error.clone(), |d, e| d.child(div().id("cap-error").text_size(px(12.)).text_color(c(pal.bad)).aria_label(e.clone()).child(e).test_support()))
            .child(div().flex().items_center().child(div().flex_1()).child(create))
            .test_support();
        div().id("cap-screen").size_full().overflow_y_scroll().flex().flex_col().items_center().px(px(24.)).py(px(28.)).child(card).test_support().into_any_element()
    }
}
