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

pub mod live;
pub mod prompts;

use serde::Deserialize;
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
        let api_key = api_key.into().trim().to_string();
        if api_key.is_empty() {
            return Err(AppError::Auth("API key is empty".into()));
        }
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| AppError::Api(format!("http client: {e}")))?;
        Ok(Self { http, api_key })
    }

    /// Build a client from the stored key (Keychain → env). `Err(EXC-KEY)` if none.
    pub fn from_stored() -> AppResult<Self> {
        let key = crate::config::get_api_key()
            .ok_or_else(|| AppError::Auth("no Claude API key configured".into()))?;
        Self::new(key)
    }

    /// `POST /v1/messages` with `body`, retrying 429/5xx/529/transport with backoff.
    pub fn messages(&self, body: &serde_json::Value) -> AppResult<MessagesResponse> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match self.send_once(body) {
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
                            matches!(code, 429 | 500 | 502 | 503 | 529),
                            retry_after,
                            format!("Claude API HTTP {code}: {}", truncate(&body, 200)),
                        ),
                        SendError::Transport(m) => (true, None, m),
                    };
                    if !retryable || attempt >= MAX_ATTEMPTS {
                        return Err(AppError::Api(msg));
                    }
                    let wait = retry_after
                        .map(|s| Duration::from_secs(s.min(30)))
                        .unwrap_or_else(|| Duration::from_millis(500 * (1u64 << (attempt - 1))));
                    std::thread::sleep(wait);
                }
            }
        }
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
}
