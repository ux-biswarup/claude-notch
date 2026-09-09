//! Claude Notch — the Tauri shell.
//!
//! Wires providers (Claude Code hooks, usage polling, demo) into the shared
//! [`AppState`], hosts the always-on-top overlay window and the tray menu, and
//! exposes a handful of commands to the UI. All logic that does not need Tauri
//! or Win32 lives in the `claude-notch-core` crate.

mod commands;
mod providers;
mod settings;
mod state;
mod tray;
mod windows;

use tauri::Manager;

pub use state::AppState;

/// Label of the overlay window (see `tauri.conf.json`).
pub const MAIN_WINDOW: &str = "main";

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        // Must be registered first so a second launch just reveals the existing overlay.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::overlay::reveal(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = settings::load(&handle);
            log::info!("settings: {settings:?}");
            app.manage(AppState::new(handle.clone(), settings));

            windows::overlay::init(&handle)?;
            tray::init(&handle)?;
            providers::start_all(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::set_demo_mode,
            commands::focus_claude,
            commands::get_settings,
            commands::set_settings,
            commands::install_hooks,
            commands::hooks_status,
            commands::reposition,
            commands::resize_to_content,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Claude Notch");
}
