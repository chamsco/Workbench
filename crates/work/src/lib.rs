//! Work: rooms where people and agents talk to each other, and the rules an
//! org's admin sets on which models they may use and how much.
//!
//! The pieces come out of Workbench's chat (crates/core/src/chat.rs), with
//! the single-user assumption taken out:
//!
//! - [`room`]: a room has members, and a member is a person or an agent.
//!   Workbench's `Role::User | Role::Assistant` could not say *which* person
//!   wrote a message, so it could not hold a conversation between two.
//! - [`turns`]: who speaks next. Workbench's group turn-taking (@Name hands
//!   the floor over, PASS leaves no message, a cap on turns), plus the rules
//!   a room with several people needs: agents stay quiet unless named, and
//!   an agent that names a person stops the chain and waits for them.
//! - [`governance`]: the org's model catalog, grants per role, team and
//!   person, token allowances, invites, and the check before every turn.
//! - [`meter`]: runs a turn through `backspace-runner` and books what it
//!   cost against whoever pays for it.
//!
//! It knows nothing about storage, transport or a UI. Messages between
//! people (delivery, encryption, presence) are Anarchy's job; this crate is
//! what sits next to it and decides what the agents do.

pub mod governance;
pub mod meter;
pub mod room;
pub mod turns;

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
