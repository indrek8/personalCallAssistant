//! Tauri app entrypoint and IPC registration.
//!
//! Module layout follows technical-design.md §3. M1 wired storage + audio
//! device enumeration; M2 adds the capture → transcript pipeline, the live
//! SessionManager, pre-flight, model management, and crash recovery.

mod ai;
pub mod audio;
mod commands;
mod config;
pub mod error;
mod events;
pub mod session;
pub mod stt;
mod storage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Route whisper.cpp / ggml logging through the `log` crate; with no logger
    // installed it is silent, keeping the console clean during transcription.
    whisper_rs::install_logging_hooks();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(session::manager::AppState::new())
        .setup(|app| {
            // EXC-CRASH: recover any session left mid-flight by a crash/force-quit.
            if let Ok(ids) = storage::recover_stale_sessions() {
                for id in ids {
                    events::emit(
                        app.handle(),
                        events::SESSION_RECOVERED,
                        serde_json::json!({ "session_id": id }),
                    );
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // --- Settings + sessions (M1) ---
            commands::list_audio_input_devices,
            commands::get_settings,
            commands::save_settings,
            commands::create_session,
            commands::list_sessions,
            commands::get_session,
            // --- Live capture pipeline (M2) ---
            commands::start_capture,
            commands::pause_capture,
            commands::resume_capture,
            commands::end_session,
            commands::run_preflight,
            commands::list_models,
            commands::download_model,
            // --- M3 live AI — API key + client (PR1) ---
            commands::test_api_key,
            commands::save_api_key,
            commands::get_api_key_status,
            // --- M3/M4/M5 stubs (named per §7) ---
            commands::ask_ai,
            commands::run_post_analysis,
            commands::save_analysis,
            commands::update_action_status,
            commands::delete_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
