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

use serde::Serialize;
use serde_json::json;
use std::thread;
use tauri::{AppHandle, State};

use crate::audio::{self, AudioDevice};
use crate::error::{AppError, AppResult};
use crate::events;
use crate::session::manager::{self, AppState};
use crate::session::model::{CreatedSession, SessionDraft, SessionFull, SessionMeta};
use crate::stt::model_mgr::{self, ModelStatus};
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

// ----------------------------------------------------------------------------
// M2 — live capture pipeline (SessionManager) + pre-flight + models
// ----------------------------------------------------------------------------

/// `start_capture({ session_id })` → `()`. Resolves devices + model from
/// settings, wires capture → STT, and starts streaming `transcript-entry`.
#[tauri::command]
pub fn start_capture(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> AppResult<()> {
    manager::start(&app, &state, session_id)
}

/// `pause_capture` → `()`.
#[tauri::command]
pub fn pause_capture(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    manager::pause(&app, &state)
}

/// `resume_capture` → `()`.
#[tauri::command]
pub fn resume_capture(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    manager::resume(&app, &state)
}

/// `end_session` → `()`. Stops capture, finalizes the WAV + transcript. (M2:
/// terminal `completed`; M4 inserts analyzing → reviewing.)
#[tauri::command]
pub fn end_session(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    manager::end(&app, &state)
}

/// One pre-flight check (flows.md §4).
#[derive(Serialize)]
pub struct PreflightCheck {
    id: String,
    label: String,
    /// `ok` | `warn` | `fail`.
    status: String,
    message: String,
    /// A command the UI can offer to fix it (e.g. `download_model`).
    fixable: Option<String>,
}

/// `run_preflight` result: `ok` is false if any check failed.
#[derive(Serialize)]
pub struct PreflightResult {
    ok: bool,
    checks: Vec<PreflightCheck>,
}

/// `run_preflight({ session_id })` → the §4 checks gating Start. M2 covers the
/// capture/transcript prerequisites; the API-key check is an M3 concern.
#[tauri::command]
pub fn run_preflight(_session_id: String) -> AppResult<PreflightResult> {
    let settings = storage::get_settings()?;
    let mut checks = Vec::new();

    match audio::default_input_id() {
        Ok(_) => checks.push(check("mic", "Microphone", "ok", "Input device available", None)),
        Err(_) => checks.push(check("mic", "Microphone", "fail", "No input device found", None)),
    }

    match audio::find_remote_loopback_id() {
        Ok(Some(_)) => checks.push(check(
            "remote",
            "Loopback device",
            "ok",
            "BlackHole / Call Assistant detected",
            None,
        )),
        Ok(None) => checks.push(check(
            "remote",
            "Loopback device",
            "fail",
            "No BlackHole / Call Assistant input — set up the Multi-Output device",
            None,
        )),
        Err(e) => checks.push(check("remote", "Loopback device", "fail", &e.to_string(), None)),
    }

    match model_mgr::model_status(&settings.whisper_model) {
        Ok(st) if st.downloaded => checks.push(check(
            "model",
            "Transcription model",
            "ok",
            &format!("{} ready", settings.whisper_model),
            None,
        )),
        Ok(_) => checks.push(check(
            "model",
            "Transcription model",
            "fail",
            &format!("Model '{}' not downloaded", settings.whisper_model),
            Some("download_model"),
        )),
        Err(e) => checks.push(check("model", "Transcription model", "fail", &e.to_string(), None)),
    }

    let ok = checks.iter().all(|c| c.status.as_str() != "fail");
    Ok(PreflightResult { ok, checks })
}

fn check(id: &str, label: &str, status: &str, message: &str, fixable: Option<&str>) -> PreflightCheck {
    PreflightCheck {
        id: id.into(),
        label: label.into(),
        status: status.into(),
        message: message.into(),
        fixable: fixable.map(|s| s.into()),
    }
}

/// `list_models` → `ModelStatus[]`.
#[tauri::command]
pub fn list_models() -> AppResult<Vec<ModelStatus>> {
    model_mgr::list_models()
}

/// `download_model({ name })` → `()`. Runs in the background, emitting
/// `model-download-progress`; a failure surfaces as `app-error`.
#[tauri::command]
pub fn download_model(app: AppHandle, name: String) -> AppResult<()> {
    thread::spawn(move || {
        let result = model_mgr::download_model(&name, |done, total| {
            let pct = total
                .map(|t| if t > 0 { (done * 100 / t) as u32 } else { 0 })
                .unwrap_or(0);
            events::emit(
                &app,
                events::MODEL_DOWNLOAD_PROGRESS,
                json!({ "name": name.as_str(), "pct": pct }),
            );
        });
        match result {
            Ok(()) => events::emit(
                &app,
                events::MODEL_DOWNLOAD_PROGRESS,
                json!({ "name": name.as_str(), "pct": 100 }),
            ),
            Err(e) => events::emit(
                &app,
                events::APP_ERROR,
                json!({ "code": e.code(), "message": e.to_string(), "recoverable": true }),
            ),
        }
    });
    Ok(())
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
