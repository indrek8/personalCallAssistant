//! Storage subsystem — real, working filesystem layer for M1.
//!
//! Base directory: `~/Library/Application Support/CallAssistant/`
//! (technical-design.md §9). Provides:
//!   - `get_settings` / `save_settings` over `settings.json`
//!   - `create_session(draft)` → writes `sessions/{uuid}/metadata.json`
//!   - `list_sessions()` → reads every session's metadata
//!   - `get_session(id)` → metadata (+ placeholders for transcript/analysis)
//!
//! All writes are **atomic**: serialize → write a temp file → fsync → rename
//! over the target, so a crash never leaves a half-written file.

pub mod schema;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::session::model::{SessionDraft, SessionFull, SessionMeta};
use schema::Settings;

/// Application support root: `~/Library/Application Support/CallAssistant/`.
///
/// Uses `dirs::data_dir()` which on macOS resolves to
/// `~/Library/Application Support`. Ensures the directory exists.
pub fn base_dir() -> AppResult<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| AppError::Storage("could not resolve OS data dir".into()))?;
    let base = data_dir.join("CallAssistant");
    fs::create_dir_all(&base)?;
    Ok(base)
}

/// `…/CallAssistant/sessions/`.
pub fn sessions_dir() -> AppResult<PathBuf> {
    let dir = base_dir()?.join("sessions");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// `…/CallAssistant/sessions/{id}/`.
fn session_dir(id: &str) -> AppResult<PathBuf> {
    Ok(sessions_dir()?.join(id))
}

/// `…/CallAssistant/settings.json`.
fn settings_path() -> AppResult<PathBuf> {
    Ok(base_dir()?.join("settings.json"))
}

/// Atomically write `bytes` to `target`: temp file in the same directory →
/// fsync → rename. Same-directory rename is atomic on the same filesystem.
fn atomic_write(target: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = target
        .parent()
        .ok_or_else(|| AppError::Storage(format!("no parent dir for {}", target.display())))?;
    fs::create_dir_all(parent)?;

    // Unique temp name so concurrent writers don't collide.
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file"),
        Uuid::new_v4()
    ));

    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }

    // Rename over the destination. On failure, clean up the temp file.
    match fs::rename(&tmp, target) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(AppError::Storage(format!(
                "atomic rename failed for {}: {e}",
                target.display()
            )))
        }
    }
}

fn write_json<T: serde::Serialize>(target: &Path, value: &T) -> AppResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    atomic_write(target, &bytes)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> AppResult<T> {
    let bytes = fs::read(path)?;
    let value = serde_json::from_slice(&bytes)?;
    Ok(value)
}

// ----------------------------------------------------------------------------
// Settings
// ----------------------------------------------------------------------------

/// Load `settings.json`, or return defaults (and persist them) on first run.
pub fn get_settings() -> AppResult<Settings> {
    let path = settings_path()?;
    if path.exists() {
        read_json(&path)
    } else {
        let defaults = Settings::default();
        write_json(&path, &defaults)?;
        Ok(defaults)
    }
}

/// Persist `settings.json` atomically.
pub fn save_settings(settings: &Settings) -> AppResult<()> {
    write_json(&settings_path()?, settings)
}

// ----------------------------------------------------------------------------
// Sessions
// ----------------------------------------------------------------------------

/// Path to a session's `metadata.json`.
fn metadata_path(id: &str) -> AppResult<PathBuf> {
    Ok(session_dir(id)?.join("metadata.json"))
}

/// Create a new session directory and write `metadata.json`; return its id.
pub fn create_session(draft: SessionDraft) -> AppResult<SessionMeta> {
    let id = Uuid::new_v4().to_string();
    let date = Utc::now().to_rfc3339();
    let meta = SessionMeta::from_draft(id.clone(), date, draft);

    let dir = session_dir(&id)?;
    fs::create_dir_all(&dir)?;
    write_json(&metadata_path(&id)?, &meta)?;
    Ok(meta)
}

/// Read every session's `metadata.json` into a list.
///
/// Sessions whose metadata won't parse are skipped (logged) rather than failing
/// the whole list — the dashboard must never crash on one bad row (EXC-CORRUPT
/// handling proper comes in M5; here we degrade by omission).
pub fn list_sessions() -> AppResult<Vec<SessionMeta>> {
    let dir = sessions_dir()?;
    let mut out = Vec::new();

    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let meta_file = entry.path().join("metadata.json");
        if !meta_file.exists() {
            continue;
        }
        match read_json::<SessionMeta>(&meta_file) {
            Ok(meta) => out.push(meta),
            Err(e) => eprintln!(
                "[storage] skipping unreadable session {}: {}",
                entry.path().display(),
                e
            ),
        }
    }

    // Newest first by creation date (ISO-8601 sorts lexicographically).
    out.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(out)
}

/// Load a single session's metadata (+ placeholders for transcript/analysis).
pub fn get_session(id: &str) -> AppResult<SessionFull> {
    let meta_file = metadata_path(id)?;
    if !meta_file.exists() {
        return Err(AppError::NotFound(format!("session {id}")));
    }
    let meta: SessionMeta = read_json(&meta_file)?;
    Ok(SessionFull {
        meta,
        transcript: Vec::new(),
        analysis: None,
    })
}
