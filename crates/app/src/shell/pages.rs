//! The pages the apps row opens in the card (Charles, 21 Nov 16:52Z, "same as the destinations
//! board"): Folders (glass folders), Agents (roster, crew, context), Library (proposals, graph),
//! and the Usage page's activity tracker. What core has is real (folders from the streams,
//! familiars, hours from the CLIs' logs); what it has not (the Librarian, agent context, routines)
//! is drawn from made-up data and marked Mockup, never shown as working.

use super::*;
use backspace_core::familiar::State as FamState;

/// Apps that open a page in the card (indexes into FUNCTIONS).
pub(crate) fn has_page(i: usize) -> bool {
    matches!(FUNCTIONS.get(i).map(|f| f.1), Some("Folders" | "Agents" | "Library"))
}

pub(crate) fn title(section: usize) -> &'static str {
    FUNCTIONS.get(section).map_or("", |f| f.1)
}

/// What the pages load and where their small controls stand.
#[derive(Default)]
pub(crate) struct State {
    /// Prompts plus tool calls per hour, the last 8 days, oldest first (core's `activity`).
    pub activity: Option<Vec<u32>>,
    pub loading: bool,
    pub lib_tab: usize,
    /// Proposal i: Some(true) accepted, Some(false) rejected.
    pub proposals: [Option<bool>; 3],
    pub note: usize,
    pub agent: usize,
    pub agent_tab: usize,
}

/// Seven rows (oldest day first, today last) of 24 local hours. `acts` ends at hour `end` (hours
/// since the epoch); `off` is the local offset from UTC in seconds.
pub(crate) fn grid(acts: &[u32], end: u64, off: i64) -> [[u32; 24]; 7] {
    let mut g = [[0u32; 24]; 7];
    let today = (end as i64 * 3600 + off).div_euclid(86_400);
    for (i, n) in acts.iter().enumerate() {
        let e = end as i64 - (acts.len() as i64 - 1 - i as i64);
        let secs = e * 3600 + off;
        let (day, hour) = (secs.div_euclid(86_400), (secs.rem_euclid(86_400) / 3600) as usize);
        let back = today - day;
        if (0..7).contains(&back) {
            g[6 - back as usize][hour] += n;
        }
    }
    g
}

/// 0 for nothing, then 1 to 4 by share of the busiest hour.
pub(crate) fn level(n: u32, max: u32) -> usize {
    if n == 0 || max == 0 {
        0
    } else {
        (1 + (n as usize * 4 - 1) / max as usize).min(4)
    }
}

const PROPOSALS: [(&str, &str, &str); 3] = [
    ("Folder", "File 3 streams under Cauldron / Onboarding", "They share a folder and a plan: Welcome, Machines and Import."),
    ("Note", "Keep \"T3 can't read the legacy export\" as a note", "It cost three tries and a handoff; the next import will want it."),
    ("Link", "Connect Machines picker to Tight tiles", "Both cite the same six-step layout."),
];

const NOTES: [(&str, &str, f32, f32); 6] = [
    ("Tight tiles", "The approved layout: 6px radii, one frame, support card on the right.", 0.50, 0.46),
    ("Onboarding v2", "Six steps behind a setting; T1 and T2 passed.", 0.24, 0.24),
    ("Legacy import", "The chat export's format is unknown; ask for a sample.", 0.20, 0.72),
    ("Machines picker", "Shows LAN and connected machines.", 0.76, 0.28),
    ("Usage", "No limit percentages in local logs; counts and tokens only.", 0.78, 0.70),
    ("Librarian", "Files streams, proposes, never moves without a yes.", 0.50, 0.86),
];
const EDGES: [(usize, usize); 7] = [(0, 1), (0, 3), (0, 4), (0, 5), (1, 2), (1, 3), (4, 5)];

impl Cauldron {
    /// Reads the activity hours off the UI thread (once; the Usage page refreshes on open).
    pub(super) fn load_pages(&mut self, cx: &mut Context<Self>) {
        if self.pages.loading {
            return;
        }
        #[cfg(debug_assertions)]
        if std::env::var_os("CAULDRON_FIXTURE_ACTIVITY").is_some() {
            // A believable week: busy 9 to 18 on weekdays, light evenings, quiet nights.
            self.pages.activity = Some((0..192u32).map(|i| {
                let (h, d) = (i % 24, i / 24);
                let base = if (9..18).contains(&h) { 6 + (i * 7 % 11) } else if (18..23).contains(&h) { (i * 5) % 4 } else { 0 };
                if d % 7 == 5 || d % 7 == 6 { base / 3 } else { base }
            }).collect());
            return;
        }
        self.pages.loading = true;
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let a = cx
                .background_executor()
                .spawn(async move {
                    let home = std::path::PathBuf::from(std::env::var("USERPROFILE").or(std::env::var("HOME")).unwrap_or_default());
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
                    backspace_core::usage::activity(&home, now, 8)
                })
                .await;
            let _ = this.update(cx, |c, cx| {
                c.pages.activity = Some(a);
                c.pages.loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn page_shell(id: &'static str, title: &'static str, sub: &str, pal: &'static Palette, kids: Vec<AnyElement>) -> AnyElement {
        div().id(id).size_full().overflow_y_scroll().flex().justify_center().child(
            div()
                .w_full()
                .max_w(px(980.))
                .flex()
                .flex_col()
                .px(px(24.))
                .py(px(8.))
                .child(div().text_size(px(20.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).text_color(c(pal.fg)).mt(px(12.)).child(title))
                .child(div().text_size(px(13.)).text_color(c(pal.fg3)).mb(px(14.)).child(sub.to_string()))
                .children(kids),
        )
        .test_support()
        .into_any_element()
    }

    /// A small "Mockup" tag for parts drawn from made-up data.
    fn mock_tag(pal: &'static Palette) -> Div {
        div().flex_none().px(px(6.)).h(px(16.)).flex().items_center().rounded(px(4.)).bg(c(pal.inset)).text_size(px(10.)).font_weight(FontWeight::MEDIUM).text_color(c(pal.fg3)).child("Mockup")
    }

    /// The page for the selected app.
    pub(super) fn section_page_body(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        match FUNCTIONS[self.section].1 {
            "Folders" => self.folders_page(pal, cx),
            "Agents" => self.agents_page(pal, cx),
            _ => self.library_page(pal, cx),
        }
    }

    // ---- Usage: the activity tracker -------------------------------------------------------

    /// "The week, hour by hour": real prompts and tool calls from the CLIs' logs.
    pub(super) fn activity_card(&self, pal: &'static Palette) -> impl IntoElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let end = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 3600);
        let off = {
            use chrono::Offset;
            chrono::Local::now().offset().fix().local_minus_utc() as i64
        };
        let g = self.pages.activity.as_ref().map(|a| grid(a, end, off));
        let max = g.as_ref().map_or(0, |g| g.iter().flatten().copied().max().unwrap_or(0));
        let total: u32 = g.as_ref().map_or(0, |g| g.iter().flatten().sum());
        let today = chrono::Local::now().date_naive();
        let mut card = div()
            .id("activity")
            .mt(px(14.))
            .p(px(14.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .rounded(px(radius::PANEL))
            .bg(c(pal.panel))
            .border_1()
            .border_color(c(pal.line))
            .shadow(lift(Lift::Button, pal))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(fg).child("The week, hour by hour"))
                    .child(div().ml_auto().text_size(px(11.5)).font_features(tnum()).text_color(fg3).child(match (&g, total) {
                        (None, _) => "Reading your logs…".to_string(),
                        (_, 0) => "No activity found".to_string(),
                        (_, n) => format!("{n} prompts and tool calls"),
                    })),
            );
        let Some(g) = g else { return card.test_support() };
        let mut rows = div().flex().flex_col().gap(px(3.));
        let mut hours = div().flex().gap(px(3.)).pl(px(34.));
        for h in 0..24 {
            hours = hours.child(div().flex_1().min_w_0().text_size(px(9.5)).text_color(fg3).font_features(tnum()).child(if h % 6 == 0 { format!("{h}") } else { String::new() }));
        }
        rows = rows.child(hours);
        for (r, row) in g.iter().enumerate() {
            let day = today - chrono::Duration::days(6 - r as i64);
            let mut line = div().flex().items_center().gap(px(3.)).child(div().w(px(31.)).flex_none().text_size(px(10.5)).text_color(fg3).child(day.format("%a").to_string()));
            for (h, n) in row.iter().enumerate() {
                let lv = level(*n, max);
                let fill = if lv == 0 { c(pal.inset) } else { ca(pal.accent, [0., 0.28, 0.5, 0.75, 1.0][lv]) };
                line = line.child(div().id(("cell", r * 24 + h)).flex_1().min_w_0().h(px(16.)).rounded(px(3.)).bg(fill).aria_label(format!("{} {h}:00, {n}", day.format("%a"))));
            }
            rows = rows.child(line);
        }
        card = card.child(rows).child(
            div().flex().items_center().gap(px(4.)).mt(px(4.)).text_size(px(10.5)).text_color(fg3).child("Less").children((0..5).map(|l| div().size(px(10.)).rounded(px(2.)).bg(if l == 0 { c(pal.inset) } else { ca(pal.accent, [0., 0.28, 0.5, 0.75, 1.0][l]) }))).child("More").child(div().ml_auto().child("Prompts and tool calls from Claude Code, Codex and OpenCode logs on this computer")),
        );
        card.test_support()
    }

    // ---- Folders ---------------------------------------------------------------------------

    fn folder_art(&self, i: usize, pal: &'static Palette, ghost: bool) -> Div {
        let dot = tokens::AGENT_DOTS[(i * 2 + 1) % tokens::AGENT_DOTS.len()];
        let (back, front) = if ghost { (ca(pal.fg4, 0.12), ca(pal.fg4, 0.08)) } else { (ca(dot, 0.40), ca(dot, 0.22)) };
        let sheet = |x: f32, y: f32| {
            div().absolute().left(px(x)).top(px(y)).w(px(58.)).h(px(66.)).rounded(px(5.)).bg(rgb(0xf4f4f2)).opacity(if ghost { 0.35 } else { 1. }).flex().flex_col().gap(px(5.)).p(px(8.)).children((0..3).map(|k| div().h(px(3.)).w(px(if k == 2 { 24. } else { 42. })).rounded_full().bg(rgb(0xd0d0cc))))
        };
        div()
            .relative()
            .w(px(148.))
            .h(px(108.))
            .flex_none()
            .child(div().absolute().left(px(0.)).top(px(12.)).w(px(148.)).h(px(90.)).rounded(px(10.)).bg(back))
            .child(div().absolute().left(px(10.)).top(px(2.)).w(px(52.)).h(px(18.)).rounded_t(px(6.)).bg(back))
            .child(sheet(20., 10.))
            .child(sheet(70., 4.))
            .child(div().absolute().left(px(0.)).top(px(38.)).w(px(148.)).h(px(70.)).rounded(px(13.)).bg(front).border_1().border_color(hsla(0., 0., 1., if ghost { 0.10 } else { 0.28 })).shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.25), offset: point(px(0.), px(6.)), blur_radius: px(14.), spread_radius: px(-6.), inset: false }]))
    }

    fn folders_page(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let list = match &self.threads {
            Threads::Ready(v) => v.clone(),
            _ => Vec::new(),
        };
        let folders = apps::folders(&list);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        let ago = |ms: u64| {
            let m = now.saturating_sub(ms) / 60_000;
            match m {
                0 => "Updated just now".to_string(),
                1..=59 => format!("Updated {m}m ago"),
                60..=1439 => format!("Updated {}h ago", m / 60),
                _ => format!("Updated {}d ago", m / 1440),
            }
        };
                let mut grid = div().flex().flex_wrap().gap(px(14.));
        for (i, (name, n)) in folders.iter().enumerate() {
            let last = list.iter().filter(|t| if name == apps::NO_FOLDER { t.project.is_none() } else { t.project.is_some() && threads::meta(t) == *name }).map(|t| t.updated).max().unwrap_or(0);
            let q = if name == apps::NO_FOLDER { String::new() } else { name.clone() };
            let live = list.iter().filter(|t| t.busy && (name == apps::NO_FOLDER && t.project.is_none() || t.project.is_some() && threads::meta(t) == *name)).count();
            grid = grid.child(
                div()
                    .id(("folder-tile", i))
                    .w(px(176.))
                    .p(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .rounded(px(radius::PANEL))
                    .bg(c(pal.panel))
                    .border_1()
                    .border_color(c(pal.line))
                    .shadow(lift(Lift::Button, pal))
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label(format!("{name}, {n} streams"))
                    .child(self.folder_art(i, pal, false).child(div().absolute().left(px(10.)).bottom(px(8.)).px(px(7.)).h(px(18.)).flex().items_center().rounded_full().bg(hsla(0., 0., 0., 0.35)).text_size(px(10.)).text_color(rgb(0xffffff)).child(ago(last))))
                    .child(div().truncate().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(name.clone()))
                    .child(div().text_size(px(12.)).text_color(fg3).font_features(tnum()).child(format!("{n} {}{}", if *n == 1 { "stream" } else { "streams" }, if live > 0 { format!(" · {live} running") } else { String::new() })))
                    .on_click(cx.listener(move |this, _, window, cx| this.filter_streams(q.clone(), window, cx)))
                    .test_support(),
            );
        }
        // The Librarian's proposed folder, ghosted; a mock until the Librarian exists.
        let k = folders.len();
        grid = grid.child(
            div()
                .id("folder-proposed")
                .w(px(176.))
                .p(px(14.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .rounded(px(radius::PANEL))
                .border_1()
                .border_color(c(pal.line_strong))
                .aria_label("Proposed by the Librarian, mockup")
                .child(self.folder_art(k, pal, true))
                .child(div().flex().items_center().gap(px(6.)).child(div().truncate().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg3).child("Onboarding")).child(Self::mock_tag(pal)))
                .child(div().text_size(px(12.)).text_color(fg3).child("Proposed by the Librarian"))
                .test_support(),
        );
        grid = grid.child(
            div()
                .id("folder-new")
                .w(px(176.))
                .min_h(px(190.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .rounded(px(radius::PANEL))
                .border_1()
                .border_dashed()
                .border_color(c(pal.line_strong))
                .text_size(px(12.5))
                .text_color(fg3)
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .aria_label("New folder")
                .child("+ New folder")
                .child(div().text_size(px(11.)).child("Start a stream in a folder"))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.select_section(0, cx);
                    this.new_thread(window, cx);
                }))
                .test_support(),
        );
        let stripe = div()
            .id("librarian-stripe")
            .mb(px(16.))
            .px(px(12.))
            .py(px(9.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(radius::PANEL))
            .bg(ca(pal.warn, 0.10))
            .border_1()
            .border_color(ca(pal.warn, 0.30))
            .text_size(px(12.5))
            .text_color(fg)
            .child("The Librarian proposes filing 3 streams under Onboarding.")
            .child(Self::mock_tag(pal))
            .child(
                div().id("stripe-review").ml_auto().px(px(10.)).h(px(24.)).flex().items_center().rounded(px(radius::CONTROL)).bg(c(pal.raised)).shadow(lift(Lift::Button, pal)).cursor_pointer().text_size(px(12.)).aria_label("Review").child("Review").on_click(cx.listener(|this, _, _, cx| {
                    this.pages.lib_tab = 0;
                    if let Some(i) = FUNCTIONS.iter().position(|f| f.1 == "Library") {
                        this.select_section(i, cx);
                    }
                })).test_support(),
            )
            .test_support();
        Self::page_shell("folders-page", "Folders", "Where your streams run. Open one to see just its streams.", pal, vec![stripe.into_any_element(), grid.into_any_element()])
    }

    // ---- Agents ----------------------------------------------------------------------------

    fn agents_page(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let real = !self.fam_views.is_empty();
        // Name, goal, state, crew (name, ticket, state).
        let roster: Vec<(String, String, FamState, Vec<(String, String, FamState)>)> = if real {
            self.fam_views.iter().map(|v| (v.name.clone(), v.goal.clone(), v.state, v.crew.iter().map(|s| (s.name.clone(), s.ticket.clone(), s.state)).collect())).collect()
        } else {
            vec![(
                "Ship onboarding v2".into(),
                "Build the 6-step onboarding behind a setting.".into(),
                FamState::Running,
                vec![("Rampart".into(), "Tests for T1-T4".into(), FamState::Done), ("T1 Builder".into(), "Welcome + Agents".into(), FamState::Done), ("T2 Builder".into(), "Machines picker".into(), FamState::Running)],
            )]
        };
        let sel = self.pages.agent.min(roster.len() - 1);
        let (name, goal, state, crew) = roster[sel].clone();
        let state_col = |s: FamState| match s {
            FamState::Running => c(pal.accent),
            FamState::NeedsYou => c(pal.warn),
            FamState::Done => c(pal.ok),
            FamState::Failed => c(pal.bad),
        };
        let state_txt = |s: FamState| match s {
            FamState::Running => "Running",
            FamState::NeedsYou => "Needs you",
            FamState::Done => "Done",
            FamState::Failed => "Failed",
        };
        let mut left = div().w(px(232.)).flex_none().h_full().flex().flex_col().gap(px(2.)).p(px(10.)).border_r_1().border_color(c(pal.line));
        left = left.child(div().px(px(6.)).pb(px(6.)).flex().items_center().gap(px(6.)).text_size(px(11.5)).text_color(fg3).child("Familiars").children((!real).then(|| Self::mock_tag(pal))));
        for (i, (n, _, s, _)) in roster.iter().enumerate() {
            let on = i == sel;
            left = left.child(
                div()
                    .id(("agent-row", i))
                    .h(px(46.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .when_live(on, |d| d.bg(c(pal.selected)))
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label(format!("{n}, {}", state_txt(*s)))
                    .child(img(Arc::new(Image::from_bytes(ImageFormat::Svg, avatar::svg(i, Mood::Idle, Look { px: 30., outline: false, ms: None, bust: true, ink: None }).into_bytes()))).size(px(30.)))
                    .child(div().flex_1().min_w_0().flex().flex_col().child(div().truncate().text_size(px(12.5)).text_color(fg).child(n.clone())).child(div().flex().items_center().gap(px(5.)).text_size(px(11.)).text_color(fg3).child(div().size(px(6.)).rounded_full().bg(state_col(*s))).child(state_txt(*s))))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.pages.agent = i;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        left = left.child(
            div().id("agent-new").mt(px(6.)).h(px(30.)).px(px(8.)).flex().items_center().rounded(px(radius::CONTROL)).cursor_pointer().text_size(px(12.5)).text_color(fg2).hover(|d| d.bg(c(pal.hover))).aria_label("New familiar").child("+ New familiar").on_click(cx.listener(|this, _, window, cx| this.new_familiar(window, cx))).test_support(),
        );
        // The middle: the goal and the crew's tickets, over the composer.
        let mut mid = div().flex_1().min_w_0().h_full().flex().flex_col().p(px(18.)).gap(px(10.));
        mid = mid.child(div().flex().items_center().gap(px(8.)).child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(name.clone())).child(div().size(px(7.)).rounded_full().bg(state_col(state))).child(div().text_size(px(12.)).text_color(fg3).child(state_txt(state))));
        mid = mid.child(div().text_size(px(13.)).line_height(px(19.)).text_color(fg2).child(goal));
        mid = mid.child(div().mt(px(8.)).text_size(px(11.5)).text_color(fg3).child("Crew"));
        for (i, (n, t, s)) in crew.iter().enumerate() {
            mid = mid.child(
                div().id(("crew", i)).px(px(10.)).py(px(7.)).flex().items_center().gap(px(10.)).rounded(px(radius::CONTROL)).bg(c(pal.panel)).border_1().border_color(c(pal.line)).aria_label(format!("{n}, {t}, {}", state_txt(*s)))
                    .child(div().size(px(7.)).rounded_full().bg(state_col(*s)))
                    .child(div().text_size(px(12.5)).font_weight(FontWeight::MEDIUM).text_color(fg).child(n.clone()))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(fg3).child(t.clone()))
                    .child(div().text_size(px(11.5)).text_color(fg3).child(state_txt(*s)))
                    .test_support(),
            );
        }
        mid = mid.child(div().flex_1()).child(
            div()
                .id("launch-box")
                .px(px(14.))
                .py(px(12.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(radius::SHELL))
                .bg(c(pal.raised))
                .shadow(lift(Lift::Box, pal))
                .text_size(px(13.))
                .text_color(fg3)
                .cursor_pointer()
                .aria_label("Launch a crew")
                .child("Launch crew: describe the goal…")
                .on_click(cx.listener(|this, _, window, cx| this.new_familiar(window, cx)))
                .test_support(),
        );
        // The right panel: context, traces and routines have no source in core yet.
        let tabs = ["Context", "Traces", "Routines"];
        let mut right = div().w(px(210.)).flex_none().h_full().flex().flex_col().gap(px(10.)).p(px(12.)).border_l_1().border_color(c(pal.line));
        right = right.child(div().flex().items_center().gap(px(6.)).child(div().text_size(px(11.5)).text_color(fg3).child("Agent panel")).child(Self::mock_tag(pal)));
        let mut tabrow = div().flex().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL)).bg(c(pal.inset));
        for (i, t) in tabs.iter().enumerate() {
            let on = i == self.pages.agent_tab;
            tabrow = tabrow.child(
                div().id(("agent-tab", i)).flex_1().h(px(22.)).flex().items_center().justify_center().rounded(px(4.)).text_size(px(11.5)).cursor_pointer().text_color(if on { fg } else { fg3 }).when_live(on, |d| d.bg(c(pal.raised))).aria_label(*t).child(*t).on_click(cx.listener(move |this, _, _, cx| {
                    this.pages.agent_tab = i;
                    cx.notify();
                })).test_support(),
            );
        }
        right = right.child(tabrow);
        match self.pages.agent_tab {
            0 => {
                right = right.child(div().flex().items_center().gap(px(10.)).child(usage::ring(Some(62), 44., c(pal.accent), c(pal.line_strong))).child(div().flex().flex_col().child(div().text_size(px(12.5)).text_color(fg).child("62% of context")).child(div().text_size(px(11.)).text_color(fg3).child("Sample numbers"))));
                // A treemap of what fills the context, as blocks sized by share.
                let blk = |id: usize, label: &'static str, w: f32, h: f32, a: f32| div().id(("ctx-block", id)).w(px(w)).h(px(h)).rounded(px(4.)).bg(ca(pal.accent, a)).p(px(5.)).text_size(px(10.)).text_color(fg).child(label);
                right = right.child(div().flex().flex_col().gap(px(3.)).child(div().flex().gap(px(3.)).child(blk(0, "Files", 88., 64., 0.45)).child(blk(1, "Chat", 62., 64., 0.28))).child(div().flex().gap(px(3.)).child(blk(2, "Tools", 54., 40., 0.20)).child(blk(3, "Rules", 38., 40., 0.12)).child(blk(4, "Notes", 58., 40., 0.30))));
            }
            1 => right = right.child(div().text_size(px(12.)).line_height(px(17.)).text_color(fg3).child("Traces of each tool call will list here. Not wired yet.")),
            _ => right = right.child(div().text_size(px(12.)).line_height(px(17.)).text_color(fg3).child("Routines the agent repeats will list here. Not wired yet.")),
        }
        div().id("agents-page").size_full().flex().child(left).child(mid).child(right).test_support().into_any_element()
    }

    // ---- Library ---------------------------------------------------------------------------

    fn library_page(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
                let tabs = ["Proposals", "Notes", "Graph", "Librarian"];
        let mut bar = div().flex().items_center().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL)).bg(c(pal.inset)).self_start();
        for (i, t) in tabs.iter().enumerate() {
            let on = i == self.pages.lib_tab;
            bar = bar.child(div().id(("lib-tab", i)).px(px(12.)).h(px(24.)).flex().items_center().rounded(px(4.)).text_size(px(12.5)).cursor_pointer().text_color(if on { fg } else { fg3 }).when_live(on, |d| d.bg(c(pal.raised)).shadow(lift(Lift::Button, pal))).aria_label(*t).child(*t).on_click(cx.listener(move |this, _, _, cx| {
                this.pages.lib_tab = i;
                cx.notify();
            })).test_support());
        }
        let bar = div().flex().items_center().gap(px(10.)).child(bar).child(Self::mock_tag(pal)).child(div().text_size(px(11.5)).text_color(fg3).child("Made-up proposals: the Librarian is not built yet"));
        let body = match self.pages.lib_tab {
            0 => {
                let mut col = div().mt(px(14.)).flex().flex_col().gap(px(10.));
                let done = self.pages.proposals.iter().filter(|p| **p == Some(true)).count();
                if done > 0 {
                    col = col.child(
                        div().id("receipt").px(px(12.)).py(px(9.)).flex().items_center().gap(px(8.)).rounded(px(radius::PANEL)).bg(ca(pal.ok, 0.10)).border_1().border_color(ca(pal.ok, 0.30)).text_size(px(12.5)).text_color(fg).aria_label(format!("Filed {done}"))
                            .child(format!("Filed {done} {}.", if done == 1 { "proposal" } else { "proposals" }))
                            .child(div().id("receipt-undo").ml_auto().px(px(10.)).h(px(24.)).flex().items_center().rounded(px(radius::CONTROL)).bg(c(pal.raised)).shadow(lift(Lift::Button, pal)).cursor_pointer().text_size(px(12.)).aria_label("Undo").child("Undo").on_click(cx.listener(|this, _, _, cx| {
                                this.pages.proposals = [None; 3];
                                cx.notify();
                            })).test_support())
                            .test_support(),
                    );
                }
                for (i, (kind, t, why)) in PROPOSALS.iter().enumerate() {
                    let st = self.pages.proposals[i];
                    let btn = |id: &'static str, label: &'static str, primary: bool| {
                        div().id((id, i)).px(px(12.)).h(px(26.)).flex().items_center().rounded(px(radius::CONTROL)).cursor_pointer().text_size(px(12.)).aria_label(label).child(label).when_live(primary, |d| d.bg(c(pal.accent)).text_color(c(pal.on_accent))).when_live(!primary, |d| d.bg(c(pal.raised)).text_color(fg2).shadow(lift(Lift::Button, pal)))
                    };
                    col = col.child(
                        div().id(("proposal", i)).p(px(14.)).flex().items_center().gap(px(12.)).rounded(px(radius::PANEL)).bg(c(pal.panel)).border_1().border_color(c(pal.line)).shadow(lift(Lift::Button, pal)).opacity(if st == Some(false) { 0.5 } else { 1. }).aria_label(*t)
                            .child(div().w(px(52.)).flex_none().text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child(*kind))
                            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(2.)).child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(fg).child(*t)).child(div().text_size(px(12.)).text_color(fg3).child(*why)))
                            .child(match st {
                                None => div().flex().gap(px(6.)).child(btn("reject", "Reject", false).on_click(cx.listener(move |this, _, _, cx| {
                                    this.pages.proposals[i] = Some(false);
                                    cx.notify();
                                })).test_support()).child(btn("accept", "Accept", true).on_click(cx.listener(move |this, _, _, cx| {
                                    this.pages.proposals[i] = Some(true);
                                    cx.notify();
                                })).test_support()),
                                Some(true) => div().text_size(px(12.)).text_color(c(pal.ok)).child("Accepted"),
                                Some(false) => div().text_size(px(12.)).text_color(fg3).child("Rejected"),
                            })
                            .test_support(),
                    );
                }
                col
            }
            1 => {
                let mut col = div().mt(px(14.)).flex().flex_col().gap(px(8.));
                for (i, (t, b, _, _)) in NOTES.iter().enumerate() {
                    col = col.child(div().id(("note", i)).p(px(12.)).rounded(px(radius::PANEL)).bg(c(pal.panel)).border_1().border_color(c(pal.line)).aria_label(*t).child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(fg).child(*t)).child(div().text_size(px(12.)).text_color(fg3).child(*b)).test_support());
                }
                col
            }
            2 => {
                let sel = self.pages.note.min(NOTES.len() - 1);
                let (gw, gh) = (560., 360.);
                let line = c(pal.line_strong);
                let accent = c(pal.accent);
                let nodes: Vec<(f32, f32)> = NOTES.iter().map(|n| (n.2 * gw, n.3 * gh)).collect();
                let nodes2 = nodes.clone();
                let mut canvas_box = div().id("graph").relative().w(px(gw)).h(px(gh)).flex_none().rounded(px(radius::PANEL)).bg(c(pal.panel)).border_1().border_color(c(pal.line)).aria_label("Graph of notes").child(
                    canvas(
                        |b, _, _| b,
                        move |b, _, window, _| {
                            for (a, z) in EDGES {
                                let mut pb = PathBuilder::stroke(px(1.2));
                                pb.move_to(point(b.origin.x + px(nodes2[a].0), b.origin.y + px(nodes2[a].1)));
                                pb.line_to(point(b.origin.x + px(nodes2[z].0), b.origin.y + px(nodes2[z].1)));
                                if let Ok(p) = pb.build() {
                                    window.paint_path(p, line);
                                }
                            }
                        },
                    )
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0(),
                );
                for (i, (t, _, _, _)) in NOTES.iter().enumerate() {
                    let on = i == sel;
                    let r = if on { 9. } else { 6. };
                    canvas_box = canvas_box.child(
                        div().id(("node", i)).absolute().left(px(nodes[i].0 - 40.)).top(px(nodes[i].1 - r)).w(px(80.)).flex().flex_col().items_center().gap(px(3.)).cursor_pointer().aria_label(*t)
                            .child(div().size(px(r * 2.)).rounded_full().bg(if on { accent } else { c(pal.fg3) }))
                            .child(div().text_size(px(10.5)).text_color(if on { fg } else { fg3 }).child(*t))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.pages.note = i;
                                cx.notify();
                            }))
                            .test_support(),
                    );
                }
                div().mt(px(14.)).flex().gap(px(14.)).child(canvas_box.test_support()).child(
                    div().id("note-panel").w(px(220.)).flex_none().p(px(14.)).flex().flex_col().gap(px(6.)).rounded(px(radius::PANEL)).bg(c(pal.panel)).border_1().border_color(c(pal.line)).shadow(lift(Lift::Button, pal)).aria_label(NOTES[sel].0)
                        .child(div().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(NOTES[sel].0))
                        .child(div().text_size(px(12.)).line_height(px(17.)).text_color(fg2).child(NOTES[sel].1))
                        .child(div().mt(px(6.)).text_size(px(11.)).text_color(fg3).child(format!("{} links", EDGES.iter().filter(|e| e.0 == sel || e.1 == sel).count())))
                        .test_support(),
                )
            }
            _ => div().mt(px(14.)).p(px(14.)).rounded(px(radius::PANEL)).bg(c(pal.panel)).border_1().border_color(c(pal.line)).text_size(px(12.5)).line_height(px(18.)).text_color(fg3).child("The Librarian reads your streams, files them into folders and links related notes. It proposes; you accept. Not built yet, so the other tabs show sample data."),
        };
        Self::page_shell("library-page", "Library", "Notes and links the Librarian keeps for you. Nothing moves without a yes.", pal, vec![bar.into_any_element(), body.into_any_element()])
    }
}

#[cfg(test)]
mod tests {
    use super::{grid, level};

    #[test]
    fn grid_puts_hours_in_local_days() {
        // 2 days of hours ending at epoch hour 49 (day 2, 01:00 UTC); +1h offset makes it 02:00.
        let acts = vec![1u32; 50];
        let g = grid(&acts, 49, 3600);
        assert_eq!(g[6][2], 1, "today's 02:00");
        assert_eq!(g[6][3], 0, "nothing after now");
        assert_eq!(g[5][23], 1, "yesterday late");
    }

    #[test]
    fn levels_scale_to_the_busiest_hour() {
        assert_eq!(level(0, 10), 0);
        assert_eq!(level(1, 10), 1);
        assert_eq!(level(10, 10), 4);
        assert_eq!(level(5, 0), 0);
    }
}
