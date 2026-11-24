//! Crash reports (Charles, 24 Nov: "report a crash and automatically create an issue with what
//! happened, the log, etc."). Nothing leaves the machine by itself: a crash is written to a file,
//! and on the next start the app offers to open a pre-filled GitHub issue that the user reads and
//! submits. The app carries no GitHub token, so it cannot file the issue on its own.
//!
//! Three things are kept under `<data dir>/crash/`:
//! - `cauldron.log`: a short running log. On a Windows release build there is no console, so
//!   everything the app writes to stderr goes here too; before this, a panic there left no trace.
//! - `crash-<time>.txt`: one per panic (any thread) or, on Windows, per fatal native exception,
//!   with the version, commit, OS, what happened, a backtrace and the end of the log.
//! - `reported/`: crash files the user has reported or dismissed, kept for reference.

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// Where crash files and the log live.
pub fn dir() -> PathBuf {
    crate::prefs::Prefs::data_dir().join("crash")
}

/// The running log.
pub fn log_path() -> PathBuf {
    dir().join("cauldron.log")
}

/// The log is cut over to `cauldron.1.log` past this size, at start.
const LOG_MAX: u64 = 1024 * 1024;
/// How many log lines a crash file carries.
const LOG_TAIL: usize = 80;
/// GitHub's new-issue page takes the body in the URL; past about 8 KB browsers and GitHub refuse it.
const URL_MAX: usize = 7500;

/// Sets up the log and the crash hooks. Call once, early in `main`, after the single-instance
/// check (so a second start that exits at once never touches the log).
pub fn install() {
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    let log = log_path();
    if std::fs::metadata(&log).map(|m| m.len() > LOG_MAX).unwrap_or(false) {
        let _ = std::fs::rename(&log, d.join("cauldron.1.log"));
    }
    #[cfg(windows)]
    win::stderr_to_log(&log);
    let restarted = std::env::var_os(crate::instance::RESTART_ENV).is_some();
    note(&format!("start {} ({}) on {}{}", crate::update::VERSION, build(), os(), if restarted { ", after an update" } else { "" }));
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let what = match (info.payload().downcast_ref::<&str>(), info.payload().downcast_ref::<String>()) {
            (Some(s), _) => s.to_string(),
            (_, Some(s)) => s.clone(),
            _ => "(no message)".to_string(),
        };
        let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let bt = std::backtrace::Backtrace::force_capture();
        record(&format!("panic in thread '{thread}' at {at}"), &what, &bt.to_string());
        previous(info);
    }));
    #[cfg(windows)]
    win::catch_native();
}

/// One line in the log, with the time.
pub fn note(line: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path()) {
        let _ = writeln!(f, "[{}] {line}", stamp(now()));
    }
}

/// Writes a crash file. `kind` is a one-line summary, `what` the message, `detail` the backtrace.
fn record(kind: &str, what: &str, detail: &str) {
    note(&format!("CRASH: {kind}: {what}"));
    let t = now();
    let body = format!(
        "kind: {kind}\nwhat: {what}\nversion: {} ({})\nos: {}\ntime: {} UTC\n\n--- backtrace ---\n{detail}\n\n--- last {LOG_TAIL} log lines ---\n{}\n",
        crate::update::VERSION,
        build(),
        os(),
        stamp(t),
        tail(&log_path(), LOG_TAIL)
    );
    let _ = std::fs::create_dir_all(dir());
    let _ = std::fs::write(dir().join(format!("crash-{t}.txt")), body);
}

/// A crash waiting to be reported: the file, and what the issue says.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub path: PathBuf,
    pub title: String,
    pub text: String,
}

/// The newest crash file not yet reported or dismissed.
pub fn pending() -> Option<Report> {
    pending_in(&dir())
}

fn pending_in(d: &Path) -> Option<Report> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(d)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("crash-") && n.ends_with(".txt")))
        .collect();
    files.sort();
    let path = files.pop()?;
    let text = std::fs::read_to_string(&path).ok()?;
    let what = text.lines().find_map(|l| l.strip_prefix("what: ")).unwrap_or("crash");
    let mut title = format!("Crash: {}", what.lines().next().unwrap_or(what));
    if title.chars().count() > 100 {
        title = title.chars().take(99).collect::<String>() + "…";
    }
    Some(Report { path, title, text })
}

/// Moves a crash file to `reported/` so it is not offered again.
pub fn set_aside(r: &Report) {
    let Some(d) = r.path.parent() else { return };
    let to = d.join("reported");
    let _ = std::fs::create_dir_all(&to);
    if let Some(name) = r.path.file_name() {
        if std::fs::rename(&r.path, to.join(name)).is_err() {
            let _ = std::fs::remove_file(&r.path);
        }
    }
}

/// The GitHub new-issue page for this report, filled in. A report too long for a URL keeps its
/// head and the end of its log, and says where the whole file is.
pub fn issue_url(r: &Report) -> String {
    let base = format!("https://github.com/{}/issues/new?labels=crash&title={}&body=", crate::update::REPO, enc(&r.title));
    let full = body(&r.text, &r.path, false);
    if base.len() + enc(&full).len() <= URL_MAX {
        return base + &enc(&full);
    }
    // Too long: drop lines from the middle of the report (the deep backtrace frames first) until it fits.
    let lines: Vec<&str> = r.text.lines().collect();
    let mut keep_head = lines.len().min(40);
    let mut keep_tail = lines.len().saturating_sub(keep_head).min(40);
    loop {
        let cut = lines.len() - keep_head - keep_tail;
        let mut t = lines[..keep_head].join("\n");
        if cut > 0 {
            t.push_str(&format!("\n… {cut} lines cut …\n"));
        }
        t.push_str(&lines[lines.len() - keep_tail..].join("\n"));
        let b = body(&t, &r.path, cut > 0);
        if base.len() + enc(&b).len() <= URL_MAX || (keep_head == 0 && keep_tail == 0) {
            return base + &enc(&b);
        }
        if keep_head > 12 {
            keep_head -= 4;
        } else if keep_tail > 0 {
            keep_tail -= 4.min(keep_tail);
        } else {
            keep_head = keep_head.saturating_sub(4);
        }
    }
}

fn body(report: &str, path: &Path, cut: bool) -> String {
    let more = if cut { format!("\nThe whole report is in `{}`; attach it if you can.\n", path.display()) } else { String::new() };
    format!("Cauldron crashed. Filed from the app's crash prompt.\n\n**What I was doing:** \n\n```\n{report}\n```\n{more}")
}

/// Percent-encodes everything but unreserved characters.
fn enc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// The last `n` lines of a file.
fn tail(p: &Path, n: usize) -> String {
    let Ok(s) = std::fs::read(p) else { return "(no log)".into() };
    let s = String::from_utf8_lossy(&s);
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

fn build() -> &'static str {
    if crate::update::BUILD.is_empty() {
        "built by hand"
    } else {
        crate::update::BUILD
    }
}

fn os() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// `2026-11-24 08:30:00` from seconds since 1970, UTC.
fn stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem / 60 % 60, rem % 60)
}

#[cfg(windows)]
mod win {
    use std::path::Path;
    use windows_sys::Win32::System::Console::{GetStdHandle, SetStdHandle, STD_ERROR_HANDLE};
    use windows_sys::Win32::System::Diagnostics::Debug::{SetUnhandledExceptionFilter, EXCEPTION_POINTERS};

    /// A release build has no console, so stderr goes nowhere; send it to the log instead.
    pub(super) fn stderr_to_log(log: &Path) {
        use std::os::windows::io::IntoRawHandle;
        // SAFETY: GetStdHandle only reads the process's handle table.
        let h = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
        if !h.is_null() && h as isize != -1 {
            return; // started from a terminal: keep writing there
        }
        if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
            // The handle stays open for the life of the process.
            // SAFETY: a valid file handle we own and never close.
            unsafe { SetStdHandle(STD_ERROR_HANDLE, f.into_raw_handle() as _) };
        }
    }

    /// Records access violations and other fatal native exceptions, which a panic hook never sees.
    pub(super) fn catch_native() {
        unsafe extern "system" fn filter(info: *const EXCEPTION_POINTERS) -> i32 {
            // SAFETY: Windows passes a valid pointer for the exception being raised.
            let (code, addr) = unsafe {
                info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()).map(|r| (r.ExceptionCode as u32, r.ExceptionAddress as usize)).unwrap_or((0, 0))
            };
            let bt = std::backtrace::Backtrace::force_capture();
            super::record("native exception", &format!("exception 0x{code:08X} at 0x{addr:X}"), &bt.to_string());
            0 // EXCEPTION_CONTINUE_SEARCH: Windows ends the process as it would have
        }
        // SAFETY: installs a process-wide filter; `filter` matches the expected signature.
        unsafe { SetUnhandledExceptionFilter(Some(filter)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crash with a huge backtrace must still open GitHub: the URL stays under what browsers take.
    #[test]
    fn a_long_report_still_fits_in_the_issue_url() {
        let mut text = String::from("kind: panic\nwhat: boom\n");
        for i in 0..3000 {
            text.push_str(&format!("  {i}: some::deep::frame::with::a::long::name at src/x.rs:{i}\n"));
        }
        let r = Report { path: PathBuf::from("crash-1.txt"), title: "Crash: boom".into(), text };
        let url = issue_url(&r);
        assert!(url.len() <= URL_MAX, "{}", url.len());
        assert!(url.contains("boom"), "the head of the report survives the cut");
    }

    /// A reported crash is not offered again; the newest one is offered first.
    #[test]
    fn the_newest_unreported_crash_is_offered_once() {
        let d = std::env::temp_dir().join(format!("cauldron-crash-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("crash-100.txt"), "what: old\n").unwrap();
        std::fs::write(d.join("crash-200.txt"), "what: new\n").unwrap();
        std::fs::write(d.join("cauldron.log"), "x\n").unwrap();
        let r = pending_in(&d).unwrap();
        assert_eq!(r.title, "Crash: new");
        set_aside(&r);
        assert_eq!(pending_in(&d).unwrap().title, "Crash: old");
        set_aside(&pending_in(&d).unwrap());
        assert_eq!(pending_in(&d), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn stamps_read_as_utc_dates() {
        assert_eq!(stamp(0), "1970-01-01 00:00:00");
        assert_eq!(stamp(1_795_509_000), "2026-11-24 08:30:00");
    }
}
