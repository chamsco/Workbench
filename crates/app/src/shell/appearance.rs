//! What the window looks like, read from the prefs on every frame: the theme and the painting.
//! Read on its own, field by field, so a prefs file core cannot parse (an old Tauri one, a retired
//! field, a value of the wrong type) still gives these two their saved values or sane defaults, and
//! never a half state. Old Tauri values carry over only where they map cleanly: "plain" is None,
//! the Tauri paintings are themselves; the ASCII sea, the koi and the aurora are gone, so they fall
//! back to the default, Flow moving (it plays for a few seconds, then rests).
//!
//! The background's motion lives in the same saved id, so core's prefs need no new field: "flow" is
//! Moving (plays, then rests), "flow-live" Always moving, "flow-still" or a painting Still (that
//! picture), "plain" Off.

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Theme {
    /// Follows the OS, live.
    System,
    Light,
    Dark,
}

impl Theme {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }
    pub(crate) fn next(self) -> Theme {
        match self {
            Theme::System => Theme::Light,
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::System,
        }
    }
}

/// The theme and the painting, as the window uses them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Look {
    pub theme: Theme,
    /// One of `backdrop::BACKDROPS`' ids; "plain" is None.
    pub backdrop: &'static str,
}

/// Settings > Appearance > Background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Motion {
    /// Plays for `REST_AFTER` on launch, on focus and on each New stream, then rests.
    Moving,
    /// Moves whenever it may (about 10% of a core, measured).
    Always,
    Still,
    Off,
}

impl Motion {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Motion::Moving => "Moving",
            Motion::Always => "Always moving",
            Motion::Still => "Still",
            Motion::Off => "Off",
        }
    }
}

impl Look {
    pub(crate) fn motion(&self) -> Motion {
        match self.backdrop {
            "flow" => Motion::Moving,
            "flow-live" => Motion::Always,
            "plain" => Motion::Off,
            _ => Motion::Still,
        }
    }
}

/// The saved id for the next Background choice: Moving, Always moving, Still (Flow held still), Off.
pub(crate) fn next_motion(m: Motion) -> &'static str {
    match m {
        Motion::Moving => "flow-live",
        Motion::Always => "flow-still",
        Motion::Still => "plain",
        Motion::Off => "flow",
    }
}

/// How long Moving plays before it rests.
pub(crate) const REST_AFTER: std::time::Duration = std::time::Duration::from_secs(8);

/// Moving's play-then-rest timer (the coordinator, 13 Nov 19:38Z): it plays for `REST_AFTER` on
/// launch, when the window comes back to the front, and each time New stream is shown, then holds
/// its last frame. Nothing else wakes it; typing in the composer does not.
#[derive(Default, Debug)]
pub(crate) struct Burst {
    until: Option<std::time::Instant>,
    /// The window in front, and New stream shown, at the last look.
    last: Option<(bool, bool)>,
}

impl Burst {
    /// Whether it plays at `now`, given whether the window is in front and New stream is shown.
    pub(crate) fn playing(&mut self, now: std::time::Instant, active: bool, new_stream: bool) -> bool {
        let woke = match self.last {
            None => true,
            Some((was_active, was_new)) => (active && !was_active) || (new_stream && !was_new),
        };
        self.last = Some((active, new_stream));
        if woke {
            self.until = Some(now + REST_AFTER);
        }
        self.until.is_some_and(|u| now < u)
    }
}

impl Default for Look {
    fn default() -> Look {
        Look { theme: Theme::System, backdrop: "flow" }
    }
}

pub(crate) fn theme(id: &str) -> Theme {
    match id {
        "light" => Theme::Light,
        "dark" => Theme::Dark,
        _ => Theme::System,
    }
}

/// A saved backdrop id as one Settings offers: the paintings are themselves, "plain" (or "none")
/// is None, anything else (an old Tauri backdrop, a typo) is the default painting.
pub(crate) fn backdrop(id: &str) -> &'static str {
    super::backdrop::BACKDROPS.iter().map(|b| b.0).find(|b| *b == id).unwrap_or(if id.eq_ignore_ascii_case("none") { "plain" } else { "flow" })
}

/// The look in a prefs file's text, or the defaults; never fails.
pub(crate) fn from_json(text: Option<&str>) -> Look {
    let v = text.and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok());
    let field = |k: &str| v.as_ref().and_then(|v| v.get(k)).and_then(|x| x.as_str()).map(str::to_string);
    Look {
        theme: field("theme").map_or(Theme::System, |t| theme(&t)),
        backdrop: field("backdrop").map_or("flow", |b| backdrop(&b)),
    }
}

/// The look saved at `path`, or in core's prefs file.
pub(crate) fn load(path: Option<&Path>) -> Look {
    let path = path.map(Path::to_path_buf).or_else(backspace_core::prefs::Prefs::path);
    from_json(path.and_then(|p| std::fs::read_to_string(p).ok()).as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_or_unreadable_file_gives_the_defaults() {
        assert_eq!(from_json(None), Look::default());
        assert_eq!(from_json(Some("not json {")), Look::default());
        assert_eq!(load(Some(Path::new("/nonexistent/cauldron/prefs.json"))), Look::default());
        assert_eq!(Look::default().backdrop, "flow");
        assert_eq!(Look::default().motion(), Motion::Moving, "Background is Moving (plays, then rests) by default");
    }

    #[test]
    fn old_tauri_values_map_only_where_they_map_cleanly() {
        assert_eq!(backdrop("plain"), "plain", "Plain is None");
        assert_eq!(backdrop("none"), "plain");
        assert_eq!(backdrop("harbour"), "harbour");
        for gone in ["aurora", "sharks-white", "sharks-black", "koi", "", "Harbour!"] {
            assert_eq!(backdrop(gone), "flow", "{gone:?} falls back to the default, Flow");
        }
        assert_eq!(theme("dark"), Theme::Dark);
        assert_eq!(theme("auto"), Theme::System, "an unknown theme follows the OS");
    }

    /// A prefs file core cannot parse (the retired "tabs" placement as a string, an old Tauri file
    /// with its own fields) still gives its theme and painting, or the defaults.
    #[test]
    fn a_file_core_cannot_parse_keeps_its_look() {
        let retired = r#"{"tabs":"floating","tab_place":"pane","theme":"light","backdrop":"fog"}"#;
        assert_eq!(from_json(Some(retired)), Look { theme: Theme::Light, backdrop: "fog" });
        let tauri = r#"{"theme":"dark","backdrop":"sharks-white","glass":true,"vivid":false,"tabs":[{"name":"Home","panes":[]}],"active_tab":3}"#;
        assert_eq!(from_json(Some(tauri)), Look { theme: Theme::Dark, backdrop: "flow" });
        let wrong_types = r#"{"theme":7,"backdrop":null}"#;
        assert_eq!(from_json(Some(wrong_types)), Look::default());
    }

    /// A saved painting is Still with that picture; the Background cycles Moving, Still, Off.
    #[test]
    fn the_background_reads_from_the_saved_id() {
        let m = |b: &str| from_json(Some(&format!(r#"{{"backdrop":"{b}"}}"#))).motion();
        assert_eq!([m("flow"), m("flow-live"), m("flow-still"), m("harbour"), m("plain")], [Motion::Moving, Motion::Always, Motion::Still, Motion::Still, Motion::Off]);
        assert_eq!([Motion::Moving, Motion::Always, Motion::Still, Motion::Off].map(next_motion), ["flow-live", "flow-still", "plain", "flow"]);
    }

    /// Moving plays for 8s on launch, on coming back to the front and on each New stream, then
    /// rests; renders in between (typing in the composer) do not wake it. A fake clock.
    #[test]
    fn moving_plays_then_rests() {
        let t0 = std::time::Instant::now();
        let at = |s: f32| t0 + std::time::Duration::from_secs_f32(s);
        let mut b = Burst::default();
        assert!(b.playing(at(0.0), true, true), "plays on launch");
        assert!(b.playing(at(7.9), true, true), "for 8 seconds");
        assert!(!b.playing(at(8.0), true, true), "then rests");
        for k in 0..50 {
            assert!(!b.playing(at(9.0 + k as f32 * 0.1), true, true), "typing re-renders but does not wake it");
        }
        assert!(!b.playing(at(20.0), false, true), "the window leaves the front");
        assert!(b.playing(at(30.0), true, true), "and plays again when it comes back");
        assert!(!b.playing(at(38.5), true, true));
        assert!(!b.playing(at(40.0), true, false), "a stream opens");
        assert!(b.playing(at(50.0), true, true), "New stream shown again plays");
        assert!(!b.playing(at(58.1), true, true));
    }

    #[test]
    fn the_theme_cycles_system_light_dark() {
        assert_eq!([Theme::System.next(), Theme::Light.next(), Theme::Dark.next()], [Theme::Light, Theme::Dark, Theme::System]);
        for t in [Theme::System, Theme::Light, Theme::Dark] {
            assert_eq!(theme(t.id()), t);
        }
    }
}
