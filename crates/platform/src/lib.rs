//! Platform services (PRD §3): what every app on Cauldron shares and none of
//! them owns. Apps publish typed events on the [`events::Bus`]; the platform
//! turns the ones a person should see into notes in the [`notify::Inbox`].
//! This crate depends on no app: an app depends on it.

pub mod events;
pub mod notify;
