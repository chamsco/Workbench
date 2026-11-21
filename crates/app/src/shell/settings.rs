//! Settings (v3 frame 7): the sidebar's list gives way to Settings' own pages, with a back row on
//! top, and the page takes the card. Rows are a label, one line of help and a control, separated by
//! hairlines. The first page is Agents and subscriptions, read from core (`accounts.json` and the
//! prefs); where core has no number, the row says "No data yet". The other pages are empty states.

use super::*;
use backspace_core::accounts::{Accounts, FailoverPolicy};

/// Settings' pages, in the sidebar's order: (name, icon).
pub(crate) const PAGES: [(&str, &str); 5] = [
    ("Agents and subscriptions", "bot"),
    ("General", "settings"),
    ("Appearance", "palette"),
    ("Machines", "monitor"),
    ("Shortcuts", "keyboard"),
];

fn policy_label(p: FailoverPolicy) -> &'static str {
    match p {
        FailoverPolicy::Automatic => "Switch and resume",
        FailoverPolicy::Manual => "Ask me",
        FailoverPolicy::Off => "Off",
    }
}

impl Cauldron {
    /// Saves the stream card's background (None: the glass) to the shared prefs.
    pub(super) fn set_stream_bg(&mut self, path: Option<std::path::PathBuf>, cx: &mut Context<Self>) {
        let mut p = Prefs::load();
        p.stream_bg = path.as_ref().map(|p| p.display().to_string());
        p.save();
        self.stream_bg = super::backdrop::stream_bg(&p);
        // The frame follows the picture: bake its layers, or go back to the painting's.
        match self.stream_bg.clone() {
            Some(path) => Self::warm_picture(path, cx),
            None => super::backdrop::clear_user(),
        }
        cx.notify();
    }

    /// Asks for a picture or GIF for the stream card; anything that is not one is ignored.
    pub(super) fn pick_stream_bg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Use as stream background".into()) });
        cx.spawn_in(window, async move |this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
            if let Ok(Ok(Some(ps))) = paths.await {
                let Some(path) = ps.into_iter().next() else { return };
                let ok = path.extension().and_then(|e| e.to_str()).is_some_and(|e| super::backdrop::MEDIA_EXT.contains(&e.to_ascii_lowercase().as_str()));
                if ok {
                    let _ = this.update(cx, |c, cx| c.set_stream_bg(Some(path), cx));
                }
            }
        })
        .detach();
    }

    /// Where the subscriptions are read from: core's `accounts.json`, or a test's own file.
    pub(super) fn accounts_path(&self) -> std::path::PathBuf {
        self.accounts_file.clone().unwrap_or_else(Accounts::path)
    }

    /// The sidebar while Settings is open: a back row, then the pages.
    pub(super) fn settings_nav(&self, page: usize, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let (fg, fg2) = (c(pal.fg), c(pal.fg2));
        let mut nav = div().id("settings-nav").flex_1().min_h_0().flex().flex_col().gap(px(2.));
        nav = nav.child(
            div()
                .id("settings-back")
                .h(px(30.))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(fg)
                .aria_label("Back to threads")
                .child(glyph("chevron-left", 15., fg))
                .child("Settings")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings = None;
                    cx.notify();
                }))
                .test_support(),
        );
        nav = nav.child(div().h(px(8.)));
        for (i, (name, icon)) in PAGES.iter().enumerate() {
            let on = i == page;
            nav = nav.child(
                div()
                    .id(("settings-page", i))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(8.))
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .when_live(on, |d| d.bg(c(pal.selected)))
                    .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                    .text_size(px(13.))
                    .text_color(if on { fg } else { fg2 })
                    .aria_label(*name)
                    .aria_selected(on)
                    .child(glyph(icon, 15., if on { fg } else { fg2 }))
                    .child(div().min_w_0().truncate().child(*name))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings = Some(i);
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        nav
    }

    /// The open Settings page, in the card.
    pub(super) fn settings_page(&self, page: usize, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let line = c(pal.line);
        let (name, _) = PAGES[page.min(PAGES.len() - 1)];
        let mut col = div().w_full().max_w(px(660.)).flex().flex_col().px(px(24.)).py(px(8.));
        col = col.child(div().text_size(px(20.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).mt(px(12.)).mb(px(4.)).child(name));
        // A row: label and one line of help on the left, the control on the right.
        let row = |id: (&'static str, usize), label: String, help: String, control: AnyElement| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(16.))
                .py(px(8.))
                .border_t_1()
                .border_color(line)
                .aria_label(label.clone())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().text_size(px(13.)).text_color(fg).child(label))
                        .child(div().text_size(px(12.)).line_height(px(16.)).text_color(fg3).child(help)),
                )
                .child(div().flex_none().child(control))
                .test_support()
        };
        let section = |title: &'static str| div().mt(px(24.)).mb(px(4.)).text_size(px(11.)).line_height(px(14.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child(title);
        let value = |text: String| div().h(px(28.)).px(px(8.)).flex().items_center().rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line_strong)).text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(fg2).child(text);
        if page == 2 {
            // Appearance: the backdrop (Tight tiles rule 7) or the plain frame, saved to the shared
            // `backdrop` pref the old app reads too.
            col = col.child(section("Look"));
            let mut seg = div().id("set-mode").flex().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line_strong));
            for (i, (label, key)) in [("System", "system"), ("Light", "light"), ("Dark", "dark")].into_iter().enumerate() {
                let on = if self.theme_mode.is_empty() { key == "system" } else { self.theme_mode == key };
                seg = seg.child(
                    div()
                        .id(("mode-opt", i))
                        .h(px(24.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .rounded(px(radius::TAG))
                        .cursor_pointer()
                        .when_live(on, |d| d.bg(c(pal.selected)))
                        .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                        .aria_selected(on)
                        .aria_label(label)
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if on { fg } else { fg2 })
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.theme_mode = key.to_string();
                            let mut p = Prefs::load();
                            p.theme = key.to_string();
                            p.save();
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            col = col.child(row(("appearance", 4), "Mode".into(), "Follow the system, or always light or dark.".into(), seg.test_support().into_any_element()));
            col = col.child(section("Window"));
            let mut seg = div().id("set-backdrop").flex().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line_strong));
            for (i, (label, on)) in [("Painting", self.painting), ("Plain", !self.painting)].into_iter().enumerate() {
                seg = seg.child(
                    div()
                        .id(("backdrop-opt", i))
                        .h(px(24.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .rounded(px(radius::TAG))
                        .cursor_pointer()
                        .when_live(on, |d| d.bg(c(pal.selected)))
                        .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                        .aria_selected(on)
                        .aria_label(label)
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if on { fg } else { fg2 })
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.painting = i == 0;
                            if this.painting {
                                Self::warm_glass(cx);
                            }
                            let mut p = Prefs::load();
                            p.backdrop = if this.painting { super::backdrop::PAINTING.into() } else { super::backdrop::PLAIN.into() };
                            p.save();
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            col = col.child(row(("appearance", 0), "Backdrop".into(), "The Harbour painting behind a frosted frame, or the plain frame.".into(), seg.test_support().into_any_element()));
            // How frosted the frame and sidebar are.
            let mut seg = div().id("set-frost").flex().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line_strong));
            for (i, level) in super::backdrop::FrostLevel::ALL.into_iter().enumerate() {
                let on = level == self.frost;
                seg = seg.child(
                    div()
                        .id(("frost-opt", i))
                        .h(px(24.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .rounded(px(radius::TAG))
                        .cursor_pointer()
                        .when_live(on, |d| d.bg(c(pal.selected)))
                        .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                        .aria_selected(on)
                        .aria_label(level.name())
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if on { fg } else { fg2 })
                        .child(level.name())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.frost = level;
                            let mut p = Prefs::load();
                            p.frost = Some(level.pref());
                            p.save();
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            col = col.child(row(("appearance", 2), "Frost".into(), "How much of the painting shows through the frame and sidebar. The cards keep their light glass.".into(), seg.test_support().into_any_element()));
            // Windows only: the window controls as a slim strip across the top (the default), or in a
            // notch cut into the top-right card (Charles, 20 Nov).
            if platform() != cauldron_chrome::layout::Platform::Mac {
                let notch = platform() == cauldron_chrome::layout::Platform::WindowsNotch;
                let mut seg = div().id("set-window").flex().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line_strong));
                for (i, name) in ["Top strip", "Notch"].into_iter().enumerate() {
                    let on = (i == 1) == notch;
                    seg = seg.child(
                        div()
                            .id(("window-opt", i))
                            .h(px(24.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .rounded(px(radius::TAG))
                            .cursor_pointer()
                            .when_live(on, |d| d.bg(c(pal.selected)))
                            .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                            .aria_selected(on)
                            .aria_label(name)
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if on { fg } else { fg2 })
                            .child(name)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                set_notch(i == 1);
                                let mut p = Prefs::load();
                                p.window_style = (i == 1).then(|| "notch".to_string());
                                p.save();
                                this.motion = Default::default();
                                cx.notify();
                            }))
                            .test_support(),
                    );
                }
                col = col.child(row(("appearance", 3), "Window controls".into(), "A slim strip across the top of the window, or a notch in the top-right corner of the stream card.".into(), seg.test_support().into_any_element()));
            }
            // The stream card: the light frost of the painting, or the user's own picture or GIF.
            let mut seg = div().id("set-stream-bg").flex().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line_strong));
            let pic = self.stream_bg.is_some();
            for (i, (label, on)) in [("Glass", !pic), ("Picture…", pic)].into_iter().enumerate() {
                seg = seg.child(
                    div()
                        .id(("stream-bg-opt", i))
                        .h(px(24.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .rounded(px(radius::TAG))
                        .cursor_pointer()
                        .when_live(on, |d| d.bg(c(pal.selected)))
                        .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                        .aria_selected(on)
                        .aria_label(label)
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if on { fg } else { fg2 })
                        .child(label)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if i == 0 {
                                this.set_stream_bg(None, cx);
                            } else {
                                this.pick_stream_bg(window, cx);
                            }
                        }))
                        .test_support(),
                );
            }
            let help: String = match &self.stream_bg {
                Some(p) => format!("Showing {} under a scrim; the frame takes its colours. Only over the painting.", p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())),
                None => "The painting under a light frost, or your own picture or GIF (png, jpg, webp, gif).".into(),
            };
            col = col.child(row(("appearance", 1), "Stream background".into(), help, seg.test_support().into_any_element()));
            if pic {
                let cur = Prefs::load();
                let mut seg = div().id("set-stream-dim").flex().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line_strong));
                for (i, (name, a)) in super::backdrop::DIMS.into_iter().enumerate() {
                    let on = (super::backdrop::dim_alpha(&cur) - a).abs() < 0.001;
                    seg = seg.child(
                        div()
                            .id(("stream-dim-opt", i))
                            .h(px(24.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .rounded(px(radius::TAG))
                            .cursor_pointer()
                            .when_live(on, |d| d.bg(c(pal.selected)))
                            .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                            .aria_selected(on)
                            .aria_label(name)
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if on { fg } else { fg2 })
                            .child(name[..1].to_uppercase() + &name[1..])
                            .on_click(cx.listener(move |_, _, _, cx| {
                                let mut p = Prefs::load();
                                p.stream_dim = Some(name.to_string());
                                p.save();
                                cx.notify();
                            }))
                            .test_support(),
                    );
                }
                col = col.child(row(("appearance", 3), "Picture dimming".into(), "How dark the scrim over your picture is, so the text stays readable.".into(), seg.test_support().into_any_element()));
            }
            // The New stream box's model picker: the tray unfolds (A), or a card stacked behind the box.
            let stacked = super::subs::tray_stacked();
            let mut seg = div().id("set-model-tray").flex().gap(px(2.)).p(px(2.)).rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line_strong));
            for (i, name) in ["Unfold", "Stacked"].into_iter().enumerate() {
                let on = (i == 1) == stacked;
                seg = seg.child(
                    div()
                        .id(("model-tray-opt", i))
                        .h(px(24.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .rounded(px(radius::TAG))
                        .cursor_pointer()
                        .when_live(on, |d| d.bg(c(pal.selected)))
                        .when_live(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                        .aria_selected(on)
                        .aria_label(name)
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if on { fg } else { fg2 })
                        .child(name)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let mut p = Prefs::load();
                            p.model_tray = (i == 1).then(|| "stacked".to_string());
                            p.save();
                            this.start.route_menu = None;
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            col = col.child(row(("appearance", 5), "Model picker".into(), "On New stream: the tray under the box unfolds, or a card stacked behind the box drops out below it.".into(), seg.test_support().into_any_element()));
            return div().id("settings-body").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element();
        }
        if page == 1 {
            // General: what is real today. Updates is a pref the old app reads too.
            let p = Prefs::load();
            col = col.child(section("Updates"));
            let on = p.check_updates;
            let toggle = div()
                .id("set-updates")
                .cursor_pointer()
                .hover(|d| d.opacity(0.85))
                .aria_label(if on { "Check for updates: on" } else { "Check for updates: off" })
                .aria_selected(on)
                .child(value(if on { "On".into() } else { "Off".into() }))
                .on_click(cx.listener(|_, _, _, cx| {
                    let mut p = Prefs::load();
                    p.check_updates = !p.check_updates;
                    p.save();
                    cx.notify();
                }))
                .test_support();
            col = col.child(row(("general", 0), "Check for updates".into(), "Look for a newer Cauldron when it starts.".into(), toggle.into_any_element()));
            col = col.child(section("Data"));
            col = col.child(row(("general", 1), "Data folder".into(), "Where streams, accounts and settings are kept.".into(), value(Prefs::path().as_deref().and_then(|p| p.parent()).map_or_else(|| "Unknown".into(), |d| d.display().to_string())).into_any_element()));
            col = col.child(row(("general", 2), "Version".into(), "This build.".into(), value(env!("CARGO_PKG_VERSION").to_string()).into_any_element()));
            return div().id("settings-body").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element();
        }
        if page == 3 {
            // Machines: the connected machines from the prefs; adding one is still the old app's job.
            let p = Prefs::load();
            col = col.child(div().text_size(px(13.)).text_color(fg3).mb(px(6.)).child("Where a stream can run. This machine is always there."));
            col = col.child(section("Machines"));
            col = col.child(row(("machine", 0), "This machine".into(), "Runs streams with the CLIs installed here.".into(), value("Here".into()).into_any_element()));
            for (i, m) in p.machines.iter().enumerate() {
                col = col.child(row(("machine", i + 1), m.name.clone(), m.url.clone(), value("Connected".into()).into_any_element()));
            }
            if p.machines.is_empty() {
                col = col.child(div().id("settings-empty").mt(px(8.)).text_size(px(13.)).text_color(fg3).aria_label("No other machines connected.").child("No other machines connected.").test_support());
            }
            return div().id("settings-body").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element();
        }
        if page == 4 {
            let cmd = if cfg!(target_os = "macos") { "\u{2318}" } else { "Ctrl+" };
            col = col.child(section("Keys"));
            for (i, (label, help, keys)) in [
                ("New stream", "Open an empty stream.", format!("{cmd}N")),
                ("Terminal", "Show or hide the terminal under the stream.", format!("{cmd}J")),
                ("Send", "Send the message in the box.", "Enter".to_string()),
                ("New line", "A line break in the box.", "Shift+Enter".to_string()),
            ]
            .into_iter()
            .enumerate()
            {
                col = col.child(row(("keys", i), label.into(), help.into(), value(keys).into_any_element()));
            }
            return div().id("settings-body").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element();
        }
        if page != 0 {
            col = col.child(div().id("settings-empty").mt(px(8.)).text_size(px(13.)).text_color(fg3).aria_label(format!("{name} settings arrive in a later milestone.")).child(format!("{name} settings arrive in a later milestone.")).test_support());
            return div().id("settings-body").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element();
        }
        col = col.child(div().text_size(px(13.)).text_color(fg3).mb(px(6.)).child("Who answers in your threads, and the subscriptions they run on."));

        // Subscriptions (board "Accounts and looks", A): a row per coding CLI with its logins under it.
        let accounts = Accounts::load(&self.accounts_path());
        col = col.child(section("Subscriptions"));
        col = col.child(self.subscription_rows(pal, cx));

        // Agents: who plans and who builds (prefs), the CLI's effort (cycles, as the composer's
        // chip does) and what happens when a subscription hits its limit (cycles, saved to accounts.json).
        let p = Prefs::load();
        col = col.child(section("Agents"));
        col = col.child(row(("agent", 0), "Orchestrator".into(), "Plans, reviews and talks to you.".into(), value(p.planner.clone().unwrap_or_else(|| "Not set".into())).into_any_element()));
        col = col.child(row(("agent", 1), "Executor".into(), "Builds each ticket.".into(), value(p.worker.clone().unwrap_or_else(|| "The router picks".into())).into_any_element()));
        let effort = div()
            .id("set-effort")
            .cursor_pointer()
            .hover(|d| d.opacity(0.85))
            .aria_label("Change effort")
            .child(value(p.cli_effort.clone().map_or_else(|| "Auto".into(), |e| e[..1].to_uppercase() + &e[1..])))
            .on_click(cx.listener(|_, _, _, cx| {
                let mut p = Prefs::load();
                let at = thread::EFFORTS.iter().position(|e| *e == p.cli_effort.as_deref()).unwrap_or(0);
                p.cli_effort = thread::EFFORTS[(at + 1) % thread::EFFORTS.len()].map(str::to_string);
                p.save();
                cx.notify();
            }))
            .test_support();
        col = col.child(row(("agent", 2), "Effort".into(), "How hard the CLI thinks before it answers.".into(), effort.into_any_element()));
        let policy = div()
            .id("set-failover")
            .cursor_pointer()
            .hover(|d| d.opacity(0.85))
            .aria_label("Change failover")
            .child(value(policy_label(accounts.policy).into()))
            .on_click(cx.listener(|this, _, _, cx| {
                let path = this.accounts_path();
                let mut a = Accounts::load(&path);
                a.policy = match a.policy {
                    FailoverPolicy::Manual => FailoverPolicy::Automatic,
                    FailoverPolicy::Automatic => FailoverPolicy::Off,
                    FailoverPolicy::Off => FailoverPolicy::Manual,
                };
                let _ = a.save(&path);
                cx.notify();
            }))
            .test_support();
        col = col.child(row(("agent", 3), "When a subscription hits its limit".into(), "Switch to the next account and resume, ask you first, or leave accounts alone.".into(), policy.into_any_element()));
        div().id("settings-body").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::super::thread::tests::open;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};

    /// The gear opens Settings: its pages take the sidebar's place under a back row, and the
    /// first page fills the card with Agents and subscriptions. With no usage from core the row
    /// says "No data yet"; the back row returns to the threads.
    #[gpui_kit::test]
    fn settings_swaps_the_sidebar_list_and_fills_the_card(cx: &mut TestAppContext) {
        let _prefs = super::thread::tests::prefs_guard();
        let (handle, _chats) = open(cx);
        // Never the user's own accounts.json: a file of the test's own, absent.
        let path = std::env::temp_dir().join(format!("cauldron-settings-none-{}.json", std::process::id()));
        handle.update(cx, |c, _, _| c.accounts_file = Some(path)).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("settings", cx);
            window.render_frame(cx);
            assert!(window.try_find(("thread", 0usize)).is_none() && window.try_find("new-thread").is_none(), "the thread list gives way");
            assert_eq!(window.find(("sub", 0usize)).label(), Some("No coding CLI found on this machine"));
            assert_eq!(window.find("settings-back").label(), Some("Back to threads"));
            assert_eq!(window.find(("settings-page", 0usize)).label(), Some("Agents and subscriptions"));
            assert_eq!(window.find(("settings-page", 0usize)).selected(), Some(true));
            let card = super::super::tests::rect(window, "card");
            let page = super::super::tests::rect(window, "settings-body");
            assert!(page.x >= card.x && page.right() <= card.right(), "the page is in the card");
            assert_eq!(window.find(("agent", 2usize)).label(), Some("Effort"));
            window.click(("settings-page", 3usize), cx);
            assert_eq!(window.find(("machine", 0usize)).label(), Some("This machine"));
            window.click(("settings-page", 4usize), cx);
            assert_eq!(window.find(("keys", 0usize)).label(), Some("New stream"));
            window.click(("settings-page", 1usize), cx);
            assert_eq!(window.find(("general", 0usize)).label(), Some("Check for updates"));
            let before = backspace_core::prefs::Prefs::load().check_updates;
            window.click("set-updates", cx);
            assert_eq!(backspace_core::prefs::Prefs::load().check_updates, !before, "the toggle saves");
            window.click(("settings-page", 2usize), cx);
            assert_eq!(window.find(("appearance", 0usize)).label(), Some("Backdrop"));
            window.click(("mode-opt", 2usize), cx);
            assert_eq!(backspace_core::prefs::Prefs::load().theme, "dark", "Mode saves");
            window.click(("mode-opt", 0usize), cx);
            assert_eq!(backspace_core::prefs::Prefs::load().theme, "system");
            assert!(window.try_find("rail").is_none(), "no right rail in Settings: it belongs to an open stream");
            // Windows only (Charles, 20 Nov): the top strip by default, or the notch.
            let windows = crate::shell::platform() != cauldron_chrome::layout::Platform::Mac;
            assert_eq!(window.try_find(("appearance", 3usize)).is_some(), windows, "Window controls is a Windows setting");
            if windows {
                assert_eq!(window.find(("window-opt", 0usize)).selected(), Some(true), "the strip is the default");
            }
            assert_eq!(window.find(("backdrop-opt", 1usize)).selected(), Some(true), "plain by default");
            window.click(("backdrop-opt", 0usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find(("backdrop-opt", 0usize)).selected(), Some(true), "Painting picked");
            assert!(window.try_find("backdrop").is_some(), "the painting is drawn");
            assert_eq!(backspace_core::prefs::Prefs::load().backdrop, "painting", "and saved");
            window.click(("backdrop-opt", 1usize), cx);
            window.render_frame(cx);
            assert!(window.try_find("backdrop").is_none(), "Plain: no painting");
            assert_eq!(backspace_core::prefs::Prefs::load().backdrop, "plain");
            window.click("settings-back", cx);
            window.render_frame(cx);
            assert!(window.try_find("settings-body").is_none(), "the card is back");
            assert!(window.try_find(("thread", 0usize)).is_some(), "the thread list is back");
        })
        .unwrap();
        let _ = handle.update(cx, |_, _, _| ());
    }

    /// Subscriptions list each CLI with its logins; one with nothing known says so rather than
    /// showing a made-up state, and an unsigned login offers Sign in.
    #[gpui_kit::test]
    fn subscriptions_list_each_cli_with_its_logins(cx: &mut TestAppContext) {
        use backspace_core::accounts::{CliStatus, LoginStatus};
        use backspace_core::harnesses::Auth;
        let (handle, _chats) = open(cx);
        let path = std::env::temp_dir().join(format!("cauldron-settings-{}.json", std::process::id()));
        handle.update(cx, move |c, _, _| {
            c.accounts_file = Some(path);
            c.start.providers = vec![CliStatus {
                provider: "claude".into(),
                name: "Claude".into(),
                installed: true,
                version: Some("2.1.0".into()),
                install: String::new(),
                logins: vec![
                    LoginStatus { id: None, name: "This machine's login".into(), signed_in: Auth::Authenticated, detail: None },
                    LoginStatus { id: Some("work".into()), name: "Work".into(), signed_in: Auth::Unauthenticated, detail: None },
                ],
                active: 0,
                models: Vec::new(),
                model_info: Vec::new(),
            }];
        })
        .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("settings", cx);
            window.render_frame(cx);
            assert_eq!(window.find(("sub-cli", 0usize)).label(), Some("Claude: 2.1.0 · 2 logins"));
            assert_eq!(window.find(("sub-login", 1usize)).label(), Some("Work: Not signed in"));
            assert_eq!(window.find(("sub-signin", 1usize)).label(), Some("Sign in"));
        })
        .unwrap();
    }
}
