//! Installing an update from inside the app.
//!
//! `backspace_core::update` answers "is main ahead of me?". This acts on the
//! answer: it downloads the same `Backspace-mac.app.zip` a person would, unpacks
//! it with `ditto`, and hands a small shell script the job of waiting for this
//! process to exit before it swaps the bundle in place of the running one and
//! opens it again. The app is replaced only after the download has been checked
//! to be a Backspace, so a half-finished download can never leave the machine
//! without an app.
//!
//! macOS only: the release's asset is a `.app` inside a zip, not an installer
//! package, so there is nothing to run and no pkg receipt to keep.

use anyhow::{anyhow, Result};
#[cfg(target_os = "macos")]
use anyhow::Context;
use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri::Emitter;

#[cfg(target_os = "macos")]
#[derive(Clone, serde::Serialize)]
struct Progress {
    stage: String,
    pct: u8,
}

#[cfg(target_os = "macos")]
fn say(app: &AppHandle, stage: &str, pct: u8) {
    let _ = app.emit(
        "update:progress",
        Progress {
            stage: stage.to_string(),
            pct,
        },
    );
}

#[cfg(not(target_os = "macos"))]
pub async fn download(_app: AppHandle) -> Result<()> {
    Err(anyhow!("Backspace installs its own updates on macOS only"))
}

#[cfg(not(target_os = "macos"))]
pub async fn install(_app: AppHandle) -> Result<()> {
    Err(anyhow!("Backspace installs its own updates on macOS only"))
}

/// A build that is downloaded, unpacked and checked, waiting for a restart.
#[cfg(target_os = "macos")]
struct Staged {
    new_app: std::path::PathBuf,
    tmp: std::path::PathBuf,
}

#[cfg(target_os = "macos")]
static STAGED: std::sync::Mutex<Option<Staged>> = std::sync::Mutex::new(None);

/// Fetch and unpack the newest build without touching the running app.
///
/// This is what makes the T3-style flow possible: the download can finish
/// while the person is still working, the footer can say "Update ready", and
/// the restart is a second, deliberate click. Nothing is swapped until
/// `install` is called.
#[cfg(target_os = "macos")]
pub async fn download(app: AppHandle) -> Result<()> {
    let staged = stage(&app).await?;
    *STAGED.lock().unwrap() = Some(staged);
    say(&app, "Ready", 96);
    Ok(())
}

/// Swap the staged build in and quit into it.
///
/// If nothing was staged -- the download never happened, or this is a call
/// from an older UI -- it stages first, so the command still does what its
/// name says. On success this process exits and never returns.
#[cfg(target_os = "macos")]
pub async fn install(app: AppHandle) -> Result<()> {
    // Take in its own statement: a temporary in a match scrutinee lives for
    // the whole match, and the guard would then be held across the await,
    // making this future non-Send.
    let waiting = STAGED.lock().unwrap().take();
    let staged = match waiting {
        Some(staged) => staged,
        None => stage(&app).await?,
    };
    apply(staged, &app)
}

/// Download, unpack, and prove the result is Backspace. Everything here is
/// reversible: it only writes to the temporary folder.
#[cfg(target_os = "macos")]
async fn stage(app: &AppHandle) -> Result<Staged> {
    use std::process::Command;

    let http = reqwest::Client::new();
    say(app, "Preparing", 2);
    let (url, size) = backspace_core::update::app_zip(&http).await?;

    let bundle = running_bundle()?;
    ensure_writable(&bundle)?;

    let tmp = std::env::temp_dir().join(format!("backspace-update-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).context("couldn't make a place to download into")?;
    let zip = tmp.join("Backspace-mac.app.zip");
    fetch(&http, &url, size, &zip, app).await?;

    say(app, "Unpacking", 82);
    let staged = tmp.join("app");
    std::fs::create_dir_all(&staged)?;
    run(Command::new("/usr/bin/ditto")
        .arg("-x")
        .arg("-k")
        .arg(&zip)
        .arg(&staged))
    .context("couldn't unpack the update")?;
    let new_app = find_app(&staged)?;
    check_identity(&new_app)?;

    Ok(Staged { new_app, tmp })
}

/// Hand the swap to a shell that waits for this process to exit.
#[cfg(target_os = "macos")]
fn apply(staged: Staged, app: &AppHandle) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    let Staged { new_app, tmp } = staged;
    let bundle = running_bundle()?;

    say(app, "Installing", 94);
    let script = tmp.join("swap.sh");
    std::fs::write(
        &script,
        r#"#!/bin/bash
# Runs after Backspace has quit. $1 pid, $2 the app to replace, $3 the new
# one, $4 the temporary folder. Puts the old bundle back if the copy fails.
pid=$1; target=$2; new=$3; tmp=$4
while kill -0 "$pid" 2>/dev/null; do sleep 0.2; done
sleep 0.4
old="$target.old.$$"
rm -rf "$old"
# Whatever happens below, the app comes back: a failed swap must not leave the
# person with no window and no way in but Finder.
if ! mv "$target" "$old"; then
  /usr/bin/open "$target" 2>/dev/null
  exit 1
fi
if ! /usr/bin/ditto "$new" "$target"; then
  rm -rf "$target"
  mv "$old" "$target"
  /usr/bin/open "$target" 2>/dev/null
  exit 1
fi
rm -rf "$old"
/usr/bin/xattr -dr com.apple.quarantine "$target" 2>/dev/null
/usr/bin/open "$target"
rm -rf "$tmp"
"#,
    )?;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;

    Command::new("/bin/bash")
        .arg(&script)
        .arg(std::process::id().to_string())
        .arg(&bundle)
        .arg(&new_app)
        .arg(&tmp)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("couldn't start the installer")?;

    say(app, "Restarting", 100);
    // The script is waiting for exactly this. No cleanup here: the script owns
    // the temporary folder and the old bundle from this point.
    app.exit(0);
    Ok(())
}


/// The `.app` this process is running out of, if it is running out of one.
#[cfg(target_os = "macos")]
fn running_bundle() -> Result<std::path::PathBuf> {
    let exe = std::env::current_exe().context("couldn't find the running app")?;
    exe.canonicalize()
        .unwrap_or(exe)
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| {
            anyhow!("this build isn't running from a .app, so there is nothing to replace")
        })
}

/// Can the app be replaced where it sits? `/Applications` is writable by an
/// admin, a mounted dmg is not, and a copy in a read-only place should say so
/// before it downloads 25 MB.
#[cfg(target_os = "macos")]
fn ensure_writable(bundle: &std::path::Path) -> Result<()> {
    let parent = bundle
        .parent()
        .ok_or_else(|| anyhow!("couldn't find where Backspace lives"))?;
    let probe = parent.join(format!(".backspace-update-{}", std::process::id()));
    std::fs::write(&probe, b"")
        .map_err(|_| anyhow!("Backspace can't write to {}", parent.display()))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

#[cfg(target_os = "macos")]
async fn fetch(
    http: &reqwest::Client,
    url: &str,
    size: u64,
    dest: &std::path::Path,
    app: &AppHandle,
) -> Result<()> {
    use std::io::Write;

    let mut res = http
        .get(url)
        .header("user-agent", "backspace")
        .send()
        .await
        .map_err(|e| anyhow!("couldn't reach GitHub ({})", e.without_url()))?;
    if !res.status().is_success() {
        return Err(anyhow!("GitHub answered {}", res.status()));
    }
    let total = res.content_length().filter(|n| *n > 0).unwrap_or(size);
    let mut file = std::fs::File::create(dest).context("couldn't write the download")?;
    let mut done: u64 = 0;
    let mut shown = 0u8;
    while let Some(chunk) = res.chunk().await? {
        file.write_all(&chunk)?;
        done += chunk.len() as u64;
        // 4% is "asking GitHub", 78% is "have the file"; the rest is unpacking.
        let pct = match total {
            0 => 40,
            n => (4 + (done.saturating_mul(74) / n)).min(78) as u8,
        };
        if pct > shown {
            shown = pct;
            say(app, "Downloading", pct);
        }
    }
    file.flush()?;
    if total > 0 && done != total {
        return Err(anyhow!(
            "the download stopped early ({} of {} bytes)",
            done,
            total
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn find_app(dir: &std::path::Path) -> Result<std::path::PathBuf> {
    let mut found = std::fs::read_dir(dir)
        .context("couldn't read the unpacked update")?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "app"));
    let app = found.next().ok_or_else(|| anyhow!("the update has no app in it"))?;
    Ok(app)
}

/// Is what we downloaded actually Backspace? The bundle identifier is the one
/// check that costs nothing and catches a wrong or truncated archive.
#[cfg(target_os = "macos")]
fn check_identity(app: &std::path::Path) -> Result<()> {
    use std::process::Command;

    let plist = app.join("Contents/Info.plist");
    if !plist.exists() {
        return Err(anyhow!("the update has no Info.plist in it"));
    }
    let out = Command::new("/usr/bin/plutil")
        .arg("-extract")
        .arg("CFBundleIdentifier")
        .arg("raw")
        .arg("-o")
        .arg("-")
        .arg(&plist)
        .output()
        .context("couldn't read the update's identity")?;
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || id != "computer.backspace.app" {
        return Err(anyhow!("the update isn't Backspace ({})", id));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn run(cmd: &mut std::process::Command) -> Result<()> {
    let out = cmd.output()?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("{}", why.trim()));
    }
    Ok(())
}
