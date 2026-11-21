//! The version button's update check (Charles, 20 Nov: "the version button clicking should let
//! the app search if there's a new release to be able to update"). A click opens a small card
//! above the button and asks GitHub's rolling release which commit it was built from
//! (`backspace_core::update`). On Windows "Update and restart" downloads Cauldron.exe, swaps it
//! for the running one and starts it; elsewhere the release page opens.

use super::*;
use backspace_core::cli_update as upd_cli;
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

/// Where one coding CLI stands in the update list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RowState {
    Checking,
    Current,
    /// A newer version is out.
    Newer(String),
    /// Not from npm, or npm could not be asked: the reason, in the row's own words.
    Manual(&'static str),
    Missing,
    Updating,
    Done(String),
    Failed(String),
}

#[derive(Clone, Debug)]
pub(crate) struct CliRow {
    pub provider: String,
    pub name: String,
    pub version: Option<String>,
    pub pkg: Option<String>,
    pub state: RowState,
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
        self.cli_rows = self
            .start
            .providers
            .iter()
            .map(|p| {
                let pkg = upd_cli::npm_package(&p.install);
                let state = match (p.installed, &pkg) {
                    (false, _) => RowState::Missing,
                    (true, None) => RowState::Manual("Updates itself"),
                    (true, Some(_)) => RowState::Checking,
                };
                CliRow { provider: p.provider.clone(), name: p.name.clone(), version: p.version.clone(), pkg, state }
            })
            .collect();
        cx.notify();
        if cfg!(test) {
            return;
        }
        // Debug builds only, for screenshots: made-up answers instead of GitHub's and npm's.
        #[cfg(debug_assertions)]
        if std::env::var_os("CAULDRON_FIXTURE_UPDATES").is_some() {
            self.update = UpdateState::Ready(UpdateInfo { current: upd::VERSION.into(), latest: "9f3c2ab".into(), newer: true, url: String::new(), build: "77950f0".into(), installs: true, changes: Vec::new(), more: 0, changes_url: String::new() });
            let row = |provider: &str, name: &str, v: Option<&str>, pkg: Option<&str>, state| CliRow { provider: provider.into(), name: name.into(), version: v.map(Into::into), pkg: pkg.map(Into::into), state };
            self.cli_rows = vec![
                row("claude", "Claude", Some("2.0.4"), Some("@anthropic-ai/claude-code"), RowState::Newer("2.1.0".into())),
                row("codex", "Codex", Some("0.46.0"), Some("@openai/codex"), RowState::Newer("0.48.0".into())),
                row("opencode", "OpenCode", Some("1.18.16"), Some("opencode-ai"), RowState::Current),
                row("gemini", "Gemini", None, Some("@google/gemini-cli"), RowState::Missing),
                row("cursor", "Cursor", Some("2025.10"), None, RowState::Manual("Updates itself")),
                row("grok", "Grok", None, None, RowState::Missing),
            ];
            return;
        }
        for r in self.cli_rows.iter().filter(|r| r.state == RowState::Checking) {
            let (provider, pkg, have) = (r.provider.clone(), r.pkg.clone().unwrap_or_default(), r.version.clone());
            let job = cx.background_executor().spawn(async move { upd_cli::latest_blocking(&pkg) });
            cx.spawn(async move |this, cx| {
                let latest = job.await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(row) = this.cli_rows.iter_mut().find(|r| r.provider == provider) {
                        row.state = match (&latest, &have) {
                            (Some(l), Some(h)) if upd_cli::newer(h, l) => RowState::Newer(l.clone()),
                            (Some(_), _) => RowState::Current,
                            (None, _) => RowState::Manual("Couldn't reach npm"),
                        };
                    }
                    cx.notify();
                });
            })
            .detach();
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

    /// How many things "Update all" would update: each CLI with a newer version, and the app.
    pub(super) fn updates_pending(&self) -> usize {
        let app = matches!(&self.update, UpdateState::Ready(i) if i.newer) as usize;
        app + self.cli_rows.iter().filter(|r| matches!(r.state, RowState::Newer(_))).count()
    }

    /// Updates every CLI that has a newer version, side by side; the app goes last, once they are
    /// all in place, because it restarts.
    pub(super) fn update_all(&mut self, cx: &mut Context<Self>) {
        let todo: Vec<(String, String, String)> = self
            .cli_rows
            .iter()
            .filter_map(|r| match (&r.state, &r.pkg) {
                (RowState::Newer(l), Some(p)) => Some((r.provider.clone(), p.clone(), l.clone())),
                _ => None,
            })
            .collect();
        for (provider, _, _) in &todo {
            if let Some(r) = self.cli_rows.iter_mut().find(|r| &r.provider == provider) {
                r.state = RowState::Updating;
            }
        }
        cx.notify();
        if todo.is_empty() {
            self.finish_updates(cx);
            return;
        }
        if cfg!(test) {
            return;
        }
        for (provider, pkg, latest) in todo {
            let job = cx.background_executor().spawn(async move { upd_cli::update_blocking(&pkg) });
            cx.spawn(async move |this, cx| {
                let res = job.await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(r) = this.cli_rows.iter_mut().find(|r| r.provider == provider) {
                        match res {
                            Ok(()) => {
                                r.version = Some(latest.clone());
                                r.state = RowState::Done(latest);
                            }
                            Err(e) => r.state = RowState::Failed(e.to_string()),
                        }
                    }
                    this.finish_updates(cx);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// When no CLI is still updating, the app's own update follows (it restarts).
    fn finish_updates(&mut self, cx: &mut Context<Self>) {
        if self.cli_rows.iter().any(|r| r.state == RowState::Updating) {
            return;
        }
        if matches!(&self.update, UpdateState::Ready(i) if i.newer) {
            self.install_update(cx);
        }
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
                Ok(exe) => match backspace_runner::cmd(&exe).env(backspace_core::instance::RESTART_ENV, "1").spawn() {
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

    /// One line per thing that can update: the app, then each coding CLI with its mark, its
    /// version and where it stands.
    #[inline(never)]
    fn update_rows(&self, pal: &'static Palette) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mut list = div().id("update-rows").flex().flex_col().mt(px(4.)).mb(px(2.));
        let row = |id: ElementId, mark: AnyElement, name: String, status: String, tone: Hsla| {
            div()
                .id(id)
                .h(px(28.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().w(px(16.)).flex_none().flex().justify_center().child(mark))
                .child(div().truncate().text_size(px(12.5)).text_color(fg).child(name.clone()))
                .child(div().flex_1().min_w_0().truncate().text_align(TextAlign::Right).text_size(px(11.5)).text_color(tone).child(status.clone()))
                .aria_label(format!("{name}, {status}"))
        };
        let (app_status, app_tone) = match &self.update {
            UpdateState::Ready(i) if i.newer => (format!("{} → {}", upd::VERSION, i.latest), c(pal.warn)),
            UpdateState::Ready(i) if i.build.is_empty() => ("Built by hand".to_string(), fg3),
            UpdateState::Ready(_) => (format!("{} · up to date", upd::VERSION), fg3),
            UpdateState::Failed(_) => ("Couldn't check".to_string(), c(pal.bad)),
            UpdateState::Installing => ("Updating…".to_string(), c(pal.accent)),
            _ => ("Checking…".to_string(), fg3),
        };
        list = list.child(row(ElementId::Name("update-row-app".into()), div().size(px(11.)).rounded_full().bg(c(pal.accent)).into_any_element(), "Cauldron".into(), app_status, app_tone).test_support());
        for (i, r) in self.cli_rows.iter().enumerate() {
            let have = r.version.clone().unwrap_or_default();
            let (status, tone) = match &r.state {
                RowState::Checking => ("Checking…".to_string(), fg3),
                RowState::Current => (format!("{have} · up to date"), fg3),
                RowState::Newer(l) => (format!("{have} → {l}"), c(pal.warn)),
                RowState::Manual(why) => (why.to_string(), fg3),
                RowState::Missing => ("Not installed".to_string(), fg3),
                RowState::Updating => ("Updating…".to_string(), c(pal.accent)),
                RowState::Done(v) => (format!("Updated to {v}"), c(pal.ok)),
                RowState::Failed(e) => (format!("Failed: {e}"), c(pal.bad)),
            };
            let mark = match super::logos::cli_mark(&r.provider).and_then(|n| super::logos::image(n, &(if r.state == RowState::Missing { pal.fg4 } else { pal.fg }).to_hex())) {
                Some(im) => img(im).size(px(14.)).into_any_element(),
                None => div().size(px(7.)).rounded_full().bg(fg2).into_any_element(),
            };
            list = list.child(row(ElementId::NamedInteger("update-row".into(), i as u64), mark, r.name.clone(), status, tone).test_support());
        }
        list.into_any_element()
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
                .shadow(super::lift(super::Lift::Button, pal))
                .when_live(primary, |d| d.bg(c(pal.accent)).text_color(c(pal.on_accent)))
                .when_live(!primary, |d| d.border_1().border_color(c(pal.line_strong)).text_color(fg).hover(|d| d.bg(c(pal.hover))))
                .aria_label(label)
                .child(label)
        };
        let mut actions = div().flex().items_center().gap(px(6.)).pt(px(4.));
        let pending = self.updates_pending();
        let busy = matches!(self.update, UpdateState::Installing) || self.cli_rows.iter().any(|r| r.state == RowState::Updating);
        if pending > 0 && !busy {
            let app_only = self.cli_rows.iter().all(|r| !matches!(r.state, RowState::Newer(_)));
            let label: String = match &self.update {
                UpdateState::Ready(i) if app_only => if i.installs { "Update and restart".into() } else { "Download".into() },
                _ => format!("Update {pending}"),
            };
            // The marks of what is about to update, so the button says which ones.
            let mut marks = div().flex().items_center().gap(px(-3.));
            for r in self.cli_rows.iter().filter(|r| matches!(r.state, RowState::Newer(_))) {
                if let Some(im) = super::logos::cli_mark(&r.provider).and_then(|n| super::logos::image(n, &pal.on_accent.to_hex())) {
                    marks = marks.child(img(im).size(px(13.)));
                }
            }
            actions = actions.child(
                div()
                    .id("update-install")
                    .h(px(28.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .shadow(super::lift(super::Lift::Button, pal))
                    .bg(c(pal.accent))
                    .text_color(c(pal.on_accent))
                    .aria_label(label.clone())
                    .child(marks)
                    .child(label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.cli_rows.iter().any(|r| matches!(r.state, RowState::Newer(_))) {
                            this.update_all(cx);
                        } else {
                            this.install_update(cx);
                        }
                    }))
                    .test_support(),
            );
        }
        if let UpdateState::Ready(i) = &self.update {
            if i.newer && !i.changes_url.is_empty() {
                let u = i.changes_url.clone();
                actions = actions.child(button("update-changes", "What's changed", false).on_click(cx.listener(move |_, _, _, cx| cx.open_url(&u))).test_support());
            }
        }
        if !busy && !matches!(self.update, UpdateState::Checking) && matches!(self.update, UpdateState::Ready(_) | UpdateState::Failed(_)) {
            actions = actions.child(button("update-again", "Check again", false).on_click(cx.listener(|this, _, _, cx| this.check_for_update(cx))).test_support());
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
            .w(px(300.))
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
            .child(self.update_rows(pal))
            .child(changes)
            .child(actions)
    }
}
