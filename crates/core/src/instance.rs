//! One Cauldron per channel (Charles, 21 Nov: opening it from the Start menu while it ran gave a
//! second window; 26 Nov: two copies of the live app must never run together, while a nightly may
//! run beside the live one).
//!
//! What two copies would actually break is the data folder: every thread, pref and familiar is a
//! file there, each copy keeps its own copy in memory and writes it back whole, so the last writer
//! silently undoes the other. So the lock is the data folder's: the first process to open a folder
//! holds `instance.lock` in it for its life, and a later one asks it to come forward and exits. A
//! channel (`live`, `nightly`) gets its own folder, which is what lets the two run side by side.
//!
//! The OS drops the lock when the holder dies, so a crash never leaves the app unable to start.
//! After a self-update the new exe is started before the old one quits; it waits a few seconds for
//! the lock instead (`RESTART_ENV`).

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Set on the process the updater starts.
pub const RESTART_ENV: &str = "CAULDRON_RESTARTED";

// `pub const CHANNEL: &str`: which release line this build belongs to. A workflow that builds
// another channel rewrites the file before building, as it does `build_stamp.rs`.
include!("channel_stamp.rs");

/// The channels this build knows. Anything else in the stamp reads as live, so a typo can never
/// give a build a fresh, empty data folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Live,
    Nightly,
}

impl Channel {
    pub fn of(stamp: &str) -> Channel {
        match stamp.trim() {
            "nightly" => Channel::Nightly,
            _ => Channel::Live,
        }
    }

    /// The window title, the tray tooltip and what Windows finds a running copy by.
    pub fn app_name(self) -> &'static str {
        match self {
            Channel::Live => "Cauldron",
            Channel::Nightly => "Cauldron Nightly",
        }
    }

    /// The data folder's name under the platform's base. Live keeps the folder every build so far
    /// has used.
    pub fn data_folder(self, macos: bool) -> &'static str {
        match (self, macos) {
            (Channel::Live, true) => "Backspace",
            (Channel::Live, false) => "backspace",
            (Channel::Nightly, true) => "Backspace Nightly",
            (Channel::Nightly, false) => "backspace-nightly",
        }
    }
}

/// This build's channel.
pub fn channel() -> Channel {
    Channel::of(CHANNEL)
}

/// The running copy's hold on its data folder. Keep it for the life of the process.
#[derive(Debug)]
pub struct Instance {
    _lock: Option<File>,
    wake: PathBuf,
    #[cfg(windows)]
    _mutex: win::Mutex,
}

impl Instance {
    /// True once each time a second start asked this copy to come forward.
    pub fn woken(&self) -> bool {
        std::fs::remove_file(&self.wake).is_ok()
    }
}

/// Takes the lock on this build's data folder. None when another copy holds it: that copy has been
/// asked to come forward, and this process should exit.
pub fn claim() -> Option<Instance> {
    let restarted = std::env::var_os(RESTART_ENV).is_some();
    let dir = crate::prefs::Prefs::data_dir();
    #[cfg(windows)]
    let mutex = match win::Mutex::take(channel(), restarted) {
        Some(m) => m,
        None => {
            win::bring_forward(channel().app_name());
            return None;
        }
    };
    let got = claim_in(&dir, if restarted { 50 } else { 1 }, Duration::from_millis(200));
    match got {
        Ok(Some(i)) => {
            #[cfg(windows)]
            let i = Instance { _mutex: mutex, ..i };
            Some(i)
        }
        // Someone else has this folder: ask them to show themselves.
        Ok(None) => {
            #[cfg(windows)]
            win::bring_forward(channel().app_name());
            None
        }
        // The folder can't be locked at all (read-only, a file system without locks): run anyway,
        // as every build before this one did, rather than leave the user with nothing.
        Err(_) => Some(unlocked(&dir)),
    }
}

/// Tries `tries` times, `gap` apart, to lock `dir`. Ok(None): another holder has it, and has been
/// left a note to come forward.
pub fn claim_in(dir: &Path, tries: u32, gap: Duration) -> std::io::Result<Option<Instance>> {
    std::fs::create_dir_all(dir)?;
    let wake = dir.join("instance.wake");
    for n in 0..tries.max(1) {
        let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("instance.lock"))?;
        match f.try_lock() {
            Ok(()) => {
                // A note left for a copy that has since gone is not for us.
                let _ = std::fs::remove_file(&wake);
                return Ok(Some(Instance {
                    _lock: Some(f),
                    wake,
                    #[cfg(windows)]
                    _mutex: win::Mutex::none(),
                }));
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                if n + 1 < tries.max(1) {
                    std::thread::sleep(gap);
                }
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(e),
        }
    }
    let _ = std::fs::write(&wake, b"");
    Ok(None)
}

fn unlocked(dir: &Path) -> Instance {
    Instance {
        _lock: None,
        wake: dir.join("instance.wake"),
        #[cfg(windows)]
        _mutex: win::Mutex::none(),
    }
}

#[cfg(windows)]
mod win {
    use super::Channel;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The named mutex builds before 26 Nov used. The live channel keeps its name, so a new build
    /// still sees an old one running (they share a data folder but the old one has no file lock).
    #[derive(Debug)]
    pub(super) struct Mutex(HANDLE);

    // SAFETY: a kernel handle, only closed on drop.
    unsafe impl Send for Mutex {}
    unsafe impl Sync for Mutex {}

    impl Mutex {
        pub(super) fn none() -> Mutex {
            Mutex(std::ptr::null_mut())
        }

        pub(super) fn take(ch: Channel, restarted: bool) -> Option<Mutex> {
            let name = wide(match ch {
                Channel::Live => "Local\\CauldronApp.Instance",
                Channel::Nightly => "Local\\CauldronApp.Instance.nightly",
            });
            let tries = if restarted { 50 } else { 1 };
            for _ in 0..tries {
                // SAFETY: a valid NUL-terminated name.
                let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
                if h.is_null() {
                    return Some(Mutex::none());
                }
                if unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
                    return Some(Mutex(h));
                }
                unsafe { CloseHandle(h) };
                if restarted {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
            // After an update the old one never let go: run anyway rather than leave nothing.
            restarted.then(Mutex::none)
        }
    }

    impl Drop for Mutex {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: our own handle, closed once.
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    pub(super) fn bring_forward(title: &str) {
        let t = wide(title);
        // SAFETY: FindWindowW with a NUL-terminated title; the calls tolerate a stale handle.
        unsafe {
            let w = FindWindowW(std::ptr::null(), t.as_ptr());
            if !w.is_null() {
                if IsIconic(w) != 0 {
                    ShowWindow(w, SW_RESTORE);
                } else if IsWindowVisible(w) == 0 {
                    // Hidden in the tray (background.rs): starting it again opens it.
                    ShowWindow(w, SW_SHOW);
                }
                SetForegroundWindow(w);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cauldron-instance-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// The requirement: a second copy on the same data folder never runs beside the first, asks the
    /// first to come forward, and can start once the first has gone.
    #[test]
    fn a_second_copy_of_one_channel_is_refused_until_the_first_quits() {
        let dir = tmp("same");
        let first = claim_in(&dir, 1, Duration::ZERO).unwrap().expect("the first copy runs");
        assert!(claim_in(&dir, 1, Duration::ZERO).unwrap().is_none(), "a second copy must not run");
        assert!(first.woken(), "the running copy is asked to come forward");
        assert!(!first.woken(), "once per ask");
        drop(first);
        assert!(claim_in(&dir, 1, Duration::ZERO).unwrap().is_some(), "free again once the first quits");
    }

    /// Nightly beside live is allowed, which only holds if the two never share a folder (and so a lock).
    #[test]
    fn nightly_and_live_hold_different_folders() {
        for mac in [true, false] {
            assert_ne!(Channel::Live.data_folder(mac), Channel::Nightly.data_folder(mac));
        }
        let base = tmp("channels");
        let live = claim_in(&base.join(Channel::Live.data_folder(false)), 1, Duration::ZERO).unwrap();
        let nightly = claim_in(&base.join(Channel::Nightly.data_folder(false)), 1, Duration::ZERO).unwrap();
        assert!(live.is_some() && nightly.is_some());
    }

    /// A stamp the app doesn't know must not move anyone's data to a new, empty folder.
    #[test]
    fn an_unknown_channel_stamp_reads_as_live() {
        assert_eq!(Channel::of("nightly"), Channel::Nightly);
        assert_eq!(Channel::of("stable"), Channel::Live);
        assert_eq!(Channel::of(""), Channel::Live);
    }
}
