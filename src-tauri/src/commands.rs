//! Commands invoked by the UI (see `src/app/ipc.ts`). Argument names are
//! converted to camelCase on the JS side by Tauri.

use claude_notch_core::snapshot::Snapshot;
use tauri::{AppHandle, State};

use crate::providers::claude_activity::{self, HooksStatus};
use crate::settings::{self, Settings};
use crate::state::AppState;
use crate::{providers, windows};

#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> Snapshot {
    state.snapshot()
}

#[tauri::command]
pub fn set_demo_mode(app: AppHandle, state: State<'_, AppState>, enabled: bool) -> Snapshot {
    let generation = state.set_demo_mode(enabled);
    if enabled {
        providers::demo::start(app, generation);
    }
    state.snapshot()
}

/// Bring the window owning the (given or focus) session to the foreground (PRD §9).
#[tauri::command]
pub fn focus_claude(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> Result<bool, String> {
    let snapshot = state.snapshot();
    let target = session_id.or_else(|| snapshot.activity.focus_session_id.clone());
    let cwd = target.and_then(|id| {
        snapshot
            .activity
            .sessions
            .iter()
            .find(|s| s.session_id == id)
            .and_then(|s| s.cwd.clone())
    });
    windows::focus::focus_claude_code(cwd.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    settings::save(&app, &settings)?;
    windows::startup::apply(&app, settings.launch_on_sign_in)?;
    state.update_settings(settings.clone());
    Ok(settings)
}

#[tauri::command]
pub fn install_hooks(state: State<'_, AppState>) -> Result<HooksStatus, String> {
    claude_activity::install(state.settings().hook_port, state.hook_listener_up())
}

#[tauri::command]
pub fn hooks_status(state: State<'_, AppState>) -> HooksStatus {
    claude_activity::status(state.settings().hook_port, state.hook_listener_up())
}

#[tauri::command]
pub fn reposition(app: AppHandle) -> Result<(), String> {
    windows::overlay::place_top_right(&app).map_err(|e| e.to_string())
}

/// The UI reports its rendered size so the transparent window can shrink to it.
#[tauri::command]
pub fn resize_to_content(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    windows::overlay::resize(&app, width, height).map_err(|e| e.to_string())
}
