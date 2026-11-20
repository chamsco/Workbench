//! "Is there a newer Backspace?" against this repo's releases.
//!
//! The macOS workflow publishes one pre-release, `mac-latest`, and replaces it
//! on every push to main. Its tag never moves and its version never changes, so
//! there is no number to compare: what changes is the commit it was built from.
//! The workflow writes that commit into `build_stamp.rs` before building, this
//! binary carries it, and this asks the release which commit *it* was built from.
//! A different commit means main moved and there is a newer build to install.
//!
//! A versioned release, if the project ever publishes one, still takes the same
//! path: its `target_commitish` is the commit, so the comparison is unchanged.
//!
//! A build made by hand has no commit recorded, so it is never offered an update.

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

pub const REPO: &str = "chamsco/Workbench";
/// The rolling pre-release this platform's workflow replaces on every push to main:
/// `windows-latest` (Cauldron.exe) on Windows, `mac-latest` elsewhere.
pub const ROLLING_TAG: &str = if cfg!(windows) { "windows-latest" } else { "mac-latest" };
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
// The commit this binary was built from, or "" for a build made by hand. CI
// writes it before building, into the source the compiler reads: see
// `build_stamp.rs` for why it is not carried alongside the build instead.
include!("build_stamp.rs");

/// The commit, kept in the file byte for byte.
///
/// Nothing here reads it, and that is the problem. A release build folds what
/// the app says about itself: `is_newer` compares seven bytes, `build` copies
/// seven bytes, and seven bytes fit in a register, so the optimizer turns the
/// commit into immediate instructions and drops the text. The workflow then
/// cannot ask the finished app which commit it was built from -- `strings` on
/// the bundle finds nothing -- and neither can anyone debugging a release. One
/// static keeps the whole commit in the file, so "the app knows its commit" is
/// something the build can prove rather than assume.
///
/// `#[used]` is what stops the linker dropping it along with the rest.
#[used]
pub static STAMP_IN_THE_BINARY: &str = BUILD;

#[derive(Serialize, Clone, Debug)]
pub struct UpdateInfo {
    /// The version of the build you are running, e.g. "0.1.0".
    pub current: String,
    /// The commit the newest rolling build came from, short, e.g. "841a8f5".
    pub latest: String,
    pub newer: bool,
    /// Where "Download & Update" sends you.
    pub url: String,
    /// The commit this build came from, short; "" when built by hand.
    pub build: String,
    /// Can this build download the update and swap itself? Windows only: the
    /// release carries one `Cauldron.exe`, which [`install_exe`] puts in place
    /// of the running one. Elsewhere the app opens the release page.
    pub installs: bool,
    /// What changed between this build and the newest one, newest first, at
    /// most `CHANGES_SHOWN` of them. Empty when nothing is newer, or when
    /// GitHub would not say.
    pub changes: Vec<Change>,
    /// How many more changes there are than `changes` shows.
    pub more: usize,
    /// The whole comparison on GitHub, for "N more changes" and "Read more".
    pub changes_url: String,
}

/// One line of "What's changed": a pull request when the commit names one.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Change {
    pub title: String,
    /// The GitHub login of whoever wrote it, when GitHub knows it.
    pub author: String,
    /// The pull request it came in by, or 0 for a commit pushed straight.
    pub pr: u64,
}

/// How many changes the update popover lists before "N more on GitHub".
pub const CHANGES_SHOWN: usize = 8;

pub async fn check(http: &reqwest::Client) -> Result<UpdateInfo> {
    let res = http
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases/tags/{ROLLING_TAG}"
        ))
        .header("user-agent", "backspace")
        .header("accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "couldn't reach GitHub ({})",
                if e.is_timeout() {
                    "timed out".to_string()
                } else if e.is_connect() {
                    "no connection".to_string()
                } else {
                    e.without_url().to_string()
                }
            )
        })?;
    // No rolling build published yet: nothing newer than this build.
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(idle());
    }
    let v: Value = res.error_for_status()?.json().await?;
    let url = v["html_url"]
        .as_str()
        .map(String::from)
        .unwrap_or_else(releases_url);
    let target = release_commit(&v);
    let newer = target.as_deref().is_some_and(|t| is_newer(t, BUILD));
    let mut info = UpdateInfo {
        latest: target.as_deref().map(short).unwrap_or_default().to_string(),
        newer,
        url,
        ..idle()
    };
    if let (true, Some(target)) = (newer, target) {
        info.changes_url = compare_url(BUILD, &target);
        // Best effort: an update with no list is still an update.
        if let Ok(all) = changes(http, BUILD, &target).await {
            info.more = all.len().saturating_sub(CHANGES_SHOWN);
            info.changes = all.into_iter().take(CHANGES_SHOWN).collect();
        }
    }
    Ok(info)
}

/// The changes between two commits, newest first: the merged pull requests
/// when main takes them, every commit when it does not.
async fn changes(http: &reqwest::Client, from: &str, to: &str) -> Result<Vec<Change>> {
    let v: Value = http
        .get(format!(
            "https://api.github.com/repos/{REPO}/compare/{from}...{to}"
        ))
        .header("user-agent", "backspace")
        .header("accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(changes_in(&v))
}

fn changes_in(v: &Value) -> Vec<Change> {
    let Some(commits) = v["commits"].as_array() else {
        return Vec::new();
    };
    let mut out: Vec<Change> = commits
        .iter()
        .filter_map(|c| {
            let author = c["author"]["login"].as_str().unwrap_or_default();
            change(c["commit"]["message"].as_str()?, author)
        })
        .collect();
    // A merged pull request brings its own commits in with its merge, and the
    // merge's title says it all. When main takes pull requests, the pull
    // requests are the changelog; the rest is their working history.
    if out.iter().any(|c| c.pr != 0) {
        out.retain(|c| c.pr != 0);
    }
    out.reverse();
    out
}

/// One commit message as a line of the changelog, or `None` for a merge that
/// says nothing (`Merge branch 'main' into ...`).
fn change(message: &str, author: &str) -> Option<Change> {
    let mut lines = message.lines().map(str::trim).filter(|l| !l.is_empty());
    let head = lines.next()?;
    if let Some(rest) = head.strip_prefix("Merge pull request #") {
        // "Merge pull request #18 from owner/branch", then the PR's title.
        let pr = rest.split_whitespace().next()?.parse().ok()?;
        let title = lines.next().unwrap_or(head).to_string();
        let author = rest
            .split(" from ")
            .nth(1)
            .and_then(|b| b.split('/').next())
            .unwrap_or(author);
        return Some(Change {
            title,
            author: author.to_string(),
            pr,
        });
    }
    if head.starts_with("Merge ") {
        return None;
    }
    // A squash merge: "Title (#18)".
    let (title, pr) = match head.strip_suffix(')').and_then(|h| h.rsplit_once(" (#")) {
        Some((t, n)) if n.bytes().all(|b| b.is_ascii_digit()) => (t, n.parse().unwrap_or(0)),
        _ => (head, 0),
    };
    Some(Change {
        title: title.to_string(),
        author: author.to_string(),
        pr,
    })
}

fn compare_url(from: &str, to: &str) -> String {
    format!(
        "https://github.com/{REPO}/compare/{}...{}",
        short(from),
        short(to)
    )
}

/// The zipped app in the rolling release: where it is and how big it is.
///
/// The install path downloads exactly what a person gets by hand — the same
/// `Backspace-mac.app.zip` the workflow packs with `ditto -c -k` — instead of
/// carrying a second kind of update that only the app understands.
pub async fn app_zip(http: &reqwest::Client) -> Result<(String, u64)> {
    let res = http
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases/tags/{ROLLING_TAG}"
        ))
        .header("user-agent", "backspace")
        .header("accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("couldn't reach GitHub ({})", e.without_url()))?;
    let v: Value = res.error_for_status()?.json().await?;
    let none = || anyhow::anyhow!("that release has no app to install");
    let asset = v["assets"]
        .as_array()
        .and_then(|list| {
            list.iter().find(|a| {
                a["name"]
                    .as_str()
                    .is_some_and(|n| n.ends_with(".app.zip"))
            })
        })
        .ok_or_else(none)?;
    let url = asset["browser_download_url"].as_str().unwrap_or_default();
    if url.is_empty() {
        return Err(anyhow::anyhow!("that release has no app to install"));
    }
    Ok((url.to_string(), asset["size"].as_u64().unwrap_or(0)))
}

/// `check` from a thread that has no async runtime (the app's background executor).
pub fn check_blocking() -> Result<UpdateInfo> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    rt.block_on(async { check(&reqwest::Client::new()).await })
}

/// The Windows build in the rolling release: where `Cauldron.exe` is and how big it is.
pub async fn exe_asset(http: &reqwest::Client) -> Result<(String, u64)> {
    let v: Value = http
        .get(format!("https://api.github.com/repos/{REPO}/releases/tags/{ROLLING_TAG}"))
        .header("user-agent", "backspace")
        .header("accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("couldn't reach GitHub ({})", e.without_url()))?
        .error_for_status()?
        .json()
        .await?;
    exe_in(&v).ok_or_else(|| anyhow::anyhow!("that release has no Cauldron.exe to install"))
}

fn exe_in(release: &Value) -> Option<(String, u64)> {
    let a = release["assets"].as_array()?.iter().find(|a| a["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case("Cauldron.exe")))?;
    let url = a["browser_download_url"].as_str().filter(|u| !u.is_empty())?;
    Some((url.to_string(), a["size"].as_u64().unwrap_or(0)))
}

/// Where the running exe is set aside while its replacement takes its name.
pub fn old_exe(exe: &std::path::Path) -> std::path::PathBuf {
    exe.with_extension("old.exe")
}

/// Downloads the newest `Cauldron.exe` and puts it where the running exe is. Windows lets a
/// running exe be renamed but not overwritten, so the running one moves to `Cauldron.old.exe`
/// (deleted by [`tidy_old_exe`] on the next start) and the download takes its name. Returns the
/// path to start; the caller starts it and quits. Blocking: call it off the UI thread.
pub fn install_exe() -> Result<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let bytes = rt.block_on(async {
        let http = reqwest::Client::new();
        let (url, size) = exe_asset(&http).await?;
        let body = http
            .get(&url)
            .header("user-agent", "backspace")
            .timeout(std::time::Duration::from_secs(600))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        if size > 0 && body.len() as u64 != size {
            anyhow::bail!("the download stopped early ({} of {} bytes)", body.len(), size);
        }
        anyhow::Ok(body)
    })?;
    swap_exe(&exe, &bytes)?;
    Ok(exe)
}

/// Writes `bytes` beside `exe`, sets the running file aside and moves the new one in. On any
/// failure the old exe is put back, so the app is never left without one.
pub fn swap_exe(exe: &std::path::Path, bytes: &[u8]) -> Result<()> {
    // An exe is a PE file: "MZ" first. Anything else (an HTML error page) must not replace it.
    if bytes.len() < 2 || &bytes[..2] != b"MZ" {
        anyhow::bail!("the download is not a Windows program");
    }
    let new = exe.with_extension("new.exe");
    std::fs::write(&new, bytes)?;
    let old = old_exe(exe);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old)?;
    if let Err(e) = std::fs::rename(&new, exe) {
        let _ = std::fs::rename(&old, exe);
        return Err(e.into());
    }
    Ok(())
}

/// Deletes the exe an update set aside last time, once it is no longer running.
pub fn tidy_old_exe() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(old_exe(&exe));
    }
}

/// Nothing to offer, because no rolling build is published at all.
fn idle() -> UpdateInfo {
    UpdateInfo {
        current: VERSION.into(),
        latest: String::new(),
        newer: false,
        url: releases_url(),
        build: short(BUILD).to_string(),
        installs: cfg!(windows),
        changes: Vec::new(),
        more: 0,
        changes_url: String::new(),
    }
}

fn releases_url() -> String {
    format!("https://github.com/{REPO}/releases")
}

/// Is the release's commit a different one than ours?
///
/// SHAs do not order, so "different" is the whole test. On the rolling release
/// that is the right question: main only moves forward, so a build from another
/// commit is a newer build of main. A build with no baked commit was made by
/// hand, and is never told to update.
fn is_newer(release: &str, build: &str) -> bool {
    !build.is_empty() && short(release) != short(build)
}

/// The commit a release was built from. The workflow passes the commit as the
/// release's `--target`, which lands in `target_commitish`; if that is a branch
/// name instead, fall back to the "Built from <sha>" line the same workflow
/// writes into the notes.
fn release_commit(v: &Value) -> Option<String> {
    if let Some(commit) = v["target_commitish"].as_str().and_then(sha) {
        return Some(commit);
    }
    let notes = v["body"].as_str()?;
    let stamp = notes.split("Built from ").nth(1)?;
    sha(&stamp.chars().take_while(char::is_ascii_alphanumeric).collect::<String>())
}

/// A commit hash: seven to forty hex characters, case-folded.
fn sha(s: &str) -> Option<String> {
    let s = s.trim();
    if (7..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(s.to_ascii_lowercase())
    } else {
        None
    }
}

/// The seven characters git prints, and the app shows.
fn short(s: &str) -> &str {
    s.get(..7).unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_commit_the_workflow_passed() {
        // `gh release create ... --target "$GITHUB_SHA"` stores it whole.
        let v = json!({ "target_commitish": "841A8F548F1D6C76BDC58F2BEE5B6ACB8882AD75" });
        assert_eq!(
            super::release_commit(&v).as_deref(),
            Some("841a8f548f1d6c76bdc58f2bee5b6acb8882ad75")
        );
    }

    #[test]
    fn falls_back_to_the_notes() {
        // The target is a branch when the release was not pointed at a commit.
        let v = json!({
            "target_commitish": "main",
            "body": "Built from 841a8f5. Unsigned: right-click Backspace.app → Open.",
        });
        assert_eq!(super::release_commit(&v).as_deref(), Some("841a8f5"));
    }

    #[test]
    fn notes_without_a_stamp_say_nothing() {
        let v = json!({ "target_commitish": "main", "body": "No stamp in here." });
        assert_eq!(super::release_commit(&v), None);
    }

    #[test]
    fn another_commit_is_newer_but_the_same_one_is_not() {
        assert!(super::is_newer("841a8f5", "16ee49c"));
        assert!(super::is_newer(
            "841a8f548f1d6c76bdc58f2bee5b6acb8882ad75",
            "16ee49c6d27b3fb9515cc088e6b558049209bcc"
        ));
        // The same commit, however much of it each side knows.
        assert!(!super::is_newer("841a8f5", "841a8f548f1d6c76bdc58f2bee5b6acb8882ad75"));
        assert!(!super::is_newer("841a8f548f1d6c76bdc58f2bee5b6acb8882ad75", "841a8f5"));
        // A build made by hand has no commit, so it is never told to update.
        assert!(!super::is_newer("841a8f5", ""));
    }

    #[test]
    fn a_merged_pull_request_reads_as_its_title() {
        let c = super::change(
            "Merge pull request #18 from chamsco/frame/code-engine\n\nCode engine in the frame",
            "web-flow",
        );
        assert_eq!(
            c,
            Some(Change {
                title: "Code engine in the frame".into(),
                author: "chamsco".into(),
                pr: 18
            })
        );
        assert_eq!(super::change("Merge branch 'main' into x", "a"), None);
        let squash = super::change("fix(web): stop reloading (#231)\n\nbody", "ann").unwrap();
        assert_eq!(
            (squash.title.as_str(), squash.pr),
            ("fix(web): stop reloading", 231)
        );
        let plain = super::change("tidy (the thing)", "ann").unwrap();
        assert_eq!((plain.title.as_str(), plain.pr), ("tidy (the thing)", 0));
    }

    #[test]
    fn the_changelog_is_pull_requests_newest_first() {
        let v = json!({ "commits": [
            { "author": { "login": "ann" }, "commit": { "message": "wip" } },
            { "author": null, "commit": { "message": "Merge pull request #1 from ann/a\n\nFirst" } },
            { "author": { "login": "bo" }, "commit": { "message": "Second (#2)" } },
        ]});
        let titles: Vec<_> = super::changes_in(&v).into_iter().map(|c| c.title).collect();
        assert_eq!(titles, ["Second", "First"]);
        // No pull requests at all: every commit is a change.
        let v = json!({ "commits": [
            { "commit": { "message": "one" } }, { "commit": { "message": "two" } },
        ]});
        assert_eq!(super::changes_in(&v).len(), 2);
    }

    #[test]
    fn only_a_hash_counts_as_a_commit() {
        assert_eq!(super::sha("841a8f5"), Some("841a8f5".into()));
        assert_eq!(super::sha("841a8f"), None);
        assert_eq!(super::sha("main"), None);
        assert_eq!(super::sha(""), None);
        assert_eq!(super::short("841a8f548f1d6c76bdc58f2bee5b6acb8882ad75"), "841a8f5");
        assert_eq!(super::short("841a8f5"), "841a8f5");
        assert_eq!(super::short(""), "");
    }

    /// The fast twin of the macOS workflow's "The app knows which commit it is"
    /// step. Both workflows write the commit into `build_stamp.rs` before
    /// building, so a stamp that stops reaching the binary fails here -- in a
    /// minute, on Linux -- instead of in the app, where losing it means an
    /// update that is never offered to anyone. `GITHUB_SHA` is unset outside CI,
    /// and a build by hand has no commit to record.
    #[test]
    fn the_stamp_is_the_commit_the_build_was_given() {
        if let Ok(said) = std::env::var("GITHUB_SHA") {
            assert_eq!(BUILD, said.trim().to_ascii_lowercase().as_str());
        }
    }

    /// The Windows install path picks the release's Cauldron.exe, never another asset.
    #[test]
    fn the_windows_asset_is_cauldron_exe() {
        let v = json!({ "assets": [
            { "name": "Cauldron-mac.app.zip", "browser_download_url": "https://x/mac", "size": 9 },
            { "name": "Cauldron.exe", "browser_download_url": "https://x/exe", "size": 42 },
        ]});
        assert_eq!(super::exe_in(&v), Some(("https://x/exe".into(), 42)));
        assert_eq!(super::exe_in(&json!({ "assets": [] })), None);
    }

    /// The swap sets the running exe aside and moves the download in; a download that is not a
    /// program leaves the old exe exactly where it was.
    #[test]
    fn swapping_the_exe_keeps_the_old_one_aside_and_refuses_junk() {
        let dir = std::env::temp_dir().join(format!("cauldron-swap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("Cauldron.exe");
        std::fs::write(&exe, b"MZold").unwrap();
        assert!(super::swap_exe(&exe, b"<html>rate limited</html>").is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"MZold");
        super::swap_exe(&exe, b"MZnew").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"MZnew");
        assert_eq!(std::fs::read(super::old_exe(&exe)).unwrap(), b"MZold");
        assert_eq!(super::old_exe(&exe), dir.join("Cauldron.old.exe"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
