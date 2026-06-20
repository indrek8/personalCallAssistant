//! `AppError` — the single error type that crosses the Tauri IPC boundary.
//!
//! It implements `serde::Serialize` so it can be returned from
//! `#[tauri::command]` functions and surface in the frontend. Where useful it
//! maps onto the `EXC-*` codes from `docs/build/flows.md` §9.

use serde::Serialize;
use thiserror::Error;

/// Application-wide error type returned from commands.
///
/// Each variant serializes to a tagged JSON object of the form
/// `{ "code": "EXC-…", "message": "…" }` so the frontend can branch on `code`.
#[derive(Debug, Clone, Error)]
pub enum AppError {
    /// A command exists but is intentionally not wired up yet in M1.
    #[error("not implemented in M1: {0}")]
    NotImplemented(String),

    /// Filesystem / storage failure (read, write, create dir, rename).
    #[error("storage error: {0}")]
    Storage(String),

    /// (De)serialization failure for a JSON file.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Audio device enumeration / Core Audio failure.
    #[error("audio error: {0}")]
    Audio(String),

    /// A requested entity (e.g. a session) could not be found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Local STT (Whisper) failure — model load or transcription.
    #[error("stt error: {0}")]
    Stt(String),

    /// Whisper model missing, download failed, or failed verification.
    #[error("model error: {0}")]
    Model(String),

    /// Missing or invalid Claude API key (EXC-KEY).
    #[error("api key error: {0}")]
    Auth(String),

    /// Claude API call failed — network, timeout, or non-2xx (EXC-API).
    #[error("api error: {0}")]
    Api(String),
}

impl AppError {
    /// Stable machine code, aligned with the `EXC-*` catalogue where one fits.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::NotImplemented(_) => "EXC-NOTIMPL",
            AppError::Storage(_) => "EXC-DISK",
            AppError::Serialization(_) => "EXC-CORRUPT",
            AppError::Audio(_) => "EXC-NODEV",
            AppError::NotFound(_) => "EXC-NOTFOUND",
            AppError::Stt(_) => "EXC-WHISPER",
            AppError::Model(_) => "EXC-MODEL",
            AppError::Auth(_) => "EXC-KEY",
            AppError::Api(_) => "EXC-API",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Serialization(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Storage(e.to_string())
    }
}

/// Convenience alias for command results.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_map_to_exc() {
        assert_eq!(AppError::Stt("x".into()).code(), "EXC-WHISPER");
        assert_eq!(AppError::Model("x".into()).code(), "EXC-MODEL");
        assert_eq!(AppError::Audio("x".into()).code(), "EXC-NODEV");
        assert_eq!(AppError::NotFound("x".into()).code(), "EXC-NOTFOUND");
        assert_eq!(AppError::Storage("x".into()).code(), "EXC-DISK");
        assert_eq!(AppError::Auth("x".into()).code(), "EXC-KEY");
        assert_eq!(AppError::Api("x".into()).code(), "EXC-API");
    }
}
