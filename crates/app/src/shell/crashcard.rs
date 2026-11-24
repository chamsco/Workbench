//! "Cauldron crashed last time" (Charles, 24 Nov: report a crash as a GitHub issue with what
//! happened and the log). The crash file is written by `backspace_core::crash`; this card offers
//! it on the next start. "Report on GitHub" opens a pre-filled new-issue page the user reads and
//! submits, and copies the whole report, in case the page had to cut it. Nothing is sent by itself.

use super::*;
use backspace_core::crash;

impl Cauldron {
    pub(super) fn crash_card(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let r = self.crash.as_ref()?;
        let (fg, fg2) = (c(pal.fg), c(pal.fg2));
        let what = r.title.strip_prefix("Crash: ").unwrap_or(&r.title).to_string();
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
        Some(
            div()
                .id("crash-card")
                .w(px(300.))
                .p(px(12.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .rounded(px(radius::PANEL))
                .bg(c(pal.raised))
                .border_1()
                .border_color(c(pal.line))
                .shadow(vec![drop(tokens::shadow::MENU)])
                .occlude()
                .aria_label("Cauldron crashed last time")
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().size(px(8.)).flex_none().rounded_full().bg(c(pal.bad)))
                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Cauldron crashed last time")),
                )
                .child(div().text_size(px(11.5)).line_height(px(16.)).text_color(fg2).truncate().child(what))
                .child(
                    div()
                        .text_size(px(11.5))
                        .line_height(px(16.))
                        .text_color(fg2)
                        .child("Report it as a GitHub issue: the page opens filled in with the error and the log, and nothing is sent until you submit it."),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .pt(px(4.))
                        .child(button("crash-report", "Report on GitHub", true).on_click(cx.listener(|this, _, _, cx| this.report_crash(cx))).test_support())
                        .child(button("crash-dismiss", "Dismiss", false).on_click(cx.listener(|this, _, _, cx| this.dismiss_crash(cx))).test_support()),
                ),
        )
    }

    fn report_crash(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.crash.take() else { return };
        // The whole report too, for when the page had to cut it to fit the address.
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(r.text.clone()));
        cx.open_url(&crash::issue_url(&r));
        crash::set_aside(&r);
        self.crash = crash::pending();
        cx.notify();
    }

    /// Sets aside every waiting crash, not only this one: one bad run can leave several.
    fn dismiss_crash(&mut self, cx: &mut Context<Self>) {
        let mut next = self.crash.take();
        while let Some(r) = next {
            crash::set_aside(&r);
            next = crash::pending().filter(|n| n.path != r.path);
        }
        cx.notify();
    }
}
