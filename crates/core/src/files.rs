//! Worktree listings and file reads, shared by the desktop shells and the
//! remote API so a connected machine browses files the same way.

use std::path::Path;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileEntry {
    pub depth: usize,
    pub name: String,
    pub path: String,
    pub dir: bool,
}

/// Depth-first walk, skipping build output and VCS internals.
pub fn scan(root: &Path) -> Vec<FileEntry> {
    let mut out = Vec::new();
    walk(root, 0, &mut out);
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<FileEntry>) {
    if depth > 4 || out.len() > 2000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<_> = rd.flatten().collect();
    items.sort_by_key(|e| (!e.path().is_dir(), e.file_name()));
    for e in items {
        let name = e.file_name().to_string_lossy().into_owned();
        if matches!(
            name.as_str(),
            ".git" | "target" | "node_modules" | ".backspace" | "dist" | ".astro"
        ) {
            continue;
        }
        let path = e.path();
        let dir = path.is_dir();
        out.push(FileEntry {
            depth,
            name,
            path: path.to_string_lossy().into_owned(),
            dir,
        });
        if dir {
            walk(&path, depth + 1, out);
        }
    }
}

/// Text of a file inside `root` (worktrees live under it), first 256 KiB.
pub fn read_within(root: &Path, path: &str) -> Result<String> {
    let p = std::path::PathBuf::from(path).canonicalize()?;
    let root = root.canonicalize()?;
    if !p.starts_with(&root) {
        bail!("outside the project");
    }
    let bytes = std::fs::read(&p)?;
    let cut = &bytes[..bytes.len().min(256 * 1024)];
    Ok(String::from_utf8_lossy(cut).into_owned())
}
