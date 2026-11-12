//! Windows gives the main thread 1 MB of stack. GPUI lays out the whole window on it, and an
//! unoptimised build of the thread view needs more, so the app asks for 8 MB as macOS gives.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
}
