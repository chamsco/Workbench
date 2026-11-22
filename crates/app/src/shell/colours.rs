//! Settings > Appearance, colours (board round 6, picks B and themes; Charles 21 Nov 04:59Z and
//! 05:04Z): a live miniature of the window is the control. Click the frame (sidebar and the edge
//! around the card) or the main card in it to choose what the swatches set. The miniature's card
//! holds a mock thread and a mock chat box so the colours and the picture read as they will.
//! Themes are saved combinations of the two colours, the backdrop and the picture.

use super::theme::{self, Target};
use super::*;

/// How long a new picture takes to wipe in over the old one, left to right.
pub(crate) const WIPE_MS: u128 = 650;

/// Built-in themes: (name, index into `theme::presets` for the frame and main colours).
const BUILT_IN: [(&str, usize); 6] = [("Default", 0), ("Slate", 1), ("Plum", 2), ("Forest", 3), ("Ember", 4), ("Ocean", 5)];

fn hex(v: u32) -> String {
    format!("#{v:06x}")
}

fn hex_of(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

/// A saved theme, split out of its pref line.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Saved {
    pub name: String,
    pub frame: Option<String>,
    pub main: Option<String>,
    pub backdrop: Option<String>,
    pub picture: Option<String>,
    pub dim: Option<String>,
}

fn opt(s: &str) -> Option<String> {
    (s != "-" && !s.is_empty()).then(|| s.to_string())
}

impl Saved {
    pub(crate) fn parse(line: &str) -> Option<Saved> {
        let f: Vec<&str> = line.split('|').collect();
        (f.len() == 6 && !f[0].is_empty()).then(|| Saved { name: f[0].to_string(), frame: opt(f[1]), main: opt(f[2]), backdrop: opt(f[3]), picture: opt(f[4]), dim: opt(f[5]) })
    }

    pub(crate) fn line(&self) -> String {
        let o = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".into());
        // A `|` in the name would split the line.
        format!("{}|{}|{}|{}|{}|{}", self.name.replace('|', "/"), o(&self.frame), o(&self.main), o(&self.backdrop), o(&self.picture), o(&self.dim))
    }

    pub(crate) fn of(name: String, p: &Prefs) -> Saved {
        Saved { name, frame: p.sidebar_color.clone(), main: p.main_color.clone(), backdrop: Some(p.backdrop.clone()), picture: p.stream_bg.clone(), dim: p.stream_dim.clone() }
    }
}

impl Cauldron {
    /// The picture layers for the card, `w` wide: the new picture, wiping in from the left over the
    /// old one while a wipe runs. The scrim goes on top.
    pub(super) fn picture_layers(&self, w: f32, round: Pixels, pal: &'static Palette) -> Option<Vec<AnyElement>> {
        let path = self.stream_bg.clone()?;
        let scrim = || div().absolute().left_0().top_0().size_full().rounded(round).bg(backdrop::surface(true, pal.frame, backdrop::dim_alpha(&Prefs::load()))).into_any_element();
        let full = |p: std::path::PathBuf| img(p).absolute().left_0().top_0().size_full().rounded(round).object_fit(ObjectFit::Cover);
        let mut out = Vec::new();
        match &self.bg_wipe {
            Some((old, since)) if since.elapsed().as_millis() < WIPE_MS => {
                if let Some(old) = old.clone() {
                    out.push(full(old).into_any_element());
                }
                let t = since.elapsed().as_millis() as f32 / WIPE_MS as f32;
                let eased = 1.0 - (1.0 - t).powi(3);
                // The new picture is the card's size, shown through a window that opens rightward.
                out.push(
                    div()
                        .id("stream-bg-wipe")
                        .absolute()
                        .left_0()
                        .top_0()
                        .h_full()
                        .w(px(w * eased))
                        .overflow_hidden()
                        .rounded(round)
                        .child(img(path).absolute().left_0().top_0().w(px(w)).h_full().object_fit(ObjectFit::Cover))
                        .test_support()
                        .into_any_element(),
                );
            }
            _ => out.push(full(path).id("stream-bg").test_support().into_any_element()),
        }
        out.push(scrim());
        Some(out)
    }

    /// Saves one of the two colours (None: the theme's) and repaints.
    fn set_colour(&mut self, target: Target, value: Option<String>, cx: &mut Context<Self>) {
        let mut p = Prefs::load();
        match target {
            Target::Sidebar => p.sidebar_color = value,
            Target::Main => p.main_color = value,
        }
        p.save();
        cx.notify();
    }

    /// Applies a saved theme: colours, backdrop, picture and dimming together.
    fn apply_saved(&mut self, t: &Saved, cx: &mut Context<Self>) {
        let mut p = Prefs::load();
        p.sidebar_color = t.frame.clone();
        p.main_color = t.main.clone();
        if let Some(b) = &t.backdrop {
            p.backdrop = b.clone();
        }
        p.stream_dim = t.dim.clone();
        let pic = t.picture.as_ref().map(std::path::PathBuf::from);
        p.save();
        self.painting = backdrop::wants_painting(&p);
        if self.painting {
            Self::warm_glass(cx);
        }
        self.set_stream_bg(pic.filter(|p| p.is_file()), cx);
    }

    /// The Colours section: miniature, swatches, custom hex, themes.
    #[inline(never)]
    pub(super) fn colour_block(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let fg3 = c(pal.fg3);
        let mut out: Vec<AnyElement> = Vec::new();
        out.push(div().mt(px(24.)).mb(px(8.)).text_size(px(11.)).line_height(px(14.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child("Colours").into_any_element());
        out.push(div().mb(px(10.)).text_size(px(12.)).line_height(px(16.)).text_color(fg3).child("Click the frame or the main card to choose what the swatches colour.").into_any_element());
        out.push(self.colour_mini(pal, cx));
        out.extend(self.colour_swatches(pal, cx));
        out.extend(self.colour_themes(pal, cx));
        out
    }

    #[inline(never)]
    fn colour_mini(&self, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let dark = pal.panel.0 < 0x80;
        let p = Prefs::load();
        let (side, main) = (theme::colour(&p, Target::Sidebar, dark), theme::colour(&p, Target::Main, dark));
        let target = self.color_target;
        let ring = |on: bool| if on { c(pal.accent) } else { c(pal.line_strong) };
        // The miniature: the whole frame is one colour, the card another.
        let round = px(radius::PANEL * 0.6);
        let bar = |w: f32, a: f32| div().h(px(5.)).w(px(w)).rounded(px(3.)).bg(c(pal.fg3)).opacity(a);
        let mut card = div()
            .id("mini-main")
            .absolute()
            .left(px(132.))
            .top(px(10.))
            .right(px(10.))
            .bottom(px(10.))
            .rounded(round)
            .overflow_hidden()
            .bg(c(main))
            .border_2()
            .border_color(if target == Target::Main { c(pal.accent) } else { c(pal.line) })
            .cursor_pointer()
            .occlude()
            .aria_label("Main card")
            .aria_selected(target == Target::Main)
            .on_click(cx.listener(|this, _, _, cx| {
                this.color_target = Target::Main;
                this.hex_input = None;
                cx.notify();
            }));
        if let Some(layers) = self.picture_layers(420., round, pal) {
            for l in layers {
                card = card.child(l);
            }
        }
        let mock = colour_mock(pal);
        card = card.child(mock);
        let card = card.test_support();
        let mut side_bars = div().absolute().left(px(14.)).top(px(16.)).w(px(100.)).flex().flex_col().gap(px(9.));
        for (w, a) in [(70., 0.9), (90., 0.5), (80., 0.5), (60., 0.5), (85., 0.5)] {
            side_bars = side_bars.child(bar(w, a));
        }
        let frame = div()
            .id("mini-frame")
            .relative()
            .w(px(560.))
            .h(px(280.))
            .rounded(px(radius::PANEL))
            .bg(c(side))
            .border_2()
            .border_color(ring(target == Target::Sidebar))
            .overflow_hidden()
            .aria_label("Frame")
            .aria_selected(target == Target::Sidebar)
            // The frame's own hit area sits under the card; the card blocks it.
            .child(
                div().id("mini-frame-hit").absolute().left_0().top_0().size_full().cursor_pointer().on_click(cx.listener(|this, _, _, cx| {
                    this.color_target = Target::Sidebar;
                    this.hex_input = None;
                    cx.notify();
                })),
            )
            .child(side_bars)
            .child(card)
            .test_support();
        div().mb(px(12.)).child(frame).into_any_element()

    }

    #[inline(never)]
    fn colour_swatches(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (fg, fg2) = (c(pal.fg), c(pal.fg2));
        let dark = pal.panel.0 < 0x80;
        let p = Prefs::load();
        let target = self.color_target;
        let mut out: Vec<AnyElement> = Vec::new();
        // Swatches for the chosen target, with the custom colour and reset.
        let (label, saved) = match target {
            Target::Sidebar => ("Frame", p.sidebar_color.clone()),
            Target::Main => ("Main", p.main_color.clone()),
        };
        let current = saved.as_deref().and_then(theme::parse_hex).map(|c| hex_of(theme::clamp(c, dark)));
        let mut swatches = div().flex().items_center().gap(px(6.));
        swatches = swatches.child(div().w(px(44.)).text_size(px(12.)).text_color(fg2).child(label));
        for (i, v) in theme::presets(target, dark).into_iter().enumerate() {
            let value = hex(v);
            let on = match &current {
                Some(cur) => *cur == value,
                None => i == 0,
            };
            swatches = swatches.child(
                div()
                    .id(("swatch", i))
                    .size(px(26.))
                    .rounded(px(7.))
                    .bg(c(Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)))
                    .border_2()
                    .border_color(if on { c(pal.accent) } else { c(pal.line_strong) })
                    .cursor_pointer()
                    .aria_label(format!("{label} {value}"))
                    .aria_selected(on)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_colour(target, (i > 0).then(|| value.clone()), cx)))
                    .test_support(),
            );
        }
        let button = |id: &'static str, text: &'static str| {
            div().id(id).h(px(26.)).px(px(10.)).flex().items_center().rounded(px(radius::CONTROL)).cursor_pointer().button_face(pal).shadow(lift(Lift::Button, pal)).text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(fg).aria_label(text).child(text)
        };
        swatches = swatches.child(
            button("colour-custom", "Custom…")
                .on_click(cx.listener(|this, _, window, cx| {
                    if this.hex_input.is_some() {
                        this.hex_input = None;
                    } else {
                        let st = cx.new(|cx| InputState::new(window, cx).placeholder("#1a1d25"));
                        let sub = cx.subscribe(&st, |this, st, ev: &InputEvent, cx| {
                            if let InputEvent::Change = ev {
                                let text = st.read(cx).value().to_string();
                                if theme::parse_hex(&text).is_some() {
                                    let t = this.color_target;
                                    this.set_colour(t, Some(format!("#{}", text.trim().trim_start_matches('#').to_ascii_lowercase())), cx);
                                }
                            }
                        });
                        st.read(cx).focus_handle(cx).focus(window, cx);
                        this.hex_input = Some((st, sub));
                    }
                    cx.notify();
                }))
                .test_support(),
        );
        out.push(div().mb(px(8.)).child(swatches).into_any_element());
        if let Some((st, _)) = &self.hex_input {
            out.push(
                div()
                    .mb(px(8.))
                    .ml(px(50.))
                    .w(px(140.))
                    .h(px(28.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(radius::CONTROL))
                    .border_1()
                    .border_color(c(pal.line_strong))
                    .child(Input::new(st).appearance(false).text_size(px(12.5)).p_0())
                    .into_any_element(),
            );
        }
        out.push(
            div()
                .flex()
                .gap(px(8.))
                .mb(px(4.))
                .child(button("colour-reset", "Reset colours").on_click(cx.listener(|this, _, _, cx| {
                    let mut p = Prefs::load();
                    p.sidebar_color = None;
                    p.main_color = None;
                    p.save();
                    this.hex_input = None;
                    cx.notify();
                })).test_support())
                .into_any_element(),
        );

        out
    }

    #[inline(never)]
    fn colour_themes(&self, pal: &'static Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (fg, fg3) = (c(pal.fg), c(pal.fg3));
        let dark = pal.panel.0 < 0x80;
        let p = Prefs::load();
        let mut out: Vec<AnyElement> = Vec::new();
        let button = |id: &'static str, text: &'static str| {
            div().id(id).h(px(26.)).px(px(10.)).flex().items_center().rounded(px(radius::CONTROL)).cursor_pointer().button_face(pal).shadow(lift(Lift::Button, pal)).text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(fg).aria_label(text).child(text)
        };
        // Themes: built in, then saved ones.
        out.push(div().mt(px(20.)).mb(px(6.)).text_size(px(11.)).line_height(px(14.)).font_weight(FontWeight::MEDIUM).text_color(fg3).child("Themes").into_any_element());
        let mut chips = div().flex().flex_wrap().gap(px(6.));
        for (i, (name, ix)) in BUILT_IN.into_iter().enumerate() {
            let (f, m) = (theme::presets(Target::Sidebar, dark)[ix], theme::presets(Target::Main, dark)[ix]);
            let t = Saved { name: name.into(), frame: (ix > 0).then(|| hex(f)), main: (ix > 0).then(|| hex(m)), backdrop: None, picture: None, dim: None };
            let dot = |v: u32| div().size(px(10.)).rounded(px(5.)).bg(c(Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))).border_1().border_color(c(pal.line_strong));
            chips = chips.child(
                button_chip(("theme-built", i), name, pal)
                    .child(dot(f))
                    .child(dot(m))
                    .child(name)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        // A built-in sets only the two colours.
                        let mut p = Prefs::load();
                        p.sidebar_color = t.frame.clone();
                        p.main_color = t.main.clone();
                        p.save();
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        for (i, line) in p.themes.iter().enumerate() {
            let Some(t) = Saved::parse(line) else { continue };
            let name = t.name.clone();
            let applied = t.clone();
            chips = chips.child(
                button_chip(("theme-saved", i), &name, pal)
                    .child(name.clone())
                    .child(
                        div()
                            .id(("theme-del", i))
                            .size(px(16.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .hover(|d| d.bg(c(pal.hover)))
                            .aria_label(format!("Remove {name}"))
                            .child(glyph("x", 11., fg3))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                let mut p = Prefs::load();
                                if i < p.themes.len() {
                                    p.themes.remove(i);
                                }
                                p.save();
                                cx.stop_propagation();
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.apply_saved(&applied, cx)))
                    .test_support(),
            );
        }
        out.push(div().mb(px(4.)).child(chips).into_any_element());
        out.push(div().mb(px(6.)).flex().child(
            button("theme-save", "Save current")
                .on_click(cx.listener(|_, _, _, cx| {
                    let mut p = Prefs::load();
                    let t = Saved::of(format!("My theme {}", p.themes.len() + 1), &p);
                    p.themes.push(t.line());
                    p.save();
                    cx.notify();
                }))
                .test_support(),
        ).into_any_element());
        out.push(div().text_size(px(12.)).line_height(px(16.)).text_color(fg3).child("A saved theme keeps both colours, the backdrop, the picture and its dimming.").into_any_element());
        out
    }
}

fn button_chip(id: (&'static str, usize), label: &str, pal: &'static Palette) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(26.))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .button_face(pal)
        .shadow(lift(Lift::Button, pal))
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(pal.fg))
        .aria_label(label.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::thread::tests::open;
    use super::Saved;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};

    /// A new picture wipes in over the old one, then settles as one picture.
    #[gpui_kit::test]
    fn a_new_picture_wipes_in_then_settles(cx: &mut TestAppContext) {
        let (handle, _chats) = open(cx);
        cx.simulate_window_resize(handle.into(), gpui_kit::size(gpui_kit::px(1200.), gpui_kit::px(1500.)));
        handle
            .update(cx, |c, _, _| {
                c.stream_bg = Some(std::path::PathBuf::from("new.png"));
                c.bg_wipe = Some((Some(std::path::PathBuf::from("old.png")), std::time::Instant::now()));
                c.settings = Some(2);
            })
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("stream-bg-wipe").is_some(), "the new picture opens from the left");
            assert!(window.try_find("stream-bg").is_none());
        })
        .unwrap();
        handle.update(cx, |c, _, _| c.bg_wipe = None).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("stream-bg").is_some(), "then it is just the picture");
            assert!(window.try_find("stream-bg-wipe").is_none());
        })
        .unwrap();
    }

    #[::core::prelude::v1::test]
    fn saved_themes_round_trip() {
        let t = Saved { name: "Night".into(), frame: Some("#101319".into()), main: None, backdrop: Some("plain".into()), picture: Some("/p/a.png".into()), dim: Some("light".into()) };
        assert_eq!(Saved::parse(&t.line()), Some(t));
        assert_eq!(Saved::parse("bad|line"), None);
    }
}

/// The mock thread and chat box inside the miniature's card: my message top right, the reply top
/// left under it, then the box. Not interactive.
#[inline(never)]
fn colour_mock(pal: &'static Palette) -> AnyElement {
    let fg3 = c(pal.fg3);
    let bar = |w: f32, a: f32| div().h(px(5.)).w(px(w)).rounded(px(3.)).bg(c(pal.fg3)).opacity(a);
    let mine = div().flex().justify_end().child(div().w(px(190.)).rounded(px(8.)).bg(c(pal.raised)).px(px(10.)).py(px(8.)).flex().flex_col().gap(px(5.)).child(bar(170., 0.8)).child(bar(110., 0.8)));
    let reply = div().w(px(250.)).flex().flex_col().gap(px(6.)).child(bar(240., 0.6)).child(bar(220., 0.6)).child(bar(150., 0.6));
    let chat_box = div()
        .h(px(44.))
        .rounded(px(10.))
        .bg(c(pal.raised))
        .shadow(lift(Lift::Box, pal))
        .px(px(12.))
        .flex()
        .items_center()
        .justify_between()
        .child(div().text_size(px(11.)).text_color(fg3).child("Ask anything"))
        .child(div().size(px(20.)).rounded(px(10.)).bg(c(pal.accent)));
    div().absolute().left_0().top_0().size_full().p(px(12.)).flex().flex_col().gap(px(8.)).child(mine).child(reply).child(div().flex_1()).child(chat_box).into_any_element()
}
