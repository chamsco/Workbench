//! The Effort and trust chips open a menu (Charles, 22 Nov: "when you click on it, show me how
//! it will look"): every option with what it does, the current one checked. Picking saves it to
//! prefs. In the reply box the menu rises over the chip; in the new-stream box it drops under it.

use super::*;
use gpui_kit::prelude::FluentBuilder;

pub(crate) const EFFORT_HELP: [(&str, &str); 5] = [
    ("Auto", "The CLI picks how hard to think, turn by turn."),
    ("Low", "Fast and cheap: small edits and quick questions."),
    ("Medium", "A balance for everyday work."),
    ("High", "Thinks longer before acting: hard bugs, design."),
    ("Max", "As much thinking as the model allows. Slow and costly."),
];

pub(crate) const TRUST_HELP: [&str; 3] = [
    "Asks before every edit and command.",
    "Edits and safe commands go ahead; anything risky still asks.",
    "Edits and runs anything without asking. Use in a folder you can throw away.",
];

impl Cauldron {
    /// Opens or closes `which` ("effort" or "trust") for the chip `id`.
    pub(super) fn toggle_chip_menu(&mut self, id: &'static str, cx: &mut Context<Self>) {
        self.chip_menu = if self.chip_menu == Some(id) { None } else { Some(id) };
        cx.notify();
    }

    /// The menu for chip `id`, when it is open: options, help, a check on the current one.
    #[inline(never)]
    pub(super) fn chip_menu_el(&self, id: &'static str, pal: &'static Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.chip_menu != Some(id) {
            return None;
        }
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let p = Prefs::load();
        let effort = id.ends_with("effort");
        let rows: Vec<(String, &'static str, bool)> = if effort {
            let now = super::thread::EFFORTS.iter().position(|e| *e == p.cli_effort.as_deref()).unwrap_or(0);
            EFFORT_HELP.iter().enumerate().map(|(i, (l, h))| (l.to_string(), *h, i == now)).collect()
        } else {
            super::thread::TRUST.iter().zip(TRUST_HELP).map(|((k, l), h)| (l.to_string(), h, *k == p.cli_permission || (p.cli_permission.is_empty() && *k == "edits"))).collect()
        };
        let mut menu = div()
            .id(SharedString::from(format!("{id}-menu")))
            .w(px(280.))
            .p(px(4.))
            .flex()
            .flex_col()
            .rounded(px(radius::PANEL))
            .border_1()
            .border_color(c(pal.line))
            .bg(c(pal.raised))
            .shadow(super::lift(super::Lift::Card, pal))
            .occlude()
            .aria_label(if effort { "Effort" } else { "What it may do without asking" })
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.chip_menu = None;
                cx.notify();
            }))
            .child(div().px(px(8.)).pt(px(6.)).pb(px(4.)).text_size(px(11.)).text_color(fg3).child(if effort { "Reasoning effort" } else { "What it may do without asking" }));
        for (i, (label, help, on)) in rows.into_iter().enumerate() {
            menu = menu.child(
                div()
                    .id((if effort { "effort-opt" } else { "trust-opt" }, i))
                    .px(px(8.))
                    .py(px(6.))
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .when(on, |d| d.bg(c(pal.selected)))
                    .hover(|d| d.bg(c(pal.hover)))
                    .aria_label(label.clone())
                    .aria_selected(on)
                    .aria_description(help)
                    .child(div().w(px(14.)).pt(px(2.)).flex_none().when(on, |d| d.child(glyph("check", 13., c(pal.accent)))))
                    .child(div().flex_1().min_w_0().flex().flex_col().gap(px(1.)).child(div().text_size(px(13.)).text_color(fg).child(label)).child(div().text_size(px(11.5)).line_height(px(16.)).text_color(fg3).child(help)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            let mut p = Prefs::load();
                            if effort {
                                p.cli_effort = super::thread::EFFORTS[i].map(str::to_string);
                            } else {
                                p.cli_permission = super::thread::TRUST[i].0.to_string();
                            }
                            p.save();
                            this.chip_menu = None;
                            cx.notify();
                        }),
                    )
                    .test_support(),
            );
        }
        let up = !id.starts_with("nt-");
        let place = div().absolute().when(up, |d| d.bottom(px(30.))).when(!up, |d| d.top(px(30.))).when(effort, |d| d.right_0()).when(!effort, |d| d.left_0());
        Some(deferred(place.child(menu.test_support())).with_priority(3).into_any_element())
    }
}
