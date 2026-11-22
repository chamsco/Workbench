//! Reply text you can select (Charles, 22 Nov: select some text and get a small bar of actions).
//! gpui-base's `SelectableText` takes plain text only; this is the same element with the styled
//! runs a reply line already has (bold, inline code), so selecting keeps how the line looks. The
//! window-wide selection layer it joins is mounted by `Root`.

use crate::gpui::{App, Bounds, Corners, Edges, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId, PaintQuad, Pixels, Point, SharedString, StyledText, Window};
use gpui_kit::base::{TextSelection, TextSelectionHandle, TextSelectionRegistration, TextSelectionRun};
use std::ops::Range;

pub(crate) struct SelText {
    id: ElementId,
    text: SharedString,
    styled: StyledText,
    color: Hsla,
}

impl SelText {
    pub(crate) fn new(id: impl Into<ElementId>, text: String, highlights: Vec<(Range<usize>, crate::gpui::HighlightStyle)>, color: Hsla) -> Self {
        let text: SharedString = text.into();
        Self { id: id.into(), styled: StyledText::new(text.clone()).with_highlights(highlights), text, color }
    }
}

fn quads(start: Point<Pixels>, end: Point<Pixels>, b: Bounds<Pixels>, lh: Pixels) -> Vec<Bounds<Pixels>> {
    if start.y == end.y {
        return vec![Bounds::from_corners(start, Point::new(end.x, end.y + lh))];
    }
    let mut q = vec![Bounds::from_corners(start, Point::new(b.right(), start.y + lh))];
    if end.y > start.y + lh {
        q.push(Bounds::from_corners(Point::new(b.left(), start.y + lh), Point::new(b.right(), end.y)));
    }
    q.push(Bounds::from_corners(Point::new(b.left(), end.y), Point::new(end.x, end.y + lh)));
    q
}

impl IntoElement for SelText {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for SelText {
    type RequestLayoutState = TextSelectionHandle;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, gid: Option<&GlobalElementId>, iid: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, TextSelectionHandle) {
        let text = self.text.clone();
        let handle = window.with_element_state(gid.expect("SelText has an id"), |kept: Option<TextSelectionHandle>, _| {
            let h = kept.unwrap_or_else(|| TextSelectionHandle::new(text, cx));
            (h.clone(), h)
        });
        let (id, ()) = self.styled.request_layout(gid, iid, window, cx);
        (id, handle)
    }

    fn prepaint(&mut self, gid: Option<&GlobalElementId>, iid: Option<&InspectorElementId>, bounds: Bounds<Pixels>, handle: &mut TextSelectionHandle, window: &mut Window, cx: &mut App) -> Hitbox {
        self.styled.prepaint(gid, iid, bounds, &mut (), window, cx);
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        handle.register(TextSelectionRegistration::new(hitbox.clone(), bounds).with_text_bounds(vec![bounds]), window, cx);
        hitbox
    }

    fn paint(&mut self, gid: Option<&GlobalElementId>, iid: Option<&InspectorElementId>, bounds: Bounds<Pixels>, handle: &mut TextSelectionHandle, _: &mut Hitbox, window: &mut Window, cx: &mut App) {
        let layout = self.styled.layout().clone();
        let before = TextSelection::selected_text(window, cx);
        let proj = handle.update_runs(&[TextSelectionRun::new(self.text.clone(), layout.clone(), bounds)], cx);
        if before != TextSelection::selected_text(window, cx) {
            window.refresh();
        }
        for r in proj.ranges().iter().flatten().cloned() {
            if let (Some(a), Some(b)) = (layout.position_for_index(r.start), layout.position_for_index(r.end)) {
                for q in quads(a, b, layout.bounds(), layout.line_height()) {
                    window.paint_quad(PaintQuad { bounds: q, background: self.color.into(), corner_radii: Corners::default(), border_widths: Edges::default(), border_color: crate::gpui::transparent_black(), border_style: crate::gpui::BorderStyle::default() });
                }
            }
        }
        self.styled.paint(gid, iid, bounds, &mut (), &mut (), window, cx);
    }
}
