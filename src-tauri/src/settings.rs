//! User settings (PRD §15, phase 6). Stored as JSON in the per-user app config
//! directory (`%APPDATA%\com.claude-notch.desktop\settings.json`). Mirrors
//! `Settings` in `src/state/State.ts`.

use std::fs;
use std::path::PathBuf;

use claude_notch_core::hooks::DEFAULT_PORT;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    Always,
    Edge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    #[default]
    TopRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReducedMotion {
    #[default]
    System,
    On,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub appearance: Appearance,
    pub notify_done: bool,
    pub notify_waiting: bool,
    pub notify_error: bool,
    pub launch_on_sign_in: bool,
    pub position: Position,
    pub reduced_motion: ReducedMotion,
    /// Loopback port the Claude Code hooks post to.
    pub hook_port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            appearance: Appearance::Always,
            // PRD §8: no persistent toast for Done by default; Waiting is the strongest attention state.
            notify_done: false,
            notify_waiting: true,
            notify_error: true,
            launch_on_sign_in: false,
            position: Position::TopRight,
            reduced_motion: ReducedMotion::System,
            hook_port: DEFAULT_PORT,
        }
    }
}

pub fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    let Some(path) = path(app) else {
        return Settings::default();
    };
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|err| {
            log::warn!("ignoring unreadable settings at {}: {err}", path.display());
            Settings::default()
        }),
        Err(_) => Settings::default(),
    }
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = path(app).ok_or("cannot resolve the app config directory")?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))
}
