//! Claude API key storage (technical-design.md §10, D11).
//!
//! The key lives in the macOS **Keychain** (via `keyring`) and is **never**
//! written to `settings.json` or logged. For dev/spikes it also falls back to the
//! `ANTHROPIC_API_KEY` environment variable, so the existing `.env` workflow keeps
//! working without a Keychain entry. Read precedence: **Keychain → env**.

use crate::error::{AppError, AppResult};

/// Keychain service + account under which the key is stored.
const KEYCHAIN_SERVICE: &str = "com.callassistant.audio";
const KEYCHAIN_ACCOUNT: &str = "anthropic-api-key";
/// Dev/spike fallback environment variable.
const ENV_VAR: &str = "ANTHROPIC_API_KEY";

fn entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| AppError::Auth(format!("keychain unavailable: {e}")))
}

/// Pick the first non-empty source: Keychain, then env. Pure, for testability.
fn resolve(keychain: Option<String>, env: Option<String>) -> Option<String> {
    [keychain, env]
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
}

/// Resolve the API key (Keychain → `ANTHROPIC_API_KEY`), or `None`.
pub fn get_api_key() -> Option<String> {
    let keychain = entry().ok().and_then(|e| e.get_password().ok());
    resolve(keychain, std::env::var(ENV_VAR).ok())
}

/// Persist the API key to the Keychain (overwrites any existing entry).
pub fn save_api_key(key: &str) -> AppResult<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::Auth("API key is empty".into()));
    }
    entry()?
        .set_password(key)
        .map_err(|e| AppError::Auth(format!("could not save key to Keychain: {e}")))
}

/// Whether an API key is available from any source — without revealing it.
pub fn has_api_key() -> bool {
    get_api_key().is_some()
}

#[cfg(test)]
mod tests {
    use super::resolve;

    #[test]
    fn keychain_wins_over_env() {
        assert_eq!(
            resolve(Some("kc".into()), Some("env".into())),
            Some("kc".into())
        );
    }

    #[test]
    fn falls_back_to_env_when_keychain_blank_or_absent() {
        assert_eq!(resolve(None, Some("env".into())), Some("env".into()));
        assert_eq!(
            resolve(Some("   ".into()), Some("env".into())),
            Some("env".into())
        );
    }

    #[test]
    fn none_when_both_absent_or_blank() {
        assert_eq!(resolve(None, None), None);
        assert_eq!(resolve(Some("".into()), Some("  ".into())), None);
    }

    #[test]
    fn trims_whitespace() {
        assert_eq!(resolve(Some("  k  ".into()), None), Some("k".into()));
    }
}
