//! Backspace desktop shell. Three columns: agents | selected agent's log |
//! approval queue. You talk to the main agent; everything else is review.
//!
//!   backspace [workspace]     (defaults to the current directory)

use std::path::PathBuf;
use std::rc::Rc;

use backspace_core::{AgentRecord, AgentStatus, Approval, Harness, LogKind, ProjectState, MAIN};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Root, Theme, ThemeMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

struct Backspace {
    harness: Rc<Harness>,
    snap: ProjectState,
    selected: usize,
    composer: Entity<InputState>,
    feedback: Entity<InputState>,
    _subs: Vec<Subscription>,
    _refresh: Task<()>,
}

impl Backspace {
    fn new(harness: Harness, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let harness = Rc::new(harness);
        let composer = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Describe the goal, or reply to the main agent…")
        });
        let feedback =
            cx.new(|cx| InputState::new(window, cx).placeholder("Feedback (required to reject)"));

        let subs = vec![cx.subscribe_in(
            &composer,
            window,
            |this, input, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    let text = input.read(cx).value().trim().to_string();
                    if !text.is_empty() {
                        this.harness.send(text);
                        input.update(cx, |s, cx| s.set_value("", window, cx));
                        this.selected = MAIN;
                    }
                }
            },
        )];

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
            selected: MAIN,
            composer,
            feedback,
            _subs: subs,
            _refresh: refresh,
        }
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        let s = &self.snap;
        let pending = s.pending_approvals().count();
        v_flex()
            .w(px(260.))
            .h_full()
            .flex_none()
            .bg(t.sidebar)
            .border_r_1()
            .border_color(t.border)
            .child(
                v_flex()
                    .p_3()
                    .gap_1()
                    .border_b_1()
                    .border_color(t.border)
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child("⌫ backspace"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(t.muted_foreground)
                            .child(s.name.clone()),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_x_3()
                            .text_xs()
                            .text_color(t.muted_foreground)
                            .child(format!("${:.4} total", s.total_cost_usd))
                            .child(format!("${:.4} router", s.router_cost_usd))
                            .child(format!("{pending} to review")),
                    ),
            )
            .child(
                v_flex()
                    .id("agents")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_2()
                    .gap_1()
                    .children(
                        tree_order(s)
                            .into_iter()
                            .map(|id| self.agent_row(&s.agents[id], &t, cx)),
                    ),
            )
    }

    fn agent_row(&self, a: &AgentRecord, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let id = a.id;
        let route = a
            .decision
            .as_ref()
            .map(|d| format!("{} · {}", d.model, d.effort))
            .unwrap_or_else(|| "not routed yet".into());
        let indent = px(14. * a.depth as f32);
        let spent = self.snap.subtree_cost(a.id);
        let money = match a.budget_usd {
            Some(b) => format!("${spent:.3} / ${b:.2}"),
            None if spent > a.cost_usd => format!("${spent:.3} tree"),
            None => format!("${:.3}", a.cost_usd),
        };
        v_flex()
            .id(("agent", id))
            .pl(indent + px(8.))
            .pr_2()
            .py_1p5()
            .rounded_md()
            .cursor_pointer()
            .when(self.selected == id, |d| d.bg(t.list_active))
            .hover(|d| d.bg(t.list_hover))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected = id;
                cx.notify();
            }))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .size(px(8.))
                            .rounded_full()
                            .bg(status_color(a.status, t)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(a.title.clone()),
                    ),
            )
            .child(
                h_flex()
                    .justify_between()
                    .text_xs()
                    .text_color(t.muted_foreground)
                    .child(route)
                    .child(money),
            )
    }

    fn log_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        let a = &self.snap.agents[self.selected.min(self.snap.agents.len() - 1)];
        let is_main = a.id == MAIN;
        let empty = is_main && a.log.is_empty();

        v_flex()
            .flex_1()
            .h_full()
            .min_w_0()
            .child(
                h_flex()
                    .px_4()
                    .py_2()
                    .gap_3()
                    .border_b_1()
                    .border_color(t.border)
                    .child(div().font_weight(FontWeight::BOLD).child(a.title.clone()))
                    .child(div().text_xs().text_color(t.muted_foreground).child(status_label(a.status)))
                    .child(div().text_xs().text_color(t.muted_foreground).child(format!(
                        "{} in / {} out tokens",
                        a.input_tokens, a.output_tokens
                    ))),
            )
            .child(
                v_flex()
                    .id(("log", a.id))
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .gap_2()
                    .when(empty, |d| {
                        d.justify_center().items_center().child(
                            div()
                                .text_color(t.muted_foreground)
                                .child("State a goal. The main agent plans, spawns sub-agents, and brings you deliverables to approve."),
                        )
                    })
                    .children(a.log.iter().map(|e| {
                        let (label, color) = match e.kind {
                            LogKind::User => ("you", t.primary),
                            LogKind::Assistant => ("agent", t.foreground),
                            LogKind::ToolCall => ("tool", t.info),
                            LogKind::ToolResult => ("result", t.muted_foreground),
                            LogKind::System => ("router", t.warning),
                            LogKind::Error => ("error", t.danger),
                        };
                        let mono = matches!(e.kind, LogKind::ToolCall | LogKind::ToolResult);
                        v_flex()
                            .gap_0p5()
                            .child(div().text_xs().text_color(color).child(label))
                            .child(
                                div()
                                    .text_sm()
                                    .when(mono, |d| d.font_family("monospace").text_color(t.muted_foreground))
                                    .child(e.text.clone()),
                            )
                    })),
            )
            .when(is_main, |d| {
                d.child(
                    div()
                        .p_3()
                        .border_t_1()
                        .border_color(t.border)
                        .child(Input::new(&self.composer)),
                )
            })
    }

    fn approvals(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        let pending: Vec<&Approval> = self.snap.pending_approvals().collect();
        v_flex()
            .w(px(360.))
            .h_full()
            .flex_none()
            .border_l_1()
            .border_color(t.border)
            .child(
                div()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(t.border)
                    .font_weight(FontWeight::BOLD)
                    .child(format!("Review ({})", pending.len())),
            )
            .child(
                v_flex()
                    .id("approvals")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_3()
                    .gap_3()
                    .when(pending.is_empty(), |d| {
                        d.child(
                            div()
                                .text_sm()
                                .text_color(t.muted_foreground)
                                .child("Nothing waiting on you."),
                        )
                    })
                    .children(pending.into_iter().map(|ap| self.approval_card(ap, &t, cx))),
            )
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .border_t_1()
                    .border_color(t.border)
                    .child(div().text_xs().text_color(t.muted_foreground).child(
                        "Rejection feedback goes back to the agent, which revises and resubmits.",
                    ))
                    .child(Input::new(&self.feedback)),
            )
    }

    fn approval_card(&self, ap: &Approval, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let id = ap.id;
        let agent = &self.snap.agents[ap.agent];
        let is_final = ap.agent == MAIN;
        v_flex()
            .p_3()
            .gap_2()
            .rounded_lg()
            .border_1()
            .border_color(if is_final { t.primary } else { t.border })
            .bg(t.background)
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child(agent.title.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.muted_foreground)
                            .child(if is_final {
                                "final deliverable"
                            } else {
                                "sub-agent"
                            }),
                    ),
            )
            .child(div().text_sm().child(ap.deliverable.summary.clone()))
            .when(!ap.deliverable.files.is_empty(), |d| {
                d.child(
                    div()
                        .text_xs()
                        .font_family("monospace")
                        .text_color(t.muted_foreground)
                        .child(ap.deliverable.files.join("\n")),
                )
            })
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
                    .child(
                        Button::new(("open", id))
                            .ghost()
                            .label("Log")
                            .on_click(cx.listener({
                                let agent = ap.agent;
                                move |this, _, _, cx| {
                                    this.selected = agent;
                                    cx.notify();
                                }
                            })),
                    ),
            )
    }
}

impl Render for Backspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        h_flex()
            .size_full()
            .bg(t.background)
            .text_color(t.foreground)
            .child(self.sidebar(cx))
            .child(self.log_view(cx))
            .child(self.approvals(cx))
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
        AgentStatus::AwaitingApproval => "awaiting your review",
        AgentStatus::Approved => "approved",
        AgentStatus::Failed => "failed",
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
            Theme::change(ThemeMode::Dark, None, cx);
            let bounds = Bounds::centered(None, size(px(1400.), px(880.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Backspace".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| Backspace::new(harness, window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("failed to open window");
            cx.activate(true);
        });
    Ok(())
}
