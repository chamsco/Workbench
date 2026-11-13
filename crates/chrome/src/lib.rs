//! Cauldron window chrome contract (docs/cauldron/UI-PLAN.md, milestone UI0).
//!
//! The numbers here come from the reviewed wireframe
//! (`tests/golden/wireframe.json`, measured from the published page). The GPUI
//! shell on macOS and Windows draws from this crate and nothing else, so the
//! two builds cannot drift apart. No GPUI dependency: it tests on any machine.

pub mod avatar;
pub mod layout;
pub mod motion;
pub mod panes;
pub mod tokens;
