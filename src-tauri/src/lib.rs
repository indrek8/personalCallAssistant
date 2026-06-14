//! Tauri app entrypoint and IPC registration for the M1 walking skeleton.
//!
//! Module layout follows technical-design.md §3. M1 wires real storage + audio
//! device enumeration; STT/AI are stubs.

mod ai;
mod audio;
mod commands;
mod error;
mod events;
mod session;
mod stt;
mod storage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // --- Real M1 commands ---
            commands::list_audio_input_devices,
            commands::get_settings,
            commands::save_settings,
            commands::create_session,
            commands::list_sessions,
            commands::get_session,
            // --- M1 stubs (named per §7) ---
            commands::start_capture,
            commands::pause_capture,
            commands::end_session,
            commands::ask_ai,
            commands::run_post_analysis,
            commands::save_analysis,
            commands::update_action_status,
            commands::delete_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
