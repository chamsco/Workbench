//! An agent in Agents (Charles, 22 Nov): clicking a captain or a crew member keeps you in Agents.
//! Its sessions sit on the left, the selected one beside them with the reply box. Talking to an
//! agent is one long thread cut into sessions (core sessions.rs: a new one starts 5 min after the
//! last reply finished); each automation keeps its own thread, so its runs are its history. The
//! figure's face follows what the agent is doing (`agent_mood`).

use super::*;
use backspace_core::agents::Agent;
use backspace_core::sessions::{self, Kind, Session};
use gpui_kit::prelude::FluentBuilder;

/// "Show every session" in `session_pick`.
pub(crate) const ALL: usize = usize::MAX;

/// An agent's figure with `mood` in its colour, or its emoji on a disc when it has one.
pub(crate) fn figure(a: &Agent, mood: Mood, size: f32, pal: &'static Palette) -> AnyElement {
    if !a.avatar.emoji.is_empty() {
        return super::captain::face(&a.avatar, size, pal).into_any_element();
    }
    let col = super::theme::parse_hex(&a.avatar.color).unwrap_or(super::tokens::AGENT_DOTS[0]);
    let svg = avatar::svg(0, mood, Look { px: size, outline: false, ms: None, bust: true, ink: Some(col) });
    img(Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))).size(px(size)).flex_none().into_any_element()
}

/// The face for what an agent is doing, from core's `Chats::doing`.
pub(crate) fn mood_for(d: &backspace_core::chat::Doing, now: u64) -> Mood {
    if d.busy {
        if d.sending {
            return Mood::Thinking;
        }
        let act = d.act.as_deref().unwrap_or("").to_ascii_lowercase();
        return if act.contains("read") || act.contains("grep") || act.contains("glob") || act.contains("ls") {
            Mood::Reading
        } else if act.contains("web") || act.contains("search") || act.contains("fetch") || act.contains("browser") {
            Mood::Watching
        } else if act.contains("edit") || act.contains("write") || act.contains("patch") {
            Mood::Typing
        } else if act.is_empty() {
            Mood::Typing
        } else {
            Mood::Working
        };
    }
    if d.failed {
        Mood::Error
    } else if d.asks {
        Mood::Asking
    } else if d.waiting {
        Mood::Waiting
    } else if d.unread {
        Mood::Surprised
    } else if d.last > 0 && now.saturating_sub(d.last) > 30 * 60_000 {
        Mood::Sleepy
    } else {
        Mood::Idle
    }
}

impl Cauldron {
    fn doing(&self, id: &str) -> backspace_core::chat::Doing {
        self.core.as_ref().map(|(_, c)| c.doing(id)).unwrap_or_default()
    }

    /// The agent's face now.
    pub(super) fn agent_mood(&self, id: &str) -> Mood {
        mood_for(&self.doing(id), backspace_core::chat::now_ms())
    }

    /// The status a row says in words (Charles, 22 Nov, after T3 Code: words, not tiny dots).
    pub(super) fn agent_status(&self, id: &str, pal: &'static Palette) -> (&'static str, Hsla) {
        let d = self.doing(id);
        if d.busy {
            ("Working", c(pal.accent))
        } else if d.failed {
            ("Failed", c(pal.bad))
        } else if d.asks {
            ("? Input", c(pal.warn))
        } else if d.unread {
            ("\u{2713} Done", c(pal.ok))
        } else {
            ("Idle", c(pal.fg3))
        }
    }

    /// The thread you talk to `a` in: a captain's own stream, a crew member's standing thread
    /// under its captain's, or any agent's newest thread.
    fn talk_thread(&self, a: &Agent) -> Option<String> {
        let (_, chats) = self.core.as_ref()?;
        if a.lead.is_some() {
            return chats.familiar_main(a).or_else(|| chats.start_familiar(a).ok());
        }
        let agents = super::thread::agents();
        let captain = agents.list().into_iter().find(|c| c.lead.as_ref().is_some_and(|l| l.crew.contains(&a.id)));
        // Its standing thread under the captain, made now if the captain has not used it yet, so
        // you can talk to it before it has a job (Charles, 22 Nov 20:04).
        if let Some(c) = captain {
            if let Some(t) = chats.familiar_main(&c).or_else(|| chats.start_familiar(&c).ok()).and_then(|main| chats.crew_thread_or_new(&main, a).ok()) {
                return Some(t);
            }
        }
        chats.list().into_iter().filter(|t| t.agent.as_deref() == Some(a.id.as_str()) && !t.archived).max_by_key(|t| t.updated).map(|t| t.id)
    }

    /// Opens agent `id` in Agents: its sessions and the thread you talk to it in.
    pub(super) fn show_agent(&mut self, id: &str, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(a) = super::thread::agents().get(id) else { return };
        let t = self.talk_thread(&a);
        self.open_agent_thread(id, t, cx);
    }

    /// Opens agent `id` in Agents on thread `t` (its talk thread, or one of its automations').
    pub(super) fn open_agent_thread(&mut self, id: &str, t: Option<String>, cx: &mut Context<Self>) {
        self.settings = None;
        self.select_section(section_of("Agents"), cx);
        self.autos_page = false;
        self.session_pick = None;
        self.refresh();
        match t {
            Some(t) => self.open_thread(t),
            None => {
                self.open = None;
                self.thread = None;
            }
        }
        self.section_page = false;
        self.agent_open = Some(id.to_string());
        cx.notify();
    }

    /// The Automations page, inside Agents.
    pub(super) fn open_automations_page(&mut self, cx: &mut Context<Self>) {
        self.settings = None;
        self.agent_open = None;
        self.select_section(section_of("Agents"), cx);
        self.autos_page = true;
        self.section_page = true;
        cx.notify();
    }

    /// The open thread's sessions, oldest first.
    pub(super) fn open_sessions(&self, t: &backspace_core::chat::Thread) -> Vec<Session> {
        sessions::split(t, backspace_core::chat::now_ms())
    }

    /// The part of the transcript shown: one session (the newest unless one is picked), or all.
    #[inline(never)]
    pub(super) fn session_slice(&self, t: &backspace_core::chat::Thread, ls: Vec<super::thread::Line>) -> Vec<super::thread::Line> {
        if self.agent_open.is_none() || self.session_pick == Some(ALL) {
            return ls;
        }
        let ss = self.open_sessions(t);
        let Some(s) = self.session_pick.and_then(|k| ss.get(k)).or(ss.last()) else { return ls };
        ls.into_iter().skip(s.start).take(s.end - s.start).collect()
    }

    /// The agent view: its rail of sessions and automations, then the thread.
    #[inline(never)]
    pub(super) fn agent_body(&self, pal: &'static Palette, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let main: AnyElement = if self.thread.is_some() {
            self.card_body(pal, window, cx)
        } else {
            div().id("agent-empty").size_full().flex().items_center().justify_center().text_size(px(13.)).text_color(c(pal.fg3)).child("No thread with this agent yet. Write below to start one.").test_support().into_any_element()
        };
        div().id("agent-view").size_full().flex().child(self.agent_rail(pal, cx)).child(div().flex_1().min_w_0().h_full().child(main)).test_support().into_any_element()
    }

    #[inline(never)]
    fn agent_rail(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let Some(a) = self.agent_open.as_deref().and_then(|id| self.agent_cached(id)) else { return div().into_any_element() };
        let (label, tone) = self.agent_status(&a.id, pal);
        let role = match &a.lead {
            Some(l) => format!("Captain \u{b7} {}", l.project.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or("")),
            None => a.job.lines().next().unwrap_or("Crew").chars().take(60).collect(),
        };
        let mut rail = div().id("agent-rail").w(px(264.)).flex_none().h_full().overflow_y_scroll().flex().flex_col().gap(px(2.)).p(px(12.)).border_r_1().border_color(c(pal.line));
        rail = rail.child(
            div()
                .id("agent-head")
                .flex()
                .items_center()
                .gap(px(10.))
                .pb(px(12.))
                .aria_label(format!("{}, {role}, {label}", a.name))
                .child(figure(&a, self.agent_mood(&a.id), 44., pal))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(1.)).child(div().truncate().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(a.name.clone())).child(div().truncate().text_size(px(11.5)).text_color(fg3).child(role)).child(div().text_size(px(11.5)).text_color(tone).child(label)))
                .test_support(),
        );
        let head = |id: &'static str, t: String| div().id(id).pt(px(10.)).pb(px(4.)).px(px(6.)).text_size(px(11.)).text_color(fg3).child(t).test_support();
        let row = |id: (&'static str, usize), on: bool, title: String, meta: String| {
            div()
                .id(id)
                .px(px(8.))
                .py(px(6.))
                .flex()
                .flex_col()
                .gap(px(1.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .when(on, |d| d.bg(c(pal.selected)))
                .hover(|d| d.bg(c(pal.hover)))
                .aria_label(format!("{title}, {meta}"))
                .aria_selected(on)
                .child(div().truncate().text_size(px(12.5)).text_color(fg).child(title))
                .child(div().truncate().text_size(px(11.)).font_features(tnum()).text_color(fg3).child(meta))
        };

        // Which thread is open: talking, or one automation's runs.
        let auto_open = self.thread.as_ref().and_then(|t| a.routines.iter().find(|r| !r.thread.is_empty() && r.thread == t.id)).cloned();
        if let Some(r) = &auto_open {
            let aid = a.id.clone();
            rail = rail.child(
                div()
                    .id("agent-back")
                    .px(px(6.))
                    .py(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(fg2)
                    .hover(|d| d.text_color(fg))
                    .aria_label("Back to talking")
                    .child(glyph("chevron-left", 13., fg3))
                    .child("Back to talking")
                    .on_click(cx.listener(move |this, _, window, cx| this.show_agent(&aid, window, cx)))
                    .test_support(),
            );
            rail = rail.child(head("agent-sessions-head", format!("Runs of {}", r.name)));
        } else {
            rail = rail.child(head("agent-sessions-head", "Sessions".into()));
        }
        let ss = self.thread.as_ref().map(|t| self.open_sessions(t)).unwrap_or_default();
        if ss.is_empty() {
            rail = rail.child(div().px(px(8.)).py(px(4.)).text_size(px(12.)).text_color(fg3).child(if auto_open.is_some() {
                "It hasn't run yet."
            } else if self.thread.is_none() {
                "Nothing yet. Write below, or its captain will give it a job."
            } else {
                "Nothing yet. Write below to start the first session."
            }));
        }
        let shown = self.session_pick.unwrap_or(ss.len().saturating_sub(1));
        if ss.len() > 1 {
            rail = rail.child(row(("agent-session-all", 0), shown == ALL, "All sessions".into(), format!("{} sessions", ss.len())).on_click(cx.listener(|this, _, _, cx| {
                this.session_pick = Some(ALL);
                cx.notify();
            })).test_support());
        }
        let now = backspace_core::chat::now_ms();
        for (k, s) in ss.iter().enumerate().rev() {
            let mins = (s.to.saturating_sub(s.from) / 60_000).max(1);
            let meta = if s.live { format!("Live \u{b7} {}", super::automations::when(s.from)) } else { format!("{} \u{b7} {mins} min", super::automations::when(s.from)) };
            let title = match &s.kind {
                Kind::Automation(n) if auto_open.is_none() => format!("\u{27f3} {n}"),
                _ => s.title.clone(),
            };
            let _ = now;
            rail = rail.child(
                row(("agent-session", k), shown == k, title, meta)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.session_pick = Some(k);
                        cx.notify();
                    }))
                    .test_support(),
            );
        }

        // Its automations (automations.rs): each opens its own runs here.
        if auto_open.is_none() {
            rail = rail.child(head("agent-autos-head", "Automations".into()));
            for (i, r) in a.routines.iter().enumerate() {
                let (aid, t) = (a.id.clone(), (!r.thread.is_empty()).then(|| r.thread.clone()));
                let edit = (a.clone(), r.clone());
                rail = rail.child(
                    row(("agent-auto", i), false, r.name.clone(), r.summary())
                        .on_click(cx.listener(move |this, _, window, cx| match &t {
                            Some(t) => this.open_agent_thread(&aid, Some(t.clone()), cx),
                            None => this.open_automation_form(Some(edit.clone()), window, cx),
                        }))
                        .test_support(),
                );
            }
            if a.lead.is_some() {
                rail = rail.child(
                    div()
                        .id("agent-auto-new")
                        .px(px(8.))
                        .py(px(6.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .rounded(px(radius::CONTROL))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(pal.hover)))
                        .text_size(px(12.5))
                        .text_color(fg2)
                        .aria_label("New automation")
                        .child(glyph("plus", 13., fg3))
                        .child("New automation")
                        .on_click(cx.listener(|this, _, window, cx| this.open_automation_form(None, window, cx)))
                        .test_support(),
                );
            } else if a.routines.is_empty() {
                rail = rail.child(div().px(px(8.)).py(px(4.)).text_size(px(12.)).text_color(fg3).child("Its captain runs the automations."));
            }
        }
        rail.into_any_element()
    }
}
