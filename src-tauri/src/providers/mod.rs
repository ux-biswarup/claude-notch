//! Providers turn raw signals into normalised observations / readings (PRD §12–§13).
//!
//! * [`claude_activity`] — Claude Code hooks → activity observations (loopback HTTP listener)
//! * [`claude_usage`]    — Claude account usage via Claude Code's own OAuth session
//! * [`demo`]            — scripted observations that exercise the real pipeline
//!
//! The pure parts (parsing, mapping, policies) live in `claude-notch-core`;
//! this module owns the I/O and the long-running tasks.

pub mod claude_activity;
pub mod claude_usage;
pub mod demo;

use tauri::AppHandle;

/// Start the always-on providers. Demo mode is started on demand from the tray / UI.
pub fn start_all(app: AppHandle) {
    claude_activity::spawn(app.clone());
    claude_usage::spawn(app);
}
