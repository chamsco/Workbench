//! "Is there a newer Backspace?" against the GitHub releases of the repo.

use anyhow::Result;
use serde::Serialize;

pub const REPO: &str = "chamsco/ohMyHarness";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Clone, Debug)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub newer: bool,
    /// Where "Download & Update" sends you.
    pub url: String,
}

pub async fn check(http: &reqwest::Client) -> Result<UpdateInfo> {
    let res = http
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases/latest"
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
    // No release published yet: nothing newer than this build.
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateInfo {
            current: VERSION.into(),
            latest: VERSION.into(),
            newer: false,
            url: format!("https://github.com/{REPO}/releases"),
        });
    }
    let v: serde_json::Value = res.error_for_status()?.json().await?;
    let tag = v["tag_name"]
        .as_str()
        .unwrap_or(VERSION)
        .trim_start_matches('v');
    Ok(UpdateInfo {
        current: VERSION.into(),
        latest: tag.into(),
        newer: newer(tag, VERSION),
        url: v["html_url"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| format!("https://github.com/{REPO}/releases")),
    })
}

/// Dotted numeric comparison; anything unparsable counts as not newer.
pub fn newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Option<Vec<u64>> {
        s.split(['.', '-'])
            .take(3)
            .map(|p| p.parse().ok())
            .collect()
    };
    match (parse(latest), parse(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn compares_versions() {
        assert!(super::newer("0.2.0", "0.1.9"));
        assert!(super::newer("1.0.0", "0.9.9"));
        assert!(!super::newer("0.1.0", "0.1.0"));
        assert!(!super::newer("garbage", "0.1.0"));
    }
}
