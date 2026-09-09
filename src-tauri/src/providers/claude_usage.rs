//! Claude Usage Provider (PRD §13, §16, phase 4).
//!
//! Polls Anthropic's account usage endpoint with Claude Code's own OAuth token.
//! Parsing, staleness and backoff policy live in `core::usage`; this file only
//! does the I/O. The token is read from disk for each request and dropped
//! immediately afterwards — never logged, never persisted (PRD §20).

use std::time::Duration;

use claude_notch_core::time::now_ms;
use claude_notch_core::usage::{
    FetchOutcome, OAUTH_BETA_HEADER, PollPolicy, USAGE_ENDPOINT, UsageSnapshot, credentials_path,
    parse_credentials, parse_usage_response,
};
use tauri::{AppHandle, Manager};

use crate::state::AppState;

struct FetchError {
    outcome: FetchOutcome,
    message: String,
}

impl FetchError {
    fn failure(message: impl Into<String>) -> Self {
        Self {
            outcome: FetchOutcome::Failure,
            message: message.into(),
        }
    }
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(poll_loop(app));
}

async fn poll_loop(app: AppHandle) {
    let client = match reqwest::Client::builder()
        .user_agent(concat!("claude-notch/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            log::error!("cannot build HTTP client for usage polling: {err}");
            return;
        }
    };

    let policy = PollPolicy::default();
    let mut failures: u32 = 0;

    loop {
        let state = app.state::<AppState>();
        if state.is_demo() {
            // Demo mode supplies its own reading; don't touch the network.
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }

        let delay = match fetch_once(&client).await {
            Ok(snapshot) => {
                failures = 0;
                log::info!(
                    "usage: {:?}% of {} window",
                    snapshot.percent,
                    snapshot.window_label
                );
                state.set_usage(snapshot);
                policy.next_delay(FetchOutcome::Success, 0)
            }
            Err(err) => {
                failures += 1;
                log::warn!("usage fetch failed ({failures}x): {}", err.message);
                let previous = state.last_usage();
                state.set_usage(UsageSnapshot::degraded(&previous, err.message));
                policy.next_delay(err.outcome, failures)
            }
        };
        tokio::time::sleep(delay).await;
    }
}

async fn fetch_once(client: &reqwest::Client) -> Result<UsageSnapshot, FetchError> {
    let path = credentials_path()
        .ok_or_else(|| FetchError::failure("Cannot resolve the Claude Code config directory"))?;
    let raw = tokio::fs::read_to_string(&path).await.map_err(|e| {
        FetchError::failure(format!(
            "Claude Code credentials not found ({}): {e}",
            path.display()
        ))
    })?;
    let token = parse_credentials(&raw).map_err(|e| FetchError {
        outcome: FetchOutcome::Unauthorized,
        message: e.to_string(),
    })?;

    let response = client
        .get(USAGE_ENDPOINT)
        .bearer_auth(&token.token)
        .header("anthropic-beta", OAUTH_BETA_HEADER)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| FetchError::failure(format!("network error: {e}")))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(FetchError {
            outcome: FetchOutcome::Unauthorized,
            message: format!(
                "usage endpoint rejected the Claude Code token ({status}); it refreshes when you next use Claude Code"
            ),
        });
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        return Err(FetchError {
            outcome: FetchOutcome::RateLimited { retry_after },
            message: "usage endpoint is rate limiting requests".into(),
        });
    }
    if !status.is_success() {
        return Err(FetchError::failure(format!(
            "usage endpoint returned HTTP {status}"
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| FetchError::failure(format!("cannot read usage response: {e}")))?;
    let mut snapshot = parse_usage_response(&body, now_ms())
        .map_err(|e| FetchError::failure(format!("unexpected usage response: {e}")))?;
    snapshot.account = token.account_label();
    Ok(snapshot)
}
