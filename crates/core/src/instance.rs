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
        // A copy built before the lock existed holds nothing to find, so look for it by its exe.
        // Skipped after a self-update (the old copy is still quitting) and with a custom data
        // folder (a dev run beside the installed app).
        Ok(Some(_)) if !restarted && std::env::var_os("BACKSPACE_DATA").is_none() && legacy_copy_running() => {
            tell_user(&format!("{} is already running as an older copy that cannot be asked to come forward. Quit it, then start again.", channel().app_name()));
            None
        }
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

/// True when another process is running this same executable as the app. Only asked once the lock
/// is ours, so any such process is a build from before the lock (a newer one would hold it). A
/// replaced exe (the app dragged over the old one) still counts: Linux marks its path "(deleted)".
/// The same exe also runs as a helper under an agent CLI (`mcp`, `msg`, `tile`), which can outlive
/// the app after a crash; those are not a second copy and must never block a start.
pub fn legacy_copy_running() -> bool {
    let Ok(me) = std::env::current_exe() else { return false };
    other_process_runs(&me, &running_exes(), std::process::id())
}

/// `procs`: (pid, exe path as the OS reports it, first argument if any).
fn other_process_runs(me: &Path, procs: &[(u32, String, Option<String>)], my_pid: u32) -> bool {
    let me = me.to_string_lossy();
    let me = me.trim_end_matches(" (deleted)");
    procs.iter().any(|(pid, exe, arg)| {
        *pid != my_pid
            && exe.trim_end_matches(" (deleted)") == me
            && !matches!(arg.as_deref(), Some("mcp" | "msg" | "tile"))
    })
}

#[cfg(target_os = "linux")]
fn running_exes() -> Vec<(u32, String, Option<String>)> {
    let Ok(rd) = std::fs::read_dir("/proc") else { return Vec::new() };
    rd.flatten()
        .filter_map(|e| {
            let pid: u32 = e.file_name().to_str()?.parse().ok()?;
            let exe = std::fs::read_link(e.path().join("exe")).ok()?;
            let cmd = std::fs::read(e.path().join("cmdline")).unwrap_or_default();
            let arg = cmd.split(|b| *b == 0).nth(1).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned());
            Some((pid, exe.to_string_lossy().into_owned(), arg))
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn running_exes() -> Vec<(u32, String, Option<String>)> {
    // `args` is the full command line, path first; the path may hold spaces, so the caller's own
    // path is matched as a prefix.
    let me = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let Ok(out) = std::process::Command::new("ps").args(["-axo", "pid=,args="]).output() else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (pid, args) = l.trim_start().split_once(' ')?;
            let args = args.trim();
            match args.strip_prefix(me.as_str()) {
                Some(rest) if rest.is_empty() || rest.starts_with(' ') => {
                    Some((pid.parse().ok()?, me.clone(), rest.split_whitespace().next().map(str::to_string)))
                }
                _ => Some((pid.parse().ok()?, args.to_string(), None)),
            }
        })
        .collect()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn running_exes() -> Vec<(u32, String, Option<String>)> {
    Vec::new() // Windows already sees an old copy through the mutex it always held.
}

/// A GUI launch has no terminal, so the refusal goes to the log and, where the OS has a plain way,
/// to a dialog or notification.
fn tell_user(msg: &str) {
    eprintln!("{msg}");
    crate::crash::note(msg);
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("osascript")
        .args(["-e", &format!("display alert \"{}\"", msg.replace('"', "'"))])
        .status();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("notify-send").args([channel().app_name(), msg]).status();
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

    /// A copy from before the lock holds nothing to find, so it is seen by its executable: the same
    /// exe in another process (even replaced on disk) means a second copy; this process alone does not.
    #[test]
    fn a_pre_lock_copy_is_found_by_its_exe() {
        let me = Path::new("/Applications/Cauldron.app/Contents/MacOS/cauldron");
        let exe = me.to_string_lossy().into_owned();
        let procs = vec![(10, exe.clone(), None), (11, "/usr/bin/other".into(), None)];
        assert!(!other_process_runs(me, &procs, 10), "only ourselves");
        assert!(other_process_runs(me, &procs, 99), "an older copy runs");
        let replaced = vec![(10, format!("{exe} (deleted)"), None)];
        assert!(other_process_runs(me, &replaced, 99), "replaced on disk, still running");
    }

    /// An agent CLI's `mcp` helper is this same exe and can outlive the app after a crash. It is not a
    /// second copy: counting it left the next launch with no window at all.
    #[test]
    fn a_leftover_helper_does_not_block_the_next_start() {
        let me = Path::new("/opt/cauldron/cauldron");
        let exe = me.to_string_lossy().into_owned();
        for sub in ["mcp", "msg", "tile"] {
            let helper = vec![(10, exe.clone(), Some(sub.to_string()))];
            assert!(!other_process_runs(me, &helper, 99), "{sub} is a helper");
        }
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
