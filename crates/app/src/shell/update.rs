//! The version button's update check (Charles, 20 Nov: "the version button clicking should let
//! the app search if there's a new release to be able to update"). A click opens a small card
//! above the button and asks GitHub's rolling release which commit it was built from
//! (`backspace_core::update`). On Windows "Update and restart" downloads Cauldron.exe, swaps it
//! for the running one and starts it; elsewhere the release page opens.

use super::*;
use backspace_core::cli_update as upd_cli;
use backspace_core::update::{self as upd, UpdateInfo};
use std::sync::atomic::{AtomicU64, Ordering};

/// How far the app's download has got: bytes so far and in all (0 until the size is known).
#[derive(Debug, Default)]
pub(crate) struct Dl {
    pub got: AtomicU64,
    pub total: AtomicU64,
}

impl Dl {
    /// 0.0 to 1.0; 0 while the size is unknown.
    pub(crate) fn fraction(&self) -> f32 {
        let t = self.total.load(Ordering::Relaxed);
        if t == 0 {
            0.0
        } else {
            (self.got.load(Ordering::Relaxed) as f32 / t as f32).clamp(0.0, 1.0)
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) enum UpdateState {
    #[default]
    Idle,
    Checking,
    Ready(UpdateInfo),
    Failed(String),
    /// The app's update is downloading in the background; nothing is replaced yet.
    Downloading(UpdateInfo, std::sync::Arc<Dl>),
    /// Downloaded and waiting: "Update ready", one click to restart into it.
    Staged(UpdateInfo),
    /// Swapping it in and restarting.
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
            UpdateState::Downloading(_, d) => {
                let mb = |b: u64| format!("{:.1} MB", b as f64 / 1_048_576.0);
                let (got, total) = (d.got.load(Ordering::Relaxed), d.total.load(Ordering::Relaxed));
                ("Downloading the update…".into(), if total > 0 { format!("{} of {}. You can keep working.", mb(got), mb(total)) } else { "Starting. You can keep working.".into() })
            }
            UpdateState::Staged(_) => ("Update ready".into(), "Restart Cauldron to finish it. Nothing changes until you do.".into()),
            UpdateState::Installing => ("Updating Cauldron…".into(), "It restarts by itself when it is in place.".into()),
        }
    }

    /// A newer app is out, being downloaded, or downloaded and waiting.
    pub(crate) fn app_newer(&self) -> bool {
        matches!(self, UpdateState::Ready(i) if i.newer) || matches!(self, UpdateState::Downloading(..) | UpdateState::Staged(_))
    }
}

impl Cauldron {
    /// Opens the card and checks, unless a check or an install is already running.
    pub(super) fn check_for_update(&mut self, cx: &mut Context<Self>) {
        self.update_open = true;
        if matches!(self.update, UpdateState::Checking | UpdateState::Installing | UpdateState::Downloading(..) | UpdateState::Staged(_)) {
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
        let app = self.update.app_newer() as usize;
        app + self.cli_rows.iter().filter(|r| matches!(r.state, RowState::Newer(_))).count()
    }

    /// Updates every CLI that has a newer version, side by side, and starts the app's download once
    /// they are all in place (the app never restarts by itself: it waits as "Update ready").
    pub(super) fn update_all(&mut self, cx: &mut Context<Self>) {
        self.run_cli_updates(None, true, cx);
    }

    /// Only the CLIs, the app untouched.
    pub(super) fn update_clis(&mut self, cx: &mut Context<Self>) {
        self.run_cli_updates(None, false, cx);
    }

    /// Only this CLI.
    pub(super) fn update_cli(&mut self, provider: &str, cx: &mut Context<Self>) {
        self.run_cli_updates(Some(provider.to_string()), false, cx);
    }

    fn run_cli_updates(&mut self, only: Option<String>, then_app: bool, cx: &mut Context<Self>) {
        let todo: Vec<(String, String, String)> = self
            .cli_rows
            .iter()
            .filter(|r| only.as_ref().is_none_or(|o| o == &r.provider))
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
            self.finish_updates(then_app, cx);
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
                    this.finish_updates(then_app, cx);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// When no CLI is still updating and "Update all" asked for it, the app's download starts.
    fn finish_updates(&mut self, then_app: bool, cx: &mut Context<Self>) {
        if self.cli_rows.iter().any(|r| r.state == RowState::Updating) {
            return;
        }
        if then_app {
            self.update_app(cx);
        }
    }

    /// The app only: downloads it in the background (Windows), or opens the release page; when it is
    /// already downloaded, restarts into it.
    pub(super) fn update_app(&mut self, cx: &mut Context<Self>) {
        match &self.update {
            UpdateState::Staged(_) => self.restart_into_update(cx),
            UpdateState::Ready(i) if i.newer => {
                if !i.installs {
                    cx.open_url(&i.url);
                } else {
                    self.start_download(cx);
                }
            }
            _ => {}
        }
    }

    /// Downloads the new exe beside the running one while the app keeps working; the rail's circle
    /// fills as it goes, and "Update ready" follows (Charles, 23 Nov).
    fn start_download(&mut self, cx: &mut Context<Self>) {
        let UpdateState::Ready(info) = &self.update else { return };
        let info = info.clone();
        let dl = std::sync::Arc::new(Dl::default());
        self.update = UpdateState::Downloading(info.clone(), dl.clone());
        cx.notify();
        if cfg!(test) {
            return;
        }
        // Debug builds only, for screenshots: a made-up download that fills over twelve seconds.
        #[cfg(debug_assertions)]
        if std::env::var_os("CAULDRON_FIXTURE_DOWNLOAD").is_some() {
            cx.spawn(async move |this, cx| {
                dl.total.store(80 * 1_048_576, Ordering::Relaxed);
                for i in 1..=100u64 {
                    cx.background_executor().timer(std::time::Duration::from_millis(120)).await;
                    dl.got.store(i * 80 * 1_048_576 / 100, Ordering::Relaxed);
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        return;
                    }
                }
                let _ = this.update(cx, |this, cx| {
                    this.update = UpdateState::Staged(info);
                    cx.notify();
                });
            })
            .detach();
            return;
        }
        let d2 = dl.clone();
        backspace_core::crash::note(&format!("update: downloading {}", info.latest));
        let task = cx.background_executor().spawn(async move {
            upd::download_exe(&move |got, total| {
                d2.got.store(got, Ordering::Relaxed);
                d2.total.store(total, Ordering::Relaxed);
            })
        });
        cx.spawn(async move |this, cx| {
            let res = task.await;
            match &res {
                Ok(p) => backspace_core::crash::note(&format!("update: downloaded to {}", p.display())),
                Err(e) => backspace_core::crash::note(&format!("update: download failed: {e:#}")),
            }
            let _ = this.update(cx, |this, cx| {
                this.update = match res {
                    Ok(_) => UpdateState::Staged(info),
                    Err(e) => UpdateState::Failed(format!("{e:#}")),
                };
                cx.notify();
            });
        })
        .detach();
        // Redraw while it downloads, so the ring and the card follow it.
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(150)).await;
            match this.update(cx, |this, cx| {
                let on = matches!(this.update, UpdateState::Downloading(..));
                if on {
                    cx.notify();
                }
                on
            }) {
                Ok(true) => {}
                _ => break,
            }
        })
        .detach();
    }

    /// Swaps the downloaded exe in and starts it. A window-wide card with a green bar covers the
    /// short wait (Charles, 23 Nov: "a green loading bar that fills up").
    pub(super) fn restart_into_update(&mut self, cx: &mut Context<Self>) {
        let UpdateState::Staged(_) = &self.update else { return };
        self.update = UpdateState::Installing;
        self.update_open = false;
        self.restart = Some((std::time::Instant::now(), None));
        cx.notify();
        if cfg!(test) {
            return;
        }
        // Debug builds only, for screenshots: the bar fills, then the app stays as it is.
        #[cfg(debug_assertions)]
        if std::env::var_os("CAULDRON_FIXTURE_DOWNLOAD").is_some() {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(std::time::Duration::from_millis(2500)).await;
                let _ = this.update(cx, |this, cx| {
                    this.restart = None;
                    this.update = UpdateState::Idle;
                    cx.notify();
                });
            })
            .detach();
            self.tick_restart(cx);
            return;
        }
        backspace_core::crash::note("update: swapping the exe");
        let task = cx.background_executor().spawn(async { upd::apply_download() });
        cx.spawn(async move |this, cx| {
            let res = task.await;
            if let Err(e) = &res {
                backspace_core::crash::note(&format!("update: swap failed: {e:#}"));
            }
            match res {
                Ok(exe) => {
                    let _ = this.update(cx, |this, cx| {
                        if let Some((t0, _)) = this.restart {
                            this.restart = Some((t0, Some(std::time::Instant::now())));
                        }
                        cx.notify();
                    });
                    // Let the bar reach the end before the app goes.
                    cx.background_executor().timer(std::time::Duration::from_millis(450)).await;
                    let _ = this.update(cx, |this, cx| match backspace_runner::cmd(&exe).env(backspace_core::instance::RESTART_ENV, "1").spawn() {
                        Ok(child) => {
                            backspace_core::crash::note(&format!("update: started the new exe (pid {}), quitting", child.id()));
                            quit_for_update(cx);
                        }
                        Err(e) => {
                            backspace_core::crash::note(&format!("update: the new exe didn't start: {e}"));
                            this.restart = None;
                            this.update = UpdateState::Failed(format!("The update is in place, but it didn't start: {e}. Open Cauldron again."));
                            cx.notify();
                        }
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.restart = None;
                        this.update = UpdateState::Failed(format!("{e:#}"));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
        self.tick_restart(cx);
    }

    /// Redraws the restart card about thirty times a second while it is up.
    fn tick_restart(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(33)).await;
            match this.update(cx, |this, cx| {
                let on = this.restart.is_some();
                if on {
                    cx.notify();
                }
                on
            }) {
                Ok(true) => {}
                _ => break,
            }
        })
        .detach();
    }

    /// The restart card: a green bar that fills over the swap. It creeps toward 90% while the swap
    /// runs and finishes the moment it is in place.
    pub(super) fn restart_el(&self, pal: &'static Palette) -> Option<AnyElement> {
        let (t0, applied) = self.restart?;
        let now = std::time::Instant::now();
        let creep = |secs: f32| 0.9 * (1.0 - (-secs / 0.8).exp());
        let p = match applied {
            Some(a) => {
                let base = creep(a.duration_since(t0).as_secs_f32());
                base + (1.0 - base) * (now.duration_since(a).as_secs_f32() / 0.3).min(1.0)
            }
            None => creep(now.duration_since(t0).as_secs_f32()),
        };
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        Some(
            div()
                .id("restart-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(c(pal.bg).opacity(0.94))
                .occlude()
                .child(
                    div()
                        .id("restart-card")
                        .w(px(300.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(14.))
                        .p(px(24.))
                        .rounded(px(radius::PANEL))
                        .bg(c(pal.raised))
                        .border_1()
                        .border_color(c(pal.line_strong))
                        .shadow(vec![drop(tokens::shadow::MENU)])
                        .aria_label("Updating Cauldron")
                        .child(div().size(px(34.)).rounded_full().bg(c(pal.accent)))
                        .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Updating Cauldron"))
                        .child(
                            div()
                                .id("restart-bar")
                                .w_full()
                                .h(px(6.))
                                .rounded_full()
                                .bg(c(pal.inset))
                                .overflow_hidden()
                                .child(div().h_full().w(relative(p.clamp(0.0, 1.0))).rounded_full().bg(c(pal.ok)))
                                .test_support(),
                        )
                        .child(div().text_size(px(11.5)).text_color(fg3).child("It restarts by itself when it is in place."))
                        .test_support(),
                )
                .into_any_element(),
        )
    }

    /// One line per thing that can update: the app, then each coding CLI with its mark, its
    /// version and where it stands.
    #[inline(never)]
    fn update_rows(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let mut list = div().id("update-rows").flex().flex_col().mt(px(4.)).mb(px(2.));
        let row = |id: ElementId, mark: AnyElement, name: String, status: String, tone: Hsla, action: Option<AnyElement>| {
            div()
                .id(id)
                .h(px(28.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().w(px(16.)).flex_none().flex().justify_center().child(mark))
                .child(div().flex_none().text_size(px(12.5)).text_color(fg).child(name.clone()))
                .child(div().flex_1().min_w_0().truncate().text_align(TextAlign::Right).text_size(px(11.5)).text_color(tone).child(status.clone()))
                .children(action)
                .aria_label(format!("{name}, {status}"))
        };
        // A row's own button: just this one updates.
        let small = |id: ElementId, label: &'static str, hint: String| {
            div()
                .id(id)
                .h(px(20.))
                .px(px(7.))
                .flex()
                .flex_none()
                .items_center()
                .whitespace_nowrap()
                .rounded(px(radius::TAG))
                .border_1()
                .border_color(c(pal.line_strong))
                .text_size(px(11.))
                .text_color(fg)
                .cursor_pointer()
                .hover(|d| d.bg(c(pal.hover)))
                .aria_label(hint)
                .child(label)
        };
        let busy = matches!(self.update, UpdateState::Installing);
        let (app_status, app_tone) = match &self.update {
            UpdateState::Ready(i) if i.newer => (format!("{} → {}", upd::VERSION, i.latest), c(pal.warn)),
            UpdateState::Downloading(_, d) => (format!("Downloading {}%", (d.fraction() * 100.0).round()), c(pal.accent)),
            UpdateState::Staged(_) => ("Ready to restart".to_string(), c(pal.ok)),
            UpdateState::Ready(i) if i.build.is_empty() => ("Built by hand".to_string(), fg3),
            UpdateState::Ready(_) => (format!("{} · up to date", upd::VERSION), fg3),
            UpdateState::Failed(_) => ("Couldn't check".to_string(), c(pal.bad)),
            UpdateState::Installing => ("Updating…".to_string(), c(pal.accent)),
            _ => ("Checking…".to_string(), fg3),
        };
        let app_action = match &self.update {
            UpdateState::Ready(i) if i.newer && !busy => Some(small(ElementId::Name("update-app-row".into()), if i.installs { "Download" } else { "Open" }, "Update only the app".into()).on_click(cx.listener(|this, _, _, cx| this.update_app(cx))).test_support().into_any_element()),
            UpdateState::Staged(_) => Some(small(ElementId::Name("update-app-row".into()), "Restart", "Restart to update".into()).on_click(cx.listener(|this, _, _, cx| this.update_app(cx))).test_support().into_any_element()),
            _ => None,
        };
        list = list.child(row(ElementId::Name("update-row-app".into()), div().size(px(11.)).rounded_full().bg(c(pal.accent)).into_any_element(), "Cauldron".into(), app_status, app_tone, app_action).test_support());
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
            let action = matches!(r.state, RowState::Newer(_)).then(|| {
                let provider = r.provider.clone();
                small(ElementId::NamedInteger("update-cli".into(), i as u64), "Update", format!("Update only {}", r.name)).on_click(cx.listener(move |this, _, _, cx| this.update_cli(&provider, cx))).test_support().into_any_element()
            });
            list = list.child(row(ElementId::NamedInteger("update-row".into(), i as u64), mark, r.name.clone(), status, tone, action).test_support());
        }
        list.into_any_element()
    }

    /// The card above the version button.
    pub(super) fn update_card(&self, pal: &'static Palette, w: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let (fg, fg2) = (c(pal.fg), c(pal.fg2));
        let (head, sub) = self.update.text();
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .h(px(28.))
                .px(px(10.))
                .flex()
                .flex_none()
                .items_center()
                .whitespace_nowrap()
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_size(px(12.))
                .shadow(super::lift(super::Lift::Button, pal))
                .when_live(primary, |d| d.bg(c(pal.accent)).text_color(c(pal.on_accent)))
                .when_live(!primary, |d| d.border_1().border_color(c(pal.line_strong)).text_color(fg).hover(|d| d.bg(c(pal.hover))))
                .aria_label(label)
                .child(label)
        };
        // Buttons wrap onto a second line rather than spill out of the card (Charles, 23 Nov).
        let mut actions = div().flex().flex_wrap().items_center().gap(px(6.)).pt(px(4.));
        let busy = matches!(self.update, UpdateState::Installing) || self.cli_rows.iter().any(|r| r.state == RowState::Updating);
        let app_ready = matches!(&self.update, UpdateState::Ready(i) if i.newer);
        let installs = matches!(&self.update, UpdateState::Ready(i) if i.installs);
        let staged = matches!(self.update, UpdateState::Staged(_));
        let newer_clis: Vec<&CliRow> = self.cli_rows.iter().filter(|r| matches!(r.state, RowState::Newer(_))).collect();
        // The marks of the CLIs about to update, so the button says which ones.
        let marks = || {
            let mut m = div().flex().items_center().gap(px(-3.));
            for r in &newer_clis {
                if let Some(im) = super::logos::cli_mark(&r.provider).and_then(|n| super::logos::image(n, &pal.on_accent.to_hex())) {
                    m = m.child(img(im).size(px(13.)));
                }
            }
            m
        };
        let big = |id: &'static str, label: String, marks: Option<Div>| {
            div()
                .id(id)
                .h(px(28.))
                .px(px(10.))
                .flex()
                .flex_none()
                .items_center()
                .whitespace_nowrap()
                .gap(px(7.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_size(px(12.))
                .shadow(super::lift(super::Lift::Button, pal))
                .bg(c(pal.accent))
                .text_color(c(pal.on_accent))
                .aria_label(label.clone())
                .children(marks)
                .child(label)
        };
        // The app and the CLIs update together or each on its own (Charles, 23 Nov). The app downloads
        // first and waits as "Update ready"; only the Restart button restarts it.
        if !busy {
            if staged {
                actions = actions.child(big("update-restart", "Restart to update".into(), None).on_click(cx.listener(|this, _, _, cx| this.update_app(cx))).test_support());
            }
            let app_label = if installs { "Download update" } else { "Open release page" };
            if app_ready && !newer_clis.is_empty() {
                actions = actions
                    .child(big("update-install", "Update all".into(), Some(marks())).on_click(cx.listener(|this, _, _, cx| this.update_all(cx))).test_support())
                    .child(button("update-app", "Only the app", false).on_click(cx.listener(|this, _, _, cx| this.update_app(cx))).test_support())
                    .child(button("update-clis", "Only the CLIs", false).on_click(cx.listener(|this, _, _, cx| this.update_clis(cx))).test_support());
            } else if app_ready {
                actions = actions.child(big("update-install", app_label.into(), None).on_click(cx.listener(|this, _, _, cx| this.update_app(cx))).test_support());
            } else if !newer_clis.is_empty() {
                let label = if newer_clis.len() == 1 { format!("Update {}", newer_clis[0].name) } else { "Update CLIs".to_string() };
                actions = actions.child(big("update-install", label, Some(marks())).on_click(cx.listener(|this, _, _, cx| this.update_clis(cx))).test_support());
            }
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
            .w(px(w))
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
            .child(self.update_rows(pal, cx))
            .child(changes)
            .child(actions)
    }
}

/// Quits so the updated exe, already started, can take over. The new one waits up to ten seconds
/// for this one to let go of the single-instance lock and then runs anyway; if quitting stalls
/// (a runtime waiting on a stuck task, a child that won't stop), two copies would end up running
/// on the same data. So this one ends itself after five seconds whatever is still going.
fn quit_for_update(cx: &mut App) {
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(5));
        backspace_core::crash::note("update: quit stalled for 5 s; ending the old process");
        backspace_runner::shutdown_opencode();
        std::process::exit(0);
    });
    cx.quit();
}
