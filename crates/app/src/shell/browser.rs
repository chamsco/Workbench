//! The in-app browser (Charles, 22 Nov: a real browser, not a text preview): WebView2 on Windows
//! and WKWebView on macOS, through wry, as a native child view laid over the Browser panel's box.
//! Linux has none: wry's WebKitGTK needs a GTK main loop, which GPUI does not run, so the panel
//! says so and offers the system browser.
//!
//! A native view draws above everything GPUI paints, so it is hidden while a menu is open.

use backspace_core::prefs::Prefs;
use gpui_kit::{Bounds, Pixels, Window};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Slot {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Slot {
    fn from_bounds(b: Bounds<Pixels>) -> Self {
        Self {
            x: f32::from(b.origin.x),
            y: f32::from(b.origin.y),
            w: f32::from(b.size.width),
            h: f32::from(b.size.height),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Request {
    url: String,
    slot: Slot,
}

#[derive(Default)]
struct LoadState {
    started: Option<Instant>,
    finished: bool,
}

pub(crate) const AVAILABLE: bool = cfg!(any(windows, target_os = "macos"));

pub(crate) struct Browser {
    #[cfg(any(windows, target_os = "macos"))]
    view: Option<(wry::WebView, String)>,
    #[cfg(any(windows, target_os = "macos"))]
    context: wry::WebContext,
    wanted: Option<Request>,
    synced: Option<Request>,
    queued: bool,
    failed_url: Option<String>,
    load: Rc<RefCell<LoadState>>,
    /// Why the view could not be made, shown in the panel.
    pub error: Option<String>,
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            #[cfg(any(windows, target_os = "macos"))]
            view: None,
            #[cfg(any(windows, target_os = "macos"))]
            context: wry::WebContext::new(Some(Prefs::data_dir().join("browser"))),
            wanted: None,
            synced: None,
            queued: false,
            failed_url: None,
            load: Rc::new(RefCell::new(LoadState::default())),
            error: None,
        }
    }
}

impl Browser {
    /// Records the native view's desired state. Native calls happen after the render pass.
    pub(crate) fn request(&mut self, want: Option<(&str, Bounds<Pixels>)>) {
        let wanted = want.map(|(url, bounds)| Request {
            url: url.to_string(),
            slot: Slot::from_bounds(bounds),
        });
        let changed = self.wanted.as_ref().map(|r| &r.url) != wanted.as_ref().map(|r| &r.url);
        self.wanted = wanted;
        if changed {
            let mut load = self.load.borrow_mut();
            load.started = self.wanted.as_ref().map(|_| Instant::now());
            load.finished = false;
        }
        if self.wanted.is_none() {
            self.failed_url = None;
            self.error = None;
        }
    }

    pub(crate) fn needs_sync(&self) -> bool {
        !self.queued
            && self.wanted != self.synced
            && self
                .wanted
                .as_ref()
                .is_none_or(|request| self.failed_url.as_deref() != Some(request.url.as_str()))
    }

    pub(crate) fn queue_sync(&mut self) {
        self.queued = true;
    }

    /// Returns the visible loading-line width and whether another frame is needed.
    pub(crate) fn load_progress(&self) -> Option<(f32, bool)> {
        let mut load = self.load.borrow_mut();
        if load.finished {
            load.finished = false;
            load.started = None;
            return Some((1.0, false));
        }
        let start = load.started?;
        Some(((start.elapsed().as_secs_f32() / 1.2).min(0.92), true))
    }

    pub(crate) fn go_back(&mut self) {
        #[cfg(any(windows, target_os = "macos"))]
        if let Some((view, _)) = &self.view {
            if let Err(e) = view.go_back() {
                self.error = Some(e.to_string());
            }
        }
    }

    pub(crate) fn go_forward(&mut self) {
        #[cfg(any(windows, target_os = "macos"))]
        if let Some((view, _)) = &self.view {
            if let Err(e) = view.go_forward() {
                self.error = Some(e.to_string());
            }
        }
    }

    pub(crate) fn reload(&mut self) {
        #[cfg(any(windows, target_os = "macos"))]
        if let Some((view, _)) = &self.view {
            if let Err(e) = view.reload() {
                self.error = Some(e.to_string());
            }
        }
    }

    /// Shows the requested URL over its bounds, or hides the view when no URL is requested.
    /// This is called after GPUI has finished the frame that laid out the browser slot.
    pub(crate) fn sync(&mut self, window: &Window) {
        self.queued = false;
        let Some(request) = self.wanted.clone() else {
            #[cfg(any(windows, target_os = "macos"))]
            if let Some((view, _)) = &self.view {
                let _ = view.set_visible(false);
            }
            self.synced = None;
            return;
        };

        #[cfg(any(windows, target_os = "macos"))]
        {
            use wry::dpi::{LogicalPosition, LogicalSize};
            let rect = wry::Rect {
                position: LogicalPosition::new(
                    f64::from(request.slot.x),
                    f64::from(request.slot.y),
                )
                .into(),
                size: LogicalSize::new(f64::from(request.slot.w), f64::from(request.slot.h)).into(),
            };
            let load = Rc::clone(&self.load);
            match &mut self.view {
                Some((v, at)) => {
                    if at != &request.url {
                        if let Err(e) = v.load_url(&request.url) {
                            self.error = Some(e.to_string());
                        }
                        *at = request.url.clone();
                    }
                    let _ = v.set_bounds(rect);
                    let _ = v.set_visible(true);
                }
                None => match wry::WebViewBuilder::new_with_web_context(&mut self.context)
                    .with_url(&request.url)
                    .with_bounds(rect)
                    .with_on_page_load_handler(move |event, _| {
                        let mut load = load.borrow_mut();
                        match event {
                            wry::PageLoadEvent::Started => {
                                load.started = Some(Instant::now());
                                load.finished = false;
                            }
                            wry::PageLoadEvent::Finished => load.finished = true,
                        }
                    })
                    .build_as_child(window)
                {
                    Ok(v) => {
                        self.view = Some((v, request.url.clone()));
                        if let Some((v, _)) = &self.view {
                            let _ = v.set_visible(true);
                        }
                        self.error = None;
                    }
                    Err(e) => {
                        self.failed_url = Some(request.url.clone());
                        self.error = Some(e.to_string());
                    }
                },
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = window;
        self.synced = Some(request);
    }
}

#[cfg(test)]
mod tests {
    use super::{Browser, Slot};
    use gpui_kit::{point, px, size, Bounds};

    fn slot() -> Bounds<gpui_kit::Pixels> {
        Bounds::new(point(px(10.), px(20.)), size(px(300.), px(200.)))
    }

    #[test]
    fn native_sync_is_queued_once_for_a_rendered_request() {
        let mut browser = Browser::default();
        browser.request(Some(("https://example.com", slot())));
        assert!(browser.needs_sync());
        browser.queue_sync();
        assert!(!browser.needs_sync());
    }

    #[test]
    fn changing_the_url_or_slot_requires_another_sync() {
        let mut browser = Browser::default();
        browser.request(Some(("https://example.com", slot())));
        browser.synced = browser.wanted.clone();
        assert!(!browser.needs_sync());

        browser.request(Some(("https://example.org", slot())));
        assert!(browser.needs_sync());
        browser.synced = browser.wanted.clone();
        browser.request(Some((
            "https://example.org",
            Bounds::new(point(px(11.), px(20.)), size(px(300.), px(200.))),
        )));
        assert!(browser.needs_sync());
    }

    #[test]
    fn closing_the_browser_clears_the_failed_url() {
        let mut browser = Browser::default();
        browser.failed_url = Some("https://example.com".into());
        browser.error = Some("failed".into());
        browser.request(None);
        assert!(browser.failed_url.is_none());
        assert!(browser.error.is_none());
    }

    #[test]
    fn slot_keeps_window_coordinates() {
        let s = Slot::from_bounds(slot());
        assert_eq!(
            s,
            Slot {
                x: 10.,
                y: 20.,
                w: 300.,
                h: 200.
            }
        );
    }
}
