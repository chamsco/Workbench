//! Bakes the commit this build came from into the binary, so the update check
//! can tell whether the rolling `mac-latest` build is a different one than the
//! build you are running. The macOS workflow exports `BACKSPACE_BUILD` as the
//! commit it is building; a build made by hand has no commit to record, and is
//! never offered an update.
//!
//! See `src/update.rs` for why a commit, and not a version, is what is compared.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=BACKSPACE_BUILD");
    println!("cargo:rerun-if-changed=build.rs");

    let asked = std::env::var("BACKSPACE_BUILD").unwrap_or_default();
    let asked = asked.trim().to_ascii_lowercase();
    // A commit hash, or nothing: a branch name, a dirty marker, or an empty
    // variable is not a commit, and recording it would leave the update check
    // comparing nonsense. The same test guards the other end, in `update::sha`.
    let stamp = if is_sha(&asked) { asked } else { String::new() };

    // The stamp is written as source, which `update.rs` includes. Cargo can hand
    // rustc an environment variable instead (`cargo:rustc-env`), but that leaves
    // the value to travel a further step before it lands in the binary, and this
    // one already went wrong once: the workflow exported the commit, the build
    // looked like it worked, and the shipped app could not find its own stamp.
    // What is written here is what the build script read, and nothing else.
    //
    // The macOS workflow fails the build when the stamp is missing; see
    // `.github/workflows/macos.yml`. A build with no stamp can never be told
    // there is an update, and it would fail silently.
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"))
        .join("build_stamp.rs");
    std::fs::write(
        &out,
        format!(
            "// Written by build.rs, from the BACKSPACE_BUILD this build was given.\n\
             // Do not edit: see ../../build.rs.\n\
             /// The commit this binary was built from, or \"\" when the build had\n\
             /// no commit to record -- a build made by hand.\n\
             pub const BUILD: &str = {stamp:?};\n"
        ),
    )
    .expect("write the build stamp");
}

/// Seven to forty hex characters, which is what a commit hash is.
fn is_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}
