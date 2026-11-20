//! One Cauldron per user session on Windows (Charles, 21 Nov: opening it from the Start menu while
//! it ran gave a second window). The first instance holds a named mutex; a later one brings the
//! running window forward and exits. After a self-update the new exe is started before the old
//! one quits, so it waits a few seconds for the mutex instead (`RESTART_ENV`).

/// Set on the process the updater starts.
pub const RESTART_ENV: &str = "CAULDRON_RESTARTED";

/// True when this process may run the app; false when another instance owns it (that window
/// has been brought forward). Always true off Windows.
pub fn claim(window_title: &str) -> bool {
    #[cfg(windows)]
    {
        win::claim(window_title)
    }
    #[cfg(not(windows))]
    {
        let _ = window_title;
        true
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub(super) fn claim(title: &str) -> bool {
        let name = wide("Local\\CauldronApp.Instance");
        let restarted = std::env::var_os(super::RESTART_ENV).is_some();
        let tries = if restarted { 50 } else { 1 };
        for _ in 0..tries {
            // SAFETY: a valid NUL-terminated name; the handle is kept for the process's life.
            let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
            if h.is_null() {
                return true;
            }
            if unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
                return true;
            }
            unsafe { CloseHandle(h) };
            if restarted {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
        if restarted {
            // The old one never let go: run anyway rather than leave the user with nothing.
            return true;
        }
        let t = wide(title);
        // SAFETY: FindWindowW with a NUL-terminated title; the calls tolerate a stale handle.
        unsafe {
            let w = FindWindowW(std::ptr::null(), t.as_ptr());
            if !w.is_null() {
                if IsIconic(w) != 0 {
                    ShowWindow(w, SW_RESTORE);
                }
                SetForegroundWindow(w);
            }
        }
        false
    }
}
