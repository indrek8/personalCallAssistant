//! Typed event names + emit helpers for the Rust → frontend channel.
//!
//! The string constants are the **authoritative** event names from
//! `docs/build/technical-design.md` §7. Both Rust and the Svelte frontend build
//! against these exact names. In M1 most emitters are not yet wired; the names
//! and a thin emit helper exist so later milestones plug in without churn.

#![allow(dead_code)]

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Each finalized utterance. Payload: `{ session_id, entry }`.
pub const TRANSCRIPT_ENTRY: &str = "transcript-entry";
/// Each live finding. Payload: `{ session_id, finding }`.
pub const AI_FINDING: &str = "ai-finding";
/// Ask-AI streaming token. Payload: `{ token }`.
pub const AI_CHAT_TOKEN: &str = "ai-chat-token";
/// Ask-AI completion. Payload: `{ answer }`.
pub const AI_CHAT_DONE: &str = "ai-chat-done";
/// After any AI call. Payload: `{ session_id, total, last }`.
pub const COST_UPDATE: &str = "cost-update";
/// Recording / paused ticks. Payload: `{ state, elapsed_ms }`.
pub const CAPTURE_STATE: &str = "capture-state";
/// When whisper lag changes. Payload: `{ lagging, queue_depth }`.
pub const WHISPER_STATUS: &str = "whisper-status";
/// Device hotplug. Payload: `{ inputs: [AudioDevice] }`.
pub const DEVICE_CHANGED: &str = "device-changed";
/// analyzing → reviewing. Payload: `{ phase }`.
pub const ANALYSIS_PROGRESS: &str = "analysis-progress";
/// Model downloads. Payload: `{ name, pct }`.
pub const MODEL_DOWNLOAD_PROGRESS: &str = "model-download-progress";
/// Any handled exception. Payload: `{ code: "EXC-…", message, recoverable }`.
pub const APP_ERROR: &str = "app-error";
/// Crash recovery on boot. Payload: `{ session_id }`.
pub const SESSION_RECOVERED: &str = "session-recovered";

/// Thin wrapper over `AppHandle::emit` that keeps event names centralized.
pub fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(e) = app.emit(event, payload) {
        // Emitting must never crash a control flow; log and move on.
        tracing_or_eprintln(&format!("failed to emit `{event}`: {e}"));
    }
}

#[inline]
fn tracing_or_eprintln(msg: &str) {
    // M1 has no tracing subscriber wired; fall back to stderr.
    eprintln!("[events] {msg}");
}
