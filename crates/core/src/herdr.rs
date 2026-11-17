//! M6: herdr behind a swappable adapter. (stub)
use std::future::Future;
use std::pin::Pin;
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const PINNED: &str = "0.9.5";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct HerdrConfig { pub enabled: bool, pub version: String, pub bin: String, pub socket: Option<String> }
impl Default for HerdrConfig { fn default() -> Self { HerdrConfig { enabled: false, version: PINNED.into(), bin: "herdr".into(), socket: None } } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal { Idle, Working, Blocked, Done, Unknown }

pub type Signals<'a> = Pin<Box<dyn Future<Output = Result<Vec<(String, Signal)>>> + Send + 'a>>;

pub trait Supervisor: Send + Sync {
    fn name(&self) -> String;
    fn signals(&self) -> Signals<'_>;
}

pub struct Native;
impl Supervisor for Native {
    fn name(&self) -> String { todo!() }
    fn signals(&self) -> Signals<'_> { todo!() }
}

pub fn parse_version(_s: &str) -> Option<String> { todo!() }
pub fn select(_c: &HerdrConfig) -> (Box<dyn Supervisor>, Option<String>) { todo!() }
