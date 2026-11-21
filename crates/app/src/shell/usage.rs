//! Usage in the sidebar foot (Charles, 21 Nov 16:00Z; board cauldron/usage-foot, direction A,
//! after the destinations board's ring beside your name): a ring with the lowest share left sits
//! in the user row. Hovering it lists every account Cauldron has connected and every CLI login it
//! detected on this machine, with what each has left; clicking opens the Usage page in the card.
//! Usage is no longer an app in the top row.

use super::*;
use backspace_core::accounts::{Accounts, CliStatus};

/// One account or login on the hover card and the Usage page.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UsageLine {
    pub cli: String,
    pub name: String,
    /// The CLI's own login on this machine (found, not added in Cauldron).
    pub detected: bool,
    /// Share of the limit left, when the CLI reported one.
    pub left: Option<u64>,
    /// When it resets, or the login's state.
    pub note: String,
}

/// Under this share left the ring and the number turn amber.
pub(crate) const LOW: u64 = 20;

/// "Resets in 3h" from a unix time in seconds; empty when unknown or past.
pub(crate) fn resets_text(resets_at: u64, now: u64) -> String {
    if resets_at <= now {
        return String::new();
    }
    let m = (resets_at - now) / 60;
    match m {
        0 => "Resets in under a minute".into(),
        1..=59 => format!("Resets in {m} min"),
        60..=2879 => format!("Resets in {}h", (m + 30) / 60),
        _ => format!("Resets in {} days", (m + 720) / 1440),
    }
}

fn cli_label(providers: &[CliStatus], provider: &str) -> String {
    providers.iter().find(|s| s.provider == provider).map(|s| s.name.clone()).unwrap_or_else(|| {
        let mut ch = provider.chars();
        ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default()
    })
}

/// Connected accounts (accounts.json, enabled) first, then each installed CLI's own login.
pub(crate) fn lines(accounts: &Accounts, providers: &[CliStatus], now: u64) -> Vec<UsageLine> {
    let mut out = Vec::new();
    for a in accounts.list.iter().filter(|a| a.enabled) {
        let left = a.usage.as_ref().and_then(|u| {
            let (used, limit) = (u.used?, u.limit?);
            (limit > 0).then(|| 100u64.saturating_sub(used * 100 / limit))
        });
        let note = a.usage.as_ref().map(|u| resets_text(u.resets_at, now)).filter(|s| !s.is_empty()).unwrap_or_else(|| if left.is_some() { String::new() } else { "No limits reported yet".into() });
        out.push(UsageLine { cli: cli_label(providers, &a.provider), name: a.name.clone(), detected: false, left, note });
    }
    for s in providers.iter().filter(|s| s.installed) {
        for l in s.logins.iter().filter(|l| l.id.is_none()) {
            out.push(UsageLine { cli: s.name.clone(), name: "This machine's login".into(), detected: true, left: None, note: subs::login_line(l) });
        }
    }
    out
}

/// A ring `d` across: the track, then `pct` of it in `col` clockwise from the top.
pub(crate) fn ring(pct: Option<u64>, d: f32, col: Hsla, track: Hsla) -> impl IntoElement {
    let frac = pct.map_or(0.0, |p| p.min(100) as f32 / 100.0);
    canvas(
        |b, _, _| b,
        move |b, _, window, _| {
            let w = (d * 0.13).max(2.0);
            let r = d / 2.0 - w / 2.0;
            let (cx0, cy0) = (b.origin.x + px(d / 2.0), b.origin.y + px(d / 2.0));
            let arc = |from: f32, to: f32| {
                let mut pb = PathBuilder::stroke(px(w));
                let n = ((to - from) * 64.0).ceil().max(2.0) as usize;
                for i in 0..=n {
                    let t = from + (to - from) * i as f32 / n as f32;
                    let a = t * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                    let p = point(cx0 + px(r * a.cos()), cy0 + px(r * a.sin()));
                    if i == 0 {
                        pb.move_to(p);
                    } else {
                        pb.line_to(p);
                    }
                }
                pb.build().ok()
            };
            if let Some(p) = arc(0.0, 1.0) {
                window.paint_path(p, track);
            }
            if frac > 0.0 {
                if let Some(p) = arc(0.0, frac) {
                    window.paint_path(p, col);
                }
            }
        },
    )
    .size(px(d))
    .flex_none()
}

impl Cauldron {
    fn now_secs() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
    }

    pub(super) fn usage_lines(&self) -> Vec<UsageLine> {
        lines(&Accounts::load(&self.accounts_path()), &self.start.providers, Self::now_secs())
    }

    fn ring_col(left: Option<u64>, pal: &'static Palette) -> Hsla {
        if left.is_some_and(|p| p < LOW) {
            c(pal.warn)
        } else {
            c(pal.fg2)
        }
    }

    /// The ring and its number in the user row: hover for the card, click for the page.
    pub(super) fn usage_button(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let low = self.usage_left().into_iter().min_by_key(|(_, p)| *p);
        let left = low.as_ref().map(|(_, p)| *p);
        let (text, label) = match &low {
            Some((who, p)) => (format!("{p}%"), format!("Usage, {p}% left on {who}")),
            None => ("–".to_string(), "Usage, no limits reported yet".to_string()),
        };
        let col = Self::ring_col(left, pal);
        div()
            .id("usage-button")
            .h(px(26.))
            .pl(px(4.))
            .pr(px(6.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(5.))
            .rounded(px(radius::CONTROL))
            .cursor_pointer()
            .text_size(px(11.5))
            .font_features(tnum())
            .text_color(if left.is_some_and(|p| p < LOW) { col } else { c(pal.fg2) })
            .when_live(self.usage_page || self.usage_hover, |d| d.bg(c(pal.hover)))
            .hover(|d| d.bg(c(pal.hover)))
            .aria_label(label)
            .aria_selected(self.usage_page)
            .child(ring(left, 18., col, c(pal.line_strong)))
            .child(text)
            .on_hover(cx.listener(|this, on: &bool, _, cx| {
                this.usage_hover = *on;
                if *on && this.start.providers.is_empty() {
                    this.load_providers(cx);
                }
                cx.notify();
            }))
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_usage(cx);
            }))
    }

    pub(super) fn open_usage(&mut self, cx: &mut Context<Self>) {
        self.usage_page = true;
        self.usage_hover = false;
        self.settings = None;
        if self.start.providers.is_empty() {
            self.load_providers(cx);
        }
        cx.notify();
    }

    fn usage_row(&self, id: (&'static str, usize), l: &UsageLine, ring_d: f32, with_cli: bool, pal: &'static Palette) -> Stateful<Div> {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let col = Self::ring_col(l.left, pal);
        let title = if with_cli { format!("{} · {}", l.cli, l.name) } else { l.name.clone() };
        let pct = l.left.map(|p| format!("{p}% left"));
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(9.))
            .py(px(3.))
            .aria_label(format!("{title}: {}", pct.clone().unwrap_or_else(|| l.note.clone())))
            .child(if l.left.is_some() {
                ring(l.left, ring_d, col, c(pal.line_strong)).into_any_element()
            } else {
                div().size(px(ring_d)).flex_none().flex().items_center().justify_center().child(div().size(px(7.)).rounded_full().bg(c(pal.line_strong))).into_any_element()
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .line_height(px(15.))
                    .child(div().truncate().text_size(px(12.5)).text_color(fg).child(title))
                    .when_live(!l.note.is_empty(), |d| d.child(div().truncate().text_size(px(11.)).text_color(fg3).child(l.note.clone()))),
            )
            .children(pct.map(|p| div().flex_none().text_size(px(12.)).font_features(tnum()).text_color(if l.left.is_some_and(|p| p < LOW) { col } else { fg }).child(p)))
    }

    /// The hover card over the user row: connected, then detected, then a link to the page.
    pub(super) fn usage_card(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let lines = self.usage_lines();
        let group = |t: &'static str| div().mt(px(4.)).text_size(px(10.5)).font_weight(FontWeight::MEDIUM).text_color(fg3).child(t);
        let mut card = div()
            .id("usage-card")
            .w(px(280.))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .rounded(px(radius::PANEL))
            .bg(c(pal.panel))
            .border_1()
            .border_color(c(pal.line_strong))
            .shadow(lift(Lift::Card, pal))
            .occlude()
            .on_hover(cx.listener(|this, on: &bool, _, cx| {
                this.usage_card_hover = *on;
                cx.notify();
            }))
            .child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Usage"));
        let (conn, det): (Vec<_>, Vec<_>) = lines.iter().enumerate().partition(|(_, l)| !l.detected);
        if lines.is_empty() {
            card = card.child(div().text_size(px(12.)).text_color(fg3).child(if self.start.providers_loading { "Looking for coding CLIs…" } else { "No accounts yet. Add one in Settings." }));
        }
        if !conn.is_empty() {
            card = card.child(group("Connected in Cauldron"));
            for (i, l) in conn {
                card = card.child(self.usage_row(("usage-pop-row", i), l, 24., true, pal).test_support());
            }
        }
        if !det.is_empty() {
            card = card.child(group("Detected on this machine"));
            for (i, l) in det {
                card = card.child(self.usage_row(("usage-pop-row", i), l, 24., true, pal).test_support());
            }
        }
        card.child(
            div()
                .id("usage-open")
                .mt(px(6.))
                .text_size(px(12.))
                .text_color(c(pal.accent))
                .cursor_pointer()
                .aria_label("Open Usage")
                .child("Open Usage")
                .on_click(cx.listener(|this, _, _, cx| this.open_usage(cx)))
                .test_support(),
        )
    }

    /// The Usage page in the card: one card per CLI with each login's ring, then what was found
    /// with no limits.
    pub(super) fn usage_page_body(&self, pal: &'static Palette, _cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let lines = self.usage_lines();
        let mut col = div().w_full().max_w(px(660.)).flex().flex_col().px(px(24.)).py(px(8.));
        col = col
            .child(div().text_size(px(20.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).mt(px(12.)).child("Usage"))
            .child(div().text_size(px(13.)).text_color(fg3).mb(px(14.)).child("What each coding CLI reported last, read every 30 seconds."));
        // Group by CLI, keeping the order lines come in.
        let mut clis: Vec<String> = Vec::new();
        for l in &lines {
            if !clis.contains(&l.cli) {
                clis.push(l.cli.clone());
            }
        }
        let mut grid = div().flex().flex_wrap().gap(px(10.));
        let mut quiet = Vec::new();
        for (k, cli) in clis.iter().enumerate() {
            let mine: Vec<(usize, &UsageLine)> = lines.iter().enumerate().filter(|(_, l)| &l.cli == cli).collect();
            if mine.iter().all(|(_, l)| l.left.is_none()) {
                quiet.extend(mine);
                continue;
            }
            let mut card = div()
                .id(("usage-cli", k))
                .w(px(296.))
                .p(px(12.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .rounded(px(radius::CONTROL))
                .border_1()
                .border_color(c(pal.line))
                .bg(c(pal.panel))
                .child(div().flex().items_center().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(fg).child(cli.clone()).child(div().ml_auto().text_size(px(11.5)).font_weight(FontWeight::NORMAL).text_color(fg3).child(match mine.len() {
                    1 => "1 login".to_string(),
                    n => format!("{n} logins"),
                })));
            for (i, l) in mine {
                card = card.child(self.usage_row(("usage-page-row", i), l, 32., false, pal).test_support());
            }
            grid = grid.child(card.test_support());
        }
        col = col.child(grid);
        if !quiet.is_empty() {
            col = col.child(div().mt(px(20.)).mb(px(4.)).text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child("Found, no limits reported"));
            for (i, l) in quiet {
                col = col.child(div().border_t_1().border_color(c(pal.line)).py(px(4.)).child(self.usage_row(("usage-page-row", i), l, 24., true, pal).test_support()));
            }
        }
        if lines.is_empty() {
            col = col.child(div().id("usage-page-empty").text_size(px(13.)).text_color(fg3).aria_label("No accounts yet").child(if self.start.providers_loading { "Looking for coding CLIs…" } else { "No accounts yet. Add one in Settings, Agents and subscriptions." }).test_support());
        }
        div().id("usage-page").size_full().overflow_y_scroll().flex().justify_center().child(col).test_support().into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{lines, resets_text};
    use backspace_core::accounts::{Account, Accounts, CliStatus, Usage};

    #[test]
    fn resets_reads_in_words() {
        assert_eq!(resets_text(0, 100), "");
        assert_eq!(resets_text(100 + 30 * 60, 100), "Resets in 30 min");
        assert_eq!(resets_text(100 + 3 * 3600, 100), "Resets in 3h");
        assert_eq!(resets_text(100 + 3 * 86400, 100), "Resets in 3 days");
    }

    #[test]
    fn connected_then_detected() {
        let acct = |id: &str, p: &str, used: u64| Account { id: id.into(), name: id.into(), provider: p.into(), enabled: true, usage: Some(Usage { used: Some(used), limit: Some(100), ..Default::default() }), ..Default::default() };
        let a = Accounts { list: vec![acct("Work", "claude", 38), acct("Personal", "codex", 86)], ..Default::default() };
        let cli = CliStatus {
            provider: "claude".into(),
            name: "Claude Code".into(),
            installed: true,
            version: None,
            install: String::new(),
            logins: vec![backspace_core::accounts::LoginStatus { id: None, name: "machine".into(), signed_in: backspace_core::harnesses::Auth::Authenticated, detail: None }],
            active: 0,
            models: Vec::new(),
            model_info: Vec::new(),
        };
        let l = lines(&a, &[cli], 0);
        assert_eq!(l.len(), 3);
        assert_eq!((l[0].cli.as_str(), l[0].left, l[0].detected), ("Claude Code", Some(62), false));
        assert_eq!((l[1].cli.as_str(), l[1].left), ("Codex", Some(14)));
        assert!(l[2].detected && l[2].left.is_none());
    }
}
