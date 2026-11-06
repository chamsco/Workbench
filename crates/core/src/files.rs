//! Worktree listings and file reads, shared by the desktop shells and the
//! remote API so a connected machine browses files the same way.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileEntry {
    pub depth: usize,
    pub name: String,
    pub path: String,
    pub dir: bool,
    /// The entry is a symlink (shown as one; see `scan` for when it is followed).
    #[serde(default)]
    pub link: bool,
}

/// Depth-first walk, skipping build output and VCS internals.
///
/// A symlink is listed as one (`link`) and is followed only when it stays
/// inside the workspace, judged by real paths, the same boundary the agent
/// file tools use (`Workspace::resolve`). One that leaves the workspace, or
/// that dangles, is listed and never entered, so the names under an external
/// target are not exposed. A link back into an ancestor is not walked either.
/// Windows junctions are not tested: whether `std` reports one as a symlink
/// there is unverified.
pub fn scan(root: &Path) -> Vec<FileEntry> {
    let mut out = Vec::new();
    if let Ok(real_root) = root.canonicalize() {
        walk(root, &real_root, &real_root, 0, &mut out);
    }
    out
}

/// What one directory entry is, for listing and for whether to enter it.
struct Kind {
    link: bool,
    dir: bool,
    /// The real folder to walk into, when it is one to enter.
    enter: Option<PathBuf>,
}

fn kind(e: &std::fs::DirEntry, real_dir: &Path, real_root: &Path) -> Kind {
    let Ok(ft) = e.file_type() else {
        return Kind { link: false, dir: false, enter: None };
    };
    if !ft.is_symlink() {
        let dir = ft.is_dir();
        return Kind { link: false, dir, enter: dir.then(|| real_dir.join(e.file_name())) };
    }
    // A link: follow it only to a real folder or file inside the workspace.
    match e.path().canonicalize() {
        Ok(real) if real.starts_with(real_root) => {
            let dir = real.is_dir();
            // A link back into the folder it sits in (or above) is a loop.
            let enter = (dir && !real_dir.starts_with(&real)).then_some(real);
            Kind { link: true, dir, enter }
        }
        _ => Kind { link: true, dir: false, enter: None },
    }
}

fn walk(dir: &Path, real_dir: &Path, real_root: &Path, depth: usize, out: &mut Vec<FileEntry>) {
    if depth > 4 || out.len() > 2000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<_> = rd.flatten().map(|e| (kind(&e, real_dir, real_root), e)).collect();
    items.sort_by_key(|(k, e)| (!k.dir, e.file_name()));
    for (k, e) in items {
        let name = e.file_name().to_string_lossy().into_owned();
        if matches!(
            name.as_str(),
            ".git" | "target" | "node_modules" | ".backspace" | "dist" | ".astro"
        ) {
            continue;
        }
        let path = e.path();
        out.push(FileEntry {
            depth,
            name,
            path: path.to_string_lossy().into_owned(),
            dir: k.dir,
            link: k.link,
        });
        if let Some(real) = k.enter {
            walk(&path, &real, real_root, depth + 1, out);
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

/// An image inside `root` as a data: URL, for showing a screenshot an agent
/// wrote (the Options canvas). Capped at 8 MiB.
pub fn read_image_within(root: &Path, path: &str) -> Result<String> {
    let p = std::path::PathBuf::from(path).canonicalize()?;
    let root = root.canonicalize()?;
    if !p.starts_with(&root) {
        bail!("outside the project");
    }
    let bytes = std::fs::read(&p)?;
    if bytes.len() > 8 * 1024 * 1024 {
        bail!("image is too large to show (over 8 MiB)");
    }
    let mime = match p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    };
    Ok(format!(
        "data:{mime};base64,{}",
        crate::chat::b64_encode(&bytes)
    ))
}
