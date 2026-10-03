//! How a tab arranges its canvases: a binary tree of splits whose leaves
//! index the tab's panes. Both shells draw from these rectangles and apply
//! drags with these edits, so a layout behaves the same in GPUI and Tauri.
//!
//! JSON: `{"leaf": 0}` or `{"dir": "row", "ratio": 0.5, "a": ..., "b": ...}`
//! (`row` puts `a` left of `b`, `col` puts it above).

use serde::{Deserialize, Serialize};

/// The leaf id of the drop placeholder in a drag preview.
pub const PH: usize = usize::MAX;

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

    pub fn leaves(&self) -> Vec<usize> {
        match self {
            Node::Leaf { leaf } => vec![*leaf],
            Node::Split { a, b, .. } => {
                let mut v = a.leaves();
                v.extend(b.leaves());
                v
            }
        }
    }

    /// Exactly the leaves 0..n, each once.
    pub fn valid_for(&self, n: usize) -> bool {
        let mut l = self.leaves();
        l.sort_unstable();
        l == (0..n).collect::<Vec<_>>()
    }

    /// Rectangles for every leaf and gutter inside `r`, `gap` apart.
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

    /// The tree without `leaf`; its sibling takes the parent's place.
    /// None when `leaf` was the whole tree.
    pub fn remove(&self, leaf: usize) -> Option<Node> {
        match self {
            Node::Leaf { leaf: l } => (*l != leaf).then(|| self.clone()),
            Node::Split { dir, ratio, a, b } => match (a.remove(leaf), b.remove(leaf)) {
                (Some(a), Some(b)) => Some(Node::split(*dir, *ratio, a, b)),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            },
        }
    }

    /// Split `target` and put `new` on `side` of it (Center replaces).
    pub fn insert(&self, target: usize, side: Side, new: usize) -> Node {
        match self {
            Node::Leaf { leaf } if *leaf == target => {
                let (t, n) = (Node::leaf(target), Node::leaf(new));
                match side {
                    Side::Left => Node::split(Dir::Row, 0.5, n, t),
                    Side::Right => Node::split(Dir::Row, 0.5, t, n),
                    Side::Top => Node::split(Dir::Col, 0.5, n, t),
                    Side::Bottom => Node::split(Dir::Col, 0.5, t, n),
                    Side::Center => n,
                }
            }
            Node::Leaf { .. } => self.clone(),
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
        assert_eq!(zone(r, 50., 50.), Side::Center);
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
}
