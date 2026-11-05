//! M3 intake (PLAN §5.1, §6): what happens to a first message before any
//! planning. RED STUB: the types exist so the acceptance tests compile; every
//! behaviour below is a placeholder that the tests are written to reject.

use std::path::Path;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::decision::TaskType;

pub const DEFAULT_MAX_ASKS: u8 = 2;
pub const ONE_SHOT_MAX_ACCEPTANCE: usize = 5;

/// Where the current task stands in intake. Persisted with the project.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct IntakeState {
    #[serde(default)]
    pub path: Option<TaskType>,
    #[serde(default)]
    pub pending: Option<PendingAsk>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PendingAsk {
    pub text: String,
    pub asks: u8,
}

pub fn question(_best: Option<(TaskType, f64)>) -> String {
    String::new()
}

pub fn parse_answer(_reply: &str) -> Option<TaskType> {
    None
}

pub fn allows(_path: TaskType, _tool: &str) -> bool {
    true
}

#[derive(Debug, Clone, PartialEq)]
pub struct OneShotSpec {
    pub key: String,
    pub title: String,
    pub goal: String,
    pub acceptance: Vec<String>,
    pub check: Option<String>,
}

impl OneShotSpec {
    pub fn from_tickets(_input: &Value) -> Result<OneShotSpec> {
        bail!("not implemented")
    }

    pub fn markdown(&self) -> String {
        String::new()
    }

    pub fn write(&self, _root: &Path) -> Result<String> {
        bail!("not implemented")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub title: String,
    pub body: String,
}

impl Report {
    pub fn from_json(_input: &Value) -> Result<Report> {
        bail!("not implemented")
    }

    pub fn write(&self, _root: &Path) -> Result<String> {
        bail!("not implemented")
    }
}
