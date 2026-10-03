//! The workbench's 16px line icons (the same paths as the HTML replica),
//! served to `svg()` from memory under `bs/<name>.svg`. Everything else falls
//! through to gpui-kit's own assets.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

const PATHS: &[(&str, &str)] = &[
    (
        "folder",
        r##"<path d="M2 4.6A1.6 1.6 0 0 1 3.6 3h2.9l1.5 1.5h4.4A1.6 1.6 0 0 1 14 6.1v5.3a1.6 1.6 0 0 1-1.6 1.6H3.6A1.6 1.6 0 0 1 2 11.4z"/>"##,
    ),
    (
        "sparkle",
        r##"<path d="M8 1.6 9.3 5.9 13.6 7 9.3 8.3 8 12.6 6.7 8.3 2.4 7l4.3-1.1z" fill="#000" stroke="none"/><circle cx="12.6" cy="12.4" r="1.2" fill="#000" stroke="none"/>"##,
    ),
    (
        "globe",
        r##"<circle cx="8" cy="8" r="5.8"/><path d="M2.2 8h11.6M8 2.2c1.7 1.6 2.6 3.6 2.6 5.8S9.7 12.2 8 13.8M8 2.2C6.3 3.8 5.4 5.8 5.4 8s.9 4.2 2.6 5.8"/>"##,
    ),
    (
        "term",
        r##"<rect x="2" y="3" width="12" height="10" rx="1.6"/><path d="m4.8 6.4 2 1.6-2 1.6M8.4 10h2.8"/>"##,
    ),
    ("plus", r##"<path d="M8 3.2v9.6M3.2 8h9.6"/>"##),
    (
        "home",
        r##"<path d="M2.6 7.2 8 2.8l5.4 4.4v5.6a.8.8 0 0 1-.8.8h-2.8V10H6.2v3.6H3.4a.8.8 0 0 1-.8-.8z"/>"##,
    ),
    (
        "window",
        r##"<rect x="2" y="3" width="12" height="10" rx="1.6"/><path d="M2 6h12"/>"##,
    ),
    (
        "gear",
        r##"<circle cx="8" cy="8" r="2.2"/><path d="M8 1.8v1.8M8 12.4v1.8M1.8 8h1.8M12.4 8h1.8M3.6 3.6l1.3 1.3M11.1 11.1l1.3 1.3M3.6 12.4l1.3-1.3M11.1 4.9l1.3-1.3"/>"##,
    ),
    (
        "sidebar",
        r##"<rect x="2" y="3" width="12" height="10" rx="1.6"/><path d="M6.2 3v10"/>"##,
    ),
    (
        "expand",
        r##"<path d="M9.6 2.6h3.8v3.8M13.4 2.6 9.2 6.8M6.4 13.4H2.6V9.6M2.6 13.4l4.2-4.2"/>"##,
    ),
    (
        "close",
        r##"<path d="m4.4 4.4 7.2 7.2M11.6 4.4l-7.2 7.2"/>"##,
    ),
    (
        "ext",
        r##"<path d="M9.2 2.6h4.2v4.2M13.4 2.6 7.6 8.4M11.6 9.6v3a.8.8 0 0 1-.8.8H3.4a.8.8 0 0 1-.8-.8V5.2a.8.8 0 0 1 .8-.8h3"/>"##,
    ),
    ("back", r##"<path d="M10 3.4 5.4 8l4.6 4.6"/>"##),
    ("fwd", r##"<path d="M6 3.4 10.6 8 6 12.6"/>"##),
    (
        "reload",
        r##"<path d="M13 8a5 5 0 1 1-1.5-3.6M13 2.8v2.6h-2.6"/>"##,
    ),
    ("chev", r##"<path d="m6 3.6 4 4.4-4 4.4"/>"##),
    ("chevd", r##"<path d="m3.6 6 4.4 4 4.4-4"/>"##),
    (
        "ticket",
        r##"<path d="M2.4 4.4h11.2v2.2a1.4 1.4 0 0 0 0 2.8v2.2H2.4V9.4a1.4 1.4 0 0 0 0-2.8z"/>"##,
    ),
    (
        "diagram",
        r##"<rect x="1.8" y="2.6" width="4.6" height="3.4" rx=".8"/><rect x="9.6" y="2.6" width="4.6" height="3.4" rx=".8"/><rect x="5.7" y="10" width="4.6" height="3.4" rx=".8"/><path d="M4.1 6v1.8h7.8V6M8 7.8V10"/>"##,
    ),
    (
        "doc",
        r##"<path d="M4 2.4h5.2L12 5.2v8.4H4z"/><path d="M9 2.4v3h3M6 8.4h4M6 10.8h4"/>"##,
    ),
    (
        "branch",
        r##"<circle cx="4.6" cy="3.6" r="1.4"/><circle cx="4.6" cy="12.4" r="1.4"/><circle cx="11.4" cy="5.6" r="1.4"/><path d="M4.6 5v6M11.4 7c0 2.6-6.8 1.8-6.8 4"/>"##,
    ),
    (
        "sun",
        r##"<circle cx="8" cy="8" r="2.8"/><path d="M8 1.6v1.6M8 12.8v1.6M1.6 8h1.6M12.8 8h1.6M3.5 3.5l1.1 1.1M11.4 11.4l1.1 1.1M3.5 12.5l1.1-1.1M11.4 4.6l1.1-1.1"/>"##,
    ),
    (
        "moon",
        r##"<path d="M12.8 10.2A5.4 5.4 0 0 1 5.8 3.2a5.4 5.4 0 1 0 7 7z"/>"##,
    ),
    (
        "auto",
        r##"<circle cx="8" cy="8" r="5.6"/><path d="M8 2.4v11.2a5.6 5.6 0 0 0 0-11.2z" fill="#000"/>"##,
    ),
    (
        "bksp",
        r##"<path d="M5.2 3.2h7.6a1 1 0 0 1 1 1v7.6a1 1 0 0 1-1 1H5.2L1.8 8z"/><path d="m7.6 6 4 4M11.6 6l-4 4"/>"##,
    ),
    (
        "pc",
        r##"<rect x="2.4" y="3" width="11.2" height="7.6" rx="1.2"/><path d="M1.4 13h13.2"/>"##,
    ),
    (
        "cloud",
        r##"<path d="M4.6 12.4h7a2.8 2.8 0 0 0 .4-5.57A4 4 0 0 0 4.3 6.2a3.1 3.1 0 0 0 .3 6.2z"/>"##,
    ),
    (
        "pin",
        r##"<path d="M6 2.6h4M6.6 2.6v4L4.6 9h6.8l-2-2.4v-4M8 9v4.4"/>"##,
    ),
    (
        "copy",
        r##"<rect x="5.4" y="5.4" width="8" height="8" rx="1.4"/><path d="M10.6 5.4V3.4a.8.8 0 0 0-.8-.8H3.4a.8.8 0 0 0-.8.8v6.4a.8.8 0 0 0 .8.8h2"/>"##,
    ),
    (
        "down",
        r##"<path d="M8 2.6v8M4.6 7.4 8 10.8l3.4-3.4M3 13.4h10"/>"##,
    ),
    ("check", r##"<path d="m3.4 8.4 3 3 6.2-6.6"/>"##),
];

pub fn path(name: &str) -> SharedString {
    format!("bs/{name}.svg").into()
}

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(name) = path
            .strip_prefix("bs/")
            .and_then(|p| p.strip_suffix(".svg"))
        {
            let Some((_, body)) = PATHS.iter().find(|(n, _)| *n == name) else {
                return Ok(None);
            };
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"##
            );
            return Ok(Some(Cow::Owned(svg.into_bytes())));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}
