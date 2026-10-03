//! Backspace core: a mixture-of-models agent harness. No UI dependencies;
//! the GPUI app and the CLI are thin shells over [`Harness`].

pub mod config;
pub mod diagram;
pub mod effort;
pub mod files;
pub mod fleet;
pub mod git;
pub mod harness;
pub mod prefs;
pub mod project;
pub mod provider;
pub mod remote;
pub mod router;
pub mod skills;
pub mod ticket;
pub mod tools;
pub mod update;

pub use config::Config;
pub use effort::Effort;
pub use harness::Harness;
pub use project::*;
pub use ticket::{Ticket, TicketState};
