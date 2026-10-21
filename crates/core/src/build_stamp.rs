// The commit this binary was built from, or "" for a build made by hand.
//
// CI overwrites this file before building, with the commit it is building. The
// value sits here, in the source the compiler reads, because every attempt to
// carry it alongside the build dropped it: the workflow exported the commit, the
// build script saw it and wrote it into OUT_DIR, and the shipped app still had
// no stamp. A file the compiler includes cannot be skipped without an error.
//
// `update.rs` includes this file; see there for what the stamp is for.
pub const BUILD: &str = "";
