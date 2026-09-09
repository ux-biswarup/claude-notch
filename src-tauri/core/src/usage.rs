//! Claude usage — the pure half of the usage provider (PRD §13, §16.2–16.5).
//! The HTTP call itself lives in the app crate (`src/providers/claude_usage.rs`).
//!
//! **Source.** Claude Code's own OAuth session. On Windows Claude Code stores it
//! at `%USERPROFILE%\.claude\.credentials.json` (`claudeAiOauth.accessToken`).
//! Claude Notch reads the token into memory for each request and never copies,
//! logs or persists it (PRD §20: "no credentials stored unnecessarily").
//!
//! **Endpoint.** The same account endpoint Codenotch and similar tools use:
//!
//! ```text
//! GET https://api.anthropic.com/api/oauth/usage
//! Authorization: Bearer <accessToken>
//! anthropic-beta: oauth-2025-04-20
//! ```
//!
//! Observed response shape (validated in phase 4 — treat as provisional):
//!
//! ```json
//! { "five_hour": { "utilization": 72.0, "resets_at": "2026-09-09T14:00:00Z" },
//!   "seven_day": { "utilization": 30.5, "resets_at": "2026-09-12T00:00:00Z" } }
//! ```
//!
//! The parser is lenient: any top-level object with a numeric `utilization`
//! becomes a window, known windows first.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::snapshot::Fidelity;

pub const USAGE_ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";
pub const OAUTH_BETA_HEADER: &str = "oauth-2025-04-20";
pub const CREDENTIALS_FILE: &str = ".credentials.json";

/// Known rate-limit windows in display order; the first present one is primary.
const KNOWN_WINDOWS: &[(&str, &str)] = &[
    ("five_hour", "5h"),
    ("seven_day", "7d"),
    ("seven_day_opus", "7d Opus"),
    ("seven_day_sonnet", "7d Sonnet"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UsageStatus {
    Ok,
    /// We have a number, but it is older than we are comfortable with.
    Stale,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub id: String,
    pub label: String,
    /// 0–100
    pub percent: f64,
    pub resets_at: Option<u64>,
}

/// The usage half of the UI snapshot. Mirrors `UsageSnapshot` in `src/state/State.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub status: UsageStatus,
    /// Primary window, 0–100.
    pub percent: Option<f64>,
    pub resets_at: Option<u64>,
    pub window_label: String,
    pub fidelity: Fidelity,
    /// e.g. "Max", "Pro" — from the subscription type, never an e-mail.
    pub account: Option<String>,
    pub fetched_at: Option<u64>,
    pub error: Option<String>,
    pub windows: Vec<UsageWindow>,
}

impl Default for UsageSnapshot {
    fn default() -> Self {
        Self::unavailable("No usage reading yet")
    }
}

impl UsageSnapshot {
    pub fn unavailable(error: impl Into<String>) -> Self {
        Self {
            status: UsageStatus::Unavailable,
            percent: None,
            resets_at: None,
            window_label: "5h".into(),
            fidelity: Fidelity::Manual,
            account: None,
            fetched_at: None,
            error: Some(error.into()),
            windows: Vec::new(),
        }
    }

    /// Re-evaluate freshness at read time. A good reading older than
    /// `stale_after_ms` keeps its number but is marked stale (PRD §16.4: "72% · stale").
    pub fn with_freshness(mut self, now_ms: u64, stale_after_ms: u64) -> Self {
        if self.status == UsageStatus::Ok {
            if let Some(fetched) = self.fetched_at {
                if now_ms.saturating_sub(fetched) > stale_after_ms {
                    self.status = UsageStatus::Stale;
                }
            }
        }
        self
    }

    /// A fetch failed: keep the last good number (marked stale) and record why.
    pub fn degraded(previous: &UsageSnapshot, error: impl Into<String>) -> Self {
        if previous.percent.is_some() {
            Self {
                status: UsageStatus::Stale,
                error: Some(error.into()),
                ..previous.clone()
            }
        } else {
            Self::unavailable(error)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UsageError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("credentials file has no claudeAiOauth.accessToken — sign in to Claude Code first")]
    MissingToken,
    #[error("usage response is not a JSON object")]
    NotAnObject,
    #[error("usage response contained no rate-limit windows")]
    NoWindows,
}

/// Claude Code's OAuth access token. `Debug` redacts the secret.
pub struct AccessToken {
    pub token: String,
    pub expires_at_ms: Option<u64>,
    pub subscription: Option<String>,
}

impl AccessToken {
    pub fn is_expired(&self, now_ms: u64) -> bool {
        self.expires_at_ms.is_some_and(|e| e <= now_ms)
    }

    /// "max" → "Max". Never exposes anything identifying.
    pub fn account_label(&self) -> Option<String> {
        self.subscription
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(capitalise)
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccessToken")
            .field("token", &"<redacted>")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("subscription", &self.subscription)
            .finish()
    }
}

#[derive(Deserialize)]
struct CredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OauthBlock>,
}

#[derive(Deserialize)]
struct OauthBlock {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<u64>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
}

pub fn parse_credentials(json: &str) -> Result<AccessToken, UsageError> {
    let file: CredentialsFile = serde_json::from_str(json)?;
    let block = file.claude_ai_oauth.ok_or(UsageError::MissingToken)?;
    let token = block
        .access_token
        .filter(|t| !t.is_empty())
        .ok_or(UsageError::MissingToken)?;
    Ok(AccessToken {
        token,
        expires_at_ms: block.expires_at,
        subscription: block.subscription_type,
    })
}

/// `%CLAUDE_CONFIG_DIR%`, else `%USERPROFILE%\.claude` (or `$HOME/.claude`).
pub fn claude_config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|home| PathBuf::from(home).join(".claude"))
}

pub fn credentials_path() -> Option<PathBuf> {
    claude_config_dir().map(|d| d.join(CREDENTIALS_FILE))
}

pub fn settings_path() -> Option<PathBuf> {
    claude_config_dir().map(|d| d.join("settings.json"))
}

/// Parse the usage endpoint's response into a snapshot with `Official` fidelity.
pub fn parse_usage_response(json: &str, now_ms: u64) -> Result<UsageSnapshot, UsageError> {
    let value: Value = serde_json::from_str(json)?;
    let object = value.as_object().ok_or(UsageError::NotAnObject)?;

    let mut windows: Vec<UsageWindow> = Vec::new();
    let mut push = |id: &str, label: &str, entry: &Value| {
        if let Some(percent) = entry.get("utilization").and_then(Value::as_f64) {
            windows.push(UsageWindow {
                id: id.to_string(),
                label: label.to_string(),
                percent: percent.clamp(0.0, 100.0),
                resets_at: entry
                    .get("resets_at")
                    .and_then(Value::as_str)
                    .and_then(parse_rfc3339_ms),
            });
        }
    };
    for (id, label) in KNOWN_WINDOWS {
        if let Some(entry) = object.get(*id) {
            push(id, label, entry);
        }
    }
    for (key, entry) in object {
        let known = KNOWN_WINDOWS.iter().any(|(id, _)| *id == key.as_str());
        if known || !entry.is_object() {
            continue;
        }
        push(key, &key.replace('_', " "), entry);
    }

    let primary = windows.first().cloned().ok_or(UsageError::NoWindows)?;
    Ok(UsageSnapshot {
        status: UsageStatus::Ok,
        percent: Some(primary.percent),
        resets_at: primary.resets_at,
        window_label: primary.label,
        fidelity: Fidelity::Official,
        account: None,
        fetched_at: Some(now_ms),
        error: None,
        windows,
    })
}

/// Parse an RFC 3339 timestamp (`2026-09-09T14:00:00Z`, `…00.123+02:00`) into
/// epoch milliseconds. Small on purpose: avoids pulling in chrono for one field.
pub fn parse_rfc3339_ms(s: &str) -> Option<u64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 20 {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> { s.get(from..to)?.parse::<i64>().ok() };

    if b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b't' | b' ')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let year = num(0, 4)?;
    let month = num(5, 7)?;
    let day = num(8, 10)?;
    let hour = num(11, 13)?;
    let minute = num(14, 16)?;
    let second = num(17, 19)?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }

    let mut idx = 19;
    let mut millis: i64 = 0;
    if b.get(idx) == Some(&b'.') {
        idx += 1;
        let start = idx;
        while idx < b.len() && b[idx].is_ascii_digit() {
            idx += 1;
        }
        let mut digits: String = s[start..idx].chars().take(3).collect();
        while digits.len() < 3 {
            digits.push('0');
        }
        millis = digits.parse().ok()?;
    }

    let offset_secs: i64 = match b.get(idx) {
        Some(b'Z') | Some(b'z') => 0,
        Some(sign @ (b'+' | b'-')) => {
            let hours = num(idx + 1, idx + 3)?;
            let minutes = num(idx + 4, idx + 6)?;
            let total = hours * 3600 + minutes * 60;
            if *sign == b'+' { total } else { -total }
        }
        _ => return None,
    };

    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hour * 3600 + minute * 60 + second - offset_secs;
    if secs < 0 {
        return None;
    }
    Some(secs as u64 * 1000 + millis as u64)
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Polling with backoff and rate-limit handling (PRD §16.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollPolicy {
    pub base: Duration,
    pub max: Duration,
}

impl Default for PollPolicy {
    fn default() -> Self {
        Self {
            base: Duration::from_secs(120),
            max: Duration::from_secs(15 * 60),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchOutcome {
    Success,
    RateLimited {
        retry_after: Option<Duration>,
    },
    /// Token rejected. Claude Code refreshes it when next used; wait the maximum.
    Unauthorized,
    /// Network / server / parse failure.
    Failure,
}

impl PollPolicy {
    pub fn next_delay(&self, outcome: FetchOutcome, consecutive_failures: u32) -> Duration {
        match outcome {
            FetchOutcome::Success => self.base,
            FetchOutcome::RateLimited {
                retry_after: Some(ra),
            } => ra.clamp(self.base, self.max),
            FetchOutcome::RateLimited { retry_after: None } => {
                self.backoff(consecutive_failures.max(2))
            }
            FetchOutcome::Unauthorized => self.max,
            FetchOutcome::Failure => self.backoff(consecutive_failures),
        }
    }

    /// Readings older than this are shown as stale.
    pub fn stale_after(&self) -> Duration {
        self.base * 3
    }

    fn backoff(&self, failures: u32) -> Duration {
        let multiplier = 2u32.saturating_pow(failures.min(8));
        (self.base * multiplier).min(self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREDENTIALS: &str = r#"{
        "claudeAiOauth": {
            "accessToken": "sk-ant-oat01-secret",
            "refreshToken": "sk-ant-ort01-secret",
            "expiresAt": 1757000000000,
            "scopes": ["user:inference", "user:profile"],
            "subscriptionType": "max"
        }
    }"#;

    #[test]
    fn reads_the_token_without_leaking_it() {
        let token = parse_credentials(CREDENTIALS).unwrap();
        assert_eq!(token.token, "sk-ant-oat01-secret");
        assert_eq!(token.expires_at_ms, Some(1_757_000_000_000));
        assert_eq!(token.account_label().as_deref(), Some("Max"));
        assert!(token.is_expired(1_757_000_000_001));
        assert!(!token.is_expired(1_757_000_000_000 - 1));
        let debug = format!("{token:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret"));
    }

    #[test]
    fn missing_or_empty_token_is_an_error() {
        assert!(matches!(
            parse_credentials("{}"),
            Err(UsageError::MissingToken)
        ));
        assert!(matches!(
            parse_credentials(r#"{"claudeAiOauth":{"accessToken":""}}"#),
            Err(UsageError::MissingToken)
        ));
        assert!(matches!(
            parse_credentials("nope"),
            Err(UsageError::Json(_))
        ));
    }

    const RESPONSE: &str = r#"{
        "five_hour": { "utilization": 72, "resets_at": "2026-09-09T14:00:00Z" },
        "seven_day": { "utilization": 30.5, "resets_at": "2026-09-12T00:00:00+00:00" },
        "seven_day_opus": { "utilization": null, "resets_at": null },
        "extra_window": { "utilization": 5 },
        "not_a_window": "ignored"
    }"#;

    #[test]
    fn parses_the_usage_response() {
        let snap = parse_usage_response(RESPONSE, 1_000).unwrap();
        assert_eq!(snap.status, UsageStatus::Ok);
        assert_eq!(snap.percent, Some(72.0));
        assert_eq!(snap.resets_at, Some(1_788_962_400_000));
        assert_eq!(snap.window_label, "5h");
        assert_eq!(snap.fidelity, Fidelity::Official);
        assert_eq!(snap.fetched_at, Some(1_000));
        let ids: Vec<&str> = snap.windows.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, vec!["five_hour", "seven_day", "extra_window"]);
        assert_eq!(snap.windows[2].label, "extra window");
    }

    #[test]
    fn clamps_and_rejects_bad_responses() {
        let snap = parse_usage_response(r#"{"five_hour":{"utilization":140}}"#, 0).unwrap();
        assert_eq!(snap.percent, Some(100.0));
        assert!(matches!(
            parse_usage_response("{}", 0),
            Err(UsageError::NoWindows)
        ));
        assert!(matches!(
            parse_usage_response("[]", 0),
            Err(UsageError::NotAnObject)
        ));
    }

    #[test]
    fn parses_rfc3339() {
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339_ms("2026-09-09T14:00:00Z"),
            Some(1_788_962_400_000)
        );
        assert_eq!(
            parse_rfc3339_ms("2026-09-09T16:00:00+02:00"),
            Some(1_788_962_400_000)
        );
        assert_eq!(
            parse_rfc3339_ms("2026-09-09T12:30:00-01:30"),
            Some(1_788_962_400_000)
        );
        assert_eq!(
            parse_rfc3339_ms("2026-09-09T14:00:00.5Z"),
            Some(1_788_962_400_500)
        );
        assert_eq!(
            parse_rfc3339_ms("2026-09-09T14:00:00.123456Z"),
            Some(1_788_962_400_123)
        );
        assert_eq!(parse_rfc3339_ms("garbage"), None);
        assert_eq!(parse_rfc3339_ms("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339_ms("2026-09-09T14:00:00"), None);
    }

    #[test]
    fn freshness_and_degradation_never_pretend_certainty() {
        let good = parse_usage_response(RESPONSE, 0).unwrap();
        assert_eq!(
            good.clone().with_freshness(400, 500).status,
            UsageStatus::Ok
        );
        let stale = good.clone().with_freshness(1_000, 500);
        assert_eq!(stale.status, UsageStatus::Stale);
        assert_eq!(stale.percent, Some(72.0), "stale keeps the number");

        let degraded = UsageSnapshot::degraded(&good, "network down");
        assert_eq!(degraded.status, UsageStatus::Stale);
        assert_eq!(degraded.percent, Some(72.0));
        assert_eq!(degraded.error.as_deref(), Some("network down"));

        let nothing = UsageSnapshot::degraded(&UsageSnapshot::default(), "still down");
        assert_eq!(nothing.status, UsageStatus::Unavailable);
        assert_eq!(nothing.percent, None);

        let unavailable = UsageSnapshot::unavailable("x").with_freshness(u64::MAX, 0);
        assert_eq!(unavailable.status, UsageStatus::Unavailable);
    }

    #[test]
    fn poll_policy_backs_off_sensibly() {
        let policy = PollPolicy {
            base: Duration::from_secs(60),
            max: Duration::from_secs(600),
        };
        assert_eq!(
            policy.next_delay(FetchOutcome::Success, 5),
            Duration::from_secs(60)
        );
        assert_eq!(
            policy.next_delay(FetchOutcome::Failure, 1),
            Duration::from_secs(120)
        );
        assert_eq!(
            policy.next_delay(FetchOutcome::Failure, 3),
            Duration::from_secs(480)
        );
        assert_eq!(
            policy.next_delay(FetchOutcome::Failure, 10),
            Duration::from_secs(600)
        );
        assert_eq!(
            policy.next_delay(
                FetchOutcome::RateLimited {
                    retry_after: Some(Duration::from_secs(5))
                },
                0
            ),
            Duration::from_secs(60)
        );
        assert_eq!(
            policy.next_delay(
                FetchOutcome::RateLimited {
                    retry_after: Some(Duration::from_secs(3_600))
                },
                0
            ),
            Duration::from_secs(600)
        );
        assert_eq!(
            policy.next_delay(FetchOutcome::RateLimited { retry_after: None }, 0),
            Duration::from_secs(240)
        );
        assert_eq!(
            policy.next_delay(FetchOutcome::Unauthorized, 0),
            Duration::from_secs(600)
        );
        assert_eq!(policy.stale_after(), Duration::from_secs(180));
    }

    #[test]
    fn snapshot_serialises_camel_case_for_the_ui() {
        let json = serde_json::to_value(UsageSnapshot::unavailable("nope")).unwrap();
        assert_eq!(json["status"], "unavailable");
        assert_eq!(json["windowLabel"], "5h");
        assert_eq!(json["fetchedAt"], Value::Null);
        assert_eq!(json["fidelity"], "manual");
    }
}
