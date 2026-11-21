//! The stream card's tab (Charles, 20 Nov, from his picture): the card grows a tab at its top
//! left that holds the stream's tabs; to its right the frame comes down to the card's top line,
//! joined by a concave foot. Radii (Charles, 21 Nov: the picture's were too round, and the top
//! should match the bottom): every card corner, the tab's top ones too, is `TAB_R` (2s); the
//! foot is one stroke; the open pill is concentric with the tab (TAB_R - S). The content shares
//! the card's glass: no inner well with its own frost.
//!
//! GPUI rounds rectangles only, and the card is glass, so the shape is the card's own rect (its
//! top-left rounded) with three pieces over it, each a crop of the very layer it stands for:
//! - the frame right of the tab, its bottom-left rounded: the concave foot;
//! - at the tab's top-right and the body's top-right, a frame square under a card square whose
//!   top-right is rounded: two convex corners.
//!
//! Nothing is drawn twice, so the glass tints never stack.

use super::*;

/// The frame stroke: the inset of the tabs.
pub(crate) const S: f32 = 6.0;
/// Every corner of the stream card, the tab's top ones included (2s).
pub(crate) const TAB_R: f32 = 12.0;
/// The body's top-right corner: the card's own.
pub(crate) const BODY_R: f32 = TAB_R;
/// The concave foot where the tab meets the card's top line (1s).
pub(crate) const FOOT_R: f32 = S;
/// The open tab's pill, one stroke inside the tab: concentric with its corners.
pub(crate) const PILL_R: f32 = TAB_R - S;
/// The tab's height over the card's top line: the tabs' inset plus their height, so the open
/// tab's pill sits on that line, as in the picture.
pub(crate) const BUMP_H: f32 = metric::HEAD_IN + metric::TAB_H;

/// The frame beside the card's tab on one side, with its two convex corners: at the tab's top
/// edge and at the body's top corner on that side. Each corner is a `TAB_R` frame square under a
/// `2 * TAB_R` card square (GPUI clamps a corner's radius to half the side).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Side {
    pub frame: Rect,
    pub tab_q: (Rect, Rect),
    pub body_q: (Rect, Rect),
}

/// The pieces for a card `main` whose tab spans `bump = (x, w)` from the card's left edge (the
/// tab follows the open tab, Charles 21 Nov): the frame left of the tab, if the tab is not at the
/// card's left, and right of it, if it does not reach the right. None when neither is there.
pub(crate) fn pieces(main: Rect, bump: (f32, f32)) -> Option<(Option<Side>, Option<Side>)> {
    let d = TAB_R * 2.0;
    let x = bump.0.max(0.0).floor();
    let w = bump.1.max(d).ceil();
    let room = FOOT_R + d;
    let left = (x >= room).then(|| {
        let bx = main.x + x;
        Side {
            frame: Rect::new(main.x, main.y, x, BUMP_H),
            tab_q: (Rect::new(bx, main.y, TAB_R, TAB_R), Rect::new(bx, main.y, d, d)),
            body_q: (Rect::new(main.x, main.y + BUMP_H, BODY_R, BODY_R), Rect::new(main.x, main.y + BUMP_H, BODY_R * 2.0, BODY_R * 2.0)),
        }
    });
    let end = x + w;
    let right = (end + room < main.w).then(|| {
        let rx = main.x + end;
        Side {
            frame: Rect::new(rx, main.y, main.w - end, BUMP_H),
            tab_q: (Rect::new(rx - TAB_R, main.y, TAB_R, TAB_R), Rect::new(rx - d, main.y, d, d)),
            body_q: (Rect::new(main.right() - BODY_R, main.y + BUMP_H, BODY_R, BODY_R), Rect::new(main.right() - BODY_R * 2.0, main.y + BUMP_H, BODY_R * 2.0, BODY_R * 2.0)),
        }
    });
    (left.is_some() || right.is_some()).then_some((left, right))
}

#[derive(Clone, Copy)]
enum Corner {
    None,
    BottomLeft(f32),
    BottomRight(f32),
    TopRight(f32),
    TopLeft(f32),
}

fn round<E: Styled>(e: E, k: Corner) -> E {
    match k {
        Corner::None => e,
        Corner::BottomLeft(r) => e.rounded_bl(px(r)),
        Corner::BottomRight(r) => e.rounded_br(px(r)),
        Corner::TopRight(r) => e.rounded_tr(px(r)),
        Corner::TopLeft(r) => e.rounded_tl(px(r)),
    }
}

impl Cauldron {
    /// The frame at `r`, exactly as the window's frame layer draws it there.
    fn frame_piece(&mut self, id: &'static str, r: Rect, k: Corner, win: (f32, f32), dark: bool, pal: &'static Palette, window: &mut Window) -> Div {
        let mut d = round(abs(r), k);
        if self.painting {
            let mut stale = Vec::new();
            let crop = self.glass.piece(id, &backdrop::frame_rgba(dark), dark, win, r, None, &mut stale);
            for old in stale {
                let _ = window.drop_image(old);
            }
            d = d
                .child(round(img(crop).absolute().left_0().top_0().size_full().object_fit(ObjectFit::Fill), k))
                .child(round(div().absolute().left_0().top_0().size_full().bg(backdrop::frame_tint_at(pal, dark, self.frost)), k));
        } else {
            d = d.bg(c(pal.frame));
        }
        d
    }

    /// The stream card's surface at `r`: its glass crop, the user's picture, or its colour.
    fn card_piece(&mut self, id: &'static str, main: Rect, r: Rect, k: Corner, win: (f32, f32), dark: bool, pal: &'static Palette, window: &mut Window) -> Div {
        let mut d = round(abs(r), k);
        if !self.painting {
            return d.bg(backdrop::card_bg(false, pal));
        }
        let mut stale = Vec::new();
        let src = match (self.stream_bg.is_some(), backdrop::user_layers()) {
            (true, Some(u)) => Some((u.pic.clone(), (main.w, main.h), Rect::new(r.x - main.x, r.y - main.y, r.w, r.h), None, backdrop::surface(true, pal.frame, backdrop::dim_alpha(&Prefs::load())))),
            (true, None) => None,
            // The card's glass is its own crop of the light layer (GlassCache::get), stretched.
            (false, _) => backdrop::card_source(dark).map(|src| (src, win, r, Some(main), backdrop::tint(pal, dark))),
        };
        d = match src {
            Some((src, area, at, within, tint)) => {
                let crop = self.glass.piece(id, &src, dark, area, at, within, &mut stale);
                d.child(round(img(crop).absolute().left_0().top_0().size_full().object_fit(ObjectFit::Fill), k)).child(round(div().absolute().left_0().top_0().size_full().bg(tint), k))
            }
            None => d.bg(backdrop::surface(true, pal.panel, backdrop::GLASS_CARD)),
        };
        for old in stale {
            let _ = window.drop_image(old);
        }
        d
    }

    /// The pieces over the stream card that cut its tab (module docs), in drawing order. They go on
    /// the window right after the card (over its border, under its tabs, tools and rail).
    pub(super) fn tab_cut(&mut self, main: Rect, bump: (f32, f32), win: (f32, f32), dark: bool, pal: &'static Palette, window: &mut Window) -> Vec<AnyElement> {
        let Some((left, right)) = pieces(main, bump) else { return Vec::new() };
        let line = c(if self.painting { pal.line_strong } else { pal.line });
        let mut out = Vec::new();
        for (side, on_left) in [(left, true), (right, false)] {
            let Some(side) = side else { continue };
            let f = side.frame;
            // One past the card's outer edge and top, over its border there.
            let (fid, cid1, cid2, qf1, qf2) = if on_left {
                ("tab-frame-l", "tab-q0-card", "tab-q3-card", "tab-q0-frame", "tab-q3-frame")
            } else {
                ("tab-frame", "tab-q1-card", "tab-q2-card", "tab-q1-frame", "tab-q2-frame")
            };
            let (out_r, foot, convex) = if on_left {
                (Rect::new(f.x - 1.0, f.y - 1.0, f.w + 1.0, f.h + 1.0), Corner::BottomRight(FOOT_R), Corner::TopLeft as fn(f32) -> Corner)
            } else {
                (Rect::new(f.x, f.y - 1.0, f.w + 1.0, f.h + 1.0), Corner::BottomLeft(FOOT_R), Corner::TopRight as fn(f32) -> Corner)
            };
            let ring = div().absolute().left_0().top_0().size_full().border_b_1().border_color(line);
            let ring = if on_left { ring.rounded_br(px(FOOT_R)).border_r_1() } else { ring.rounded_bl(px(FOOT_R)).border_l_1() };
            // The frame beside the tab is window frame: on Windows it drags the window (the
            // stream card rises into the strip there, 20 Nov). Drawn before the tabs and the
            // caption buttons, so their clicks stay theirs.
            let piece = self.frame_piece(fid, out_r, foot, win, dark, pal, window).child(ring);
            let piece = if on_left { piece.id("tab-cut-l") } else { piece.id("tab-cut") };
            out.push(piece.window_control_area(WindowControlArea::Drag).test_support().into_any_element());
            for (fqid, cqid, (fq, q), r) in [(qf1, cid1, side.tab_q, TAB_R), (qf2, cid2, side.body_q, BODY_R)] {
                let fq = if on_left { Rect::new(fq.x - 1.0, fq.y - 1.0, fq.w + 1.0, fq.h + 1.0) } else { Rect::new(fq.x, fq.y - 1.0, fq.w + 1.0, fq.h + 1.0) };
                out.push(self.frame_piece(fqid, fq, Corner::None, win, dark, pal, window).into_any_element());
                let edge = div().absolute().left_0().top_0().size_full().border_t_1().border_color(line);
                let edge = if on_left { edge.rounded_tl(px(r)).border_l_1() } else { edge.rounded_tr(px(r)).border_r_1() };
                let card = self.card_piece(cqid, main, q, convex(r), win, dark, pal, window).child(edge);
                out.push(card.into_any_element());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tab's top corners are the card's bottom ones, the pill is concentric with the tab, and
    /// the pill sits on the card's top line.
    #[::core::prelude::v1::test]
    fn the_top_corners_match_the_bottom_and_the_pill_is_concentric() {
        assert_eq!(BODY_R, TAB_R);
        assert_eq!(PILL_R, TAB_R - S);
        assert!(TAB_R < 17.0, "Charles, 21 Nov: the 17px tab top was too round");
        assert_eq!(BUMP_H, metric::HEAD_IN + metric::TAB_H, "the open tab's pill ends on the card's top line");
        assert!((metric::HEAD_IN - S).abs() < 0.5, "the tabs sit one stroke in, as the picture's pill");
    }

    /// The frame on the right starts where the tab ends and reaches the card's right edge; a tab
    /// away from the card's left also has frame on its left; a tab as wide as the card cuts
    /// nothing.
    #[::core::prelude::v1::test]
    fn the_pieces_sit_beside_the_tab_wherever_it_is() {
        let main = Rect::new(260.0, 44.0, 900.0, 600.0);
        let (l, r) = pieces(main, (0.0, 300.0)).unwrap();
        assert!(l.is_none());
        let r = r.unwrap();
        assert_eq!((r.frame.x, r.frame.y, r.frame.right(), r.frame.h), (560.0, 44.0, main.right(), BUMP_H));
        assert_eq!((r.tab_q.0.right(), r.tab_q.0.y), (560.0, main.y));
        assert_eq!((r.body_q.0.right(), r.body_q.0.y), (main.right(), main.y + BUMP_H));
        for ((frame, card), rr) in [(r.tab_q, TAB_R), (r.body_q, BODY_R)] {
            assert_eq!((frame.w, card.w), (rr, rr * 2.0), "GPUI caps a radius at half the side");
        }
        let (l, r) = pieces(main, (400.0, 120.0)).unwrap();
        let l = l.unwrap();
        assert_eq!((l.frame.x, l.frame.right(), l.tab_q.0.x, l.body_q.0.x), (main.x, 660.0, 660.0, main.x));
        assert_eq!(r.unwrap().frame.x, 780.0);
        let (l, r) = pieces(main, (700.0, 200.0)).unwrap();
        assert!(l.is_some() && r.is_none(), "a tab at the card's right end has frame only on its left");
        assert!(pieces(main, (0.0, 890.0)).is_none());
    }
}
