//! SessionManager — orchestrates one live capture+STT session and bridges the
//! background pipeline threads to Tauri events (technical-design.md §2–3).
//!
//! `start` wires capture (PR1) → STT pipeline (PR2) and spawns forwarder threads
//! that turn the pipeline's channels into `transcript-entry` / `whisper-status`
//! events. The cpal streams + Whisper state live on their own threads; this
//! holds only `Send` handles + control signals behind a `Mutex` in Tauri state.

use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crossbeam_channel::unbounded;
use serde_json::json;
use tauri::AppHandle;

use crate::audio::{self, capture::{CaptureEvent, CaptureSession}};
use crate::error::{AppError, AppResult};
use crate::events;
use crate::session::model::{SessionStatus, TranscriptEntry};
use crate::stt::{model_mgr, SttConfig, SttPipeline, WhisperStatus};
use crate::storage;

/// Tauri-managed application state: the single in-flight live session, if any.
#[derive(Default)]
pub struct AppState {
    live: Mutex<Option<LiveSession>>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A running live session: the capture + STT handles, the forwarder threads, and
/// a pause-aware clock.
struct LiveSession {
    session_id: String,
    capture: Option<CaptureSession>,
    stt: Option<SttPipeline>,
    forwarders: Vec<JoinHandle<()>>,
    clock: Clock,
    paused: bool,
}

/// Pause-aware elapsed-time clock (recorded time only — paused gaps excluded).
struct Clock {
    accumulated_ms: u64,
    segment_start: Option<Instant>,
}

impl Clock {
    fn started() -> Self {
        Self {
            accumulated_ms: 0,
            segment_start: Some(Instant::now()),
        }
    }
    fn pause(&mut self) {
        if let Some(start) = self.segment_start.take() {
            self.accumulated_ms += start.elapsed().as_millis() as u64;
        }
    }
    fn resume(&mut self) {
        if self.segment_start.is_none() {
            self.segment_start = Some(Instant::now());
        }
    }
    fn elapsed_ms(&self) -> u64 {
        self.accumulated_ms
            + self
                .segment_start
                .map(|s| s.elapsed().as_millis() as u64)
                .unwrap_or(0)
    }
}

fn emit_capture_state(app: &AppHandle, state: &str, elapsed_ms: u64) {
    events::emit(
        app,
        events::CAPTURE_STATE,
        json!({ "state": state, "elapsed_ms": elapsed_ms }),
    );
}

/// Start capturing for `session_id`. Records `status = recording` **up front** so
/// a crash anywhere during start-up is still recovered (EXC-CRASH), and rolls the
/// session back to `failed` if start-up itself fails (M3).
pub fn start(app: &AppHandle, state: &AppState, session_id: String) -> AppResult<()> {
    let mut guard = state.live.lock().unwrap();
    if guard.is_some() {
        return Err(AppError::Audio("a session is already live".into()));
    }

    storage::set_session_status(&session_id, SessionStatus::Recording)?;
    match start_inner(app, &session_id) {
        Ok(live) => {
            emit_capture_state(app, "recording", 0);
            *guard = Some(live);
            Ok(())
        }
        Err(e) => {
            // Roll back so a failed start isn't left as a phantom recording.
            let _ = storage::set_session_status(&session_id, SessionStatus::Failed);
            Err(e)
        }
    }
}

/// Resolve devices + model, wire capture → STT, and spawn the event forwarders.
fn start_inner(app: &AppHandle, session_id: &str) -> AppResult<LiveSession> {
    let settings = storage::get_settings()?;
    let model_path = model_mgr::model_path(&settings.whisper_model)?;
    let transcript_path = storage::transcript_path(session_id)?;
    let wav_path = storage::audio_path(session_id)?;

    // Mic = the selected device if still present, else the system default.
    let mic_id = match settings.capture_device_id.clone() {
        Some(id) if audio::find_input_device_by_id(&id).is_ok() => id,
        _ => audio::default_input_id()?,
    };
    let remote_id = audio::find_remote_loopback_id()?
        .ok_or_else(|| AppError::Audio("no BlackHole / Call Assistant loopback device".into()))?;

    // Pipeline channels → forwarder threads → Tauri events.
    let (entry_tx, entry_rx) = unbounded::<TranscriptEntry>();
    let (status_tx, status_rx) = unbounded::<WhisperStatus>();
    let (dev_tx, dev_rx) = unbounded::<CaptureEvent>();

    let stt = SttPipeline::start(SttConfig {
        model_path,
        transcript_path,
        entry_tx,
        status_tx,
    })?;
    let capture =
        CaptureSession::start(mic_id, remote_id, wav_path, Some(stt.sender()), Some(dev_tx))?;

    let mut forwarders = Vec::new();
    {
        let app = app.clone();
        let sid = session_id.to_string();
        forwarders.push(thread::spawn(move || {
            while let Ok(entry) = entry_rx.recv() {
                events::emit(
                    &app,
                    events::TRANSCRIPT_ENTRY,
                    json!({ "session_id": sid, "entry": entry }),
                );
            }
        }));
    }
    {
        let app = app.clone();
        let sid = session_id.to_string();
        forwarders.push(thread::spawn(move || {
            while let Ok(s) = status_rx.recv() {
                events::emit(
                    &app,
                    events::WHISPER_STATUS,
                    json!({ "session_id": sid, "lagging": s.lagging, "queue_depth": s.queue_depth }),
                );
            }
        }));
    }
    // EXC-DEV-DROP: surface a device fallback as an app-error toast + a refreshed
    // device list.
    {
        let app = app.clone();
        forwarders.push(thread::spawn(move || {
            while let Ok(ev) = dev_rx.recv() {
                let message = match &ev {
                    CaptureEvent::FellBack { tag, device } => {
                        format!("{tag:?} input disconnected — switched to {device}")
                    }
                    CaptureEvent::Lost { tag } => {
                        format!("{tag:?} input disconnected — no fallback available")
                    }
                };
                events::emit(
                    &app,
                    events::APP_ERROR,
                    json!({ "code": "EXC-DEV-DROP", "message": message, "recoverable": true }),
                );
                if let Ok(inputs) = audio::list_input_devices() {
                    events::emit(&app, events::DEVICE_CHANGED, json!({ "inputs": inputs }));
                }
            }
        }));
    }

    Ok(LiveSession {
        session_id: session_id.to_string(),
        capture: Some(capture),
        stt: Some(stt),
        forwarders,
        clock: Clock::started(),
        paused: false,
    })
}

/// Pause capture (passive — the meeting is unaffected).
pub fn pause(app: &AppHandle, state: &AppState) -> AppResult<()> {
    let mut guard = state.live.lock().unwrap();
    let live = guard
        .as_mut()
        .ok_or_else(|| AppError::Audio("no live session to pause".into()))?;
    if !live.paused {
        if let Some(c) = &live.capture {
            c.pause();
        }
        // Finalize the in-progress utterance so audio across the pause gap isn't
        // fused into one mis-timed entry (H1).
        if let Some(s) = &live.stt {
            s.flush();
        }
        live.clock.pause();
        live.paused = true;
        storage::set_session_status(&live.session_id, SessionStatus::Paused)?;
        emit_capture_state(app, "paused", live.clock.elapsed_ms());
    }
    Ok(())
}

/// Resume capture after a pause.
pub fn resume(app: &AppHandle, state: &AppState) -> AppResult<()> {
    let mut guard = state.live.lock().unwrap();
    let live = guard
        .as_mut()
        .ok_or_else(|| AppError::Audio("no live session to resume".into()))?;
    if live.paused {
        if let Some(c) = &live.capture {
            c.resume();
        }
        live.clock.resume();
        live.paused = false;
        storage::set_session_status(&live.session_id, SessionStatus::Recording)?;
        emit_capture_state(app, "recording", live.clock.elapsed_ms());
    }
    Ok(())
}

/// End the session: stop capture (finalize WAV), flush the final transcript, join
/// the forwarders, and record the terminal status + duration. (M2 has no
/// post-analysis yet, so the session is saved transcript-only as `completed`;
/// M4 inserts analyzing → reviewing before this.)
pub fn end(app: &AppHandle, state: &AppState) -> AppResult<()> {
    let mut live = state
        .live
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| AppError::Audio("no live session to end".into()))?;

    let _ = storage::set_session_status(&live.session_id, SessionStatus::Ending);
    emit_capture_state(app, "ending", live.clock.elapsed_ms());

    // Best-effort teardown (H2): a failure in one step must not skip the others,
    // or the session is left stuck `ending` with leaked threads. Stop capture
    // first (finalizes the WAV and drops the tee feeding STT), then STT (drains +
    // transcribes the tail), then join the forwarders; surface the first error.
    let mut first_err: Option<AppError> = None;
    if let Some(capture) = live.capture.take() {
        if let Err(e) = capture.stop() {
            first_err.get_or_insert(e);
        }
    }
    if let Some(stt) = live.stt.take() {
        if let Err(e) = stt.stop() {
            first_err.get_or_insert(e);
        }
    }
    for h in live.forwarders.drain(..) {
        let _ = h.join();
    }

    let duration_ms = live.clock.elapsed_ms();
    if let Err(e) = storage::set_session_completed(&live.session_id, duration_ms) {
        first_err.get_or_insert(e);
    }
    emit_capture_state(app, "ended", duration_ms);

    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn clock_excludes_paused_time() {
        let mut c = Clock::started();
        std::thread::sleep(Duration::from_millis(40));
        c.pause();
        let at_pause = c.elapsed_ms();
        // While paused, elapsed is frozen (segment_start is None).
        std::thread::sleep(Duration::from_millis(60));
        assert!(
            c.elapsed_ms() <= at_pause + 5,
            "paused time leaked: {} vs {at_pause}",
            c.elapsed_ms()
        );
        c.resume();
        std::thread::sleep(Duration::from_millis(40));
        assert!(
            c.elapsed_ms() >= at_pause + 30,
            "resume did not advance the clock: {}",
            c.elapsed_ms()
        );
    }
}
