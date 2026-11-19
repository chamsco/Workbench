//! Familiars (Copilots board rounds 2-4, Charles 16-17 Nov): agents you hand a goal to. A familiar
//! plans, opens subagents and runs the stages itself, stopping only for you. Opening one swaps
//! the stream card's body for the old shell's content area: a centred stream card with the
//! Highlights panel beside it, both inside the one rounded card. Round 3 (T1): the tabs are folder
//! tabs on the stream card's top edge, from its left edge to its right, with no title row: the
//! familiar first (accent tint, wheel, never collapses), then one slim tab per subagent. The open
//! tab joins the card (its fill, rounded top, concave shoulders) and slides between tabs.
//! Highlights keeps its own tabs on the same line.
//!
//! Round 4 (17 Nov): familiars run on core. Each is a harness on its folder, started on the long
//! path (core `familiar.rs`); the view is `familiar::view` of its state, refreshed on every change,
//! and every change also takes `familiar::drive` off the UI thread, which carries a finished run
//! through Verify, Ship and Learn. The stream ends on the delivery card (D2), or on an ask card
//! when it waits for you. A familiar starts from the New stream box with Familiar on (S1).

use super::*;
use backspace_core::deliver::Target;
use backspace_core::familiar::{self as core_fam, EventKind, Record, View};
use backspace_core::platform::events::Kind;
use backspace_core::platform::notify::{Inbox, Note};
use gpui_kit::prelude::FluentBuilder;
pub(crate) use backspace_core::familiar::State;
use backspace_core::phase::{Phase, Stage};
use backspace_core::Harness;

/// A familiar's stream fill and the frame around it: solid on the plain frame, glass over the
/// painting (the frame is then the card's own frost).
pub(crate) fn fam_layers(painting: bool, pal: &Palette) -> (Hsla, Hsla) {
    if painting { (super::backdrop::surface(true, pal.panel, 0.5), gpui_kit::transparent_black()) } else { (c(pal.panel), c(pal.card)) }
}

pub(crate) fn state_text(s: State) -> &'static str {
    state_label(s)
}

fn state_label(s: State) -> &'static str {
    match s {
        State::Running => "Working",
        State::NeedsYou => "Needs you",
        State::Done => "Done",
        State::Failed => "Failed",
    }
}

/// A familiar this window runs: its record and its harness.
pub(crate) struct FamLive {
    pub(crate) rec: Record,
    pub(crate) h: Arc<Harness>,
}

/// Debug and test builds: two made-up familiars for screenshots and layout tests
/// (`CAULDRON_FIXTURE_FAMILIARS=1`). Marked Preview wherever they show.
pub(crate) fn fixtures() -> Vec<View> {
    use backspace_core::familiar::{Crew, Event};
    let ev = |k: EventKind, t: &str| Event { kind: k, text: t.into() };
    let crew = |n: &str, t: &str, s: State, log: &[&str]| Crew { name: n.into(), ticket: t.into(), state: s, log: log.iter().map(|l| l.to_string()).collect() };
    vec![
        View {
            id: "fx-1".into(),
            name: "Ship onboarding v2".into(),
            goal: "Build the 6-step onboarding from the Tight tiles board, behind a setting.".into(),
            state: State::NeedsYou,
            stage: Phase::Execute,
            plan: Some("Onboarding v2".into()),
            events: vec![ev(EventKind::Done, "Rampart sealed 14 tests for T1-T4"), ev(EventKind::Agent, "Opened 4 Builder tabs"), ev(EventKind::Done, "T1 Welcome + Agents passed 5/5"), ev(EventKind::Alert, "T3 Import failed 3 times, handed to you")],
            ask: Some("T3 can't read the legacy chat export: the format is unknown. Skip import for now, or attach a sample file?".into()),
            yes_reply: None,
            approval: None,
            crew: vec![
                crew("Rampart", "Tests for T1-T4", State::Done, &["Read spec.md and baseline.json", "Wrote 14 tests", "Sealed"]),
                crew("T1 Builder", "Welcome + Agents", State::Done, &["Covers R1-R5", "Try 1: 5/5 pass", "Delivered"]),
                crew("T2 Builder", "Machines picker", State::Running, &["Covers R6", "Try 1: 3/4 pass"]),
                crew("T3 Builder", "Import", State::NeedsYou, &["Covers R7-R9", "Try 1: R7 fails", "Handed to you"]),
                crew("T4 Builder", "Make it yours", State::Running, &["Covers R10-R11"]),
            ],
            verify: None,
            delivery: None,
            cost_usd: 1.84,
            tickets_done: 1,
            tickets: 4,
        },
        View {
            id: "fx-2".into(),
            name: "Add CSV export".into(),
            goal: "Export the report table as CSV from the CLI, with a header row and RFC 4180 quoting.".into(),
            state: State::Done,
            stage: Phase::Done,
            plan: Some("Add CSV export".into()),
            events: vec![ev(EventKind::Done, "Plan passed: approved by you"), ev(EventKind::Done, "Execute passed: every ticket finished")],
            ask: None,
            yes_reply: None,
            approval: None,
            crew: vec![crew("T1 Builder", "CSV writer", State::Done, &["Delivered"])],
            verify: Some(backspace_core::deliver::VerifyReport {
                passed: true,
                head: "3f9a1c2d7e".into(),
                base: "1b2c3d4e5f".into(),
                criteria: vec![
                    backspace_core::deliver::CriterionResult { key: "A1".into(), criterion: "Header row".into(), command: "cargo test csv_header".into(), passed: true, ..Default::default() },
                    backspace_core::deliver::CriterionResult { key: "A2".into(), criterion: "Quoting".into(), command: "cargo test csv_quote".into(), passed: true, ..Default::default() },
                ],
                ..Default::default()
            }),
            delivery: Some(backspace_core::deliver::DeliveryRecord { title: "Add CSV export".into(), target: Target::PullRequest, commit: "3f9a1c2d7e".into(), branch: "cauldron/add-csv-export-3f9a1c2".into(), status: "delivered".into(), pr_url: Some("https://github.com/o/r/pull/52".into()), evidence: vec![], limitations: vec!["awaiting review and merge by a person".into()], at: 0 }),
            cost_usd: 0.92,
            tickets_done: 1,
            tickets: 1,
        },
    ]
}

/// The folder tabs' strip: its height (the open tab's), an inactive tab's height under it, and the gap between tabs.
const STRIP_H: f32 = 30.0;
const FTAB_H: f32 = 28.0;
const FTAB_GAP: f32 = 2.0;
/// Subagent tabs: robot, short name, dot, between these widths.
const SUB_MIN: f32 = 64.0;
const SUB_MAX: f32 = 108.0;
/// The "+N" chip that stands for the subagent tabs that don't fit.
const MORE_W: f32 = 30.0;
/// Highlights' own tabs, over its column.
pub(crate) const HL_W: f32 = 260.0;
const HL_TABS: [&str; 3] = ["Gates", "Changes", "Budget"];
/// The body's padding and the gap between the stream card and Highlights.
const BODY_PAD: f32 = 6.0;

/// What `familiar_strips` hands the shell: the two tab lines, and for the stream card its
/// top-left corner (square under the first tab) and the fade of a newly opened tab's content.
pub(crate) struct FamStrips {
    pub(crate) strip: Stateful<Div>,
    pub(crate) hl_tabs: Stateful<Div>,
    pub(crate) corner: f32,
    pub(crate) fade: f32,
}

/// A subagent's tab label: its first word ("T3" for "T3 Builder"); the full name is the tip.
fn short(name: &str) -> &str {
    name.split_whitespace().next().unwrap_or(name)
}

/// Where the stream card sits in the body area at `x` `w` wide: its x and width; Highlights follows it.
pub(crate) fn stream_span(x: f32, body_w: f32) -> (f32, f32) {
    (x + BODY_PAD, body_w - 3.0 * BODY_PAD - HL_W)
}

pub(crate) fn dot(id: impl Into<ElementId>, state: State, pal: &'static Palette) -> Stateful<Div> {
    let d = div().id(id).size(px(7.)).flex_none().rounded_full().aria_label(state_label(state));
    match state {
        // The halo is an absolute 13px disc of tint around the 7px dot (a box shadow draws square), so it
        // takes no room in the row; the tint over the dot itself leaves it the accent.
        State::Running => d.relative().bg(c(pal.accent)).child(div().absolute().left(px(-3.)).top(px(-3.)).size(px(13.)).rounded_full().bg(ca(pal.accent, 0.18))),
        State::NeedsYou => d.bg(c(pal.warn)),
        State::Done => d.border_1().border_color(c(pal.fg4)),
        State::Failed => d.bg(c(pal.bad)),
    }
}

/// A small "Preview" tag in the warning colour: made-up familiars (screenshots, tests).
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
        .aria_label("Preview: made-up familiars")
        .child("Preview")
}

/// The five stages on the foot, the current one in the accent.
fn stage_line(stage: Phase, pal: &'static Palette) -> Stateful<Div> {
    let now = stage.stage();
    let done = stage == Phase::Done;
    let mut d = div().id("fam-stages").flex().gap(px(12.)).aria_label(if done { "Stage: done".to_string() } else { format!("Stage: {}", now.key()) });
    for st in Stage::ALL {
        let label = match st {
            Stage::Plan => "Plan",
            Stage::Execute => "Execute",
            Stage::Verify => "Verify",
            Stage::Ship => "Ship",
            Stage::Learn => "Learn",
        };
        let col = if done || st < now { c(pal.ok) } else if st == now { c(pal.accent) } else { c(pal.fg3) };
        d = d.child(div().text_color(col).child(label));
    }
    d
}

impl Cauldron {
    /// Opens familiar `i`: its tab first, one tab per subagent, the support card folded.
    pub(crate) fn open_familiar(&mut self, i: usize) {
        let Some(f) = self.fam_views.get(i) else { return };
        self.familiar = Some(i);
        self.open = None;
        self.thread = None;
        self.selected = None;
        self.settings = None;
        self.coordinator = Some(f.name.clone().into());
        self.agents = f.crew.iter().enumerate().map(|(k, s)| AgentTab { name: s.name.clone().into(), dot: k }).collect();
        self.idle.clear();
        self.active = 0;
        self.tab_first = 0;
        self.drawer = None;
        self.fam_to = None;
        self.fam_since = None;
        self.fam_hl = 0;
        self.fam_error = None;
        self.ask_open = None;
        self.ask_hover = false;
    }

    /// New subagents get a tab as they appear; the open tab stays where it is.
    fn sync_familiar_tabs(&mut self) {
        let Some(f) = self.familiar.and_then(|i| self.fam_views.get(i)) else { return };
        for (k, s) in f.crew.iter().enumerate() {
            let name: SharedString = s.name.clone().into();
            if !self.agents.iter().chain(&self.idle).any(|a| a.name == name) {
                self.agents.push(AgentTab { name, dot: k });
            }
        }
    }

    /// The familiar's folder tabs over its stream card and Highlights' tabs over its column, both
    /// on the line above the card body. `main` is the card, `rail_here` whether the rail is in it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn familiar_strips(&mut self, main: Rect, rail_here: bool, pal: &'static Palette, now: std::time::Instant, reduce: bool, window: &mut Window, cx: &mut Context<Self>) -> FamStrips {
        let Some(f) = self.familiar.and_then(|i| self.fam_views.get(i)).cloned() else { return FamStrips { strip: div().id("fam-strip"), hl_tabs: div().id("fam-hl-tabs"), corner: radius::PANEL, fade: 1.0 } };
        let (fg, fg3, accent) = (c(pal.fg), c(pal.fg3), c(pal.accent));
        let body_w = main.w - 2.0 - if rail_here { metric::RAIL_W } else { 0.0 };
        let (sx, sw) = stream_span(main.x + 1.0, body_w);
        let sy = main.y + 1.0 + metric::HEAD_H - STRIP_H;
        let measure = |t: &str, weight: FontWeight| {
            let run = TextRun { len: t.len(), font: Font { weight, features: sans_features(), ..font(SANS) }, color: fg, background_color: None, underline: None, strikethrough: None };
            f32::from(window.text_system().shape_line(SharedString::from(t.to_string()), px(12.), &[run], None).width)
        };
        // The familiar's tab: wheel, "Familiar", dot; the subagents': robot, short name, dot.
        let fam_w = (9.0 + 13.0 + 6.0 + measure("Familiar", FontWeight::SEMIBOLD) + 6.0 + 7.0 + 12.0).ceil();
        let widths: Vec<f32> = self.agents.iter().map(|a| (10.0 + 13.0 + 6.0 + measure(short(&a.name), FontWeight::MEDIUM) + 6.0 + 7.0 + 10.0).ceil().clamp(SUB_MIN, SUB_MAX)).collect();
        let n = widths.len();
        // The window of subagent tabs that fits after the familiar's, from `first`; the rest are "+N".
        let fits = |first: usize| {
            let all: f32 = widths.iter().map(|w| w + FTAB_GAP).sum();
            if fam_w + all <= sw {
                return n - first.min(n);
            }
            let (mut x, mut k) = (fam_w + FTAB_GAP, 0);
            for w in &widths[first.min(n)..] {
                if x + w + FTAB_GAP + MORE_W > sw {
                    break;
                }
                x += w + FTAB_GAP;
                k += 1;
            }
            k.max(1).min(n - first.min(n))
        };
        let mut first = if fits(0) == n { 0 } else { self.tab_first.min(n.saturating_sub(1)) };
        if self.active > 0 {
            let a = self.active - 1;
            if a < first {
                first = a;
            }
            while a >= first + fits(first) && first < a {
                first += 1;
            }
        }
        self.tab_first = first;
        let k = fits(first);

        // Each tab's x in the strip; the open one is where the joined shape heads.
        let mut xs = vec![0.0f32];
        let mut x = fam_w + FTAB_GAP;
        for w in &widths[first..first + k] {
            xs.push(x);
            x += w + FTAB_GAP;
        }
        let target = if self.active == 0 { (0.0, fam_w) } else if (first..first + k).contains(&(self.active - 1)) { (xs[self.active - first], widths[self.active - 1]) } else { (0.0, fam_w) };
        let dur = cauldron_chrome::motion::time::INDICATOR;
        let shown = |me: &Self| match me.fam_to {
            Some(to) => (crate::motion::ease_between(me.fam_from.0, to.0, me.fam_since, now, dur), crate::motion::ease_between(me.fam_from.1, to.1, me.fam_since, now, dur)),
            None => target,
        };
        if self.fam_to.is_some_and(|to| to != target) {
            // Starts from where the shape is drawn now, so a quick second click continues the move.
            self.fam_from = shown(self);
            self.fam_since = (!reduce).then_some(now);
            self.fam_to = Some(target);
            if reduce {
                self.fam_from = target;
            }
        } else if self.fam_to.is_none() {
            self.fam_to = Some(target);
        }
        let (jx, jw) = shown(self);
        let k_run = self.fam_since.map_or(1.0, |t0| crate::motion::run(now, t0, dur));
        if k_run < 1.0 {
            window.request_animation_frame();
        } else {
            self.fam_since = None;
        }

        // The joined tab: the card's fill and hairline on three sides, rounded top corners, 1px into
        // the card to cover its top line; concave shoulders where it meets the card's top edge,
        // none past the card's own edges.
        let r = radius::PANEL;
        let (l_sh, r_sh) = (jx.clamp(0.0, r), (sw - jx - jw).clamp(0.0, r));
        let line = c(pal.line);
        // Over the painting both layers are glass: the stream's fill translucent, the frame around
        // it the card's own frost. Translucent layers cannot overpaint, so the concave shoulders
        // are left out there; the plain frame keeps them.
        let (fill, frame) = fam_layers(self.painting, pal);
        let mut strip = abs(Rect::new(sx, sy, sw, STRIP_H))
            .id("fam-strip")
            .track_focus(&self.tab_focus.get_or_insert_with(|| cx.focus_handle()).clone())
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                let last = this.agents.len();
                match ev.keystroke.key.as_str() {
                    "left" => this.active = this.active.saturating_sub(1),
                    "right" => this.active = (this.active + 1).min(last),
                    "home" => this.active = 0,
                    "end" => this.active = last,
                    "delete" if this.active > 0 => this.close_agent(this.active - 1),
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(div().id("fam-joined").absolute().left(px(jx)).top_0().w(px(jw)).h(px(STRIP_H + 1.0)).bg(fill).border_t_1().border_l_1().border_r_1().border_color(line).rounded_t(px(r)).test_support());
        if l_sh > 0.5 && !self.painting {
            strip = strip.child(div().absolute().left(px(jx + 1.0 - l_sh)).top(px(STRIP_H + 1.0 - l_sh)).size(px(l_sh)).bg(fill).child(div().size_full().bg(frame).border_r_1().border_b_1().border_color(line).rounded_br(px(l_sh))));
        }
        if r_sh > 0.5 && !self.painting {
            strip = strip.child(div().absolute().left(px(jx + jw - 1.0)).top(px(STRIP_H + 1.0 - r_sh)).size(px(r_sh)).bg(fill).child(div().size_full().bg(frame).border_l_1().border_b_1().border_color(line).rounded_bl(px(r_sh))));
        }

        // Each tab sits in an absolute slot: the tip makes the tab itself `relative`.
        let slot = |x: f32| div().absolute().left(px(x)).top(px(STRIP_H - FTAB_H));
        let tab = |id: (&'static str, usize), w: f32, on: bool| {
            div()
                .id(id)
                .w(px(w))
                .h(px(FTAB_H))
                .flex()
                .items_center()
                .gap(px(6.))
                .rounded_t(px(r))
                .occlude()
                .cursor_pointer()
                .aria_selected(on)
                .text_size(px(12.))
                .line_height(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if on { fg } else { fg3 })
        };
        let fam_on = self.active == 0;
        let fam_tab = tab(("tab", 0), fam_w, fam_on)
            .pl(px(9.))
            .pr(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(accent)
            .when_live(!fam_on, |d| d.bg(ca(pal.accent, 0.12)))
            .aria_label(f.name.clone())
            .child(glyph("ship-wheel", 13., accent))
            .child("Familiar")
            .child(dot(("tab-state", 0usize), f.state, pal).test_support())
            .on_click(cx.listener(|this, _, window, cx| {
                this.active = 0;
                this.focus_tabs(window, cx);
                cx.notify();
            }));
        strip = strip.child(slot(0.0).child(self.tipped(("tab", 0usize), fam_tab, f.name.clone(), Side::Below, pal, cx).test_support()));
        for (s, j) in (first..first + k).enumerate() {
            let i = j + 1;
            let on = i == self.active;
            let name = self.agents[j].name.clone();
            let group: SharedString = format!("ftab{i}").into();
            let close = div()
                .id(("tab-close", i))
                .absolute()
                .right(px(4.))
                .top(px((FTAB_H - 18.0) / 2.0))
                .size(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::TAG))
                .bg(if on { fill } else { frame })
                .opacity(0.)
                .group_hover(group.clone(), |s| s.opacity(1.))
                .hover(|d| d.bg(ca(pal.fg, 0.08)))
                .aria_label("Close tab")
                .child(glyph("x", 12., fg3))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.close_agent(j);
                    cx.notify();
                }))
                .test_support();
            let t = tab(("tab", i), widths[j], on)
                .group(group)
                .px(px(10.))
                .when_live(!on, |d| d.hover(|d| d.bg(ca(pal.fg, 0.05))))
                .aria_label(name.clone())
                .child(glyph("bot", 13., fg3))
                .child(div().min_w_0().truncate().child(SharedString::from(short(&name).to_string())))
                .children(self.familiar_sub(&name).map(|sub| dot(("tab-state", i), sub.state, pal).test_support()))
                .child(close)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.active = i;
                    this.focus_tabs(window, cx);
                    cx.notify();
                }));
            strip = strip.child(slot(xs[s + 1]).child(self.tipped(("tab", i), t, name, Side::Below, pal, cx).test_support()));
        }
        if k < n {
            let more: SharedString = format!("+{}", n - k).into();
            let m = tab(("fam-more", 0), MORE_W, false)
                .justify_center()
                .font_family(MONO)
                .text_size(px(11.))
                .hover(|d| d.bg(ca(pal.fg, 0.05)))
                .aria_label(more.clone())
                .child(more)
                .on_click(cx.listener(move |this, _, _, cx| {
                    // The next hidden tab comes into view; past the last the window starts over.
                    this.tab_first = if first + k < n { first + 1 } else { 0 };
                    cx.notify();
                }));
            strip = strip.child(slot(x).child(m.test_support()));
        }

        // Highlights' tabs, over its column on the same line.
        let mut hl_tabs = abs(Rect::new(sx + sw + BODY_PAD, sy, HL_W, STRIP_H)).id("fam-hl-tabs").flex().items_end().gap(px(2.)).pb(px(2.)).px(px(4.));
        // The option opened from the decision card gets its own tab here, first so it
        // never sits under the window buttons, while its question waits.
        if self.ask_open.is_some() && self.fam_question().is_some() {
            let on = self.fam_hl == super::decide::OPTION_TAB;
            hl_tabs = hl_tabs.child(
                div()
                    .id("fam-hl-option")
                    .h(px(26.))
                    .px(px(9.))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .rounded(px(radius::CONTROL))
                    .occlude()
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(if on { fg } else { fg3 })
                    .when_live(on, |d| d.bg(fill))
                    .when_live(!on, |d| d.hover(|d| d.bg(ca(pal.fg, 0.05))))
                    .aria_selected(on)
                    .aria_label("Option")
                    .child(div().size(px(6.)).rounded_full().bg(c(pal.warn)))
                    .child("Option")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.fam_hl = super::decide::OPTION_TAB;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }

        for (i, name) in HL_TABS.into_iter().enumerate() {
            let on = self.fam_hl == i;
            hl_tabs = hl_tabs.child(
                div()
                    .id(("fam-hl", i))
                    .h(px(26.))
                    .px(px(9.))
                    .flex()
                    .items_center()
                    .rounded(px(radius::CONTROL))
                    .occlude()
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(if on { fg } else { fg3 })
                    .when_live(on, |d| d.bg(fill))
                    .when_live(!on, |d| d.hover(|d| d.bg(ca(pal.fg, 0.05))))
                    .aria_selected(on)
                    .aria_label(name)
                    .child(name)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.fam_hl = i;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }

        // The content of a newly opened tab fades in with the move; with less motion it just swaps.
        let fade = k_run;
        FamStrips { strip, hl_tabs, corner: l_sh, fade }
    }

    /// The open familiar's subagent behind tab name `name`.
    pub(crate) fn familiar_sub(&self, name: &str) -> Option<&backspace_core::familiar::Crew> {
        self.fam_views.get(self.familiar?)?.crew.iter().find(|s| s.name == name)
    }

    /// Opens the familiars saved in the data folder, each on its own harness, off the UI thread.
    /// Debug builds with `CAULDRON_FIXTURE_FAMILIARS=1` show the made-up ones instead.
    pub(crate) fn load_familiars(&mut self, cx: &mut Context<Self>) {
        self.load_inbox(cx);
        #[cfg(debug_assertions)]
        if std::env::var_os("CAULDRON_FIXTURE_FAMILIARS").is_some() {
            self.fam_views = fixtures();
            self.fam_fixture = true;
            return;
        }
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let opened = cx
                .background_executor()
                .spawn(async {
                    core_fam::load(&Prefs::data_dir())
                        .into_iter()
                        .filter_map(|rec| match Harness::open(rec.folder.clone()) {
                            Ok(h) => Some(FamLive { rec, h: Arc::new(h) }),
                            Err(e) => {
                                eprintln!("familiar {}: {e:#}", rec.name);
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |c, cx| {
                for f in opened {
                    c.adopt_familiar(f, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Takes a running familiar into the window: its view now, and a watch that refreshes the view
    /// and drives the run on every change.
    pub(crate) fn adopt_familiar(&mut self, f: FamLive, cx: &mut Context<Self>) {
        let changes = f.h.changes();
        let snap = f.h.snapshot();
        let v = core_fam::view(&f.rec, &snap, f.h.root());
        self.fam_questions.insert(f.rec.id.clone(), super::decide::pending(&snap.options).cloned());
        let id = f.rec.id.clone();
        self.fam_views.push(v);
        self.fam_live.push(f);
        self.drive_familiar(&id, cx);
        let task = cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while changes.recv().await.is_ok() {
                if this
                    .update(cx, |c, cx| {
                        c.refresh_familiar(&id, cx);
                        c.drive_familiar(&id, cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        self.fam_watch.push(task);
    }

    /// The view again from the harness; what is news to a person (a new ask, a delivery, a failure)
    /// goes to the platform's notification inbox, off the UI thread.
    fn refresh_familiar(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(k) = self.fam_live.iter().position(|f| f.rec.id == id) else { return };
        let f = &self.fam_live[k];
        let snap = f.h.snapshot();
        let v = core_fam::view(&f.rec, &snap, f.h.root());
        self.fam_questions.insert(id.to_string(), super::decide::pending(&snap.options).cloned());
        let before = self.fam_views.iter().find(|x| x.id == id);
        if let Some(e) = core_fam::event_for(before, &v) {
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                // Quiet hours are not a setting yet: every note alerts.
                let filed = cx.background_executor().spawn(async move { Inbox::open(&Prefs::data_dir()).push(e, None, 12) }).await;
                match filed {
                    Ok(Some(n)) => {
                        let _ = this.update(cx, |c, cx| c.take_note(n, cx));
                    }
                    Ok(None) => {}
                    Err(err) => eprintln!("notification inbox: {err:#}"),
                }
            })
            .detach();
        }
        if let Some(slot) = self.fam_views.iter_mut().find(|x| x.id == id) {
            *slot = v;
        }
        if self.familiar.and_then(|i| self.fam_views.get(i)).is_some_and(|x| x.id == id) {
            self.sync_familiar_tabs();
        }
    }

    /// Verify, Ship and Learn run git, the plan's commands and gh: never on the UI thread. One
    /// drive at a time per familiar; the next change starts the next.
    fn drive_familiar(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.fam_driving.contains(id) {
            return;
        }
        let Some(f) = self.fam_live.iter().find(|f| f.rec.id == id) else { return };
        let snap = f.h.snapshot();
        let main = snap.agents.first().map(|a| a.status);
        let Some(main) = main.filter(|s| *s == backspace_core::project::AgentStatus::Approved) else { return };
        let (h, target, id) = (f.h.clone(), f.rec.target, id.to_string());
        self.fam_driving.insert(id.clone());
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let res = cx
                .background_executor()
                .spawn(async move {
                    let facts: Vec<_> = snap.tickets.iter().map(backspace_core::deliver::TicketFacts::of).collect();
                    let data = Prefs::data_dir();
                    let memory = backspace_core::memory::Memory::open(data.clone());
                    let mut learn = || core_fam::learn_with_rules(&h, &memory, &data);
                    core_fam::drive(h.root(), main, &facts, target, &mut learn)
                })
                .await;
            let _ = this.update(cx, |c, cx| {
                c.fam_driving.remove(&id);
                if let Err(e) = res {
                    c.fam_error = Some(format!("{e:#}"));
                }
                c.refresh_familiar(&id, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Starts a familiar from the New stream box (S1): remembered in the data folder, its harness
    /// opened on the folder off the UI thread, the goal handed over on the long path, then opened.
    pub(crate) fn start_familiar(&mut self, goal: String, folder: String, cx: &mut Context<Self>) {
        let target = Target::parse(&Prefs::load().ship_target).unwrap_or_default();
        self.start.sending = true;
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let res = cx
                .background_executor()
                .spawn(async move {
                    let rec = core_fam::add(&Prefs::data_dir(), &goal, std::path::Path::new(&folder), target)?;
                    let h = match Harness::open(rec.folder.clone()) {
                        Ok(h) => h,
                        Err(e) => {
                            let _ = core_fam::remove(&Prefs::data_dir(), &rec.id);
                            return Err(e);
                        }
                    };
                    h.start_long(goal);
                    anyhow::Ok(FamLive { rec, h: Arc::new(h) })
                })
                .await;
            let _ = this.update(cx, |c, cx| {
                c.start.sending = false;
                match res {
                    Ok(f) => {
                        c.adopt_familiar(f, cx);
                        c.start.familiar = false;
                        c.open_familiar(c.fam_views.len() - 1);
                    }
                    Err(e) => c.start.error = Some(format!("{e:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The open familiar's harness, if it is a live one.
    pub(super) fn open_live(&self) -> Option<&FamLive> {
        let v = self.fam_views.get(self.familiar?)?;
        self.fam_live.iter().find(|f| f.rec.id == v.id)
    }

    /// Sends the familiar's composer text to its main agent.
    fn send_familiar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((st, _)) = self.fam_composer.as_ref() else { return };
        let text = st.read(cx).value().trim().to_string();
        if text.is_empty() {
            return;
        }
        match self.open_live() {
            Some(f) => {
                f.h.send(text);
                self.fam_error = None;
                st.update(cx, |s, cx| s.set_value("", window, cx));
            }
            None => self.fam_error = Some("This is a made-up familiar: nothing to send to.".into()),
        }
        cx.notify();
    }

    pub(crate) fn ensure_fam_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fam_composer.is_some() {
            return;
        }
        let st = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 6).submit_on_enter(true).placeholder("Message the familiar…"));
        let sub = cx.subscribe_in(&st, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { shift: false, .. } = ev {
                this.send_familiar(window, cx);
            }
        });
        self.fam_composer = Some((st, sub));
    }

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
                .when_live(self.fam_fixture, |d| d.child(div().ml_auto().child(preview_tag("familiars-preview", pal).test_support())))
                .test_support(),
        );
        if self.fam_views.is_empty() {
            g = g.child(
                div()
                    .id("familiars-empty")
                    .px(px(8.))
                    .pb(px(4.))
                    .text_size(px(11.5))
                    .line_height(px(16.))
                    .text_color(fg3)
                    .aria_label("No familiars yet")
                    .child("Start one from New stream with Familiar on.")
                    .test_support(),
            );
        }
        for (i, f) in self.fam_views.iter().enumerate() {
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
                    .aria_label(f.name.clone())
                    .child(glyph("ship-wheel", 14., c(pal.accent)))
                    .child(div().flex_1().min_w_0().truncate().child(f.name.clone()))
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
    /// card (the familiar's own, or the open subagent's) right under its folder tabs, with no title
    /// row, and the Highlights panel beside it. `corner` is the card's top-left radius (square under
    /// the first tab) and `fade` the new tab's content fading in.
    pub(crate) fn familiar_body(&self, pal: &'static Palette, (corner, fade): (f32, f32), cx: &mut Context<Self>) -> AnyElement {
        let Some(f) = self.familiar.and_then(|i| self.fam_views.get(i)) else { return div().into_any_element() };
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let sub = (self.active > 0).then(|| self.agents.get(self.active - 1)).flatten().and_then(|a| self.familiar_sub(&a.name));
        let live = self.open_live().is_some();

        let event = |k: usize, kind: EventKind, text: String| {
            let (icon, col) = match kind {
                EventKind::Done => ("check", fg3),
                EventKind::Agent => ("bot", fg3),
                EventKind::Alert => ("circle-alert", c(pal.warn)),
                EventKind::Note => ("message-square", fg3),
            };
            div().id(("familiar-event", k)).flex().items_start().gap(px(8.)).text_size(px(12.)).line_height(px(17.)).text_color(fg2).aria_label(text.clone()).child(div().pt(px(2.)).child(glyph(icon, 13., col))).child(div().flex_1().min_w_0().child(text)).test_support()
        };
        let label = |t: String| div().text_size(px(10.5)).text_color(fg3).child(t);
        let boxed = |id: &'static str| div().id(id).flex().flex_col().gap(px(4.)).p(px(10.)).rounded(px(radius::CONTROL)).bg(c(pal.inset)).border_1().border_color(c(pal.line));
        let button = |id: &'static str, text: &'static str, primary: bool| {
            div()
                .id(id)
                .h(px(26.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(radius::CONTROL))
                .text_size(px(12.))
                .cursor_pointer()
                .aria_label(text)
                .when_live(primary, |d| d.bg(c(pal.accent)).text_color(c(pal.on_accent)))
                .when_live(!primary, |d| d.border_1().border_color(c(pal.line)).text_color(fg2).hover(|d| d.bg(ca(pal.fg, 0.05))))
                .child(text)
        };

        let (state, note, mut col) = match sub {
            None => {
                let mut col = div().flex().flex_col().gap(px(12.)).child(boxed("familiar-goal").aria_label(f.goal.clone()).child(label(format!("Goal · {}", f.name))).child(div().text_size(px(13.)).text_color(fg).child(f.goal.clone())).test_support());
                if let Some(p) = &f.plan {
                    col = col.child(div().id("familiar-plan").flex().flex_col().gap(px(2.)).aria_label(p.clone()).child(label("Familiar · plan".into())).child(div().text_size(px(13.)).text_color(fg).child(p.clone())).test_support());
                }
                for (k, e) in f.events.iter().enumerate() {
                    col = col.child(event(k, e.kind, e.text.clone()));
                }
                // D2: the run ends on the delivery card; Learn's line follows it.
                if let Some(d) = &f.delivery {
                    let mut card = div()
                        .id("familiar-delivery")
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .p(px(12.))
                        .rounded(px(radius::CONTROL))
                        .border_1()
                        .border_color(c(pal.line))
                        .bg(ca(pal.accent, 0.08))
                        .aria_label(format!("Delivered: {}", d.pr_url.clone().unwrap_or_else(|| d.branch.clone())))
                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(match &d.pr_url {
                            Some(u) => format!("Delivered: pull request {}", u.rsplit('/').next().map(|n| format!("#{n}")).unwrap_or_default()),
                            None => format!("Delivered on {}", d.branch),
                        }));
                    if let Some(r) = &f.verify {
                        for (k, cr) in r.criteria.iter().enumerate() {
                            card = card.child(
                                div()
                                    .id(("delivery-criterion", k))
                                    .flex()
                                    .gap(px(8.))
                                    .text_size(px(12.))
                                    .text_color(fg2)
                                    .aria_label(format!("{} {}: {}", cr.key, cr.criterion, if cr.passed { "passed" } else { "failed" }))
                                    .child(glyph(if cr.passed { "check" } else { "x" }, 13., if cr.passed { c(pal.ok) } else { c(pal.warn) }))
                                    .child(format!("{} {}", cr.key, cr.criterion))
                                    .child(div().font_family(MONO).text_size(px(11.)).text_color(fg3).child(cr.command.clone()))
                                    .test_support(),
                            );
                        }
                    }
                    let short = &d.commit[..d.commit.len().min(7)];
                    let mut tail = format!("Verified on {short} · no test weakened");
                    for l in &d.limitations {
                        tail.push_str(" · ");
                        tail.push_str(l);
                    }
                    card = card.child(div().text_size(px(11.5)).text_color(fg3).child(tail));
                    if let Some(u) = d.pr_url.clone() {
                        card = card.child(div().flex().gap(px(6.)).child(button("open-pr", "Open pull request", true).on_click(cx.listener(move |_, _, _, cx| cx.open_url(&u))).test_support()));
                    }
                    col = col.child(card.test_support());
                }
                if let Some(ask) = &f.ask {
                    let mut card = div()
                        .id("familiar-ask")
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .p(px(10.))
                        .rounded(px(radius::CONTROL))
                        .border_1()
                        .border_color(ca(pal.warn, 0.7))
                        .bg(ca(pal.warn, 0.08))
                        .aria_label(ask.clone())
                        .child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Needs you"))
                        .child(div().text_size(px(12.5)).text_color(fg2).child(ask.clone()));
                    let mut btns = div().flex().gap(px(6.));
                    if let (Some(yes), true) = (f.yes_reply.clone(), live) {
                        btns = btns.child(button("fam-yes", "Yes", true).on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(l) = this.open_live() {
                                l.h.send(yes.clone());
                            }
                            cx.notify();
                        })).test_support());
                    }
                    if let (Some(id), true) = (f.approval, live) {
                        btns = btns.child(button("fam-approve", "Approve", true).on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(l) = this.open_live() {
                                l.h.approve(id);
                            }
                            cx.notify();
                        })).test_support());
                    }
                    card = card.child(btns);
                    col = col.child(card.test_support());
                }
                let n = f.crew.len();
                (f.state, format!("coordinating {n} subagent{}", if n == 1 { "" } else { "s" }), col)
            }
            Some(s) => {
                let mut col = div().flex().flex_col().gap(px(12.)).child(boxed("sub-ticket").aria_label(s.ticket.clone()).child(label(format!("Ticket from the familiar · {}", s.name))).child(div().text_size(px(13.)).text_color(fg).child(s.ticket.clone())).test_support());
                for (k, text) in s.log.iter().enumerate() {
                    col = col.child(event(k, EventKind::Agent, text.clone()));
                }
                (s.state, "reports to the familiar".to_string(), col)
            }
        };
        col = col.w_full().max_w(px(560.));
        if let Some(e) = &self.fam_error {
            col = col.child(div().id("fam-error").text_size(px(12.)).text_color(c(pal.bad)).aria_label(e.clone()).child(e.clone()).test_support());
        }

        // The foot (the board's `.foot`): the state, what this agent does in the run, the spend;
        // the five stages on the right, the current one in the accent.
        let foot = div()
            .id("familiar-foot")
            .h(px(30.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(12.))
            .border_t_1()
            .border_color(c(pal.line))
            .text_size(px(11.))
            .text_color(fg3)
            .child(dot("familiar-state", state, pal).test_support())
            .child(state_label(state))
            .child(div().child(format!("· {note} · ${:.2}", f.cost_usd)))
            .when_live(self.fam_fixture, |d| d.child(preview_tag("familiar-preview", pal).test_support()))
            .child(div().ml_auto().child(stage_line(f.stage, pal).test_support()));
        let composer = match (&self.fam_composer, sub.is_none() && live) {
            (Some((st, _)), true) => div()
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
                .aria_label("Message the familiar")
                .child(gpui_kit::component::input::Textarea::new(st).appearance(false))
                .test_support(),
            _ => {
                let hint = if sub.is_some() { "Messages go to the familiar: open its tab to reply" } else { "Made-up familiar: nothing to message" };
                div().id("familiar-composer").w_full().max_w(px(560.)).flex_none().p(px(10.)).rounded(px(10.)).border_1().border_color(c(pal.line)).bg(c(pal.inset)).text_size(px(12.5)).text_color(fg3).aria_label(hint).child(hint).test_support()
            }
        };
        let question = sub.is_none().then(|| self.fam_question()).flatten();
        let stream = div()
            .id("familiar-stream")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(radius::PANEL))
            .rounded_tl(px(corner))
            .bg(fam_layers(self.painting, pal).0)
            .border_1()
            .border_color(c(pal.line))
            .child(div().id("familiar-scroll").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().items_center().px(px(16.)).pt(px(18.)).pb(px(16.)).opacity(fade).child(col))
            .child(div().flex().flex_col().items_center().gap(px(8.)).px(px(16.)).pb(px(12.)).when_live(question.is_some(), |d| d.child(self.decision_card(question.as_ref().expect("checked"), pal, cx))).child(composer))
            .child(foot.test_support())
            .test_support();

        // Highlights: what each gate said, what changed, what it spent.
        let kv = |k: &'static str, v: String| div().flex().justify_between().gap(px(8.)).text_size(px(12.)).text_color(fg2).child(k).child(div().text_color(fg).font_features(tnum()).child(v));
        let note = |t: String| div().text_size(px(11.5)).line_height(px(16.)).text_color(fg3).child(t);
        let mut hl = div().id("familiar-highlights").w(px(HL_W)).flex_none().h_full().flex().flex_col().gap(px(10.)).px(px(8.)).py(px(10.)).aria_label(if self.fam_hl == super::decide::OPTION_TAB { "Option" } else { HL_TABS[self.fam_hl.min(2)] });
        let past = |p: Phase| f.stage > p;
        hl = match self.fam_hl {
            super::decide::OPTION_TAB if question.is_some() => hl.child(self.option_pane(question.as_ref().expect("checked"), pal, cx)),
            0 | super::decide::OPTION_TAB => hl
                .child(kv("Plan", if past(Phase::Plan) { "approved".into() } else { "in progress".into() }))
                .child(kv("Execute", format!("{} of {} tickets", f.tickets_done, f.tickets)))
                .child(div().h(px(5.)).rounded(px(3.)).bg(c(pal.inset)).child(div().h_full().rounded(px(3.)).bg(c(pal.accent)).w(relative(if f.tickets == 0 { 0.0 } else { f.tickets_done as f32 / f.tickets as f32 }))))
                .child(kv(
                    "Verify",
                    match &f.verify {
                        Some(r) if r.passed => format!("{} of {}", r.criteria.len(), r.criteria.len()),
                        Some(r) => format!("{} issues", r.issues.len() + r.weakened.len()),
                        None => "not yet".into(),
                    },
                ))
                .child(kv("Ship", f.delivery.as_ref().map_or("not yet".into(), |d| d.pr_url.as_ref().and_then(|u| u.rsplit('/').next()).map_or(d.branch.clone(), |n| format!("PR #{n}")))))
                .child(kv("Learn", if f.stage == Phase::Done { "filed".into() } else { "after Ship".into() }))
                .child(div().mt(px(6.)).child(note("Merging stays yours: Ship stops at a branch or a pull request.".into()))),
            1 => match &f.verify {
                Some(r) => hl.child(kv("Run", format!("{}..{}", &r.base[..r.base.len().min(7)], &r.head[..r.head.len().min(7)]))).children(r.weakened.iter().map(|w| note(w.clone()))).child(note(".backspace/VERIFY.md has the full report.".into())),
                None => hl.child(note("The run's changes show here once Verify has looked at them.".into())),
            },
            _ => hl.child(kv("Total", format!("${:.2}", f.cost_usd))).children(f.crew.iter().map(|c| note(format!("{} · {}", c.name, state_label(c.state))))),
        };

        div().id("familiar").size_full().flex().gap(px(BODY_PAD)).px(px(BODY_PAD)).pb(px(BODY_PAD)).bg(fam_layers(self.painting, pal).1).child(stream).child(hl.test_support()).test_support().into_any_element()
    }
}

/// How long a toast stays up.
const TOAST: std::time::Duration = std::time::Duration::from_secs(6);

impl Cauldron {
    /// A note the platform just filed: newest first in the inbox, and a toast if it alerts. The
    /// toast goes after [`TOAST`] unless a newer one replaced it.
    pub(crate) fn take_note(&mut self, n: Note, cx: &mut Context<Self>) {
        self.inbox.retain(|x| x.id != n.id);
        self.inbox.insert(0, n.clone());
        if n.alert {
            let id = n.id;
            self.inbox_toast = Some((n, std::time::Instant::now()));
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                cx.background_executor().timer(TOAST).await;
                let _ = this.update(cx, |c, cx| {
                    if c.inbox_toast.as_ref().is_some_and(|(t, _)| t.id == id) {
                        c.inbox_toast = None;
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    /// The inbox's notes from the data folder, off the UI thread.
    pub(crate) fn load_inbox(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut notes = cx.background_executor().spawn(async { Inbox::open(&Prefs::data_dir()).list() }).await;
            notes.reverse();
            let _ = this.update(cx, |c, cx| {
                // Screenshots only: replay the newest note as if it just came in, so it toasts.
                #[cfg(debug_assertions)]
                let replay = std::env::var_os("CAULDRON_FIXTURE_TOAST").and_then(|_| notes.first().cloned());
                c.inbox = notes;
                #[cfg(debug_assertions)]
                if let Some(n) = replay {
                    // Late enough that a slow software-rendered start has drawn its first frames.
                    cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                        cx.background_executor().timer(std::time::Duration::from_secs(20)).await;
                        let _ = this.update(cx, |c, cx| c.take_note(n, cx));
                    })
                    .detach();
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens what note `i` is about (its familiar, when it is one) and marks it read.
    fn open_note(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(n) = self.inbox.get_mut(i) else { return };
        n.read = true;
        let (id, source) = (n.id, n.event.source.clone());
        self.inbox_open = false;
        if self.inbox_toast.as_ref().is_some_and(|(t, _)| t.id == id) {
            self.inbox_toast = None;
        }
        if let Some(k) = self.fam_views.iter().position(|f| f.id == source) {
            self.open_familiar(k);
        }
        if !self.fam_fixture {
            cx.background_executor().spawn(async move { Inbox::open(&Prefs::data_dir()).mark_read(id) }).detach();
        }
        cx.notify();
    }

    fn read_all(&mut self, cx: &mut Context<Self>) {
        for n in &mut self.inbox {
            n.read = true;
        }
        self.inbox_toast = None;
        if !self.fam_fixture {
            cx.background_executor().spawn(async move { Inbox::open(&Prefs::data_dir()).mark_all_read() }).detach();
        }
        cx.notify();
    }

    fn note_dot(kind: Kind, pal: &'static Palette) -> Div {
        let d = div().size(px(7.)).flex_none().rounded_full().mt(px(5.));
        match kind {
            Kind::NeedsYou => d.bg(c(pal.warn)),
            Kind::Delivered => d.bg(c(pal.accent)),
            Kind::Failed => d.bg(c(pal.bad)),
            Kind::Info => d.border_1().border_color(c(pal.fg3)),
        }
    }

    /// The bell's label: "Inbox", or with the unread count.
    pub(crate) fn inbox_label(&self) -> String {
        match self.inbox.iter().filter(|n| !n.read).count() {
            0 => "Inbox".into(),
            n => format!("Inbox, {n} unread"),
        }
    }

    /// The popover under the bell (Inbox board, direction A): newest first, unread tinted.
    pub(crate) fn inbox_popover(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mut list = div().id("inbox-list").flex().flex_col().gap(px(2.)).max_h(px(360.)).overflow_y_scroll();
        if self.inbox.is_empty() {
            list = list.child(div().id("inbox-empty").px(px(8.)).py(px(10.)).text_size(px(12.)).text_color(fg3).aria_label("Nothing yet").child("Nothing yet. Familiars post here when they need you, deliver or stop.").test_support());
        }
        for (i, n) in self.inbox.iter().enumerate().take(50) {
            let label = if n.read { n.event.title.clone() } else { format!("Unread: {}", n.event.title) };
            list = list.child(
                div()
                    .id(("inbox-note", i))
                    .flex()
                    .gap(px(8.))
                    .px(px(8.))
                    .py(px(7.))
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .when(!n.read, |d| d.bg(c(pal.hover)))
                    .hover(|d| d.bg(ca(pal.fg, 0.07)))
                    .aria_label(label)
                    .child(Self::note_dot(n.event.kind, pal))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().text_size(px(12.5)).text_color(fg).when(!n.read, |d| d.font_weight(FontWeight::MEDIUM)).child(n.event.title.clone()))
                            .when(!n.event.body.is_empty(), |d| d.child(div().text_size(px(11.5)).line_height(px(15.)).text_color(fg2).line_clamp(2).child(n.event.body.clone()))),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.open_note(i, cx)))
                    .test_support(),
            );
        }
        div()
            .id("inbox")
            .w(px(300.))
            .p(px(6.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .rounded(px(radius::PANEL))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .shadow(vec![drop(tokens::shadow::MENU)])
            .occlude()
            .aria_label("Inbox")
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.inbox_open = false;
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .px(px(8.))
                    .pt(px(4.))
                    .pb(px(6.))
                    .text_size(px(11.5))
                    .child(div().text_color(fg).font_weight(FontWeight::SEMIBOLD).child("Inbox"))
                    .child(
                        div()
                            .id("inbox-read-all")
                            .text_color(fg2)
                            .cursor_pointer()
                            .hover(|d| d.text_color(fg))
                            .aria_label("Mark all read")
                            .child("Mark all read")
                            .on_click(cx.listener(|this, _, _, cx| this.read_all(cx)))
                            .test_support(),
                    ),
            )
            .child(list)
    }

    /// The newest alerting note, for six seconds, at the window's bottom right. Clicking it opens
    /// the note; it never takes focus.
    pub(crate) fn inbox_toast(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let (n, _) = self.inbox_toast.as_ref()?;
        let i = self.inbox.iter().position(|x| x.id == n.id)?;
        Some(
            div()
                .id("inbox-toast")
                .w(px(280.))
                .p(px(10.))
                .flex()
                .gap(px(8.))
                .rounded(px(radius::PANEL))
                .bg(c(pal.raised))
                .border_1()
                .border_color(c(pal.line))
                .shadow(vec![drop(tokens::shadow::MENU)])
                .cursor_pointer()
                .occlude()
                .aria_label(n.event.title.clone())
                .child(Self::note_dot(n.event.kind, pal))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().text_size(px(12.5)).font_weight(FontWeight::MEDIUM).text_color(c(pal.fg)).child(n.event.title.clone()))
                        .when(!n.event.body.is_empty(), |d| d.child(div().text_size(px(11.5)).text_color(c(pal.fg2)).truncate().child(n.event.body.clone()))),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.open_note(i, cx))),
        )
    }
}
