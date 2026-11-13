//! What the window looks like, read from the prefs on every frame: the theme and the painting.
//! Read on its own, field by field, so a prefs file core cannot parse (an old Tauri one, a retired
//! field, a value of the wrong type) still gives these two their saved values or sane defaults, and
//! never a half state. Old Tauri values carry over only where they map cleanly: "plain" is None,
//! the Tauri paintings are themselves; the ASCII sea, the koi and the aurora are gone, so they fall
//! back to the default, the Flow held still (Moving is a choice: it costs about 10% of a core).
//!
//! The background's motion lives in the same saved id, so core's prefs need no new field: "flow" is
//! Moving, "flow-still" or a painting is Still (that picture), "plain" is Off.

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
    Moving,
    Still,
    Off,
}

impl Motion {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Motion::Moving => "Moving",
            Motion::Still => "Still",
            Motion::Off => "Off",
        }
    }
}

impl Look {
    pub(crate) fn motion(&self) -> Motion {
        match self.backdrop {
            "flow" => Motion::Moving,
            "plain" => Motion::Off,
            _ => Motion::Still,
        }
    }
}

/// The saved id for the next Background choice: Moving, Still (Flow held still), Off.
pub(crate) fn next_motion(m: Motion) -> &'static str {
    match m {
        Motion::Moving => "flow-still",
        Motion::Still => "plain",
        Motion::Off => "flow",
    }
}

impl Default for Look {
    fn default() -> Look {
        Look { theme: Theme::System, backdrop: "flow-still" }
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
    super::backdrop::BACKDROPS.iter().map(|b| b.0).find(|b| *b == id).unwrap_or(if id.eq_ignore_ascii_case("none") { "plain" } else { "flow-still" })
}

/// The look in a prefs file's text, or the defaults; never fails.
pub(crate) fn from_json(text: Option<&str>) -> Look {
    let v = text.and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok());
    let field = |k: &str| v.as_ref().and_then(|v| v.get(k)).and_then(|x| x.as_str()).map(str::to_string);
    Look {
        theme: field("theme").map_or(Theme::System, |t| theme(&t)),
        backdrop: field("backdrop").map_or("flow-still", |b| backdrop(&b)),
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
        assert_eq!(Look::default().backdrop, "flow-still");
        assert_eq!(Look::default().motion(), Motion::Still, "Background is Still by default: moving costs more than the board allowed");
    }

    #[test]
    fn old_tauri_values_map_only_where_they_map_cleanly() {
        assert_eq!(backdrop("plain"), "plain", "Plain is None");
        assert_eq!(backdrop("none"), "plain");
        assert_eq!(backdrop("harbour"), "harbour");
        for gone in ["aurora", "sharks-white", "sharks-black", "koi", "", "Harbour!"] {
            assert_eq!(backdrop(gone), "flow-still", "{gone:?} falls back to the default, Flow held still");
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
        assert_eq!(from_json(Some(tauri)), Look { theme: Theme::Dark, backdrop: "flow-still" });
        let wrong_types = r#"{"theme":7,"backdrop":null}"#;
        assert_eq!(from_json(Some(wrong_types)), Look::default());
    }

    /// A saved painting is Still with that picture; the Background cycles Moving, Still, Off.
    #[test]
    fn the_background_reads_from_the_saved_id() {
        let m = |b: &str| from_json(Some(&format!(r#"{{"backdrop":"{b}"}}"#))).motion();
        assert_eq!([m("flow"), m("flow-still"), m("harbour"), m("plain")], [Motion::Moving, Motion::Still, Motion::Still, Motion::Off]);
        assert_eq!([next_motion(Motion::Moving), next_motion(Motion::Still), next_motion(Motion::Off)], ["flow-still", "plain", "flow"]);
    }

    #[test]
    fn the_theme_cycles_system_light_dark() {
        assert_eq!([Theme::System.next(), Theme::Light.next(), Theme::Dark.next()], [Theme::Light, Theme::Dark, Theme::System]);
        for t in [Theme::System, Theme::Light, Theme::Dark] {
            assert_eq!(theme(t.id()), t);
        }
    }
}
