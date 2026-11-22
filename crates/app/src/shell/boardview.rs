//! The Board view of a coordinator stream (Charles, 22 Nov): Chat | Board at the top of the
//! stream; the board is every generation as a node (core whiteboard.rs), joined along its thread,
//! with a pen, labels, sticky notes and hand-drawn links on top. A node opens in a centred split
//! that takes most of the window: the node on the left (how we got here, what was asked, its
//! pictures, what changed, its references), a chat about it on the right under Open stream,
//! Branch from here and Send to an agent. Side streams have no board: what they ship lands here.

use std::cell::Cell;
use std::rc::Rc;

use super::*;
use backspace_core::chat::{Role, SideCtx, Thread};
use backspace_core::whiteboard::{Board, Boards, Link, Node, Note, NODE_W};
use gpui_kit::prelude::FluentBuilder;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Tool {
    #[default]
    Select,
    Hand,
    Pen,
    Text,
    Sticky,
    Link,
    Erase,
}

const TOOLS: [(Tool, &str, &str, &str); 7] = [
    (Tool::Select, "mouse-pointer-2", "Select and move", "V"),
    (Tool::Hand, "hand", "Pan", "H"),
    (Tool::Pen, "pencil", "Draw", "P"),
    (Tool::Text, "type", "Write", "T"),
    (Tool::Sticky, "sticky-note", "Sticky note", "S"),
    (Tool::Link, "move-up-right", "Link two nodes", "A"),
    (Tool::Erase, "eraser", "Erase", "E"),
];

pub(crate) enum Drag {
    Pan { sx: f32, sy: f32, ox: f32, oy: f32 },
    Node { id: String, sx: f32, sy: f32, ox: f32, oy: f32, moved: bool },
    Pen(Vec<(f32, f32)>),
    Split { sx: f32, w0: f32, total: f32 },
}

pub(crate) struct BoardState {
    /// Board shown instead of the chat for the open coordinator stream.
    pub view: bool,
    /// Whose board is loaded, and how many messages it had when it was synced.
    pub thread: String,
    pub seen: usize,
    pub b: Board,
    /// The screen offset of board point (0, 0) and the zoom.
    pub cam: (f32, f32, f32),
    pub fitted: bool,
    pub tool: Tool,
    pub drag: Option<Drag>,
    pub link_from: Option<String>,
    /// Follow the threads down instead of across.
    pub down: bool,
    /// The node open in the split.
    pub open: Option<String>,
    /// The chat side's width, as a share of the split.
    pub split: f32,
    pub note_edit: Option<(String, Entity<InputState>, Subscription)>,
    pub chat_input: Option<(Entity<InputState>, Subscription)>,
    pub agent_menu: bool,
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl Default for BoardState {
    fn default() -> Self {
        BoardState {
            view: false,
            thread: String::new(),
            seen: 0,
            b: Board::default(),
            cam: (60., 90., 0.8),
            fitted: false,
            tool: Tool::Select,
            drag: None,
            link_from: None,
            down: false,
            open: None,
            split: 0.38,
            note_edit: None,
            chat_input: None,
            agent_menu: false,
            bounds: Rc::new(Cell::new(Bounds::default())),
        }
    }
}

/// A node's height on the board, as drawn (for links and Fit).
fn node_h(n: &Node) -> f32 {
    44. + if n.images.is_empty() { 0. } else { 140. } + 17. * n.changes.len().min(4) as f32 + 34.
}

fn clock(ms: u64) -> String {
    use chrono::{Local, TimeZone};
    Local.timestamp_millis_opt(ms as i64).single().map(|t| t.format("%H:%M").to_string()).unwrap_or_default()
}

impl BoardState {
    /// Board point to display point: across as stored, down with the axes swapped.
    fn disp(&self, x: f32, y: f32) -> (f32, f32) {
        if self.down {
            (y * 0.98, x * 1.15)
        } else {
            (x, y)
        }
    }

    fn undisp(&self, x: f32, y: f32) -> (f32, f32) {
        if self.down {
            (y / 1.15, x / 0.98)
        } else {
            (x, y)
        }
    }

    /// Board point to screen point, relative to the board's top left.
    fn to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        let (dx, dy) = self.disp(x, y);
        (dx * self.cam.2 + self.cam.0, dy * self.cam.2 + self.cam.1)
    }

    /// A window position to a board point.
    fn to_board(&self, p: Point<Pixels>) -> (f32, f32) {
        let o = self.bounds.get().origin;
        let (sx, sy) = (f32::from(p.x - o.x), f32::from(p.y - o.y));
        self.undisp((sx - self.cam.0) / self.cam.2, (sy - self.cam.1) / self.cam.2)
    }

    /// A node's box in display units.
    fn node_box(&self, n: &Node) -> (f32, f32, f32, f32) {
        let (x, y) = self.disp(n.x, n.y);
        (x, y, NODE_W, node_h(n))
    }

    /// Frames the board: everything, at any zoom (`all`), or at a zoom you can read with the
    /// newest work in view (opening the board, switching Across/Down).
    fn fit(&mut self) {
        self.frame(false);
    }

    fn frame(&mut self, all: bool) {
        let b = self.bounds.get();
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        if self.b.nodes.is_empty() || w < 10. {
            self.cam = (60., 90., 0.8);
            return;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for n in &self.b.nodes {
            let (x, y, nw, nh) = self.node_box(n);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + nw);
            y1 = y1.max(y + nh);
        }
        let k = ((w - 120.) / (x1 - x0)).min((h - 180.) / (y1 - y0)).clamp(if all { 0.2 } else { 0.75 }, 1.0);
        // Too long to show whole: the newest end stays in view (right Across, bottom Down).
        let cx = if (x1 - x0) * k <= w - 120. { (w - (x1 - x0) * k) / 2. - x0 * k } else if self.down { 60. - x0 * k } else { w - 60. - x1 * k };
        let cy = if (y1 - y0) * k <= h - 180. { 60. + (h - 120. - (y1 - y0) * k) / 2. - y0 * k } else if self.down { h - 100. - y1 * k } else { 70. - y0 * k };
        self.cam = (cx, cy, k);
        self.fitted = true;
    }

    fn zoom_at(&mut self, f: f32, sx: f32, sy: f32) {
        let k = (self.cam.2 * f).clamp(0.2, 2.0);
        self.cam.0 = sx - (sx - self.cam.0) * k / self.cam.2;
        self.cam.1 = sy - (sy - self.cam.1) * k / self.cam.2;
        self.cam.2 = k;
    }
}

/// A coordinator: a stream of its own, not a side stream or a group.
pub(crate) fn is_coordinator(t: &Thread) -> bool {
    t.side_of.is_none() && t.members.is_empty()
}

impl Cauldron {
    /// Boards sit beside the threads they belong to.
    fn boards(&self) -> Boards {
        Boards::open(&self.core.as_ref().map_or_else(Prefs::data_dir, |(_, ch)| ch.dir().to_path_buf()))
    }

    fn board_save(&self) {
        if !self.board.thread.is_empty() {
            let _ = self.boards().save(&self.board.thread, &self.board.b);
        }
    }

    /// Loads the open stream's board when it changed, and adds the nodes its new replies (and its
    /// side streams') made.
    pub(super) fn board_sync(&mut self, t: &Thread) {
        let Some((_, chats)) = self.core.as_ref() else { return };
        let sides: Vec<Thread> = chats.list().into_iter().filter(|x| x.side_of.as_deref() == Some(t.id.as_str())).filter_map(|x| chats.peek(&x.id)).collect();
        let seen = t.messages.len() + sides.iter().map(|s| s.messages.len()).sum::<usize>();
        if self.board.thread == t.id && self.board.seen == seen {
            return;
        }
        if self.board.thread != t.id {
            self.board.b = self.boards().load(&t.id);
            self.board.thread = t.id.clone();
            self.board.fitted = false;
            self.board.open = None;
        }
        self.board.seen = seen;
        if self.board.b.sync(t, &sides) {
            self.board_save();
        }
    }

    /// Before render: brings the open coordinator's board up to date, and fits it once it has a size.
    #[inline(never)]
    pub(super) fn board_prepare(&mut self) {
        if !self.board.view || self.open.is_none() {
            return;
        }
        let Some(t) = self.thread.take() else { return };
        if is_coordinator(&t) {
            self.board_sync(&t);
            if !self.board.fitted && f32::from(self.board.bounds.get().size.width) > 10. {
                self.board.fit();
            }
        }
        self.thread = Some(t);
    }

    /// Pins reply `msg` of the open stream to its coordinator's board (a side stream's reply lands
    /// on its own lane there). Says so in the box's error line when there is no board to pin to.
    pub(super) fn board_pin(&mut self, msg: &str, cx: &mut Context<Self>) {
        let (Some(t), Some((_, chats))) = (self.thread.clone(), self.core.as_ref()) else { return };
        let root = t.side_of.clone().unwrap_or_else(|| t.id.clone());
        let Some(main) = (if root == t.id { Some(t.clone()) } else { chats.peek(&root) }).filter(is_coordinator) else {
            self.send_error = Some("Only a coordinator stream has a board.".into());
            cx.notify();
            return;
        };
        let lane = chats.list().into_iter().filter(|x| x.side_of.as_deref() == Some(root.as_str())).position(|x| x.id == t.id).map_or(0, |p| p + 1);
        self.board_sync(&main);
        let via = (lane > 0).then(|| t.title.clone());
        if let Some(i) = t.messages.iter().position(|m| m.id == msg) {
            if self.board.b.pin(&t, i, via, lane) {
                self.board_save();
            }
        }
        cx.notify();
    }

    fn set_board_view(&mut self, on: bool, cx: &mut Context<Self>) {
        self.board.view = on;
        if on {
            if let Some(t) = self.thread.clone() {
                self.board_sync(&t);
            }
            self.board.fitted = false;
        }
        cx.notify();
    }

    /// Chat | Board over a coordinator stream.
    #[inline(never)]
    pub(super) fn view_switch(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let seg = |id: &'static str, label: &'static str, on: bool| {
            div()
                .id(id)
                .h(px(24.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(if on { c(pal.accent) } else { c(pal.fg2) })
                .when(on, |d| d.bg(c(pal.accent_soft)).font_weight(FontWeight::SEMIBOLD))
                .when(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                .aria_label(label)
                .aria_selected(on)
                .child(label)
        };
        let on = self.board.view;
        div()
            .id("view-switch")
            .absolute()
            .top(px(8.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .flex()
                    .gap(px(2.))
                    .p(px(2.))
                    .rounded(px(radius::CONTROL + 2.))
                    .bg(c(pal.raised))
                    .border_1()
                    .border_color(c(pal.line))
                    .shadow(super::lift(super::Lift::Button, pal))
                    .child(seg("view-chat", "Chat", !on).on_click(cx.listener(|this, _, _, cx| this.set_board_view(false, cx))).test_support())
                    .child(seg("view-board", "Board", on).on_click(cx.listener(|this, _, _, cx| this.set_board_view(true, cx))).test_support()),
            )
            .test_support()
            .into_any_element()
    }

    fn board_note_start(&mut self, id: String, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx).placeholder("Write…"));
        st.update(cx, |s, cx| s.set_value(text, window, cx));
        let nid = id.clone();
        let sub = cx.subscribe_in(&st, window, move |this, st, ev: &InputEvent, _, cx| {
            if let InputEvent::PressEnter { .. } | InputEvent::Blur = ev {
                let v = st.read(cx).value().trim().to_string();
                if let Some(n) = this.board.b.notes.iter_mut().find(|n| n.id == nid) {
                    n.text = v.clone();
                }
                if v.is_empty() {
                    this.board.b.notes.retain(|n| n.id != nid);
                }
                this.board.note_edit = None;
                this.board_save();
                cx.notify();
            }
        });
        let f = st.read(cx).focus_handle(cx);
        window.focus(&f, cx);
        self.board.note_edit = Some((id, st, sub));
    }

    fn board_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let (bx, by) = self.board.to_board(ev.position);
        let (sx, sy) = (f32::from(ev.position.x), f32::from(ev.position.y));
        match self.board.tool {
            Tool::Pen => self.board.drag = Some(Drag::Pen(vec![(bx, by)])),
            Tool::Text | Tool::Sticky => {
                let id = format!("n{}", backspace_core::chat::now_ms());
                self.board.b.notes.push(Note { id: id.clone(), x: bx, y: by, text: String::new(), sticky: self.board.tool == Tool::Sticky });
                self.board.tool = Tool::Select;
                self.board_note_start(id, String::new(), window, cx);
            }
            Tool::Erase => {
                let k = self.board.cam.2;
                let near = |p: &(f32, f32)| ((p.0 - bx).powi(2) + (p.1 - by).powi(2)).sqrt() * k < 8.;
                if let Some(i) = self.board.b.strokes.iter().position(|s| s.iter().any(near)) {
                    self.board.b.strokes.remove(i);
                    self.board_save();
                }
            }
            _ => self.board.drag = Some(Drag::Pan { sx, sy, ox: self.board.cam.0, oy: self.board.cam.1 }),
        }
        cx.notify();
    }

    fn board_node_down(&mut self, id: String, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        match self.board.tool {
            Tool::Link => match self.board.link_from.take() {
                None => self.board.link_from = Some(id),
                Some(a) if a != id => {
                    self.board.b.links.push(Link { from: a, to: id, fork: false, manual: true });
                    self.board_save();
                }
                Some(_) => {}
            },
            Tool::Erase => {
                self.board.b.remove(&id);
                self.board_save();
            }
            _ => {
                let (bx, by) = self.board.to_board(ev.position);
                if let Some(n) = self.board.b.nodes.iter().find(|n| n.id == id) {
                    self.board.drag = Some(Drag::Node { id, sx: bx, sy: by, ox: n.x, oy: n.y, moved: false });
                }
            }
        }
        cx.notify();
    }

    fn board_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        if ev.pressed_button != Some(MouseButton::Left) {
            if self.board.drag.is_some() {
                self.board_up(cx);
            }
            return;
        }
        let (bx, by) = self.board.to_board(ev.position);
        let (sx, sy) = (f32::from(ev.position.x), f32::from(ev.position.y));
        match &mut self.board.drag {
            Some(Drag::Pan { sx: x0, sy: y0, ox, oy }) => self.board.cam = (*ox + sx - *x0, *oy + sy - *y0, self.board.cam.2),
            Some(Drag::Pen(pts)) => pts.push((bx, by)),
            Some(Drag::Node { id, sx: x0, sy: y0, ox, oy, moved }) => {
                let (dx, dy) = (bx - *x0, by - *y0);
                if dx.abs() + dy.abs() > 3. / self.board.cam.2 {
                    *moved = true;
                }
                let (nx, ny) = (*ox + dx, *oy + dy);
                let id = id.clone();
                if let Some(n) = self.board.b.nodes.iter_mut().find(|n| n.id == id) {
                    n.x = nx;
                    n.y = ny;
                }
            }
            Some(Drag::Split { sx: x0, w0, total }) => {
                let w = (*w0 - (sx - *x0)).clamp(300., *total - 320.);
                self.board.split = w / *total;
            }
            None => return,
        }
        cx.notify();
    }

    fn board_up(&mut self, cx: &mut Context<Self>) {
        match self.board.drag.take() {
            Some(Drag::Pen(pts)) if pts.len() > 1 => {
                self.board.b.strokes.push(pts);
                self.board_save();
            }
            Some(Drag::Node { id, moved: false, .. }) => {
                self.board.open = Some(id);
                self.board.agent_menu = false;
            }
            Some(Drag::Node { .. }) => self.board_save(),
            _ => {}
        }
        cx.notify();
    }

    fn board_wheel(&mut self, ev: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let b = self.board.bounds.get();
        let (sx, sy) = (f32::from(ev.position.x - b.origin.x), f32::from(ev.position.y - b.origin.y));
        let (dx, dy) = match ev.delta {
            ScrollDelta::Lines(p) => (p.x * 40., p.y * 40.),
            ScrollDelta::Pixels(p) => (f32::from(p.x), f32::from(p.y)),
        };
        if ev.modifiers.control || ev.modifiers.platform {
            self.board.zoom_at(if dy > 0. { 1.1 } else { 1. / 1.1 }, sx, sy);
        } else {
            self.board.cam.0 += dx;
            self.board.cam.1 += dy;
        }
        cx.notify();
    }

    /// A coordinator stream: its chat or its board, with the Chat | Board switch on top.
    #[inline(never)]
    pub(super) fn coordinator_body(&self, t: &Thread, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let inner = if self.board.view { self.board_el(pal, cx) } else { self.thread_view(t, pal, cx) };
        div().relative().size_full().child(inner).child(self.view_switch(pal, cx)).into_any_element()
    }

    /// The board, when the open stream shows Board.
    #[inline(never)]
    pub(super) fn board_el(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let st = &self.board;
        let k = st.cam.2;
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let lane_col = |n: &Node| if n.via.is_some() { c(pal.warn) } else { c(pal.accent) };
        // Links and strokes, painted in one pass under the nodes.
        let mut segs: Vec<((f32, f32), (f32, f32), bool, bool)> = Vec::new();
        for l in &st.b.links {
            let (Some(a), Some(b)) = (st.b.nodes.iter().find(|n| n.id == l.from), st.b.nodes.iter().find(|n| n.id == l.to)) else { continue };
            let (ax, ay, aw, ah) = st.node_box(a);
            let (bx, by, _, bh) = st.node_box(b);
            let (p1, p2) = if st.down { ((ax + aw / 2., ay + ah), (bx + NODE_W / 2., by)) } else { ((ax + aw, ay + ah.min(180.) / 2.), (bx, by + bh.min(180.) / 2.)) };
            let s1 = (p1.0 * k + st.cam.0, p1.1 * k + st.cam.1);
            let s2 = (p2.0 * k + st.cam.0, p2.1 * k + st.cam.1);
            segs.push((s1, s2, l.fork, l.manual));
        }
        let mut strokes: Vec<Vec<(f32, f32)>> = st.b.strokes.iter().map(|s| s.iter().map(|p| st.to_screen(p.0, p.1)).collect()).collect();
        if let Some(Drag::Pen(pts)) = &st.drag {
            strokes.push(pts.iter().map(|p| st.to_screen(p.0, p.1)).collect());
        }
        let (line, accent, ink, dot) = (c(pal.fg3), c(pal.accent), c(pal.fg), c(pal.line_strong));
        let down = st.down;
        let cam = st.cam;
        let bounds_out = st.bounds.clone();
        let paint = canvas(
            move |b, _, _| {
                bounds_out.set(b);
                b
            },
            move |b, _, window, _| {
                let o = b.origin;
                let at = |x: f32, y: f32| point(o.x + px(x), o.y + px(y));
                let step = 22. * cam.2;
                if step >= 6. {
                    let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
                    let r = (1.1 * cam.2).max(0.7);
                    let mut y = cam.1.rem_euclid(step);
                    while y < h {
                        let mut x = cam.0.rem_euclid(step);
                        while x < w {
                            window.paint_quad(fill(Bounds::new(at(x - r, y - r), size(px(2. * r), px(2. * r))), dot).corner_radii(px(r)));
                            x += step;
                        }
                        y += step;
                    }
                }
                for &((x1, y1), (x2, y2), fork, manual) in &segs {
                    let col = if manual { accent } else { line };
                    let w = (1.8 * cam.2).max(1.);
                    let mut pb = PathBuilder::stroke(px(w));
                    if fork {
                        pb = pb.dash_array(&[px(6. * cam.2), px(5. * cam.2)]);
                    }
                    pb.move_to(at(x1, y1));
                    if down {
                        let m = (y2 - y1) / 2.;
                        pb.cubic_bezier_to(at(x2, y2), at(x1, y1 + m), at(x2, y2 - m));
                    } else {
                        let m = (x2 - x1) / 2.;
                        pb.cubic_bezier_to(at(x2, y2), at(x1 + m, y1), at(x2 - m, y2));
                    }
                    if let Ok(p) = pb.build() {
                        window.paint_path(p, col);
                    }
                    let a = 8. * cam.2.max(0.5);
                    let mut head = PathBuilder::fill();
                    if down {
                        head.move_to(at(x2 - a / 2., y2 - a));
                        head.line_to(at(x2, y2));
                        head.line_to(at(x2 + a / 2., y2 - a));
                    } else {
                        head.move_to(at(x2 - a, y2 - a / 2.));
                        head.line_to(at(x2, y2));
                        head.line_to(at(x2 - a, y2 + a / 2.));
                    }
                    head.close();
                    if let Ok(p) = head.build() {
                        window.paint_path(p, col);
                    }
                }
                for s in &strokes {
                    if s.len() < 2 {
                        continue;
                    }
                    let mut pb = PathBuilder::stroke(px((2.4 * cam.2).max(1.2)));
                    pb.move_to(at(s[0].0, s[0].1));
                    for p in &s[1..] {
                        pb.line_to(at(p.0, p.1));
                    }
                    if let Ok(p) = pb.build() {
                        window.paint_path(p, ink);
                    }
                }
            },
        )
        .absolute()
        .inset_0()
        .size_full();

        let mut world = div().absolute().inset_0().overflow_hidden().child(paint);
        for (i, n) in st.b.nodes.iter().enumerate() {
            let (sx, sy) = st.to_screen(n.x, n.y);
            let linking = st.link_from.as_deref() == Some(n.id.as_str());
            let id = n.id.clone();
            let mut chips: Vec<(String, Hsla, bool)> = n.refs.iter().filter(|r| r.kind != "Picture").take(2).map(|r| (if r.kind == "Commit" { r.text.clone() } else { r.kind.clone() }, fg3, false)).collect();
            if let Some(v) = &n.via {
                chips.push((format!("via {v}"), c(pal.warn), true));
            }
            if n.pinned {
                chips.push(("Pinned".into(), c(pal.accent), true));
            }
            let mut card = div()
                .id(("board-node", i))
                .absolute()
                .left(px(sx))
                .top(px(sy))
                .w(px(NODE_W * k))
                .flex()
                .flex_col()
                .rounded(px(12. * k))
                .bg(c(pal.raised))
                .border_1()
                .border_color(if linking { c(pal.accent) } else { c(pal.line_strong) })
                .shadow(super::lift(super::Lift::Button, pal))
                .cursor_pointer()
                .aria_label(n.title.clone())
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.board_node_down(id.clone(), ev, cx);
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8. * k))
                        .px(px(12. * k))
                        .pt(px(10. * k))
                        .pb(px(6. * k))
                        .child(div().size(px(8. * k)).flex_none().rounded_full().bg(lane_col(n)))
                        .child(div().flex_1().min_w_0().truncate().text_size(px(13.5 * k)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(n.title.clone()))
                        .child(div().flex_none().text_size(px(11. * k)).text_color(fg3).child(clock(n.at))),
                );
            if let Some(p) = n.images.first() {
                card = card.child(
                    div()
                        .mx(px(12. * k))
                        .h(px(132. * k))
                        .rounded(px(8. * k))
                        .overflow_hidden()
                        .border_1()
                        .border_color(c(pal.line))
                        .child(img(std::path::PathBuf::from(p)).size_full().object_fit(ObjectFit::Cover)),
                );
            }
            let mut ul = div().flex().flex_col().gap(px(3. * k)).px(px(12. * k)).pt(px(8. * k));
            for ch in n.changes.iter().take(4) {
                ul = ul.child(div().flex().gap(px(6. * k)).text_size(px(12. * k)).line_height(px(16. * k)).text_color(fg2).child(div().flex_none().child("\u{2022}")).child(div().min_w_0().truncate().child(ch.clone())));
            }
            card = card.child(ul);
            let mut chip_row = div().flex().flex_wrap().gap(px(5. * k)).px(px(12. * k)).pt(px(8. * k)).pb(px(10. * k));
            for (t, col, strong) in chips {
                chip_row = chip_row.child(div().px(px(6. * k)).py(px(1. * k)).rounded(px(5. * k)).border_1().border_color(if strong { col.opacity(0.0) } else { c(pal.line) }).when(strong, |d| d.bg(col.opacity(0.13))).text_size(px(10.5 * k)).text_color(col).child(t));
            }
            world = world.child(card.child(chip_row).test_support());
        }
        let editing = st.note_edit.as_ref().map(|(id, s, _)| (id.clone(), s.clone()));
        for (i, nt) in st.b.notes.iter().enumerate() {
            let (sx, sy) = st.to_screen(nt.x, nt.y);
            let nid = nt.id.clone();
            let text = nt.text.clone();
            let mut d = div().id(("board-note", i)).absolute().left(px(sx)).top(px(sy)).cursor_pointer();
            d = if nt.sticky {
                d.w(px(170. * k)).min_h(px(80. * k)).p(px(10. * k)).rounded(px(4. * k)).bg(gpui::rgb(0xffe58a)).text_size(px(12.5 * k)).line_height(px(17. * k)).text_color(gpui::rgb(0x3d3304)).shadow(super::lift(super::Lift::Button, pal))
            } else {
                d.min_w(px(30. * k)).text_size(px(16. * k)).font_weight(FontWeight::SEMIBOLD).text_color(fg)
            };
            d = match editing.as_ref().filter(|(id, _)| id == &nt.id) {
                Some((_, s)) => d.child(Input::new(s).appearance(false).text_size(px(if nt.sticky { 12.5 } else { 16. } * k)).p_0()),
                None => d.child(nt.text.clone()),
            };
            world = world.child(
                d.aria_label(nt.text.clone())
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        if this.board.tool == Tool::Erase {
                            this.board.b.notes.retain(|n| n.id != nid);
                            this.board_save();
                        } else if this.board.note_edit.is_none() {
                            this.board_note_start(nid.clone(), text.clone(), window, cx);
                        }
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        if st.b.nodes.is_empty() {
            world = world.child(
                div()
                    .id("board-empty")
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(4.))
                    .aria_label("Nothing on the board yet")
                    .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(fg2).child("Nothing on the board yet"))
                    .child(div().max_w(px(380.)).text_center().text_size(px(12.5)).text_color(fg3).child("A reply that ships something (a commit, a picture, an HTML page) becomes a node here. Pin any other reply from its hover bar."))
                    .test_support(),
            );
        }
        // The tools, at the bottom.
        let mut tools = div().id("board-tools").absolute().bottom(px(14.)).left_0().right_0().flex().justify_center();
        let mut bar = div().flex().gap(px(2.)).p(px(4.)).rounded(px(radius::PANEL)).bg(c(pal.raised)).border_1().border_color(c(pal.line)).shadow(super::lift(super::Lift::Button, pal));
        for (j, (tool, icon, label, key)) in TOOLS.iter().enumerate() {
            let on = st.tool == *tool;
            let t = *tool;
            bar = bar.child(
                div()
                    .id(("board-tool", j))
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .when(on, |d| d.bg(c(pal.accent_soft)))
                    .when(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                    .aria_label(format!("{label} ({key})"))
                    .aria_selected(on)
                    .child(glyph(icon, 16., if on { c(pal.accent) } else { fg2 }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.board.tool = t;
                        this.board.link_from = None;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        tools = tools.child(bar);
        let small = |id: &'static str, label: &'static str| div().id(id).h(px(26.)).px(px(8.)).flex().items_center().justify_center().rounded(px(radius::CONTROL)).cursor_pointer().hover(|d| d.bg(c(pal.hover))).text_size(px(12.)).text_color(fg2).aria_label(label).child(label);
        let zoom = div()
            .id("board-zoom")
            .absolute()
            .bottom(px(14.))
            .right(px(14.))
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(3.))
            .rounded(px(radius::CONTROL + 2.))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .child(small("board-zoom-out", "\u{2212}").on_click(cx.listener(|this, _, _, cx| {
                let b = this.board.bounds.get();
                this.board.zoom_at(1. / 1.2, f32::from(b.size.width) / 2., f32::from(b.size.height) / 2.);
                cx.notify();
            })))
            .child(div().w(px(40.)).text_center().text_size(px(11.5)).text_color(fg3).child(format!("{}%", (k * 100.).round())))
            .child(small("board-zoom-in", "+").on_click(cx.listener(|this, _, _, cx| {
                let b = this.board.bounds.get();
                this.board.zoom_at(1.2, f32::from(b.size.width) / 2., f32::from(b.size.height) / 2.);
                cx.notify();
            })))
            .child(small("board-fit", "Fit").on_click(cx.listener(|this, _, _, cx| {
                this.board.frame(true);
                cx.notify();
            })));
        let dir = div()
            .id("board-dir")
            .absolute()
            .top(px(8.))
            .left(px(12.))
            .flex()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(radius::CONTROL + 2.))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .child(small("board-across", "Across").when(!down, |d| d.bg(c(pal.accent_soft)).text_color(c(pal.accent))).on_click(cx.listener(|this, _, _, cx| {
                this.board.down = false;
                this.board.fit();
                cx.notify();
            })))
            .child(small("board-down", "Down").when(down, |d| d.bg(c(pal.accent_soft)).text_color(c(pal.accent))).on_click(cx.listener(|this, _, _, cx| {
                this.board.down = true;
                this.board.fit();
                cx.notify();
            })));
        div()
            .id("board")
            .relative()
            .size_full()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, window, cx| this.board_down(ev, window, cx)))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| this.board_move(ev, cx)))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| this.board_up(cx)))
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| this.board_wheel(ev, cx)))
            .child(world)
            .child(tools)
            .child(zoom)
            .child(dir)
            .test_support()
            .into_any_element()
    }

    // ---- the node, opened ------------------------------------------------------------------

    fn board_node(&self) -> Option<Node> {
        let id = self.board.open.as_ref()?;
        self.board.b.nodes.iter().find(|n| &n.id == id).cloned()
    }

    fn board_close(&mut self, cx: &mut Context<Self>) {
        self.board.open = None;
        self.board.chat_input = None;
        self.board.agent_menu = false;
        cx.notify();
    }

    /// What goes along with a message about a node: the node itself.
    fn node_brief(n: &Node) -> String {
        let mut s = format!("About this board node: \u{201c}{}\u{201d}.\nWhat was asked: {}\n", n.title, n.ask);
        if !n.changes.is_empty() {
            s.push_str(&format!("What changed:\n- {}\n", n.changes.join("\n- ")));
        }
        for r in &n.refs {
            s.push_str(&format!("{}: {}\n", r.kind, r.text));
        }
        s
    }

    fn board_chat_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(n), Some((st, _)), Some((_, chats))) = (self.board_node(), self.board.chat_input.as_ref(), self.core.as_ref()) else { return };
        let text = st.read(cx).value().trim().to_string();
        if text.is_empty() {
            return;
        }
        let chats = chats.clone();
        let st = st.clone();
        let ctx = self.mode_ctx();
        let chat = match n.chat.clone().filter(|c| chats.peek(c).is_some()) {
            Some(c) => Ok((c, text.clone())),
            None => chats.side(&self.board.thread, &format!("About: {}", n.title), SideCtx::None).map(|t| (t.id, format!("{}\n{text}", Self::node_brief(&n)))),
        };
        match chat.and_then(|(cid, msg)| chats.send(&cid, &msg, Vec::new(), None, ctx).map(|_| cid)) {
            Ok(cid) => {
                if let Some(x) = self.board.b.nodes.iter_mut().find(|x| x.id == n.id) {
                    x.chat = Some(cid);
                }
                self.board_save();
                st.update(cx, |s, cx| s.set_value("", window, cx));
                self.send_error = None;
            }
            Err(e) => self.send_error = Some(e.to_string()),
        }
        self.refresh();
        cx.notify();
    }

    fn board_open_stream(&mut self, n: &Node, cx: &mut Context<Self>) {
        self.board_close(cx);
        self.board.view = false;
        self.open_thread(n.thread.clone());
        cx.notify();
    }

    fn board_branch(&mut self, n: &Node, cx: &mut Context<Self>) {
        let Some((_, chats)) = self.core.as_ref() else { return };
        match chats.branch(&n.thread, &n.id) {
            Ok(t) => {
                self.board_close(cx);
                self.board.view = false;
                self.refresh();
                self.open_thread(t.id);
            }
            Err(e) => self.send_error = Some(e.to_string()),
        }
        cx.notify();
    }

    fn board_send_to(&mut self, n: &Node, agent: &str, cx: &mut Context<Self>) {
        let Some((_, chats)) = self.core.as_ref() else { return };
        let Some(a) = super::thread::agents().get(agent) else { return };
        let project = chats.peek(&n.thread).and_then(|t| t.project);
        let t = chats.create_in(a.route.clone(), backspace_core::chat::Scope { project, agent: Some(a.id.clone()), title: Some(n.title.clone()), ..Default::default() });
        let _ = chats.send(&t.id, &format!("{}\nTake it from here.", Self::node_brief(n)), Vec::new(), None, self.mode_ctx());
        self.board_close(cx);
        self.board.view = false;
        self.refresh();
        self.open_thread(t.id);
        cx.notify();
    }

    /// The opened node: a centred split over most of the window.
    #[inline(never)]
    pub(super) fn board_modal_el(&mut self, pal: &'static Palette, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let n = self.board_node()?;
        if self.board.chat_input.is_none() {
            let st = cx.new(|cx| InputState::new(window, cx).placeholder("Ask about this node, or say what to change\u{2026}"));
            let sub = cx.subscribe_in(&st, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.board_chat_send(window, cx);
                }
            });
            let f = st.read(cx).focus_handle(cx);
            window.focus(&f, cx);
            self.board.chat_input = Some((st, sub));
        }
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let vp = window.viewport_size();
        let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
        let (w, h) = ((vw * 0.92).min(1500.), vh * 0.9);
        let (x, y) = ((vw - w) / 2., (vh - h) / 2.);
        let right_w = (w * self.board.split).clamp(300., (w - 320.).max(300.));
        let sec = |title: &'static str| div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(fg3).child(title.to_uppercase());
        // How we got here: the nodes that led to it, each one a click away.
        let mut path = div().id("node-path").flex().flex_wrap().items_center().gap(px(6.));
        for (j, pid) in self.board.b.came_from(&n.id).into_iter().enumerate() {
            let Some(p) = self.board.b.nodes.iter().find(|x| x.id == pid) else { continue };
            let cur = pid == n.id;
            if j > 0 {
                path = path.child(div().text_color(fg3).child("\u{2192}"));
            }
            path = path.child(
                div()
                    .id(("node-path-step", j))
                    .px(px(8.))
                    .py(px(3.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(if cur { c(pal.accent) } else { c(pal.line) })
                    .when(cur, |d| d.bg(c(pal.accent_soft)))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(if cur { c(pal.accent) } else { fg2 })
                    .aria_label(p.title.clone())
                    .child(p.title.clone())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.board.open = Some(pid.clone());
                        this.board.chat_input = None;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        let mut left = div()
            .id("node-left")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .px(px(26.))
            .py(px(22.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(12.))
                    .child(div().id("node-title").flex_1().text_size(px(22.)).line_height(px(28.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).aria_label(n.title.clone()).child(n.title.clone()).test_support())
                    .child(
                        div()
                            .id("node-close")
                            .size(px(30.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(radius::CONTROL))
                            .bg(c(pal.inset))
                            .cursor_pointer()
                            .aria_label("Close")
                            .child(glyph("x", 15., fg2))
                            .on_click(cx.listener(|this, _, _, cx| this.board_close(cx)))
                            .test_support(),
                    ),
            )
            .child(path)
            .child(div().flex().flex_col().gap(px(6.)).child(sec("What was asked")).child(div().id("node-ask").p(px(10.)).rounded(px(9.)).bg(c(pal.inset)).text_size(px(13.)).line_height(px(19.)).text_color(fg2).aria_label(n.ask.clone()).child(n.ask.clone()).test_support()));
        if !n.images.is_empty() {
            let mut ex = div().flex().flex_col().gap(px(8.));
            for p in &n.images {
                ex = ex.child(img(std::path::PathBuf::from(p)).max_w_full().max_h(px(h * 0.6)).rounded(px(10.)).object_fit(ObjectFit::Contain));
            }
            left = left.child(div().flex().flex_col().gap(px(6.)).child(sec("Examples")).child(ex));
        }
        if !n.changes.is_empty() {
            let mut ul = div().flex().flex_col().gap(px(4.));
            for ch in &n.changes {
                ul = ul.child(div().flex().gap(px(8.)).text_size(px(13.)).line_height(px(19.)).text_color(fg2).child("\u{2022}").child(div().min_w_0().child(ch.clone())));
            }
            left = left.child(div().flex().flex_col().gap(px(6.)).child(sec("What changed")).child(ul));
        }
        let mut refs = div().flex().flex_wrap().gap(px(8.));
        let mut all = n.refs.clone();
        if let Some(v) = &n.via {
            all.push(backspace_core::whiteboard::Ref { kind: "Side stream".into(), text: v.clone() });
        }
        for (j, r) in all.iter().enumerate() {
            refs = refs.child(div().id(("node-ref", j)).max_w(px(320.)).flex().items_center().gap(px(8.)).px(px(10.)).py(px(7.)).rounded(px(9.)).border_1().border_color(c(pal.line)).bg(c(pal.inset)).aria_label(format!("{}: {}", r.kind, r.text)).child(div().flex_none().text_size(px(10.)).font_weight(FontWeight::SEMIBOLD).text_color(fg3).child(r.kind.to_uppercase())).child(div().min_w_0().truncate().text_size(px(12.5)).text_color(fg2).child(r.text.clone())).test_support());
        }
        left = left.child(div().flex().flex_col().gap(px(6.)).child(sec("References")).child(refs));

        // The chat side: the three ways on, then the talk about this node, then the box.
        let (n1, n2) = (n.clone(), n.clone());
        let act = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .flex_1()
                .h(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_size(px(12.5))
                .when(primary, |d| d.bg(c(pal.accent)).text_color(c(pal.on_accent)).font_weight(FontWeight::SEMIBOLD))
                .when(!primary, |d| d.border_1().border_color(c(pal.line_strong)).text_color(fg).hover(|d| d.bg(c(pal.hover))))
                .aria_label(label)
                .child(label)
        };
        let mut acts = div()
            .relative()
            .flex()
            .gap(px(6.))
            .p(px(12.))
            .border_b_1()
            .border_color(c(pal.line))
            .child(act("node-open", "Open stream", true).on_click(cx.listener(move |this, _, _, cx| this.board_open_stream(&n1, cx))).test_support())
            .child(act("node-branch", "Branch from here", false).on_click(cx.listener(move |this, _, _, cx| this.board_branch(&n2, cx))).test_support())
            .child(act("node-send", "Send to an agent", false).on_click(cx.listener(|this, _, _, cx| {
                this.board.agent_menu = !this.board.agent_menu;
                cx.notify();
            })).test_support());
        if self.board.agent_menu {
            let mut menu = div().id("node-agents").absolute().top(px(48.)).right(px(12.)).w(px(220.)).p(px(4.)).flex().flex_col().rounded(px(radius::PANEL)).bg(c(pal.raised)).border_1().border_color(c(pal.line)).shadow(super::lift(super::Lift::Button, pal));
            let agents = super::thread::agents().list();
            if agents.is_empty() {
                menu = menu.child(div().p(px(8.)).text_size(px(12.)).text_color(fg3).child("No agents yet. Make one with Create crew."));
            }
            for (j, a) in agents.into_iter().take(10).enumerate() {
                let (nn, aid) = (n.clone(), a.id.clone());
                menu = menu.child(div().id(("node-agent", j)).h(px(28.)).px(px(8.)).flex().items_center().rounded(px(radius::CONTROL)).cursor_pointer().hover(|d| d.bg(c(pal.hover))).text_size(px(12.5)).text_color(fg).aria_label(a.name.clone()).child(a.name.clone()).on_click(cx.listener(move |this, _, _, cx| this.board_send_to(&nn, &aid, cx))).test_support());
            }
            acts = acts.child(deferred(menu).with_priority(8));
        }
        let mut chat = div().id("node-chat").flex_1().min_h_0().overflow_y_scroll().p(px(14.)).flex().flex_col().gap(px(10.));
        let msgs = n.chat.as_ref().and_then(|cid| self.core.as_ref().and_then(|(_, ch)| ch.peek(cid))).map(|t| t.messages).unwrap_or_default();
        if msgs.is_empty() {
            chat = chat.child(div().text_size(px(12.5)).text_color(fg3).child("No talk about this node yet. What you write goes to a side stream of this one, with the node as its context."));
        }
        for (j, m) in msgs.iter().enumerate() {
            let me = m.role == Role::User;
            let shown = if me && j == 0 { m.text.lines().last().unwrap_or("").to_string() } else { m.text.clone() };
            let shown = if shown.trim().is_empty() && !me { "\u{2026}".to_string() } else { super::plan::strip(&shown) };
            chat = chat.child(
                div()
                    .id(("node-msg", j))
                    .max_w(relative(0.88))
                    .when(me, |d| d.self_end().bg(c(pal.accent_soft)))
                    .when(!me, |d| d.self_start().bg(c(pal.raised)).border_1().border_color(c(pal.line)))
                    .px(px(11.))
                    .py(px(8.))
                    .rounded(px(11.))
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(if me { fg } else { fg2 })
                    .child(div().text_size(px(10.5)).text_color(fg3).child(if me { "You" } else { "Reply" }))
                    .child(shown)
                    .test_support(),
            );
        }
        let input = self.board.chat_input.as_ref().map(|(s, _)| s.clone()).expect("made above");
        let compose = div()
            .id("node-compose")
            .m(px(12.))
            .flex()
            .items_end()
            .gap(px(8.))
            .pl(px(12.))
            .pr(px(6.))
            .py(px(6.))
            .rounded(px(12.))
            .border_1()
            .border_color(c(pal.line_strong))
            .bg(c(pal.raised))
            .child(div().flex_1().min_w_0().child(Input::new(&input).appearance(false).text_size(px(13.)).p_0()))
            .child(div().id("node-send-msg").size(px(28.)).flex().items_center().justify_center().rounded(px(8.)).bg(c(pal.accent)).cursor_pointer().aria_label("Send").child(glyph("arrow-up", 14., c(pal.on_accent))).on_click(cx.listener(|this, _, window, cx| this.board_chat_send(window, cx))).test_support());
        let right = div().id("node-right").w(px(right_w)).flex_none().h_full().flex().flex_col().bg(c(pal.card)).child(acts).child(chat).children(self.send_error.clone().map(|e| div().px(px(14.)).text_size(px(12.)).text_color(c(pal.bad)).child(e))).child(compose);
        let total = w;
        let splitter = div()
            .id("node-split")
            .w(px(7.))
            .flex_none()
            .h_full()
            .cursor(CursorStyle::ResizeLeftRight)
            .bg(c(pal.inset))
            .border_l_1()
            .border_r_1()
            .border_color(c(pal.line))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.board.drag = Some(Drag::Split { sx: f32::from(ev.position.x), w0: right_w, total });
            }))
            .test_support();
        let modal = div()
            .id("node-modal")
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(w))
            .h(px(h))
            .flex()
            .rounded(px(16.))
            .overflow_hidden()
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line_strong))
            .shadow(super::lift(super::Lift::Button, pal))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(left)
            .child(splitter)
            .child(right)
            .test_support();
        let scrim = div()
            .id("node-scrim")
            .absolute()
            .left_0()
            .top_0()
            .w(px(vw))
            .h(px(vh))
            .bg(gpui::rgba(0x0c0d146b))
            .occlude()
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if matches!(this.board.drag, Some(Drag::Split { .. })) {
                    this.board_move(ev, cx);
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                if matches!(this.board.drag, Some(Drag::Split { .. })) {
                    this.board.drag = None;
                    cx.notify();
                }
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.board_close(cx)))
            .child(modal)
            .test_support();
        Some(deferred(scrim).with_priority(6).into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::super::thread::tests::{data_replying, open_data};
    use backspace_core::chat::Role;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};

    /// t1's reply shipped a commit; t3 is a side stream of t1.
    fn shipped_data() -> std::path::PathBuf {
        let d = data_replying("Fixed the stream rows.\n\n- Folder and branch under the name\n- Pills kept beside the mark");
        let f = d.join("threads").join("t1.json");
        let mut t: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        t["messages"][1]["acts"] = serde_json::json!([{ "name": "Bash", "input": "git commit -m rows", "output": "[side-toggle de0b59f] rows", "start": 1, "end": 2 }]);
        std::fs::write(&f, t.to_string()).unwrap();
        let mut side = t.clone();
        side["id"] = "t3".into();
        side["title"] = "Check the rows".into();
        side["side_of"] = "t1".into();
        std::fs::write(d.join("threads").join("t3.json"), side.to_string()).unwrap();
        d
    }

    /// A coordinator has Chat | Board; the reply that shipped a commit is a node; clicking it opens
    /// the split, and writing there starts a side chat about the node.
    #[gpui_kit::test]
    fn a_coordinator_board_shows_shipped_replies_and_opens_a_node(cx: &mut TestAppContext) {
        let (handle, chats) = open_data(cx, shipped_data());
        let open = |cx: &mut TestAppContext, id: &str| {
            handle.update(cx, |c, _, cx| {
                c.open_thread(id.to_string());
                cx.notify();
            })
            .unwrap()
        };
        open(cx, "t3");
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("view-switch").is_none(), "a side stream has no board");
        })
        .unwrap();
        open(cx, "t1");
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("view-switch").is_some(), "a coordinator has Chat | Board");
            window.click("view-board", cx);
            window.render_frame(cx);
            window.render_frame(cx);
            assert!(window.try_find(("board-node", 0usize)).is_some(), "the commit made a node");
            window.click(("board-node", 0usize), cx);
            window.render_frame(cx);
            assert!(window.try_find("node-modal").is_some(), "the node opens in the split");
            assert!(window.try_find("node-open").is_some() && window.try_find("node-branch").is_some() && window.try_find("node-send").is_some());
        })
        .unwrap();
        handle
            .update(cx, |c, window, cx| {
                let (st, _) = c.board.chat_input.as_ref().map(|(s, _)| s.clone()).map(|s| (s, ())).expect("the chat box");
                st.update(cx, |s, cx| s.set_value("Why the pill?", window, cx));
                c.board_chat_send(window, cx);
            })
            .unwrap();
        let chat = handle.update(cx, |c, _, _| c.board.b.nodes[0].chat.clone()).unwrap().expect("a side chat");
        let t = chats.peek(&chat).unwrap();
        assert_eq!(t.side_of.as_deref(), Some("t1"));
        let sent = t.messages.iter().find(|m| m.role == Role::User).unwrap();
        assert!(sent.text.contains("de0b59f") && sent.text.ends_with("Why the pill?"), "{}", sent.text);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find(("node-msg", 0usize)).is_some(), "the chat shows in the right pane");
        })
        .unwrap();
    }

    /// A reply that shipped nothing is no node until you pin it from its ⋯ menu.
    #[gpui_kit::test]
    fn pinning_a_reply_puts_it_on_the_board(cx: &mut TestAppContext) {
        let (handle, _) = open_data(cx, data_replying("We should keep the pills."));
        handle
            .update(cx, |c, _, cx| {
                c.open_thread("t1".into());
                c.board_sync(&c.thread.clone().unwrap());
                assert!(c.board.b.nodes.is_empty(), "a plain reply is no node");
                c.board_pin("m2", cx);
                assert_eq!(c.board.b.nodes.len(), 1);
                assert!(c.board.b.nodes[0].pinned);
            })
            .unwrap();
    }
}
