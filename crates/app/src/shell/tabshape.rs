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

/// The pieces' rects for a card `main` whose tab is `bump_w` wide: the frame right of the tab,
/// and the corners at the tab's top-right and the body's top-right, each a `TAB_R` frame square
/// under a `2 * TAB_R` card square (GPUI clamps a corner's radius to half the side). None when
/// the tab fills the card's width (nothing to cut).
pub(crate) fn pieces(main: Rect, bump_w: f32) -> Option<(Rect, [(Rect, Rect); 2])> {
    let bump_w = bump_w.max(TAB_R * 2.0).ceil();
    if bump_w + FOOT_R + TAB_R * 2.0 >= main.w {
        return None;
    }
    let right = Rect::new(main.x + bump_w, main.y, main.w - bump_w, BUMP_H);
    let d = TAB_R * 2.0;
    let q1 = (Rect::new(right.x - TAB_R, main.y, TAB_R, TAB_R), Rect::new(right.x - d, main.y, d, d));
    let q2 = (Rect::new(main.right() - BODY_R, main.y + BUMP_H, BODY_R, BODY_R), Rect::new(main.right() - BODY_R * 2.0, main.y + BUMP_H, BODY_R * 2.0, BODY_R * 2.0));
    Some((right, [q1, q2]))
}


#[derive(Clone, Copy)]
enum Corner {
    None,
    BottomLeft(f32),
    TopRight(f32),
}

fn round<E: Styled>(e: E, k: Corner) -> E {
    match k {
        Corner::None => e,
        Corner::BottomLeft(r) => e.rounded_bl(px(r)),
        Corner::TopRight(r) => e.rounded_tr(px(r)),
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
            return d.bg(c(pal.panel));
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
    pub(super) fn tab_cut(&mut self, main: Rect, bump_w: f32, win: (f32, f32), dark: bool, pal: &'static Palette, window: &mut Window) -> Vec<AnyElement> {
        let Some((right, qs)) = pieces(main, bump_w) else { return Vec::new() };
        let line = c(if self.painting { pal.line_strong } else { pal.line });
        let mut out = Vec::new();
        // One past the card's right and top edges, over its border there.
        let right_out = Rect::new(right.x, main.y - 1.0, right.w + 1.0, right.h + 1.0);
        let foot = self
            .frame_piece("tab-frame", right_out, Corner::BottomLeft(FOOT_R), win, dark, pal, window)
            .child(div().absolute().left_0().top_0().size_full().rounded_bl(px(FOOT_R)).border_l_1().border_b_1().border_color(line));
        // The frame right of the tab is window frame: on Windows it drags the window (the
        // stream card rises into the strip there, 20 Nov). Drawn before the tabs and the caption
        // buttons, so their clicks stay theirs.
        out.push(foot.id("tab-cut").window_control_area(WindowControlArea::Drag).test_support().into_any_element());
        for (fid, cid, (fq, q), r) in [("tab-q1-frame", "tab-q1-card", qs[0], TAB_R), ("tab-q2-frame", "tab-q2-card", qs[1], BODY_R)] {
            let fq = Rect::new(fq.x, fq.y - 1.0, fq.w + 1.0, fq.h + 1.0);
            out.push(self.frame_piece(fid, fq, Corner::None, win, dark, pal, window).into_any_element());
            let card = self
                .card_piece(cid, main, q, Corner::TopRight(r), win, dark, pal, window)
                .child(div().absolute().left_0().top_0().size_full().rounded_tr(px(r)).border_t_1().border_r_1().border_color(line));
            out.push(card.into_any_element());
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

    /// The frame piece starts where the tab ends and reaches the card's right edge; the corner
    /// squares sit at the tab's top-right and at the body's top-right. A tab as wide as the card
    /// cuts nothing.
    #[::core::prelude::v1::test]
    fn the_pieces_sit_at_the_tabs_edge_and_the_cards_corner() {
        let main = Rect::new(260.0, 44.0, 900.0, 600.0);
        let (right, [q1, q2]) = pieces(main, 300.0).unwrap();
        assert_eq!((right.x, right.y, right.right(), right.h), (560.0, 44.0, main.right(), BUMP_H));
        for ((frame, card), r) in [(q1, TAB_R), (q2, BODY_R)] {
            assert_eq!((frame.right(), frame.y, frame.w), (card.right(), card.y, r));
            assert_eq!(card.w, r * 2.0, "GPUI caps a radius at half the side");
        }
        assert_eq!((q1.0.right(), q1.0.y), (right.x, main.y));
        assert_eq!((q2.0.right(), q2.0.y), (main.right(), main.y + BUMP_H));
        assert!(pieces(main, 890.0).is_none());
    }
}
