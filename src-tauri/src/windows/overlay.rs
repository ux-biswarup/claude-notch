//! Phase 1: the always-on-top, transparent, top-right overlay (PRD §19).
//! Window flags (transparent, no decorations, always on top, skip taskbar,
//! no focus on show) come from `tauri.conf.json`; this module owns placement.

use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, WebviewWindow};

use crate::MAIN_WINDOW;

/// Gap between the overlay and the screen edge, in logical pixels.
const MARGIN_LOGICAL_PX: f64 = 8.0;

pub fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN_WINDOW)
}

/// Position and show the overlay at startup.
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = main_window(app) else {
        log::error!("main window '{MAIN_WINDOW}' not found");
        return Ok(());
    };
    place(&window)?;
    window.show()
}

/// Show (if hidden) and snap back to the top-right corner.
pub fn reveal(app: &AppHandle) {
    if let Some(window) = main_window(app) {
        if let Err(err) = window.show() {
            log::warn!("cannot show overlay: {err}");
        }
        if let Err(err) = place(&window) {
            log::warn!("cannot place overlay: {err}");
        }
    }
}

pub fn place_top_right(app: &AppHandle) -> tauri::Result<()> {
    match main_window(app) {
        Some(window) => place(&window),
        None => Ok(()),
    }
}

/// Resize to the UI's rendered content (logical pixels) and re-anchor to the corner.
pub fn resize(app: &AppHandle, width: f64, height: f64) -> tauri::Result<()> {
    let Some(window) = main_window(app) else {
        return Ok(());
    };
    let width = width.clamp(120.0, 600.0);
    let height = height.clamp(80.0, 900.0);
    window.set_size(LogicalSize::new(width, height))?;
    place(&window)
}

/// Anchor the window to the top-right of the primary monitor's work area
/// (i.e. above other windows but not over the taskbar).
fn place(window: &WebviewWindow) -> tauri::Result<()> {
    let monitor = match window.primary_monitor()? {
        Some(monitor) => monitor,
        None => match window.current_monitor()? {
            Some(monitor) => monitor,
            None => return Ok(()),
        },
    };
    let area = monitor.work_area();
    let size = window.outer_size()?;
    let margin = (MARGIN_LOGICAL_PX * monitor.scale_factor()).round() as i32;

    let x = area.position.x + area.size.width as i32 - size.width as i32 - margin;
    let y = area.position.y + margin;
    window.set_position(PhysicalPosition::new(x, y))
}
