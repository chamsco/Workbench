//! The skinny rail's icons: the sidebar at its narrowest, and the band that slides in over a folded
//! sidebar (Charles, 23 Nov: the rail is the sidebar's minimum, and what hover shows once it is
//! collapsed). New stream, the apps, Settings and you, stacked; the band adds Search on top.

use super::*;

impl Cauldron {
    /// Adds the rail's buttons to `host`, `bw` wide and `hh` high; `dy` shifts every y (a host that
    /// starts below the window's top), `search` puts Search first.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn rail_children<H: ParentElement>(&self, mut host: H, bw: f32, hh: f32, dy: f32, search: bool, plat: Platform, pal: &'static Palette, accent: Hsla, cx: &mut Context<Self>) -> H {
        let x0 = (bw - metric::SBTN) / 2.0;
        let at = |y: f32, el: AnyElement| div().absolute().left(px(x0)).top(px(y + dy)).child(el);
        // Below the lights on macOS, in their place on Windows.
        let top = if plat == Platform::Mac { 47.0 } else { 10.0 };
        let step = if search { 32.0 + if plat == Platform::Mac { 0.0 } else { 5.0 } } else { 0.0 };
        if search {
            host = host.child(at(top, self.tipped("indent-search", icon_button("indent-search", "search", "Search streams", pal, false), "Search streams", Side::Right, pal, cx).on_click(cx.listener(|this, _, window, cx| {
                this.indent = false;
                this.open_spotlight(window, cx);
            })).test_support().into_any_element()));
        }
        host = host.child(at(top + step, self.tipped("indent-new", icon_button("indent-new", "square-pen", NEW_TAB, pal, false), NEW_TAB, Side::Right, pal, cx).on_click(cx.listener(|this, _, window, cx| {
            this.indent = false;
            this.new_thread(window, cx);
        })).test_support().into_any_element()));
        let rule_y = top + step + 36.0;
        host = host.child(div().absolute().left(px(x0 + 3.0)).top(px(rule_y + dy)).w(px(22.)).h(px(1.)).bg(c(pal.line)));
        for (i, (name, label)) in FUNCTIONS.iter().enumerate() {
            let b = icon_button(("indent-fn", i), name, label, pal, i == self.section).on_click(cx.listener(move |this, _, _, cx| this.select_section(i, cx)));
            host = host.child(at(rule_y + 9.0 + 32.0 * i as f32, self.tipped(("indent-fn", i), b, *label, Side::Right, pal, cx).test_support().into_any_element()));
        }
        host = host.child(at(hh - 70.0, self.tipped("indent-settings", icon_button("indent-settings", "settings", "Settings", pal, false), "Settings", Side::Right, pal, cx).on_click(cx.listener(|this, _, _, cx| {
            this.settings = Some(0);
            this.sidebar_open = true;
            this.indent = false;
            cx.notify();
        })).test_support().into_any_element()));
        let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "You".into());
        let initial = user.chars().next().map(|ch| ch.to_uppercase().to_string()).unwrap_or_else(|| "Y".into());
        host.child(
            div()
                .absolute()
                .left(px((bw - 24.0) / 2.0))
                .top(px(hh - 34.0 + dy))
                .size(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(accent)
                .text_size(px(11.))
                .text_color(c(pal.on_accent))
                .child(initial),
        )
    }
}
