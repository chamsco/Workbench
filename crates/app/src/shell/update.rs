//! The version button's update check (Charles, 20 Nov: "the version button clicking should let
//! the app search if there's a new release to be able to update"). A click opens a small card
//! above the button and asks GitHub's rolling release which commit it was built from
//! (`backspace_core::update`). On Windows "Update and restart" downloads Cauldron.exe, swaps it
//! for the running one and starts it; elsewhere the release page opens.

use super::*;
use backspace_core::update::{self as upd, UpdateInfo};

#[derive(Clone, Debug, Default)]
pub(crate) enum UpdateState {
    #[default]
    Idle,
    Checking,
    Ready(UpdateInfo),
    Failed(String),
    Installing,
}

impl UpdateState {
    /// The card's headline and its smaller line.
    pub(crate) fn text(&self) -> (String, String) {
        match self {
            UpdateState::Idle | UpdateState::Checking => ("Checking for updates…".into(), format!("You have Cauldron {}.", upd::VERSION)),
            UpdateState::Ready(i) if i.newer => (
                "A new version is ready".into(),
                format!("Built from {} on main. You have {}{}.", i.latest, upd::VERSION, if i.build.is_empty() { String::new() } else { format!(" ({})", i.build) }),
            ),
            UpdateState::Ready(i) if i.build.is_empty() => ("This build can't update itself".into(), "It was built by hand, so it has no release to compare with.".into()),
            UpdateState::Ready(i) => ("You're up to date".into(), format!("Cauldron {} ({}) is the newest build.", upd::VERSION, i.build)),
            UpdateState::Failed(e) => ("Couldn't check for updates".into(), e.clone()),
            UpdateState::Installing => ("Downloading the update…".into(), "Cauldron restarts by itself when it is in place.".into()),
        }
    }
}

impl Cauldron {
    /// Opens the card and checks, unless a check or an install is already running.
    pub(super) fn check_for_update(&mut self, cx: &mut Context<Self>) {
        self.update_open = true;
        if matches!(self.update, UpdateState::Checking | UpdateState::Installing) {
            cx.notify();
            return;
        }
        self.update = UpdateState::Checking;
        cx.notify();
        if cfg!(test) {
            return;
        }
        let task = cx.background_executor().spawn(async { upd::check_blocking() });
        cx.spawn(async move |this, cx| {
            let res = task.await;
            let _ = this.update(cx, |this, cx| {
                this.update = match res {
                    Ok(i) => UpdateState::Ready(i),
                    Err(e) => UpdateState::Failed(format!("{e:#}")),
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// Windows: download, swap, start the new exe, quit. Elsewhere: the release page.
    fn install_update(&mut self, cx: &mut Context<Self>) {
        let UpdateState::Ready(info) = &self.update else { return };
        if !info.installs {
            cx.open_url(&info.url);
            return;
        }
        self.update = UpdateState::Installing;
        cx.notify();
        let task = cx.background_executor().spawn(async { upd::install_exe() });
        cx.spawn(async move |this, cx| {
            let res = task.await;
            let _ = this.update(cx, |this, cx| match res {
                Ok(exe) => match backspace_runner::cmd(&exe).spawn() {
                    Ok(_) => cx.quit(),
                    Err(e) => {
                        this.update = UpdateState::Failed(format!("The update is in place, but it didn't start: {e}. Open Cauldron again."));
                        cx.notify();
                    }
                },
                Err(e) => {
                    this.update = UpdateState::Failed(format!("{e:#}"));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The card above the version button.
    pub(super) fn update_card(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let (fg, fg2) = (c(pal.fg), c(pal.fg2));
        let (head, sub) = self.update.text();
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .h(px(28.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_size(px(12.))
                .when_live(primary, |d| d.bg(c(pal.accent)).text_color(c(pal.on_accent)))
                .when_live(!primary, |d| d.border_1().border_color(c(pal.line_strong)).text_color(fg).hover(|d| d.bg(c(pal.hover))))
                .aria_label(label)
                .child(label)
        };
        let mut actions = div().flex().gap(px(6.)).pt(px(4.));
        match &self.update {
            UpdateState::Ready(i) if i.newer => {
                let label = if i.installs { "Update and restart" } else { "Download" };
                actions = actions.child(button("update-install", label, true).on_click(cx.listener(|this, _, _, cx| this.install_update(cx))).test_support());
                if !i.changes_url.is_empty() {
                    let u = i.changes_url.clone();
                    actions = actions.child(button("update-changes", "What's changed", false).on_click(cx.listener(move |_, _, _, cx| cx.open_url(&u))).test_support());
                }
            }
            UpdateState::Ready(_) | UpdateState::Failed(_) => {
                actions = actions.child(button("update-again", "Check again", false).on_click(cx.listener(|this, _, _, cx| this.check_for_update(cx))).test_support());
            }
            _ => {}
        }
        let mut changes = div().flex().flex_col().gap(px(2.));
        if let UpdateState::Ready(i) = &self.update {
            for (k, ch) in i.changes.iter().take(5).enumerate() {
                let line = if ch.pr > 0 { format!("#{} {}", ch.pr, ch.title) } else { ch.title.clone() };
                changes = changes.child(div().id(("update-change", k)).truncate().text_size(px(11.5)).text_color(fg2).child(line).test_support());
            }
        }
        div()
            .id("update-card")
            .w(px(272.))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .rounded(px(radius::PANEL))
            .bg(c(pal.raised))
            .border_1()
            .border_color(c(pal.line))
            .shadow(vec![drop(tokens::shadow::MENU)])
            .occlude()
            .aria_label(head.clone())
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.update_open = false;
                cx.notify();
            }))
            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(head))
            .child(div().text_size(px(11.5)).line_height(px(16.)).text_color(fg2).child(sub))
            .child(changes)
            .child(actions)
    }
}
