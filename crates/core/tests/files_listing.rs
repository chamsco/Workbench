//! The UI's directory listing (`files::scan`) and the agent-facing file tools
//! must agree on where the workspace ends. `Workspace::resolve` already
//! refuses a symlink that leaves it; the listing used to follow such a link
//! and show the names inside the target, outside the workspace.
//!
//! Rules asserted here:
//! * a symlink is listed as a symlink (`link`), and an external one is never
//!   followed: its target's names are not enumerated;
//! * a dangling link is listed and not followed;
//! * an internal link keeps working as before (a link to a folder inside the
//!   workspace is listed as a folder and its contents shown, a second route to
//!   a folder included), except that a link back into the folder it sits in or
//!   above is not walked, so a loop cannot fill the listing;
//! * ordinary folders list as they did.
//!
//! Unix only: links are made with `std::os::unix`. Windows junctions are NOT
//! tested (no Windows runner here); whether `std` reports a junction as a
//! symlink there is unverified, so their behaviour is unknown.
#![cfg(unix)]

use std::os::unix::fs::symlink;
use std::path::PathBuf;

use backspace_core::files::{read_within, scan, FileEntry};

struct Fixture {
    root: PathBuf,
    outside: PathBuf,
}

fn fixture(tag: &str) -> Fixture {
    let base = std::env::temp_dir().join(format!("bs-listing-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (root, outside) = (base.join("project"), base.join("outside"));
    std::fs::create_dir_all(root.join("src/deep")).unwrap();
    std::fs::write(root.join("src/inside.txt"), "inside").unwrap();
    std::fs::write(root.join("src/deep/leaf.txt"), "leaf").unwrap();
    std::fs::write(root.join("README.md"), "readme").unwrap();
    std::fs::create_dir_all(outside.join("hidden-dir/deeper")).unwrap();
    std::fs::write(outside.join("secret.txt"), "TOP SECRET").unwrap();
    std::fs::write(outside.join("hidden-dir/deeper/buried.txt"), "x").unwrap();
    Fixture { root, outside }
}

fn names(es: &[FileEntry]) -> Vec<String> {
    es.iter().map(|e| e.name.clone()).collect()
}

fn entry<'a>(es: &'a [FileEntry], name: &str) -> &'a FileEntry {
    es.iter().find(|e| e.name == name).unwrap_or_else(|| panic!("no entry {name} in {:?}", names(es)))
}

/// Nothing from the outside tree is named anywhere in the listing.
fn assert_nothing_outside(es: &[FileEntry], f: &Fixture) {
    for leaked in ["secret.txt", "hidden-dir", "deeper", "buried.txt"] {
        assert!(!es.iter().any(|e| e.name == leaked), "{leaked} was listed: {:?}", names(es));
    }
    let out = f.outside.to_string_lossy().into_owned();
    assert!(!es.iter().any(|e| e.path.contains(&out)), "a path under the outside tree was listed: {:?}", names(es));
}

#[test]
fn an_external_symlinked_folder_is_shown_and_not_entered() {
    let f = fixture("extdir");
    symlink(&f.outside, f.root.join("out")).unwrap();
    let es = scan(&f.root);
    assert_nothing_outside(&es, &f);
    let e = entry(&es, "out");
    assert!(e.link, "shown as a symlink");
    assert!(!e.dir, "an external link is not offered as a folder to open");
}

#[test]
fn an_external_symlinked_file_is_shown_and_not_followed() {
    let f = fixture("extfile");
    symlink(f.outside.join("secret.txt"), f.root.join("leak.txt")).unwrap();
    let es = scan(&f.root);
    let e = entry(&es, "leak.txt");
    assert!(e.link && !e.dir);
    // The path the listing hands out cannot be read either.
    let err = read_within(&f.root, &e.path).expect_err("an external link must not be readable").to_string();
    assert!(err.contains("outside"), "{err}");
}

#[test]
fn an_internal_symlink_keeps_its_existing_behaviour() {
    let f = fixture("internal");
    symlink(f.root.join("src"), f.root.join("alias")).unwrap();
    symlink(f.root.join("src/inside.txt"), f.root.join("note.txt")).unwrap();
    let es = scan(&f.root);
    let alias = entry(&es, "alias");
    assert!(alias.link && alias.dir, "a link to a folder inside the workspace is still a folder");
    let kids: Vec<_> = es.iter().filter(|e| e.depth == alias.depth + 1 && e.path.starts_with(&alias.path)).map(|e| e.name.clone()).collect();
    assert!(kids.contains(&"inside.txt".to_string()) && kids.contains(&"deep".to_string()), "its contents are shown: {kids:?}");
    let note = entry(&es, "note.txt");
    assert!(note.link && !note.dir);
    assert_eq!(std::fs::read_to_string(&note.path).unwrap(), "inside");
}

#[test]
fn a_dangling_symlink_is_shown_and_not_followed() {
    let f = fixture("dangling");
    symlink(f.outside.join("not-there"), f.root.join("gone")).unwrap();
    symlink(f.root.join("missing-inside"), f.root.join("gone-inside")).unwrap();
    let es = scan(&f.root);
    for n in ["gone", "gone-inside"] {
        let e = entry(&es, n);
        assert!(e.link && !e.dir, "{n}: {e:?}");
    }
}

#[test]
fn a_symlink_below_the_top_cannot_reveal_an_external_folder() {
    let f = fixture("nested");
    // One level down, through a chain, and inside a folder reached by an internal link.
    symlink(&f.outside, f.root.join("src/deep/out")).unwrap();
    symlink(&f.outside, f.root.join("hop2")).unwrap();
    symlink(f.root.join("hop2"), f.root.join("hop1")).unwrap();
    symlink(f.root.join("src"), f.root.join("alias")).unwrap();
    let es = scan(&f.root);
    assert_nothing_outside(&es, &f);
    assert!(entry(&es, "hop1").link && !entry(&es, "hop1").dir, "a chain that ends outside is not a folder");
    assert!(es.iter().filter(|e| e.name == "out").all(|e| e.link && !e.dir));
}

#[test]
fn a_loop_back_into_an_ancestor_is_not_walked() {
    let f = fixture("loop");
    symlink(&f.root, f.root.join("src/up")).unwrap(); // a loop to the root
    symlink(f.root.join("src/deep"), f.root.join("src/deep/self")).unwrap(); // a loop to its own folder
    let es = scan(&f.root);
    assert!(es.len() < 20, "the walk is bounded: {} entries: {:?}", es.len(), names(&es));
    for n in ["up", "self"] {
        let e = entry(&es, n);
        assert!(e.link, "{n} is shown as a link");
        assert!(!es.iter().any(|x| x.path.starts_with(&format!("{}/", e.path))), "nothing is listed under {n}");
    }
    assert_eq!(es.iter().filter(|e| e.name == "leaf.txt").count(), 1, "{:?}", names(&es));
}

#[test]
fn a_second_route_to_a_folder_inside_the_workspace_is_still_listed_as_before() {
    let f = fixture("twice");
    symlink(f.root.join("src"), f.root.join("again")).unwrap();
    let es = scan(&f.root);
    assert_eq!(es.iter().filter(|e| e.name == "leaf.txt").count(), 2, "src's contents under src and under the link: {:?}", names(&es));
    assert_nothing_outside(&es, &f);
}

#[test]
fn ordinary_folders_list_as_before() {
    let f = fixture("plain");
    for skipped in [".git", "target", "node_modules", ".backspace", "dist"] {
        std::fs::create_dir_all(f.root.join(skipped)).unwrap();
        std::fs::write(f.root.join(skipped).join("x"), "x").unwrap();
    }
    let es = scan(&f.root);
    assert_eq!(names(&es), vec!["src", "deep", "leaf.txt", "inside.txt", "README.md"], "folders first, depth-first, skipped names left out");
    let depths: Vec<usize> = es.iter().map(|e| e.depth).collect();
    assert_eq!(depths, vec![0, 1, 2, 1, 0]);
    assert!(es.iter().all(|e| !e.link));
    assert!(entry(&es, "src").dir && !entry(&es, "README.md").dir);
}

#[test]
fn a_root_reached_through_a_symlink_lists_normally() {
    let f = fixture("rootlink");
    let link = f.root.parent().unwrap().join("project-link");
    symlink(&f.root, &link).unwrap();
    symlink(&f.outside, f.root.join("out")).unwrap();
    let es = scan(&link);
    assert_nothing_outside(&es, &f);
    assert!(names(&es).contains(&"inside.txt".to_string()), "{:?}", names(&es));
    assert!(entry(&es, "out").link && !entry(&es, "out").dir);
}
