//! The Streams filter (Charles, 23 Nov: "next to streams, filter by kind of streams: by folder, by
//! status"). A header above the list with a filter button; open, it lists the statuses and the
//! folders to pick from; a picked one shows as a chip that clears it.

use super::*;
use crate::threads::Group;
use gpui_kit::prelude::FluentBuilder;

/// What the list is narrowed to: a status group, a folder, both or neither.
#[derive(Clone, Default, PartialEq, Debug)]
pub(crate) struct StreamFilter {
    pub status: Option<Group>,
    pub folder: Option<String>,
}

impl StreamFilter {
    pub(crate) fn is_empty(&self) -> bool {
        self.status.is_none() && self.folder.is_none()
    }

    /// Whether `t` passes: its group (a stream finished today counts as Done) and its folder.
    pub(crate) fn passes(&self, t: &ThreadInfo) -> bool {
        self.status.is_none_or(|g| Group::of(t) == g) && self.folder.as_ref().is_none_or(|f| &threads::meta(t) == f)
    }
}

impl Cauldron {
    /// The header above the list: "Streams", the filter button, and a chip per active filter; open,
    /// the choices under it.
    pub(super) fn streams_head(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Div {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let on = !self.sfilter.is_empty();
        let chip = |id: &'static str, label: String, clear: fn(&mut Cauldron)| {
            div()
                .id(id)
                .h(px(20.))
                .px(px(7.))
                .flex()
                .flex_none()
                .items_center()
                .gap(px(4.))
                .rounded_full()
                .bg(c(pal.selected))
                .text_size(px(11.))
                .text_color(fg)
                .cursor_pointer()
                .aria_label(format!("{label}, clear"))
                .child(div().max_w(px(90.)).truncate().child(label))
                .child(glyph("x", 9., fg3))
                .on_click(cx.listener(move |this, _, _, cx| {
                    clear(this);
                    cx.notify();
                }))
                .test_support()
        };
        let mut head = div().flex().flex_col().child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(30.))
                .px(px(8.))
                .pt(px(6.))
                .text_size(px(11.5))
                .text_color(fg3)
                .child(div().child("Streams"))
                .when_some(self.sfilter.status, |d, g| d.child(chip("filter-status-chip", g.label().to_string(), |t| t.sfilter.status = None)))
                .when_some(self.sfilter.folder.clone(), |d, f| d.child(chip("filter-folder-chip", f, |t| t.sfilter.folder = None)))
                .child(
                    div()
                        .id("filter-btn")
                        .ml_auto()
                        .size(px(22.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(radius::TAG))
                        .cursor_pointer()
                        .when(self.sfilter_open || on, |d| d.bg(c(pal.selected)))
                        .hover(|d| d.bg(c(pal.hover)))
                        .aria_label("Filter streams")
                        .aria_expanded(self.sfilter_open)
                        .child(glyph("list-filter", 13., if on { fg } else { fg2 }))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sfilter_open = !this.sfilter_open;
                            cx.notify();
                        }))
                        .test_support(),
                ),
        );
        if !self.sfilter_open {
            return head;
        }
        let folders: Vec<String> = match &self.threads {
            Threads::Ready(v) => {
                let mut f: Vec<String> = v.iter().filter(|t| t.side_of.is_none()).map(threads::meta).collect();
                f.sort();
                f.dedup();
                f
            }
            _ => Vec::new(),
        };
        let opt = |id: (&'static str, usize), label: String, picked: bool, set: Box<dyn Fn(&mut Cauldron)>| {
            div()
                .id(id)
                .h(px(24.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(6.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(if picked { fg } else { fg2 })
                .when(picked, |d| d.bg(c(pal.selected)))
                .hover(|d| d.bg(c(pal.hover)))
                .aria_label(label.clone())
                .aria_selected(picked)
                .child(div().flex_1().min_w_0().truncate().child(label))
                .when(picked, |d| d.child(glyph("check", 11., fg2)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    set(this);
                    cx.notify();
                }))
                .test_support()
        };
        let lbl = |t: &'static str| div().px(px(8.)).pt(px(6.)).pb(px(2.)).text_size(px(10.5)).text_color(fg3).child(t);
        let mut panel = div().id("filter-panel").flex().flex_col().mx(px(4.)).mb(px(4.)).p(px(4.)).rounded(px(radius::CONTROL)).bg(c(pal.hover)).child(lbl("Status"));
        let st = [(None, "All"), (Some(Group::NeedsYou), "Needs you"), (Some(Group::Running), "Running"), (Some(Group::Done), "Done")];
        for (i, (g, label)) in st.into_iter().enumerate() {
            panel = panel.child(opt(("filter-status", i), label.to_string(), self.sfilter.status == g, Box::new(move |t| t.sfilter.status = g)));
        }
        panel = panel.child(lbl("Folder")).child(opt(("filter-folder", 0), "All folders".into(), self.sfilter.folder.is_none(), Box::new(|t| t.sfilter.folder = None)));
        for (i, f) in folders.into_iter().enumerate() {
            let picked = self.sfilter.folder.as_ref() == Some(&f);
            let name = f.clone();
            panel = panel.child(opt(("filter-folder", i + 1), f, picked, Box::new(move |t| t.sfilter.folder = Some(name.clone()))));
        }
        head = head.child(panel);
        head
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A filter by folder and by status both have to hold; a stream finished today is still Done.
    #[::core::prelude::v1::test]
    fn filter_needs_both_the_status_and_the_folder() {
        let mut a = threads::sample("a", "A");
        a.project = Some("/x/router".into());
        let mut b = threads::sample("b", "B");
        b.project = Some("/x/router".into());
        b.busy = true;
        let f = StreamFilter { status: Some(Group::Done), folder: Some("router".into()) };
        assert!(f.passes(&a));
        assert!(!f.passes(&b), "running is not Done");
        assert!(!StreamFilter { status: None, folder: Some("landing".into()) }.passes(&a));
    }
}
