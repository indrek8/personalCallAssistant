//! The Tauri IPC command surface (technical-design.md §7).
//!
//! M1 implements the real commands needed for the walking skeleton:
//!   - `list_audio_input_devices` (real, via cpal)
//!   - `get_settings` / `save_settings`
//!   - `create_session` / `list_sessions` / `get_session`
//!
//! Every other §7 command is registered with its exact name but returns an
//! `AppError::NotImplemented` so the IPC contract is fully present and the
//! frontend can wire against it now.
//!
//! Command names here MUST match §7 exactly — they are the integration contract.

use crate::audio::{self, AudioDevice};
use crate::error::{AppError, AppResult};
use crate::session::model::{CreatedSession, SessionDraft, SessionFull, SessionMeta};
use crate::storage::{self, schema::Settings};

// ----------------------------------------------------------------------------
// Real M1 commands
// ----------------------------------------------------------------------------

/// `list_audio_input_devices` → `AudioDevice[]` ({ id, name, is_default }).
#[tauri::command]
pub fn list_audio_input_devices() -> AppResult<Vec<AudioDevice>> {
    audio::list_input_devices()
}

/// `get_settings` → `Settings` (loaded on boot).
#[tauri::command]
pub fn get_settings() -> AppResult<Settings> {
    storage::get_settings()
}

/// `save_settings(Settings)` → `()`.
#[tauri::command]
pub fn save_settings(settings: Settings) -> AppResult<()> {
    storage::save_settings(&settings)
}

/// `create_session(SessionDraft)` → `{ session_id }`. Writes `metadata.json`.
#[tauri::command]
pub fn create_session(draft: SessionDraft) -> AppResult<CreatedSession> {
    let meta = storage::create_session(draft)?;
    Ok(CreatedSession {
        session_id: meta.id,
    })
}

/// `list_sessions` → `SessionMeta[]` (dashboard list).
#[tauri::command]
pub fn list_sessions() -> AppResult<Vec<SessionMeta>> {
    storage::list_sessions()
}

/// `get_session({ id })` → `SessionFull`.
#[tauri::command]
pub fn get_session(id: String) -> AppResult<SessionFull> {
    storage::get_session(&id)
}

// ----------------------------------------------------------------------------
// M1 stubs — registered, named per §7, not implemented yet
// ----------------------------------------------------------------------------

fn not_impl(name: &str) -> AppError {
    AppError::NotImplemented(name.to_string())
}

/// `start_capture({ session_id })` → `()`. Spawns the capture pipeline (M2).
#[tauri::command]
pub fn start_capture(_session_id: String) -> AppResult<()> {
    Err(not_impl("start_capture"))
}

/// `pause_capture` → `()` (M2).
#[tauri::command]
pub fn pause_capture() -> AppResult<()> {
    Err(not_impl("pause_capture"))
}

/// `end_session` → `()`. Finalize → analyzing (M2/M4).
#[tauri::command]
pub fn end_session() -> AppResult<()> {
    Err(not_impl("end_session"))
}

/// `ask_ai({ question })` → `{ answer, cost }` (M3).
#[tauri::command]
pub fn ask_ai(_question: String) -> AppResult<serde_json::Value> {
    Err(not_impl("ask_ai"))
}

/// `run_post_analysis({ session_id })` → `()` (M4).
#[tauri::command]
pub fn run_post_analysis(_session_id: String) -> AppResult<()> {
    Err(not_impl("run_post_analysis"))
}

/// `save_analysis({ session_id, analysis })` → `()` (M4).
#[tauri::command]
pub fn save_analysis(
    _session_id: String,
    _analysis: serde_json::Value,
) -> AppResult<()> {
    Err(not_impl("save_analysis"))
}

/// `update_action_status({ session_id, action_id, status })` → `()` (M4/M5).
#[tauri::command]
pub fn update_action_status(
    _session_id: String,
    _action_id: String,
    _status: String,
) -> AppResult<()> {
    Err(not_impl("update_action_status"))
}

/// `delete_session({ id })` → `()` (M5).
#[tauri::command]
pub fn delete_session(_id: String) -> AppResult<()> {
    Err(not_impl("delete_session"))
}
