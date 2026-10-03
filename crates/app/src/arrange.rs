//! Canvas arrangement: the active tab's layout tree drawn at absolute
//! rectangles, gutters, and dragging canvases and tabs around. A drag
//! previews its drop live: the other canvases glide to where they would end
//! up around a dotted "+" slot.

use std::time::Instant;

use backspace_core::layout::{self, Dir, Node, Rect, Side, PH};
use backspace_core::prefs::PaneSpec;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::ui::*;
use crate::*;

const GAP: f32 = 6.;
const SLIDE: f32 = 0.18;
/// How far the pointer travels before a press becomes a drag.
const SLOP: f32 = 5.;

pub(crate) struct PaneDrag {
    pub(crate) slot: usize,
    start: Point<Pixels>,
    pub(crate) pos: Point<Pixels>,
    pub(crate) active: bool,
    /// The arrangement when the drag began. Hit-testing uses it, so the
    /// canvas under the pointer doesn't move away as the preview reflows.
    base: Node,
    leaves: Vec<(usize, Rect)>,
    grab: (f32, f32),
    size: (f32, f32),
    target: Option<(usize, Side)>,
    pub(crate) tab_to: Option<usize>,
}

pub(crate) struct TabDrag {
    from: usize,
    start: Point<Pixels>,
    pub(crate) x: f32,
    pub(crate) active: bool,
    widths: Vec<f32>,
    mids: Vec<f32>,
    x0: f32,
    grab: f32,
    pub(crate) idx: usize,
}

/// Where each animated thing is gliding from and to, and since when.
#[derive(Default)]
pub(crate) struct Anim {
    rects: HashMap<usize, (Rect, Rect, Instant)>,
    pub(crate) xs: HashMap<usize, (f32, f32, Instant)>,
    key: Option<(usize, i32, i32, Option<usize>)>,
}

fn ease(k: f32) -> f32 {
    1. - (1. - k).powi(3)
}

fn lerp_rect(a: Rect, b: Rect, k: f32) -> Rect {
    let l = |x: f32, y: f32| x + (y - x) * k;
    Rect {
        x: l(a.x, b.x),
        y: l(a.y, b.y),
        w: l(a.w, b.w),
        h: l(a.h, b.h),
    }
}

impl Anim {
    /// The rect to draw `key` at now, gliding towards `to`.
    fn rect(&mut self, key: usize, to: Rect, snap: bool, now: Instant, moving: &mut bool) -> Rect {
        let k = |t0: Instant| ease(((now - t0).as_secs_f32() / SLIDE).min(1.));
        match self.rects.get(&key).copied() {
            _ if snap => {
                self.rects.insert(key, (to, to, now));
                to
            }
            Some((from, target, t0)) if target == to => {
                let k = k(t0);
                if k < 1. {
                    *moving = true;
                }
                lerp_rect(from, to, k)
            }
            Some((from, target, t0)) => {
                let cur = lerp_rect(from, target, k(t0));
                self.rects.insert(key, (cur, to, now));
                *moving = true;
                cur
            }
            None => {
                self.rects.insert(key, (to, to, now));
                to
            }
        }
    }

    fn x(&mut self, key: usize, to: f32, now: Instant, moving: &mut bool) -> f32 {
        let k = |t0: Instant| ease(((now - t0).as_secs_f32() / 0.16).min(1.));
        match self.xs.get(&key).copied() {
            Some((from, target, t0)) if target == to => {
                let k = k(t0);
                if k < 1. {
                    *moving = true;
                }
                from + (to - from) * k
            }
            Some((from, target, t0)) => {
                let cur = from + (target - from) * k(t0);
                self.xs.insert(key, (cur, to, now));
                *moving = true;
                cur
            }
            None => {
                self.xs.insert(key, (to, to, now));
                to
            }
        }
    }
}

fn hit(b: Bounds<Pixels>, p: Point<Pixels>) -> bool {
    p.x >= b.origin.x
        && p.x <= b.origin.x + b.size.width
        && p.y >= b.origin.y
        && p.y <= b.origin.y + b.size.height
}

/// A tab's arrangement drawn small, for unnamed tabs and layout pickers.
pub(crate) fn tree_icon(t: &Node, w: f32, h: f32, color: Hsla) -> Div {
    let (leaves, _) = t.layout(
        Rect {
            x: 1.,
            y: 1.,
            w: w - 2.,
            h: h - 2.,
        },
        1.6,
    );
    div()
        .relative()
        .flex_none()
        .w(px(w))
        .h(px(h))
        .children(leaves.into_iter().map(|(_, r)| {
            div()
                .absolute()
                .left(px(r.x))
                .top(px(r.y))
                .w(px(r.w))
                .h(px(r.h))
                .rounded(px(1.1))
                .bg(color)
        }))
}

impl Workbench {
    fn grid_rect(&self) -> Rect {
        let s = self.grid_bounds.get().size;
        Rect {
            x: 0.,
            y: 0.,
            w: f32::from(s.width),
            h: f32::from(s.height),
        }
    }

    /// The tree to draw: the tab's own, or what a drag would make of it.
    fn shown_tree(&self) -> Node {
        match &self.pdrag {
            Some(d) if d.active => {
                if d.tab_to.is_some() {
                    d.base
                        .remove(d.slot)
                        .unwrap_or_else(|| d.base.rename(d.slot, PH))
                } else {
                    let (t, side) = d.target.unwrap_or((d.slot, Side::Center));
                    d.base.moved(d.slot, t, side, PH)
                }
            }
            _ => self.tab().tree(),
        }
    }

    /// The active tab's canvases at their layout rectangles.
    pub(crate) fn grid(
        &mut self,
        w: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.pal;
        let count = self.tab().panes.len();
        if self.focus >= count {
            self.focus = 0;
        }
        if self.max.is_some_and(|m| m >= count) {
            self.max = None;
        }
        let full = self.grid_rect();
        let (leaves, gutters) = match self.max {
            Some(m) => (vec![(m, full)], vec![]),
            None => self.shown_tree().layout(full, GAP),
        };
        // Glide only when the arrangement changes, not when the window resizes
        // or the tab switches.
        let key = (
            self.prefs.active_tab,
            full.w as i32,
            full.h as i32,
            self.max,
        );
        let snap = self.anim.key != Some(key) || matches!(self.drag, Some(Drag::Gut(_)));
        self.anim.key = Some(key);
        let dragging = self.pdrag.as_ref().is_some_and(|d| d.active);
        if !dragging {
            self.anim.rects.remove(&PH);
        }
        let now = Instant::now();
        let mut moving = false;
        let mut kids: Vec<AnyElement> = Vec::new();
        for (leaf, to) in leaves {
            let r = self.anim.rect(leaf, to, snap, now, &mut moving);
            let at = div()
                .absolute()
                .left(px(r.x))
                .top(px(r.y))
                .w(px(r.w))
                .h(px(r.h))
                .flex();
            if leaf == PH {
                kids.push(
                    at.rounded(px(9.))
                        .border(px(1.5))
                        .border_dashed()
                        .border_color(p.fg3)
                        .bg(p.pane.opacity(0.45))
                        .items_center()
                        .justify_center()
                        .child(icon("plus", 44., p.fg3))
                        .into_any_element(),
                );
            } else {
                kids.push(at.child(self.pane(leaf, w, window, cx)).into_any_element());
            }
        }
        self.gutters = if dragging { Vec::new() } else { gutters };
        for (i, g) in self.gutters.clone().into_iter().enumerate() {
            kids.push(self.gutter(i, &g, cx).into_any_element());
        }
        if let Some(d) = self.pdrag.as_ref().filter(|d| d.active) {
            let b = self.grid_bounds.get();
            let (slot, to_tab) = (d.slot, d.tab_to.is_some());
            let (gx, gy) = if to_tab { (12., -14.) } else { d.grab };
            let s = if to_tab { 0.6 } else { 1. };
            let r = Rect {
                x: f32::from(d.pos.x - b.origin.x) - gx,
                y: f32::from(d.pos.y - b.origin.y) - gy,
                w: d.size.0 * s,
                h: d.size.1 * s,
            };
            // Remembered so a drop glides from where the card was let go.
            self.anim.rects.insert(slot, (r, r, now));
            kids.push(
                div()
                    .absolute()
                    .left(px(r.x))
                    .top(px(r.y))
                    .w(px(r.w))
                    .h(px(r.h))
                    .flex()
                    .opacity(0.92)
                    .rounded(px(9.))
                    .shadow(vec![BoxShadow {
                        color: hsla(0., 0., 0., 0.28),
                        offset: point(px(0.), px(18.)),
                        blur_radius: px(40.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .child(self.pane(slot, w, window, cx))
                    .into_any_element(),
            );
        }
        if moving || full.w == 0. {
            window.request_animation_frame();
        }
        let gb = self.grid_bounds.clone();
        let measure = canvas(
            move |b, window, _| {
                if gb.get() != b {
                    gb.set(b);
                    window.refresh();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        div()
            .id("grid")
            .size_full()
            .relative()
            .child(measure)
            .children(kids)
            .into_any_element()
    }

    fn gutter(&self, i: usize, g: &layout::Gutter, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = self.pal;
        let vertical = g.dir == Dir::Row;
        let on = self.drag == Some(Drag::Gut(i));
        let id = SharedString::from(format!("gut{i}"));
        let path = g.path.clone();
        div()
            .id(id.clone())
            .absolute()
            .left(px(g.rect.x))
            .top(px(g.rect.y))
            .w(px(g.rect.w))
            .h(px(g.rect.h))
            .group(id.clone())
            .map(|el| {
                if vertical {
                    el.cursor_col_resize()
                } else {
                    el.cursor_row_resize()
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.drag = Some(Drag::Gut(i));
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                if e.click_count() == 2 {
                    let mut t = this.tab().tree();
                    t.set_ratio(&path, 0.5);
                    this.tab_mut().layout = Some(t);
                    this.save_tabs();
                    cx.notify();
                }
            }))
            .child(
                div()
                    .absolute()
                    .rounded(px(2.))
                    .bg(if on {
                        p.pane_edge_on
                    } else {
                        transparent_black()
                    })
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

    pub(crate) fn drag_gutter(&mut self, i: usize, pos: Point<Pixels>) {
        let Some(g) = self.gutters.get(i).cloned() else {
            return;
        };
        let b = self.grid_bounds.get();
        let (x, y) = (f32::from(pos.x - b.origin.x), f32::from(pos.y - b.origin.y));
        let v = match g.dir {
            Dir::Row => (x - g.span.x) / g.span.w.max(1.),
            Dir::Col => (y - g.span.y) / g.span.h.max(1.),
        };
        let mut t = self.tab().tree();
        t.set_ratio(&g.path, v);
        self.tab_mut().layout = Some(t);
    }

    // -------------------------------------------------------------- canvases

    pub(crate) fn press_pane(&mut self, slot: usize, at: Point<Pixels>) {
        if self.max.is_some() {
            return;
        }
        self.pdrag = Some(PaneDrag {
            slot,
            start: at,
            pos: at,
            active: false,
            base: Node::leaf(0),
            leaves: Vec::new(),
            grab: (0., 0.),
            size: (0., 0.),
            target: None,
            tab_to: None,
        });
    }

    /// Pointer moved with a canvas pressed: start or update the drag.
    pub(crate) fn move_pane(&mut self, at: Point<Pixels>) -> bool {
        let full = self.grid_rect();
        let gb = self.grid_bounds.get();
        let base = self.tab().tree();
        let active_tab = self.prefs.active_tab;
        let tabs = self.tab_bounds.borrow().clone();
        let Some(d) = self.pdrag.as_mut() else {
            return false;
        };
        if !d.active {
            if dist(at, d.start) < SLOP {
                return false;
            }
            let (leaves, _) = base.layout(full, GAP);
            let Some(r) = leaves.iter().find(|l| l.0 == d.slot).map(|l| l.1) else {
                return false;
            };
            let (w, h) = (r.w.min(320.), r.h.min(200.));
            let (sx, sy) = (
                f32::from(d.start.x - gb.origin.x) - r.x,
                f32::from(d.start.y - gb.origin.y) - r.y,
            );
            d.grab = (sx.min(w - 40.).max(0.), sy.min(16.));
            d.size = (w, h);
            d.base = base;
            d.leaves = leaves;
            d.active = true;
        }
        d.pos = at;
        d.tab_to = tabs
            .iter()
            .position(|b| hit(*b, at))
            .filter(|&i| i != active_tab);
        if d.tab_to.is_some() {
            return true;
        }
        let (x, y) = (f32::from(at.x - gb.origin.x), f32::from(at.y - gb.origin.y));
        if let Some(&(leaf, r)) = d.leaves.iter().find(|(_, r)| r.contains(x, y)) {
            let side = if leaf == d.slot {
                Side::Center
            } else {
                layout::zone(r, x, y)
            };
            d.target = Some((leaf, side));
        }
        true
    }

    pub(crate) fn drop_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = self.pdrag.take() else {
            return;
        };
        if !d.active {
            return;
        }
        if let Some(ti) = d.tab_to {
            self.move_to_tab(d.slot, ti, window, cx);
            return;
        }
        if let Some((t, side)) = d.target {
            self.tab_mut().layout = Some(d.base.moved(d.slot, t, side, d.slot));
            self.focus = d.slot;
            self.save_tabs();
        }
        cx.notify();
    }

    fn move_to_tab(&mut self, slot: usize, ti: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.prefs.tabs.get(ti).is_none_or(|t| t.panes.len() >= 4) {
            cx.notify();
            return;
        }
        let from = self.tab_mut();
        let spec = from.panes[slot].clone();
        if from.panes.len() > 1 {
            from.layout = from.tree().remove(slot).map(|t| t.renumber(slot));
            from.panes.remove(slot);
        } else {
            from.panes[0] = PaneSpec::default();
        }
        let to = &mut self.prefs.tabs[ti];
        let n = to.panes.len();
        let tree = to.tree();
        to.panes.push(spec);
        to.layout = Some(Node::split(
            Dir::Row,
            n as f32 / (n + 1) as f32,
            tree,
            Node::leaf(n),
        ));
        self.anim.rects.clear();
        self.prefs.active_tab = ti;
        self.focus = n;
        self.max = None;
        self.save_tabs();
        self.load_pane_inputs(window, cx);
        cx.notify();
    }

    // -------------------------------------------------------------- tabs

    pub(crate) fn press_tab(&mut self, i: usize, at: Point<Pixels>) {
        self.tab_dragged = false;
        self.tdrag = Some(TabDrag {
            from: i,
            start: at,
            x: 0.,
            active: false,
            widths: Vec::new(),
            mids: Vec::new(),
            x0: 0.,
            grab: 0.,
            idx: i,
        });
    }

    pub(crate) fn move_tab(&mut self, at: Point<Pixels>) -> bool {
        let tabs = self.tab_bounds.borrow().clone();
        let Some(d) = self.tdrag.as_mut() else {
            return false;
        };
        if !d.active {
            if dist(at, d.start) < SLOP || tabs.len() <= d.from {
                return false;
            }
            d.widths = tabs.iter().map(|b| f32::from(b.size.width)).collect();
            d.mids = tabs
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != d.from)
                .map(|(_, b)| f32::from(b.origin.x + b.size.width / 2.))
                .collect();
            d.x0 = f32::from(tabs[0].origin.x);
            d.grab = f32::from(d.start.x - tabs[d.from].origin.x);
            d.active = true;
            self.tab_dragged = true;
        }
        let x = f32::from(at.x);
        d.x = x - d.x0 - d.grab;
        d.idx = d.mids.iter().filter(|&&m| m < x).count();
        true
    }

    pub(crate) fn drop_tab(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.tdrag.take() else {
            return;
        };
        self.anim.xs.clear();
        if !d.active {
            return;
        }
        let active = self.prefs.active_tab;
        let t = self.prefs.tabs.remove(d.from);
        self.prefs.tabs.insert(d.idx, t);
        let mut order: Vec<usize> = (0..self.prefs.tabs.len()).collect();
        let f = order.remove(d.from);
        order.insert(d.idx, f);
        self.prefs.active_tab = order.iter().position(|&o| o == active).unwrap_or(0);
        self.anim.key = None;
        self.save_tabs();
        cx.notify();
    }

    /// While a tab is dragged the strip is laid out by hand: the others
    /// glide apart around a dotted slot and the dragged one follows the
    /// pointer.
    pub(crate) fn tab_strip(
        &mut self,
        tabs: Vec<AnyElement>,
        plus: AnyElement,
        window: &mut Window,
    ) -> Div {
        let strip = div().flex().gap(px(4.)).items_center().min_w_0();
        let Some(d) = self.tdrag.as_ref().filter(|d| d.active) else {
            return strip.children(tabs).child(plus);
        };
        let (from, idx, dx) = (d.from, d.idx, d.x);
        let widths = d.widths.clone();
        let p = self.pal;
        let now = Instant::now();
        let mut moving = false;
        let mut x = 0.;
        let mut kids = Vec::new();
        let mut dragged = None;
        let mut k = 0;
        for (i, el) in tabs.into_iter().enumerate() {
            if i == from {
                dragged = Some(el);
                continue;
            }
            if k == idx {
                kids.push((usize::MAX, x, widths[from], None));
                x += widths[from] + 4.;
            }
            kids.push((i, x, widths.get(i).copied().unwrap_or(40.), Some(el)));
            x += widths.get(i).copied().unwrap_or(40.) + 4.;
            k += 1;
        }
        if k == idx {
            kids.push((usize::MAX, x, widths[from], None));
            x += widths[from] + 4.;
        }
        let total = x - 4.;
        let mut placed: Vec<AnyElement> = Vec::new();
        for (key, to, w, el) in kids {
            let at = self.anim.x(key, to, now, &mut moving);
            let slot = div().absolute().top(px(0.)).left(px(at));
            placed.push(match el {
                Some(el) => slot.child(el).into_any_element(),
                None => slot
                    .w(px(w))
                    .h(px(22.))
                    .rounded(px(6.))
                    .border(px(1.5))
                    .border_dashed()
                    .border_color(p.fg3)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("plus", 12., p.fg3))
                    .into_any_element(),
            });
        }
        if let Some(el) = dragged {
            placed.push(
                div()
                    .absolute()
                    .top(px(0.))
                    .left(px(dx))
                    .rounded(px(6.))
                    .shadow(vec![BoxShadow {
                        color: hsla(0., 0., 0., 0.2),
                        offset: point(px(0.), px(6.)),
                        blur_radius: px(16.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .child(el)
                    .into_any_element(),
            );
        }
        if moving {
            window.request_animation_frame();
        }
        strip
            .child(
                div()
                    .relative()
                    .flex_none()
                    .w(px(total))
                    .h(px(22.))
                    .children(placed),
            )
            .child(plus)
    }
}

fn dist(a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    let (dx, dy) = (f32::from(a.x - b.x), f32::from(a.y - b.y));
    (dx * dx + dy * dy).sqrt()
}
