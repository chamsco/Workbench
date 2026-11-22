//! The in-app browser (Charles, 22 Nov: a real browser, not a text preview): WebView2 on Windows
//! and WKWebView on macOS, through wry, as a native child view laid over the Browser panel's box.
//! Linux has none: wry's WebKitGTK needs a GTK main loop, which GPUI does not run, so the panel
//! says so and offers the system browser.
//!
//! A native view draws above everything GPUI paints, so it is hidden while a menu is open.

use gpui_kit::{Bounds, Pixels, Window};

pub(crate) const AVAILABLE: bool = cfg!(any(windows, target_os = "macos"));

#[derive(Default)]
pub(crate) struct Browser {
    #[cfg(any(windows, target_os = "macos"))]
    view: Option<(wry::WebView, String)>,
    /// Why the view could not be made, shown in the panel.
    pub error: Option<String>,
}

impl Browser {
    /// Shows `url` over `bounds` (window coordinates), or hides the view when `want` is None.
    pub(crate) fn sync(&mut self, want: Option<(&str, Bounds<Pixels>)>, window: &Window) {
        #[cfg(any(windows, target_os = "macos"))]
        {
            use wry::dpi::{LogicalPosition, LogicalSize};
            let Some((url, b)) = want else {
                if let Some((v, _)) = &self.view {
                    let _ = v.set_visible(false);
                }
                return;
            };
            let rect = wry::Rect {
                position: LogicalPosition::new(f64::from(f32::from(b.origin.x)), f64::from(f32::from(b.origin.y))).into(),
                size: LogicalSize::new(f64::from(f32::from(b.size.width)), f64::from(f32::from(b.size.height))).into(),
            };
            match &mut self.view {
                Some((v, at)) => {
                    if at != url {
                        let _ = v.load_url(url);
                        *at = url.to_string();
                    }
                    let _ = v.set_bounds(rect);
                    let _ = v.set_visible(true);
                }
                None => match wry::WebViewBuilder::new().with_url(url).with_bounds(rect).build_as_child(window) {
                    Ok(v) => {
                        self.view = Some((v, url.to_string()));
                        self.error = None;
                    }
                    Err(e) => self.error = Some(e.to_string()),
                },
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = (want, window);
    }
}
