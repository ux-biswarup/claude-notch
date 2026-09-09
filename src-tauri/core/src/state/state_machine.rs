//! The canonical activity state machine (PRD §7, §14).
//!
//! Providers translate raw signals into [`ActivityEvent`]s; [`transition`]
//! maps them onto the five normalised states the UI understands. Most of the
//! intelligence lives in the provider mapping (see `hooks.rs`); the machine
//! itself stays small and fully tabulated so its behaviour is easy to audit.

use serde::{Deserialize, Serialize};

/// Normalised state consumed by the UI. Exactly one is active at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ClaudeState {
    #[default]
    Idle,
    Working,
    Waiting,
    Done,
    Error,
}

impl ClaudeState {
    pub const ALL: [ClaudeState; 5] = [
        Self::Idle,
        Self::Working,
        Self::Waiting,
        Self::Done,
        Self::Error,
    ];

    /// Attention priority when several sessions disagree (higher wins).
    /// "Does Claude need me?" outranks everything else (PRD §3, §22.4).
    pub const fn priority(self) -> u8 {
        match self {
            Self::Waiting => 4,
            Self::Error => 3,
            Self::Working => 2,
            Self::Done => 1,
            Self::Idle => 0,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Working => "Working",
            Self::Waiting => "Waiting for you",
            Self::Done => "Done",
            Self::Error => "Something went wrong",
        }
    }
}

/// Observations a provider can report (PRD §14). Providers own the mapping
/// from their raw signals to these; the UI never sees them directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityEvent {
    /// A Claude Code session started, resumed, or was cleared.
    ProcessStarted,
    /// The user submitted a prompt; Claude is now working on it.
    PromptSubmitted,
    /// Claude is about to run a tool.
    ToolRunning,
    /// Claude produced output or a tool returned; work continues.
    OutputReceived,
    /// Claude is blocked on the user (permission prompt, question, elicitation).
    WaitingForInput,
    /// Claude finished responding for this turn.
    TaskCompleted,
    /// The turn failed (API error, crash).
    ErrorDetected,
    /// The session ended.
    ProcessExited,
}

/// Pure transition function. Returning the current state means "no change".
pub fn transition(current: ClaudeState, event: ActivityEvent) -> ClaudeState {
    use ActivityEvent as E;
    use ClaudeState as S;

    match (current, event) {
        (_, E::ProcessStarted) => S::Idle,
        (_, E::ProcessExited) => S::Idle,
        (_, E::PromptSubmitted) => S::Working,
        (_, E::ToolRunning) => S::Working,
        (_, E::OutputReceived) => S::Working,
        (_, E::WaitingForInput) => S::Waiting,
        (_, E::ErrorDetected) => S::Error,
        // An error is sticky: a trailing Stop after a failure must not paint it
        // white. Only fresh activity (prompt/tool/output) clears it.
        (S::Error, E::TaskCompleted) => S::Error,
        (_, E::TaskCompleted) => S::Done,
    }
}

#[cfg(test)]
mod tests {
    use super::ActivityEvent::*;
    use super::ClaudeState::*;
    use super::*;

    #[test]
    fn follows_the_prd_diagram() {
        assert_eq!(transition(Idle, PromptSubmitted), Working);
        assert_eq!(transition(Working, WaitingForInput), Waiting);
        assert_eq!(transition(Waiting, OutputReceived), Working);
        assert_eq!(transition(Working, TaskCompleted), Done);
        assert_eq!(transition(Working, ErrorDetected), Error);
        assert_eq!(transition(Done, ProcessExited), Idle);
    }

    #[test]
    fn a_new_prompt_restarts_from_any_state() {
        for state in ClaudeState::ALL {
            assert_eq!(
                transition(state, PromptSubmitted),
                Working,
                "from {state:?}"
            );
            assert_eq!(transition(state, ToolRunning), Working, "from {state:?}");
        }
    }

    #[test]
    fn session_lifecycle_events_reset_to_idle() {
        for state in ClaudeState::ALL {
            assert_eq!(transition(state, ProcessStarted), Idle);
            assert_eq!(transition(state, ProcessExited), Idle);
        }
    }

    #[test]
    fn error_is_sticky_until_fresh_activity() {
        assert_eq!(transition(Error, TaskCompleted), Error);
        assert_eq!(transition(Error, OutputReceived), Working);
        assert_eq!(transition(Error, PromptSubmitted), Working);
    }

    #[test]
    fn waiting_outranks_everything() {
        let mut sorted = ClaudeState::ALL.to_vec();
        sorted.sort_by_key(|s| std::cmp::Reverse(s.priority()));
        assert_eq!(sorted, vec![Waiting, Error, Working, Done, Idle]);
    }

    #[test]
    fn serialises_lowercase_for_the_ui() {
        assert_eq!(serde_json::to_string(&Waiting).unwrap(), "\"waiting\"");
        assert_eq!(
            serde_json::to_string(&WaitingForInput).unwrap(),
            "\"waiting_for_input\""
        );
    }
}
