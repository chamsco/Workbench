//! The decision engine (PRD §5): typed decisions with one calibrated
//! distribution per question, and the cost ladder that says where a step
//! starts and when it may climb.
//!
//! It depends on nothing in Cauldron. A host plugs in its own model
//! descriptions by implementing [`ladder::Rung`]; persistence, model serving
//! and notifications stay with the host.

pub mod decision;
pub mod ladder;
