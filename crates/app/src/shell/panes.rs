//! Split panes inside the thread card, direction B (Charles, 13 Nov: three at most, panes you can
//! drag, an "Open a surface" list in a new pane). The card head's layout button steps through
//! `cauldron_chrome::panes::Shape::ALL`, drawing the shape it would switch to and morphing to the
//! next when clicked; every divider has a line to drag; a pane's header drags it onto another pane
//! to swap them. The geometry is `cauldron_chrome::panes`; nothing here picks a position.

use super::*;
use cauldron_chrome::panes::{self, Line, Shape, GAP};

/// The layout button's drawing: an 18x14 sketch of a shape.
const SKETCH_W: f32 = 18.0;
const SKETCH_H: f32 = 14.0;
/// A line's grab area across the 4px gap.
const GRAB: f32 = 12.0;
/// A pane's header.
const HEAD_H: f32 = 32.0;

/// What a pane shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Content {
    /// The thread (or the New thread screen): always in some pane.
    Thread,
    /// A new pane: the "Open a surface" list.
    Empty,
    Surface(Surface),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Surface {
    Browser,
    Terminal,
    Files,
    Diff,
    PullRequest,
    LinkedPrs,
    Device,
}

/// The list in a new pane (Charles's screenshot, 13 Nov): icon, name, key, and whether it can open
/// yet. Diff and the pull requests wait for core.
const SURFACES: [(Surface, &str, &str, &str, bool); 7] = [
    (Surface::Browser, "globe", "Browser", "B", true),
    (Surface::Terminal, "terminal", "Terminal", "T", true),
    (Surface::Files, "files", "Files", "F", true),
    (Surface::Diff, "diff", "Diff", "D", false),
    (Surface::PullRequest, "git-pull-request", "Pull request", "P", false),
    (Surface::LinkedPrs, "link", "Linked pull requests", "L", false),
    (Surface::Device, "smartphone", "Device", "M", true),
];

impl Surface {
    fn name(self) -> &'static str {
        SURFACES.iter().find(|s| s.0 == self).map_or("", |s| s.2)
    }
    fn icon(self) -> &'static str {
        SURFACES.iter().find(|s| s.0 == self).map_or("square", |s| s.1)
    }
}

impl Content {
    fn title(self) -> &'static str {
        match self {
            Content::Thread => "Thread",
            Content::Empty => "New pane",
            Content::Surface(s) => s.name(),
        }
    }
}

/// The pane being dragged by its header: its slot.
#[derive(Clone, Copy)]
pub(crate) struct PaneDrag(usize);

/// What follows the pointer while a pane is dragged: its name on a chip.
struct DragChip(&'static str, &'static Palette);

impl Render for DragChip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pal = self.1;
        div()
            .px(px(10.))
            .h(px(28.))
            .flex()
            .items_center()
            .rounded(px(radius::CONTROL))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line_strong))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(pal.fg))
            .child(self.0)
    }
}

/// A shape's panes at sketch size, even splits, each inset 1px so the gaps show. Always three: a
/// shape with fewer repeats its last pane, so a morph between any two shapes has three to move.
fn sketch(shape: Shape) -> [Rect; 3] {
    let big = Rect::new(0.0, 0.0, 1000.0, 1000.0 * SKETCH_H / SKETCH_W);
    let rs = panes::rects(big, panes::Split { shape, ..Default::default() });
    let k = SKETCH_W / big.w;
    let at = |i: usize| {
        let r = rs[i.min(rs.len() - 1)];
        Rect::new(r.x * k + 0.75, r.y * k + 0.75, r.w * k - 1.5, r.h * k - 1.5)
    };
    [at(0), at(1), at(2)]
}

/// A 24px icon button for a pane header.
fn head_button(id: impl Into<ElementId>, icon: &str, label: &'static str, on: bool, pal: &'static Palette) -> Stateful<Div> {
    let col = if on { c(pal.fg2) } else { c(pal.fg4) };
    div()
        .id(id)
        .w(px(24.))
        .h(px(24.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::INNER))
        .when_live(on, |d| d.cursor_pointer().hover(|d| d.bg(c(pal.hover))))
        .aria_label(label)
        .child(glyph(icon, 14., col))
}

impl Cauldron {
    /// The layout button for the card head: it draws the shape a click switches to, and morphs
    /// from the last one over 200ms (none with reduced motion).
    pub(super) fn layout_button(&self, pal: &'static Palette, now: std::time::Instant, reduce: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let to = self.panes.shape.next();
        let k = match self.layout_since {
            Some((_, t0)) if !reduce => cauldron_chrome::motion::SETTLE.at(crate::motion::run(now, t0, cauldron_chrome::motion::time::LAYOUT)),
            _ => 1.0,
        };
        let from = self.layout_since.map_or(to, |(f, _)| f);
        let (a, b) = (sketch(from), sketch(to));
        let mut art = div().relative().w(px(SKETCH_W)).h(px(SKETCH_H)).flex_none();
        for i in 0..3 {
            let r = cauldron_chrome::motion::lerp_rect(a[i], b[i], k);
            art = art.child(abs(r).rounded(px(2.)).border_1().border_color(c(pal.fg2)));
        }
        let label: SharedString = format!("Layout: {}", to.label()).into();
        let el = div()
            .id("layout")
            .w(px(28.))
            .h(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::CONTROL))
            .cursor_pointer()
            .hover(|d| d.bg(c(pal.hover)))
            .aria_label(label.clone())
            .child(art)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_shape(to);
                cx.notify();
            }));
        self.tipped("layout-tip", el, label, Side::Below, pal, cx)
    }

    /// Switches the card to `shape`, starting the layout button's morph from what it drew. Panes
    /// that go away keep their place in the order, so stepping back and forth keeps them.
    pub(super) fn set_shape(&mut self, shape: Shape) {
        self.layout_since = Some((self.panes.shape.next(), std::time::Instant::now()));
        self.panes.shape = shape;
        // The thread always stays on screen: if it sat in a pane that is gone, it moves to the first.
        if let Some(i) = self.pane_content.iter().position(|p| *p == Content::Thread) {
            if i >= shape.count() {
                self.pane_content.swap(0, i);
            }
        }
    }

    /// Closes the pane in `slot`: the ones after it move up, a new pane goes last.
    fn close_pane(&mut self, slot: usize) {
        let Some(p) = self.panes.shape.prev() else { return };
        let gone = self.pane_content[slot];
        self.pane_content.copy_within(slot + 1.., slot);
        self.pane_content[panes::MAX - 1] = if gone == Content::Thread { Content::Thread } else { Content::Empty };
        self.set_shape(p);
    }

    fn open_surface(&mut self, slot: usize, s: Surface) {
        if SURFACES.iter().any(|x| x.0 == s && x.4) {
            self.pane_content[slot] = Content::Surface(s);
        }
    }

    /// The card's inside, split: each pane has a header (+, its name to drag it by, expand, split
    /// bottom, the drawer and close) over its content; the lines sit over the gaps. `area` is in
    /// window coordinates.
    pub(super) fn pane_stage(&mut self, pal: &'static Palette, area: Rect, shape: Shape, body: AnyElement, window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let sp = panes::Split { shape, ..self.panes };
        let local = |r: Rect| Rect::new(r.x - area.x, r.y - area.y, r.w, r.h);
        let ring = c(pal.line);
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let accent = c(pal.accent);
        let mut stage = div().id("pane-stage").relative().size_full();
        let mut body = Some(body);
        while self.pane_focus.len() < panes::MAX {
            self.pane_focus.push(cx.focus_handle());
        }
        let can_add = shape.add();
        let can_down = shape.split_down();
        let drawer_open = self.drawer.is_some();
        // Tabs in the pane header: a 40px header with the thread's tabs in its pane, the layout button
        // in the first; floating, the 32px header with the pane's name.
        let in_pane = !self.tabs_floating && self.coordinator.is_some();
        let head_h = if in_pane { 40.0 } else { HEAD_H };
        let now = std::time::Instant::now();
        let reduce = cx.reduce_motion();
        for (slot, r) in panes::rects(area, sp).into_iter().enumerate() {
            let content = self.pane_content[slot];
            let title = content.title();
            let strip = (in_pane && content == Content::Thread).then(|| self.tab_strip(pal, r.w, cx));
            let layout = (in_pane && slot == 0).then(|| self.layout_button(pal, now, reduce, cx));
            // A narrow pane keeps its tabs: expand, split bottom and the drawer toggle make way.
            let roomy = r.w >= super::ptabs::NARROW;
            let head = div()
                .id(("pane-head", slot))
                .h(px(head_h))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(2.))
                .px(px(4.))
                .border_b_1()
                .border_color(ring)
                .child(head_button(("pane-add", slot), "plus", "Add a pane", can_add.is_some(), pal).when_live(can_add.is_some(), |d| {
                    d.on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(s) = this.panes.shape.add() {
                            this.set_shape(s);
                        }
                        cx.notify();
                    }))
                }).test_support())
                .children(strip.map(|s| s.test_support()))
                // The name is the handle: drag it onto another pane to swap the two.
                .child(
                    div()
                        .id(("pane-grip", slot))
                        .when_live(in_pane && content == Content::Thread, |d| d.flex_none().w(px(12.)).child(glyph("grip-vertical", 12., fg3)))
                        .when_live(!(in_pane && content == Content::Thread), |d| d.flex_1())
                        .min_w_0()
                        .h_full()
                        .flex()
                        .items_center()
                        .px(px(4.))
                        .cursor_grab()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(fg2)
                        .aria_label(title)
                        .when_live(!(in_pane && content == Content::Thread), |d| d.child(div().min_w_0().truncate().child(title)))
                        .on_drag(PaneDrag(slot), move |_, _, _, cx| cx.new(|_| DragChip(title, pal)))
                        .test_support(),
                )
                .when_live(roomy, |d| d.child(head_button(("pane-expand", slot), "maximize-2", "Expand", true, pal).on_click(cx.listener(move |this, _, _, cx| {
                    // This pane alone: it moves first and the card goes back to one pane.
                    this.pane_content.swap(0, slot);
                    this.set_shape(Shape::One);
                    cx.notify();
                })).test_support()))
                .when_live(roomy, |d| d.child(head_button(("pane-down", slot), "panel-bottom", "Split bottom", can_down.is_some(), pal).when_live(can_down.is_some(), |d| {
                    d.on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(s) = this.panes.shape.split_down() {
                            this.set_shape(s);
                        }
                        cx.notify();
                    }))
                }).test_support()))
                .when_live(roomy, |d| d.child(head_button(("pane-drawer", slot), "panel-right", if drawer_open { "Hide the drawer" } else { "Show the drawer" }, true, pal).on_click(cx.listener(|this, _, _, cx| {
                    let p = this.drawer.or(this.drawer_last).unwrap_or(Panel::Highlights);
                    this.toggle_panel(p);
                    cx.notify();
                })).test_support()))
                .children(layout.map(|l| l.test_support()))
                .when_live(content != Content::Thread, |d| {
                    d.child(head_button(("pane-close", slot), "x", "Close pane", true, pal).on_click(cx.listener(move |this, _, _, cx| {
                        this.close_pane(slot);
                        cx.notify();
                    })).test_support())
                });
            let inner: AnyElement = match content {
                Content::Thread => body.take().unwrap_or_else(|| div().into_any_element()),
                Content::Empty => {
                    let mut list = div().id(("surfaces", slot)).w_full().max_w(px(300.)).flex().flex_col().gap(px(2.));
                    for (i, (s, icon, name, key, ok)) in SURFACES.into_iter().enumerate() {
                        let col = if ok { fg } else { c(pal.fg4) };
                        let mut row = div()
                            .id(("surface", slot * 10 + i))
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px(px(8.))
                            .rounded(px(radius::INNER))
                            .text_size(px(13.))
                            .text_color(col)
                            .aria_label(name)
                            .child(glyph(icon, 15., if ok { fg2 } else { c(pal.fg4) }))
                            .child(div().flex_1().min_w_0().truncate().child(name));
                        if s == Surface::Browser {
                            row = row.child(glyph("chevron-down", 13., fg3));
                        }
                        row = row.child(
                            div()
                                .flex_none()
                                .w(px(20.))
                                .h(px(20.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(radius::TAG))
                                .bg(c(pal.inset))
                                .border_1()
                                .border_color(ring)
                                .font_family("Geist Mono")
                                .text_size(px(11.))
                                .text_color(if ok { fg3 } else { c(pal.fg4) })
                                .child(key),
                        );
                        if ok {
                            row = row.cursor_pointer().hover(|d| d.bg(c(pal.hover))).on_click(cx.listener(move |this, _, _, cx| {
                                this.open_surface(slot, s);
                                cx.notify();
                            }));
                        }
                        list = list.child(row.test_support());
                    }
                    let focus = self.pane_focus[slot].clone();
                    div()
                        .id(("pane-empty", slot))
                        .track_focus(&focus)
                        .size_full()
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(12.))
                        .p(px(12.))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| this.pane_focus[slot].focus(window, cx)))
                        .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _, cx| {
                            let k = ev.keystroke.key.to_uppercase();
                            if let Some(s) = SURFACES.iter().find(|x| x.3 == k) {
                                this.open_surface(slot, s.0);
                                cx.notify();
                            }
                        }))
                        .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(fg).child("Open a surface"))
                        .child(list)
                        .into_any_element()
                }
                Content::Surface(s) => div()
                    .id(("surface-body", slot))
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .p(px(12.))
                    .aria_label(s.name())
                    .child(glyph(s.icon(), 20., fg3))
                    .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(fg2).child(s.name()))
                    .child(div().max_w_full().text_size(px(12.)).text_color(fg3).text_center().child(format!("The {} surface arrives in a later milestone.", s.name().to_lowercase())))
                    .test_support()
                    .into_any_element(),
            };
            let p = abs(local(r))
                .id(("pane", slot))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(radius::INSET))
                .bg(c(pal.panel))
                .border_1()
                .border_color(ring)
                .aria_label(title)
                .drag_over::<PaneDrag>(move |d, _, _, _| d.border_color(accent))
                .on_drop(cx.listener(move |this, d: &PaneDrag, _, cx| {
                    this.pane_content.swap(d.0, slot);
                    cx.notify();
                }))
                .child(head.test_support())
                .child(div().flex_1().min_h_0().overflow_hidden().child(inner));
            stage = stage.child(p.test_support());
        }
        let _ = window;
        for (line, l) in panes::lines(area, sp) {
            let vertical = line != Line::H;
            let l = local(l);
            let hit = if vertical { Rect::new(l.x + GAP / 2.0 - GRAB / 2.0, l.y, GRAB, l.h) } else { Rect::new(l.x, l.y + GAP / 2.0 - GRAB / 2.0, l.w, GRAB) };
            let id = match line {
                Line::V => "line-v",
                Line::V2 => "line-v2",
                Line::H => "line-h",
            };
            let dragging = self.pane_drag.is_some_and(|(l, _)| l == line);
            let mark = if vertical { div().w(px(2.)).h(px(28.)) } else { div().h(px(2.)).w(px(28.)) };
            stage = stage.child(
                abs(hit)
                    .id(id)
                    .flex()
                    .items_center()
                    .justify_center()
                    .group(id)
                    .when_live(vertical, |d| d.cursor_col_resize())
                    .when_live(!vertical, |d| d.cursor_row_resize())
                    .aria_label(if vertical { "Resize panes left and right" } else { "Resize panes above and below" })
                    .child(mark.rounded(px(1.)).when_live(dragging, |d| d.bg(accent)).group_hover(id, |d| d.bg(accent)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            if ev.click_count >= 2 {
                                this.panes.reset(line);
                                this.pane_drag = None;
                            } else {
                                this.pane_drag = Some((line, area));
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .test_support(),
            );
        }
        stage
    }

    /// While a line is held, the pointer moves it (on the root, so it keeps up outside the line).
    pub(super) fn drag_line(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some((line, area)) = self.pane_drag else { return };
        if ev.pressed_button != Some(MouseButton::Left) {
            self.pane_drag = None;
        } else {
            let at = if line == Line::H { ev.position.y } else { ev.position.x };
            self.panes.drag(line, area, f32::from(at));
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{rect, W, H};
    use super::super::Cauldron;
    use super::{Content, Surface};
    use cauldron_chrome::panes::Shape;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{point, px, size, AppContext, TestAppContext, Window, WindowHandle};

    fn open(cx: &mut TestAppContext) -> WindowHandle<Cauldron> {
        cx.update(gpui_kit::init);
        cx.update(|cx| cx.set_reduce_motion(true));
        let handle = cx.add_window(|_, _| Cauldron { drawer: None, ..Cauldron::new() });
        cx.simulate_window_resize(handle.into(), size(px(W), px(H)));
        handle
    }

    fn count(window: &Window) -> usize {
        (0..4usize).filter(|&i| window.try_find(("pane", i)).is_some()).count()
    }

    /// The card head's layout button names the shape it switches to, and each click steps to the
    /// next of the six, the last back to one pane: never more than three panes.
    #[gpui_kit::test]
    fn the_layout_button_steps_through_six_shapes(cx: &mut TestAppContext) {
        let handle = open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let head = rect(window, "card-head");
            let b = rect(window, "layout");
            assert!(b.x >= head.x && b.right() <= head.right() && b.y >= head.y && b.bottom() <= head.bottom(), "the button sits in the card head");
            assert_eq!(count(window), 0, "one pane: the card as it was, no pane frames");
            let mut seen = Vec::new();
            for _ in 0..6 {
                let label = window.find("layout").label().unwrap().to_string();
                window.click("layout", cx);
                seen.push((label, count(window)));
            }
            let want = [
                ("Layout: Two side by side", 2),
                ("Layout: Two stacked", 2),
                ("Layout: Three side by side", 3),
                ("Layout: Two on top, one below", 3),
                ("Layout: One tall, two stacked", 3),
                ("Layout: One pane", 0),
            ];
            assert_eq!(seen, want.map(|(l, n)| (l.to_string(), n)));
        })
        .unwrap();
    }

    /// A new pane is the "Open a surface" list (Charles's screenshot): seven rows with a key each,
    /// Diff and the pull requests dimmed. Opening a row loads it; a dimmed row does nothing.
    #[gpui_kit::test]
    fn a_new_pane_offers_the_surfaces(cx: &mut TestAppContext) {
        let handle = open(cx);
        handle.update(cx, |c, _, _| c.panes.shape = Shape::Cols).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("pane", 1usize)).label(), Some("New pane"));
            let names: Vec<String> = (0..7usize).map(|i| window.find(("surface", 10 + i)).label().unwrap().to_string()).collect();
            assert_eq!(names, ["Browser", "Terminal", "Files", "Diff", "Pull request", "Linked pull requests", "Device"]);
            window.click(("surface", 13usize), cx);
            assert_eq!(window.find(("pane", 1usize)).label(), Some("New pane"), "Diff is not available yet");
            window.click(("surface", 11usize), cx);
            assert_eq!(window.find(("pane", 1usize)).label(), Some("Terminal"));
        })
        .unwrap();
        assert_eq!(handle.update(cx, |c, _, _| c.pane_content[1]).unwrap(), Content::Surface(Surface::Terminal));
    }

    /// Panes sit 4px apart and keep 200 x 120; closing one returns to the shape before, and the
    /// thread pane has no close.
    #[gpui_kit::test]
    fn panes_are_4px_apart_and_close_steps_back(cx: &mut TestAppContext) {
        let handle = open(cx);
        handle.update(cx, |c, _, _| c.panes.shape = Shape::Three).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let (a, b, d) = (rect(window, ("pane", 0usize)), rect(window, ("pane", 1usize)), rect(window, ("pane", 2usize)));
            assert_eq!((b.x - a.right(), d.x - b.right()), (4.0, 4.0), "4px between columns");
            for i in 0..3usize {
                let r = rect(window, ("pane", i));
                assert!(r.w >= 200.0 && r.h >= 120.0, "pane {i} {r:?} keeps 200 x 120");
            }
            assert!(window.try_find(("pane-close", 0usize)).is_none(), "the thread stays");
            window.click(("pane-close", 2usize), cx);
            assert_eq!(count(window), 2, "closing a pane of three leaves two side by side");
            window.click(("pane-add", 0usize), cx);
            assert_eq!(count(window), 3, "+ adds one back");
            assert!(window.try_find(("pane-add", 0usize)).is_some());
        })
        .unwrap();
    }

    /// Dragging a pane by its name onto another swaps the two.
    #[gpui_kit::test]
    fn a_pane_drags_onto_another_to_swap(cx: &mut TestAppContext) {
        let handle = open(cx);
        handle.update(cx, |c, _, _| c.panes.shape = Shape::OneTwo).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find(("pane", 0usize)).label(), Some("Thread"));
            window.drag_to(("pane-grip", 0usize), ("pane", 2usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find(("pane", 2usize)).label(), Some("Thread"), "the thread moved to the bottom right");
            assert_eq!(window.find(("pane", 0usize)).label(), Some("New pane"));
        })
        .unwrap();
    }

    /// Every line drags its split with the pointer and stops at 200px; a double-click evens it.
    #[gpui_kit::test]
    fn the_lines_drag_the_split(cx: &mut TestAppContext) {
        let handle = open(cx);
        handle.update(cx, |c, _, _| c.panes.shape = Shape::Three).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let first = rect(window, ("pane", 0usize));
            let v = rect(window, "line-v2");
            let (mx, my) = (v.x + v.w / 2.0, v.y + v.h / 2.0);
            window.drag(point(px(mx), px(my)), point(px(mx + 50.0), px(my)), cx);
            window.render_frame(cx);
            assert_eq!(rect(window, ("pane", 0usize)), first, "the second line leaves the first column");
            assert!((rect(window, ("pane", 1usize)).right() + 2.0 - (mx + 50.0)).abs() <= 1.0, "the line follows the pointer");
            window.drag(point(px(mx + 50.0), px(my)), point(px(W - 5.0), px(my)), cx);
            window.render_frame(cx);
            assert_eq!(rect(window, ("pane", 2usize)).w, 200.0, "no narrower than 200");
            window.double_click("line-v2", cx);
            window.render_frame(cx);
            let ws: Vec<f32> = (0..3usize).map(|i| rect(window, ("pane", i)).w).collect();
            assert!(ws.iter().all(|w| (w - ws[0]).abs() <= 1.0), "even again: {ws:?}");
        })
        .unwrap();
    }
}
