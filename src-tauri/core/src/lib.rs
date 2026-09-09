//! Claude Notch core: everything that can be reasoned about without a window.
//!
//! * [`state`] — the canonical activity state machine and per-session store (PRD §7, §14)
//! * [`hooks`] — Claude Code hook payloads → normalised observations (PRD §19 phase 3)
//! * [`usage`] — usage parsing, fidelity, staleness and polling policy (PRD §13, §16)
//! * [`demo`] — a scripted session that exercises the whole pipeline (PRD §16.6)
//! * [`http`] — the tiny HTTP/1.1 parser used by the loopback hook listener
//! * [`snapshot`] — the serialisable contract shared with the UI (`src/state/State.ts`)
//!
//! This crate deliberately has no Tauri or Windows dependencies, so it compiles
//! and its tests run with any Rust toolchain — including the GNU one, which
//! needs no Visual Studio Build Tools.

pub mod demo;
pub mod hooks;
pub mod http;
pub mod snapshot;
pub mod state;
pub mod time;
pub mod usage;

pub use snapshot::{ActivitySource, Fidelity, Snapshot};
pub use state::{ActivityEvent, ActivityStore, ClaudeState, Observation};
pub use usage::UsageSnapshot;
