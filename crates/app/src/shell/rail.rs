//! The skinny rail's icons: the sidebar at its narrowest, and the band that slides in over a folded
//! sidebar (Charles, 23 Nov: the rail is the sidebar's minimum, and what hover shows once it is
//! collapsed). New stream, the apps, Settings and you, stacked; the band adds Search on top.

use super::*;
use gpui_kit::prelude::FluentBuilder;

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
        // A rule after the apps, then the open app's own items (Charles, 23 Nov: "a separator between the
        // apps and the current sidebar element"): streams, or crews, as round buttons.
        let apps_end = rule_y + 9.0 + 32.0 * FUNCTIONS.len() as f32;
        host = host.child(div().absolute().left(px(x0 + 3.0)).top(px(apps_end + 2.0 + dy)).w(px(22.)).h(px(1.)).bg(c(pal.line)));
        let update_y = hh - 104.0;
        let room = ((update_y - (apps_end + 12.0)) / 32.0).floor().max(0.0) as usize;
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let round = |id: ElementId, label: String, initial: String, ring: Hsla, on: bool| {
            div()
                .id(id)
                .size(px(26.))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_1()
                .border_color(ring)
                .cursor_pointer()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if on { fg } else { fg3 })
                .when(on, |d| d.bg(c(pal.selected)))
                .hover(|d| d.bg(c(pal.hover)))
                .aria_label(label)
                .child(initial)
        };
        let letter = |t: &str| t.chars().find(|ch| ch.is_alphanumeric()).map_or_else(|| "?".to_string(), |ch| ch.to_uppercase().to_string());
        let mut at_y = apps_end + 10.0;
        match FUNCTIONS[self.section].1 {
            "Streams" => {
                if let Threads::Ready(v) = &self.threads {
                    let mine = |t: &ThreadInfo| t.agent.as_deref().is_some_and(|a| self.agent_cached(a).is_some());
                    for (i, t) in v.iter().filter(|t| !t.archived && t.side_of.is_none() && !mine(t)).take(room).enumerate() {
                        let ring = match threads::Status::of(t) {
                            Some(threads::Status::Working) => c(pal.accent),
                            Some(threads::Status::Failed) => c(pal.bad),
                            Some(threads::Status::Input) => c(pal.warn),
                            Some(threads::Status::Done) => c(pal.ok),
                            None => c(pal.line),
                        };
                        let id = t.id.clone();
                        let on = self.open.as_deref() == Some(t.id.as_str());
                        let b = round(ElementId::NamedInteger("rail-stream".into(), i as u64), t.title.clone(), letter(&t.title), ring, on).on_click(cx.listener(move |this, _, _, cx| {
                            this.selected = Some(id.clone());
                            this.open_thread(id.clone());
                            this.indent = false;
                            cx.notify();
                        }));
                        host = host.child(div().absolute().left(px(x0 + 1.0)).top(px(at_y + dy)).child(self.tipped(ElementId::NamedInteger("rail-stream".into(), i as u64), b, t.title.clone(), Side::Right, pal, cx).test_support()));
                        at_y += 32.0;
                    }
                }
            }
            "Agents" => {
                for (i, a) in self.start.captains.iter().take(room).enumerate() {
                    let id = a.id.clone();
                    let on = self.agent_open.as_deref() == Some(a.id.as_str());
                    let b = round(ElementId::NamedInteger("rail-crew".into(), i as u64), a.name.clone(), letter(&a.name), c(pal.line), on).on_click(cx.listener(move |this, _, window, cx| {
                        this.show_agent(&id, window, cx);
                        this.indent = false;
                    }));
                    host = host.child(div().absolute().left(px(x0 + 1.0)).top(px(at_y + dy)).child(self.tipped(ElementId::NamedInteger("rail-crew".into(), i as u64), b, a.name.clone(), Side::Right, pal, cx).test_support()));
                    at_y += 32.0;
                }
            }
            _ => {}
        }
        // The update, as a circle: warm with an arrow while something is newer, quiet otherwise. It does
        // what the version button does: opens the card and checks (there is no automatic check).
        let newer = self.updates_pending() > 0;
        let tone = if newer { c(pal.warn) } else { fg3 };
        let b = div()
            .id("rail-update")
            .size(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(if newer { tone } else { c(pal.line) })
            .cursor_pointer()
            .hover(|d| d.bg(c(pal.hover)))
            .when(self.update_open, |d| d.bg(c(pal.selected)))
            .aria_label(if newer { "Updates available" } else { "Check for updates" })
            .child(if newer { glyph("arrow-up", 13., tone).into_any_element() } else { div().size(px(6.)).rounded_full().bg(c(pal.accent)).into_any_element() })
            .on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                if this.update_open {
                    this.update_open = false;
                    cx.notify();
                } else {
                    this.check_for_update(cx);
                }
            }));
        let label = if newer { "Updates available" } else { "Check for updates" };
        host = host.child(at(update_y, self.tipped("rail-update", b, label, Side::Right, pal, cx).test_support().into_any_element()));
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
