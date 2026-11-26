//! Cauldron stays on (Charles, 22 Nov: "always on, in the background … close it from the tray,
//! right click, close for good or open"). Closing the window hides it; the icon in the
//! notification area (Windows) or the menu bar (macOS) opens it again or quits for good.
//! Settings > General turns either part off. Linux has no tray under GPUI yet, so there closing
//! the window quits as before.

use gpui_kit::{AnyWindowHandle, App, AsyncApp, Window};

/// True where this build has a tray icon to come back from.
pub const HAS_TRAY: bool = cfg!(any(windows, target_os = "macos"));

/// Brings the hidden window back: shown, raised and focused.
pub fn show(window: &mut Window, cx: &mut App) {
    native::show(window);
    cx.activate(true);
    window.activate_window();
}

/// The window's close button: hide instead of closing while "Keep running" is on.
/// Returns whether the window may close.
pub fn on_close(window: &mut Window, cx: &mut App) -> bool {
    if !HAS_TRAY || !backspace_core::prefs::Prefs::load().keep_running {
        return true;
    }
    native::hide(window, cx);
    false
}

/// Called once the window is open: matches the login entry to the pref and puts up the tray icon.
pub fn install(window: AnyWindowHandle, cx: &mut App) {
    let p = backspace_core::prefs::Prefs::load();
    if let Err(e) = backspace_core::autostart::sync(p.start_at_login) {
        eprintln!("start at login: {e}");
    }
    tray::install(window, cx);
}

/// A second start of this channel asked the running copy to show itself (instance.rs). Windows
/// also raises the window directly; elsewhere this is the only way it comes forward.
pub fn come_forward_on_ask(window: AnyWindowHandle, instance: std::sync::Arc<backspace_core::instance::Instance>, cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| loop {
        cx.background_executor().timer(std::time::Duration::from_millis(400)).await;
        if instance.woken() {
            handle(Ask::Open, window, cx);
        }
    })
    .detach();
}

/// What the tray asked for.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Ask {
    Open,
    Quit,
}

/// Acts on what the tray asked.
pub fn handle(ask: Ask, window: AnyWindowHandle, cx: &mut AsyncApp) {
    match ask {
        Ask::Open => {
            let _ = window.update(cx, |_, window, cx| show(window, cx));
        }
        Ask::Quit => {
            let _ = cx.update(|cx| cx.quit());
        }
    }
}

#[cfg(any(windows, target_os = "macos"))]
mod tray {
    use super::Ask;
    use gpui_kit::{AnyWindowHandle, App, AsyncApp};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    const OPEN: &str = "cauldron-open";
    const QUIT: &str = "cauldron-quit";

    /// The app's purple disc, drawn at 32 px (the sidebar's mark), so no image file ships for it.
    fn icon() -> Option<Icon> {
        const S: u32 = 32;
        let mut rgba = Vec::with_capacity((S * S * 4) as usize);
        let (r, c) = (13.0f32, (S as f32 - 1.0) / 2.0);
        for y in 0..S {
            for x in 0..S {
                let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
                let a = (r + 0.5 - d).clamp(0.0, 1.0);
                rgba.extend_from_slice(&[0x7c, 0x5c, 0xff, (a * 255.0) as u8]);
            }
        }
        Icon::from_rgba(rgba, S, S).ok()
    }

    pub(super) fn install(window: AnyWindowHandle, cx: &mut App) {
        let menu = Menu::new();
        let open = MenuItem::with_id(OPEN, "Open Cauldron", true, None);
        let quit = MenuItem::with_id(QUIT, "Quit Cauldron", true, None);
        if menu.append_items(&[&open, &PredefinedMenuItem::separator(), &quit]).is_err() {
            return;
        }
        let mut b = TrayIconBuilder::new().with_tooltip(backspace_core::instance::channel().app_name()).with_menu(Box::new(menu)).with_menu_on_left_click(false);
        if let Some(i) = icon() {
            b = b.with_icon(i);
        }
        let tray = match b.build() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("tray icon: {e}");
                return;
            }
        };
        // The tray's events arrive on channels; read them a few times a second. The icon lives
        // as long as this loop, which is as long as the app.
        cx.spawn(async move |cx: &mut AsyncApp| {
            let _keep = tray;
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(200)).await;
                let mut asks = Vec::new();
                while let Ok(e) = MenuEvent::receiver().try_recv() {
                    asks.push(if e.id == QUIT { Ask::Quit } else { Ask::Open });
                }
                while let Ok(e) = TrayIconEvent::receiver().try_recv() {
                    let open = match e {
                        TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } => true,
                        TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => true,
                        _ => false,
                    };
                    if open {
                        asks.push(Ask::Open);
                    }
                }
                for a in asks {
                    super::handle(a, window, cx);
                }
            }
        })
        .detach();
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod tray {
    pub(super) fn install(_window: gpui_kit::AnyWindowHandle, _cx: &mut gpui_kit::App) {}
}

#[cfg(windows)]
mod native {
    use gpui_kit::{App, Window};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_HIDE, SW_SHOW};

    fn hwnd(window: &Window) -> Option<windows_sys::Win32::Foundation::HWND> {
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get() as _),
            _ => None,
        }
    }

    pub(super) fn hide(window: &mut Window, _cx: &mut App) {
        if let Some(h) = hwnd(window) {
            // SAFETY: a live window handle owned by this process.
            unsafe { ShowWindow(h, SW_HIDE) };
        }
    }

    pub(super) fn show(window: &mut Window) {
        if let Some(h) = hwnd(window) {
            // SAFETY: as above.
            unsafe {
                ShowWindow(h, SW_SHOW);
                SetForegroundWindow(h);
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod native {
    use gpui_kit::{App, Window};

    /// macOS hides the whole app (Cmd+H); the menu bar icon brings it back.
    pub(super) fn hide(_window: &mut Window, cx: &mut App) {
        cx.hide();
    }

    pub(super) fn show(_window: &mut Window) {}
}

#[cfg(not(any(windows, target_os = "macos")))]
mod native {
    use gpui_kit::{App, Window};

    pub(super) fn hide(_window: &mut Window, _cx: &mut App) {}

    pub(super) fn show(_window: &mut Window) {}
}
