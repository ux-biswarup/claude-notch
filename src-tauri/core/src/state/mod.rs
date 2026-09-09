//! Canonical activity state (PRD §7, §14) and the per-session store that
//! aggregates several Claude Code sessions into the one state the UI shows.

pub mod state_machine;
pub mod store;

pub use state_machine::{ActivityEvent, ClaudeState, transition};
pub use store::{ActivitySnapshot, ActivityStore, Applied, Observation, SessionInfo};
