//! Familiars (Copilots board round 2, Charles 16 Nov): agents you hand a goal to. A familiar
//! plans, opens subagents and runs the stages itself, stopping only for you. Opening one swaps
//! the stream card's body for the old shell's content area: a centred stream card with the
//! Highlights panel beside it, both inside the one rounded card. The card header keeps the tab
//! row: the familiar pinned first (accent, wheel icon, no close), a hairline, then one tab per
//! subagent.
//!
//! ponytail: everything here is a labelled preview. Core has no familiar yet: nothing advances
//! stages on its own (Captain plan), subagents get no worktrees of their own until M6, and asks
//! do not notify. The rows, events and crew are a fixed example, marked "Preview" in the sidebar
//! and in the view, never shown as running work.

use super::*;

/// Where a familiar or one of its subagents stands; drawn as the sidebar's state dots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Running,
    NeedsYou,
    Done,
}

impl State {
    fn label(self) -> &'static str {
        match self {
            State::Running => "Working",
            State::NeedsYou => "Needs you",
            State::Done => "Done",
        }
    }
}

/// A subagent the familiar opened for a ticket.
pub(crate) struct Sub {
    pub(crate) name: &'static str,
    pub(crate) ticket: &'static str,
    pub(crate) state: State,
    /// Its own stream, as event lines.
    pub(crate) log: &'static [&'static str],
}

/// One familiar: its goal, its plan, the events it logged and its crew.
pub(crate) struct Familiar {
    pub(crate) name: &'static str,
    pub(crate) goal: &'static str,
    pub(crate) state: State,
    pub(crate) plan: &'static str,
    pub(crate) events: &'static [(&'static str, &'static str)],
    pub(crate) ask: Option<&'static str>,
    pub(crate) crew: &'static [Sub],
}

/// The preview's example familiars.
pub(crate) const EXAMPLES: [Familiar; 2] = [
    Familiar {
        name: "Ship onboarding v2",
        goal: "Build the 6-step onboarding from the Tight tiles board, behind a setting. Stop before Ship.",
        state: State::NeedsYou,
        plan: "Split into 4 tickets: Welcome + Agents, Machines, Import, Make it yours. Rampart writes the tests first; one Builder per ticket.",
        events: &[
            ("check", "Rampart sealed 14 tests for T1-T4"),
            ("bot", "Opened 4 Builder tabs"),
            ("check", "T1 Welcome + Agents passed 5/5"),
            ("circle-alert", "T3 Import failed 3 times, handed to you"),
        ],
        ask: Some("T3 can't read the legacy chat export: the format is unknown. Skip import for now, or attach a sample file?"),
        crew: &[
            Sub { name: "Rampart", ticket: "Tests for T1-T4", state: State::Done, log: &["Read spec.md and baseline.json", "Wrote 14 tests", "Each failed 3 times on the baseline", "Sealed"] },
            Sub { name: "T1 Builder", ticket: "Welcome + Agents", state: State::Done, log: &["Covers R1-R5", "Try 1: 5/5 pass", "Delivered"] },
            Sub { name: "T2 Builder", ticket: "Machines picker", state: State::Running, log: &["Covers R6", "Try 1: 3/4 pass"] },
            Sub { name: "T3 Builder", ticket: "Import", state: State::NeedsYou, log: &["Covers R7-R9", "Try 1: R7 fails, unknown JSON root", "Try 2: R7 fails, same error", "Try 3: no progress, escalated once", "Handed to you"] },
            Sub { name: "T4 Builder", ticket: "Make it yours", state: State::Running, log: &["Covers R10-R11", "Try 1: 1/2 pass"] },
        ],
    },
    Familiar {
        name: "Cut router latency",
        goal: "Bring the router's p95 under 40 ms on the eval set without losing accuracy.",
        state: State::Running,
        plan: "Profile first, then one ticket per hot spot. No ticket ships without the eval run.",
        events: &[("bot", "Opened 2 Builder tabs"), ("check", "Profile: tokenizer is 61% of p95")],
        ask: None,
        crew: &[
            Sub { name: "T1 Builder", ticket: "Cache the tokenizer", state: State::Running, log: &["Try 1: 2/3 pass"] },
            Sub { name: "T2 Builder", ticket: "Batch keyword scans", state: State::Running, log: &["Reading the router"] },
        ],
    },
];

pub(crate) const PREVIEW_NOTE: &str = "Preview: familiars don't run yet";
/// The familiar's pinned tab: the wheel icon and the gap after it.
pub(crate) const MAIN_ICON_W: f32 = 13.0 + 6.0;

pub(crate) fn dot(id: impl Into<ElementId>, state: State, pal: &'static Palette) -> Stateful<Div> {
    let d = div().id(id).size(px(7.)).flex_none().rounded_full().aria_label(state.label());
    match state {
        // The halo is an absolute 13px disc of tint around the 7px dot (a box shadow draws square), so it
        // takes no room in the row; the tint over the dot itself leaves it the accent.
        State::Running => d.relative().bg(c(pal.accent)).child(div().absolute().left(px(-3.)).top(px(-3.)).size(px(13.)).rounded_full().bg(ca(pal.accent, 0.18))),
        State::NeedsYou => d.bg(c(pal.warn)),
        State::Done => d.border_1().border_color(c(pal.fg4)),
    }
}

/// A small "Preview" tag in the warning colour.
fn preview_tag(id: &'static str, pal: &'static Palette) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(17.))
        .px(px(5.))
        .flex()
        .items_center()
        .rounded(px(radius::TAG))
        .border_1()
        .border_color(ca(pal.warn, 0.6))
        .text_size(px(10.5))
        .text_color(c(pal.warn))
        .aria_label(PREVIEW_NOTE)
        .child("Preview")
}

impl Cauldron {
    /// Opens familiar `i`: its tab first, one tab per subagent, the support card folded.
    pub(crate) fn open_familiar(&mut self, i: usize) {
        let Some(f) = EXAMPLES.get(i) else { return };
        self.familiar = Some(i);
        self.open = None;
        self.thread = None;
        self.selected = None;
        self.settings = None;
        self.coordinator = Some(f.name.into());
        self.agents = f.crew.iter().enumerate().map(|(k, s)| AgentTab { name: s.name.into(), dot: k }).collect();
        self.idle.clear();
        self.active = 0;
        self.tab_first = 0;
        self.drawer = None;
    }

    /// The open familiar's subagent behind tab name `name`.
    pub(crate) fn familiar_sub(&self, name: &str) -> Option<&'static Sub> {
        EXAMPLES.get(self.familiar?)?.crew.iter().find(|s| s.name == name)
    }

    /// The sidebar's Familiars group, above the streams.
    pub(crate) fn familiar_group(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Div {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mut g = div().flex().flex_col().child(
            div()
                .id("familiars-head")
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(34.))
                .flex_none()
                .px(px(8.))
                .pt(px(14.))
                .pb(px(4.))
                .text_size(px(11.5))
                .line_height(px(16.))
                .text_color(fg3)
                .aria_label("Familiars")
                .child("Familiars")
                .child(div().ml_auto().child(preview_tag("familiars-preview", pal).test_support()))
                .test_support(),
        );
        for (i, f) in EXAMPLES.iter().enumerate() {
            let on = self.familiar == Some(i);
            g = g.child(
                div()
                    .id(("familiar", i))
                    .h(px(30.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(8.))
                    .rounded(px(radius::CONTROL))
                    .text_size(px(13.))
                    .text_color(if on { fg } else { fg2 })
                    .when_live(on, |d| d.bg(c(pal.hover)))
                    .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                    .cursor_pointer()
                    .aria_label(f.name)
                    .child(glyph("ship-wheel", 14., c(pal.accent)))
                    .child(div().flex_1().min_w_0().truncate().child(f.name))
                    .child(dot(("familiar-dot", i), f.state, pal).test_support())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_familiar(i);
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        g
    }

    /// The card's body while a familiar is open: the old shell's content area. A centred stream
    /// card (the familiar's own, or the open subagent's), the Highlights panel beside it.
    pub(crate) fn familiar_body(&self, pal: &'static Palette) -> AnyElement {
        let Some(f) = self.familiar.and_then(|i| EXAMPLES.get(i)) else { return div().into_any_element() };
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let sub = (self.active > 0).then(|| self.agents.get(self.active - 1)).flatten().and_then(|a| self.familiar_sub(&a.name));

        let event = |k: usize, icon: &'static str, text: &'static str| {
            let col = if icon == "circle-alert" { c(pal.warn) } else { fg3 };
            div().id(("familiar-event", k)).flex().items_center().gap(px(8.)).text_size(px(12.)).text_color(fg2).aria_label(text).child(glyph(icon, 13., col)).child(text).test_support()
        };
        let label = |t: &'static str| div().text_size(px(10.5)).text_color(fg3).child(t);
        let boxed = |id: &'static str| div().id(id).flex().flex_col().gap(px(4.)).p(px(10.)).rounded(px(radius::CONTROL)).bg(c(pal.inset)).border_1().border_color(c(pal.line));

        let (title, state, head_note, mut col) = match sub {
            None => {
                let mut col = div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(boxed("familiar-goal").aria_label(f.goal).child(label("Goal")).child(div().text_size(px(13.)).text_color(fg).child(f.goal)).test_support())
                    .child(div().id("familiar-plan").flex().flex_col().gap(px(2.)).aria_label(f.plan).child(label("Familiar · plan")).child(div().text_size(px(13.)).text_color(fg).child(f.plan)).test_support());
                for (k, (icon, text)) in f.events.iter().enumerate() {
                    col = col.child(event(k, icon, text));
                }
                if let Some(ask) = f.ask {
                    col = col.child(
                        div()
                            .id("familiar-ask")
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .p(px(10.))
                            .rounded(px(radius::CONTROL))
                            .border_1()
                            .border_color(ca(pal.warn, 0.7))
                            .bg(ca(pal.warn, 0.08))
                            .aria_label(ask)
                            .child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Needs you"))
                            .child(div().text_size(px(12.5)).text_color(fg2).child(ask))
                            .test_support(),
                    );
                }
                let n = f.crew.len();
                (f.name, f.state, format!("coordinating {n} subagents"), col)
            }
            Some(s) => {
                let mut col = div().flex().flex_col().gap(px(12.)).child(boxed("sub-ticket").aria_label(s.ticket).child(label("Ticket from the familiar")).child(div().text_size(px(13.)).text_color(fg).child(s.ticket)).test_support());
                for (k, text) in s.log.iter().enumerate() {
                    col = col.child(event(k, "bot", text));
                }
                (s.name, s.state, "reports to the familiar".to_string(), col)
            }
        };
        col = col.w_full().max_w(px(560.));

        let head = div()
            .id("familiar-head")
            .h(px(36.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .border_b_1()
            .border_color(c(pal.line))
            .child(glyph(if sub.is_some() { "bot" } else { "ship-wheel" }, 14., if sub.is_some() { fg3 } else { c(pal.accent) }))
            .child(div().id("familiar-title").min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).text_size(px(12.5)).text_color(fg).aria_label(title).child(title).test_support())
            .child(dot("familiar-state", state, pal).test_support())
            .child(div().text_size(px(11.)).text_color(fg3).child(state.label()))
            .child(div().ml_auto().text_size(px(11.)).text_color(fg3).child(head_note))
            .child(preview_tag("familiar-preview", pal).test_support());
        let composer_hint = if sub.is_some() { "Messaging a subagent comes with core's familiars" } else { "Steering a familiar comes with core's familiars" };
        let composer = div()
            .id("familiar-composer")
            .w_full()
            .max_w(px(560.))
            .flex_none()
            .p(px(10.))
            .rounded(px(10.))
            .border_1()
            .border_color(c(pal.line))
            .bg(c(pal.inset))
            .text_size(px(12.5))
            .text_color(fg3)
            .aria_label(composer_hint)
            .child(composer_hint)
            .test_support();
        let stream = div()
            .id("familiar-stream")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(radius::PANEL))
            .bg(c(pal.panel))
            .border_1()
            .border_color(c(pal.line))
            .child(head.test_support())
            .child(div().id("familiar-scroll").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().items_center().px(px(16.)).py(px(16.)).child(col))
            .child(div().flex().justify_center().px(px(16.)).pb(px(12.)).child(composer))
            .test_support();

        // Highlights (the old shell's right panel): what this run is waiting on and what it spent.
        let done = f.crew.iter().filter(|s| s.state == State::Done && s.name != "Rampart").count();
        let tickets = f.crew.iter().filter(|s| s.name != "Rampart").count();
        let kv = |k: &'static str, v: String| div().flex().justify_between().gap(px(8.)).text_size(px(12.)).text_color(fg2).child(k).child(div().text_color(fg).font_features(tnum()).child(v));
        let hl = div()
            .id("familiar-highlights")
            .w(px(260.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(8.))
            .py(px(8.))
            .aria_label("Highlights")
            .child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Gates"))
            .child(kv("Plan", "approved".into()))
            .child(kv("Execute", format!("{done} of {tickets} tickets")))
            .child(div().h(px(5.)).rounded(px(3.)).bg(c(pal.inset)).child(div().h_full().rounded(px(3.)).bg(c(pal.accent)).w(relative(if tickets == 0 { 0.0 } else { done as f32 / tickets as f32 }))))
            .child(kv("Ship", "waits for you".into()))
            .child(div().mt(px(6.)).text_size(px(11.5)).line_height(px(16.)).text_color(fg3).child("A preview: nothing here runs yet. Familiars need stages that advance on their own and a sandbox per subagent (M6)."))
            .test_support();

        div().id("familiar").size_full().flex().gap(px(6.)).p(px(6.)).bg(c(pal.card)).child(stream).child(hl).test_support().into_any_element()
    }
}
