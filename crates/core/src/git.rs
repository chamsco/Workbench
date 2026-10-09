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
    git_env(dir, args, &[]).await
}

async fn git_env(dir: &Path, args: &[&str], env: &[(&str, &Path)]) -> Result<String> {
    let out = Command::new("git")
        .args(IDENT)
        .args(args)
        .envs(env.iter().map(|(k, v)| (*k, *v)))
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

/// The branch a project's agents land their work on. Yours is never touched.
pub const RUN_BRANCH: &str = "backspace/run";

/// Names that look like secrets: left out of the snapshot agents start from.
fn secret_like(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    name.starts_with(".env")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || name.starts_with("credentials")
        || name.starts_with("secrets")
        || [".pem", ".key", ".p12", ".pfx", ".keystore", ".jks"].iter().any(|e| name.ends_with(e))
}

/// Make `root` a repo and give the agents their own branch, `backspace/run`,
/// checked out in `.backspace/worktrees/_run`. A new one starts from a
/// snapshot of your checkout (HEAD plus uncommitted and new files) built in a
/// scratch index with `commit-tree`, so your branch, index and working tree
/// are left exactly as they were; secret-looking files are left out. An
/// existing one is reused, so a reopened project carries on where it was.
/// Returns the run worktree and notes for the log.
pub async fn ensure_run(root: &Path) -> Result<(PathBuf, Vec<String>)> {
    let dir = root.join(".backspace");
    tokio::fs::create_dir_all(&dir).await?;
    // Ignore our own folder without touching the user's .gitignore.
    tokio::fs::write(dir.join(".gitignore"), "*\n").await?;

    let mut notes = Vec::new();
    if git(root, &["rev-parse", "--is-inside-work-tree"]).await.is_err() {
        git(root, &["init", "-q"]).await?;
        notes.push("initialised a git repository in the workspace".to_string());
    }
    let path = worktree_path(root, "_run");
    let p = path.to_string_lossy().to_string();
    if git(root, &["rev-parse", "--verify", "-q", RUN_BRANCH]).await.is_ok() {
        if !path.join(".git").exists() {
            let _ = git(root, &["worktree", "prune"]).await;
            git(root, &["worktree", "add", "-q", &p, RUN_BRANCH]).await?;
        }
        notes.push(format!("carrying on `{RUN_BRANCH}`"));
        return Ok((path, notes));
    }

    let index = dir.join("index-snapshot");
    let _ = tokio::fs::remove_file(&index).await;
    let env = [("GIT_INDEX_FILE", index.as_path())];
    let head = git(root, &["rev-parse", "--verify", "-q", "HEAD"]).await.ok();
    if head.is_some() {
        git_env(root, &["read-tree", "HEAD"], &env).await?;
    }
    git_env(root, &["add", "-A"], &env).await?;
    let new_files = git(root, &["ls-files", "--others", "--exclude-standard"]).await.unwrap_or_default();
    let skipped: Vec<&str> = new_files.lines().filter(|f| secret_like(f)).collect();
    for f in &skipped {
        git_env(root, &["rm", "--cached", "-q", "--", f], &env).await?;
    }
    let tree = git_env(root, &["write-tree"], &env).await?;
    let _ = tokio::fs::remove_file(&index).await;
    let mut args = vec!["commit-tree", tree.as_str(), "-m", "backspace: snapshot of your checkout"];
    if let Some(h) = &head {
        args.extend(["-p", h.as_str()]);
    }
    let commit = git(root, &args).await?;
    if head.is_some() && git(root, &["diff", "--quiet", "HEAD", &commit]).await.is_err() {
        notes.push("started from your checkout, uncommitted changes included".into());
    }
    if !skipped.is_empty() {
        notes.push(format!("left out files that look like secrets: {}", skipped.join(", ")));
    }
    git(root, &["branch", RUN_BRANCH, &commit]).await?;
    git(root, &["worktree", "add", "-q", &p, RUN_BRANCH]).await?;
    notes.push(format!("agents work on `{RUN_BRANCH}`; your branch is left alone. Merge it when you're happy: git merge {RUN_BRANCH}"));
    Ok((path, notes))
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

/// Per-file added/removed lines of `branch` against where it forked from
/// `base`. Binary files count as 0/0.
pub async fn numstat(
    dir: &Path,
    base: &str,
    branch: &str,
) -> Result<Vec<crate::project::FileChange>> {
    let out = git(dir, &["diff", "--numstat", &format!("{base}...{branch}")]).await?;
    Ok(out
        .lines()
        .filter_map(|l| {
            let mut it = l.splitn(3, '\t');
            let (a, r, p) = (it.next()?, it.next()?, it.next()?);
            Some(crate::project::FileChange {
                path: p.to_string(),
                added: a.parse().unwrap_or(0),
                removed: r.parse().unwrap_or(0),
            })
        })
        .collect())
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

/// Commits a day over the last `days` days (oldest first) across every
/// branch, and lines added and removed, for Home's activity widget.
pub fn activity(dir: &Path, days: u32) -> Result<(Vec<u32>, u64, u64)> {
    let out = std::process::Command::new("git")
        .args(["log", "--all", &format!("--since={days}.days"), "--date=short", "--format=@%cd", "--numstat"])
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .output()?;
    if !out.status.success() {
        bail!("not a git repository");
    }
    let today = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() / 86_400;
    let (mut per_day, mut added, mut removed) = (vec![0u32; days as usize], 0u64, 0u64);
    for l in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(d) = l.strip_prefix('@') {
            if let Some(n) = day_number(d) {
                let back = today.saturating_sub(n) as usize;
                if back < per_day.len() {
                    let i = per_day.len() - 1 - back;
                    per_day[i] += 1;
                }
            }
        } else {
            let mut it = l.split('\t');
            added += it.next().and_then(|a| a.parse::<u64>().ok()).unwrap_or(0);
            removed += it.next().and_then(|r| r.parse::<u64>().ok()).unwrap_or(0);
        }
    }
    Ok((per_day, added, removed))
}

/// The last `n` commits on every branch, newest first: (short hash,
/// subject, author, when, branch names).
pub fn log(dir: &Path, n: usize) -> Result<Vec<(String, String, String, String, String)>> {
    let out = std::process::Command::new("git")
        .args(["log", "--all", &format!("-n{n}"), "--date=relative", "--format=%h%x1f%s%x1f%an%x1f%ad%x1f%D"])
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .output()?;
    if !out.status.success() {
        bail!("not a git repository");
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\x1f');
            Some((f.next()?.into(), f.next()?.into(), f.next()?.into(), f.next()?.into(), f.next().unwrap_or("").into()))
        })
        .collect())
}

/// One uncommitted change: git's two-letter status, the path, lines added
/// and removed against HEAD (0 and 0 for a new untracked file).
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Change {
    pub status: String,
    pub path: String,
    pub added: u64,
    pub removed: u64,
}

fn git_out(dir: &Path, args: &[&str]) -> Result<String> {
    let out = std::process::Command::new("git").args(args).current_dir(dir).stdin(std::process::Stdio::null()).output()?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// What is uncommitted in `dir` (staged, unstaged and untracked), and the
/// branch it is on: the side panel's Changes.
pub fn changes(dir: &Path) -> Result<(String, Vec<Change>)> {
    // `--show-current` names a branch with no commit yet; detached, it is empty.
    let branch = git_out(dir, &["branch", "--show-current"]).map(|b| b.trim().to_string()).unwrap_or_default();
    let branch = if branch.is_empty() { git_out(dir, &["rev-parse", "--short", "HEAD"]).map(|h| h.trim().to_string()).unwrap_or_default() } else { branch };
    let status = git_out(dir, &["status", "--porcelain=v1", "-uall"])?;
    // A repo with no commit yet has no HEAD to diff against.
    let numstat = git_out(dir, &["diff", "HEAD", "--numstat"]).unwrap_or_default();
    let mut files = parse_changes(&status, &numstat);
    // ponytail: the first 500; a folder of thousands of untracked files says nothing more.
    files.truncate(500);
    Ok((branch, files))
}

fn parse_changes(status: &str, numstat: &str) -> Vec<Change> {
    let counts: std::collections::HashMap<String, (u64, u64)> = numstat
        .lines()
        .filter_map(|l| {
            let mut f = l.splitn(3, '\t');
            let (a, r, p) = (f.next()?, f.next()?, f.next()?);
            Some((renamed_to(p), (a.parse().unwrap_or(0), r.parse().unwrap_or(0))))
        })
        .collect();
    status
        .lines()
        .filter(|l| l.len() > 3)
        .map(|l| {
            let path = l[3..].rsplit(" -> ").next().unwrap_or(&l[3..]).trim_matches('"').to_string();
            let (added, removed) = counts.get(&path).copied().unwrap_or((0, 0));
            Change { status: l[..2].trim().to_string(), path, added, removed }
        })
        .collect()
}

/// numstat writes a rename as "old => new" or "dir/{old => new}/rest": the new path.
fn renamed_to(p: &str) -> String {
    match (p.find('{'), p.find('}')) {
        (Some(a), Some(b)) if a < b => {
            let inner = &p[a + 1..b];
            format!("{}{}{}", &p[..a], inner.rsplit(" => ").next().unwrap_or(inner), &p[b + 1..])
        }
        _ => p.rsplit(" => ").next().unwrap_or(p).to_string(),
    }
}

/// One file's uncommitted diff against HEAD; a new file is shown whole.
pub fn diff_file(dir: &Path, file: &str) -> Result<String> {
    let d = git_out(dir, &["diff", "HEAD", "--", file]).unwrap_or_default();
    if !d.trim().is_empty() {
        return Ok(d);
    }
    let root = dunce::canonicalize(dir)?;
    let p = dunce::canonicalize(root.join(file))?;
    if !p.starts_with(&root) {
        bail!("{file} is outside the project");
    }
    let text = std::fs::read_to_string(&p)?;
    Ok(format!("new file {file}\n{}", text.lines().map(|l| format!("+{l}\n")).collect::<String>()))
}

/// Days since 1970-01-01 for a YYYY-MM-DD date.
fn day_number(d: &str) -> Option<u64> {
    let mut p = d.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, day) = (p.next()??, p.next()??, p.next()??);
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    u64::try_from(era * 146_097 + doe - 719_468).ok()
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

    #[test]
    fn parse_changes_reads_status_and_counts() {
        let status = " M src/a.rs\n?? new.txt\nR  old.rs -> lib/b.rs\nA  c.rs\n";
        let numstat = "3\t1\tsrc/a.rs\n5\t0\t{old.rs => lib/b.rs}\n2\t0\tc.rs\n";
        let c = parse_changes(status, numstat);
        assert_eq!(c.len(), 4);
        assert_eq!((c[0].status.as_str(), c[0].path.as_str(), c[0].added, c[0].removed), ("M", "src/a.rs", 3, 1));
        assert_eq!((c[1].status.as_str(), c[1].added), ("??", 0));
        assert_eq!((c[2].path.as_str(), c[2].added), ("lib/b.rs", 5));
        assert_eq!(c[3].added, 2);
        assert_eq!(renamed_to("src/{a => b}/x.rs"), "src/b/x.rs");
    }

    #[test]
    fn day_numbers() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(day_number("2026-10-07"), Some(20_733));
    }

    #[tokio::test]
    async fn branch_merge_and_conflict() {
        let root = std::env::temp_dir().join(format!("bs-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("shared.txt"), "base\n").unwrap();
        std::fs::write(root.join(".env"), "SECRET=1\n").unwrap();
        let (run, notes) = ensure_run(&root).await.unwrap();
        assert!(notes.iter().any(|n| n.contains(".env")), "{notes:?}");
        // Your checkout is untouched: no commit on it, nothing staged.
        assert!(git(&root, &["rev-parse", "--verify", "-q", "HEAD"]).await.is_err());
        assert!(run.join("shared.txt").exists() && !run.join(".env").exists());
        let base = RUN_BRANCH;

        let (a, b) = (worktree_path(&root, "a"), worktree_path(&root, "b"));
        add_worktree(&root, &a, &branch_name("a"), base).await.unwrap();
        add_worktree(&root, &b, &branch_name("b"), base).await.unwrap();
        std::fs::write(a.join("shared.txt"), "from a\n").unwrap();
        std::fs::write(b.join("shared.txt"), "from b\n").unwrap();
        assert!(commit_all(&a, "a").await.unwrap());
        assert!(commit_all(&b, "b").await.unwrap());
        assert!(diff_stat(&root, base, &branch_name("a")).await.unwrap().contains("shared.txt"));

        assert!(matches!(merge(&run, &branch_name("a"), "merge a").await.unwrap(), Merge::Merged));
        let read = || std::fs::read_to_string(run.join("shared.txt")).unwrap().replace("\r\n", "\n");
        assert_eq!(read(), "from a\n");
        match merge(&run, &branch_name("b"), "merge b").await.unwrap() {
            Merge::Conflict(f) => assert_eq!(f, "shared.txt"),
            Merge::Merged => panic!("expected a conflict"),
        }
        // Aborted cleanly: the run still has a's version and no conflict markers.
        assert_eq!(read(), "from a\n");
        // Your file is as you left it.
        assert_eq!(std::fs::read_to_string(root.join("shared.txt")).unwrap(), "base\n");
        // Reopening carries on the same branch.
        assert!(ensure_run(&root).await.unwrap().1[0].contains("carrying on"));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
