//! AI subsystem — Claude Messages API client over `reqwest::blocking`.
//!
//! There is no official Anthropic Rust SDK, so this is a thin raw-HTTP client
//! (technical-design.md §6; D14 models, D15 threading). It is deliberately
//! synchronous to match the rest of the backend (capture / STT / model_mgr are
//! all std-thread + blocking): live analysis (PR2) and chat (PR3) drive it from
//! dedicated threads, and the `test_api_key` command drives it via
//! `spawn_blocking`. PR1 wires the client, cost accounting, retries, and the
//! key-validation ping; the request builders for findings/chat land in PR2/PR3.

#![allow(dead_code)] // Built out across M3 PRs; some helpers land before their callers.

pub mod chat;
pub mod live;
pub mod prompts;

use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::error::{AppError, AppResult};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

/// Live-analysis model (fast/cheap) — verified current (D14). Bare id, no suffix.
pub const MODEL_HAIKU: &str = "claude-haiku-4-5";
/// Chat + post-analysis model (D14).
pub const MODEL_SONNET: &str = "claude-sonnet-4-6";

/// Max attempts for a retryable (429/5xx/529/transport) failure before giving up.
const MAX_ATTEMPTS: u32 = 4;

/// Default per-request timeout. Generous — chat streams can run long. The live
/// batcher overrides this with a short timeout so teardown can't stall.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Per-million-token USD rates `(input, output)` for a model.
fn rates(model: &str) -> (f64, f64) {
    match model {
        MODEL_SONNET => (3.00, 15.00),
        MODEL_HAIKU => (1.00, 5.00),
        _ => (1.00, 5.00),
    }
}

/// Token usage block returned by the Messages API.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

impl Usage {
    /// USD cost of this usage on `model`. Cache writes bill 1.25× input (5-min
    /// ephemeral), cache reads 0.10× input (prompt-caching economics).
    pub fn cost(&self, model: &str) -> f64 {
        let (in_rate, out_rate) = rates(model);
        (self.input_tokens as f64 * in_rate
            + self.output_tokens as f64 * out_rate
            + self.cache_creation_input_tokens as f64 * in_rate * 1.25
            + self.cache_read_input_tokens as f64 * in_rate * 0.10)
            / 1_000_000.0
    }
}

/// One content block of a Messages response (we only care about `text` blocks).
#[derive(Debug, Clone, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
}

/// A successful Messages API response.
#[derive(Debug, Clone, Deserialize)]
pub struct MessagesResponse {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub usage: Usage,
}

impl MessagesResponse {
    /// All `text` blocks concatenated.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter(|b| b.kind == "text")
            .map(|b| b.text.as_str())
            .collect()
    }
}

/// Result of consuming a streamed Messages response (PR3). `stop_reason` is the
/// terminal reason from the final `message_delta` (`end_turn`, `max_tokens`,
/// `refusal`, …), or `None` if the stream ended without one — a dropped /
/// incomplete connection. The caller decides how to present each case.
#[derive(Debug, Clone, Default)]
pub struct StreamOutcome {
    pub text: String,
    pub usage: Usage,
    pub stop_reason: Option<String>,
}

/// Per-attempt failure — enough to decide retry + map onto an `AppError`.
enum SendError {
    /// Non-2xx HTTP status (with any `retry-after` and the body for diagnostics).
    Status {
        code: u16,
        retry_after: Option<u64>,
        body: String,
    },
    /// Transport-level failure (connection, timeout, decode).
    Transport(String),
}

/// A thin Claude Messages API client. Holds the resolved key + an HTTP client.
pub struct ClaudeClient {
    http: reqwest::blocking::Client,
    api_key: String,
}

impl ClaudeClient {
    /// Build a client around an explicit key (e.g. the one being tested).
    pub fn new(api_key: impl Into<String>) -> AppResult<Self> {
        Self::with_timeout(api_key, DEFAULT_TIMEOUT)
    }

    /// Build a client with an explicit per-request timeout. The live batcher uses
    /// a short one so a stuck socket can't stall session teardown (see
    /// [`messages_cancellable`](Self::messages_cancellable)).
    pub fn with_timeout(api_key: impl Into<String>, timeout: Duration) -> AppResult<Self> {
        let api_key = api_key.into().trim().to_string();
        if api_key.is_empty() {
            return Err(AppError::Auth("API key is empty".into()));
        }
        let http = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| AppError::Api(format!("http client: {e}")))?;
        Ok(Self { http, api_key })
    }

    /// Build a client from the stored key (Keychain → env). `Err(EXC-KEY)` if none.
    pub fn from_stored() -> AppResult<Self> {
        Self::from_stored_with_timeout(DEFAULT_TIMEOUT)
    }

    /// Like [`from_stored`](Self::from_stored), with an explicit per-request timeout.
    pub fn from_stored_with_timeout(timeout: Duration) -> AppResult<Self> {
        let key = crate::config::get_api_key()
            .ok_or_else(|| AppError::Auth("no Claude API key configured".into()))?;
        Self::with_timeout(key, timeout)
    }

    /// `POST /v1/messages` with `body`, retrying 429/5xx/529/transport with backoff.
    pub fn messages(&self, body: &serde_json::Value) -> AppResult<MessagesResponse> {
        self.messages_cancellable(body, None)
    }

    /// Like [`messages`](Self::messages), but a `cancel` flag (set by session
    /// teardown) short-circuits the retry/backoff loop so `end()` doesn't block
    /// behind a stack of retries. An HTTP attempt already in flight still runs to
    /// its timeout — `reqwest::blocking` has no mid-request cancellation — so the
    /// caller should also use a short per-request timeout (see [`with_timeout`](Self::with_timeout)).
    pub fn messages_cancellable(
        &self,
        body: &serde_json::Value,
        cancel: Option<&AtomicBool>,
    ) -> AppResult<MessagesResponse> {
        retry_send(|| self.send_once(body), cancel)
    }

    fn send_once(&self, body: &serde_json::Value) -> Result<MessagesResponse, SendError> {
        let resp = self
            .http
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(body)
            .send()
            .map_err(|e| SendError::Transport(format!("request failed: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<u64>().ok());
            let body = resp.text().unwrap_or_default();
            return Err(SendError::Status {
                code: status.as_u16(),
                retry_after,
                body,
            });
        }
        resp.json::<MessagesResponse>()
            .map_err(|e| SendError::Transport(format!("decode response: {e}")))
    }

    /// Validate the key with a 1-token ping; returns the model that answered.
    pub fn test_key(&self) -> AppResult<String> {
        let body = serde_json::json!({
            "model": MODEL_HAIKU,
            "max_tokens": 1,
            "messages": [{ "role": "user", "content": "ping" }],
        });
        let resp = self.messages(&body)?;
        Ok(if resp.model.is_empty() {
            MODEL_HAIKU.to_string()
        } else {
            resp.model
        })
    }

    /// SSE-stream a Messages request, calling `on_token` for each text delta.
    /// Returns the full concatenated text + final usage. Used by Ask-AI chat
    /// (PR3, D13). Not retried — a chat turn is interactive, so a failure surfaces
    /// immediately rather than stalling behind backoff.
    pub fn stream_text(
        &self,
        body: &serde_json::Value,
        on_token: impl FnMut(&str),
    ) -> AppResult<StreamOutcome> {
        let resp = self
            .http
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(body)
            .send()
            .map_err(|e| AppError::Api(format!("request failed: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            if status.as_u16() == 401 {
                return Err(AppError::Auth("invalid Claude API key (HTTP 401)".into()));
            }
            let body = resp.text().unwrap_or_default();
            return Err(AppError::Api(format!(
                "Claude API HTTP {}: {}",
                status.as_u16(),
                truncate(&body, 200)
            )));
        }

        parse_sse(BufReader::new(resp).lines(), on_token)
    }
}

/// Parse a stream of SSE lines (`data: {json}`) into accumulated text + usage,
/// invoking `on_token` for each text delta. Extracted from `stream_text` so the
/// frame handling is unit-testable without a live connection.
fn parse_sse(
    lines: impl Iterator<Item = std::io::Result<String>>,
    mut on_token: impl FnMut(&str),
) -> AppResult<StreamOutcome> {
    let mut text = String::new();
    let mut usage = Usage::default();
    let mut stop_reason: Option<String> = None;
    for line in lines {
        let line = line.map_err(|e| AppError::Api(format!("stream read: {e}")))?;
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(ev) = serde_json::from_str::<StreamEvent>(data) else {
            continue; // ignore frames we don't model (ping, content_block_start, …)
        };
        match ev.kind.as_str() {
            "message_start" => {
                if let Some(m) = ev.message {
                    usage.input_tokens = m.usage.input_tokens;
                    usage.cache_read_input_tokens = m.usage.cache_read_input_tokens;
                    usage.cache_creation_input_tokens = m.usage.cache_creation_input_tokens;
                }
            }
            "content_block_delta" => {
                if let Some(d) = ev.delta {
                    if d.kind.as_deref() == Some("text_delta") {
                        if let Some(t) = d.text {
                            on_token(&t);
                            text.push_str(&t);
                        }
                    }
                }
            }
            "message_delta" => {
                if let Some(u) = ev.usage {
                    usage.output_tokens = u.output_tokens;
                }
                if let Some(sr) = ev.delta.and_then(|d| d.stop_reason) {
                    stop_reason = Some(sr);
                }
            }
            // A mid-stream error (e.g. `overloaded_error`) arrives as a 200 SSE
            // frame, not an HTTP error. Surface it instead of silently returning
            // the partial text as if the answer were complete.
            "error" => {
                let detail = ev
                    .error
                    .map(|e| {
                        let kind = if e.kind.is_empty() { "error".into() } else { e.kind };
                        if e.message.is_empty() {
                            kind
                        } else {
                            format!("{kind}: {}", truncate(&e.message, 200))
                        }
                    })
                    .unwrap_or_else(|| "unknown error".into());
                return Err(AppError::Api(format!("Claude stream error — {detail}")));
            }
            _ => {}
        }
    }
    Ok(StreamOutcome {
        text,
        usage,
        stop_reason,
    })
}

/// SSE frames we care about from a streamed Messages response (PR3). Unmodeled
/// frames (`ping`, `content_block_start`, `message_stop`) deserialize and fall
/// through the match harmlessly.
#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    message: Option<StreamMessage>,
    #[serde(default)]
    delta: Option<StreamDelta>,
    #[serde(default)]
    usage: Option<DeltaUsage>,
    #[serde(default)]
    error: Option<StreamError>,
}

#[derive(Deserialize)]
struct StreamMessage {
    #[serde(default)]
    usage: Usage,
}

#[derive(Deserialize)]
struct StreamDelta {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    text: Option<String>,
    /// Present on the terminal `message_delta` frame.
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct DeltaUsage {
    #[serde(default)]
    output_tokens: u64,
}

/// An `error` event the API can emit mid-stream (e.g. `overloaded_error`).
#[derive(Deserialize)]
struct StreamError {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    message: String,
}

/// Whether an HTTP status warrants a retry (rate-limit / transient server error).
fn is_retryable_status(code: u16) -> bool {
    matches!(code, 429 | 500 | 502 | 503 | 529)
}

/// Backoff before the next attempt: honor a `retry-after` (capped at 30 s), else
/// exponential `500 ms · 2^(attempt-1)`.
fn backoff_delay(attempt: u32, retry_after: Option<u64>) -> Duration {
    match retry_after {
        Some(s) => Duration::from_secs(s.min(30)),
        None => Duration::from_millis(500 * (1u64 << attempt.saturating_sub(1))),
    }
}

/// The retry/backoff state machine, generic over `send` so it's unit-testable
/// without a live HTTP client. Retries 429/5xx/529/transport up to `MAX_ATTEMPTS`
/// (honoring retry-after); a 401 fails fast; a set `cancel` short-circuits the
/// wait so session teardown isn't stalled behind a stack of retries.
fn retry_send<F>(mut send: F, cancel: Option<&AtomicBool>) -> AppResult<MessagesResponse>
where
    F: FnMut() -> Result<MessagesResponse, SendError>,
{
    let mut attempt = 0;
    loop {
        attempt += 1;
        match send() {
            Ok(resp) => return Ok(resp),
            Err(SendError::Status { code: 401, .. }) => {
                return Err(AppError::Auth("invalid Claude API key (HTTP 401)".into()));
            }
            Err(err) => {
                let (retryable, retry_after, msg) = match err {
                    SendError::Status {
                        code,
                        retry_after,
                        body,
                    } => (
                        is_retryable_status(code),
                        retry_after,
                        format!("Claude API HTTP {code}: {}", truncate(&body, 200)),
                    ),
                    SendError::Transport(m) => (true, None, m),
                };
                let stopping = cancel.is_some_and(|c| c.load(Ordering::Relaxed));
                if !retryable || attempt >= MAX_ATTEMPTS || stopping {
                    return Err(AppError::Api(msg));
                }
                // Interruptible backoff: wake promptly if teardown sets `cancel`.
                if !sleep_unless_cancelled(backoff_delay(attempt, retry_after), cancel) {
                    return Err(AppError::Api(msg));
                }
            }
        }
    }
}

/// Sleep up to `dur`, waking early (returning `false`) if `cancel` becomes set.
/// Returns `true` if the full duration elapsed (caller may retry). With no flag
/// it sleeps the whole duration. Polls at ~100 ms granularity so a long backoff
/// can't stall session teardown.
fn sleep_unless_cancelled(dur: Duration, cancel: Option<&AtomicBool>) -> bool {
    let Some(cancel) = cancel else {
        std::thread::sleep(dur);
        return true;
    };
    let step = Duration::from_millis(100);
    let mut left = dur;
    while left > Duration::ZERO {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let chunk = left.min(step);
        std::thread::sleep(chunk);
        left -= chunk;
    }
    !cancel.load(Ordering::Relaxed)
}

/// Char-boundary-safe truncation for embedding an error body in a message.
fn truncate(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((idx, _)) => format!("{}…", &s[..idx]),
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haiku_cost_is_in_micro_usd() {
        // 100 in / 20 out on Haiku → 100*1 + 20*5 = 200 micro-USD = $0.0002.
        let u = Usage {
            input_tokens: 100,
            output_tokens: 20,
            ..Default::default()
        };
        assert!((u.cost(MODEL_HAIKU) - 0.0002).abs() < 1e-12, "got {}", u.cost(MODEL_HAIKU));
    }

    #[test]
    fn cache_read_is_cheap() {
        // 1000 cache-read tokens on Haiku ≈ 1000 * 1.0 * 0.10 = 100 micro-USD.
        let u = Usage {
            cache_read_input_tokens: 1000,
            ..Default::default()
        };
        assert!((u.cost(MODEL_HAIKU) - 0.0001).abs() < 1e-12);
    }

    #[test]
    fn sonnet_rates_are_higher_than_haiku() {
        let u = Usage {
            input_tokens: 1_000_000,
            ..Default::default()
        };
        assert!((u.cost(MODEL_SONNET) - 3.0).abs() < 1e-9);
        assert!((u.cost(MODEL_HAIKU) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn empty_key_is_rejected() {
        assert!(ClaudeClient::new("   ").is_err());
    }

    #[test]
    fn response_text_concats_only_text_blocks() {
        let r: MessagesResponse = serde_json::from_value(serde_json::json!({
            "model": "claude-haiku-4-5",
            "content": [
                {"type": "text", "text": "hello "},
                {"type": "thinking", "text": "ignore"},
                {"type": "text", "text": "world"}
            ],
            "usage": {"input_tokens": 5, "output_tokens": 2}
        }))
        .unwrap();
        assert_eq!(r.text(), "hello world");
        assert_eq!(r.usage.input_tokens, 5);
    }

    #[test]
    fn truncate_is_char_safe() {
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("abcdef", 3), "abc…");
        // Multi-byte char at the boundary must not panic.
        let _ = truncate("aé😀bc", 2);
    }

    #[test]
    fn parse_sse_extracts_text_and_usage() {
        let frames: Vec<&str> = vec![
            r#"data: {"type":"message_start","message":{"usage":{"input_tokens":10,"cache_read_input_tokens":3}}}"#,
            r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Hel"}}"#,
            "event: ping",
            r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"lo"}}"#,
            r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":5}}"#,
            "data: [DONE]",
            "",
        ];
        let mut toks: Vec<String> = Vec::new();
        let out = parse_sse(
            frames.into_iter().map(|s| Ok::<String, std::io::Error>(s.to_string())),
            |t| toks.push(t.to_string()),
        )
        .unwrap();
        assert_eq!(out.text, "Hello");
        assert_eq!(toks, vec!["Hel".to_string(), "lo".to_string()]);
        assert_eq!(out.usage.input_tokens, 10);
        assert_eq!(out.usage.cache_read_input_tokens, 3);
        assert_eq!(out.usage.output_tokens, 5);
        assert_eq!(out.stop_reason.as_deref(), Some("end_turn"));
    }

    #[test]
    fn sleep_unless_cancelled_returns_promptly_when_set() {
        let flag = AtomicBool::new(true); // already cancelled
        let start = std::time::Instant::now();
        assert!(!sleep_unless_cancelled(Duration::from_secs(10), Some(&flag)));
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "must not sleep the full duration once cancelled"
        );
    }

    #[test]
    fn sleep_unless_cancelled_completes_without_flag() {
        assert!(sleep_unless_cancelled(Duration::from_millis(10), None));
    }

    #[test]
    fn parse_sse_surfaces_mid_stream_error() {
        let frames: Vec<&str> = vec![
            r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Hel"}}"#,
            r#"data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
            "",
        ];
        let mut toks: Vec<String> = Vec::new();
        let res = parse_sse(
            frames.into_iter().map(|s| Ok::<String, std::io::Error>(s.to_string())),
            |t| toks.push(t.to_string()),
        );
        let err = res.expect_err("a mid-stream error frame must surface as Err");
        assert!(err.to_string().contains("overloaded_error"), "got: {err}");
        // Tokens streamed before the error are still delivered to the caller.
        assert_eq!(toks, vec!["Hel".to_string()]);
    }

    fn ok_resp() -> MessagesResponse {
        serde_json::from_str("{}").unwrap()
    }

    #[test]
    fn is_retryable_status_classifies_codes() {
        for code in [429, 500, 502, 503, 529] {
            assert!(is_retryable_status(code), "{code} should be retryable");
        }
        for code in [200, 400, 401, 403, 404, 413, 422, 501] {
            assert!(!is_retryable_status(code), "{code} should not be retryable");
        }
    }

    #[test]
    fn backoff_honors_capped_retry_after() {
        assert_eq!(backoff_delay(1, Some(2)), Duration::from_secs(2));
        assert_eq!(backoff_delay(1, Some(30)), Duration::from_secs(30));
        // retry-after wins over exponential and is capped at 30 s.
        assert_eq!(backoff_delay(3, Some(120)), Duration::from_secs(30));
    }

    #[test]
    fn backoff_is_exponential_without_retry_after() {
        assert_eq!(backoff_delay(1, None), Duration::from_millis(500));
        assert_eq!(backoff_delay(2, None), Duration::from_millis(1000));
        assert_eq!(backoff_delay(3, None), Duration::from_millis(2000));
    }

    #[test]
    fn retry_send_returns_first_success() {
        let mut calls: u32 = 0;
        let r = retry_send(
            || {
                calls += 1;
                Ok(ok_resp())
            },
            None,
        );
        assert!(r.is_ok());
        assert_eq!(calls, 1);
    }

    #[test]
    fn retry_send_retries_transient_then_succeeds() {
        let mut calls: u32 = 0;
        let r = retry_send(
            || {
                calls += 1;
                if calls < 3 {
                    // retry_after: Some(0) → zero backoff, so the test is fast.
                    Err(SendError::Status { code: 429, retry_after: Some(0), body: "rl".into() })
                } else {
                    Ok(ok_resp())
                }
            },
            None,
        );
        assert!(r.is_ok());
        assert_eq!(calls, 3);
    }

    #[test]
    fn retry_send_gives_up_after_max_attempts() {
        let mut calls: u32 = 0;
        let r = retry_send(
            || {
                calls += 1;
                Err(SendError::Status { code: 503, retry_after: Some(0), body: "down".into() })
            },
            None,
        );
        assert!(matches!(r, Err(AppError::Api(_))));
        assert_eq!(calls, MAX_ATTEMPTS);
    }

    #[test]
    fn retry_send_fails_fast_on_401() {
        let mut calls: u32 = 0;
        let r = retry_send(
            || {
                calls += 1;
                Err(SendError::Status { code: 401, retry_after: None, body: "bad key".into() })
            },
            None,
        );
        assert!(matches!(r, Err(AppError::Auth(_))));
        assert_eq!(calls, 1);
    }

    #[test]
    fn retry_send_does_not_retry_non_retryable() {
        let mut calls: u32 = 0;
        let r = retry_send(
            || {
                calls += 1;
                Err(SendError::Status { code: 400, retry_after: None, body: "bad req".into() })
            },
            None,
        );
        assert!(matches!(r, Err(AppError::Api(_))));
        assert_eq!(calls, 1);
    }

    #[test]
    fn retry_send_short_circuits_when_cancelled() {
        let cancel = AtomicBool::new(true);
        let mut calls: u32 = 0;
        let r = retry_send(
            || {
                calls += 1;
                Err(SendError::Status { code: 429, retry_after: Some(0), body: "rl".into() })
            },
            Some(&cancel),
        );
        assert!(matches!(r, Err(AppError::Api(_))));
        assert_eq!(calls, 1, "a set cancel flag must prevent any retry");
    }
}
