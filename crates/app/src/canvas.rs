//! The canvases a tab holds: agent session, worktree, browser, diagram
//! review, PLAN.md, and the picker an empty canvas shows.

use backspace_core::diagram::{self, Diagram};
use backspace_core::prefs::PaneSpec;
use backspace_core::{
    AgentRecord, AgentStatus, Approval, ApprovalKind, ApprovalState, LogKind, MAIN,
};
use gpui_kit::component::input::Input;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::stage::{self, Cam};
use crate::ui::*;
use crate::*;

impl Workbench {
    fn agent_body(&mut self, slot: usize, a: &AgentRecord, w: Pixels) -> impl IntoElement {
        let p = self.pal;
        // Stay pinned to the tail unless the reader scrolled up; a pane that
        // just opened (or changed agent) starts at the tail.
        let sh = &self.panes[slot].scroll;
        let near_bottom = -sh.offset().y >= sh.max_offset().y - px(40.);
        if near_bottom || self.panes[slot].seen != a.id {
            sh.scroll_to_bottom();
        }
        self.panes[slot].seen = a.id;
        let s = &self.snap;

        let path = a
            .worktree
            .clone()
            .unwrap_or_else(|| s.workspace.clone())
            .display()
            .to_string();
        let routed = match &a.decision {
            Some(d) if d.confidence > 0. => format!(
                "{} · routed by {} ({}% confident)",
                route(a),
                d.source,
                (d.confidence * 100.).round()
            ),
            Some(d) => format!("{} · routed by {}", route(a), d.source),
            None => route(a),
        };
        let banner = div()
            .flex()
            .gap(px(14.))
            .items_center()
            .mt(px(4.))
            .child(mascot(p.accent))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .line_height(relative(1.45))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child("Backspace v0.1.0"),
                    )
                    .child(div().text_color(p.term_dim).child(routed))
                    .child(div().text_color(p.term_dim).child(path)),
            );
        let spin = SPIN[self.spin];
        let mut lines: Vec<AnyElement> = vec![];
        // CSS collapses adjacent margins: each gap is the larger of the two.
        let mut prev_mb = 14.; // the banner's margin-bottom
        for e in &a.log {
            let (mt, mb) = log_margins(e.kind, &e.text);
            lines.push(
                div()
                    .mt(px(mt.max(prev_mb)))
                    .child(log_el(e.kind, &e.text, &p))
                    .into_any_element(),
            );
            prev_mb = mb;
        }
        let tail_gap = |mt: f32| px(mt.max(prev_mb));
        match a.status {
            AgentStatus::Running => lines.push(
                div()
                    .mt(tail_gap(10.))
                    .text_color(p.accent)
                    .child(format!(
                        "{spin} Working… ({} · ↓ {} tokens)",
                        a.decision
                            .as_ref()
                            .map_or("routing".into(), |d| d.model.clone()),
                        thousands(a.input_tokens)
                    ))
                    .into_any_element(),
            ),
            AgentStatus::AwaitingApproval => lines.push(
                div()
                    .mt(tail_gap(10.))
                    .text_color(p.accent)
                    .child(format!("{spin} Waiting for your review…"))
                    .into_any_element(),
            ),
            _ => {}
        }
        if a.id == MAIN && !a.log.iter().any(|e| e.kind == LogKind::User) {
            lines.push(
                div()
                    .mt(tail_gap(8.))
                    .child(note(
                        &p,
                        "Type a goal below. The main agent interviews you, then proposes tickets for you to approve.",
                    ))
                    .into_any_element(),
            );
        }

        let total = s.total_cost_usd.max(0.000001);
        let sub = s.subtree_cost(a.id);
        let share = (sub / total).min(1.);
        let filled = (share * 10.).round() as usize;
        let model = a.decision.as_ref().map_or("not routed".into(), |d| {
            format!("{} @ {}", d.model, d.effort)
        });
        let branch = a.branch.clone().unwrap_or_else(|| "-".into());
        let hint: StyledText = if a.id == MAIN {
            rich(&[
                ("▸▸ plan approval on ", None),
                ("(review in Diagram)", Some(p.blue)),
                (" · ⌘1–9 switch tabs", None),
            ])
        } else {
            let text = match a.ticket.as_ref().and_then(|k| s.ticket(k)) {
                Some(tk) => format!(
                    "▸▸ check: {} · {}",
                    tk.check.clone().unwrap_or_else(|| "none".into()),
                    tk.state.label().replace('_', "-")
                ),
                None => format!("▸▸ {:?}", a.status).to_lowercase(),
            };
            StyledText::new(text)
        };
        let name = s.name.clone();
        let prompt = if a.id == MAIN {
            div()
                .flex_1()
                .min_w_0()
                .child(
                    Input::new(&self.composer)
                        .appearance(false)
                        .font_family(MONO)
                        .text_size(px(12.))
                        .h(px(18.))
                        .p_0(),
                )
                .into_any_element()
        } else {
            div()
                .text_color(p.fg4)
                .child("Only the main agent takes messages")
                .into_any_element()
        };
        let footer = div()
            .flex_none()
            .px(px(12.))
            .pb(px(7.))
            .font_family(MONO)
            .text_size(px(12.))
            .line_height(relative(1.5))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .py(px(5.))
                    .hair_t(w, p.pane_edge)
                    .hair_b(w, p.pane_edge)
                    .text_color(p.term_fg)
                    .child(div().text_color(p.term_dim).child("❯"))
                    .child(prompt),
            )
            .child(
                div()
                    .mt(px(4.))
                    .text_color(p.term_dim)
                    .truncate()
                    .child(rich(&[
                        (&format!("[{model}] "), None),
                        (&format!("▣ {name}"), Some(p.blue)),
                        (" | ", None),
                        (&format!("⎇ {branch}"), Some(p.green)),
                    ])),
            )
            .child(div().text_color(p.term_dim).truncate().child(rich(&[
                (&"█".repeat(filled), Some(p.green)),
                (&"░".repeat(10 - filled), Some(p.green)),
                (&format!(" {}% of spend | ", (share * 100.).round()), None),
                (&format!("${sub:.3}"), Some(p.amber)),
                (
                    &format!(
                        " | {}↓ {}↑",
                        thousands(a.input_tokens),
                        thousands(a.output_tokens)
                    ),
                    None,
                ),
            ])))
            .child(div().text_color(p.accent).truncate().child(hint));
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .id(("tb", slot))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.panes[slot].scroll)
                    .pt(px(10.))
                    .px(px(12.))
                    .pb(px(6.))
                    .font_family(MONO)
                    .text_size(px(12.))
                    .line_height(relative(1.55))
                    .text_color(p.term_fg)
                    .child(banner)
                    .children(lines),
            )
            .child(footer)
    }

    fn files_body(
        &mut self,
        slot: usize,
        id: usize,
        w: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = self.pal;
        let filter = self.panes[slot].filter.read(cx).value().to_lowercase();
        let label = match self.snap.agents.get(id).and_then(|a| a.ticket.clone()) {
            Some(k) => format!("{k} worktree"),
            None => "Primary worktree".into(),
        };
        let rows: Vec<(usize, String, String, bool)> = self
            .tree(id, cx)
            .iter()
            .filter(|e| filter.is_empty() || e.name.to_lowercase().contains(&filter))
            .take(400)
            .map(|e| (e.depth, e.name.clone(), e.path.clone(), e.dir))
            .collect();
        let preview = self.panes[slot].preview.clone();
        let ext_color = |name: &str, dir: bool| {
            if dir {
                p.fg3
            } else if name.ends_with(".html") || name.ends_with(".htm") {
                p.red
            } else if name.ends_with(".py") {
                p.blue
            } else if name.ends_with(".sql") {
                p.amber
            } else if name.ends_with(".md") {
                p.violet
            } else {
                p.fg
            }
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .p(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h(px(26.))
                            .rounded(px(6.))
                            .bg(p.hover)
                            .hair_all(w, p.pane_edge)
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .child(
                                Input::new(&self.panes[slot].filter)
                                    .appearance(false)
                                    .text_size(px(12.))
                                    .h(px(24.))
                                    .p_0(),
                            ),
                    )
                    .child(
                        div()
                            .w(px(30.))
                            .h(px(24.))
                            .rounded(px(6.))
                            .hair_all(w, p.pane_edge)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.))
                            .text_color(p.fg3)
                            .child("Aa"),
                    ),
            )
            .child(
                div()
                    .id(("ftree", slot))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(6.))
                    .pb(px(8.))
                    .text_size(px(12.5))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .h(px(24.))
                            .px(px(6.))
                            .text_color(p.fg3)
                            .child(icon("chevd", 14., p.fg3))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(p.fg)
                                    .child(label),
                            ),
                    )
                    .children(
                        rows.into_iter()
                            .enumerate()
                            .map(|(n, (depth, name, path, dir))| {
                                let sel = preview.as_ref().is_some_and(|(pp, _)| *pp == path);
                                let c = ext_color(&name, dir);
                                div()
                                    .id(("ft", n))
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .h(px(24.))
                                    .pl(px(18. + depth as f32 * 14.))
                                    .pr(px(6.))
                                    .rounded(px(5.))
                                    .whitespace_nowrap()
                                    .cursor_pointer()
                                    .when(sel, |d| d.bg(p.sel))
                                    .hover(|d| d.bg(p.hover))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !dir {
                                            this.read_preview(slot, path.clone(), cx);
                                        }
                                    }))
                                    .child(icon(if dir { "chev" } else { "doc" }, 14., c))
                                    .child(name)
                            }),
                    ),
            )
            .when_some(preview, |d, (_, text)| {
                d.child(
                    div()
                        .id(("fprev", slot))
                        .max_h(relative(0.45))
                        .flex_none()
                        .overflow_y_scroll()
                        .hair_t(w, p.pane_edge)
                        .py(px(8.))
                        .px(px(12.))
                        .font_family(MONO)
                        .text_size(px(11.5))
                        .line_height(relative(1.5))
                        .text_color(p.term_fg)
                        .child(text),
                )
            })
    }

    // -------------------------------------------------------------- browser

    // -------------------------------------------------------------- diagram

    fn diagram_pane(
        &mut self,
        slot: usize,
        w: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.pal;
        let s = self.snap.clone();
        if self.review.is_none_or(|r| r >= s.approvals.len()) {
            self.review = s
                .pending_approvals()
                .next()
                .map(|a| a.id)
                .or(s.approvals.last().map(|a| a.id));
        }
        let ap = self.review.and_then(|r| s.approvals.get(r)).cloned();
        let label = |a: &Approval| match a.kind {
            ApprovalKind::Plan => format!("Plan · {} tickets", a.tickets.len()),
            ApprovalKind::Deliverable if a.agent == MAIN => "Final deliverable".into(),
            ApprovalKind::Deliverable => format!("Ticket · {}", a.tickets.join(", ")),
        };
        // Canvases are often narrow: stack the list over the drawing there.
        let narrow = f32::from(self.panes[slot].bounds.get().size.width) < 700.;
        let list = div()
            .id("dg-list")
            .map(|d| {
                if narrow {
                    d.w_full().h(px(86.)).flex_none()
                } else {
                    d.w(px(250.)).flex_none().h_full()
                }
            })
            .rounded(px(9.))
            .bg(p.pane)
            .hair_all(w, p.pane_edge)
            .p(px(8.))
            .map(|d| {
                if narrow {
                    d.overflow_x_scroll().flex().flex_row()
                } else {
                    d.overflow_y_scroll().flex().flex_col()
                }
            })
            .gap(px(4.))
            .when(!narrow, |d| {
                d.child(
                    div()
                        .mt(px(4.))
                        .mx(px(6.))
                        .mb(px(6.))
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.fg3)
                        .child("Reviews · diagram first"),
                )
            })
            .when(s.approvals.is_empty(), |d| {
                d.child(nothing(&p, "Nothing has needed your review yet."))
            })
            .children(s.approvals.iter().rev().map(|a| {
                let sel = Some(a.id) == self.review;
                let id = a.id;
                let (chip, c, edge) = match a.state {
                    ApprovalState::Pending => ("pending", p.amber, p.amber),
                    ApprovalState::Approved => ("approved", p.green, p.green),
                    ApprovalState::Rejected { .. } => ("rejected", p.fg4, p.pane_edge),
                };
                let kind = match a.kind {
                    ApprovalKind::Plan => "PLAN",
                    _ if a.agent == MAIN => "FINAL",
                    _ => "TICKET",
                };
                div()
                    .id(("rv", id))
                    .when(narrow, |d| d.min_w(px(160.)).flex_none())
                    .py(px(8.))
                    .px(px(9.))
                    .rounded(px(7.))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .cursor_pointer()
                    .hair_all(
                        w,
                        if sel {
                            p.pane_edge
                        } else {
                            transparent_black()
                        },
                    )
                    .when(sel, |d| d.bg(p.sel))
                    .hover(|d| d.bg(if sel { p.sel } else { p.hover }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.review = Some(id);
                        this.panes[slot].cam = None;
                        this.panes[slot].user_cam = false;
                        cx.notify()
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(10.5))
                            .text_color(p.fg3)
                            .child(icon("diagram", 14., p.fg3))
                            .child(kind),
                    )
                    .child(div().font_weight(FontWeight::MEDIUM).child(
                        if a.kind == ApprovalKind::Plan {
                            label(a)
                        } else {
                            s.agents[a.agent].title.clone()
                        },
                    ))
                    .child(
                        div().flex().child(
                            div()
                                .text_size(px(10.5))
                                .line_height(px(14.))
                                .px(px(7.))
                                .py(px(1.))
                                .rounded_full()
                                .hair_all(w, edge)
                                .text_color(c)
                                .child(chip),
                        ),
                    )
            }));

        let Some(ap) = ap else {
            return div()
                .size_full()
                .p(px(6.))
                .flex()
                .when(narrow, |d| d.flex_col())
                .gap(px(6.))
                .child(list)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .h_full()
                        .rounded(px(9.))
                        .bg(p.pane)
                        .hair_all(w, p.pane_edge)
                        .child(empty_mark(
                            &p,
                            "diagram",
                            "Plans and deliverables appear here, drawn by the harness before you read them.",
                        )),
                )
                .into_any_element();
        };
        let d: Diagram = diagram::for_approval(&s, &ap);

        // Fit to the stage (and the card over it) until the user pans or zooms.
        let sb = self.panes[slot].stage_bounds.get();
        let (sw, sh) = (f32::from(sb.size.width), f32::from(sb.size.height));
        let card_h = f32::from(self.panes[slot].card_bounds.get().size.height);
        let key = (ap.id, d.nodes.len(), sw, sh, card_h);
        if sw > 0.
            && (self.panes[slot].cam.is_none()
                || (!self.panes[slot].user_cam && self.panes[slot].fitted != Some(key)))
        {
            self.panes[slot].cam = Some(stage::fit(&d, sw, sh, card_h));
            self.panes[slot].fitted = Some(key);
        }
        if sw == 0. || card_h == 0. {
            // Sizes arrive with the first paint; draw once more with them.
            cx.on_next_frame(window, |_, _, cx| cx.notify());
        }
        let cam = self.panes[slot].cam.unwrap_or(Cam {
            x: 0.,
            y: 0.,
            k: 1.,
        });

        let facts = div()
            .absolute()
            .top(px(10.))
            .left(px(12.))
            .right(px(120.))
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .child(chip_f(&p, w, true).child(d.title.clone()))
            .children(d.facts.iter().map(|(k, v)| {
                chip_f(&p, w, false).child(
                    div()
                        .flex()
                        .child(
                            div()
                                .font_family(MONO)
                                .font_weight(FontWeight::MEDIUM)
                                .child(v.clone()),
                        )
                        .child(format!(" {k}")),
                )
            }));
        let zoomc = div()
            .absolute()
            .top(px(10.))
            .right(px(10.))
            .flex()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(8.))
            .bg(p.glass)
            .hair_all(w, p.pane_edge)
            .child(
                ib_with("zout", "−", &p, false)
                    .on_click(cx.listener(move |this, _, _, cx| this.zoom_center(slot, 1.25, cx))),
            )
            .child(
                ib_with("zin", "+", &p, false)
                    .on_click(cx.listener(move |this, _, _, cx| this.zoom_center(slot, 0.8, cx))),
            )
            .child(
                ib("zfit", "expand", &p, false).on_click(cx.listener(move |this, _, _, cx| {
                    this.panes[slot].cam = None;
                    this.panes[slot].user_cam = false;
                    cx.notify()
                })),
            );

        let st = ap.state.clone();
        let id = ap.id;
        let agent = ap.agent;
        let cb = self.panes[slot].card_bounds.clone();
        let actions = if st == ApprovalState::Pending {
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(
                    div()
                        .min_h(px(52.))
                        .rounded(px(8.))
                        .hair_all(w, p.pane_edge)
                        .bg(p.hover)
                        .py(px(4.))
                        .px(px(9.))
                        .child(
                            Input::new(&self.panes[slot].feedback)
                                .appearance(false)
                                .text_size(px(12.5))
                                .p_0(),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(8.))
                        .items_center()
                        .child(
                            btn("appr", "Approve", &p, w)
                                .bg(p.fg)
                                .text_color(p.desk_base)
                                .border_color(transparent_black())
                                .on_click(
                                    cx.listener(move |this, _, _, _| this.backend().approve(id)),
                                ),
                        )
                        .child(
                            btn("rej", "Reject", &p, w)
                                .text_color(p.red)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let fb = this.panes[slot]
                                        .feedback
                                        .read(cx)
                                        .value()
                                        .trim()
                                        .to_string();
                                    if fb.is_empty() {
                                        this.panes[slot].fb_err = true;
                                    } else {
                                        this.backend().reject(id, fb);
                                        this.panes[slot]
                                            .feedback
                                            .update(cx, |s, cx| s.set_value("", window, cx));
                                    }
                                    cx.notify()
                                })),
                        )
                        .child(btn("sess", "Session", &p, w).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.open(PaneSpec::agent(agent), cx)
                            }),
                        ))
                        .when(self.panes[slot].fb_err, |d| {
                            d.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.red)
                                    .child("Write what should change first."),
                            )
                        }),
                )
        } else {
            let (txt, c) = match &st {
                ApprovalState::Rejected { feedback } => (format!("rejected: {feedback}"), p.fg4),
                _ => ("approved".to_string(), p.green),
            };
            div()
                .flex()
                .gap(px(8.))
                .items_center()
                .child(
                    div()
                        .text_size(px(10.5))
                        .px(px(7.))
                        .py(px(1.))
                        .rounded_full()
                        .hair_all(w, c)
                        .text_color(c)
                        .child(txt),
                )
                .child(btn("sess", "Session", &p, w).on_click(
                    cx.listener(move |this, _, _, cx| this.open(PaneSpec::agent(agent), cx)),
                ))
        };
        let card = div()
            .id("rcard")
            .absolute()
            .bottom(px(if narrow { 8. } else { 12. }))
            .map(|d| {
                if narrow {
                    d.left(px(8.)).right(px(8.))
                } else {
                    // CSS width is content-box: 360 + 2x14 padding + hairlines.
                    d.right(px(12.)).w(px(360. + 28. + 2. * f32::from(w)))
                }
            })
            .max_h(relative(0.9))
            .overflow_y_scroll()
            .p(px(14.))
            .rounded(px(12.))
            .bg(p.glass)
            .hair_all(w, p.pane_edge)
            .shadow(vec![BoxShadow {
                color: hsla(0., 0., 0., 0.14),
                offset: point(px(0.), px(12.)),
                blur_radius: px(30.),
                spread_radius: px(0.),
                inset: false,
            }])
            .flex()
            .flex_col()
            .gap(px(10.))
            .cursor_default()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                canvas(move |b, _, _| cb.set(b), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .child(label(&ap)),
            )
            .child(
                div()
                    .text_color(p.fg2)
                    .line_height(relative(1.5))
                    .child(pre_line(&ap.deliverable.summary)),
            )
            .when(!d.warnings.is_empty(), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .children(d.warnings.iter().map(|wn| {
                            div()
                                .flex()
                                .text_size(px(12.))
                                .text_color(p.amber)
                                .child(
                                    div()
                                        .w(px(14.))
                                        .flex_none()
                                        .font_weight(FontWeight::BOLD)
                                        .child("!"),
                                )
                                .child(div().flex_1().child(wn.clone()))
                        })),
                )
            })
            .when(!d.files.is_empty(), |el| {
                el.child(stage::file_bars(&d.files, &p, MONO))
            })
            .child(actions);

        let stage_el = div()
            .id("stage")
            .flex_1()
            .min_w_0()
            .h_full()
            .relative()
            .rounded(px(9.))
            .bg(p.pane)
            .hair_all(w, p.pane_edge)
            .overflow_hidden()
            .cursor(if self.pan.is_some_and(|(s, _)| s == slot) {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            })
            .child(stage::layer(
                &d,
                cam,
                &p,
                UI.into(),
                MONO,
                self.panes[slot].stage_bounds.clone(),
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, _, _| {
                    this.pan = Some((slot, e.position))
                }),
            )
            .on_scroll_wheel(cx.listener(move |this, e: &ScrollWheelEvent, _, cx| {
                let dy = f32::from(e.delta.pixel_delta(px(16.)).y);
                let b = this.panes[slot].stage_bounds.get();
                let at = e.position - b.origin;
                if let Some(cam) = this.panes[slot].cam.as_mut() {
                    cam.zoom(
                        (-dy * 0.0015).exp(),
                        f32::from(at.x),
                        f32::from(at.y),
                        f32::from(b.size.width),
                    );
                    this.panes[slot].user_cam = true;
                    cx.notify();
                }
            }))
            .child(facts)
            .child(zoomc)
            .child(card);
        div()
            .size_full()
            .p(px(6.))
            .flex()
            .when(narrow, |d| d.flex_col())
            .gap(px(6.))
            .child(list)
            .child(stage_el)
            .into_any_element()
    }

    // -------------------------------------------------------------- docs + new

    fn docs_pane(&mut self, w: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let text = self.plan_text(cx);
        let mut body: Vec<AnyElement> = vec![];
        match text {
            None => {
                body.push(h1(&p, "PLAN.md"));
                body.push(
                    div()
                        .text_size(px(12.5))
                        .text_color(p.fg3)
                        .mb(px(26.))
                        .child(self.snap.name.clone())
                        .into_any_element(),
                );
                body.push(para(
                    &p,
                    "The main agent writes PLAN.md before it proposes tickets. It will show here once it exists.",
                ));
            }
            Some(src) => {
                let mut code: Option<Vec<String>> = None;
                for raw in src.lines() {
                    let l = raw.trim_end();
                    if l.starts_with("```") {
                        match code.take() {
                            Some(lines) => body.push(
                                div()
                                    .my(px(8.))
                                    .py(px(10.))
                                    .px(px(12.))
                                    .rounded(px(8.))
                                    .bg(p.hover)
                                    .font_family(MONO)
                                    .text_size(px(12.5))
                                    .line_height(relative(1.5))
                                    .child(lines.join("\n"))
                                    .into_any_element(),
                            ),
                            None => code = Some(vec![]),
                        }
                        continue;
                    }
                    if let Some(c) = code.as_mut() {
                        c.push(l.to_string());
                        continue;
                    }
                    let plain = l.replace("**", "").replace('`', "");
                    if let Some(h) = plain.strip_prefix("# ") {
                        body.push(h1(&p, h));
                    } else if let Some(h) = plain.strip_prefix("## ").or(plain.strip_prefix("### "))
                    {
                        body.push(
                            div()
                                .mt(px(26.))
                                .mb(px(8.))
                                .text_size(px(15.))
                                .font_weight(FontWeight::BOLD)
                                .child(h.to_string())
                                .into_any_element(),
                        );
                    } else if let Some(li) = plain.strip_prefix("- ").or(plain.strip_prefix("* ")) {
                        body.push(
                            div()
                                .flex()
                                .pl(px(6.))
                                .child(div().w(px(14.)).flex_none().child("•"))
                                .child(div().flex_1().child(li.to_string()))
                                .into_any_element(),
                        );
                    } else if !plain.trim().is_empty() {
                        body.push(para(&p, &plain));
                    }
                }
            }
        }
        div()
            .id("docs")
            .size_full()
            .overflow_y_scroll()
            .rounded(px(9.))
            .bg(p.pane)
            .hair_all(w, p.pane_edge)
            .child(
                div()
                    .max_w(px(680.))
                    .mx_auto()
                    .pt(px(40.))
                    .px(px(28.))
                    .pb(px(60.))
                    .text_size(px(14.))
                    .line_height(relative(1.6))
                    .children(body),
            )
            .into_any_element()
    }

    // -------------------------------------------------------------- grid

    fn gutter(&self, id: &'static str, d: Drag, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = self.pal;
        let vertical = d != Drag::Rows;
        let line = if self.drag == Some(d) {
            p.pane_edge_on
        } else {
            transparent_black()
        };
        div()
            .id(id)
            .flex_none()
            .relative()
            .group(id)
            .map(|el| {
                if vertical {
                    el.w(px(6.)).h_full().cursor_col_resize()
                } else {
                    el.h(px(6.)).w_full().cursor_row_resize()
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, _| this.drag = Some(d)),
            )
            .on_click(cx.listener(|this, e: &ClickEvent, _, cx| {
                if e.click_count() == 2 {
                    let t = this.tab_mut();
                    t.cols.clear();
                    t.rows = 0.56;
                    this.save_tabs();
                    cx.notify();
                }
            }))
            .child(
                div()
                    .absolute()
                    .rounded(px(2.))
                    .bg(line)
                    .group_hover(id, |s| s.bg(p.pane_edge_on))
                    .map(|el| {
                        if vertical {
                            el.top(relative(0.3))
                                .bottom(relative(0.3))
                                .left(px(2.))
                                .w(px(2.))
                        } else {
                            el.left(relative(0.3))
                                .right(relative(0.3))
                                .top(px(2.))
                                .h(px(2.))
                        }
                    }),
            )
    }

    /// The active tab's canvases: one, two or three side by side, or 2x2.
    pub(crate) fn grid(
        &mut self,
        w: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let count = self.tab().panes.len();
        if self.focus >= count {
            self.focus = 0;
        }
        if self.max.is_some_and(|m| m >= count) {
            self.max = None;
        }
        let n = if self.max.is_some() { 1 } else { count };
        let shown: Vec<usize> = match self.max {
            Some(m) => vec![m],
            None => (0..n).collect(),
        };
        let mut panes: Vec<AnyElement> =
            shown.iter().map(|&i| self.pane(i, w, window, cx)).collect();
        let gb = self.grid_bounds.clone();
        let measure = canvas(move |b, _, _| gb.set(b), |_, _, _, _| {})
            .absolute()
            .size_full();
        // The stylesheet's `calc((100% - gutters) * ratio)`, from last frame's size.
        let size = self.grid_bounds.get().size;
        let (cols, rows) = self.splits(n);
        let gutters = if n == 3 { 12. } else { 6. };
        let span = |total: Pixels, ratio: f32| -> DefiniteLength {
            if total > px(0.) {
                px((f32::from(total) - gutters) * ratio).into()
            } else {
                relative(ratio)
            }
        };
        let cell = |el: AnyElement| div().size_full().flex().child(el);
        let grid = match n {
            1 => div().size_full().flex().child(panes.remove(0)),
            2 => {
                let (a, b) = (panes.remove(0), panes.remove(0));
                div()
                    .size_full()
                    .flex()
                    .child(
                        div()
                            .w(span(size.width, cols[0]))
                            .h_full()
                            .flex_none()
                            .flex()
                            .child(a),
                    )
                    .child(self.gutter("gut-v", Drag::Col(0), cx))
                    .child(div().flex_1().min_w_0().h_full().flex().child(b))
            }
            3 => {
                let mut it = panes.into_iter();
                let (a, b, c) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
                div()
                    .size_full()
                    .flex()
                    .child(
                        div()
                            .w(span(size.width, cols[0]))
                            .h_full()
                            .flex_none()
                            .flex()
                            .child(a),
                    )
                    .child(self.gutter("gut-v", Drag::Col(0), cx))
                    .child(
                        div()
                            .w(span(size.width, cols[1] - cols[0]))
                            .h_full()
                            .flex_none()
                            .flex()
                            .child(b),
                    )
                    .child(self.gutter("gut-v2", Drag::Col(1), cx))
                    .child(div().flex_1().min_w_0().h_full().flex().child(c))
            }
            _ => {
                let mut it = panes.into_iter();
                let mut next = || cell(it.next().unwrap());
                let (a, b, c, d) = (next(), next(), next(), next());
                let cw = span(size.width, cols[0]);
                let row = |l: Div, r: Div, g: Stateful<Div>| {
                    div()
                        .w_full()
                        .flex()
                        .child(div().w(cw).h_full().flex_none().flex().child(l))
                        .child(g)
                        .child(div().flex_1().min_w_0().h_full().flex().child(r))
                };
                let (g1, g2, gh) = (
                    self.gutter("gut-v1", Drag::Col(0), cx),
                    self.gutter("gut-v2", Drag::Col(0), cx),
                    self.gutter("gut-h", Drag::Rows, cx),
                );
                let rh = if size.height > px(0.) {
                    DefiniteLength::from(px((f32::from(size.height) - 6.) * rows))
                } else {
                    relative(rows)
                };
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(row(a, b, g1).h(rh).flex_none())
                    .child(gh)
                    .child(row(c, d, g2).flex_1().min_h_0())
            }
        };
        div()
            .size_full()
            .relative()
            .child(measure)
            .child(grid)
            .into_any_element()
    }

    /// Column split fractions and the 2x2 row split for `n` canvases.
    pub(crate) fn splits(&self, n: usize) -> (Vec<f32>, f32) {
        let t = self.tab();
        let want = if n == 3 { 2 } else { 1 };
        let cols = if t.cols.len() == want {
            t.cols.clone()
        } else if n == 3 {
            vec![1. / 3., 2. / 3.]
        } else {
            vec![0.5]
        };
        (cols, if t.rows > 0. { t.rows } else { 0.56 })
    }

    fn pane_title(&self, spec: &PaneSpec) -> String {
        let a = self.snap.agents.get(spec.agent);
        match spec.kind.as_str() {
            "agent" => match a {
                Some(a) if a.id == MAIN => format!("Main agent · {}", self.snap.name),
                Some(a) => a.title.clone(),
                None => "Session".into(),
            },
            "files" => a
                .and_then(|a| a.ticket.as_ref())
                .map_or("Primary worktree".into(), |k| format!("{k} worktree")),
            "browser" => spec.url.as_deref().map_or("Browser".into(), |u| {
                u.trim_start_matches("http://")
                    .trim_start_matches("https://")
                    .to_string()
            }),
            "diagram" => "Diagram review".into(),
            "docs" => "PLAN.md".into(),
            _ => "New canvas".into(),
        }
    }

    fn pane(
        &mut self,
        slot: usize,
        w: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.pal;
        let spec = self.tab().panes[slot].clone();
        let on = slot == self.focus;
        let count = self.tab().panes.len();
        let (ic, ic_color) = match spec.kind.as_str() {
            "agent" => ("sparkle", p.accent),
            "files" => ("folder", p.fg3),
            "browser" => ("globe", p.fg3),
            "diagram" => ("diagram", p.fg3),
            "docs" => ("doc", p.fg3),
            _ => ("window", p.fg3),
        };
        let agent_id = spec.agent;
        let head = div()
            .h(px(30.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(7.))
            .pl(px(10.))
            .pr(px(6.))
            .text_size(px(12.))
            .hair_b(w, p.pane_edge)
            .child(icon(ic, 14., ic_color))
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .min_w_0()
                    .truncate()
                    .child(self.pane_title(&spec)),
            )
            .child(div().flex_1())
            .when(spec.kind == "agent", |d| {
                d.child(
                    ib(("pf", slot), "folder", &p, false)
                        .size(px(22.))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_pane(slot, PaneSpec::files(agent_id), window, cx)
                        })),
                )
            })
            .when(spec.kind != "empty", |d| {
                d.child(
                    ib(("ps", slot), "window", &p, false)
                        .size(px(22.))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_pane(slot, PaneSpec::default(), window, cx)
                        })),
                )
            })
            .when(count > 1, |d| {
                d.child(
                    ib(("pm", slot), "expand", &p, false)
                        .size(px(22.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.max = if this.max.is_none() { Some(slot) } else { None };
                            cx.notify()
                        })),
                )
            })
            .child(ib(("pc", slot), "close", &p, false).size(px(22.)).on_click(
                cx.listener(move |this, _, window, cx| this.close_pane(slot, window, cx)),
            ));
        let offline =
            !matches!(spec.kind.as_str(), "empty" | "browser") && self.offline_note().is_some();
        let body = if offline {
            div()
                .p(px(8.))
                .children(self.offline_note())
                .into_any_element()
        } else {
            match spec.kind.as_str() {
                "agent" => {
                    let a = self
                        .snap
                        .agents
                        .get(spec.agent)
                        .or(self.snap.agents.first())
                        .cloned();
                    match a {
                        Some(a) => self.agent_body(slot, &a, w).into_any_element(),
                        None => nothing(&p, "No such agent on this machine."),
                    }
                }
                "files" => self.files_body(slot, spec.agent, w, cx).into_any_element(),
                "browser" => self.browser_pane(slot, &spec, w, cx),
                "diagram" => self.diagram_pane(slot, w, window, cx),
                "docs" => self.docs_pane(w, cx),
                _ => self.chooser(slot, w, cx),
            }
        };
        let pb = self.panes[slot].bounds.clone();
        div()
            .id(("pane", slot))
            .size_full()
            .min_w_0()
            .min_h_0()
            .relative()
            .flex()
            .flex_col()
            .rounded(px(9.))
            .bg(p.pane)
            .hair_all(w, if on { p.pane_edge_on } else { p.pane_edge })
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if this.focus != slot {
                        this.focus = slot;
                        cx.notify();
                    }
                }),
            )
            .child(
                canvas(move |b, _, _| pb.set(b), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(head)
            .child(div().flex_1().min_h_0().flex().flex_col().child(body))
            .into_any_element()
    }

    // -------------------------------------------------------------- picker

    fn chooser(&mut self, slot: usize, w: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let narrow = f32::from(self.panes[slot].bounds.get().size.width) < 700.;
        let card_w = if narrow { 130. } else { 150. };
        let card = |id: &'static str, ic: &'static str, t: &'static str, s: &'static str| {
            div()
                .id((id, slot))
                .w(px(card_w + 24. + 2. * f32::from(w)))
                .p(px(12.))
                .rounded(px(9.))
                .bg(p.pane)
                .hair_all(w, p.pane_edge)
                .flex()
                .flex_col()
                .gap(px(6.))
                .cursor_pointer()
                .hover(|d| d.border_color(p.pane_edge_on))
                .child(icon(ic, 14., p.fg))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_size(px(12.5))
                        .child(t),
                )
                .child(div().text_size(px(11.5)).text_color(p.fg3).child(s))
        };
        let pick = |kind: &'static str| {
            cx.listener(
                move |this: &mut Self,
                      _: &ClickEvent,
                      window: &mut Window,
                      cx: &mut Context<Self>| {
                    this.set_pane(slot, PaneSpec::of(kind), window, cx)
                },
            )
        };
        let agents: Vec<(usize, String, AgentStatus)> = tree_order(&self.snap)
            .into_iter()
            .filter(|&id| id != MAIN)
            .map(|id| {
                let a = &self.snap.agents[id];
                (a.id, a.title.clone(), a.status)
            })
            .collect();
        let t = self.started.elapsed().as_secs_f32();
        let label = |s: &'static str| {
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.fg3)
                .child(s)
        };
        div()
            .id(("chooser", slot))
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .p(px(18.))
            .child(label("Show in this canvas"))
            .child(
                div()
                    .max_w(px(560.))
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(10.))
                    .child(
                        card(
                            "c-agent",
                            "sparkle",
                            "Main agent",
                            "Talk to the agent that runs the project",
                        )
                        .on_click(pick("agent")),
                    )
                    .child(
                        card("c-browser", "globe", "Browser", "A dev server preview")
                            .on_click(pick("browser")),
                    )
                    .child(
                        card(
                            "c-diagram",
                            "diagram",
                            "Diagram review",
                            "Plans and deliverables to approve",
                        )
                        .on_click(pick("diagram")),
                    )
                    .child(
                        card(
                            "c-files",
                            "folder",
                            "Worktree",
                            "Browse the project's files",
                        )
                        .on_click(pick("files")),
                    )
                    .child(
                        card("c-docs", "doc", "PLAN.md", "The plan the main agent wrote")
                            .on_click(pick("docs")),
                    ),
            )
            .when(!agents.is_empty(), |d| {
                d.child(label("Agent sessions")).child(
                    div()
                        .max_w(px(560.))
                        .flex()
                        .flex_wrap()
                        .justify_center()
                        .gap(px(6.))
                        .children(agents.into_iter().map(|(id, title, st)| {
                            div()
                                .id(("chip", id))
                                .h(px(26.))
                                .px(px(10.))
                                .rounded(px(7.))
                                .bg(p.pane)
                                .hair_all(w, p.pane_edge)
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .text_size(px(12.))
                                .cursor_pointer()
                                .hover(|d| d.border_color(p.pane_edge_on))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.set_pane(slot, PaneSpec::agent(id), window, cx)
                                }))
                                .child(icon("sparkle", 14., p.accent))
                                .child(title)
                                .child(status_dot(&p, st, t))
                        })),
                )
            })
            .into_any_element()
    }

    // -------------------------------------------------------------- browser

    fn browser_pane(
        &mut self,
        slot: usize,
        spec: &PaneSpec,
        w: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.pal;
        let cur = spec.url.clone();
        let viewport = self.panes[slot].viewport;
        let vps = [
            (Viewport::Desktop, "window"),
            (Viewport::Tablet, "doc"),
            (Viewport::Phone, "term"),
        ];
        let bar = div()
            .h(px(38.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .px(px(8.))
            .child(ib(("bback", slot), "back", &p, false))
            .child(ib(("bfwd", slot), "fwd", &p, false))
            .child(
                ib(("breload", slot), "reload", &p, false).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if let Some(u) = this.tab().panes[slot].url.clone() {
                            this.fetch(u, cx)
                        }
                    },
                )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .px(px(10.))
                    .rounded(px(7.))
                    .bg(p.hover)
                    .hair_all(w, p.pane_edge)
                    .text_size(px(12.))
                    .child(icon("globe", 14., p.fg2))
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.panes[slot].url)
                                .appearance(false)
                                .text_size(px(12.))
                                .h(px(24.))
                                .p_0(),
                        ),
                    )
                    .when(cur.is_some(), |d| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(5.))
                                .text_size(px(10.5))
                                .text_color(p.green)
                                .child(dot(p.green, None))
                                .child("live"),
                        )
                    }),
            )
            .child(div().flex().gap(px(2.)).children(vps.map(|(v, ic)| {
                ib((ic, slot), ic, &p, viewport == v).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.panes[slot].viewport = v;
                        cx.notify()
                    },
                ))
            })))
            .when_some(cur.clone(), |d, u| {
                d.child(
                    ib(("bext", slot), "ext", &p, false)
                        .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&u))),
                )
            });
        let page = match &cur {
            None => div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .p(px(24.))
                .text_color(p.page_dim)
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.page_fg)
                        .child("No preview yet"),
                )
                .child(div().max_w(px(520.)).text_center().child(
                    "Type a dev server address above, for example the URL an agent printed after npm run dev, and press Enter.",
                ))
                .into_any_element(),
            Some(u) => match self.pages.get(u) {
                None => div()
                    .p(px(24.))
                    .text_color(p.page_dim)
                    .child(format!("Loading {u}…"))
                    .into_any_element(),
                Some(Err(e)) => div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .p(px(24.))
                    .text_color(p.page_dim)
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.page_fg)
                            .child("Can't reach the page"),
                    )
                    .child(div().text_center().child(e.clone()))
                    .into_any_element(),
                Some(Ok(pg)) => {
                    let url = u.clone();
                    div()
                        .id(("page", slot))
                        .size_full()
                        .overflow_y_scroll()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .py(px(8.))
                                .px(px(12.))
                                .text_size(px(12.))
                                .text_color(p.page_dim)
                                .border_b_1()
                                .border_color(p.page_line)
                                .child(format!(
                                    "Text snapshot · {} · {} ms · GPUI has no embedded web engine",
                                    pg.status, pg.ms
                                ))
                                .child(div().flex_1())
                                .child(
                                    div()
                                        .id(("open-ext", slot))
                                        .flex()
                                        .items_center()
                                        .gap(px(4.))
                                        .text_color(p.page_accent)
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url)))
                                        .child("Open in browser")
                                        .child(icon("ext", 12., p.page_accent)),
                                ),
                        )
                        .child(
                            div()
                                .max_w(px(560.))
                                .mx_auto()
                                .py(px(48.))
                                .px(px(24.))
                                .text_size(px(15.))
                                .line_height(relative(1.5))
                                .text_color(p.page_fg)
                                .child(
                                    div()
                                        .text_size(px(30.))
                                        .line_height(relative(1.2))
                                        .font_weight(FontWeight::BOLD)
                                        .mb(px(14.))
                                        .child(pg.title.clone()),
                                )
                                .children(pg.text.iter().map(|l| div().mb(px(6.)).child(l.clone()))),
                        )
                        .into_any_element()
                }
            },
        };
        let frame = div().h_full().bg(p.page_bg).child(page);
        let frame = match viewport {
            Viewport::Desktop => frame.w_full(),
            Viewport::Tablet => frame.w(px(768.)),
            Viewport::Phone => frame.w(px(390.)),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(bar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .justify_center()
                    .bg(p.page_desk)
                    .child(frame),
            )
            .into_any_element()
    }
}
