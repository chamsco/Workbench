//! Checkpoints: the folder as it was when each message was sent, kept in the folder's own git
//! as hidden commits under `refs/cauldron/<stream>/<message>`. Nothing touches the user's index,
//! branch or stash: a snapshot is built in a throwaway index file, so untracked files are in it
//! and ignored ones are not. A folder that is not a git repository has no checkpoints.

use std::path::{Path, PathBuf};

fn git(dir: &Path, index: Option<&Path>, args: &[&str]) -> Option<String> {
    let mut c = backspace_runner::cmd("git");
    c.arg("-C").arg(dir).args(["-c", "user.name=Cauldron", "-c", "user.email=cauldron@localhost", "-c", "commit.gpgsign=false"]).args(args);
    if let Some(i) = index {
        c.env("GIT_INDEX_FILE", i);
    }
    let out = c.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The tree of everything in the folder now, tracked and untracked, ignoring what .gitignore does.
pub fn snapshot_tree(dir: &Path) -> Option<String> {
    let gd = git(dir, None, &["rev-parse", "--absolute-git-dir"])?;
    let idx: PathBuf = Path::new(&gd).join(format!("cauldron-index-{}-{}", std::process::id(), crate::chat::now_ms()));
    // Start from HEAD's tree so unchanged files are not re-hashed.
    if git(dir, None, &["rev-parse", "--verify", "-q", "HEAD"]).is_some() {
        git(dir, Some(&idx), &["read-tree", "HEAD"]);
    }
    let tree = git(dir, Some(&idx), &["add", "-A", "--", "."]).and_then(|_| git(dir, Some(&idx), &["write-tree"]));
    let _ = std::fs::remove_file(&idx);
    tree
}

fn name(stream: &str, msg: &str) -> String {
    let safe = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect::<String>();
    format!("refs/cauldron/{}/{}", safe(stream), safe(msg))
}

/// Saves the folder as it is now for this message. Returns the checkpoint's commit.
pub fn take(dir: &Path, stream: &str, msg: &str) -> Option<String> {
    let tree = snapshot_tree(dir)?;
    let mut args = vec!["commit-tree", tree.as_str(), "-m", "Cauldron checkpoint"];
    let head = git(dir, None, &["rev-parse", "--verify", "-q", "HEAD"]);
    if let Some(h) = head.as_deref() {
        args.extend(["-p", h]);
    }
    let commit = git(dir, None, &args)?;
    git(dir, None, &["update-ref", &name(stream, msg), &commit])?;
    Some(commit)
}

/// One saved checkpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub msg: String,
    pub commit: String,
    pub at: u64,
}

/// This stream's checkpoints, newest first.
pub fn list(dir: &Path, stream: &str) -> Vec<Point> {
    let prefix = name(stream, "");
    let Some(out) = git(dir, None, &["for-each-ref", "--sort=-creatordate", "--format=%(refname)%09%(objectname)%09%(creatordate:unix)", &prefix]) else { return Vec::new() };
    out.lines()
        .filter_map(|l| {
            let mut p = l.split('\t');
            let (r, c, t) = (p.next()?, p.next()?, p.next()?);
            Some(Point { msg: r.strip_prefix(&prefix)?.to_string(), commit: c.to_string(), at: t.parse::<u64>().ok()? * 1000 })
        })
        .collect()
}

/// Puts the folder back as `commit` had it: files changed since come back, files made since go
/// away. A checkpoint of the present is taken first (as message "before-restore"), so a restore
/// can itself be undone.
pub fn restore(dir: &Path, stream: &str, commit: &str) -> Result<(), String> {
    let now = snapshot_tree(dir).ok_or("This folder is not a git repository.")?;
    take(dir, stream, &format!("before-restore-{}", crate::chat::now_ms()));
    let added = git(dir, None, &["diff", "--name-only", "--diff-filter=A", commit, &now]).unwrap_or_default();
    for f in added.lines().filter(|f| !f.is_empty()) {
        let _ = std::fs::remove_file(dir.join(f));
    }
    git(dir, None, &["checkout", commit, "--", "."]).ok_or("Could not restore the files.")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(dir: &Path, args: &[&str]) {
        assert!(backspace_runner::cmd("git").arg("-C").arg(dir).args(args).status().unwrap().success(), "{args:?}");
    }

    #[test]
    fn a_checkpoint_keeps_the_folder_and_restore_puts_it_back() {
        let dir = std::env::temp_dir().join(format!("ckpt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        sh(&dir, &["init", "-q"]);
        std::fs::write(dir.join("a.txt"), "one").unwrap();
        sh(&dir, &["add", "."]);
        sh(&dir, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "first"]);
        std::fs::write(dir.join("a.txt"), "two").unwrap();
        std::fs::write(dir.join("new.txt"), "n").unwrap();
        let c = take(&dir, "s1", "m1").expect("a checkpoint");
        // The agent works on.
        std::fs::write(dir.join("a.txt"), "three").unwrap();
        std::fs::write(dir.join("later.txt"), "x").unwrap();
        std::fs::remove_file(dir.join("new.txt")).unwrap();
        assert_eq!(list(&dir, "s1").first().map(|p| (p.msg.as_str(), p.commit.as_str())), Some(("m1", c.as_str())));
        restore(&dir, "s1", &c).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "two");
        assert_eq!(std::fs::read_to_string(dir.join("new.txt")).unwrap(), "n");
        assert!(!dir.join("later.txt").exists());
        // The user's own index and branch were never touched.
        assert!(git(&dir, None, &["status", "--porcelain"]).unwrap().contains("a.txt"));
        assert_eq!(list(&dir, "s1").len(), 2, "the restore left its own checkpoint");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
