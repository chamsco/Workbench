//! The file tools (`read`, `list`, `write`, `edit`) stay inside the workspace.
//! `Workspace::resolve` used to check the path lexically, so a symlink inside
//! the project that points outside it was followed: `read` returned the
//! target's text, `write` created files beside it. The research path leans on
//! `read` and `list` alone (no shell), which made this the path's real
//! boundary.
//!
//! What is covered: the four file tools. What is not: `bash`, which is not
//! sandboxed until M6, and a symlink planted between the check and the use
//! (it would take a second process, and `bash` already reaches further).
//!
//! Unix only: symlinks are created with `std::os::unix`.
#![cfg(unix)]

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::time::Duration;

use backspace_core::tools::Workspace;
use serde_json::json;

struct Fixture {
    ws: Workspace,
    root: PathBuf,
    outside: PathBuf,
}

fn fixture(tag: &str) -> Fixture {
    let base = std::env::temp_dir().join(format!("bs-boundary-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (root, outside) = (base.join("project"), base.join("outside"));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), "TOP SECRET").unwrap();
    std::fs::write(root.join("src/inside.txt"), "inside text").unwrap();
    Fixture { ws: Workspace { root: root.clone(), bash_timeout: Duration::from_secs(5) }, root, outside }
}

async fn refused(ws: &Workspace, tool: &str, input: serde_json::Value) -> String {
    let e = ws.run(tool, &input).await.expect_err(&format!("{tool} {input} must be refused"));
    format!("{e:#}")
}

fn says_symlink(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("symlink") && m.contains("workspace")
}

#[tokio::test]
async fn a_symlink_to_a_file_outside_cannot_be_read() {
    let f = fixture("file");
    symlink(f.outside.join("secret.txt"), f.root.join("leak.txt")).unwrap();
    let e = refused(&f.ws, "read", json!({"path": "leak.txt"})).await;
    assert!(says_symlink(&e), "{e}");
    assert!(!e.contains("TOP SECRET"));
}

#[tokio::test]
async fn a_symlinked_folder_outside_cannot_be_read_through_listed_or_written_into() {
    let f = fixture("dir");
    symlink(&f.outside, f.root.join("out")).unwrap();
    assert!(says_symlink(&refused(&f.ws, "read", json!({"path": "out/secret.txt"})).await));
    assert!(says_symlink(&refused(&f.ws, "list", json!({"path": "out"})).await));
    assert!(says_symlink(&refused(&f.ws, "write", json!({"path": "out/new.txt", "content": "x"})).await));
    assert!(says_symlink(&refused(&f.ws, "edit", json!({"path": "out/secret.txt", "old_text": "TOP", "new_text": "LOW"})).await));
    assert!(!f.outside.join("new.txt").exists(), "nothing was written outside");
    assert_eq!(std::fs::read_to_string(f.outside.join("secret.txt")).unwrap(), "TOP SECRET", "nothing was edited outside");
}

#[tokio::test]
async fn writing_through_a_dangling_symlink_cannot_create_a_file_outside() {
    let f = fixture("dangling");
    symlink(f.outside.join("not-yet.txt"), f.root.join("trap.txt")).unwrap();
    let e = refused(&f.ws, "write", json!({"path": "trap.txt", "content": "x"})).await;
    assert!(e.to_lowercase().contains("symlink"), "{e}");
    assert!(!f.outside.join("not-yet.txt").exists());
}

#[tokio::test]
async fn a_chain_of_symlinks_is_followed_to_its_end() {
    let f = fixture("chain");
    symlink(f.outside.join("secret.txt"), f.root.join("hop2")).unwrap();
    symlink(f.root.join("hop2"), f.root.join("hop1")).unwrap();
    assert!(says_symlink(&refused(&f.ws, "read", json!({"path": "hop1"})).await));
}

#[tokio::test]
async fn an_absolute_path_through_a_symlink_is_refused_too() {
    let f = fixture("abs");
    symlink(&f.outside, f.root.join("out")).unwrap();
    let abs = f.root.join("out/secret.txt");
    assert!(says_symlink(&refused(&f.ws, "read", json!({"path": abs.to_string_lossy()})).await));
}

#[tokio::test]
async fn a_symlink_that_stays_inside_the_workspace_still_works() {
    let f = fixture("inside");
    symlink(f.root.join("src"), f.root.join("alias")).unwrap();
    symlink(f.root.join("src/inside.txt"), f.root.join("note.txt")).unwrap();
    assert_eq!(f.ws.run("read", &json!({"path": "note.txt"})).await.unwrap(), "inside text");
    assert_eq!(f.ws.run("read", &json!({"path": "alias/inside.txt"})).await.unwrap(), "inside text");
    assert!(f.ws.run("list", &json!({"path": "alias"})).await.unwrap().contains("inside.txt"));
    f.ws.run("write", &json!({"path": "alias/made.txt", "content": "y"})).await.unwrap();
    assert_eq!(std::fs::read_to_string(f.root.join("src/made.txt")).unwrap(), "y");
}

#[tokio::test]
async fn a_workspace_that_is_itself_reached_through_a_symlink_works() {
    // /tmp is a symlink on some systems, and a worktree can be opened by a link.
    let f = fixture("rootlink");
    let link: PathBuf = f.root.parent().unwrap().join("project-link");
    symlink(&f.root, &link).unwrap();
    let ws = Workspace { root: link.clone(), bash_timeout: Duration::from_secs(5) };
    assert_eq!(ws.run("read", &json!({"path": "src/inside.txt"})).await.unwrap(), "inside text");
    symlink(f.outside.join("secret.txt"), f.root.join("leak.txt")).unwrap();
    assert!(says_symlink(&refused(&ws, "read", json!({"path": "leak.txt"})).await));
}

#[tokio::test]
async fn listing_shows_a_symlink_by_name_and_never_its_target_contents() {
    let f = fixture("listing");
    symlink(&f.outside, f.root.join("out")).unwrap();
    let listing = f.ws.run("list", &json!({})).await.unwrap();
    assert!(listing.lines().any(|l| l == "out"), "a symlink is listed as a file, not followed: {listing}");
    assert!(!listing.contains("secret.txt"));
}

#[test]
fn the_lexical_rules_still_hold() {
    let w = Workspace { root: Path::new("/tmp/bs-boundary-nonexistent-root").to_path_buf(), bash_timeout: Duration::from_secs(1) };
    assert!(w.resolve("../x").is_err());
    assert!(w.resolve("/etc/passwd").is_err());
    assert_eq!(w.resolve("a/../b.txt").unwrap(), Path::new("/tmp/bs-boundary-nonexistent-root/b.txt"));
}
