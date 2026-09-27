//! Git plumbing for per-agent isolation. Each ticket agent works on its own
//! branch in its own worktree; accepted work is merged into the parent's
//! branch. Every call is serialized by the caller (see `Inner::git`), since
//! worktrees share one ref store.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use tokio::process::Command;

const IDENT: [&str; 4] = [
    "-c",
    "user.name=backspace",
    "-c",
    "user.email=backspace@localhost",
];

async fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(IDENT)
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .context("running git")?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success() {
        bail!("git {}: {}", args.join(" "), text.trim());
    }
    Ok(text.trim().to_string())
}

/// Make `root` a repo with at least one commit, and keep `.backspace/` out of
/// it. Returns a note for the log when it had to snapshot uncommitted work.
pub async fn ensure_repo(root: &Path) -> Result<Option<String>> {
    let dir = root.join(".backspace");
    tokio::fs::create_dir_all(&dir).await?;
    // Ignore our own folder without touching the user's .gitignore.
    tokio::fs::write(dir.join(".gitignore"), "*\n").await?;

    let mut note = None;
    if git(root, &["rev-parse", "--is-inside-work-tree"])
        .await
        .is_err()
    {
        git(root, &["init", "-q"]).await?;
        note = Some("initialised a git repository in the workspace".to_string());
    }
    if !git(root, &["status", "--porcelain"]).await?.is_empty()
        || git(root, &["rev-parse", "HEAD"]).await.is_err()
    {
        git(root, &["add", "-A"]).await?;
        git(
            root,
            &[
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "backspace: snapshot before run",
            ],
        )
        .await?;
        note = Some("committed a snapshot of uncommitted work so agents branch from it".into());
    }
    Ok(note)
}

pub async fn current_branch(dir: &Path) -> Result<String> {
    git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).await
}

/// Create `branch` from `base` checked out at `path`. Reuses an existing one.
pub async fn add_worktree(root: &Path, path: &Path, branch: &str, base: &str) -> Result<()> {
    if path.join(".git").exists() {
        return Ok(());
    }
    let p = path.to_string_lossy();
    if git(root, &["rev-parse", "--verify", "-q", branch])
        .await
        .is_ok()
    {
        git(root, &["worktree", "add", "-q", &p, branch]).await?;
    } else {
        git(root, &["worktree", "add", "-q", "-b", branch, &p, base]).await?;
    }
    Ok(())
}

/// Commit everything in `dir`. Returns false when there was nothing to commit.
pub async fn commit_all(dir: &Path, msg: &str) -> Result<bool> {
    git(dir, &["add", "-A"]).await?;
    if git(dir, &["diff", "--cached", "--quiet"]).await.is_ok() {
        return Ok(false);
    }
    git(dir, &["commit", "-q", "-m", msg]).await?;
    Ok(true)
}

/// `git diff --stat` of `branch` against where it forked from `base`.
pub async fn diff_stat(dir: &Path, base: &str, branch: &str) -> Result<String> {
    git(dir, &["diff", "--stat", &format!("{base}...{branch}")]).await
}

pub enum Merge {
    Merged,
    /// Conflicting files; the merge was aborted and `into` is unchanged.
    Conflict(String),
}

/// Merge `branch` into whatever `into` has checked out.
pub async fn merge(into: &Path, branch: &str, msg: &str) -> Result<Merge> {
    commit_all(into, "backspace: work in progress before merge").await?;
    match git(into, &["merge", "--no-ff", "-q", "-m", msg, branch]).await {
        Ok(_) => Ok(Merge::Merged),
        Err(e) => {
            let files = git(into, &["diff", "--name-only", "--diff-filter=U"])
                .await
                .unwrap_or_default();
            let _ = git(into, &["merge", "--abort"]).await;
            if files.is_empty() {
                Err(e)
            } else {
                Ok(Merge::Conflict(files))
            }
        }
    }
}

pub fn worktree_path(root: &Path, key: &str) -> PathBuf {
    root.join(".backspace").join("worktrees").join(key)
}

pub fn branch_name(key: &str) -> String {
    format!("backspace/{key}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn branch_merge_and_conflict() {
        let root = std::env::temp_dir().join(format!("bs-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("shared.txt"), "base\n").unwrap();
        assert!(ensure_repo(&root).await.unwrap().is_some());
        let base = current_branch(&root).await.unwrap();

        let (a, b) = (worktree_path(&root, "a"), worktree_path(&root, "b"));
        add_worktree(&root, &a, &branch_name("a"), &base)
            .await
            .unwrap();
        add_worktree(&root, &b, &branch_name("b"), &base)
            .await
            .unwrap();
        std::fs::write(a.join("shared.txt"), "from a\n").unwrap();
        std::fs::write(b.join("shared.txt"), "from b\n").unwrap();
        assert!(commit_all(&a, "a").await.unwrap());
        assert!(commit_all(&b, "b").await.unwrap());
        assert!(diff_stat(&root, &base, &branch_name("a"))
            .await
            .unwrap()
            .contains("shared.txt"));

        assert!(matches!(
            merge(&root, &branch_name("a"), "merge a").await.unwrap(),
            Merge::Merged
        ));
        assert_eq!(
            std::fs::read_to_string(root.join("shared.txt")).unwrap(),
            "from a\n"
        );
        match merge(&root, &branch_name("b"), "merge b").await.unwrap() {
            Merge::Conflict(f) => assert_eq!(f, "shared.txt"),
            Merge::Merged => panic!("expected a conflict"),
        }
        // Aborted cleanly: root still has a's version and no conflict markers.
        assert_eq!(
            std::fs::read_to_string(root.join("shared.txt")).unwrap(),
            "from a\n"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
