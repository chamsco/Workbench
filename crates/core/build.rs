//! Bakes the commit this build came from into the binary, so the update check
//! can tell whether the rolling `mac-latest` build is a different one than the
//! build you are running. The macOS workflow exports `BACKSPACE_BUILD` as the
//! commit it is building; a build made by hand has no commit to record, and is
//! never offered an update.
//!
//! See `src/update.rs` for why a commit, and not a version, is what is compared.

fn main() {
    println!("cargo:rerun-if-env-changed=BACKSPACE_BUILD");
    println!("cargo:rerun-if-changed=build.rs");

    let asked = std::env::var("BACKSPACE_BUILD").unwrap_or_default();
    let asked = asked.trim().to_ascii_lowercase();
    // A short or full SHA. Anything else (a branch name, a dirty marker, an
    // empty variable) is not a commit, and is not recorded at all, leaving the
    // binary with no stamp: see `update::BUILD`.
    let is_sha = (7..=40).contains(&asked.len()) && asked.bytes().all(|b| b.is_ascii_hexdigit());
    if is_sha {
        println!("cargo:rustc-env=BACKSPACE_BUILD={asked}");
    }
}
