//! Familiars (Copilots board round 2, Charles 16 Nov): agents you hand a goal to. A familiar
//! plans, opens subagents and runs the stages itself, stopping only for you. Opening one swaps
//! the stream card's body for the old shell's content area: a centred stream card with the
//! Highlights panel beside it, both inside the one rounded card. Round 3 (T1): the tabs are folder
//! tabs on the stream card's top edge, from its left edge to its right, with no title row: the
//! familiar first (accent tint, wheel, never collapses), then one slim tab per subagent. The open
//! tab joins the card (its fill, rounded top, concave shoulders) and slides between tabs.
//! Highlights keeps its own tabs on the same line.
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
        self.fam_to = None;
        self.fam_since = None;
        self.fam_hl = 0;
    }

    /// The familiar's folder tabs over its stream card and Highlights' tabs over its column, both
    /// on the line above the card body. `main` is the card, `rail_here` whether the rail is in it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn familiar_strips(&mut self, main: Rect, rail_here: bool, pal: &'static Palette, now: std::time::Instant, reduce: bool, window: &mut Window, cx: &mut Context<Self>) -> FamStrips {
        let f = &EXAMPLES[self.familiar.unwrap_or(0)];
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
        let fill = c(pal.panel);
        let frame = c(pal.card);
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
        if l_sh > 0.5 {
            strip = strip.child(div().absolute().left(px(jx + 1.0 - l_sh)).top(px(STRIP_H + 1.0 - l_sh)).size(px(l_sh)).bg(fill).child(div().size_full().bg(frame).border_r_1().border_b_1().border_color(line).rounded_br(px(l_sh))));
        }
        if r_sh > 0.5 {
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
            .aria_label(f.name)
            .child(glyph("ship-wheel", 13., accent))
            .child("Familiar")
            .child(dot(("tab-state", 0usize), f.state, pal).test_support())
            .on_click(cx.listener(|this, _, window, cx| {
                this.active = 0;
                this.focus_tabs(window, cx);
                cx.notify();
            }));
        strip = strip.child(slot(0.0).child(self.tipped(("tab", 0usize), fam_tab, f.name, Side::Below, pal, cx).test_support()));
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
    /// card (the familiar's own, or the open subagent's) right under its folder tabs, with no title
    /// row, and the Highlights panel beside it. `corner` is the card's top-left radius (square under
    /// the first tab) and `fade` the new tab's content fading in.
    pub(crate) fn familiar_body(&self, pal: &'static Palette, (corner, fade): (f32, f32)) -> AnyElement {
        let Some(f) = self.familiar.and_then(|i| EXAMPLES.get(i)) else { return div().into_any_element() };
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let sub = (self.active > 0).then(|| self.agents.get(self.active - 1)).flatten().and_then(|a| self.familiar_sub(&a.name));

        let event = |k: usize, icon: &'static str, text: &'static str| {
            let col = if icon == "circle-alert" { c(pal.warn) } else { fg3 };
            div().id(("familiar-event", k)).flex().items_center().gap(px(8.)).text_size(px(12.)).text_color(fg2).aria_label(text).child(glyph(icon, 13., col)).child(text).test_support()
        };
        let label = |t: String| div().text_size(px(10.5)).text_color(fg3).child(t);
        let boxed = |id: &'static str| div().id(id).flex().flex_col().gap(px(4.)).p(px(10.)).rounded(px(radius::CONTROL)).bg(c(pal.inset)).border_1().border_color(c(pal.line));

        let (state, note, mut col) = match sub {
            None => {
                let mut col = div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(boxed("familiar-goal").aria_label(f.goal).child(label(format!("Goal · {}", f.name))).child(div().text_size(px(13.)).text_color(fg).child(f.goal)).test_support())
                    .child(div().id("familiar-plan").flex().flex_col().gap(px(2.)).aria_label(f.plan).child(label("Familiar · plan".into())).child(div().text_size(px(13.)).text_color(fg).child(f.plan)).test_support());
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
                (f.state, format!("coordinating {n} subagents"), col)
            }
            Some(s) => {
                let mut col = div().flex().flex_col().gap(px(12.)).child(boxed("sub-ticket").aria_label(s.ticket).child(label(format!("Ticket from the familiar · {}", s.name))).child(div().text_size(px(13.)).text_color(fg).child(s.ticket)).test_support());
                for (k, text) in s.log.iter().enumerate() {
                    col = col.child(event(k, "bot", text));
                }
                (s.state, "reports to the familiar".to_string(), col)
            }
        };
        col = col.w_full().max_w(px(560.));

        // The foot (the board's `.foot`): the state, what this agent does in the run, the Preview
        // tag; the stages on the right, Execute current.
        let mut stages = div().flex().gap(px(12.));
        for name in ["Plan", "Execute", "Ship", "Commit"] {
            stages = stages.child(div().text_color(if name == "Execute" { c(pal.accent) } else { fg3 }).child(name));
        }
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
            .child(state.label())
            .child(div().child(format!("· {note}")))
            .child(preview_tag("familiar-preview", pal).test_support())
            .child(div().ml_auto().child(stages));
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
            .rounded_tl(px(corner))
            .bg(c(pal.panel))
            .border_1()
            .border_color(c(pal.line))
            .child(div().id("familiar-scroll").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().items_center().px(px(16.)).pt(px(18.)).pb(px(16.)).opacity(fade).child(col))
            .child(div().flex().justify_center().px(px(16.)).pb(px(12.)).child(composer))
            .child(foot.test_support())
            .test_support();

        // Highlights (the old shell's right panel): what this run is waiting on and what it spent.
        let done = f.crew.iter().filter(|s| s.state == State::Done && s.name != "Rampart").count();
        let tickets = f.crew.iter().filter(|s| s.name != "Rampart").count();
        let kv = |k: &'static str, v: String| div().flex().justify_between().gap(px(8.)).text_size(px(12.)).text_color(fg2).child(k).child(div().text_color(fg).font_features(tnum()).child(v));
        let note = |t: &'static str| div().text_size(px(11.5)).line_height(px(16.)).text_color(fg3).child(t);
        let mut hl = div().id("familiar-highlights").w(px(HL_W)).flex_none().h_full().flex().flex_col().gap(px(10.)).px(px(8.)).py(px(10.)).aria_label(HL_TABS[self.fam_hl.min(2)]);
        hl = match self.fam_hl {
            0 => hl
                .child(kv("Plan", "approved".into()))
                .child(kv("Execute", format!("{done} of {tickets} tickets")))
                .child(div().h(px(5.)).rounded(px(3.)).bg(c(pal.inset)).child(div().h_full().rounded(px(3.)).bg(c(pal.accent)).w(relative(if tickets == 0 { 0.0 } else { done as f32 / tickets as f32 }))))
                .child(kv("Ship", "waits for you".into()))
                .child(div().mt(px(6.)).child(note("A preview: nothing here runs yet. Familiars need stages that advance on their own and a sandbox per subagent (M6)."))),
            1 => hl.child(note("Each subagent's changes show here once it has a worktree of its own (M6).")),
            _ => hl.child(note("Spend and time per subagent show here once core runs familiars.")),
        };

        div().id("familiar").size_full().flex().gap(px(BODY_PAD)).px(px(BODY_PAD)).pb(px(BODY_PAD)).bg(c(pal.card)).child(stream).child(hl.test_support()).test_support().into_any_element()
    }
}
