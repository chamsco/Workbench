//! The apps tray (19 Nov, after Charles's Ghostex reference and the context sidebar approved on
//! 13 Nov): each app swaps the sidebar's list. Streams is the stream list; Folders groups streams
//! by the folder they run in (a click filters Streams to it); Agents lists the familiars. Usage left
//! the row on 21 Nov for the sidebar foot (usage.rs). Automations and Library have no source yet and say so.

use super::*;

/// The apps row's geometry: separate buttons, left-packed.
pub(crate) const TRAY_BTN_W: f32 = 32.0;
pub(crate) const TRAY_BTN_H: f32 = 30.0;
pub(crate) const TRAY_GAP: f32 = 4.0;

/// Folders of the streams, with how many streams each has, most streams first, then by name.
/// Streams with no folder count under "No folder" (last).
pub(crate) fn folders(list: &[ThreadInfo]) -> Vec<(String, usize)> {
    let mut by: std::collections::BTreeMap<String, usize> = Default::default();
    let mut none = 0;
    for t in list {
        match &t.project {
            Some(_) => *by.entry(threads::meta(t)).or_default() += 1,
            None => none += 1,
        }
    }
    let mut v: Vec<(String, usize)> = by.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    if none > 0 {
        v.push((NO_FOLDER.into(), none));
    }
    v
}

pub(crate) const NO_FOLDER: &str = "No folder";
pub(crate) const NEW_FAMILIAR: &str = "New familiar";

impl Cauldron {
    /// The sidebar's list for app `section` other than Streams.
    pub(super) fn app_list(&self, section: usize, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mut list = div().id("app-list").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().pb(px(8.));
        let head = |id: &'static str, label: String| {
            div().id(id).flex().items_center().h(px(34.)).flex_none().px(px(8.)).pt(px(14.)).pb(px(4.)).text_size(px(11.5)).line_height(px(16.)).text_color(fg3).aria_label(label.clone()).child(label).test_support()
        };
        let row = |id: (&'static str, usize), dot: Option<Hsla>, title: String, meta: String| {
            div()
                .id(id)
                .h(px(30.))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .aria_label(format!("{title}, {meta}"))
                .when_live(dot.is_some(), |d| d.child(div().size(px(7.)).flex_none().rounded_full().bg(dot.unwrap_or(fg3))))
                .child(div().flex_1().min_w_0().truncate().text_color(fg).child(title))
                .child(div().flex_none().text_size(px(11.)).font_features(tnum()).text_color(fg3).child(meta))
        };
        let note = |id: &'static str, title: &'static str, body: &'static str| {
            div().id(id).flex().flex_col().gap(px(4.)).px(px(8.)).pt(px(14.)).aria_label(format!("{title}. {body}")).child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg2).child(title)).child(div().text_size(px(11.5)).line_height(px(16.)).text_color(fg3).child(body)).test_support()
        };
        match FUNCTIONS[section].1 {
            "Library" => {
                let v = match &self.threads {
                    Threads::Ready(v) => folders(v),
                    _ => Vec::new(),
                };
                list = list.child(head("folders-head", "Where your streams run".into()));
                if v.is_empty() {
                    return list.child(note("folders-empty", "No folders yet", "A stream started in a folder shows it here.")).child(note("library-later", "The Librarian is not built yet", "Its tabs show sample proposals, notes and a graph, marked Mockup."));
                }
                for (i, (name, n)) in v.into_iter().enumerate() {
                    let q = if name == NO_FOLDER { String::new() } else { name.clone() };
                    list = list.child(
                        row(("folder", i), Some(c(pal.fg4)), name, n.to_string())
                            .on_click(cx.listener(move |this, _, window, cx| this.filter_streams(q.clone(), window, cx)))
                            .test_support(),
                    );
                }
                list.child(note("library-later", "The Librarian is not built yet", "Its tabs show sample proposals, notes and a graph, marked Mockup."))
            }
            "Agents" => {
                // Familiars: captains with their crews (captain.rs), each opening its stream.
                list = list.child(head("captains-head", "Familiars".into()));
                if self.start.captains.is_empty() {
                    list = list.child(note("captains-empty", "No familiars yet", "New familiar makes a captain that learns one project and leads a standing crew."));
                }
                let roster_all = super::thread::agents();
                for (i, a) in self.start.captains.iter().enumerate() {
                    let id = a.id.clone();
                    let project = a.lead.as_ref().map(|l| l.project.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or("").to_string()).unwrap_or_default();
                    list = list.child(
                        div()
                            .id(("captain-row", i))
                            .h(px(46.))
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(9.))
                            .rounded(px(radius::CONTROL))
                            .cursor_pointer()
                            .hover(|d| d.bg(c(pal.hover)))
                            .aria_label(format!("{}, captain of {project}", a.name))
                            .child(super::captain::face(&a.avatar, 30., pal))
                            .child(div().flex_1().min_w_0().flex().flex_col().child(div().truncate().text_color(fg).child(a.name.clone())).child(div().truncate().text_size(px(11.)).text_color(fg3).child(format!("Captain · {project}"))))
                            .on_click(cx.listener(move |this, _, _, cx| this.open_captain(&id, cx)))
                            .test_support(),
                    );
                    for (j, m) in a.lead.iter().flat_map(|l| l.crew.iter()).filter_map(|id| roster_all.get(id)).enumerate() {
                        list = list.child(
                            div()
                                .id(("crew-row", i * 16 + j))
                                .ml(px(22.))
                                .pl(px(10.))
                                .h(px(28.))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .border_l_1()
                                .border_color(c(pal.line))
                                .text_size(px(12.))
                                .text_color(fg2)
                                .aria_label(format!("{}, crew of {}", m.name, a.name))
                                .child(super::captain::face(&m.avatar, 18., pal))
                                .child(div().truncate().child(m.name.clone()))
                                .test_support(),
                        );
                    }
                }
                let (real, roster) = self.roster();
                list = list.child(div().flex().items_center().gap(px(6.)).child(head("agents-head", "Missions".into())).children((!real).then(|| Self::mock_tag(pal))));
                for (i, (name, _, state, _)) in roster.into_iter().enumerate() {
                    let on = self.section_page && self.pages.agent == i;
                    let dot = match state {
                        familiar::State::NeedsYou => c(pal.warn),
                        familiar::State::Running => c(pal.accent),
                        familiar::State::Failed => c(pal.bad),
                        familiar::State::Done => c(pal.fg4),
                    };
                    list = list.child(
                        div()
                            .id(("agent-row", i))
                            .h(px(46.))
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(9.))
                            .rounded(px(radius::CONTROL))
                            .cursor_pointer()
                            .when_live(on, |d| d.bg(c(pal.selected)))
                            .hover(|d| d.bg(c(pal.hover)))
                            .aria_label(format!("{name}, {}", familiar::state_text(state)))
                            .child(img(Arc::new(Image::from_bytes(ImageFormat::Svg, avatar::svg(i, Mood::Idle, Look { px: 30., outline: false, ms: None, bust: true, ink: None }).into_bytes()))).size(px(30.)))
                            .child(div().flex_1().min_w_0().flex().flex_col().child(div().truncate().text_color(fg).child(name)).child(div().flex().items_center().gap(px(5.)).text_size(px(11.)).text_color(fg3).child(div().size(px(6.)).rounded_full().bg(dot)).child(familiar::state_text(state))))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                // The page beside shows it (Charles, 21 Nov 17:33Z: the roster lives in the sidebar).
                                this.pages.agent = i;
                                this.select_section(2, cx);
                                cx.notify();
                            }))
                            .test_support(),
                    );
                }
                if self.fam_views.is_empty() {
                    list = list.child(note("agents-empty", "No missions yet", "A mission hands a goal to the pipeline in a folder: it plans, builds, verifies and ships."));
                }
                list
            }
            _ => list.child(note("automations-later", "Automations arrive in a later build", "Scheduled and repeating streams will be listed here.")),
        }
    }

    /// The sidebar's New button for app `section` (Charles, 20 Nov: "every section has a New
    /// stream button when it should adapt"): (id, icon, label). None hides it where the app has
    /// nothing to make yet, rather than drawing a button that does nothing.
    pub(super) fn new_action(section: usize) -> Option<(&'static str, &'static str, &'static str)> {
        match FUNCTIONS.get(section).map(|f| f.1) {
            Some("Streams") | None => Some(("new-thread", "square-pen", NEW_TAB)),
            Some("Library") => Some(("new-folder", "folder-plus", "Open a folder")),
            Some("Agents") => Some(("new-familiar", "sparkles", NEW_FAMILIAR)),
            _ => None,
        }
    }

    /// What the section's New button does.
    pub(super) fn do_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match FUNCTIONS.get(self.section).map(|f| f.1) {
            Some("Library") => {
                self.settings = None;
                self.new_thread(window, cx);
                self.pick_folder(window, cx);
            }
            Some("Agents") => self.new_familiar(window, cx),
            _ => {
                self.settings = None;
                self.new_thread(window, cx);
            }
        }
    }

    /// New familiar: the captain form (captain.rs; Charles, 22 Nov). The goal pipeline is a
    /// mission now, one link away on that form (`new_mission`).
    pub(super) fn new_familiar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_captain_form(window, cx);
    }

    /// The New stream screen with a mission on: the goal goes to the pipeline in a folder
    /// (plan, Rampart, builders, ship).
    pub(super) fn new_mission(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = None;
        self.new_thread(window, cx);
        self.start.familiar = true;
        cx.notify();
    }

    /// Folders: back to Streams, searching for the folder (the search matches a stream's folder).
    pub(super) fn filter_streams(&mut self, q: String, window: &mut Window, cx: &mut Context<Self>) {
        self.select_section(0, cx);
        self.query = q.clone();
        self.search_open = !q.is_empty();
        if let Some((st, _)) = &self.search {
            st.update(cx, |s, cx| s.set_value(q, window, cx));
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: &str, project: Option<&str>) -> ThreadInfo {
        let mut v = threads::fixture(1_000_000_000).remove(0);
        v.id = id.into();
        v.project = project.map(Into::into);
        v
    }

    /// Folders count streams by their folder's name, busiest first, "No folder" last.
    #[::core::prelude::v1::test]
    fn folders_count_streams_busiest_first_no_folder_last() {
        let v = vec![t("a", Some("/x/Cauldron")), t("b", None), t("c", Some("/y/Ocean")), t("d", Some("C:\\w\\Cauldron"))];
        assert_eq!(folders(&v), vec![("Cauldron".into(), 2), ("Ocean".into(), 1), (NO_FOLDER.into(), 1)]);
        assert!(folders(&[]).is_empty());
    }

    /// Six separate buttons with gaps fit the sidebar's width.
    #[::core::prelude::v1::test]
    fn six_app_buttons_fit_the_sidebar() {
        assert!(6.0 * TRAY_BTN_W + 5.0 * TRAY_GAP <= 236.0 - 18.0, "six apps fit the sidebar");
    }
}
