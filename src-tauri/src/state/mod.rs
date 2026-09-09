//! Tauri-managed application state: the activity store, the last usage reading,
//! demo mode and settings — plus the single place that pushes snapshots to the UI.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use claude_notch_core::demo;
use claude_notch_core::snapshot::{ActivitySource, Snapshot};
use claude_notch_core::state::{ActivityStore, Applied, ClaudeState, Observation};
use claude_notch_core::time::now_ms;
use claude_notch_core::usage::{PollPolicy, UsageSnapshot};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use crate::settings::Settings;

/// Emitted with a [`Snapshot`] payload whenever anything the UI shows changes.
pub const SNAPSHOT_EVENT: &str = "notch:snapshot";
/// Emitted (no payload) when the tray asks the UI to show settings.
pub const OPEN_SETTINGS_EVENT: &str = "notch:open-settings";

/// Sessions silent for this long are dropped (a crashed Claude Code never sends SessionEnd).
const SESSION_MAX_IDLE_MS: u64 = 12 * 60 * 60 * 1000;

pub struct AppState {
    app: AppHandle,
    poll: PollPolicy,
    listening: AtomicBool,
    inner: Mutex<Inner>,
}

struct Inner {
    activity: ActivityStore,
    usage: UsageSnapshot,
    demo_mode: bool,
    /// Bumped on every demo toggle so a stale demo runner knows to stop.
    demo_generation: u64,
    settings: Settings,
}

impl AppState {
    pub fn new(app: AppHandle, settings: Settings) -> Self {
        Self {
            app,
            poll: PollPolicy::default(),
            listening: AtomicBool::new(false),
            inner: Mutex::new(Inner {
                activity: ActivityStore::new(ActivitySource::None),
                usage: UsageSnapshot::unavailable("Waiting for the first usage reading"),
                demo_mode: false,
                demo_generation: 0,
                settings,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn snapshot(&self) -> Snapshot {
        let inner = self.lock();
        let stale_after = self.poll.stale_after().as_millis() as u64;
        Snapshot {
            activity: inner.activity.snapshot(),
            usage: inner.usage.clone().with_freshness(now_ms(), stale_after),
            demo_mode: inner.demo_mode,
        }
    }

    /// Feed an observation through the state machine and publish the result.
    /// Observations from the wrong source for the current mode are dropped, so
    /// real hooks never leak into a demo and a stopped demo cannot resurrect itself.
    pub fn apply_observation(&self, obs: Observation, source: ActivitySource) -> Option<Applied> {
        let (applied, settings) = {
            let mut inner = self.lock();
            let expected = if inner.demo_mode {
                ActivitySource::Demo
            } else {
                ActivitySource::Hooks
            };
            if source != expected {
                return None;
            }
            if inner.activity.source() != source {
                inner.activity.set_source(source);
            }
            let at_ms = obs.at_ms;
            inner.activity.prune(at_ms, SESSION_MAX_IDLE_MS);
            (inner.activity.apply(obs), inner.settings.clone())
        };
        if applied.aggregate_changed() {
            self.notify(applied.aggregate_after, &settings);
        }
        self.emit();
        Some(applied)
    }

    pub fn set_usage(&self, usage: UsageSnapshot) {
        self.lock().usage = usage;
        self.emit();
    }

    pub fn last_usage(&self) -> UsageSnapshot {
        self.lock().usage.clone()
    }

    pub fn is_demo(&self) -> bool {
        self.lock().demo_mode
    }

    pub fn demo_generation(&self) -> u64 {
        self.lock().demo_generation
    }

    /// Toggle demo mode. Clears all sessions so the two sources never mix and
    /// returns the generation token the new demo runner must hold.
    pub fn set_demo_mode(&self, enabled: bool) -> u64 {
        let generation = {
            let mut inner = self.lock();
            inner.demo_mode = enabled;
            inner.demo_generation += 1;
            inner.activity.clear();
            inner.activity.set_source(if enabled {
                ActivitySource::Demo
            } else {
                ActivitySource::None
            });
            inner.usage = if enabled {
                demo::demo_usage(now_ms())
            } else {
                UsageSnapshot::unavailable("Waiting for the next usage reading")
            };
            inner.demo_generation
        };
        self.emit();
        generation
    }

    pub fn settings(&self) -> Settings {
        self.lock().settings.clone()
    }

    pub fn update_settings(&self, settings: Settings) {
        self.lock().settings = settings;
    }

    pub fn hook_listener_up(&self) -> bool {
        self.listening.load(Ordering::Relaxed)
    }

    pub fn set_hook_listener_up(&self, up: bool) {
        self.listening.store(up, Ordering::Relaxed);
    }

    /// Push the current snapshot to every webview.
    pub fn emit(&self) {
        let snapshot = self.snapshot();
        if let Err(err) = self.app.emit(SNAPSHOT_EVENT, &snapshot) {
            log::warn!("failed to emit snapshot: {err}");
        }
    }

    /// Optional system notification on attention-worthy transitions (PRD §8, §15).
    fn notify(&self, state: ClaudeState, settings: &Settings) {
        let body = match state {
            ClaudeState::Waiting if settings.notify_waiting => "Claude needs your input",
            ClaudeState::Done if settings.notify_done => "Claude finished the task",
            ClaudeState::Error if settings.notify_error => "Claude Code reported an error",
            _ => return,
        };
        if let Err(err) = self
            .app
            .notification()
            .builder()
            .title("Claude Notch")
            .body(body)
            .show()
        {
            log::warn!("notification failed: {err}");
        }
    }
}
