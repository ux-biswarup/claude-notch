//! Demo runner (PRD §16.6). Plays `core::demo::script()` through the real
//! state pipeline until demo mode is switched off or re-armed.

use claude_notch_core::demo::{DemoCursor, demo_usage, observation};
use claude_notch_core::snapshot::ActivitySource;
use claude_notch_core::state::ActivityEvent;
use claude_notch_core::time::now_ms;
use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// `generation` is the token returned by `AppState::set_demo_mode`; the runner
/// exits as soon as it no longer matches, so toggles never leave ghosts behind.
pub fn start(app: AppHandle, generation: u64) {
    tauri::async_runtime::spawn(async move {
        let mut cursor = DemoCursor::new();
        loop {
            let state = app.state::<AppState>();
            if !state.is_demo() || state.demo_generation() != generation {
                break;
            }
            let step = cursor.next_step();
            if step.event == ActivityEvent::ProcessStarted {
                // Keep the demo usage reading fresh so it never shows as stale.
                state.set_usage(demo_usage(now_ms()));
            }
            state.apply_observation(observation(&step, now_ms()), ActivitySource::Demo);
            tokio::time::sleep(step.hold).await;
        }
        log::debug!("demo runner {generation} stopped");
    });
}
