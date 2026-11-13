//! Global search (the context-sidebar board, section 4; Charles, 13 Nov 18:57Z): a magnifier at
//! the sidebar's top row and the folded rail's top, or Cmd+K (Mac) / Ctrl+K (Windows, Linux), opens
//! a centred menu. Results rank, they do not filter: the open destination's group first, then
//! everything else under Elsewhere. With nothing typed: recent streams, this destination's actions,
//! and the places to go.
//!
//! What it finds is real where core has it: streams (title and last words), core's agents, the
//! folders streams are in, notes in memory, Settings' pages and the destinations. The example agents
//! and crews show with "Example" on them (UI-PLAN decision 5). Actions are only ones that work.

use super::pages::{tag, AppPage};
use super::*;

/// What a result opens.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    Stream(String),
    Agent(usize),
    Crew(usize),
    Folder(String),
    Note,
    Page(AppPage),
    Settings(usize),
    NewStream,
    ToggleSidebar,
}

/// One thing search can find.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub title: String,
    /// The kind and its context, shown at the right: "Stream · workbench".
    pub sub: String,
    /// More text matched after the title (a stream's last words, a note's body).
    pub text: String,
    /// The destination it belongs to; None for Settings and the places.
    pub home: Option<AppPage>,
    pub icon: &'static str,
    pub example: bool,
    pub action: bool,
    pub target: Target,
}

/// A group of results, by index into the entries.
#[derive(Debug, PartialEq)]
pub(crate) struct Group {
    pub label: String,
    pub items: Vec<usize>,
}

/// The results for `query` on destination `here`. Matching is case-insensitive substring on the
/// title, then the context and text; a title that starts with the query comes first, then one that
/// contains it, then a match in the text. The open destination's results come first, the rest
/// follow under Elsewhere. An empty query gives Recent, this destination's actions and Go to.
pub(crate) fn rank(query: &str, here: AppPage, entries: &[Entry]) -> Vec<Group> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        let recent: Vec<usize> = entries.iter().enumerate().filter(|(_, e)| matches!(e.target, Target::Stream(_))).map(|(i, _)| i).take(3).collect();
        let actions: Vec<usize> = entries.iter().enumerate().filter(|(_, e)| e.action && e.home == Some(here)).map(|(i, _)| i).collect();
        let places: Vec<usize> = entries.iter().enumerate().filter(|(_, e)| matches!(e.target, Target::Page(p) if p != here) || (e.target == Target::Settings(0) && e.sub == "Go to")).map(|(i, _)| i).collect();
        return [("Recent".to_string(), recent), (format!("Actions in {}", here.label()), actions), ("Go to".to_string(), places)]
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(label, items)| Group { label, items })
            .collect();
    }
    let score = |e: &Entry| {
        let t = e.title.to_lowercase();
        if t.starts_with(&q) {
            Some(0)
        } else if t.contains(&q) {
            Some(1)
        } else if e.sub.to_lowercase().contains(&q) || e.text.to_lowercase().contains(&q) {
            Some(2)
        } else {
            None
        }
    };
    let mut hits: Vec<(usize, u8)> = entries.iter().enumerate().filter_map(|(i, e)| score(e).map(|s| (i, s))).collect();
    hits.sort_by_key(|&(_, s)| s);
    let (mine, rest): (Vec<_>, Vec<_>) = hits.into_iter().map(|(i, _)| i).partition(|&i| entries[i].home == Some(here));
    [(here.label().to_string(), mine), ("Elsewhere".to_string(), rest)]
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(label, items)| Group { label, items })
        .collect()
}

/// The open search: its input, the highlighted row, what it searches and where focus goes back.
pub(crate) struct SearchState {
    pub input: Entity<InputState>,
    _sub: Subscription,
    pub sel: usize,
    pub back: Option<FocusHandle>,
}

/// Whether a key press opens or closes search: Cmd+K on a Mac; Ctrl+K elsewhere unless a terminal
/// has focus (Ctrl+K deletes to the end of the line there); Ctrl+Shift+K anywhere.
pub(crate) fn is_toggle(k: &Keystroke, mac: bool, in_terminal: bool) -> bool {
    if k.key != "k" || k.modifiers.alt {
        return false;
    }
    let m = &k.modifiers;
    if mac {
        return m.platform && !m.control;
    }
    m.control && !m.platform && (m.shift || !in_terminal)
}

impl Cauldron {
    /// Everything search can find right now.
    fn search_entries(&self) -> Vec<Entry> {
        let mut v = Vec::new();
        let ready: &[backspace_core::chat::ThreadInfo] = match &self.threads {
            Threads::Ready(l) => l,
            _ => &[],
        };
        let mut streams: Vec<&backspace_core::chat::ThreadInfo> = ready.iter().collect();
        streams.sort_by_key(|t| std::cmp::Reverse(t.updated));
        for t in streams {
            let f = threads::folder(t).unwrap_or_else(|| threads::NO_FOLDER.into());
            v.push(Entry { title: t.title.clone(), sub: format!("Stream · {f}"), text: t.preview.clone(), home: Some(AppPage::Streams), icon: "message-square", example: false, action: false, target: Target::Stream(t.id.clone()) });
        }
        v.push(Entry { title: "New stream".into(), sub: "Action".into(), text: String::new(), home: Some(AppPage::Streams), icon: "square-pen", example: false, action: true, target: Target::NewStream });
        for (i, a) in self.roster().into_iter().enumerate() {
            v.push(Entry { title: a.name.clone(), sub: format!("Agent · {}", a.status), text: a.job.clone(), home: Some(AppPage::Agents), icon: "bot", example: a.example, action: false, target: Target::Agent(i) });
        }
        for (r, (name, members, _)) in pages::ROOMS.iter().enumerate() {
            v.push(Entry { title: (*name).into(), sub: format!("Crew · {} agents", members.len()), text: String::new(), home: Some(AppPage::Agents), icon: "bot", example: true, action: false, target: Target::Crew(r) });
        }
        for (name, ts) in threads::by_folder(ready, "") {
            v.push(Entry { title: name.clone(), sub: format!("Folder · {} streams", ts.len()), text: String::new(), home: Some(AppPage::Folders), icon: "folder", example: false, action: false, target: Target::Folder(name) });
        }
        for (text, place) in &self.search_notes {
            let title: String = text.chars().take(80).collect();
            v.push(Entry { title, sub: format!("Note · {place}"), text: text.clone(), home: Some(AppPage::Library), icon: "database", example: false, action: false, target: Target::Note });
        }
        for p in AppPage::ALL {
            v.push(Entry { title: p.label().into(), sub: "Go to".into(), text: String::new(), home: None, icon: p.icon(), example: false, action: false, target: Target::Page(p) });
        }
        v.push(Entry { title: "Settings".into(), sub: "Go to".into(), text: String::new(), home: None, icon: "settings", example: false, action: false, target: Target::Settings(0) });
        for (i, (name, icon)) in settings::PAGES.iter().enumerate() {
            v.push(Entry { title: format!("Settings: {name}"), sub: "Settings".into(), text: String::new(), home: None, icon, example: false, action: false, target: Target::Settings(i) });
        }
        let side = if self.sidebar_open { "Hide sidebar" } else { "Show sidebar" };
        for p in AppPage::ALL {
            v.push(Entry { title: side.into(), sub: "Action".into(), text: String::new(), home: Some(p), icon: "panel-left", example: false, action: true, target: Target::ToggleSidebar });
        }
        v
    }

    /// Opens search, or closes it if open.
    pub(super) fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_ui.is_some() {
            self.close_search(window, cx);
            return;
        }
        self.search_notes = super::context::notes();
        let st = cx.new(|cx| InputState::new(window, cx).placeholder("Search streams, agents, folders, notes"));
        let sub = cx.subscribe(&st, |this, _, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                if let Some(s) = this.search_ui.as_mut() {
                    s.sel = 0;
                }
                cx.notify();
            }
        });
        let back = window.focused(cx);
        st.read(cx).focus_handle(cx).focus(window, cx);
        self.search_ui = Some(SearchState { input: st, _sub: sub, sel: 0, back });
        cx.notify();
    }

    pub(super) fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = self.search_ui.take() {
            if let Some(f) = s.back {
                f.focus(window, cx);
            } else if let Some(f) = &self.root_focus {
                f.focus(window, cx);
            }
        }
        cx.notify();
    }

    /// The results as shown: everything searchable now (core's streams can arrive after search
    /// opens), the groups, and the entries in order.
    fn search_results(&self, cx: &App) -> (Vec<Entry>, Vec<Group>, Vec<usize>) {
        let Some(s) = &self.search_ui else { return (Vec::new(), Vec::new(), Vec::new()) };
        let q = s.input.read(cx).value().to_string();
        let here = if self.settings.is_some() { AppPage::Streams } else { self.page };
        let entries = self.search_entries();
        let groups = rank(&q, here, &entries);
        let order = groups.iter().flat_map(|g| g.items.iter().copied()).collect();
        (entries, groups, order)
    }

    fn open_target(&mut self, t: Target, window: &mut Window, cx: &mut Context<Self>) {
        self.close_search(window, cx);
        match t {
            Target::Stream(id) => {
                self.open_page(AppPage::Streams);
                self.selected = Some(id.clone());
                self.open_thread(id);
            }
            Target::Agent(i) => {
                self.open_page(AppPage::Agents);
                self.agent_sel = Some(i);
            }
            Target::Crew(r) => {
                self.open_page(AppPage::Agents);
                self.agent_sel = None;
                self.room_sel = r;
            }
            Target::Folder(name) => {
                self.open_page(AppPage::Folders);
                let at = match &self.threads {
                    Threads::Ready(v) => threads::by_folder(v, "").iter().position(|(n, _)| *n == name),
                    _ => None,
                };
                self.side_sel[AppPage::Folders as usize] = at.map(|i| i + 1000);
            }
            Target::Note => {
                self.open_page(AppPage::Library);
            }
            Target::Page(p) => self.open_page(p),
            Target::Settings(i) => self.settings = Some(i),
            Target::NewStream => {
                self.open_page(AppPage::Streams);
                self.new_thread(window, cx);
            }
            Target::ToggleSidebar => self.sidebar_open = !self.sidebar_open,
        }
        cx.notify();
    }

    /// Up, Down, Enter and Esc while search is open (before the input sees them).
    pub(super) fn search_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let (entries, _, order) = self.search_results(cx);
        let Some(s) = self.search_ui.as_mut() else { return false };
        match ev.keystroke.key.as_str() {
            "down" => s.sel = (s.sel + 1).min(order.len().saturating_sub(1)),
            "up" => s.sel = s.sel.saturating_sub(1),
            "escape" => self.close_search(window, cx),
            "enter" => {
                if let Some(&i) = order.get(s.sel) {
                    let t = entries[i].target.clone();
                    self.open_target(t, window, cx);
                }
            }
            _ => return false,
        }
        cx.notify();
        true
    }

    /// The menu over a scrim, centred, about 560 wide.
    pub(super) fn search_overlay(&self, w: f32, h: f32, dark: bool, pal: &'static Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let s = self.search_ui.as_ref()?;
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let (entries, groups, order) = self.search_results(cx);
        let here = if self.settings.is_some() { AppPage::Streams } else { self.page };
        let typed = !s.input.read(cx).value().trim().is_empty();
        let mut list = div().id("search-results").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().px(px(6.)).pb(px(6.)).aria_label("Results");
        let mut k = 0usize;
        for g in &groups {
            list = list.child(div().px(px(10.)).pt(px(10.)).pb(px(4.)).text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child(g.label.clone()));
            for &i in &g.items {
                let e = &entries[i];
                let on = s.sel == k;
                let row = k;
                let target = e.target.clone();
                list = list.child(
                    div()
                        .id(("search-row", k))
                        .h(px(34.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .px(px(10.))
                        .rounded(px(radius::INNER))
                        .cursor_pointer()
                        .when_live(on, |d| d.bg(c(pal.selected)))
                        .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                        .aria_label(e.title.clone())
                        .aria_selected(on)
                        .child(glyph(e.icon, 15., c(pal.fg2)))
                        .child(div().flex_1().min_w_0().truncate().text_size(px(13.)).text_color(fg).child(e.title.clone()))
                        .when_live(e.example, |d| d.child(tag(("search-example", k), "Example", pal).test_support()))
                        .child(div().flex_none().max_w(px(200.)).truncate().text_size(px(12.)).text_color(fg3).child(e.sub.clone()))
                        .on_mouse_move(cx.listener(move |this, _, _, cx| {
                            if let Some(s) = this.search_ui.as_mut() {
                                if s.sel != row {
                                    s.sel = row;
                                    cx.notify();
                                }
                            }
                        }))
                        .on_click(cx.listener(move |this, _, window, cx| this.open_target(target.clone(), window, cx)))
                        .test_support(),
                );
                k += 1;
            }
        }
        if order.is_empty() {
            list = list.child(div().id("search-none").px(px(10.)).py(px(14.)).text_size(px(13.)).text_color(fg3).child("Nothing matches.").test_support());
        }
        let key = |t: &str| div().h(px(18.)).min_w(px(18.)).px(px(4.)).flex().items_center().justify_center().rounded(px(4.)).border_1().border_color(c(pal.line_strong)).text_size(px(10.5)).text_color(fg3).child(t.to_string());
        let mac = cfg!(target_os = "macos");
        let foot = div()
            .h(px(36.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(14.))
            .border_t_1()
            .border_color(c(pal.line))
            .text_size(px(11.5))
            .text_color(fg3)
            .child(key("↑"))
            .child(key("↓"))
            .child("move")
            .child(div().w(px(6.)))
            .child(key("↵"))
            .child("open")
            .child(div().w(px(6.)))
            .child(key("Esc"))
            .child("close")
            .child(div().flex_1())
            .child("Opens from anywhere")
            .child(key(if mac { "⌘" } else { "Ctrl" }))
            .child(key("K"));
        let bw = 560f32.min(w - 32.0);
        let box_ = div()
            .id("search-menu")
            .absolute()
            .left(px((w - bw) / 2.0))
            .top(px((h * 0.14).max(40.)))
            .w(px(bw))
            .max_h(px((h - 120.0).min(460.)))
            .flex()
            .flex_col()
            .rounded(px(14.))
            .border_1()
            .border_color(c(pal.line_strong))
            .bg(c(pal.raised))
            .shadow_lg()
            .overflow_hidden()
            .aria_label("Search")
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("search-field")
                    .h(px(50.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .border_b_1()
                    .border_color(c(pal.line))
                    .aria_label("Search streams, agents, folders, notes")
                    .child(glyph("search", 16., c(pal.fg2)))
                    .child(div().flex_1().min_w_0().child(Input::new(&s.input).appearance(false).text_size(px(15.)).p_0()))
                    .child(div().flex_none().text_size(px(11.5)).text_color(fg3).child(if typed { format!("{} first", here.label()) } else { String::new() }))
                    .test_support(),
            )
            .child(list.test_support())
            .child(foot);
        let mut scrim = gpui_kit::black();
        scrim.a = if dark { 0.45 } else { 0.25 };
        Some(
            div()
                .id("search-scrim")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(scrim)
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_search(window, cx)))
                .child(box_.test_support()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{is_toggle, rank, Entry, Target};
    use crate::shell::pages::AppPage;
    use gpui_kit::{Focusable, Keystroke, Modifiers};

    /// The icon opens search with its input focused; typing ranks the open destination first;
    /// Esc closes it; Cmd+K (or Ctrl+K) opens it from anywhere and closes it again.
    #[gpui_kit::test]
    fn search_opens_ranks_and_closes(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        use gpui_kit::AppContext;
        let (handle, _chats) = super::super::thread::tests::open(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("search-btn").label(), Some("Search"));
            window.click("search-btn", cx);
            window.render_frame(cx);
            assert!(window.try_find("search-menu").is_some(), "the menu opens");
            assert_eq!(window.find(("search-row", 0usize)).selected(), Some(true), "the first result highlighted");
        })
        .unwrap();
        let first = handle
            .update(cx, |c, window, cx| {
                let s = c.search_ui.as_ref().unwrap();
                assert!(s.input.read(cx).focus_handle(cx).is_focused(window), "the input has focus");
                let input = s.input.clone();
                input.update(cx, |st, cx| st.set_value("m", window, cx));
                let (_, groups, _) = c.search_results(cx);
                groups.iter().map(|g| g.label.clone()).collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(first.first().map(String::as_str), Some("Streams"), "on Streams, streams first: {first:?}");
        cx.simulate_keystrokes(handle.into(), "escape");
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("search-menu").is_none(), "Esc closes");
        })
        .unwrap();
        let key = if cfg!(target_os = "macos") { "cmd-k" } else { "ctrl-k" };
        cx.simulate_keystrokes(handle.into(), key);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("search-menu").is_some(), "the shortcut opens it");
        })
        .unwrap();
        cx.simulate_keystrokes(handle.into(), key);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("search-menu").is_none(), "and closes it again");
        })
        .unwrap();
    }

    fn e(title: &str, home: Option<AppPage>, text: &str) -> Entry {
        Entry { title: title.into(), sub: String::new(), text: text.into(), home, icon: "x", example: false, action: false, target: Target::Note }
    }

    /// The open destination's group comes first, the rest under Elsewhere; matching ignores case
    /// and is a substring; a title that starts with the query beats one that contains it, which
    /// beats a match in the text.
    #[test]
    fn the_current_destination_ranks_first() {
        let entries = vec![
            e("Pricing page", Some(AppPage::Streams), ""),
            e("Round 3 build", Some(AppPage::Streams), "fix the PR checks"),
            e("PR 43 CI", Some(AppPage::Streams), ""),
            e("Pele", Some(AppPage::Agents), ""),
            e("Fix policy tests", Some(AppPage::Library), "a PR note"),
            e("Prune notes", None, ""),
        ];
        let g = rank("pr", AppPage::Streams, &entries);
        assert_eq!(g.iter().map(|g| g.label.as_str()).collect::<Vec<_>>(), ["Streams", "Elsewhere"]);
        assert_eq!(g[0].items, [0, 2, 1], "starts with, then the text match");
        assert_eq!(g[1].items, [5, 4], "the rest ranked, not filtered out");
        let g = rank("PR", AppPage::Agents, &entries);
        assert_eq!(g[0].label, "Elsewhere", "nothing on Agents matches, so only Elsewhere");
        assert_eq!(rank("PELE", AppPage::Agents, &entries)[0].items, [3], "case does not matter");
        assert!(rank("zzz", AppPage::Streams, &entries).is_empty());
    }

    #[test]
    fn nothing_typed_gives_recent_actions_and_places() {
        let mut entries = vec![
            Entry { target: Target::Stream("a".into()), ..e("A", Some(AppPage::Streams), "") },
            Entry { action: true, target: Target::NewStream, ..e("New stream", Some(AppPage::Streams), "") },
            Entry { target: Target::Page(AppPage::Streams), ..e("Streams", None, "") },
            Entry { target: Target::Page(AppPage::Library), ..e("Library", None, "") },
        ];
        let g = rank("", AppPage::Streams, &entries);
        assert_eq!(g.iter().map(|g| (g.label.as_str(), g.items.clone())).collect::<Vec<_>>(), [("Recent", vec![0]), ("Actions in Streams", vec![1]), ("Go to", vec![3])]);
        entries.remove(1);
        assert_eq!(rank("", AppPage::Agents, &entries).iter().map(|g| g.label.as_str()).collect::<Vec<_>>(), ["Recent", "Go to"], "no actions there, no empty group");
    }

    /// Cmd+K on a Mac; Ctrl+K elsewhere, except in a terminal, where Ctrl+Shift+K still works.
    #[test]
    fn the_shortcut_by_platform() {
        let k = |platform, control, shift| Keystroke { key: "k".into(), modifiers: Modifiers { platform, control, shift, ..Default::default() }, key_char: None };
        assert!(is_toggle(&k(true, false, false), true, false));
        assert!(!is_toggle(&k(false, true, false), true, false), "Ctrl+K on a Mac is the terminal's");
        assert!(is_toggle(&k(false, true, false), false, false));
        assert!(!is_toggle(&k(false, true, false), false, true), "a terminal keeps Ctrl+K");
        assert!(is_toggle(&k(false, true, true), false, true), "Ctrl+Shift+K works there");
    }
}
