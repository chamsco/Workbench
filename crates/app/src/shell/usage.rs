//! Usage: what each coding tool on this machine did in the last 24 hours and 7 days, from its own
//! logs (core's `usage::tool_usage_summary`: sessions, prompts, tool calls, tokens). Real.
//!
//! What no local file holds is never made up (docs/cauldron/USAGE-DATA.md): how much of a plan's
//! limit is left, when it resets, what it cost. Those say "Not available" next to an empty ring.
//! The logs are read off the main thread when the page opens.

use super::pages::{tag, AppPage};
use super::*;
use backspace_core::usage::{ToolUsage, ToolWindow};

/// A tool's name as people say it.
pub(super) fn tool_name(id: &str) -> &str {
    match id {
        "claude-code" => "Claude Code",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        other => other,
    }
}

/// 1.2M, 340k, 980.
pub(super) fn short(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.0}k", n as f64 / 1_000.0),
        _ => format!("{:.1}M", n as f64 / 1_000_000.0),
    }
}

/// The two windows added over every tool.
pub(super) fn totals(tools: &[ToolUsage]) -> (ToolWindow, ToolWindow) {
    let mut t = (ToolWindow::default(), ToolWindow::default());
    for u in tools {
        for (acc, w) in [(&mut t.0, &u.today), (&mut t.1, &u.week)] {
            acc.sessions += w.sessions;
            acc.prompts += w.prompts;
            acc.tool_calls += w.tool_calls;
            acc.tokens.input += w.tokens.input;
            acc.tokens.output += w.tokens.output;
            acc.tokens.cache_read += w.tokens.cache_read;
            acc.tokens.cache_write += w.tokens.cache_write;
        }
    }
    t
}

impl Cauldron {
    /// Reads the logs in the background, once per opening of the page.
    pub(super) fn load_usage(&mut self, cx: &mut Context<Self>) {
        // Debug builds: `CAULDRON_USAGE_NOW=<ms>` ends the windows then, for screenshots of the fixture.
        let now = std::env::var("CAULDRON_USAGE_NOW").ok().filter(|_| cfg!(debug_assertions)).and_then(|v| v.parse().ok()).unwrap_or_else(now_ms);
        let home = self.usage_home.clone().unwrap_or_else(|| std::path::PathBuf::from(std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default()));
        self.usage = None;
        self.usage_task = Some(cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let got = cx.background_executor().spawn(async move { backspace_core::usage::tool_usage_summary(&home, now) }).await;
            let _ = this.update(cx, |this, cx| {
                this.usage = Some(got);
                cx.notify();
            });
        }));
    }

    fn usage_numbers(&self, w: &ToolWindow, pal: &'static Palette) -> Div {
        let cell = |n: String, label: &'static str| div().flex().flex_col().gap(px(1.)).min_w(px(70.)).child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(c(pal.fg)).child(n)).child(div().text_size(px(11.)).text_color(c(pal.fg3)).child(label));
        div().flex().gap(px(14.)).child(cell(w.sessions.to_string(), "sessions")).child(cell(w.prompts.to_string(), "prompts")).child(cell(w.tool_calls.to_string(), "tool calls")).child(cell(short(w.tokens.total()), "tokens"))
    }

    /// One tool, or the overview: the two windows, then the limit (not available).
    fn usage_card(&self, id: impl Into<ElementId>, name: String, sub: String, today: &ToolWindow, week: &ToolWindow, pal: &'static Palette) -> Stateful<Div> {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let window = |label: &'static str, w: &ToolWindow| div().flex().flex_col().gap(px(6.)).child(div().text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child(label)).child(self.usage_numbers(w, pal));
        div()
            .id(id)
            .p(px(14.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .rounded(px(14.))
            .border_1()
            .border_color(c(pal.line))
            .aria_label(name.clone())
            .child(div().flex().flex_col().gap(px(2.)).child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(name)).child(div().truncate().text_size(px(11.5)).text_color(fg3).child(sub)))
            .child(div().flex().gap(px(28.)).flex_wrap().child(window("Last 24 hours", today)).child(window("Last 7 days", week)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .pt(px(8.))
                    .border_t_1()
                    .border_color(c(pal.line))
                    .child(div().size(px(16.)).flex_none().rounded_full().border_2().border_color(c(pal.line_strong)))
                    .child(div().flex_1().text_size(px(12.)).text_color(fg3).child("Limit left and reset time: Not available. No file on this machine holds them.")),
            )
    }

    pub(super) fn usage_page(&self, pal: &'static Palette) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let pick = self.side_sel[AppPage::Usage as usize].and_then(|k| (1000..2000).contains(&k).then_some(k - 1000));
        let mut col = div().w_full().max_w(px(760.)).mx_auto().flex().flex_col().gap(px(14.)).px(px(24.)).py(px(18.));
        col = col.child(div().flex().flex_col().gap(px(4.)).child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Usage")).child(div().text_size(px(13.)).text_color(fg3).child("What each coding tool on this machine did, read from its own logs.")));
        match &self.usage {
            None => col = col.child(div().id("usage-reading").text_size(px(13.)).text_color(fg2).child("Reading the tools' logs…").test_support()),
            Some(tools) if tools.is_empty() => col = col.child(div().id("usage-none").p(px(14.)).rounded(px(14.)).border_1().border_color(c(pal.line)).text_size(px(13.)).text_color(fg2).child("No data yet: no Claude Code, Codex or OpenCode logs on this machine.").test_support()),
            Some(tools) => match pick.and_then(|i| tools.get(i)) {
                Some(u) => {
                    let sub = format!("Read from {} · as of {} ago", u.source_path, threads::age(u.as_of, now_ms()));
                    col = col.child(self.usage_card("usage-tool", tool_name(&u.tool).to_string(), sub, &u.today, &u.week, pal).test_support());
                }
                None => {
                    let (d, w) = totals(tools);
                    col = col.child(self.usage_card("usage-overview-card", "All tools".into(), format!("{} on this machine", tools.iter().map(|u| tool_name(&u.tool)).collect::<Vec<_>>().join(", ")), &d, &w, pal).test_support());
                    for (i, u) in tools.iter().enumerate() {
                        col = col.child(self.usage_card(("usage-card", i), tool_name(&u.tool).to_string(), format!("Read from {}", u.source_path), &u.today, &u.week, pal).test_support());
                    }
                }
            },
        }
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().text_size(px(11.5)).text_color(fg3).child("Alerts when a limit is close need the limit, which no local file holds."))
                .child(tag("usage-alerts-example", "Example", pal).test_support()),
        );
        div().id("page-body").size_full().overflow_y_scroll().child(col).test_support().into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{short, tool_name};

    #[test]
    fn numbers_read_short() {
        assert_eq!([short(980), short(34_600), short(1_250_000)], ["980", "35k", "1.2M"]);
        assert_eq!(tool_name("claude-code"), "Claude Code");
    }

    /// The page reads core's numbers (here its fixture home, never the real one): one card per
    /// tool and the overview; a limit or reset is never shown as a number.
    #[gpui_kit::test]
    fn usage_shows_core_numbers_and_no_limits(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        use gpui_kit::AppContext;
        let (handle, _chats) = super::super::thread::tests::open(cx);
        let home = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../core/tests/fixtures/usage_home");
        handle
            .update(cx, |c, _, cx| {
                c.usage_home = Some(home.clone());
                c.open_page(super::AppPage::Usage);
                c.load_usage(cx);
                c.motion = Default::default();
            })
            .unwrap();
        cx.run_until_parked();
        let tools = handle.update(cx, |c, _, _| c.usage.clone()).unwrap().expect("read");
        assert!(!tools.is_empty(), "the fixture has tools");
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("usage-overview-card").is_some());
            for i in 0..tools.len() {
                assert_eq!(window.find(("usage-card", i)).label(), Some(tool_name(&tools[i].tool)));
            }
        })
        .unwrap();
    }
}
