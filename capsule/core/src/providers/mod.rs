pub mod hyperv;
pub mod wsl;

use crate::model::{Capabilities, CreateSpec, Workspace};
use crate::powershell;
use anyhow::Result;

pub trait Provider {
    fn name(&self) -> &'static str;
    fn available(&self) -> bool;
    fn create(&self, spec: &CreateSpec) -> Result<Workspace>;
    fn start(&self, name: &str) -> Result<Workspace>;
    fn stop(&self, name: &str, force: bool) -> Result<Workspace>;
    fn delete(&self, name: &str) -> Result<()>;
    fn list(&self) -> Result<Vec<Workspace>>;
    fn status(&self, name: &str) -> Result<Workspace>;
}

pub fn all() -> Vec<Box<dyn Provider>> {
    vec![Box::new(hyperv::HyperVProvider), Box::new(wsl::WslProvider)]
}

pub fn capabilities() -> Capabilities {
    Capabilities {
        hyperv: powershell::hyperv_present(),
        wsl: wsl::WslProvider.available(),
        elevated: powershell::elevated(),
    }
}
