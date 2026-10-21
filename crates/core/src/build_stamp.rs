// The commit this binary was built from, or "" for a build made by hand.
//
// CI overwrites this file before building, with the commit it is building, so
// the value sits in the source the compiler reads: no environment variable to
// export, no build script to run, nothing between the workflow and rustc that
// can quietly not happen.
//
// `update.rs` includes this file; see there for what the stamp is for, and why
// it also keeps a copy of itself in the binary where the workflow can find it.
pub const BUILD: &str = "";
