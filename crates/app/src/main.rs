//! Backspace desktop workbench.
//!
//! A project sidebar on the left (agent tree or ticket board, run status at
//! the bottom), horizontal view tabs in the title bar, and one to four split
//! panes (a row of up to three, or a 2x2 grid), all resizable by dragging the
//! gaps between them. Any view opens in the focused pane: an agent's session,
//! the review queue, the ticket board, or an agent's worktree. Every review
//! card leads with a diagram the harness draws from its own data. Light, dark
//! or follow-the-system theme; frosted window background on macOS.
//!
//!   backspace [workspace]     (defaults to the current directory)

mod diagram_view;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use backspace_core::diagram;
use backspace_core::{
    AgentKind, AgentRecord, AgentStatus, Approval, ApprovalKind, Harness, LogKind, ProjectState,
    TicketState, MAIN,
};
use gpui_kit::base::{h_resizable, resizable_panel, v_resizable, ResizeHandleRenderer};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Root, Theme, ThemeMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

/// gpui takes one family name, not a CSS stack, so pick a face each OS ships.
const MONO: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else if cfg!(target_os = "windows") {
    "Consolas"
} else {
    "DejaVu Sans Mono"
};

/// macOS blurs what is behind a translucent window; elsewhere a translucent
/// background would just show the desktop unblurred, so stay opaque.
const GLASS: bool = cfg!(target_os = "macos");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ThemePref {
    System,
    Light,
    Dark,
}

impl ThemePref {
    fn path() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/backspace/ui.toml"))
    }

    fn load() -> Self {
        let value = Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| {
                s.lines().find_map(|l| {
                    let (k, v) = l.split_once('=')?;
                    (k.trim() == "theme").then(|| v.trim().trim_matches('"').to_string())
                })
            });
        match value.as_deref() {
            Some("light") => Self::Light,
            Some("dark") => Self::Dark,
            _ => Self::System,
        }
    }

    /// ui.toml only holds UI preferences, so rewriting it whole is fine.
    fn save(self) {
        if let Some(p) = Self::path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, format!("theme = \"{}\"\n", self.name()));
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::System => "Auto",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }

    fn apply(self, window: &mut Window, cx: &mut App) {
        match self {
            Self::System => Theme::sync_system_appearance(Some(window), cx),
            Self::Light => Theme::change(ThemeMode::Light, Some(window), cx),
            Self::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Agent(usize),
    Review,
    Tickets,
    Files(usize),
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Agents,
    Tickets,
}

struct Entry {
    depth: usize,
    name: String,
    path: PathBuf,
    dir: bool,
}

struct Workbench {
    harness: Rc<Harness>,
    snap: ProjectState,
    /// Views open as title-bar tabs.
    tabs: Vec<View>,
    /// What each split pane shows; its length is the layout (1–3 in a row,
    /// 4 as a 2x2 grid).
    panes: Vec<View>,
    focused: usize,
    side: Side,
    composer: Entity<InputState>,
    feedback: Entity<InputState>,
    new_ticket: Entity<InputState>,
    file_filter: Entity<InputState>,
    trees: HashMap<usize, (Instant, Vec<Entry>)>,
    preview: Option<(PathBuf, String)>,
    theme_pref: ThemePref,
    _subs: Vec<Subscription>,
    _refresh: Task<()>,
}

fn on_enter(
    input: &Entity<InputState>,
    window: &mut Window,
    cx: &mut Context<Workbench>,
    f: impl Fn(&mut Workbench, String) -> bool + 'static,
) -> Subscription {
    cx.subscribe_in(
        input,
        window,
        move |this, input, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = input.read(cx).value().trim().to_string();
                if !text.is_empty() && f(this, text) {
                    input.update(cx, |s, cx| s.set_value("", window, cx));
                }
                cx.notify();
            }
        },
    )
}

impl Workbench {
    fn new(harness: Harness, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let harness = Rc::new(harness);
        let input = |p: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(p))
        };
        let composer = input("Describe the goal, or answer the main agent…", window, cx);
        let feedback = input("Feedback for the agent (needed to reject)", window, cx);
        let new_ticket = input("File a ticket — title: what is wrong or wanted", window, cx);
        let file_filter = input("Filter files", window, cx);

        let theme_pref = ThemePref::load();
        theme_pref.apply(window, cx);

        let subs = vec![
            on_enter(&composer, window, cx, |this, text| {
                this.harness.send(text);
                true
            }),
            on_enter(&new_ticket, window, cx, |this, text| {
                let (title, body) = text.split_once(':').unwrap_or((&text, &text));
                this.harness.file_ticket(title.trim(), body.trim()).is_ok()
            }),
            cx.observe(&file_filter, |_, _, cx| cx.notify()),
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.theme_pref == ThemePref::System {
                    Theme::sync_system_appearance(Some(window), cx);
                    cx.notify();
                }
            }),
        ];

        let changes = harness.changes();
        let refresh = cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while changes.recv().await.is_ok() {
                let alive = this.update(cx, |this, cx| {
                    this.snap = this.harness.snapshot();
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        });

        Self {
            snap: harness.snapshot(),
            harness,
            tabs: vec![View::Agent(MAIN), View::Review, View::Tickets],
            panes: vec![View::Agent(MAIN), View::Review],
            focused: 0,
            side: Side::Agents,
            composer,
            feedback,
            new_ticket,
            file_filter,
            trees: HashMap::new(),
            preview: None,
            theme_pref,
            _subs: subs,
            _refresh: refresh,
        }
    }

    fn open(&mut self, v: View, cx: &mut Context<Self>) {
        if !self.tabs.contains(&v) {
            self.tabs.push(v);
        }
        if let Some(i) = self.panes.iter().position(|p| *p == v) {
            self.focused = i;
        } else {
            self.panes[self.focused] = v;
        }
        cx.notify();
    }

    /// Open beside the focused pane, splitting if there is room.
    fn open_beside(&mut self, v: View, cx: &mut Context<Self>) {
        if !self.tabs.contains(&v) {
            self.tabs.push(v);
        }
        if self.panes.len() < 4 {
            self.panes.insert(self.focused + 1, v);
            self.focused += 1;
        } else {
            self.focused = (self.focused + 1) % self.panes.len();
            self.panes[self.focused] = v;
        }
        cx.notify();
    }

    fn close_tab(&mut self, v: View, cx: &mut Context<Self>) {
        if v == View::Agent(MAIN) {
            return;
        }
        self.tabs.retain(|t| *t != v);
        for p in self.panes.iter_mut().filter(|p| **p == v) {
            *p = View::Agent(MAIN);
        }
        cx.notify();
    }

    fn set_layout(&mut self, n: usize, cx: &mut Context<Self>) {
        let defaults = [
            View::Agent(MAIN),
            View::Review,
            View::Files(MAIN),
            View::Tickets,
        ];
        while self.panes.len() < n {
            let next = defaults
                .iter()
                .find(|d| !self.panes.contains(d))
                .copied()
                .unwrap_or(View::Tickets);
            if !self.tabs.contains(&next) {
                self.tabs.push(next);
            }
            self.panes.push(next);
        }
        self.panes.truncate(n);
        self.focused = self.focused.min(n - 1);
        cx.notify();
    }

    fn title(&self, v: View) -> String {
        match v {
            View::Agent(MAIN) => "Main agent".into(),
            View::Agent(id) => self
                .snap
                .agents
                .get(id)
                .map(|a| a.title.clone())
                .unwrap_or_default(),
            View::Review => format!("Review ({})", self.snap.pending_approvals().count()),
            View::Tickets => "Tickets".into(),
            View::Files(id) => match self.snap.agents.get(id).and_then(|a| a.ticket.clone()) {
                Some(k) => format!("{k} worktree"),
                None => "Primary worktree".into(),
            },
        }
    }

    fn tree(&mut self, id: usize) -> &Vec<Entry> {
        let root = self
            .snap
            .agents
            .get(id)
            .and_then(|a| a.worktree.clone())
            .unwrap_or_else(|| self.snap.workspace.clone());
        let stale = self
            .trees
            .get(&id)
            .is_none_or(|(at, _)| at.elapsed() > Duration::from_secs(2));
        if stale {
            let mut out = Vec::new();
            scan(&root, 0, &mut out);
            self.trees.insert(id, (Instant::now(), out));
        }
        &self.trees[&id].1
    }

    // ------------------------------------------------------------- chrome

    fn titlebar(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let pending = self.snap.pending_approvals().count();
        h_flex()
            .h(px(40.))
            .px_3()
            .gap_3()
            .flex_none()
            .child(
                div()
                    .w(px(220.))
                    .flex_none()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("⌫ backspace"),
            )
            .child(
                h_flex()
                    .id("tabs")
                    .flex_1()
                    .min_w_0()
                    .justify_center()
                    .gap_1()
                    .overflow_x_scroll()
                    .children(self.tabs.iter().copied().enumerate().map(|(i, v)| {
                        let shown = self.panes.contains(&v);
                        let active = self.panes.get(self.focused) == Some(&v);
                        h_flex()
                            .id(("tab", i))
                            .flex_none()
                            .gap_1()
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .border_1()
                            .border_color(if active {
                                t.foreground.opacity(0.5)
                            } else {
                                t.border
                            })
                            .when(shown, |d| d.bg(t.list_active))
                            .when(!shown, |d| d.text_color(t.muted_foreground))
                            .hover(|d| d.bg(t.list_hover))
                            .on_click(cx.listener(move |this, _, _, cx| this.open(v, cx)))
                            .child(self.title(v))
                            .when(v == View::Review && pending > 0, |d| {
                                d.child(div().size(px(6.)).rounded_full().bg(t.warning))
                            })
                            .when(v != View::Agent(MAIN), |d| {
                                d.child(
                                    div()
                                        .id(("close-tab", i))
                                        .text_color(t.muted_foreground)
                                        .hover(|d| d.text_color(t.foreground))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.close_tab(v, cx)
                                        }))
                                        .child("×"),
                                )
                            })
                    })),
            )
            .child(h_flex().gap_1().flex_none().children((1..=4).map(|n| {
                let on = self.panes.len() == n;
                let ink = if on {
                    t.foreground.opacity(0.6)
                } else {
                    t.muted_foreground.opacity(0.5)
                };
                let cell = move || div().flex_1().rounded(px(1.)).bg(ink);
                div()
                    .id(("layout", n))
                    .w(px(26.))
                    .h(px(20.))
                    .p(px(3.))
                    .rounded_sm()
                    .cursor_pointer()
                    .border_1()
                    .border_color(if on {
                        t.foreground.opacity(0.6)
                    } else {
                        t.border
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.set_layout(n, cx)))
                    .child(if n == 4 {
                        v_flex()
                            .size_full()
                            .gap(px(2.))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .gap(px(2.))
                                    .child(cell().h_full())
                                    .child(cell().h_full()),
                            )
                            .child(
                                h_flex()
                                    .flex_1()
                                    .gap(px(2.))
                                    .child(cell().h_full())
                                    .child(cell().h_full()),
                            )
                            .into_any_element()
                    } else {
                        h_flex()
                            .size_full()
                            .gap(px(2.))
                            .children((0..n).map(|_| cell().h_full()))
                            .into_any_element()
                    })
            })))
            .child(self.theme_toggle(t, cx))
            .child(
                div().w(px(150.)).flex_none().flex().justify_end().child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_full()
                        .border_1()
                        .border_color(t.border)
                        .text_xs()
                        .font_family(MONO)
                        .child(format!("${:.3}", self.snap.total_cost_usd)),
                ),
            )
    }

    /// A half-filled circle drawn with divs (no glyph to go missing in a
    /// font), plus the current preference.
    fn theme_toggle(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("theme")
            .flex_none()
            .gap_1p5()
            .px_2()
            .h(px(20.))
            .rounded_sm()
            .border_1()
            .border_color(t.border)
            .cursor_pointer()
            .text_xs()
            .hover(|d| d.bg(t.list_hover))
            .on_click(cx.listener(|this, _, window, cx| {
                this.theme_pref = this.theme_pref.next();
                this.theme_pref.save();
                this.theme_pref.apply(window, cx);
                cx.notify();
            }))
            .child(
                h_flex()
                    .size(px(10.))
                    .rounded_full()
                    .border_1()
                    .border_color(t.foreground)
                    .overflow_hidden()
                    .child(div().w_1_2().h_full().bg(t.foreground)),
            )
            .child(self.theme_pref.label())
    }

    fn sidebar(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let seg = |id: &'static str, label: &'static str, side: Side, cx: &mut Context<Self>| {
            div()
                .id(id)
                .px_2()
                .py_0p5()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .when(self.side == side, |d| d.bg(t.list_active))
                .when(self.side != side, |d| d.text_color(t.muted_foreground))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.side = side;
                    cx.notify();
                }))
                .child(label)
        };
        v_flex()
            .size_full()
            .pt_1()
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .truncate()
                            .child(self.snap.name.clone()),
                    )
                    .child(div().flex_1())
                    .child(seg("side-agents", "Agents", Side::Agents, cx))
                    .child(seg("side-tickets", "Tickets", Side::Tickets, cx)),
            )
            .child(
                v_flex()
                    .id("side-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_1p5()
                    .gap_px()
                    .map(|d| match self.side {
                        Side::Agents => d.children(
                            tree_order(&self.snap)
                                .into_iter()
                                .map(|id| self.agent_row(&self.snap.agents[id], t, cx)),
                        ),
                        Side::Tickets => d.children(self.ticket_groups(t, cx)),
                    }),
            )
            .child(self.run_card(t, cx))
    }

    fn agent_row(&self, a: &AgentRecord, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let id = a.id;
        let open = self.panes.contains(&View::Agent(id));
        v_flex()
            .id(("agent", id))
            .pl(px(10. + 14. * a.depth as f32))
            .pr_2()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .when(open, |d| d.bg(t.list_active))
            .hover(|d| d.bg(t.list_hover))
            .on_click(cx.listener(move |this, _, _, cx| this.open(View::Agent(id), cx)))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .size(px(7.))
                            .flex_none()
                            .rounded_full()
                            .bg(status_color(a.status, t)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .min_w_0()
                            .truncate()
                            .when(a.kind == AgentKind::Triage, |d| {
                                d.text_color(t.muted_foreground)
                            })
                            .child(a.title.clone()),
                    )
                    .when(!a.escalations.is_empty(), |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(t.warning)
                                .child(format!("↑{}", a.escalations.len())),
                        )
                    }),
            )
            .child(
                div()
                    .pl(px(15.))
                    .text_size(px(11.))
                    .font_family(MONO)
                    .text_color(t.muted_foreground)
                    .truncate()
                    .child(diagram::route_line(a)),
            )
    }

    fn ticket_groups(&self, t: &Theme, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let groups: [(&str, &[TicketState]); 5] = [
            (
                "Needs you",
                &[
                    TicketState::Proposed,
                    TicketState::InReview,
                    TicketState::ReadyForHuman,
                    TicketState::NeedsInfo,
                ],
            ),
            ("Running", &[TicketState::Queued, TicketState::InProgress]),
            (
                "Ready",
                &[TicketState::ReadyForAgent, TicketState::NeedsTriage],
            ),
            ("Done", &[TicketState::Done]),
            ("Closed", &[TicketState::Failed, TicketState::Wontfix]),
        ];
        let mut out = Vec::new();
        for (name, states) in groups {
            let rows: Vec<_> = self
                .snap
                .tickets
                .iter()
                .filter(|x| states.contains(&x.state))
                .collect();
            if rows.is_empty() {
                continue;
            }
            out.push(
                div()
                    .px_2()
                    .pt_2()
                    .pb_1()
                    .text_xs()
                    .text_color(t.muted_foreground)
                    .child(format!("{name} · {}", rows.len()))
                    .into_any_element(),
            );
            for tk in rows {
                let assignee = tk.assignee;
                out.push(
                    h_flex()
                        .id(("side-ticket", tk.num))
                        .px_2()
                        .py_1()
                        .gap_2()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|d| d.bg(t.list_hover))
                        .on_click(cx.listener(move |this, _, _, cx| match assignee {
                            Some(a) => this.open(View::Agent(a), cx),
                            None => this.open(View::Tickets, cx),
                        }))
                        .child(
                            div()
                                .text_xs()
                                .font_family(MONO)
                                .text_color(t.muted_foreground)
                                .child(format!("{:02}", tk.num)),
                        )
                        .child(div().text_sm().min_w_0().truncate().child(tk.title.clone()))
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(t.muted_foreground)
                                .child(tk.state.label()),
                        )
                        .into_any_element(),
                );
            }
        }
        if out.is_empty() {
            out.push(
                div()
                    .p_3()
                    .text_sm()
                    .text_color(t.muted_foreground)
                    .child("No tickets yet.")
                    .into_any_element(),
            );
        }
        out
    }

    fn run_card(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let s = &self.snap;
        let running = s
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Running)
            .count();
        let pending = s.pending_approvals().count();
        let row = |label: String, value: String, c: Hsla| {
            h_flex()
                .gap_2()
                .text_xs()
                .child(div().size(px(6.)).flex_none().rounded_full().bg(c))
                .child(div().flex_none().child(label))
                .child(div().flex_1())
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_family(MONO)
                        .text_color(t.muted_foreground)
                        .child(value),
                )
        };
        v_flex()
            .id("run-card")
            .m_2()
            .p_2p5()
            .gap_1p5()
            .rounded_lg()
            .border_1()
            .border_color(t.border)
            .bg(t.background)
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.open(View::Review, cx)))
            .child(
                div()
                    .text_xs()
                    .text_color(t.muted_foreground)
                    .child("This run"),
            )
            .child(row("Agents running".into(), running.to_string(), t.info))
            .child(row(
                "Waiting on you".into(),
                pending.to_string(),
                if pending > 0 {
                    t.warning
                } else {
                    t.muted_foreground
                },
            ))
            .child(row(
                "Spent".into(),
                format!(
                    "${:.3} · router ${:.3}",
                    s.total_cost_usd, s.router_cost_usd
                ),
                t.success,
            ))
    }

    // -------------------------------------------------------------- panes

    fn pane(
        &mut self,
        i: usize,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let v = self.panes[i];
        let focused = i == self.focused;
        let title = self.title(v);
        let agent_id = match v {
            View::Agent(id) => Some(id),
            _ => None,
        };
        let body = match v {
            View::Agent(id) => self.agent_view(id, t, cx).into_any_element(),
            View::Review => self.review_view(t, cx).into_any_element(),
            View::Tickets => self.tickets_view(t, cx).into_any_element(),
            View::Files(id) => self.files_view(id, t, window, cx).into_any_element(),
        };
        let closable = self.panes.len() > 1;
        v_flex()
            .size_full()
            .rounded_lg()
            .border_1()
            .border_color(if focused {
                t.foreground.opacity(0.28)
            } else {
                t.border
            })
            .bg(t.background.opacity(if GLASS { 0.9 } else { 1. }))
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    if this.focused != i {
                        this.focused = i;
                        cx.notify();
                    }
                }),
            )
            .child(
                h_flex()
                    .h(px(34.))
                    .flex_none()
                    .px_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(t.border)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(title),
                    )
                    .child(div().flex_1())
                    .when_some(agent_id, |d, id| {
                        d.child(
                            div()
                                .id(("files-btn", i))
                                .text_xs()
                                .text_color(t.muted_foreground)
                                .cursor_pointer()
                                .hover(|d| d.text_color(t.foreground))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_beside(View::Files(id), cx)
                                }))
                                .child("open files →"),
                        )
                    })
                    .when(closable, |d| {
                        d.child(
                            div()
                                .id(("close-pane", i))
                                .text_sm()
                                .text_color(t.muted_foreground)
                                .cursor_pointer()
                                .hover(|d| d.text_color(t.foreground))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.panes.remove(i);
                                    this.focused = this.focused.min(this.panes.len() - 1);
                                    cx.notify();
                                }))
                                .child("×"),
                        )
                    }),
            )
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    fn agent_view(&self, id: usize, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(a) = self.snap.agents.get(id) else {
            return v_flex().into_any_element();
        };
        let is_main = id == MAIN;
        let empty = is_main && !a.log.iter().any(|e| e.kind == LogKind::User);
        let status = {
            let mut parts = vec![format!("[{}]", diagram::route_line(a))];
            if let Some(k) = &a.ticket {
                parts.push(k.clone());
            }
            if let Some(b) = &a.branch {
                parts.push(format!("⎇ {b}"));
            }
            parts.push(format!("${:.3}", self.snap.subtree_cost(id)));
            parts.push(format!("{}↓ {}↑", a.input_tokens, a.output_tokens));
            parts.push(status_label(a.status).to_string());
            parts.join("  ·  ")
        };
        v_flex()
            .size_full()
            .child(
                v_flex()
                    .id(("log", id))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_3()
                    .py_2()
                    .gap_1p5()
                    .font_family(MONO)
                    .text_size(px(12.5))
                    .when(empty, |d| d.justify_center().child(self.welcome(t, cx)))
                    .children(a.log.iter().map(|e| log_line(e.kind, &e.text, t))),
            )
            .when(is_main, |d| {
                d.child(
                    div()
                        .px_3()
                        .pt_2()
                        .border_t_1()
                        .border_color(t.border)
                        .child(Input::new(&self.composer)),
                )
            })
            .child(
                div()
                    .px_3()
                    .py_1p5()
                    .text_size(px(11.))
                    .font_family(MONO)
                    .text_color(t.muted_foreground)
                    .truncate()
                    .child(status),
            )
            .into_any_element()
    }

    fn welcome(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let card = |id: &'static str, title: &'static str, sub: &'static str| {
            v_flex()
                .id(id)
                .w(px(180.))
                .p_3()
                .gap_1()
                .rounded_lg()
                .border_1()
                .border_color(t.border)
                .bg(t.muted.opacity(0.3))
                .cursor_pointer()
                .hover(|d| d.border_color(t.foreground.opacity(0.4)))
                .font_family(t.font_family.clone())
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(div().text_xs().text_color(t.muted_foreground).child(sub))
        };
        v_flex()
            .items_center()
            .gap_4()
            .py_8()
            .child(
                div()
                    .text_sm()
                    .text_color(t.muted_foreground)
                    .font_family(t.font_family.clone())
                    .child(
                        "Type a goal below. The main agent interviews you, then proposes tickets.",
                    ),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_center()
                    .gap_2()
                    .child(
                        card("w-review", "Review queue", "Plans and work waiting on you").on_click(
                            cx.listener(|this, _, _, cx| this.open_beside(View::Review, cx)),
                        ),
                    )
                    .child(
                        card("w-tickets", "Tickets", "Board, and file a ticket").on_click(
                            cx.listener(|this, _, _, cx| this.open_beside(View::Tickets, cx)),
                        ),
                    )
                    .child(
                        card("w-files", "Worktree", "Browse the project files").on_click(
                            cx.listener(|this, _, _, cx| this.open_beside(View::Files(MAIN), cx)),
                        ),
                    ),
            )
    }

    fn review_view(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let pending: Vec<&Approval> = self.snap.pending_approvals().collect();
        v_flex()
            .size_full()
            .child(
                v_flex()
                    .id("review-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_3()
                    .gap_4()
                    .when(pending.is_empty(), |d| d.child(div().p_2().text_sm().text_color(t.muted_foreground).child("Nothing waiting on you.")))
                    .children(pending.into_iter().map(|ap| self.review_card(ap, t, cx))),
            )
            .child(
                v_flex()
                    .p_3()
                    .gap_1()
                    .border_t_1()
                    .border_color(t.border)
                    .child(div().text_xs().text_color(t.muted_foreground).child("Rejecting sends this feedback into the agent's conversation and moves it one rung up the model ladder."))
                    .child(Input::new(&self.feedback)),
            )
    }

    fn review_card(&self, ap: &Approval, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let id = ap.id;
        let agent = ap.agent;
        let d = diagram::for_approval(&self.snap, ap);
        let kind = match ap.kind {
            ApprovalKind::Plan => format!("Plan · {} tickets", ap.tickets.len()),
            ApprovalKind::Deliverable if ap.agent == MAIN => "Final deliverable".into(),
            ApprovalKind::Deliverable => format!("Ticket {}", ap.tickets.join(", ")),
        };
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(div().text_xs().text_color(ink(t.warning, t)).child(kind))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(self.snap.agents[agent].title.clone()),
                    ),
            )
            // Diagram first: the shape of the work before the words about it.
            .child(diagram_view::card(&d, t, id, MONO))
            .child(div().text_sm().child(ap.deliverable.summary.clone()))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new(("approve", id))
                            .primary()
                            .label("Approve")
                            .on_click(cx.listener(move |this, _, _, _| this.harness.approve(id))),
                    )
                    .child(
                        Button::new(("reject", id))
                            .danger()
                            .label("Reject")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                let fb = this.feedback.read(cx).value().trim().to_string();
                                if fb.is_empty() {
                                    return;
                                }
                                this.harness.reject(id, fb);
                                this.feedback
                                    .update(cx, |s, cx| s.set_value("", window, cx));
                            })),
                    )
                    .child(Button::new(("log", id)).ghost().label("Session").on_click(
                        cx.listener(move |this, _, _, cx| this.open_beside(View::Agent(agent), cx)),
                    ))
                    .when(ap.kind == ApprovalKind::Deliverable && agent != MAIN, |d| {
                        d.child(Button::new(("files", id)).ghost().label("Files").on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.open_beside(View::Files(agent), cx)
                            }),
                        ))
                    }),
            )
    }

    fn tickets_view(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                v_flex()
                    .id("ticket-board")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_3()
                    .gap_2()
                    .children(self.snap.tickets.iter().map(|tk| {
                        let assignee = tk.assignee;
                        v_flex()
                            .id(("ticket", tk.num))
                            .p_2()
                            .gap_0p5()
                            .rounded_md()
                            .border_1()
                            .border_color(t.border)
                            .when(assignee.is_some(), |d| {
                                d.cursor_pointer().hover(|d| d.bg(t.list_hover))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(a) = assignee {
                                    this.open(View::Agent(a), cx)
                                }
                            }))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family(MONO)
                                            .text_color(t.muted_foreground)
                                            .child(format!("{:02}", tk.num)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .min_w_0()
                                            .truncate()
                                            .child(tk.title.clone()),
                                    )
                                    .child(div().flex_1())
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ticket_color(tk.state, t))
                                            .child(tk.state.label()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(t.muted_foreground)
                                    .child(format!(
                                        "{} · {} criteria{}{}",
                                        tk.key,
                                        tk.acceptance.len(),
                                        if tk.check.is_some() {
                                            " · check"
                                        } else {
                                            " · no check"
                                        },
                                        if tk.blocked_by.is_empty() {
                                            String::new()
                                        } else {
                                            format!(" · after {}", tk.blocked_by.join(", "))
                                        }
                                    )),
                            )
                    })),
            )
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(t.border)
                    .child(Input::new(&self.new_ticket)),
            )
    }

    fn files_view(
        &mut self,
        id: usize,
        t: &Theme,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let filter = self.file_filter.read(cx).value().to_lowercase();
        let preview = self.preview.clone();
        let rows: Vec<(usize, String, PathBuf, bool)> = self
            .tree(id)
            .iter()
            .filter(|e| filter.is_empty() || e.name.to_lowercase().contains(&filter))
            .take(400)
            .map(|e| (e.depth, e.name.clone(), e.path.clone(), e.dir))
            .collect();
        v_flex()
            .size_full()
            .child(div().p_2().child(Input::new(&self.file_filter)))
            .child(
                v_flex()
                    .id(("tree", id))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .font_family(MONO)
                    .text_size(px(12.))
                    .children(
                        rows.into_iter()
                            .enumerate()
                            .map(|(n, (depth, name, path, dir))| {
                                let p = path.clone();
                                h_flex()
                                    .id(("file", n))
                                    .pl(px(6. + depth as f32 * 14.))
                                    .py(px(2.))
                                    .gap_1p5()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .hover(|d| d.bg(t.list_hover))
                                    .when(
                                        preview.as_ref().is_some_and(|(pp, _)| *pp == path),
                                        |d| d.bg(t.list_active),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !dir {
                                            let text = std::fs::read_to_string(&p)
                                                .map(|s| {
                                                    s.lines()
                                                        .take(300)
                                                        .collect::<Vec<_>>()
                                                        .join("\n")
                                                })
                                                .unwrap_or_else(|_| {
                                                    "(binary or unreadable)".into()
                                                });
                                            this.preview = Some((p.clone(), text));
                                            cx.notify();
                                        }
                                    }))
                                    .child(
                                        div()
                                            .text_color(if dir {
                                                t.muted_foreground
                                            } else {
                                                t.info
                                            })
                                            .child(if dir { "▸" } else { "·" }),
                                    )
                                    .child(div().truncate().child(name))
                            }),
                    ),
            )
            .when_some(preview, |d, (path, text)| {
                d.child(
                    v_flex()
                        .h(px(260.))
                        .flex_none()
                        .border_t_1()
                        .border_color(t.border)
                        .child(
                            div()
                                .px_3()
                                .py_1()
                                .text_xs()
                                .text_color(t.muted_foreground)
                                .truncate()
                                .child(path.display().to_string()),
                        )
                        .child(
                            div()
                                .id("preview")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .px_3()
                                .pb_2()
                                .font_family(MONO)
                                .text_size(px(11.5))
                                .whitespace_normal()
                                .child(text),
                        ),
                )
            })
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        if self.focused >= self.panes.len() {
            self.focused = 0;
        }
        let panes: Vec<AnyElement> = (0..self.panes.len())
            .map(|i| self.pane(i, &t, window, cx))
            .collect();

        // Gaps between panes are the drag handles: invisible until dragged.
        let handle: ResizeHandleRenderer = Rc::new(|ctx, _, cx| {
            let c = if ctx.is_active() {
                cx.theme().foreground.opacity(0.35)
            } else {
                transparent_black()
            };
            let line = div().flex_none().bg(c);
            Some(
                match ctx.axis() {
                    Axis::Horizontal => line.h_full().w(px(2.)),
                    Axis::Vertical => line.w_full().h(px(2.)),
                }
                .into_any_element(),
            )
        });
        let cell = |el: AnyElement| resizable_panel().p(px(3.)).child(el);
        let area = if panes.len() == 4 {
            let mut it = panes.into_iter();
            let mut next = || it.next().unwrap();
            let (a, b, c, d) = (next(), next(), next(), next());
            h_resizable("grid")
                .with_handle_appearance(handle.clone())
                .child(
                    resizable_panel().child(
                        v_resizable("grid-left")
                            .with_handle_appearance(handle.clone())
                            .child(cell(a))
                            .child(cell(c)),
                    ),
                )
                .child(
                    resizable_panel().child(
                        v_resizable("grid-right")
                            .with_handle_appearance(handle.clone())
                            .child(cell(b))
                            .child(cell(d)),
                    ),
                )
                .into_any_element()
        } else {
            // One id per layout so each remembers its own split sizes.
            h_resizable(("row", panes.len()))
                .with_handle_appearance(handle.clone())
                .children(panes.into_iter().map(cell))
                .into_any_element()
        };

        v_flex()
            .size_full()
            .bg(t.sidebar.opacity(if GLASS { 0.72 } else { 1. }))
            .text_color(t.foreground)
            .child(self.titlebar(&t, cx))
            .child(
                div().flex_1().min_h_0().child(
                    h_resizable("workbench")
                        .with_handle_appearance(handle)
                        .child(
                            resizable_panel()
                                .size(px(260.))
                                .size_range(px(236.)..px(440.))
                                .flex_none()
                                .child(self.sidebar(&t, cx)),
                        )
                        .child(resizable_panel().pr(px(3.)).pb(px(3.)).child(area)),
                ),
            )
    }
}

/// Theme accents are tuned for dark backgrounds; on light ones amber and
/// cyan text fades out, so pull their lightness down for text use.
fn ink(c: Hsla, t: &Theme) -> Hsla {
    if t.is_dark() {
        c
    } else {
        hsla(c.h, c.s, c.l.min(0.4), c.a)
    }
}

fn log_line(kind: LogKind, text: &str, t: &Theme) -> AnyElement {
    let (glyph, color) = match kind {
        LogKind::User => ("›", t.foreground),
        LogKind::Assistant => ("●", t.foreground),
        LogKind::ToolCall => ("⎿", ink(t.info, t)),
        LogKind::ToolResult => (" ", t.muted_foreground),
        LogKind::System => ("※", ink(t.warning, t)),
        LogKind::Error => ("✗", ink(t.danger, t)),
    };
    let body = match kind {
        // Tool output is context, not reading material: keep it short.
        LogKind::ToolResult => {
            let lines: Vec<&str> = text.lines().collect();
            if lines.len() > 6 {
                format!(
                    "{}\n… {} more lines",
                    lines[..6].join("\n"),
                    lines.len() - 6
                )
            } else {
                text.to_string()
            }
        }
        _ => text.to_string(),
    };
    h_flex()
        .items_start()
        .gap_2()
        .when(kind == LogKind::User, |d| {
            d.px_1()
                .py_0p5()
                .rounded_sm()
                .bg(t.foreground.opacity(0.9))
                .text_color(t.background)
        })
        .child(
            div()
                .w(px(10.))
                .flex_none()
                .text_color(if kind == LogKind::User {
                    t.background
                } else {
                    color
                })
                .child(glyph),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_normal()
                .when(
                    matches!(
                        kind,
                        LogKind::ToolCall | LogKind::ToolResult | LogKind::System
                    ),
                    |d| d.text_color(color),
                )
                .child(body),
        )
        .into_any_element()
}

/// Depth-first walk, skipping build output and VCS internals.
fn scan(dir: &Path, depth: usize, out: &mut Vec<Entry>) {
    if depth > 4 || out.len() > 2000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<_> = rd.flatten().collect();
    items.sort_by_key(|e| (!e.path().is_dir(), e.file_name()));
    for e in items {
        let name = e.file_name().to_string_lossy().into_owned();
        if matches!(
            name.as_str(),
            ".git" | "target" | "node_modules" | ".backspace" | "dist" | ".astro"
        ) {
            continue;
        }
        let path = e.path();
        let dir = path.is_dir();
        out.push(Entry {
            depth,
            name,
            path: path.clone(),
            dir,
        });
        if dir {
            scan(&path, depth + 1, out);
        }
    }
}

/// Depth-first, so each agent sits directly under its manager.
fn tree_order(s: &ProjectState) -> Vec<usize> {
    fn walk(s: &ProjectState, id: usize, out: &mut Vec<usize>) {
        out.push(id);
        for child in s.agents.iter().filter(|a| a.parent == Some(id)) {
            walk(s, child.id, out);
        }
    }
    let mut out = Vec::with_capacity(s.agents.len());
    walk(s, MAIN, &mut out);
    out
}

fn status_color(s: AgentStatus, t: &Theme) -> Hsla {
    match s {
        AgentStatus::Idle | AgentStatus::Queued => t.muted_foreground,
        AgentStatus::Running => t.info,
        AgentStatus::AwaitingApproval => t.warning,
        AgentStatus::Approved => t.success,
        AgentStatus::Failed => t.danger,
    }
}

fn status_label(s: AgentStatus) -> &'static str {
    match s {
        AgentStatus::Idle => "idle",
        AgentStatus::Queued => "queued",
        AgentStatus::Running => "running",
        AgentStatus::AwaitingApproval => "waiting for review",
        AgentStatus::Approved => "approved",
        AgentStatus::Failed => "failed",
    }
}

fn ticket_color(s: TicketState, t: &Theme) -> Hsla {
    match s {
        TicketState::Done => t.success,
        TicketState::Failed => t.danger,
        TicketState::InProgress | TicketState::InReview | TicketState::Queued => t.info,
        TicketState::NeedsTriage
        | TicketState::NeedsInfo
        | TicketState::ReadyForHuman
        | TicketState::Proposed => t.warning,
        _ => t.muted_foreground,
    }
}

fn main() -> anyhow::Result<()> {
    let ws = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    let harness = Harness::open(ws)?;

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Backspace".into()),
                        ..Default::default()
                    }),
                    window_background: if GLASS {
                        WindowBackgroundAppearance::Blurred
                    } else {
                        WindowBackgroundAppearance::Opaque
                    },
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| Workbench::new(harness, window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("failed to open window");
            cx.activate(true);
        });
    Ok(())
}
