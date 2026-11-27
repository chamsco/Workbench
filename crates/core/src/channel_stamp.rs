// Which release line this build belongs to: "live" or "nightly". The live app's workflows leave it
// as it is; a nightly workflow writes "nightly" here before building. See `instance.rs`.
pub const CHANNEL: &str = "live";
