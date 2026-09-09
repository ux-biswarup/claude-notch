//! Claude Activity Provider (PRD §13, phase 3).
//!
//! Claude Code runs our hook command at each subscribed lifecycle event; the
//! command forwards the JSON payload to `POST http://127.0.0.1:<port>/hook`.
//! This module hosts that listener and feeds observations into [`AppState`].
//! It also installs / inspects the hooks in `~/.claude/settings.json`.

use std::fs;
use std::path::Path;

use claude_notch_core::hooks::{self, observation_from_hook};
use claude_notch_core::http::{
    HttpRequest, MAX_REQUEST_BYTES, ParseStatus, parse_request, response,
};
use claude_notch_core::snapshot::ActivitySource;
use claude_notch_core::time::now_ms;
use claude_notch_core::usage::settings_path;
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{Duration, timeout};

use crate::state::AppState;

const READ_TIMEOUT: Duration = Duration::from_secs(3);

/// Start the loopback listener on the configured port.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let port = app.state::<AppState>().settings().hook_port;
        if let Err(err) = serve(app.clone(), port).await {
            log::error!("hook listener on 127.0.0.1:{port} stopped: {err}");
            app.state::<AppState>().set_hook_listener_up(false);
        }
    });
}

async fn serve(app: AppHandle, port: u16) -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    log::info!("listening for Claude Code hooks on http://127.0.0.1:{port}/hook");
    app.state::<AppState>().set_hook_listener_up(true);
    loop {
        let (stream, _) = listener.accept().await?;
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = handle(app, stream).await {
                log::debug!("hook connection error: {err}");
            }
        });
    }
}

async fn handle(app: AppHandle, mut stream: TcpStream) -> std::io::Result<()> {
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];

    let request = loop {
        match parse_request(&buf) {
            ParseStatus::Complete(req) => break req,
            ParseStatus::Invalid => {
                stream
                    .write_all(&response(400, "Bad Request", "invalid request"))
                    .await?;
                return Ok(());
            }
            ParseStatus::Incomplete => {
                if buf.len() > MAX_REQUEST_BYTES {
                    stream
                        .write_all(&response(413, "Payload Too Large", ""))
                        .await?;
                    return Ok(());
                }
                let read = match timeout(READ_TIMEOUT, stream.read(&mut chunk)).await {
                    Ok(Ok(0)) => return Ok(()),
                    Ok(Ok(n)) => n,
                    Ok(Err(err)) => return Err(err),
                    Err(_) => {
                        stream
                            .write_all(&response(408, "Request Timeout", ""))
                            .await?;
                        return Ok(());
                    }
                };
                buf.extend_from_slice(&chunk[..read]);
            }
        }
    };

    let reply = route(&app, &request);
    stream.write_all(&reply).await?;
    stream.shutdown().await
}

fn route(app: &AppHandle, req: &HttpRequest) -> Vec<u8> {
    match (req.method.as_str(), req.path.as_str()) {
        ("POST", "/hook") => {
            let body = String::from_utf8_lossy(&req.body);
            match observation_from_hook(&body, now_ms()) {
                Ok(Some(obs)) => {
                    log::debug!("hook → {:?} for session {}", obs.event, obs.session_id);
                    app.state::<AppState>()
                        .apply_observation(obs, ActivitySource::Hooks);
                    response(204, "No Content", "")
                }
                Ok(None) => response(204, "No Content", ""),
                Err(err) => {
                    log::warn!("ignoring malformed hook payload: {err}");
                    response(400, "Bad Request", &err.to_string())
                }
            }
        }
        ("GET", "/health") => response(200, "OK", "ok"),
        _ => response(404, "Not Found", ""),
    }
}

// --- hook installation --------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HooksStatus {
    pub installed: bool,
    pub settings_path: Option<String>,
    pub port: u16,
    pub listening: bool,
    pub message: Option<String>,
}

fn read_settings(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let raw =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&raw).map_err(|e| format!("{} is not valid JSON: {e}", path.display()))
}

/// Are our hooks present in the user's Claude Code settings?
pub fn status(port: u16, listening: bool) -> HooksStatus {
    let Some(path) = settings_path() else {
        return HooksStatus {
            installed: false,
            settings_path: None,
            port,
            listening,
            message: Some("Cannot resolve the Claude Code config directory".into()),
        };
    };
    let installed = read_settings(&path)
        .map(|value| hooks::hooks_installed(&value, port))
        .unwrap_or(false);
    HooksStatus {
        installed,
        settings_path: Some(path.display().to_string()),
        port,
        listening,
        message: None,
    }
}

/// Merge our hooks into `~/.claude/settings.json`, backing the file up first.
/// Other hooks and settings are preserved (see `core::hooks::install_hooks`).
pub fn install(port: u16, listening: bool) -> Result<HooksStatus, String> {
    let path = settings_path()
        .ok_or("Cannot resolve the Claude Code config directory (USERPROFILE is not set)")?;
    let mut value = read_settings(&path)?;
    if !value.is_object() && !value.is_null() {
        return Err(format!("{} is not a JSON object", path.display()));
    }

    let changed = hooks::install_hooks(&mut value, port);
    if changed {
        if path.exists() {
            let backup = path.with_extension("json.claude-notch.bak");
            fs::copy(&path, &backup).map_err(|e| format!("cannot back up settings: {e}"))?;
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let pretty = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
        fs::write(&path, pretty + "\n")
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        log::info!("installed Claude Code hooks into {}", path.display());
    }

    Ok(HooksStatus {
        installed: true,
        settings_path: Some(path.display().to_string()),
        port,
        listening,
        message: Some(if changed {
            "Hooks installed. New Claude Code sessions will report their state.".into()
        } else {
            "Hooks were already installed.".into()
        }),
    })
}
