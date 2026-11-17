//! Verify and Ship (PRD §4.4-4.5): red stub, see tests/deliver.rs.
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::ticket::TicketState;

pub const PLAN_JSON: &str = ".backspace/planning/plan.json";
pub const VERIFY_MD: &str = ".backspace/VERIFY.md";
pub const VERIFY_JSON: &str = ".backspace/verify.json";
pub const DELIVERY_MD: &str = ".backspace/DELIVERY.md";
pub const DELIVERY_JSON: &str = ".backspace/delivery.json";

pub struct TicketFacts {
    pub key: String,
    pub state: TicketState,
    pub covers: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct CriterionResult {
    pub key: String,
    pub criterion: String,
    pub command: String,
    pub passed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct VerifyReport {
    pub passed: bool,
    pub head: String,
    pub criteria: Vec<CriterionResult>,
    pub issues: Vec<String>,
    pub weakened: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub enum Target {
    #[default]
    Local,
    Branch,
    PullRequest,
}

#[derive(Default)]
pub struct ShipOpts {
    pub target: Target,
    pub gh: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct DeliveryRecord {
    pub commit: String,
    pub branch: String,
    pub status: String,
    pub pr_url: Option<String>,
    pub limitations: Vec<String>,
}

pub fn verify(_root: &Path, _tickets: &[TicketFacts]) -> Result<VerifyReport> {
    bail!("Verify is not built yet")
}

pub fn ship(_root: &Path, _opts: &ShipOpts) -> Result<DeliveryRecord> {
    bail!("Ship is not built yet")
}
