//! The serialisable contract shared with the UI. Mirrors `src/state/State.ts`.

use serde::{Deserialize, Serialize};

use crate::state::ActivitySnapshot;
use crate::usage::UsageSnapshot;

/// How trustworthy a reading is (PRD §16.3). The UI must not present
/// `Derived` or `Manual` data as authoritative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Fidelity {
    /// Straight from Anthropic's account endpoint.
    Official,
    /// Computed or inferred locally (e.g. from transcripts).
    Derived,
    /// Entered by the user or produced by demo mode.
    #[default]
    Manual,
}

/// Where activity observations come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ActivitySource {
    Hooks,
    Demo,
    #[default]
    None,
}

/// Everything the UI needs, in one message (`notch:snapshot`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub activity: ActivitySnapshot,
    pub usage: UsageSnapshot,
    pub demo_mode: bool,
}
