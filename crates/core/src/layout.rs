//! How a tab arranges its canvases: a binary tree of splits whose leaves
//! index the tab's panes. Both shells draw from these rectangles and apply
//! drags with these edits, so a layout behaves the same in GPUI and Tauri.
//!
//! JSON: `{"leaf": 0}` or `{"dir": "row", "ratio": 0.5, "a": ..., "b": ...}`
//! (`row` puts `a` left of `b`, `col` puts it above).
//!
//! A node can also be a **stack**: `{"stack": [0, 3, 1], "active": 1}` — the
//! same leaves sharing one rectangle, like cards on a pile. They are in front
//! order (last on top) and `active` is the one that takes the pointer. A stack
//! is what a drop *on* a canvas means when it is not a split: you asked for the
//! same spot, so the two share it rather than swapping or halving.

use serde::{Deserialize, Serialize};

/// The leaf id of the drop placeholder in a drag preview.
pub const PH: usize = usize::MAX;

/// How many cards one stack may hold. Past this the edges are too thin to name.
pub const MAX_STACK: usize = 4;
/// The visible strip of a card behind the front one, in the shell's own units
/// (CSS pixels for the Tauri shell, logical for GPUI). A stack's shared
/// rectangle is inset by `strip * (len - 1)` so the front card still fills what
/// is left.
pub const STACK_STRIP: f32 = 12.;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    Row,
    Col,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum Node {
    Leaf {
        leaf: usize,
    },
    Split {
        dir: Dir,
        ratio: f32,
        a: Box<Node>,
        b: Box<Node>,
    },
    /// Leaves sharing one rectangle: `stack[0]` at the back, the last in front.
    Stack {
        stack: Vec<usize>,
        /// Which member the pointer is on, an index into `stack`.
        #[serde(default)]
        active: usize,
    },
}

/// Where a dragged canvas lands relative to the one under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
    /// Swap places.
    Center,
    /// Share the spot: a stack. Only offered when the pane under the pointer
    /// can take another card.
    Stack,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// A draggable gap: `path` addresses its split (false = a, true = b from the
/// root), `span` is the split's whole rectangle, for turning a pointer
/// position back into a ratio.
#[derive(Clone, Debug, PartialEq)]
pub struct Gutter {
    pub path: Vec<bool>,
    pub dir: Dir,
    pub rect: Rect,
    pub span: Rect,
}

/// Layout presets offered when making a tab: (id, label, canvases).
pub const PRESETS: [(&str, &str, usize); 7] = [
    ("1", "1", 1),
    ("2", "2", 2),
    ("3", "3", 3),
    ("4", "2×2", 4),
    ("top1", "1 over 2", 3),
    ("bottom1", "2 over 1", 3),
    ("left1", "1 | 2", 3),
];

impl Node {
    pub fn leaf(i: usize) -> Node {
        Node::Leaf { leaf: i }
    }

    pub fn split(dir: Dir, ratio: f32, a: Node, b: Node) -> Node {
        Node::Split {
            dir,
            ratio,
            a: Box::new(a),
            b: Box::new(b),
        }
    }

    /// The layout a tab gets for `n` canvases when it has none saved.
    pub fn default_for(n: usize) -> Node {
        let l = Node::leaf;
        match n {
            0 | 1 => l(0),
            2 => Node::split(Dir::Row, 0.5, l(0), l(1)),
            3 => Node::split(
                Dir::Row,
                1. / 3.,
                l(0),
                Node::split(Dir::Row, 0.5, l(1), l(2)),
            ),
            _ => Node::split(
                Dir::Col,
                0.56,
                Node::split(Dir::Row, 0.5, l(0), l(1)),
                Node::split(Dir::Row, 0.5, l(2), l(3)),
            ),
        }
    }

    pub fn preset(id: &str) -> Node {
        let l = Node::leaf;
        match id {
            "top1" => Node::split(Dir::Col, 0.5, l(0), Node::split(Dir::Row, 0.5, l(1), l(2))),
            "bottom1" => Node::split(Dir::Col, 0.5, Node::split(Dir::Row, 0.5, l(0), l(1)), l(2)),
            "left1" => Node::split(Dir::Row, 0.5, l(0), Node::split(Dir::Col, 0.5, l(1), l(2))),
            n => Node::default_for(n.parse().unwrap_or(1)),
        }
    }

    /// A stack, or the plain pane when there is nothing to stack. Kept as one
    /// constructor so a stack never has a single card: a one-card stack and a
    /// leaf would draw differently for no reason.
    pub fn stack(leaves: Vec<usize>, active: usize) -> Node {
        match leaves.len() {
            0 => Node::leaf(0),
            1 => Node::leaf(leaves[0]),
            n => Node::Stack { stack: leaves, active: active.min(n - 1) },
        }
    }

    pub fn leaves(&self) -> Vec<usize> {
        match self {
            Node::Leaf { leaf } => vec![*leaf],
            Node::Split { a, b, .. } => {
                let mut v = a.leaves();
                v.extend(b.leaves());
                v
            }
            // Every card is still a canvas in the tab; the order they are listed
            // in does not matter here, only that each appears once.
            Node::Stack { stack, .. } => stack.clone(),
        }
    }

    /// The stack holding `leaf`, or None if it is not stacked. The members come
    /// back back-to-front, so the last one is the one in front.
    pub fn stack_of(&self, leaf: usize) -> Option<(Vec<usize>, usize)> {
        match self {
            Node::Stack { stack, active } => {
                stack.iter().position(|l| *l == leaf).map(|_| (stack.clone(), *active))
            }
            Node::Split { a, b, .. } => a.stack_of(leaf).or_else(|| b.stack_of(leaf)),
            Node::Leaf { .. } => None,
        }
    }

    pub fn is_stacked(&self, leaf: usize) -> bool {
        match self {
            Node::Leaf { .. } => false,
            Node::Stack { stack, .. } => stack.contains(&leaf),
            Node::Split { a, b, .. } => a.is_stacked(leaf) || b.is_stacked(leaf),
        }
    }

    /// Bring `leaf` to the front of the stack holding it. The one that was in
    /// front stays in the stack, underneath.
    pub fn raise(&self, leaf: usize) -> Node {
        match self {
            Node::Leaf { .. } => self.clone(),
            Node::Stack { stack, active } => {
                let Some(pos) = stack.iter().position(|l| *l == leaf) else {
                    return self.clone();
                };
                let mut s = stack.clone();
                let l = s.remove(pos);
                s.push(l);
                let n = s.len();
                Node::Stack { stack: s, active: (*active).min(n.saturating_sub(1)) }
            }
            Node::Split { dir, ratio, a, b } => {
                Node::split(*dir, *ratio, a.raise(leaf), b.raise(leaf))
            }
        }
    }

    /// Put `pane` on top of the stack holding `target`, taking `pane` out of the
    /// tree first when it is already somewhere in it. This is the only way to add
    /// a card, so the two steps cannot get out of order and a card appears once.
    pub fn stack_with(&self, target: usize, pane: usize) -> Node {
        if self.leaves().contains(&pane) {
            match self.remove(pane) {
                Some(rest) => rest.push_onto(target, pane),
                None => self.clone(),
            }
        } else {
            self.push_onto(target, pane)
        }
    }

    /// Add `new` to the stack holding `target`, assuming `new` is not in the tree
    /// yet. `stack_with` is the one to call from outside; the tests use this
    /// directly to build a stack without a drag.
    fn push_onto(&self, target: usize, new: usize) -> Node {
        match self {
            Node::Leaf { leaf } if *leaf == target => Node::stack(vec![target, new], 1),
            Node::Leaf { .. } => self.clone(),
            Node::Stack { stack, .. } => {
                if !stack.contains(&target) || stack.contains(&new) {
                    return self.clone();
                }
                let mut s = stack.clone();
                let spilling = if s.len() >= MAX_STACK {
                    // Full: the oldest card leaves the stack. It does not
                    // disappear -- it becomes a pane beside the stack, which is
                    // where a card goes when it no longer fits.
                    Some(s.remove(0))
                } else {
                    None
                };
                s.push(new);
                let st = Node::stack(s.clone(), s.len() - 1);
                match spilling {
                    Some(old) => Node::split(Dir::Col, 0.75, Node::leaf(old), st),
                    None => st,
                }
            }
            Node::Split { dir, ratio, a, b } => {
                Node::split(*dir, *ratio, a.push_onto(target, new), b.push_onto(target, new))
            }
        }
    }

    /// Taking `leaf` out of its stack, back to a plain pane. The stack it left
    /// shrinks; a stack of one is just that pane.
    pub fn unstack(&self, leaf: usize) -> Node {
        match self {
            Node::Leaf { .. } => self.clone(),
            Node::Stack { stack, active } => {
                let Some(pos) = stack.iter().position(|l| *l == leaf) else {
                    return self.clone();
                };
                let mut s = stack.clone();
                s.remove(pos);
                let a = if *active > pos { active - 1 } else { *active };
                Node::stack(s, a)
            }
            Node::Split { dir, ratio, a, b } => {
                Node::split(*dir, *ratio, a.unstack(leaf), b.unstack(leaf))
            }
        }
    }

    /// Exactly the leaves 0..n, each once.
    pub fn valid_for(&self, n: usize) -> bool {
        let mut l = self.leaves();
        l.sort_unstable();
        l == (0..n).collect::<Vec<_>>()
    }

    /// Rectangles for every leaf and gutter inside `r`, `gap` apart. A stack
    /// lays its back cards out as strips across the top of the shared
    /// rectangle and hands the front card what is left, so the front canvas
    /// stays whole and the cards behind only ever show an edge.
    pub fn layout(&self, r: Rect, gap: f32) -> (Vec<(usize, Rect)>, Vec<Gutter>) {
        let mut leaves = vec![];
        let mut gutters = vec![];
        self.place(r, gap, &mut vec![], &mut leaves, &mut gutters);
        (leaves, gutters)
    }

    fn place(
        &self,
        r: Rect,
        gap: f32,
        path: &mut Vec<bool>,
        leaves: &mut Vec<(usize, Rect)>,
        gutters: &mut Vec<Gutter>,
    ) {
        match self {
            Node::Leaf { leaf } => leaves.push((*leaf, r)),
            Node::Stack { stack, active } => {
                let n = stack.len();
                if n == 0 {
                    return;
                }
                let front = (*active).min(n - 1);
                let strips = (n - 1) as f32;
                let mut y = r.y;
                for (i, leaf) in stack.iter().enumerate() {
                    if i == front {
                        continue;
                    }
                    leaves.push((
                        *leaf,
                        Rect {
                            x: r.x,
                            y,
                            w: (r.w - strips * 6.).max(0.),
                            h: STACK_STRIP,
                        },
                    ));
                    y += STACK_STRIP;
                }
                leaves.push((
                    stack[front],
                    Rect {
                        x: r.x,
                        y,
                        w: r.w,
                        h: (r.h - strips * STACK_STRIP).max(0.),
                    },
                ));
            }
            Node::Split { dir, ratio, a, b } => {
                let (ra, g, rb) = match dir {
                    Dir::Row => {
                        let wa = ((r.w - gap) * ratio).max(0.);
                        (
                            Rect { w: wa, ..r },
                            Rect {
                                x: r.x + wa,
                                w: gap,
                                ..r
                            },
                            Rect {
                                x: r.x + wa + gap,
                                w: (r.w - gap - wa).max(0.),
                                ..r
                            },
                        )
                    }
                    Dir::Col => {
                        let ha = ((r.h - gap) * ratio).max(0.);
                        (
                            Rect { h: ha, ..r },
                            Rect {
                                y: r.y + ha,
                                h: gap,
                                ..r
                            },
                            Rect {
                                y: r.y + ha + gap,
                                h: (r.h - gap - ha).max(0.),
                                ..r
                            },
                        )
                    }
                };
                gutters.push(Gutter {
                    path: path.clone(),
                    dir: *dir,
                    rect: g,
                    span: r,
                });
                path.push(false);
                a.place(ra, gap, path, leaves, gutters);
                path.pop();
                path.push(true);
                b.place(rb, gap, path, leaves, gutters);
                path.pop();
            }
        }
    }

    pub fn set_ratio(&mut self, path: &[bool], value: f32) {
        match (self, path.split_first()) {
            (Node::Split { ratio, .. }, None) => *ratio = value.clamp(0.1, 0.9),
            (Node::Split { a, b, .. }, Some((side, rest))) => {
                if *side { b } else { a }.set_ratio(rest, value)
            }
            _ => {}
        }
    }

    /// The tree without `leaf`; its sibling takes the parent's place, and a
    /// card leaves its stack. None when `leaf` was the whole tree.
    pub fn remove(&self, leaf: usize) -> Option<Node> {
        match self {
            Node::Leaf { leaf: l } => (*l != leaf).then(|| self.clone()),
            Node::Split { dir, ratio, a, b } => match (a.remove(leaf), b.remove(leaf)) {
                (Some(a), Some(b)) => Some(Node::split(*dir, *ratio, a, b)),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            },
            Node::Stack { stack, active } => {
                let Some(pos) = stack.iter().position(|l| *l == leaf) else {
                    return Some(self.clone());
                };
                let mut s = stack.clone();
                s.remove(pos);
                if s.is_empty() {
                    return None;
                }
                let a = if *active > pos { active - 1 } else { *active };
                Some(Node::stack(s, a))
            }
        }
    }

    /// Split `target` and put `new` on `side` of it. `Center` shares the spot
    /// when the pane is already a stack, and otherwise replaces it as before.
    /// `Stack` always stacks, and never silently loses the old pane.
    pub fn insert(&self, target: usize, side: Side, new: usize) -> Node {
        match self {
            Node::Leaf { leaf } if *leaf == target => {
                let (t, n) = (Node::leaf(target), Node::leaf(new));
                match side {
                    Side::Left => Node::split(Dir::Row, 0.5, n, t),
                    Side::Right => Node::split(Dir::Row, 0.5, t, n),
                    Side::Top => Node::split(Dir::Col, 0.5, n, t),
                    Side::Bottom => Node::split(Dir::Col, 0.5, t, n),
                    // Nothing beside it, so sharing the spot is the only reading
                    // that keeps both.
                    Side::Center | Side::Stack => Node::stack(vec![target, new], 1),
                }
            }
            Node::Leaf { .. } => self.clone(),
            Node::Stack { stack, .. } => {
                if !stack.contains(&target) {
                    return self.clone();
                }
                match side {
                    // Dropped on a card that is already here: show it, or show
                    // the new one. Nothing moves.
                    Side::Center => {
                        let mut s = stack.clone();
                        match s.iter().position(|l| *l == new) {
                            Some(p) => {
                                let l = s.remove(p);
                                s.push(l);
                            }
                            None if s.len() >= MAX_STACK => {
                                let pos = s.iter().position(|l| *l == target).unwrap();
                                s[pos] = new;
                            }
                            None => s.push(new),
                        }
                        Node::stack(s.clone(), s.len() - 1)
                    }
                    // `insert` is for splitting; adding a card goes through
                    // `stack_with`, which first takes the pane out of the tree.
                    Side::Stack => self.clone(),
                    // Pulled out of the stack and split the stack's rectangle
                    // with it, so the stack keeps its spot and the card leaves.
                    _ => match self.remove(target) {
                        Some(rest) => rest.insert(target, side, new),
                        None => self.clone(),
                    },
                }
            }
            Node::Split { dir, ratio, a, b } => Node::split(
                *dir,
                *ratio,
                a.insert(target, side, new),
                b.insert(target, side, new),
            ),
        }
    }

    /// Rename leaf `from` to `to` (other leaves untouched).
    pub fn rename(&self, from: usize, to: usize) -> Node {
        match self {
            Node::Leaf { leaf } if *leaf == from => Node::leaf(to),
            Node::Leaf { .. } => self.clone(),
            Node::Stack { stack, active } => {
                let s: Vec<usize> = stack.iter().map(|l| if *l == from { to } else { *l }).collect();
                Node::Stack { stack: s, active: *active }
            }
            Node::Split { dir, ratio, a, b } => {
                Node::split(*dir, *ratio, a.rename(from, to), b.rename(from, to))
            }
        }
    }

    pub fn swap(&self, x: usize, y: usize) -> Node {
        match self {
            Node::Leaf { leaf } if *leaf == x => Node::leaf(y),
            Node::Leaf { leaf } if *leaf == y => Node::leaf(x),
            Node::Leaf { .. } => self.clone(),
            Node::Stack { stack, active } => {
                let s: Vec<usize> = stack
                    .iter()
                    .map(|l| if *l == x { y } else if *l == y { x } else { *l })
                    .collect();
                Node::Stack { stack: s, active: *active }
            }
            Node::Split { dir, ratio, a, b } => {
                Node::split(*dir, *ratio, a.swap(x, y), b.swap(x, y))
            }
        }
    }

    /// After pane `removed` is deleted, close the gap in the numbering.
    pub fn renumber(&self, removed: usize) -> Node {
        match self {
            Node::Leaf { leaf } if *leaf > removed && *leaf != PH => Node::leaf(leaf - 1),
            Node::Leaf { .. } => self.clone(),
            Node::Stack { stack, active } => {
                let s: Vec<usize> = stack
                    .iter()
                    .map(|l| if *l > removed && *l != PH { l - 1 } else { *l })
                    .collect();
                Node::Stack { stack: s, active: *active }
            }
            Node::Split { dir, ratio, a, b } => {
                Node::split(*dir, *ratio, a.renumber(removed), b.renumber(removed))
            }
        }
    }

    /// The tree after dropping `dragged` on `side` of `target`, with the
    /// dropped canvas shown as `shown` (PH while previewing, `dragged` to
    /// apply).
    pub fn moved(&self, dragged: usize, target: usize, side: Side, shown: usize) -> Node {
        if dragged == target {
            return self.rename(dragged, shown);
        }
        match side {
            Side::Center => self.swap(dragged, target).rename(dragged, shown),
            Side::Stack => self.stack_with(target, shown),
            _ => match self.remove(dragged) {
                Some(rest) => rest.insert(target, side, shown),
                None => self.rename(dragged, shown),
            },
        }
    }

    /// Split `target` along its longer side for a new canvas `new`.
    pub fn add_beside(&self, target: usize, rect: Rect, new: usize) -> Node {
        let side = if rect.w >= rect.h {
            Side::Right
        } else {
            Side::Bottom
        };
        self.insert(target, side, new)
    }
}

/// Which part of a canvas the pointer is over: the outer quarter on each
/// side splits there, the middle swaps.
pub fn zone(r: Rect, x: f32, y: f32) -> Side {
    zone_maybe_stack(r, x, y, true)
}

/// As `zone`, but `can_stack` says whether the pane under the pointer already
/// holds as many cards as it may; a full stack falls back to swapping.
pub fn zone_maybe_stack(r: Rect, x: f32, y: f32, can_stack: bool) -> Side {
    let dx = (x - r.x) / r.w.max(1.);
    let dy = (y - r.y) / r.h.max(1.);
    let edges = [
        (dx, Side::Left),
        (1. - dx, Side::Right),
        (dy, Side::Top),
        (1. - dy, Side::Bottom),
    ];
    let (d, side) = edges
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap();
    if d < 0.25 {
        side
    } else if can_stack {
        Side::Stack
    } else {
        Side::Center
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = Rect {
        x: 0.,
        y: 0.,
        w: 1000.,
        h: 600.,
    };

    #[test]
    fn defaults_and_presets_cover_every_pane() {
        for n in 1..=4 {
            assert!(Node::default_for(n).valid_for(n));
        }
        for (id, _, n) in PRESETS {
            assert!(Node::preset(id).valid_for(n), "{id}");
        }
    }

    #[test]
    fn one_over_two_is_a_long_top_and_two_squares() {
        let (leaves, gutters) = Node::preset("top1").layout(R, 6.);
        assert_eq!(
            leaves[0],
            (
                0,
                Rect {
                    x: 0.,
                    y: 0.,
                    w: 1000.,
                    h: 297.
                }
            )
        );
        assert_eq!(
            leaves[1].1,
            Rect {
                x: 0.,
                y: 303.,
                w: 497.,
                h: 297.
            }
        );
        assert_eq!(
            leaves[2].1,
            Rect {
                x: 503.,
                y: 303.,
                w: 497.,
                h: 297.
            }
        );
        assert_eq!(gutters.len(), 2);
    }

    #[test]
    fn drag_to_an_edge_splits_there_and_the_old_spot_closes() {
        // 2x2; drag canvas 3 to the top of canvas 0.
        let t = Node::default_for(4);
        let after = t.moved(3, 0, Side::Top, 3);
        assert!(after.valid_for(4));
        let (leaves, _) = after.layout(R, 0.);
        let r = |i| leaves.iter().find(|(l, _)| *l == i).unwrap().1;
        assert!(r(3).y < r(0).y && r(3).x == r(0).x);
        // Canvas 2 lost its neighbour and now spans the bottom row.
        assert_eq!(r(2).w, 1000.);
        // The preview is the same shape with a placeholder.
        assert_eq!(t.moved(3, 0, Side::Top, PH), after.rename(3, PH));
    }

    #[test]
    fn centre_swaps_and_self_is_a_no_op() {
        let t = Node::default_for(3);
        assert_eq!(t.moved(0, 2, Side::Center, 0).leaves(), vec![2, 1, 0]);
        assert_eq!(t.moved(1, 1, Side::Left, 1), t);
    }

    #[test]
    fn remove_and_renumber_keep_it_valid() {
        let t = Node::preset("left1");
        let after = t.remove(0).unwrap().renumber(0);
        assert!(after.valid_for(2));
        assert_eq!(Node::leaf(0).remove(0), None);
    }

    #[test]
    fn zones() {
        let r = Rect {
            x: 0.,
            y: 0.,
            w: 100.,
            h: 100.,
        };
        assert_eq!(zone(r, 50., 50.), Side::Stack);
        assert_eq!(zone_maybe_stack(r, 50., 50., false), Side::Center);
        assert_eq!(zone(r, 5., 50.), Side::Left);
        assert_eq!(zone(r, 50., 95.), Side::Bottom);
    }

    #[test]
    fn json_shape() {
        let t = Node::preset("top1");
        let s = serde_json::to_string(&t).unwrap();
        assert!(
            s.starts_with(r#"{"dir":"col","ratio":0.5,"a":{"leaf":0}"#),
            "{s}"
        );
        assert_eq!(serde_json::from_str::<Node>(&s).unwrap(), t);
    }
    fn names(t: &Node) -> Vec<Vec<usize>> {
        // Every stack in the tree, back to front.
        let mut out = vec![];
        fn walk(n: &Node, out: &mut Vec<Vec<usize>>) {
            match n {
                Node::Stack { stack, .. } => out.push(stack.clone()),
                Node::Split { a, b, .. } => {
                    walk(a, out);
                    walk(b, out);
                }
                Node::Leaf { .. } => {}
            }
        }
        walk(t, &mut out);
        out
    }

    #[test]
    fn stacking_two_keeps_both_and_raising_swaps_which_is_in_front() {
        let t = Node::default_for(2);
        assert!(!t.is_stacked(0));
        // Dropping 1 onto 0 makes a stack of both, 1 in front.
        let after = t.moved(1, 0, Side::Stack, 1);
        assert!(after.valid_for(2), "no canvas is lost");
        assert!(after.is_stacked(0) && after.is_stacked(1));
        assert_eq!(after.stack_of(0), Some((vec![0, 1], 1)));
        assert_eq!(names(&after), vec![vec![0, 1]]);
        // The front card fills the space; the one behind is a strip.
        let (leaves, gutters) = after.layout(R, 0.);
        assert!(gutters.is_empty(), "a stack is not a split");
        let r = |i| leaves.iter().find(|(l, _)| *l == i).unwrap().1;
        assert_eq!(r(0).y, 0., "the card behind shows its edge at the top");
        assert_eq!(r(0).h, STACK_STRIP, "just an edge");
        assert_eq!(r(1).y, STACK_STRIP, "the front card starts below it");
        assert_eq!(r(1).h, 600. - STACK_STRIP, "and takes everything that is left");
        // Raising the back card brings it forward and pushes the other back.
        let raised = after.raise(0);
        assert_eq!(raised.stack_of(0), Some((vec![1, 0], 1)));
        let (leaves, _) = raised.layout(R, 0.);
        let r = |i| leaves.iter().find(|(l, _)| *l == i).unwrap().1;
        assert_eq!(r(1).h, STACK_STRIP, "the card that was in front is now an edge");
        assert_eq!(r(0).h, 600. - STACK_STRIP, "and this one has the room");
    }

    #[test]
    fn dropping_on_a_cards_middle_stacks_instead_of_swapping() {
        let t = Node::default_for(2);
        assert_eq!(zone(R, 500., 300.), Side::Stack);
        assert_eq!(t.moved(1, 0, Side::Stack, 1).stack_of(0), Some((vec![0, 1], 1)));
        // A third on the same spot is a third card.
        let three = t.moved(1, 0, Side::Stack, 1).moved(2, 0, Side::Stack, 2);
        assert!(three.valid_for(3));
        assert_eq!(three.stack_of(2), Some((vec![0, 1, 2], 2)));
        let (leaves, _) = three.layout(R, 0.);
        assert_eq!(leaves.len(), 3);
        let r = |i| leaves.iter().find(|(l, _)| *l == i).unwrap().1;
        assert_eq!(r(2).h, 600. - 2. * STACK_STRIP);
        assert_eq!((r(1).h, r(0).h), (STACK_STRIP, STACK_STRIP));
        // A pane that cannot take another card swaps instead.
        assert_eq!(zone_maybe_stack(R, 500., 300., false), Side::Center);
    }

    #[test]
    fn a_full_stack_never_loses_a_canvas() {
        // Card 0 is a pane of its own; the stack takes 1..MAX_STACK.
        let mut st = Node::stack(vec![1, 2], 1);
        for i in 3..=MAX_STACK {
            st = st.push_onto(1, i);
        }
        let t = Node::split(Dir::Row, 0.5, Node::leaf(0), st);
        assert!(t.valid_for(MAX_STACK + 1), "{t:?}");
        assert_eq!(names(&t)[0].len(), MAX_STACK, "the stack is full");
        // One more than fits: the oldest card leaves the stack as a pane of its
        // own rather than vanishing.
        let over = t.push_onto(1, MAX_STACK + 1);
        assert!(over.valid_for(MAX_STACK + 2), "{over:?}");
        assert!(!over.is_stacked(1), "the oldest card came out of the stack");
        assert_eq!(names(&over)[0].len(), MAX_STACK, "and the stack is refilled");
        // Nothing is ever in the tree twice, at the cap either.
        let mut l = over.leaves();
        l.sort_unstable();
        l.dedup();
        assert_eq!(l.len(), MAX_STACK + 2);
    }

    #[test]
    fn a_card_can_come_out_and_a_split_still_works() {
        let t = Node::default_for(2).moved(1, 0, Side::Stack, 1);
        assert_eq!(names(&t), vec![vec![0, 1]]);
        // Taking the back card out leaves the other one holding the whole tab.
        // `unstack` moves the card out; the tab renumbers it into slot 0, the
        // same way closing a pane does.
        let out = t.unstack(0).renumber(0);
        assert!(out.valid_for(1), "{out:?}");
        assert_eq!(out.leaves(), vec![0], "{out:?}");
        assert!(!out.is_stacked(0), "a lone card is not a stack");
        // Dragging the card to a side takes it out of the stack and splits
        // there, which is the other half of "same spot, or beside it".
        let split = t.remove(1).unwrap().insert(0, Side::Right, 1);
        assert!(split.valid_for(2), "{split:?}");
        assert!(!split.is_stacked(1), "{split:?}");
        let (leaves, gutters) = split.layout(R, 6.);
        assert_eq!(gutters.len(), 1, "now there is a real split");
        let r = |i| leaves.iter().find(|(l, _)| *l == i).unwrap().1;
        assert!(r(0).w < 1000. && r(1).w < 1000., "each takes half, not the whole");
    }

    #[test]
    fn closing_a_card_keeps_the_stack_valid_and_renumbers_it() {
        // {0, 1} in a stack, with leaf 2 as a pane beside it.
        let t = Node::default_for(3).moved(1, 0, Side::Stack, 1);
        assert_eq!(names(&t), vec![vec![0, 1]]);
        // Closing the pane beside the stack leaves the stack holding the whole
        // tab. Nothing is lost and the numbering still starts at 0.
        let after = t.remove(2).unwrap().renumber(2);
        assert!(after.valid_for(2), "{after:?}");
        assert!(after.is_stacked(0) && after.is_stacked(1), "{after:?}");
        // Closing one card of the stack: the tree shrinks to a single card,
        // which is a plain pane again, and the numbers close up.
        let after = t.remove(2).unwrap().remove(1).unwrap();
        assert!(after.valid_for(1), "{after:?}");
        assert!(!after.is_stacked(0), "{after:?}");
        assert_eq!(after.remove(0), None, "and the last card ends the tree");
    }

    #[test]
    fn a_stack_survives_json_and_older_shapes_still_load() {
        let t = Node::default_for(3).moved(2, 1, Side::Stack, 2);
        let s = serde_json::to_string(&t).unwrap();
        assert!(s.contains("\"stack\":["), "{s}");
        assert_eq!(serde_json::from_str::<Node>(&s).unwrap(), t);
        // `active` with no value reads as the back card.
        let back: Node = serde_json::from_str(r#"{"stack":[0,1]}"#).unwrap();
        assert_eq!(back, Node::Stack { stack: vec![0, 1], active: 0 });
        // The shapes from before stacks still parse, unchanged.
        assert_eq!(serde_json::from_str::<Node>(r#"{"leaf":2}"#).unwrap(), Node::leaf(2));
        assert!(serde_json::from_str::<Node>(r#"{"dir":"row","ratio":0.5,"a":{"leaf":0},"b":{"leaf":1}}"#).is_ok());
    }
}