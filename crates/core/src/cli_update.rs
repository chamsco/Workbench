//! Updating the coding CLIs from the sidebar's Update button (board round 6, pick U2). The CLIs
//! that install from npm (`CliStatus::install` reads `npm i -g <package>`) are compared with the
//! registry's latest and updated with `npm i -g <package>@latest`. The others (a shell script, a
//! web page) update themselves or by hand, and the row says so. Every call starts a process: call
//! off the UI thread.

use anyhow::{bail, Result};

/// The npm package in an install line like `npm i -g @openai/codex`.
pub fn npm_package(install: &str) -> Option<String> {
    let mut it = install.split_whitespace();
    (it.next()? == "npm").then_some(())?;
    let mut global = false;
    let mut pkg = None;
    for w in it {
        match w {
            "i" | "install" | "add" => {}
            "-g" | "--global" => global = true,
            w if !w.starts_with('-') => pkg = Some(w.to_string()),
            _ => {}
        }
    }
    if global {
        pkg
    } else {
        None
    }
}

/// Is `latest` a newer dotted version than `installed`? Numbers compare as numbers; anything
/// that does not parse is not newer.
pub fn newer(installed: &str, latest: &str) -> bool {
    let nums = |s: &str| -> Option<Vec<u64>> { s.trim().trim_start_matches('v').split('-').next()?.split('.').map(|p| p.parse().ok()).collect() };
    match (nums(installed), nums(latest)) {
        (Some(a), Some(b)) => b > a,
        _ => false,
    }
}

fn npm() -> std::process::Command {
    backspace_runner::cmd(if cfg!(windows) { "npm.cmd" } else { "npm" })
}

/// The registry's latest version of `pkg`, or None when npm is missing or offline.
pub fn latest_blocking(pkg: &str) -> Option<String> {
    let out = npm().args(["view", pkg, "version"]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|v| !v.is_empty())
}

/// `npm i -g pkg@latest`; the error is npm's own last line.
pub fn update_blocking(pkg: &str) -> Result<()> {
    let out = npm().args(["i", "-g", &format!("{pkg}@latest")]).output()?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("npm failed");
    bail!("{}", last.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_package_is_read_from_an_npm_install_line() {
        assert_eq!(npm_package("npm i -g @openai/codex").as_deref(), Some("@openai/codex"));
        assert_eq!(npm_package("npm install --global opencode-ai").as_deref(), Some("opencode-ai"));
        assert_eq!(npm_package("curl https://cursor.com/install -fsS | bash"), None);
        assert_eq!(npm_package("https://x.ai/cli"), None);
        assert_eq!(npm_package("npm i left-pad"), None, "a local install is not a CLI");
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert!(newer("1.9.0", "1.10.0"));
        assert!(newer("2.0.0", "2.0.1"));
        assert!(!newer("2.0.1", "2.0.1"));
        assert!(!newer("2.1.0", "2.0.9"));
        assert!(newer("v1.0.0", "1.0.1-beta.2"));
        assert!(!newer("weird", "1.0.0"));
    }
}
