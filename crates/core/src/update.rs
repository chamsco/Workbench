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
/// The rolling pre-release the macOS workflow replaces on every push to main.
pub const ROLLING_TAG: &str = "mac-latest";
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
}

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
    Ok(UpdateInfo {
        current: VERSION.into(),
        latest: target.as_deref().map(short).unwrap_or_default().to_string(),
        newer: target.as_deref().is_some_and(|t| is_newer(t, BUILD)),
        url,
        build: short(BUILD).to_string(),
    })
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

/// Nothing to offer, because no rolling build is published at all.
fn idle() -> UpdateInfo {
    UpdateInfo {
        current: VERSION.into(),
        latest: String::new(),
        newer: false,
        url: releases_url(),
        build: short(BUILD).to_string(),
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
}
