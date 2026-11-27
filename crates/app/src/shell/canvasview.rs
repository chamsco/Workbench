//! A stream's Canvas (Charles, 24 Nov): the work laid out as it happens instead of a transcript.
//! The ask is the start node, what it took in hangs off its left, its questions and your answers
//! sit under it, helpers branch out with a live preview each, then what came back, the plan, and
//! the Tasks tray on the right. The chat box floats over the bottom, rounded and see-through
//! ("liquid silver"), with a status pill at the top centre saying what is going on now. Laid out
//! again from the thread on each render (canvasmodel.rs); only the camera is kept.

use std::time::Instant;

use super::canvasmodel::{self, Card, Flow, Kind, Live};
use super::*;
use backspace_core::chat::Thread;
use gpui_kit::prelude::FluentBuilder;

/// How long Canvas and Board take to hand over to each other (stretched in debug by
/// CAULDRON_MOTION_SLOW, for screenshots of the middle).
pub(crate) fn swap_ms() -> u64 {
    (420. * super::motion::slow()) as u64
}

/// The silver of the floating chat box and the pill: light glass over the canvas.
fn silver(pal: &'static Palette) -> (Hsla, Hsla, Hsla) {
    let dark = pal.panel.0 < 0x80;
    if dark {
        (hsla(220. / 360., 0.10, 0.70, 0.30), hsla(220. / 360., 0.10, 0.42, 0.34), hsla(0., 0., 1., 0.20))
    } else {
        (hsla(220. / 360., 0.20, 0.99, 0.80), hsla(220. / 360., 0.16, 0.88, 0.66), hsla(0., 0., 1., 0.90))
    }
}

impl Cauldron {
    /// The open stream's canvas, laid out from the thread and its side streams.
    pub(super) fn canvas_flow(&self, t: &Thread) -> Flow {
        let Some((_, chats)) = self.core.as_ref() else { return Flow::default() };
        let sides: Vec<Thread> = chats.list().into_iter().filter(|x| x.side_of.as_deref() == Some(t.id.as_str())).filter_map(|x| chats.peek(&x.id)).collect();
        let files = |th: &str, id: &str| chats.attachment_path(th, id).ok().filter(|p| p.exists()).map(|p| p.to_string_lossy().to_string());
        canvasmodel::build(t, &sides, chats.is_busy(&t.id), &files)
    }

    /// Whether the canvas shows the lineage of what was made (coding) or the reasoning: the
    /// stream's choice, else lineage once it has shipped a commit.
    fn lineage_on(&self) -> bool {
        self.board.lineage.unwrap_or_else(|| self.board.b.nodes.iter().any(|n| n.via.is_none() && n.refs.iter().any(|r| r.kind == "Commit")))
    }

    /// What the canvas draws: the reasoning flow, or the lineage with the same pill and tasks.
    fn canvas_view(&self, t: &Thread) -> Flow {
        let flow = self.canvas_flow(t);
        if !self.lineage_on() {
            return flow;
        }
        let mut l = canvasmodel::lineage(&self.board.b);
        l.pill = flow.pill;
        l.tasks = flow.tasks;
        l
    }

    /// Fits the canvas once it has a size, and again when new cards arrive (newest in view).
    #[inline(never)]
    pub(super) fn canvas_prepare(&mut self) {
        if !self.board.canvas || self.open.is_none() {
            return;
        }
        let Some(t) = self.thread.clone() else { return };
        let b = self.board.cbounds.get();
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        if w < 10. {
            return;
        }
        let f = self.canvas_view(&t);
        let key = f.cards.len() * 2 + usize::from(self.lineage_on());
        if self.board.cfit == Some(key) {
            return;
        }
        self.board.cfit = Some(key);
        let Some((x0, y0, x1, y1)) = f.extent() else {
            self.board.ccam = (w / 2. - canvasmodel::START_W / 2., h * 0.32, 1.0);
            return;
        };
        // Room for the pill above and the floating box below. Readable first: when the newest
        // ask's work is too wide to show whole at 80%, its newest end stays in view, as the work
        // moves right (start, helpers, results, plan).
        let (top, bottom, side) = (80., 140., 70.);
        let k = ((w - 2. * side) / (x1 - x0)).min((h - top - bottom) / (y1 - y0)).clamp(0.8, 1.0);
        let cx_ = if (x1 - x0) * k <= w - 2. * side { (w - (x1 - x0) * k) / 2. - x0 * k } else { w - side - x1 * k };
        let cy = if (y1 - y0) * k <= h - top - bottom { top + (h - top - bottom - (y1 - y0) * k) / 2. - y0 * k } else { top - y0 * k };
        self.board.ccam = (cx_, cy, k);
    }

    fn canvas_down(&mut self, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        let (sx, sy) = (f32::from(ev.position.x), f32::from(ev.position.y));
        self.board.cpan = Some((sx, sy, self.board.ccam.0, self.board.ccam.1));
        self.board.tray = false;
        cx.notify();
    }

    fn canvas_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some((x0, y0, ox, oy)) = self.board.cpan else { return };
        if ev.pressed_button != Some(MouseButton::Left) {
            self.board.cpan = None;
            return;
        }
        let (sx, sy) = (f32::from(ev.position.x), f32::from(ev.position.y));
        self.board.ccam = (ox + sx - x0, oy + sy - y0, self.board.ccam.2);
        cx.notify();
    }

    fn canvas_wheel(&mut self, ev: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let b = self.board.cbounds.get();
        let (sx, sy) = (f32::from(ev.position.x - b.origin.x), f32::from(ev.position.y - b.origin.y));
        let (dx, dy) = match ev.delta {
            ScrollDelta::Lines(p) => (p.x * 40., p.y * 40.),
            ScrollDelta::Pixels(p) => (f32::from(p.x), f32::from(p.y)),
        };
        let cam = &mut self.board.ccam;
        if ev.modifiers.control || ev.modifiers.platform {
            let k = (cam.2 * if dy > 0. { 1.1 } else { 1. / 1.1 }).clamp(0.3, 1.6);
            cam.0 = sx - (sx - cam.0) * k / cam.2;
            cam.1 = sy - (sy - cam.1) * k / cam.2;
            cam.2 = k;
        } else {
            cam.0 += dx;
            cam.1 += dy;
        }
        cx.notify();
    }

    /// The canvas. `zoom` scales it about its centre and `alpha` fades it, for the hand-over to
    /// and from the Board; both are 1 at rest.
    #[inline(never)]
    pub(super) fn canvas_el(&self, t: &Thread, zoom: f32, alpha: f32, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let f = self.canvas_view(t);
        let b = self.board.cbounds.get();
        let (bw, bh) = (f32::from(b.size.width), f32::from(b.size.height));
        // The camera, with the hand-over's zoom about the middle of the view.
        let (mut ox, mut oy, mut k) = self.board.ccam;
        if (zoom - 1.).abs() > 0.001 {
            let (mx, my) = (bw / 2., bh / 2.);
            ox = mx - (mx - ox) * zoom;
            oy = my - (my - oy) * zoom;
            k *= zoom;
        }
        let at = move |x: f32, y: f32| (x * k + ox, y * k + oy);
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let now = super::ledger::now_ms();

        // Lines between the cards, under them: an input bends into the start's left side; the rest
        // leave from a card's right side and land on the next one's left, as in a flow.
        let mut segs: Vec<((f32, f32), (f32, f32), bool, bool)> = Vec::new();
        for &(a, bi, side) in &f.edges {
            let (ca_, cb) = (&f.cards[a], &f.cards[bi]);
            let p1 = (ca_.x + ca_.w, ca_.y + if side { ca_.h / 2. } else { (ca_.h / 2.).min(40.) });
            let p2 = (cb.x, cb.y + if cb.kind == Kind::Start || side { (cb.h / 2.).min(36.) } else { (cb.h / 2.).min(40.) });
            segs.push((at(p1.0, p1.1), at(p2.0, p2.1), side, cb.live == Live::Working));
        }
        let (line, live_line, dot) = (ca_fg(pal, 0.22), ca_fg(pal, 0.45), ca_fg(pal, 0.10));
        let bounds_out = self.board.cbounds.clone();
        let paint = canvas(
            move |b, _, _| {
                bounds_out.set(b);
                b
            },
            move |b, _, window, _| {
                let o = b.origin;
                let pt = |x: f32, y: f32| point(o.x + px(x), o.y + px(y));
                let step = 22. * k;
                if step >= 7. {
                    let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
                    let r = (1.0 * k).max(0.7);
                    let mut y = oy.rem_euclid(step);
                    while y < h {
                        let mut x = ox.rem_euclid(step);
                        while x < w {
                            window.paint_quad(fill(Bounds::new(pt(x - r, y - r), size(px(2. * r), px(2. * r))), dot).corner_radii(px(r)));
                            x += step;
                        }
                        y += step;
                    }
                }
                for &((x1, y1), (x2, y2), _side, live) in &segs {
                    let mut pb = PathBuilder::stroke(px((1.3 * k).max(1.)));
                    if live {
                        pb = pb.dash_array(&[px(5. * k), px(4. * k)]);
                    }
                    pb.move_to(pt(x1, y1));
                    let m = ((x2 - x1) / 2.).max(18. * k);
                    pb.cubic_bezier_to(pt(x2, y2), pt(x1 + m, y1), pt(x2 - m, y2));
                    if let Ok(p) = pb.build() {
                        window.paint_path(p, if live { live_line } else { line });
                    }
                }
            },
        )
        .absolute()
        .inset_0()
        .size_full();

        let mut world = div().absolute().inset_0().overflow_hidden().child(paint);
        for (i, card) in f.cards.iter().enumerate() {
            let (sx, sy) = at(card.x, card.y);
            // Off screen: skip it.
            if bw > 10. && (sx > bw + 40. || sy > bh + 40. || sx + card.w * k < -40. || sy + card.h * k < -40.) {
                continue;
            }
            world = world.child(self.canvas_card(i, card, sx, sy, k, now, pal, cx));
        }
        if f.cards.is_empty() {
            world = world.child(
                div()
                    .id("canvas-empty")
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .pb(px(80.))
                    .aria_label("What should we get done?")
                    .child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("What should we get done?"))
                    .child(div().max_w(px(420.)).text_center().text_size(px(13.)).text_color(fg3).child("Ask below. The ask becomes the first node, and the work branches out from it as it happens."))
                    .test_support(),
            );
        }

        let pill = self.canvas_pill(&f, now, pal, cx);
        let tasks = self.canvas_tasks(&f, pal, cx);
        let mode = self.canvas_mode(pal, cx);
        let zoom_label = div().id("canvas-zoom").absolute().top(px(14.)).right(px(18.)).text_size(px(12.)).font_features(tnum()).text_color(fg3).child(format!("{}%", (k * 100.).round())).test_support();

        // The chat box, floating: a gap under it, rounded, see-through silver.
        let (bw_, _) = self.thread_box.get();
        let narrow = bw_ > 0.0 && bw_ < 360.0;
        let (top, bot, edge) = silver(pal);
        let composer = self
            .composer_box(t, narrow, false, pal, cx)
            .rounded(px(22.))
            .border_color(edge)
            .bg(linear_gradient(180., linear_color_stop(top, 0.), linear_color_stop(bot, 1.)))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.28), offset: point(px(0.), px(10.)), blur_radius: px(30.), spread_radius: px(-6.), inset: false }]);
        let float = div().id("canvas-composer").absolute().left_0().right_0().bottom(px(18.)).flex().justify_center().px(px(16.)).child(div().w_full().max_w(px(680.)).child(composer.test_support())).test_support();
        let _ = fg2;

        div()
            .id("canvas")
            .relative()
            .size_full()
            .overflow_hidden()
            .opacity(alpha)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _, cx| this.canvas_down(ev, cx)))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| this.canvas_move(ev, cx)))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.board.cpan = None;
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| this.canvas_wheel(ev, cx)))
            .child(world)
            .child(pill)
            .child(zoom_label)
            .child(mode)
            .child(tasks)
            .child(float)
            .test_support()
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn canvas_card(&self, i: usize, card: &Card, sx: f32, sy: f32, k: f32, now: u64, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let _ = i;
        let base = div().id(ElementId::Name(card.id.clone().into())).absolute().left(px(sx)).top(px(sy)).w(px(card.w * k)).aria_label(card.title.clone());
        let el = match card.kind {
            // The ask: large, no box, "You · time" over it.
            Kind::Start => base
                .flex()
                .flex_col()
                .gap(px(4. * k))
                .child(div().text_size(px(11.5 * k)).text_color(fg3).child(format!("You · {}", super::boardview::clock(card.at))))
                .child(div().text_size(px(19. * k)).line_height(px(26. * k)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(card.title.clone())),
            // What it took in: name and kind, right-aligned, and its icon in a small square.
            Kind::Input => {
                let icon = match card.sub.as_str() {
                    "Memory" => "brain",
                    "Skill" => "book-open",
                    "File" => "file",
                    _ => "link",
                };
                base.flex()
                    .items_center()
                    .justify_end()
                    .gap(px(8. * k))
                    .h(px(card.h * k))
                    .child(div().flex().flex_col().items_end().child(div().text_size(px(12.5 * k)).font_weight(FontWeight::SEMIBOLD).text_color(fg).truncate().child(card.title.clone())).child(div().text_size(px(10.5 * k)).text_color(fg3).child(card.sub.clone())))
                    .child(div().size(px(24. * k)).flex_none().flex().items_center().justify_center().rounded(px(7. * k)).bg(c(pal.raised)).border_1().border_color(c(pal.line)).child(glyph(icon, 13. * k, fg2)))
            }
            // A question, inline under the ask, with your answer as a chip.
            Kind::Ask => base
                .flex()
                .gap(px(10. * k))
                .child(div().mt(px(1. * k)).size(px(20. * k)).flex_none().flex().items_center().justify_center().rounded_full().bg(c(pal.accent_soft)).child(glyph("bot", 12. * k, c(pal.accent))))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8. * k))
                        .min_w_0()
                        .child(div().text_size(px(13.5 * k)).line_height(px(20. * k)).text_color(if card.answer.is_some() { fg2 } else { fg }).child(card.title.clone()))
                        .when_some(card.answer.clone(), |d, a| d.child(div().flex().child(div().px(px(10. * k)).py(px(5. * k)).rounded(px(8. * k)).bg(c(pal.raised)).border_1().border_color(c(pal.line_strong)).text_size(px(12.5 * k)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(a)))),
                ),
            Kind::Version => {
                let mut parts = card.sub.split('\u{1f}');
                let (ver, when, commit) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                let mut b = base
                    .flex()
                    .flex_col()
                    .rounded(px(12. * k))
                    .bg(c(pal.raised))
                    .border_1()
                    .border_color(if card.latest { c(pal.accent) } else { c(pal.line_strong) })
                    .shadow(super::lift(super::Lift::Button, pal))
                    .overflow_hidden()
                    .cursor_pointer()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8. * k))
                            .px(px(12. * k))
                            .pt(px(10. * k))
                            .pb(px(2. * k))
                            .child(div().px(px(6. * k)).rounded(px(5. * k)).bg(if card.latest { c(pal.accent_soft) } else { c(pal.hover) }).text_size(px(11. * k)).font_weight(FontWeight::SEMIBOLD).text_color(if card.latest { c(pal.accent) } else { fg2 }).child(ver.to_string()))
                            .child(div().flex_1().min_w_0().truncate().text_size(px(13. * k)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(card.title.clone()))
                            .child(div().flex_none().text_size(px(10.5 * k)).text_color(fg3).child(when.to_string())),
                    );
                if let Some(p) = &card.image {
                    b = b.child(div().mx(px(12. * k)).mt(px(8. * k)).h(px(120. * k)).rounded(px(8. * k)).overflow_hidden().border_1().border_color(c(pal.line)).child(img(std::path::PathBuf::from(p)).size_full().object_fit(ObjectFit::Cover)));
                }
                let mut ul = div().flex().flex_col().gap(px(3. * k)).px(px(12. * k)).pt(px(8. * k));
                for l in &card.lines {
                    ul = ul.child(div().text_size(px(12. * k)).line_height(px(16. * k)).text_color(fg2).truncate().child(l.clone()));
                }
                let mut chips = div().flex().flex_wrap().gap(px(5. * k)).px(px(12. * k)).pt(px(8. * k)).pb(px(10. * k));
                let chip = |t: String, col: Hsla, soft: bool| div().px(px(6. * k)).py(px(1. * k)).rounded(px(5. * k)).border_1().border_color(if soft { col.opacity(0.0) } else { c(pal.line) }).when(soft, |d| d.bg(col.opacity(0.13))).text_size(px(10.5 * k)).text_color(col).child(t);
                if !commit.is_empty() {
                    chips = chips.child(chip(commit.to_string(), fg3, false));
                }
                if let Some(v) = &card.answer {
                    chips = chips.child(chip(v.clone(), c(pal.warn), true));
                }
                if card.latest {
                    chips = chips.child(chip("Latest".into(), c(pal.accent), true));
                }
                b = b.child(ul).child(chips);
                b
            }
            Kind::Helper | Kind::Result | Kind::Plan => {
                let (label_icon, state_col) = match card.live {
                    Live::Working => ("loader", c(pal.accent)),
                    Live::Done => ("check", c(pal.ok)),
                    Live::Failed => ("circle-alert", c(pal.bad)),
                };
                let mut b = base
                    .flex()
                    .flex_col()
                    .rounded(px(12. * k))
                    .bg(c(pal.raised))
                    .border_1()
                    .border_color(if card.live == Live::Working { c(pal.accent).opacity(0.5) } else { c(pal.line_strong) })
                    .shadow(super::lift(super::Lift::Button, pal))
                    .overflow_hidden();
                let head_icon = match card.kind {
                    Kind::Helper if card.sub.starts_with("Reading") => "globe",
                    Kind::Helper if card.sub.starts_with("Searching") => "search",
                    Kind::Helper => "git-fork",
                    Kind::Result => "sparkles",
                    _ => "list-checks",
                };
                let head_label = match card.kind {
                    Kind::Plan => "Plan".to_string(),
                    _ => card.title.clone(),
                };
                b = b.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8. * k))
                        .px(px(12. * k))
                        .pt(px(10. * k))
                        .pb(px(4. * k))
                        .child(glyph(head_icon, 13. * k, fg2))
                        .child(div().flex_1().min_w_0().truncate().text_size(px(13. * k)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(head_label))
                        .when(card.kind != Kind::Plan, |d| {
                            d.child(div().flex().items_center().gap(px(4. * k)).text_size(px(10.5 * k)).text_color(state_col).child(glyph(label_icon, 11. * k, state_col)).child(match card.live {
                                Live::Working => "Working",
                                Live::Done => "Done",
                                Live::Failed => "Stopped",
                            }))
                        })
                        .when(card.kind == Kind::Plan, |d| d.child(div().text_size(px(10.5 * k)).text_color(fg3).child(card.sub.clone()))),
                );
                if card.kind == Kind::Plan {
                    b = b.child(div().px(px(12. * k)).pt(px(2. * k)).text_size(px(16. * k)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(card.title.clone()));
                    let mut steps = div().flex().flex_col().gap(px(6. * k)).px(px(12. * k)).pt(px(10. * k)).pb(px(12. * k));
                    for l in &card.lines {
                        let (state, text) = l.split_once('\u{1f}').unwrap_or(("todo", l));
                        let (ic, col) = match state {
                            "done" => ("circle-check", c(pal.ok)),
                            "doing" => ("loader", c(pal.accent)),
                            _ => ("circle", fg3),
                        };
                        steps = steps.child(div().flex().items_center().gap(px(8. * k)).child(glyph(ic, 13. * k, col)).child(div().min_w_0().truncate().text_size(px(12.5 * k)).text_color(if state == "done" { fg3 } else { fg }).child(text.to_string())));
                    }
                    b = b.child(steps);
                } else {
                    if !card.sub.is_empty() && card.kind == Kind::Helper && card.live != Live::Done {
                        let sub = if card.live == Live::Working { super::ledger::shimmer(&card.sub, now, fg3, fg, false).into_any_element() } else { div().child(card.sub.clone()).into_any_element() };
                        b = b.child(div().px(px(12. * k)).text_size(px(11.5 * k)).text_color(fg3).child(sub));
                    }
                    if let Some(p) = &card.image {
                        b = b.child(div().mx(px(12. * k)).mt(px(8. * k)).h(px((if card.kind == Kind::Result { 180. } else { 140. }) * k)).rounded(px(8. * k)).overflow_hidden().border_1().border_color(c(pal.line)).child(img(std::path::PathBuf::from(p)).size_full().object_fit(ObjectFit::Cover)));
                    }
                    let mut ul = div().flex().flex_col().gap(px(3. * k)).px(px(12. * k)).pt(px(6. * k)).pb(px(12. * k));
                    for l in &card.lines {
                        ul = ul.child(div().text_size(px(12. * k)).line_height(px(16. * k)).text_color(fg2).truncate().child(l.clone()));
                    }
                    if card.kind == Kind::Result && !card.sub.is_empty() {
                        ul = ul.child(div().pt(px(4. * k)).flex().items_center().gap(px(5. * k)).text_size(px(11. * k)).text_color(fg3).child(glyph("file", 11. * k, fg3)).child(div().truncate().child(card.sub.clone())));
                    }
                    b = b.child(ul);
                }
                b
            }
        };
        let el = if let Some(nid) = card.node.clone() {
            el.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(cx.listener(move |this, _, _, cx| {
                this.board.swap = Some((Instant::now(), true));
                this.board_show_node(&nid, cx);
            }))
        } else {
            el
        };
        let el = match card.open.clone() {
            Some(sid) => el.cursor_pointer().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(cx.listener(move |this, _, _, cx| {
                this.selected = Some(sid.clone());
                this.open_thread(sid.clone());
                cx.notify();
            })),
            None => el.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        };
        el.test_support().into_any_element()
    }

    /// Reasoning | Lineage, top left: how the canvas reads the stream.
    fn canvas_mode(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let on = self.lineage_on();
        let seg = |id: &'static str, icon: &'static str, label: &'static str, sel: bool| {
            div()
                .id(id)
                .h(px(26.))
                .px(px(9.))
                .flex()
                .items_center()
                .gap(px(5.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if sel { fg } else { fg3 })
                .when(sel, |d| d.button_face(pal).pressed().shadow(super::lift(super::Lift::Button, pal)))
                .when(!sel, |d| d.hover(|d| d.bg(c(pal.hover)).text_color(fg)))
                .aria_label(label)
                .aria_selected(sel)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(glyph(icon, 12., if sel { c(pal.fg2) } else { fg3 }))
                .child(label)
        };
        div()
            .id("canvas-mode")
            .absolute()
            .top(px(12.))
            .left(px(16.))
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(radius::CONTROL + 2.))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .child(seg("canvas-reasoning", "brain", "Reasoning", !on).on_click(cx.listener(|this, _, _, cx| {
                this.board.lineage = Some(false);
                cx.notify();
            })).test_support())
            .child(seg("canvas-lineage", "git-fork", "Lineage", on).on_click(cx.listener(|this, _, _, cx| {
                this.board.lineage = Some(true);
                cx.notify();
            })).test_support())
            .test_support()
            .into_any_element()
    }

    /// The status pill at the top centre: an orb, what is going on, Stop while it works and Next
    /// once the plan is ready.
    fn canvas_pill(&self, f: &Flow, now: u64, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg2));
        let (top, bot, edge) = silver(pal);
        let p = &f.pill;
        let text = if p.live { super::ledger::shimmer(&p.text, now, fg3, fg, false).into_any_element() } else { div().child(p.text.clone()).into_any_element() };
        // The orb: a ring that turns while it works.
        let turn = (now % 1400) as f32 / 1400.;
        let orb = div().size(px(18.)).flex_none().flex().items_center().justify_center().rounded_full().bg(c(pal.accent_soft)).child(glyph(if p.live { "loader" } else { "bot" }, 11., c(pal.accent)).with_transformation(Transformation::rotate(radians(if p.live { turn * std::f32::consts::TAU } else { 0. }))));
        let mut pill = div()
            .id("canvas-pill")
            .flex()
            .items_center()
            .gap(px(9.))
            .h(px(34.))
            .pl(px(9.))
            .pr(px(if p.live || p.next { 5. } else { 14. }))
            .rounded_full()
            .border_1()
            .border_color(edge)
            .bg(linear_gradient(180., linear_color_stop(top, 0.), linear_color_stop(bot, 1.)))
            .shadow(super::lift(super::Lift::Button, pal))
            .text_size(px(12.5))
            .text_color(fg)
            .aria_label(p.text.clone())
            .child(orb)
            .child(text);
        if p.live {
            pill = pill.child(
                div()
                    .id("canvas-stop")
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .cursor_pointer()
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label("Stop")
                    .child(glyph("square", 11., fg3))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let (Some(id), Some((_, chats))) = (this.open.clone(), this.core.as_ref()) {
                            chats.stop(&id);
                        }
                        this.refresh();
                        cx.notify();
                    }))
                    .test_support(),
            );
        } else if p.next {
            pill = pill.child(
                div()
                    .id("canvas-next")
                    .h(px(24.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .rounded_full()
                    .cursor_pointer()
                    .bg(c(pal.hover))
                    .font_weight(FontWeight::MEDIUM)
                    .aria_label("Next: open the tasks")
                    .child("Next")
                    .child(glyph("chevron-up", 12., fg3))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.board.tray = !this.board.tray;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        div().absolute().top(px(12.)).left_0().right_0().flex().justify_center().child(pill.test_support()).into_any_element()
    }

    /// The Tasks counter on the right edge, and the tray of the plan's steps it opens.
    fn canvas_tasks(&self, f: &Flow, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let open = f.tasks.iter().filter(|s| s.state != "done").count();
        let counter = div()
            .id("canvas-tasks")
            .w(px(42.))
            .py(px(8.))
            .flex()
            .flex_col()
            .items_center()
            .rounded(px(12.))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line_strong))
            .shadow(super::lift(super::Lift::Button, pal))
            .cursor_pointer()
            .aria_label(format!("{open} tasks"))
            .aria_expanded(self.board.tray)
            .when(open > 0, |d| d.child(div().w(px(16.)).h(px(2.)).mb(px(5.)).rounded_full().bg(c(pal.warn))))
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).text_color(fg).child(open.to_string()))
            .child(div().text_size(px(9.5)).text_color(fg3).child("Tasks"))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, _, cx| {
                this.board.tray = !this.board.tray;
                cx.notify();
            }))
            .test_support();
        let mut col = div().absolute().right(px(14.)).top_0().bottom_0().flex().items_center().gap(px(12.)).child(counter);
        if self.board.tray {
            let title = self.thread.as_ref().map_or_else(String::new, |t| t.title.clone());
            let mut tray = div()
                .id("canvas-tray")
                .w(px(260.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .p(px(14.))
                .rounded(px(14.))
                .bg(c(pal.raised))
                .border_1()
                .border_color(c(pal.line_strong))
                .shadow(super::lift(super::Lift::Button, pal))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
            if f.tasks.is_empty() {
                tray = tray.child(div().text_size(px(12.5)).text_color(fg3).child("No tasks yet. When the stream writes a plan, its steps land here."));
            }
            for s in &f.tasks {
                let (ic, col_) = match s.state.as_str() {
                    "done" => ("circle-check", c(pal.ok)),
                    "doing" => ("loader", c(pal.accent)),
                    _ => ("circle", fg3),
                };
                tray = tray.child(
                    div()
                        .flex()
                        .gap(px(9.))
                        .child(div().pt(px(2.)).child(glyph(ic, 14., col_)))
                        .child(div().flex().flex_col().min_w_0().child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(if s.state == "done" { fg3 } else { fg }).child(s.text.clone())).child(div().text_size(px(11.)).text_color(fg3).truncate().child(title.clone()))),
                );
            }
            col = col.child(deferred(tray.test_support().with_animation("tray-rise", super::motion::anim(180.), |d, t| d.opacity(t))).with_priority(2));
            // The tray sits left of the counter.
            col = col.flex_row_reverse();
        }
        col.into_any_element()
    }

    /// Chat, Canvas or Board for the open coordinator (0, 1, 2). Canvas and Board hand over with a
    /// zoom (Charles, 24 Nov); the stream remembers whether it shows its chat or its canvas.
    pub(super) fn set_stream_view(&mut self, v: u8, cx: &mut Context<Self>) {
        let root = self.board.thread.clone();
        if !root.is_empty() && self.open.as_deref() != Some(root.as_str()) {
            self.selected = Some(root.clone());
            self.open_thread(root.clone());
        }
        let was = if self.board.view { 2 } else if self.board.canvas { 1 } else { 0 };
        if was == v {
            return;
        }
        if (was == 1 && v == 2) || (was == 2 && v == 1) {
            self.board.swap = Some((Instant::now(), v == 2));
        }
        self.board.view = v == 2;
        if v == 2 {
            self.board.fitted = false;
        }
        if v != 2 {
            self.board.canvas = v == 1;
            self.board.cfit = None;
            if let (Some(id), Some((_, chats))) = (self.open.clone(), self.core.as_ref()) {
                let _ = chats.set_view(&id, (v == 1).then_some("canvas"));
            }
        } else {
            // Board keeps where it came from, so Board → back lands on the same one.
            self.board.canvas = was == 1;
        }
        cx.notify();
    }
}

fn ca_fg(pal: &'static Palette, a: f32) -> Hsla {
    let mut h = c(pal.fg);
    h.a = a;
    h
}
