//! STT subsystem (Whisper) — stub for M1.
//!
//! The `WhisperWorker` (model load, transcription, attribution) and
//! `model_mgr` land in M2 per technical-design.md §5. Nothing here is wired in
//! M1; this module exists so the layout in §3 is complete and later milestones
//! drop in without touching `lib.rs`.

#![allow(dead_code)]

/// Placeholder marker for the future Whisper worker.
pub struct WhisperWorker;
