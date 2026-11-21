//! Who answers (board "Accounts and looks", 20 Nov, direction A for both): one route chip that
//! names the CLI, model and account, opening one menu with every coding CLI and its state; and the
//! Subscriptions rows in Settings, a row per CLI with its logins under it. Each login is a profile
//! folder the CLI's own sign-in writes to (core `accounts.rs`); the radio picks the one new turns
//! run on. Statuses come from `accounts::status_all`, which starts processes, so they load off the
//! UI thread when core opens and again when the menu or Settings asks.

use super::*;
use backspace_core::accounts::{self, Accounts, CliStatus};
use backspace_core::chat::{Route, RouteKind};
use backspace_core::harnesses::Auth;
use gpui_kit::prelude::FluentBuilder;

/// The New stream box's picker is the stacked card (Settings > Appearance > Model picker); the
/// default since Charles's 21 Nov ask ("expand down the stack from under").
pub(crate) fn tray_stacked() -> bool {
    Prefs::load().model_tray.as_deref() != Some("unfold")
}

/// The stacked card's picker height once it has dropped out (picker v2 inside).
pub(crate) const STACK_OPEN_H: f32 = 380.0;

/// Stacked picker: how far the card hides under the box while folded, and the gap it drops to.
pub(crate) const STACK_TUCK: f32 = 12.0;
pub(crate) const STACK_GAP: f32 = 6.0;

/// The new-stream tray's height once it has unfolded (Model tray A).
pub(crate) const TRAY_OPEN_H: f32 = 216.0;

/// Where a picked route goes: the new-stream box, or an open stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PickFor {
    New,
    Thread(String),
}

/// Models offered under a CLI, besides its own default. Only names the CLI itself documents as
/// stable aliases; anything else stays the CLI's own choice.
pub(crate) fn models(provider: &str) -> &'static [&'static str] {
    match provider {
        "claude" => &["opus", "sonnet", "haiku"],
        _ => &[],
    }
}

/// What a CLI's model picker offers: its own default, the aliases above, then whatever the CLI
/// lists itself (OpenCode: every configured provider's models).
pub(crate) fn model_opts(s: &CliStatus) -> Vec<Option<String>> {
    let mut v: Vec<Option<String>> = vec![None];
    for m in models(&s.provider).iter().map(|m| m.to_string()).chain(s.models.iter().cloned()) {
        if !v.iter().any(|o| o.as_deref() == Some(m.as_str())) {
            v.push(Some(m));
        }
    }
    v
}

/// The CLI's display name for an id, from the statuses when they are in, else the id.
fn cli_name(list: &[CliStatus], provider: &str) -> String {
    list.iter().find(|s| s.provider == provider).map_or_else(|| provider.to_string(), |s| s.name.clone())
}

/// "Claude · opus · Work": the CLI, the model when one is pinned, the account when it is not
/// the machine's own login.
pub(crate) fn route_label(list: &[CliStatus], route: Option<&Route>) -> String {
    let Some(r) = route else { return "Choose who answers".into() };
    if r.kind != RouteKind::Cli {
        return r.model.clone().unwrap_or_else(|| r.provider.clone());
    }
    let mut parts = vec![cli_name(list, &r.provider)];
    if let Some(m) = r.model.as_ref().filter(|m| !m.is_empty()) {
        parts.push(m.clone());
    }
    if let Some(s) = list.iter().find(|s| s.provider == r.provider) {
        if let Some(l) = s.logins.get(s.active).filter(|l| l.id.is_some()) {
            parts.push(l.name.clone());
        }
    }
    parts.join(" · ")
}

/// One line on a login's state, for people to read.
pub(crate) fn login_line(l: &accounts::LoginStatus) -> String {
    match (l.signed_in, &l.detail) {
        (Auth::Authenticated, Some(d)) => d.clone(),
        (Auth::Authenticated, None) => "Signed in".into(),
        (Auth::Unauthenticated, _) => "Not signed in".into(),
        (Auth::Unknown, _) => "Sign-in not checked".into(),
    }
}

impl Cauldron {
    /// Reads every CLI's state off the UI thread, then redraws.
    pub(super) fn load_providers(&mut self, cx: &mut Context<Self>) {
        if self.start.providers_loading {
            return;
        }
        self.start.providers_loading = true;
        let path = self.accounts_path();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let list = cx.background_executor().spawn(async move { accounts::status_all(&Accounts::load(&path), &Prefs::load()) }).await;
            let _ = this.update(cx, |c, cx| {
                c.start.providers = list;
                c.start.providers_loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// Puts a route on the new-stream box (remembered as the default) or on an open stream.
    pub(super) fn pick_route(&mut self, to: &PickFor, route: Route) {
        match to {
            PickFor::New => {
                let mut p = Prefs::load();
                p.default_route = Some(route.clone());
                p.save();
                self.start.route = Some(route);
            }
            PickFor::Thread(id) => {
                if let Some((_, chats)) = self.core.as_ref() {
                    if let Err(e) = chats.set_route(id, route) {
                        self.send_error = Some(e.to_string());
                    }
                }
                self.refresh();
            }
        }
    }

    /// Runs `provider` on this login from the next turn on, everywhere.
    pub(super) fn pick_login(&mut self, provider: &str, id: Option<String>, cx: &mut Context<Self>) {
        let path = self.accounts_path();
        let mut a = Accounts::load(&path);
        a.set_active(provider, id.as_deref());
        let _ = a.save(&path);
        if let Some(s) = self.start.providers.iter_mut().find(|s| s.provider == provider) {
            s.active = s.logins.iter().position(|l| l.id == id).unwrap_or(0);
        }
        cx.notify();
    }

    /// Opens the CLI's own sign-in in a terminal for one login; a failure shows where the
    /// route's errors show.
    pub(super) fn sign_in(&mut self, provider: &str, id: Option<&str>, cx: &mut Context<Self>) {
        if let Err(e) = accounts::sign_in(&self.accounts_path(), provider, id) {
            self.start.error = Some(format!("Couldn't open {provider}'s sign-in: {e}"));
        }
        cx.notify();
    }

    /// Adds a login and opens its sign-in; the new row appears when the statuses reload.
    pub(super) fn add_login(&mut self, provider: &str, cx: &mut Context<Self>) {
        match accounts::add_and_sign_in(&self.accounts_path(), provider) {
            Ok(_) => {}
            Err(e) => self.start.error = Some(format!("Couldn't add a {provider} account: {e}")),
        }
        self.load_providers(cx);
        cx.notify();
    }

    /// The route chip: what answers, opening the menu. `id` keeps each place's chip findable.
    pub(super) fn route_chip(&self, id: &'static str, to: PickFor, route: Option<&Route>, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let label = route_label(&self.start.providers, route);
        let open = self.start.route_menu.as_ref() == Some(&to);
        let ready = route.is_none_or(|r| r.kind != RouteKind::Cli || self.start.providers.iter().find(|s| s.provider == r.provider).is_none_or(CliStatus::ready));
        let to2 = to.clone();
        let r0 = route.cloned();
        div()
            .id(id)
            .relative()
            .h(px(24.))
            .min_w_0()
            .flex_shrink(1.)
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(5.))
            .rounded(px(radius::TAG))
            .text_size(px(12.))
            .text_color(fg)
            .cursor_pointer()
            .hover(|d| d.bg(c(pal.hover)))
            .aria_label(label.clone())
            .aria_expanded(open)
            .when(!ready, |d| d.aria_description("Not signed in"))
            .child(div().size(px(6.)).flex_none().rounded_full().bg(if ready { c(pal.ok) } else { c(pal.warn) }))
            .child(div().min_w_0().truncate().child(label))
            .child(glyph("chevron-down", 12., fg3))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                let was = this.start.route_menu.as_ref() == Some(&to2);
                this.start.route_menu = if was { None } else { Some(to2.clone()) };
                if !was {
                    this.load_providers(cx);
                    this.open_picker(r0.as_ref(), window, cx);
                }
                cx.notify();
            }))
            // The new-stream box unfolds its tray instead (`route_tray`); an open stream gets the
            // model picker (board Picker A).
            // An open stream's picker is a tray on top of its chat box (thread.rs).
            .test_support()
            .into_any_element()
    }

    /// Model tray A (Charles, 20 Nov: "the under tray should drop down with an animation where
    /// it expands down to reveal the whole picker"): under the new-stream box's chips the tray
    /// grows to `TRAY_OPEN_H`, showing who answers, the model, the account and the effort at once.
    /// The box never moves. Picking a model, the chip again or a click outside folds it.
    pub(super) fn route_tray(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let route = self.start.route.clone();
        let current = route.as_ref().filter(|r| r.kind == RouteKind::Cli).map(|r| r.provider.clone());
        let head = |id: &'static str, text: &'static str| div().id(id).px(px(9.)).pt(px(8.)).pb(px(4.)).text_size(px(11.)).text_color(fg3).aria_label(text).child(text).test_support();
        let item = |id: (&'static str, usize), on: bool| {
            div()
                .id(id)
                .h(px(28.))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(9.))
                .rounded(px(radius::CONTROL))
                .text_size(px(12.5))
                .text_color(if on { fg } else { fg2 })
                .when(on, |d| d.bg(c(pal.selected)))
        };
        // Who answers: every coding CLI, installed and signed-in ones first.
        let mut left = div().id("tray-who").w(px(200.)).flex_none().flex().flex_col().gap(px(1.)).child(head("tray-who-head", "Who answers"));
        if self.start.providers.is_empty() {
            left = left.child(div().id("route-loading").px(px(9.)).py(px(6.)).text_size(px(12.)).text_color(fg3).aria_label("Looking for coding CLIs…").child("Looking for coding CLIs…").test_support());
        }
        let mut list: Vec<&CliStatus> = self.start.providers.iter().collect();
        list.sort_by_key(|s| (!s.installed, !s.ready()));
        let mut chosen = None;
        for (i, s) in list.iter().enumerate() {
            let on = current.as_deref() == Some(s.provider.as_str());
            if on {
                chosen = Some(*s);
            }
            let login = s.logins.get(s.active);
            let state = if !s.installed { "Not installed".to_string() } else { login.map_or_else(String::new, login_line) };
            let dot = if !s.installed { c(pal.line_strong) } else if s.ready() { c(pal.ok) } else { c(pal.warn) };
            let (prov, installed, ready) = (s.provider.clone(), s.installed, s.ready());
            let active_id = login.and_then(|l| l.id.clone());
            let mut row = item(("route-cli", i), on)
                .aria_label(format!("{} — {state}", s.name))
                .aria_selected(on)
                .child(div().size(px(7.)).flex_none().rounded_full().bg(dot))
                .child(div().flex_1().min_w_0().truncate().child(s.name.clone()))
                .child(div().max_w(px(80.)).truncate().text_size(px(11.)).text_color(fg3).child(state));
            if installed {
                row = row.cursor_pointer().when(!on, |d| d.hover(|d| d.bg(c(pal.hover)))).on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    if !ready {
                        this.sign_in(&prov, active_id.as_deref(), cx);
                        return;
                    }
                    // Picking who answers keeps the tray open for its model.
                    this.pick_route(&PickFor::New, Route { kind: RouteKind::Cli, provider: prov.clone(), model: None });
                    cx.notify();
                }));
            } else {
                row = row.opacity(0.55).aria_description(format!("Install: {}", s.install));
            }
            left = left.child(row.test_support());
        }
        // The chosen CLI's models, its accounts, and the effort.
        let mut right = div().id("tray-model").flex_1().min_w_0().flex().flex_col().gap(px(1.)).child(head("tray-model-head", "Model"));
        match chosen {
            Some(s) => {
                let pinned = route.as_ref().and_then(|r| r.model.clone());
                // A CLI with its own list (OpenCode) can name dozens: they scroll in the tray.
                let mut list = div().id("tray-models").flex().flex_col().gap(px(1.)).max_h(px(96.)).overflow_y_scroll();
                for (j, m) in model_opts(s).into_iter().enumerate() {
                    let sel = pinned == m;
                    let (prov, m2) = (s.provider.clone(), m.clone());
                    // Aliases read as names ("Opus"); listed ids ("openrouter/qwen/...") stay as written.
                    let name = m.as_deref().map_or_else(|| "Its default model".to_string(), |m| {
                        if m.contains('/') {
                            return m.to_string();
                        }
                        let mut ch = m.chars();
                        ch.next().map(|f| format!("{}{}", f.to_uppercase(), ch.as_str())).unwrap_or_default()
                    });
                    list = list.child(
                        item(("route-model", j), sel)
                            .cursor_pointer()
                            .when(!sel, |d| d.hover(|d| d.bg(c(pal.hover))))
                            .aria_label(name.clone())
                            .aria_selected(sel)
                            .child(div().flex_1().child(name))
                            .when(sel, |d| d.child(glyph("check", 13., fg2)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.pick_route(&PickFor::New, Route { kind: RouteKind::Cli, provider: prov.clone(), model: m2.clone() });
                                this.start.route_menu = None;
                                cx.notify();
                            }))
                            .test_support(),
                    );
                }
                right = right.child(list);
                if s.logins.len() > 1 {
                    right = right.child(head("tray-runs-on", "Runs on"));
                    for (j, l) in s.logins.iter().enumerate() {
                        let sel = j == s.active;
                        let (prov, lid) = (s.provider.clone(), l.id.clone());
                        right = right.child(
                            item(("route-login", j), sel)
                                .cursor_pointer()
                                .when(!sel, |d| d.hover(|d| d.bg(c(pal.hover))))
                                .aria_label(l.name.clone())
                                .aria_selected(sel)
                                .child(div().flex_1().min_w_0().truncate().child(l.name.clone()))
                                .child(div().max_w(px(120.)).truncate().text_size(px(11.)).text_color(fg3).child(login_line(l)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.pick_login(&prov, lid.clone(), cx);
                                }))
                                .test_support(),
                        );
                    }
                }
            }
            None => {
                right = right.child(div().id("tray-pick-first").px(px(9.)).py(px(6.)).text_size(px(12.)).text_color(fg3).aria_label("Pick who answers first.").child("Pick who answers first.").test_support());
            }
        }
        // Effort: one segmented row instead of a chip that cycles.
        let effort = Prefs::load().cli_effort;
        let mut seg = div().id("tray-effort").flex().gap(px(2.)).p(px(2.)).mx(px(9.)).rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line_strong));
        for (k, e) in super::thread::EFFORTS.iter().enumerate() {
            let on = effort.as_deref() == *e;
            let label = e.map_or_else(|| "Auto".to_string(), |v| {
                let mut ch = v.chars();
                ch.next().map(|f| format!("{}{}", f.to_uppercase(), ch.as_str())).unwrap_or_default()
            });
            let val = e.map(str::to_string);
            seg = seg.child(
                div()
                    .id(("tray-effort-opt", k))
                    .flex_1()
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::TAG))
                    .cursor_pointer()
                    .text_size(px(11.5))
                    .text_color(if on { fg } else { fg2 })
                    .when(on, |d| d.bg(c(pal.selected)))
                    .when(!on, |d| d.hover(|d| d.bg(c(pal.hover))))
                    .aria_selected(on)
                    .aria_label(label.clone())
                    .child(label)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.stop_propagation();
                        let mut p = Prefs::load();
                        p.cli_effort = val.clone();
                        p.save();
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        let manage = div()
            .id("route-manage")
            .h(px(26.))
            .px(px(9.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(radius::CONTROL))
            .cursor_pointer()
            .hover(|d| d.bg(c(pal.hover)))
            .text_size(px(12.))
            .text_color(fg3)
            .aria_label("Manage subscriptions")
            .child(glyph("settings", 13., fg3))
            .child("Manage subscriptions…")
            .on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.start.route_menu = None;
                this.settings = Some(0);
                cx.notify();
            }))
            .test_support();
        right = right.child(head("tray-effort-head", "Effort")).child(seg.test_support()).child(div().flex_1()).child(manage);
        let body = div()
            .flex()
            .flex_col()
            .h(px(TRAY_OPEN_H))
            .px(px(6.))
            .pb(px(6.))
            .border_t_1()
            .border_color(c(pal.line))
            .child(div().flex_1().min_h_0().flex().gap(px(10.)).overflow_hidden().child(left.test_support()).child(right.test_support()));
        // Height 0 to TRAY_OPEN_H over 220 ms; the rows fade in a beat behind the edge.
        div()
            .id("route-tray")
            .overflow_hidden()
            .occlude()
            .aria_label("Who answers")
            .child(body)
            .test_support()
            .with_animation("route-tray-open", super::motion::anim(220.), |d, t| {
                let rows = ((t - 0.27) / 0.73).clamp(0.0, 1.0);
                d.h(px(TRAY_OPEN_H * t)).opacity(0.35 + 0.65 * rows)
            })
            .into_any_element()
    }

    /// Settings > Subscriptions: a row per CLI, its logins under it, each with Sign in; the radio
    /// is the login new turns run on.
    pub(super) fn subscription_rows(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Div {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let btn = |id: (&'static str, usize), label: &str, primary: bool| {
            div()
                .id(id)
                .h(px(26.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(radius::CONTROL))
                .border_1()
                .border_color(if primary { c(pal.accent) } else { c(pal.line) })
                .when(primary, |d| d.bg(c(pal.accent)).text_color(c(pal.on_accent)))
                .when(!primary, |d| d.text_color(fg2))
                .text_size(px(12.))
                .cursor_pointer()
                .hover(|d| d.opacity(0.85))
                .aria_label(label.to_string())
                .child(label.to_string())
        };
        let mut col = div().flex().flex_col();
        if self.start.providers.is_empty() {
            let text = if self.start.providers_loading { "Looking for coding CLIs…" } else { "No coding CLI found on this machine" };
            col = col.child(div().id(("sub", 0usize)).py(px(10.)).text_size(px(13.)).text_color(fg3).aria_label(text).child(text).test_support());
        }
        for (i, s) in self.start.providers.iter().enumerate() {
            let dot = if !s.installed { c(pal.line_strong) } else if s.ready() { c(pal.ok) } else { c(pal.warn) };
            let sub = if s.installed {
                format!("{}{}", s.version.as_deref().map(|v| format!("{v} · ")).unwrap_or_default(), match s.logins.len() { 1 => "1 login".into(), n => format!("{n} logins") })
            } else {
                format!("Not installed · {}", s.install)
            };
            let prov = s.provider.clone();
            col = col.child(
                div()
                    .id(("sub-cli", i))
                    .py(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .border_t_1()
                    .border_color(c(pal.line))
                    .aria_label(format!("{}: {sub}", s.name))
                    .child(div().size(px(8.)).flex_none().rounded_full().bg(dot))
                    .child(div().flex_1().min_w_0().flex().flex_col().child(div().text_size(px(13.)).text_color(fg).child(s.name.clone())).child(div().text_size(px(12.)).text_color(fg3).truncate().child(sub)))
                    .when(s.installed, |d| {
                        d.child(btn(("sub-add", i), "Add account", false).on_click(cx.listener(move |this, _, _, cx| this.add_login(&prov, cx))).test_support())
                    })
                    .test_support(),
            );
            if !s.installed {
                continue;
            }
            for (j, l) in s.logins.iter().enumerate() {
                let sel = j == s.active;
                let signed = l.signed_in == Auth::Authenticated;
                let (p1, p2, id1, id2) = (s.provider.clone(), s.provider.clone(), l.id.clone(), l.id.clone());
                col = col.child(
                    div()
                        .id(("sub-login", i * 16 + j))
                        .py(px(7.))
                        .pl(px(18.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .aria_label(format!("{}: {}", l.name, login_line(l)))
                        .aria_selected(sel)
                        .child(
                            div()
                                .id(("sub-radio", i * 16 + j))
                                .size(px(14.))
                                .flex_none()
                                .rounded_full()
                                .border_color(if sel { c(pal.accent) } else { c(pal.line_control) })
                                .when(sel, |d| d.border(px(4.)))
                                .when(!sel, |d| d.border_1())
                                .cursor_pointer()
                                .aria_label(format!("Run new turns on {}", l.name))
                                .on_click(cx.listener(move |this, _, _, cx| this.pick_login(&p1, id1.clone(), cx)))
                                .test_support(),
                        )
                        .child(div().flex_1().min_w_0().flex().flex_col().child(div().text_size(px(12.5)).text_color(fg).child(l.name.clone())).child(div().text_size(px(12.)).text_color(if signed { fg3 } else { c(pal.warn) }).truncate().child(login_line(l))))
                        .child(btn(("sub-signin", i * 16 + j), if signed { "Sign in again" } else { "Sign in" }, !signed).on_click(cx.listener(move |this, _, _, cx| this.sign_in(&p2, id2.as_deref(), cx))).test_support())
                        .test_support(),
                );
            }
        }
        col.child(
            div()
                .id("sub-refresh")
                .mt(px(8.))
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(12.))
                .text_color(fg2)
                .cursor_pointer()
                .hover(|d| d.text_color(fg))
                .aria_label("Check again")
                .child(glyph("refresh-cw", 12., fg3))
                .child(if self.start.providers_loading { "Checking…" } else { "Check again" })
                .on_click(cx.listener(|this, _, _, cx| this.load_providers(cx)))
                .test_support(),
        )
    }
}
