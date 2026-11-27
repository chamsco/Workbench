//! Start Cauldron when the user signs in (Charles, 22 Nov: "always on"). Windows: the per-user
//! Run key. macOS: a LaunchAgent. Linux: an XDG autostart entry. Each points at the running exe,
//! so an installed copy registers itself; debug builds never register (a dev build from `target/`
//! must not start at every login).

use anyhow::Result;

#[allow(dead_code)]
/// The login entry's name, per channel (instance.rs): a nightly never repoints or removes the
/// live app's entry.
fn name() -> &'static str {
    crate::instance::channel().app_name()
}

/// The entry's id on macOS and Linux; live keeps the one every earlier build wrote.
fn id() -> &'static str {
    match crate::instance::channel() {
        crate::instance::Channel::Live => "cauldron",
        crate::instance::Channel::Nightly => "cauldron-nightly",
    }
}

/// Makes the login entry match `on`. Cheap and idempotent: called at every start.
pub fn sync(on: bool) -> Result<()> {
    if cfg!(debug_assertions) {
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    if on {
        enable(&exe)
    } else {
        disable()
    }
}

#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(windows)]
fn reg(args: &[&str]) -> Result<()> {
    use std::os::windows::process::CommandExt;
    // No console window flashing at startup.
    let out = std::process::Command::new("reg").args(args).creation_flags(0x0800_0000).output()?;
    anyhow::ensure!(out.status.success(), "reg: {}", String::from_utf8_lossy(&out.stderr).trim());
    Ok(())
}

#[cfg(windows)]
fn enable(exe: &std::path::Path) -> Result<()> {
    let cmd = format!("\"{}\"", exe.display());
    reg(&["add", RUN_KEY, "/v", name(), "/t", "REG_SZ", "/d", &cmd, "/f"])
}

#[cfg(windows)]
fn disable() -> Result<()> {
    // Already gone is fine.
    let _ = reg(&["delete", RUN_KEY, "/v", name(), "/f"]);
    Ok(())
}

#[cfg(target_os = "macos")]
fn plist() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(format!("Library/LaunchAgents/app.{}.login.plist", id())))
}

#[cfg(target_os = "macos")]
fn enable(exe: &std::path::Path) -> Result<()> {
    let p = plist().ok_or_else(|| anyhow::anyhow!("no home folder"))?;
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>app.{}.login</string><key>ProgramArguments</key><array><string>{}</string></array><key>RunAtLoad</key><true/></dict></plist>\n",
        id(),
        xml(&exe.display().to_string())
    );
    if std::fs::read_to_string(&p).ok().as_deref() == Some(body.as_str()) {
        return Ok(());
    }
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(p, body)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(target_os = "macos")]
fn disable() -> Result<()> {
    if let Some(p) = plist() {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn desktop() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    Some(base.join(format!("autostart/{}.desktop", id())))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn enable(exe: &std::path::Path) -> Result<()> {
    let p = desktop().ok_or_else(|| anyhow::anyhow!("no home folder"))?;
    let body = format!("[Desktop Entry]\nType=Application\nName={}\nExec=\"{}\"\nX-GNOME-Autostart-enabled=true\n", name(), exe.display());
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(p, body)?;
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn disable() -> Result<()> {
    if let Some(p) = desktop() {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}
