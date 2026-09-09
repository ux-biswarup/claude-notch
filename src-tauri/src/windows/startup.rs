//! Phase 6: launch on sign-in. `tauri-plugin-autostart` writes to
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` — per-user, no admin (PRD §10).

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// Make the registry match the setting; a no-op when it already does.
pub fn apply(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    let current = manager.is_enabled().map_err(|e| e.to_string())?;
    let result = match (enabled, current) {
        (true, false) => manager.enable(),
        (false, true) => manager.disable(),
        _ => return Ok(()),
    };
    result.map_err(|e| e.to_string())
}

pub fn is_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}
