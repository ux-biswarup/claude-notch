//! Demo mode (PRD §16.6): a scripted session that drives the *real* pipeline
//! (state machine → store → snapshot → UI) without Claude Code running, so the
//! UX can be built and tuned in isolation. Mirrored in `src/app/demo.ts` for
//! the browser preview.

use std::time::Duration;

use crate::snapshot::Fidelity;
use crate::state::{ActivityEvent, Observation};
use crate::usage::{UsageSnapshot, UsageStatus, UsageWindow};

pub const DEMO_SESSION: &str = "demo";
pub const DEMO_CWD: &str = r"C:\Personal\projects\claude-notch";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoStep {
    pub event: ActivityEvent,
    pub detail: &'static str,
    /// How long to stay on this step before advancing.
    pub hold: Duration,
}

const fn step(event: ActivityEvent, detail: &'static str, hold_ms: u64) -> DemoStep {
    DemoStep {
        event,
        detail,
        hold: Duration::from_millis(hold_ms),
    }
}

/// Working → Waiting → Working → Done → Working → Error → (exit) → …
pub fn script() -> &'static [DemoStep] {
    use ActivityEvent::*;
    static SCRIPT: [DemoStep; 9] = [
        step(ProcessStarted, "Session started", 1_500),
        step(PromptSubmitted, "Reading the request", 2_000),
        step(ToolRunning, "Running Bash", 3_000),
        step(WaitingForInput, "Needs permission: Edit", 5_000),
        step(OutputReceived, "Applying edit", 2_500),
        step(TaskCompleted, "Task completed", 4_000),
        step(PromptSubmitted, "Working on a follow-up", 2_000),
        step(ErrorDetected, "API request failed", 4_000),
        step(ProcessExited, "Session ended", 1_500),
    ];
    &SCRIPT
}

/// Cycles through [`script`] forever.
#[derive(Debug, Default)]
pub struct DemoCursor {
    next: usize,
}

impl DemoCursor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_step(&mut self) -> DemoStep {
        let steps = script();
        let step = steps[self.next % steps.len()];
        self.next = (self.next + 1) % steps.len();
        step
    }
}

pub fn observation(step: &DemoStep, now_ms: u64) -> Observation {
    Observation::new(DEMO_SESSION, step.event, now_ms)
        .with_cwd(Some(DEMO_CWD.to_string()))
        .with_detail(Some(step.detail.to_string()))
}

/// A plausible, clearly-labelled usage reading for demo mode (fidelity = manual).
pub fn demo_usage(now_ms: u64) -> UsageSnapshot {
    let five_hour_reset = now_ms + (2 * 60 + 13) * 60_000;
    UsageSnapshot {
        status: UsageStatus::Ok,
        percent: Some(72.0),
        resets_at: Some(five_hour_reset),
        window_label: "5h".into(),
        fidelity: Fidelity::Manual,
        account: Some("Demo".into()),
        fetched_at: Some(now_ms),
        error: None,
        windows: vec![
            UsageWindow {
                id: "five_hour".into(),
                label: "5h".into(),
                percent: 72.0,
                resets_at: Some(five_hour_reset),
            },
            UsageWindow {
                id: "seven_day".into(),
                label: "7d".into(),
                percent: 31.0,
                resets_at: Some(now_ms + 3 * 86_400_000),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::ActivitySource;
    use crate::state::{ActivityStore, ClaudeState};

    #[test]
    fn script_visits_every_state_through_the_real_pipeline() {
        let mut store = ActivityStore::new(ActivitySource::Demo);
        let mut cursor = DemoCursor::new();
        let mut seen = Vec::new();
        for i in 0..script().len() {
            let step = cursor.next_step();
            store.apply(observation(&step, i as u64));
            seen.push(store.aggregate());
        }
        for state in ClaudeState::ALL {
            assert!(
                seen.contains(&state),
                "demo never shows {state:?}: {seen:?}"
            );
        }
        assert_eq!(*seen.last().unwrap(), ClaudeState::Idle);
        assert!(store.is_empty(), "exit must clear the demo session");
    }

    #[test]
    fn cursor_wraps() {
        let mut cursor = DemoCursor::new();
        let first = cursor.next_step();
        for _ in 1..script().len() {
            cursor.next_step();
        }
        assert_eq!(cursor.next_step(), first);
    }

    #[test]
    fn demo_usage_is_labelled_manual() {
        let usage = demo_usage(1_000);
        assert_eq!(usage.fidelity, Fidelity::Manual);
        assert_eq!(usage.percent, Some(72.0));
        assert_eq!(usage.account.as_deref(), Some("Demo"));
    }
}
