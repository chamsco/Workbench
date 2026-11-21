//! The decision card (Charles, 19 Nov: the board's Decision B, with changes). When a familiar's
//! agent calls `show_options` (core `options.rs`), its open question docks above the familiar's
//! composer as a compact card: the question and one chip per option. Hovering grows it to show each
//! option's note, preview and Pick button; clicking an option opens it in the Highlights pane on the
//! right, full size, with Pick there too. A pick goes back to the agent as the tool's result.
//!
//! Plain streams (CLI chats) have no ask path in core, so only familiars show the card.

use super::*;
use backspace_core::options::{OptionSet, Variant};

/// The Highlights tab an opened option shows in, after Gates, Changes and Budget.
pub(crate) const OPTION_TAB: usize = 3;

/// Debug builds: the made-up question the first fixture familiar asks, for screenshots. Previews
/// come from `CAULDRON_FIXTURE_BOARDS=<dir>` (a.png, b.png, c.png) when it is set.
pub(crate) fn fixture_set() -> OptionSet {
    let dir = std::env::var("CAULDRON_FIXTURE_BOARDS").ok();
    let img = |id: &str| dir.as_ref().map(|d| format!("{d}/{id}.png"));
    let v = |id: &str, label: &str, note: &str, rec: bool| Variant { id: id.into(), label: label.into(), note: note.into(), image: img(id), url: None, recommended: rec };
    OptionSet {
        id: "fx-opt".into(),
        title: "How should onboarding handle old chats?".into(),
        note: "T3 can't read the legacy export. This decides whether v2 ships this week or waits for a sample file.".into(),
        variants: vec![
            v("a", "Skip import", "Ship v2 now; import arrives in a later build.", true),
            v("b", "Wait for a sample", "Import works on day one, but v2 waits on you.", false),
            v("c", "Paste instead", "A paste box replaces the file picker for now.", false),
        ],
        picked: None,
        at: 0,
        by: "Familiar".into(),
    }
}

/// The latest set still waiting for an answer.
pub(crate) fn pending(sets: &[OptionSet]) -> Option<&OptionSet> {
    sets.iter().rev().find(|o| o.picked.is_none())
}

/// A–F for the variant at `i`.
pub(crate) fn letter(i: usize) -> char {
    (b'A' + i.min(25) as u8) as char
}

impl Cauldron {
    /// The open familiar's waiting question, if it has one.
    pub(super) fn fam_question(&self) -> Option<OptionSet> {
        let f = self.fam_views.get(self.familiar?)?;
        if self.fam_live.iter().any(|l| l.rec.id == f.id) {
            return self.fam_questions.get(&f.id).cloned().flatten().filter(|s| !self.ask_answered.contains(&s.id));
        }
        (self.fam_fixture && self.familiar == Some(0) && !self.ask_answered.contains("fx-opt")).then(fixture_set)
    }

    /// Sends the pick to the agent (or, for the made-up familiar, just records it).
    fn pick_option(&mut self, set: &str, variant: &str, cx: &mut Context<Self>) {
        if let Some(l) = self.open_live() {
            l.h.pick_option(set, Some(variant));
        }
        self.ask_answered.insert(set.to_string());
        self.ask_open = None;
        self.ask_hover = false;
        if self.fam_hl == OPTION_TAB {
            self.fam_hl = 0;
        }
        cx.notify();
    }

    /// The docked card above the familiar's composer.
    pub(super) fn decision_card(&self, set: &OptionSet, pal: &'static Palette, cx: &mut Context<Self>) -> AnyElement {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let big = self.ask_hover;
        let mut card = div()
            .id("decision-card")
            .w_full()
            .max_w(px(560.))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(if big { 8. } else { 6. }))
            .p(px(10.))
            .rounded(px(10.))
            .border_1()
            .border_color(ca(pal.warn, if big { 0.75 } else { 0.5 }))
            .bg(backdrop::surface(self.painting, pal.raised, backdrop::GLASS_COMPOSER))
            .when_live(big, |d| d.shadow(vec![drop(tokens::shadow::MENU)]))
            .aria_label(format!("Needs you: {}", set.title))
            .on_hover(cx.listener(|this, on: &bool, _, cx| {
                if this.ask_hover != *on {
                    this.ask_hover = *on;
                    cx.notify();
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().size(px(7.)).flex_none().rounded_full().bg(c(pal.warn)))
                    .child(div().flex_1().min_w_0().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(set.title.clone()))
                    .child(div().flex_none().text_size(px(11.)).text_color(fg3).child(if big { "Click one to open it" } else { "Hover to compare" })),
            );
        if big && !set.note.is_empty() {
            card = card.child(div().text_size(px(12.)).line_height(px(17.)).text_color(fg2).child(set.note.clone()));
        }
        if !big {
            // Compact: one chip per option.
            let mut chips = div().flex().flex_wrap().gap(px(6.));
            for (i, v) in set.variants.iter().enumerate() {
                let open = self.ask_open.as_deref() == Some(v.id.as_str());
                chips = chips.child(
                    div()
                        .id(("decision-chip", i))
                        .h(px(24.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .rounded(px(radius::CONTROL))
                        .border_1()
                        .border_color(c(if open { pal.line_strong } else { pal.line }))
                        .bg(ca(pal.fg, if open { 0.08 } else { 0.03 }))
                        .text_size(px(12.))
                        .text_color(fg2)
                        .aria_label(format!("{}. {}", letter(i), v.label))
                        .child(div().font_weight(FontWeight::SEMIBOLD).text_color(fg).child(format!("{}.", letter(i))))
                        .child(v.label.clone())
                        .when_live(v.recommended, |d| d.child(glyph("star", 11., c(pal.accent))))
                        .test_support(),
                );
            }
            return card.child(chips).test_support().into_any_element();
        }
        // Grown: a row per option with its note, preview and Pick.
        for (i, v) in set.variants.iter().enumerate() {
            let open = self.ask_open.as_deref() == Some(v.id.as_str());
            let (sid, vid, vid2) = (set.id.clone(), v.id.clone(), v.id.clone());
            let mut text = div().flex_1().min_w_0().flex().flex_col().gap(px(2.)).child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(12.5))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(fg).child(format!("{}. {}", letter(i), v.label)))
                    .when_live(v.recommended, |d| d.child(div().flex().items_center().gap(px(3.)).text_size(px(11.)).text_color(c(pal.accent)).child(glyph("star", 11., c(pal.accent))).child("Recommended"))),
            );
            if !v.note.is_empty() {
                text = text.child(div().text_size(px(11.5)).line_height(px(16.)).text_color(fg3).child(v.note.clone()));
            }
            let mut row = div()
                .id(("decision-row", i))
                .flex()
                .items_center()
                .gap(px(10.))
                .p(px(8.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .border_1()
                .border_color(c(if open { pal.line_strong } else { pal.line }))
                .bg(ca(pal.fg, if open { 0.07 } else { 0.02 }))
                .hover(|d| d.bg(ca(pal.fg, 0.06)))
                .aria_label(format!("{}. {}: {}", letter(i), v.label, v.note))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.ask_open = Some(vid.clone());
                    this.fam_hl = OPTION_TAB;
                    cx.notify();
                }));
            if let Some(p) = v.image.as_deref().filter(|p| !p.trim().is_empty()) {
                row = row.child(img(self.option_image(p)).w(px(72.)).h(px(44.)).flex_none().rounded(px(radius::TAG)).object_fit(ObjectFit::Cover));
            }
            row = row.child(text).child(
                div()
                    .id(("decision-pick", i))
                    .h(px(24.))
                    .px(px(10.))
                    .flex()
                    .flex_none()
                    .items_center()
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .when_live(v.recommended, |d| d.bg(c(pal.accent)).text_color(c(pal.on_accent)))
                    .when_live(!v.recommended, |d| d.border_1().border_color(c(pal.line_strong)).text_color(fg2).hover(|d| d.bg(ca(pal.fg, 0.06))))
                    .aria_label(format!("Pick {}", letter(i)))
                    .child("Pick")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.pick_option(&sid, &vid2, cx);
                    }))
                    .test_support(),
            );
            card = card.child(row.test_support());
        }
        card.child(div().text_size(px(11.)).text_color(fg3).child("Pick sends your answer to the familiar. Anything else: type it below.")).test_support().into_any_element()
    }

    /// An option's preview: a path in the familiar's folder, or an absolute one.
    fn option_image(&self, p: &str) -> std::path::PathBuf {
        let path = std::path::PathBuf::from(p);
        if path.is_absolute() {
            return path;
        }
        match self.open_live() {
            Some(l) => l.h.root().join(path),
            None => path,
        }
    }

    /// The opened option, full size, in the Highlights pane.
    pub(super) fn option_pane(&self, set: &OptionSet, pal: &'static Palette, cx: &mut Context<Self>) -> Div {
        let (fg, fg2, fg3) = (c(pal.fg), c(pal.fg2), c(pal.fg3));
        let i = self.ask_open.as_ref().and_then(|id| set.variants.iter().position(|v| &v.id == id)).unwrap_or(0);
        let Some(v) = set.variants.get(i) else { return div() };
        let (sid, vid) = (set.id.clone(), v.id.clone());
        let mut d = div().flex().flex_col().gap(px(8.)).child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(13.))
                .child(div().font_weight(FontWeight::SEMIBOLD).text_color(fg).child(format!("{}. {}", letter(i), v.label)))
                .when_live(v.recommended, |d| d.child(glyph("star", 12., c(pal.accent)))),
        );
        match v.image.as_deref().filter(|p| !p.trim().is_empty()) {
            Some(p) => d = d.child(img(self.option_image(p)).id("option-preview").w_full().h(px(170.)).rounded(px(radius::CONTROL)).object_fit(ObjectFit::Contain).test_support()),
            None => d = d.child(div().id("option-preview").h(px(60.)).flex().items_center().justify_center().rounded(px(radius::CONTROL)).border_1().border_color(c(pal.line)).text_size(px(11.5)).text_color(fg3).child("No preview: a text option").test_support()),
        }
        if !v.note.is_empty() {
            d = d.child(div().text_size(px(12.)).line_height(px(17.)).text_color(fg2).child(v.note.clone()));
        }
        d.child(
            div()
                .id("option-pane-pick")
                .shadow(super::lift(super::Lift::Button, pal))
                .h(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .bg(c(pal.accent))
                .text_color(c(pal.on_accent))
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .aria_label(format!("Pick {}", letter(i)))
                .child(format!("Pick {}", letter(i)))
                .on_click(cx.listener(move |this, _, _, cx| this.pick_option(&sid, &vid, cx)))
                .test_support(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The card shows the newest set still waiting; answered or dismissed ones are done.
    #[::core::prelude::v1::test]
    fn the_card_shows_the_newest_unanswered_set() {
        let mut a = fixture_set();
        a.id = "opt1".into();
        let mut b = fixture_set();
        b.id = "opt2".into();
        assert_eq!(pending(&[a.clone(), b.clone()]).map(|s| s.id.as_str()), Some("opt2"));
        b.picked = Some("a".into());
        assert_eq!(pending(&[a.clone(), b.clone()]).map(|s| s.id.as_str()), Some("opt1"));
        a.picked = Some(String::new());
        assert!(pending(&[a, b]).is_none(), "a dismissed set is answered too");
        assert_eq!((letter(0), letter(2)), ('A', 'C'));
        assert_eq!(fixture_set().variants.iter().filter(|v| v.recommended).count(), 1);
    }
}
