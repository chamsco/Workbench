//! Backspace core: a mixture-of-models agent harness. No UI dependencies;
//! the GPUI app and the CLI are thin shells over [`Harness`].

pub mod config;
pub mod effort;
pub mod git;
pub mod harness;
pub mod project;
pub mod provider;
pub mod router;
pub mod skills;
pub mod ticket;
pub mod tools;

pub use config::Config;
pub use effort::Effort;
pub use harness::Harness;
pub use project::*;
pub use ticket::{Ticket, TicketState};
