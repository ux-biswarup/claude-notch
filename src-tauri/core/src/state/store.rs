//! Per-session store. Several Claude Code sessions can run at once (two VS Code
//! windows, a terminal); each has its own state, and the UI shows the one that
//! most needs attention (PRD §9: "identify the relevant session").

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{ActivityEvent, ClaudeState, transition};
use crate::snapshot::ActivitySource;

/// A normalised observation from any provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub session_id: String,
    pub event: ActivityEvent,
    /// Epoch milliseconds.
    pub at_ms: u64,
    pub cwd: Option<String>,
    /// Human-readable context, e.g. "Needs permission: Bash".
    pub detail: Option<String>,
}

impl Observation {
    pub fn new(session_id: impl Into<String>, event: ActivityEvent, at_ms: u64) -> Self {
        Self {
            session_id: session_id.into(),
            event,
            at_ms,
            cwd: None,
            detail: None,
        }
    }

    pub fn with_cwd(mut self, cwd: Option<String>) -> Self {
        self.cwd = cwd;
        self
    }

    pub fn with_detail(mut self, detail: Option<String>) -> Self {
        self.detail = detail;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub state: ClaudeState,
    pub cwd: Option<String>,
    pub last_event: ActivityEvent,
    pub last_event_at: u64,
    pub detail: Option<String>,
}

/// What the UI receives for the activity half of the snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySnapshot {
    /// Aggregate across sessions (waiting > error > working > done > idle).
    pub state: ClaudeState,
    pub reason: Option<String>,
    /// Session that drives the aggregate; target for "Open Claude Code".
    pub focus_session_id: Option<String>,
    pub sessions: Vec<SessionInfo>,
    pub source: ActivitySource,
    pub updated_at: u64,
}

/// Result of applying one observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    pub session_before: ClaudeState,
    pub session_after: ClaudeState,
    pub aggregate_before: ClaudeState,
    pub aggregate_after: ClaudeState,
}

impl Applied {
    pub fn aggregate_changed(&self) -> bool {
        self.aggregate_before != self.aggregate_after
    }
}

#[derive(Debug, Default)]
pub struct ActivityStore {
    sessions: BTreeMap<String, SessionInfo>,
    source: ActivitySource,
    updated_at: u64,
}

impl ActivityStore {
    pub fn new(source: ActivitySource) -> Self {
        Self {
            source,
            ..Self::default()
        }
    }

    pub fn source(&self) -> ActivitySource {
        self.source
    }

    pub fn set_source(&mut self, source: ActivitySource) {
        self.source = source;
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Feed one observation through the state machine for its session.
    pub fn apply(&mut self, obs: Observation) -> Applied {
        let aggregate_before = self.aggregate();
        let Observation {
            session_id,
            event,
            at_ms,
            cwd,
            detail,
        } = obs;

        let session_before = self
            .sessions
            .get(&session_id)
            .map(|s| s.state)
            .unwrap_or_default();
        let session_after = transition(session_before, event);

        if event == ActivityEvent::ProcessExited {
            self.sessions.remove(&session_id);
        } else {
            let entry = self
                .sessions
                .entry(session_id.clone())
                .or_insert_with(|| SessionInfo {
                    session_id,
                    state: session_after,
                    cwd: None,
                    last_event: event,
                    last_event_at: at_ms,
                    detail: None,
                });
            entry.state = session_after;
            entry.last_event = event;
            entry.last_event_at = at_ms;
            if cwd.is_some() {
                entry.cwd = cwd;
            }
            entry.detail = detail;
        }

        self.updated_at = self.updated_at.max(at_ms);
        Applied {
            session_before,
            session_after,
            aggregate_before,
            aggregate_after: self.aggregate(),
        }
    }

    /// The session that drives the aggregate: highest priority, most recent on ties.
    pub fn focus_session(&self) -> Option<&SessionInfo> {
        self.sessions.values().max_by(|a, b| {
            a.state
                .priority()
                .cmp(&b.state.priority())
                .then(a.last_event_at.cmp(&b.last_event_at))
        })
    }

    pub fn aggregate(&self) -> ClaudeState {
        self.focus_session().map(|s| s.state).unwrap_or_default()
    }

    pub fn session(&self, id: &str) -> Option<&SessionInfo> {
        self.sessions.get(id)
    }

    pub fn sessions(&self) -> impl Iterator<Item = &SessionInfo> {
        self.sessions.values()
    }

    /// Drop sessions silent for longer than `max_idle_ms` (a crashed Claude Code
    /// never sends SessionEnd). Returns how many were removed.
    pub fn prune(&mut self, now_ms: u64, max_idle_ms: u64) -> usize {
        let before = self.sessions.len();
        self.sessions
            .retain(|_, s| now_ms.saturating_sub(s.last_event_at) <= max_idle_ms);
        before - self.sessions.len()
    }

    pub fn clear(&mut self) {
        self.sessions.clear();
    }

    pub fn snapshot(&self) -> ActivitySnapshot {
        let focus = self.focus_session();
        ActivitySnapshot {
            state: focus.map(|s| s.state).unwrap_or_default(),
            reason: focus.and_then(|s| s.detail.clone()),
            focus_session_id: focus.map(|s| s.session_id.clone()),
            sessions: self.sessions.values().cloned().collect(),
            source: self.source,
            updated_at: self.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ActivityEvent::*;
    use ClaudeState::*;

    fn obs(session: &str, event: ActivityEvent, at: u64) -> Observation {
        Observation::new(session, event, at)
    }

    #[test]
    fn single_session_walks_the_prd_diagram() {
        let mut store = ActivityStore::new(ActivitySource::Hooks);
        assert_eq!(store.aggregate(), Idle);

        let a = store.apply(obs("s1", PromptSubmitted, 1));
        assert_eq!((a.session_after, a.aggregate_after), (Working, Working));
        assert!(a.aggregate_changed());

        store.apply(obs("s1", ToolRunning, 2));
        assert_eq!(store.aggregate(), Working);

        store.apply(obs("s1", WaitingForInput, 3));
        assert_eq!(store.aggregate(), Waiting);

        store.apply(obs("s1", OutputReceived, 4));
        assert_eq!(store.aggregate(), Working);

        let done = store.apply(obs("s1", TaskCompleted, 5));
        assert_eq!(done.aggregate_after, Done);

        let again = store.apply(obs("s1", TaskCompleted, 6));
        assert!(!again.aggregate_changed());
    }

    #[test]
    fn waiting_outranks_working_across_sessions() {
        let mut store = ActivityStore::new(ActivitySource::Hooks);
        store.apply(obs("a", PromptSubmitted, 10));
        store.apply(obs("b", PromptSubmitted, 11));
        store.apply(obs("b", WaitingForInput, 12));
        assert_eq!(store.aggregate(), Waiting);
        assert_eq!(store.focus_session().unwrap().session_id, "b");

        // b gets answered; a is still working
        store.apply(obs("b", OutputReceived, 13));
        assert_eq!(store.aggregate(), Working);
    }

    #[test]
    fn ties_go_to_the_most_recent_session() {
        let mut store = ActivityStore::new(ActivitySource::Hooks);
        store.apply(obs("a", PromptSubmitted, 10));
        store.apply(obs("b", PromptSubmitted, 20));
        assert_eq!(store.focus_session().unwrap().session_id, "b");
    }

    #[test]
    fn process_exit_removes_the_session_and_falls_back() {
        let mut store = ActivityStore::new(ActivitySource::Hooks);
        store.apply(obs("a", PromptSubmitted, 1));
        store.apply(obs("b", WaitingForInput, 2));
        assert_eq!(store.aggregate(), Waiting);

        let applied = store.apply(obs("b", ProcessExited, 3));
        assert_eq!(applied.aggregate_after, Working);
        assert_eq!(store.len(), 1);
        assert!(store.session("b").is_none());

        store.apply(obs("a", ProcessExited, 4));
        assert!(store.is_empty());
        assert_eq!(store.aggregate(), Idle);
    }

    #[test]
    fn keeps_cwd_across_events_that_omit_it() {
        let mut store = ActivityStore::new(ActivitySource::Hooks);
        store.apply(obs("a", ProcessStarted, 1).with_cwd(Some("C:\\work\\app".into())));
        store.apply(obs("a", PromptSubmitted, 2));
        assert_eq!(
            store.session("a").unwrap().cwd.as_deref(),
            Some("C:\\work\\app")
        );
    }

    #[test]
    fn prune_drops_silent_sessions() {
        let mut store = ActivityStore::new(ActivitySource::Hooks);
        store.apply(obs("old", PromptSubmitted, 1_000));
        store.apply(obs("new", PromptSubmitted, 5_000));
        assert_eq!(store.prune(6_000, 2_000), 1);
        assert!(store.session("old").is_none());
        assert!(store.session("new").is_some());
    }

    #[test]
    fn snapshot_reports_reason_and_focus() {
        let mut store = ActivityStore::new(ActivitySource::Hooks);
        store.apply(obs("a", PromptSubmitted, 1));
        store
            .apply(obs("a", WaitingForInput, 2).with_detail(Some("Needs permission: Bash".into())));
        let snap = store.snapshot();
        assert_eq!(snap.state, Waiting);
        assert_eq!(snap.reason.as_deref(), Some("Needs permission: Bash"));
        assert_eq!(snap.focus_session_id.as_deref(), Some("a"));
        assert_eq!(snap.sessions.len(), 1);
        assert_eq!(snap.source, ActivitySource::Hooks);
        assert_eq!(snap.updated_at, 2);

        let json = serde_json::to_value(&snap).unwrap();
        assert_eq!(json["focusSessionId"], "a");
        assert_eq!(json["sessions"][0]["lastEvent"], "waiting_for_input");
    }

    #[test]
    fn empty_store_snapshot_is_idle() {
        let snap = ActivityStore::new(ActivitySource::None).snapshot();
        assert_eq!(snap.state, Idle);
        assert!(snap.focus_session_id.is_none());
        assert!(snap.sessions.is_empty());
    }
}
