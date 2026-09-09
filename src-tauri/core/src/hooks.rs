//! Claude Code → Claude Notch signal mapping (PRD §14, §19 phase 3, §21).
//!
//! Claude Code exposes lifecycle *hooks*: shell commands it runs at well-defined
//! moments, passing a JSON payload on stdin. Claude Notch subscribes to the
//! events in [`SUBSCRIBED_EVENTS`]; the hook command simply forwards that JSON
//! to the local listener (`POST http://127.0.0.1:<port>/hook`). This module
//! turns a payload into an [`Observation`] and knows how to (un)install the
//! hooks in `~/.claude/settings.json` without disturbing other hooks.
//!
//! Only documented payload fields are read, and every field is optional: an
//! unexpected shape degrades to "no observation", never to a crash.

use serde_json::{Map, Value, json};

use crate::state::{ActivityEvent, Observation};

pub const DEFAULT_PORT: u16 = 47831;

/// Trailing shell comment that marks our hook commands so they can be found and replaced.
pub const HOOK_MARKER: &str = "# claude-notch";

/// Claude Code hook events Claude Notch subscribes to.
pub const SUBSCRIBED_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

/// The subset of a hook payload we act on. Everything is optional by design.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HookPayload {
    pub session_id: Option<String>,
    pub hook_event_name: String,
    pub cwd: Option<String>,
    pub tool_name: Option<String>,
    /// Notification: permission_prompt | idle_prompt | auth_success | elicitation_dialog
    pub notification_type: Option<String>,
    pub message: Option<String>,
    /// SessionStart: startup | resume | clear | compact
    pub source: Option<String>,
    /// SessionEnd reason
    pub reason: Option<String>,
    /// StopFailure error, stringified whatever its shape.
    pub error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("invalid hook JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("hook payload is not a JSON object")]
    NotAnObject,
    #[error("hook payload has no session_id")]
    MissingSession,
}

impl HookPayload {
    pub fn parse(json: &str) -> Result<Self, HookError> {
        let value: Value = serde_json::from_str(json)?;
        if !value.is_object() {
            return Err(HookError::NotAnObject);
        }
        Ok(Self::from_value(&value))
    }

    /// Lenient extraction: non-string fields are treated as absent.
    pub fn from_value(value: &Value) -> Self {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
        Self {
            session_id: text("session_id"),
            hook_event_name: text("hook_event_name").unwrap_or_default(),
            cwd: text("cwd"),
            tool_name: text("tool_name"),
            notification_type: text("notification_type"),
            message: text("message"),
            source: text("source"),
            reason: text("reason"),
            error: value.get("error").map(|e| match e {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            }),
        }
    }
}

/// Map a hook event onto an activity event. `None` = observed but not state-relevant.
pub fn classify(p: &HookPayload) -> Option<ActivityEvent> {
    use ActivityEvent as E;
    match p.hook_event_name.as_str() {
        // Compaction happens mid-turn; it must not reset the session to Idle.
        "SessionStart" => Some(if p.source.as_deref() == Some("compact") {
            E::OutputReceived
        } else {
            E::ProcessStarted
        }),
        "UserPromptSubmit" => Some(E::PromptSubmitted),
        "PreToolUse" => Some(E::ToolRunning),
        // A failing tool is routine for Claude (it reads the error and carries on);
        // it is not a product-level error.
        "PostToolUse" | "PostToolUseFailure" | "SubagentStart" | "SubagentStop" | "PreCompact"
        | "PostCompact" => Some(E::OutputReceived),
        "PermissionRequest" => Some(E::WaitingForInput),
        "Notification" => match p.notification_type.as_deref() {
            Some("permission_prompt") | Some("elicitation_dialog") => Some(E::WaitingForInput),
            // idle_prompt fires ~60 s after a turn ended — Done already says that.
            // auth_success is noise.
            Some(_) => None,
            None => p
                .message
                .as_deref()
                .filter(|m| m.to_ascii_lowercase().contains("permission"))
                .map(|_| E::WaitingForInput),
        },
        "Stop" => Some(E::TaskCompleted),
        "StopFailure" => Some(E::ErrorDetected),
        "SessionEnd" => Some(E::ProcessExited),
        _ => None,
    }
}

/// Human-readable context for the click card (PRD §9).
pub fn detail_for(p: &HookPayload, event: ActivityEvent) -> Option<String> {
    use ActivityEvent as E;
    match event {
        E::WaitingForInput => Some(match (&p.tool_name, &p.message) {
            (Some(tool), _) => format!("Needs permission: {tool}"),
            (None, Some(msg)) => msg.clone(),
            (None, None) => "Waiting for you".to_string(),
        }),
        E::ToolRunning => p.tool_name.as_ref().map(|t| format!("Running {t}")),
        E::ErrorDetected => Some(
            p.error
                .clone()
                .unwrap_or_else(|| "Claude Code reported an error".to_string()),
        ),
        E::TaskCompleted => Some("Task completed".to_string()),
        E::PromptSubmitted => Some("Working on your request".to_string()),
        E::ProcessStarted => Some("Session started".to_string()),
        E::OutputReceived | E::ProcessExited => None,
    }
}

/// Parse one hook payload into an observation (or `None` if it is not state-relevant).
pub fn observation_from_hook(json: &str, now_ms: u64) -> Result<Option<Observation>, HookError> {
    let payload = HookPayload::parse(json)?;
    let session_id = payload
        .session_id
        .clone()
        .ok_or(HookError::MissingSession)?;
    Ok(classify(&payload).map(|event| {
        Observation::new(session_id, event, now_ms)
            .with_cwd(payload.cwd.clone())
            .with_detail(detail_for(&payload, event))
    }))
}

/// The shell command installed as every hook. Uses `curl`, which ships with
/// Windows 10+ and Git for Windows, so no helper binary is needed. Never fails
/// the hook if Claude Notch is not running (`|| true`).
pub fn hook_command(port: u16) -> String {
    format!(
        "curl -s -m 2 -X POST -H \"Content-Type: application/json\" --data-binary @- \
         http://127.0.0.1:{port}/hook >/dev/null 2>&1 || true {HOOK_MARKER}"
    )
}

fn is_our_hook(entry: &Value) -> bool {
    entry
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(HOOK_MARKER))
}

/// Remove every Claude Notch hook, leaving other hooks untouched. Returns true if anything changed.
pub fn remove_hooks(settings: &mut Value) -> bool {
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return false;
    };
    let mut changed = false;
    let mut empty_events = Vec::new();

    for (event, groups) in hooks.iter_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        let before = groups.len();
        for group in groups.iter_mut() {
            if let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                let n = list.len();
                list.retain(|h| !is_our_hook(h));
                if list.len() != n {
                    changed = true;
                }
            }
        }
        groups.retain(|g| {
            g.get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|l| !l.is_empty())
        });
        if groups.len() != before {
            changed = true;
        }
        if groups.is_empty() {
            empty_events.push(event.clone());
        }
    }

    for event in empty_events {
        hooks.remove(&event);
        changed = true;
    }
    if hooks.is_empty() {
        if let Some(obj) = settings.as_object_mut() {
            obj.remove("hooks");
        }
    }
    changed
}

/// Install (or re-install for a new port) the Claude Notch hooks. Existing
/// foreign hooks and all other settings are preserved. Returns true if the
/// document changed.
pub fn install_hooks(settings: &mut Value, port: u16) -> bool {
    if !settings.is_object() {
        *settings = Value::Object(Map::new());
    }
    let before = settings.clone();
    remove_hooks(settings);

    let obj = settings.as_object_mut().expect("settings is an object");
    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    if !hooks.is_object() {
        *hooks = Value::Object(Map::new());
    }
    let hooks = hooks.as_object_mut().expect("hooks is an object");

    let entry = json!({
        "hooks": [{ "type": "command", "command": hook_command(port), "timeout": 5 }]
    });
    for event in SUBSCRIBED_EVENTS {
        let groups = hooks
            .entry(*event)
            .or_insert_with(|| Value::Array(Vec::new()));
        if !groups.is_array() {
            *groups = Value::Array(Vec::new());
        }
        groups
            .as_array_mut()
            .expect("groups is an array")
            .push(entry.clone());
    }

    *settings != before
}

/// True when every subscribed event forwards to our listener on `port`.
pub fn hooks_installed(settings: &Value, port: u16) -> bool {
    let needle = format!("127.0.0.1:{port}/hook");
    let Some(hooks) = settings.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    SUBSCRIBED_EVENTS.iter().all(|event| {
        hooks
            .get(*event)
            .and_then(Value::as_array)
            .is_some_and(|groups| {
                groups.iter().any(|g| {
                    g.get("hooks")
                        .and_then(Value::as_array)
                        .is_some_and(|list| {
                            list.iter().any(|h| {
                                is_our_hook(h)
                                    && h.get("command")
                                        .and_then(Value::as_str)
                                        .is_some_and(|c| c.contains(&needle))
                            })
                        })
                })
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ActivityEvent::*;

    fn payload(event: &str, extra: Value) -> String {
        let mut v = json!({
            "session_id": "abc",
            "transcript_path": "C:\\Users\\me\\.claude\\projects\\x\\abc.jsonl",
            "cwd": "C:\\work\\app",
            "hook_event_name": event,
            "permission_mode": "default"
        });
        if let (Some(target), Some(source)) = (v.as_object_mut(), extra.as_object()) {
            for (k, val) in source {
                target.insert(k.clone(), val.clone());
            }
        }
        v.to_string()
    }

    fn classify_str(event: &str, extra: Value) -> Option<ActivityEvent> {
        classify(&HookPayload::parse(&payload(event, extra)).unwrap())
    }

    #[test]
    fn maps_lifecycle_events() {
        assert_eq!(
            classify_str("SessionStart", json!({"source": "startup"})),
            Some(ProcessStarted)
        );
        assert_eq!(
            classify_str("SessionStart", json!({"source": "resume"})),
            Some(ProcessStarted)
        );
        assert_eq!(
            classify_str("SessionStart", json!({"source": "compact"})),
            Some(OutputReceived)
        );
        assert_eq!(
            classify_str("SessionEnd", json!({"reason": "exit"})),
            Some(ProcessExited)
        );
    }

    #[test]
    fn maps_work_events() {
        assert_eq!(
            classify_str("UserPromptSubmit", json!({})),
            Some(PromptSubmitted)
        );
        assert_eq!(
            classify_str("PreToolUse", json!({"tool_name": "Bash"})),
            Some(ToolRunning)
        );
        assert_eq!(
            classify_str("PostToolUse", json!({"tool_name": "Bash"})),
            Some(OutputReceived)
        );
        assert_eq!(
            classify_str("PostToolUseFailure", json!({"tool_name": "Bash"})),
            Some(OutputReceived)
        );
        assert_eq!(
            classify_str("Stop", json!({"stop_hook_active": false})),
            Some(TaskCompleted)
        );
        assert_eq!(classify_str("StopFailure", json!({})), Some(ErrorDetected));
    }

    #[test]
    fn maps_attention_events() {
        assert_eq!(
            classify_str("PermissionRequest", json!({"tool_name": "Edit"})),
            Some(WaitingForInput)
        );
        assert_eq!(
            classify_str(
                "Notification",
                json!({"notification_type": "permission_prompt", "message": "..."})
            ),
            Some(WaitingForInput)
        );
        assert_eq!(
            classify_str(
                "Notification",
                json!({"notification_type": "elicitation_dialog"})
            ),
            Some(WaitingForInput)
        );
        assert_eq!(
            classify_str("Notification", json!({"notification_type": "idle_prompt"})),
            None
        );
        assert_eq!(
            classify_str("Notification", json!({"notification_type": "auth_success"})),
            None
        );
        // Older payloads without a type: fall back to the message text.
        assert_eq!(
            classify_str(
                "Notification",
                json!({"message": "Claude needs your permission to use Bash"})
            ),
            Some(WaitingForInput)
        );
        assert_eq!(
            classify_str("Notification", json!({"message": "Something else"})),
            None
        );
    }

    #[test]
    fn ignores_unknown_events() {
        assert_eq!(classify_str("SomethingNew", json!({})), None);
        assert_eq!(classify_str("", json!({})), None);
    }

    #[test]
    fn builds_observations_with_context() {
        let obs = observation_from_hook(
            &payload("PermissionRequest", json!({"tool_name": "Edit"})),
            42,
        )
        .unwrap()
        .expect("state-relevant");
        assert_eq!(obs.session_id, "abc");
        assert_eq!(obs.event, WaitingForInput);
        assert_eq!(obs.at_ms, 42);
        assert_eq!(obs.cwd.as_deref(), Some("C:\\work\\app"));
        assert_eq!(obs.detail.as_deref(), Some("Needs permission: Edit"));

        let err = observation_from_hook(
            &payload(
                "StopFailure",
                json!({"error": {"type": "api_error", "message": "overloaded"}}),
            ),
            1,
        )
        .unwrap()
        .unwrap();
        assert_eq!(err.event, ErrorDetected);
        assert!(err.detail.unwrap().contains("overloaded"));

        let quiet = observation_from_hook(
            &payload("Notification", json!({"notification_type": "idle_prompt"})),
            1,
        )
        .unwrap();
        assert!(quiet.is_none());
    }

    #[test]
    fn rejects_unusable_payloads() {
        assert!(matches!(
            observation_from_hook(r#"{"hook_event_name":"Stop"}"#, 0),
            Err(HookError::MissingSession)
        ));
        assert!(matches!(
            observation_from_hook("not json", 0),
            Err(HookError::Json(_))
        ));
        assert!(matches!(
            observation_from_hook("[]", 0),
            Err(HookError::NotAnObject)
        ));
    }

    #[test]
    fn non_string_fields_do_not_break_parsing() {
        let p = HookPayload::parse(&payload("PreToolUse", json!({"tool_name": 5, "cwd": null})))
            .unwrap();
        assert_eq!(p.tool_name, None);
        assert_eq!(p.cwd, None);
        assert_eq!(classify(&p), Some(ToolRunning));
    }

    #[test]
    fn installs_hooks_into_empty_settings() {
        let mut settings = json!({});
        assert!(install_hooks(&mut settings, 47831));
        assert!(hooks_installed(&settings, 47831));
        assert!(!hooks_installed(&settings, 1));
        for event in SUBSCRIBED_EVENTS {
            assert!(settings["hooks"][*event].is_array(), "{event} missing");
        }
        let cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(cmd.contains("127.0.0.1:47831/hook"));
        assert!(cmd.ends_with(HOOK_MARKER));
        assert_eq!(settings["hooks"]["Stop"][0]["hooks"][0]["timeout"], 5);
    }

    #[test]
    fn install_handles_null_and_empty_documents() {
        let mut settings = Value::Null;
        assert!(install_hooks(&mut settings, 47831));
        assert!(hooks_installed(&settings, 47831));
    }

    fn settings_with_foreign_hooks() -> Value {
        json!({
            "model": "opus",
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "command", "command": "echo done" }] }],
                "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "my-linter" }] }]
            }
        })
    }

    #[test]
    fn install_is_idempotent_and_preserves_other_hooks() {
        let mut settings = settings_with_foreign_hooks();
        assert!(install_hooks(&mut settings, 47831));
        assert!(
            !install_hooks(&mut settings, 47831),
            "second install must be a no-op"
        );

        assert_eq!(settings["model"], "opus");
        let stop = settings["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0]["hooks"][0]["command"], "echo done");
        let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert_eq!(pre[0]["matcher"], "Bash");
    }

    #[test]
    fn changing_the_port_replaces_our_entries() {
        let mut settings = json!({});
        install_hooks(&mut settings, 47831);
        assert!(install_hooks(&mut settings, 5000));
        assert!(hooks_installed(&settings, 5000));
        assert!(!hooks_installed(&settings, 47831));
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn remove_restores_foreign_hooks_exactly() {
        let original = settings_with_foreign_hooks();
        let mut settings = original.clone();
        install_hooks(&mut settings, 47831);
        assert!(remove_hooks(&mut settings));
        assert_eq!(settings, original);
        assert!(!remove_hooks(&mut settings), "nothing left to remove");
    }

    #[test]
    fn remove_drops_the_hooks_key_when_only_ours_existed() {
        let mut settings = json!({"model": "opus"});
        install_hooks(&mut settings, 47831);
        assert!(remove_hooks(&mut settings));
        assert_eq!(settings, json!({"model": "opus"}));
    }

    #[test]
    fn hook_command_is_a_safe_forwarder() {
        let cmd = hook_command(1234);
        assert!(cmd.starts_with("curl "));
        assert!(cmd.contains("--data-binary @-"));
        assert!(cmd.contains("http://127.0.0.1:1234/hook"));
        assert!(cmd.contains("|| true"));
        assert!(cmd.ends_with(HOOK_MARKER));
    }
}
