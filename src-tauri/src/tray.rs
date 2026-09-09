//! Tray icon + menu. The overlay has no chrome of its own, so this is where
//! demo mode, hook installation, settings and quit live.

use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_notification::NotificationExt;

use crate::state::{AppState, OPEN_SETTINGS_EVENT};
use crate::{providers, windows};

const DEMO_ITEM: &str = "demo";
const HOOKS_ITEM: &str = "install-hooks";
const SETTINGS_ITEM: &str = "settings";
const SNAP_ITEM: &str = "snap";
const QUIT_ITEM: &str = "quit";

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let demo = CheckMenuItem::with_id(app, DEMO_ITEM, "Demo mode", true, false, None::<&str>)?;
    let hooks = MenuItem::with_id(app, HOOKS_ITEM, "Connect Claude Code…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, SETTINGS_ITEM, "Settings…", true, None::<&str>)?;
    let snap = MenuItem::with_id(app, SNAP_ITEM, "Snap to top-right", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ITEM, "Quit Claude Notch", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;

    let items: [&dyn IsMenuItem<Wry>; 6] = [&demo, &hooks, &settings, &snap, &separator, &quit];
    let menu = Menu::with_items(app, &items)?;

    let demo_for_handler = demo.clone();
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Claude Notch")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            handle_menu(app, event.id().0.as_str(), &demo_for_handler);
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

fn handle_menu(app: &AppHandle, id: &str, demo: &CheckMenuItem<Wry>) {
    match id {
        DEMO_ITEM => {
            let enabled = demo.is_checked().unwrap_or(false);
            let generation = app.state::<AppState>().set_demo_mode(enabled);
            if enabled {
                providers::demo::start(app.clone(), generation);
            }
        }
        HOOKS_ITEM => {
            let state = app.state::<AppState>();
            let result = providers::claude_activity::install(
                state.settings().hook_port,
                state.hook_listener_up(),
            );
            let (title, body) = match result {
                Ok(status) => (
                    "Claude Code connected",
                    status.message.unwrap_or_else(|| "Hooks installed".into()),
                ),
                Err(err) => ("Could not connect Claude Code", err),
            };
            if let Err(err) = app.notification().builder().title(title).body(body).show() {
                log::warn!("notification failed: {err}");
            }
        }
        SETTINGS_ITEM => {
            windows::overlay::reveal(app);
            if let Err(err) = app.emit(OPEN_SETTINGS_EVENT, ()) {
                log::warn!("failed to emit open-settings: {err}");
            }
        }
        SNAP_ITEM => windows::overlay::reveal(app),
        QUIT_ITEM => app.exit(0),
        _ => {}
    }
}
