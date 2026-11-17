//! The sandbox's launcher on Windows (sandbox.rs): runs one command at low
//! integrity in a job object. Any Backspace binary does the same when given
//! `__backspace_jail` first; this one exists for the tests and for builds
//! that ship it beside the app.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(backspace_core::sandbox::jail_run(&args));
}
