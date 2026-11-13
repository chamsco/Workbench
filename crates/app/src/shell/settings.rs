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
    /// Where the subscriptions are read from: core's `accounts.json`, or a test's own file.
    fn accounts_path(&self) -> std::path::PathBuf {
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
            // Appearance: the new thread's painting (background A), as the Tauri app offered them.
            let p = Prefs::load();
            let name = super::backdrop::BACKDROPS.iter().find(|b| b.0 == p.backdrop).map_or("Harbour", |b| b.1);
            let bg = div()
                .id("set-backdrop")
                .cursor_pointer()
                .hover(|d| d.opacity(0.85))
                .aria_label("Change background")
                .child(value(name.into()))
                .on_click(cx.listener(|_, _, _, cx| {
                    let mut p = Prefs::load();
                    let all = super::backdrop::BACKDROPS;
                    let at = all.iter().position(|b| b.0 == p.backdrop).unwrap_or(0);
                    p.backdrop = all[(at + 1) % all.len()].0.to_string();
                    p.save();
                    cx.notify();
                }))
                .test_support();
            col = col.child(div().text_size(px(13.)).text_color(fg3).mb(px(6.)).child("How Cauldron looks around your work."));
            col = col.child(section("Background"));
            col = col.child(row(("look", 0), "Painting".into(), "Shown on the new thread, in the old dither, with the composer on frosted glass. Never behind a thread.".into(), bg.into_any_element()));
            return div().id("settings-body").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element();
        }
        if page != 0 {
            col = col.child(
                div()
                    .id("settings-empty")
                    .mt(px(8.))
                    .text_size(px(13.))
                    .text_color(fg3)
                    .aria_label(format!("{name} settings arrive in a later milestone."))
                    .child(format!("{name} settings arrive in a later milestone."))
                    .test_support(),
            );
            return div().id("settings-body").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element();
        }
        col = col.child(div().text_size(px(13.)).text_color(fg3).mb(px(6.)).child("Who answers in your threads, and the subscriptions they run on."));

        // Subscriptions: one row per login core knows (accounts.json). Usage only where the CLI reported it.
        let accounts = Accounts::load(&self.accounts_path());
        col = col.child(section("Subscriptions"));
        if accounts.list.is_empty() {
            col = col.child(row(("sub", 0), "No subscriptions on this machine yet".into(), "They appear here once a coding CLI account is added to Cauldron.".into(), div().into_any_element()));
        }
        for (i, a) in accounts.list.iter().enumerate() {
            let who = a.identity.clone().map_or_else(|| "Not signed in yet".to_string(), |w| format!("Signed in as {w}"));
            let help = format!("{} · {}", a.provider, who);
            let usage = match a.usage.as_ref().and_then(|u| Some((u.used?, u.limit?))) {
                Some((used, limit)) if limit > 0 => {
                    let left = 100u64.saturating_sub(used * 100 / limit);
                    let low = left < 20;
                    div()
                        .w(px(140.))
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(div().h(px(4.)).rounded(px(2.)).bg(c(pal.line_strong)).child(div().h_full().w(relative(left as f32 / 100.0)).rounded(px(2.)).bg(if low { c(pal.warn) } else { fg3 })))
                        .child(div().text_size(px(11.)).text_color(fg3).flex().justify_end().child(format!("{left}% left")))
                        .into_any_element()
                }
                _ => div().id(("sub-usage", i)).text_size(px(12.)).text_color(fg3).aria_label("No data yet").child("No data yet").test_support().into_any_element(),
            };
            col = col.child(row(("sub", i), a.name.clone(), help, usage));
        }

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
        let (handle, _chats) = open(cx);
        // Never the user's own accounts.json: a file of the test's own, absent.
        let path = std::env::temp_dir().join(format!("cauldron-settings-none-{}.json", std::process::id()));
        handle.update(cx, |c, _, _| c.accounts_file = Some(path)).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("settings", cx);
            window.render_frame(cx);
            assert!(window.try_find(("thread", 0usize)).is_none() && window.try_find("new-thread").is_none(), "the thread list gives way");
            assert_eq!(window.find(("sub", 0usize)).label(), Some("No subscriptions on this machine yet"));
            assert_eq!(window.find("settings-back").label(), Some("Back to threads"));
            assert_eq!(window.find(("settings-page", 0usize)).label(), Some("Agents and subscriptions"));
            assert_eq!(window.find(("settings-page", 0usize)).selected(), Some(true));
            let card = super::super::tests::rect(window, "card");
            let page = super::super::tests::rect(window, "settings-body");
            assert!(page.x >= card.x && page.right() <= card.right(), "the page is in the card");
            assert_eq!(window.find(("agent", 2usize)).label(), Some("Effort"));
            window.click(("settings-page", 1usize), cx);
            assert_eq!(window.find("settings-empty").label(), Some("General settings arrive in a later milestone."));
            window.click(("settings-page", 2usize), cx);
            assert_eq!(window.find(("look", 0usize)).label(), Some("Painting"), "Appearance: the background choice");
            window.click("settings-back", cx);
            window.render_frame(cx);
            assert!(window.try_find("settings-body").is_none(), "the card is back");
            assert!(window.try_find(("thread", 0usize)).is_some(), "the thread list is back");
        })
        .unwrap();
        let _ = handle.update(cx, |_, _, _| ());
    }

    /// Settings carries no plugins (Charles, 13 Nov): no drawer, no strip, no rail, no terminal, and
    /// the card runs to the Shell's padding; back in the threads they return.
    #[gpui_kit::test]
    fn settings_has_no_plugins(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        let rest = |cx: &mut TestAppContext| handle.update(cx, |c, _, _| c.motion = Default::default()).unwrap();
        handle.update(cx, |c, _, _| c.terminal = true).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("settings", cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            for id in ["drawer", "drawer-strip", "rrail", "terminal"] {
                assert!(window.try_find(id).is_none(), "no {id} in Settings");
            }
            let (card, shell) = (super::super::tests::rect(window, "card"), super::super::tests::rect(window, "shell"));
            assert_eq!(card.right(), shell.right() - 6.0, "the card takes the right gutter");
            window.click("settings-back", cx);
        })
        .unwrap();
        rest(cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("terminal").is_some(), "back in the thread, the terminal returns");
        })
        .unwrap();
    }

    /// A subscription with no usage from core shows "No data yet", never a made-up number.
    #[gpui_kit::test]
    fn a_subscription_without_usage_says_no_data_yet(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        // A file of the test's own, never the user's accounts.json.
        let path = std::env::temp_dir().join(format!("cauldron-settings-{}.json", std::process::id()));
        let mut a = backspace_core::accounts::Accounts::default();
        a.add("claude", "Work");
        a.save(&path).unwrap();
        let p2 = path.clone();
        handle.update(cx, move |c, _, _| c.accounts_file = Some(p2)).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("settings", cx);
            window.render_frame(cx);
            assert_eq!(window.find(("sub", 0usize)).label(), Some("Work"));
            assert_eq!(window.find(("sub-usage", 0usize)).label(), Some("No data yet"));
        })
        .unwrap();
        let _ = std::fs::remove_file(path);
    }
}
