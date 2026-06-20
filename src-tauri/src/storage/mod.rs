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
use crate::session::model::{
    SessionDraft, SessionFull, SessionMeta, SessionStatus, TranscriptEntry,
};
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

/// Path to a session's incremental transcript (`transcript.jsonl`).
pub fn transcript_path(id: &str) -> AppResult<PathBuf> {
    Ok(session_dir(id)?.join("transcript.jsonl"))
}

/// Path to a session's ground-truth recording (`audio.wav`).
// Wired by the SessionManager + crash recovery in PR3.
#[allow(dead_code)]
pub fn audio_path(id: &str) -> AppResult<PathBuf> {
    Ok(session_dir(id)?.join("audio.wav"))
}

/// Path to a session's live-AI log (`ai_live.json`, one record per line — M3).
pub fn ai_live_path(id: &str) -> AppResult<PathBuf> {
    Ok(session_dir(id)?.join("ai_live.json"))
}

/// Path to a session's Ask-AI chat log (`chat.json`, one record per line — M3).
pub fn chat_path(id: &str) -> AppResult<PathBuf> {
    Ok(session_dir(id)?.join("chat.json"))
}

/// Path to a session's user-saved actions (`saved_actions.json`, one per line —
/// M3 `[+ Save action]`; M4 merges these into post-analysis).
pub fn saved_actions_path(id: &str) -> AppResult<PathBuf> {
    Ok(session_dir(id)?.join("saved_actions.json"))
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
    list_sessions_in(&sessions_dir()?)
}

fn list_sessions_in(dir: &Path) -> AppResult<Vec<SessionMeta>> {
    let mut out = Vec::new();

    for entry in fs::read_dir(dir)? {
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
    let transcript = read_transcript_at(&transcript_path(id)?)?;
    Ok(SessionFull {
        meta,
        transcript,
        analysis: None,
    })
}

/// Load just a session's metadata (no transcript) — used at capture start to read
/// `context_notes` + `budget_cap` for the live-AI batcher (M3).
pub fn get_session_meta(id: &str) -> AppResult<SessionMeta> {
    read_json(&metadata_path(id)?)
}

// ----------------------------------------------------------------------------
// Transcript — incremental JSONL (one entry per line)
// ----------------------------------------------------------------------------
//
// The transcript is the second ground-truth artifact (after the WAV). It is
// written **append-only, one JSON object per line**, so a crash never corrupts a
// half-written array — the §9 "JSONL internally, serialized to JSON on read"
// option. `get_session` reads it back into the `SessionFull.transcript` array
// the frontend already expects.

/// Append one transcript entry as a JSON line. Creates the file if needed.
pub fn append_transcript_entry_at(path: &Path, entry: &TranscriptEntry) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_string(entry)?;
    line.push('\n');
    let mut f = fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(line.as_bytes())?;
    f.flush()?;
    Ok(())
}

/// Append one transcript entry to a session's `transcript.jsonl`.
// Convenience over `_at`; used by the SessionManager in PR3.
#[allow(dead_code)]
pub fn append_transcript_entry(id: &str, entry: &TranscriptEntry) -> AppResult<()> {
    append_transcript_entry_at(&transcript_path(id)?, entry)
}

/// Append one arbitrary JSON value as a line to a `.jsonl`-style log (crash-safe
/// append). Used for `ai_live.json` (M3) and `chat.json` (PR3).
pub fn append_json_line(path: &Path, value: &serde_json::Value) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_string(value)?;
    line.push('\n');
    let mut f = fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(line.as_bytes())?;
    f.flush()?;
    Ok(())
}

/// Read a `transcript.jsonl` back into entries. A missing file is an empty
/// transcript; an unparseable line is skipped (logged) rather than failing the
/// whole read — partial transcripts must always remain viewable.
pub fn read_transcript_at(path: &Path) -> AppResult<Vec<TranscriptEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<TranscriptEntry>(line) {
            Ok(entry) => out.push(entry),
            Err(e) => eprintln!("[storage] skipping bad transcript line in {}: {e}", path.display()),
        }
    }
    Ok(out)
}

// ----------------------------------------------------------------------------
// Status transitions + crash recovery
// ----------------------------------------------------------------------------

/// Patch a session's `status` in `metadata.json`.
pub fn set_session_status(id: &str, status: SessionStatus) -> AppResult<()> {
    let path = metadata_path(id)?;
    let mut meta: SessionMeta = read_json(&path)?;
    meta.status = status;
    write_json(&path, &meta)
}

/// Mark a session completed with its final recorded duration + total API cost.
pub fn set_session_completed(id: &str, duration_ms: u64, total_api_cost: f64) -> AppResult<()> {
    let path = metadata_path(id)?;
    let mut meta: SessionMeta = read_json(&path)?;
    meta.status = SessionStatus::Completed;
    meta.duration_ms = duration_ms;
    meta.total_api_cost = total_api_cost;
    write_json(&path, &meta)
}

/// On boot, recover sessions left mid-flight by a crash/force-quit (EXC-CRASH):
/// repair the (possibly unfinalized) WAV header and mark the session terminal so
/// it never stays stuck as `recording`. The WAV + `transcript.jsonl` were written
/// incrementally, so the data survives. Returns the recovered session ids.
/// (M4 will route these to re-analysis instead of straight to `completed`.)
pub fn recover_stale_sessions() -> AppResult<Vec<String>> {
    recover_stale_sessions_in(&sessions_dir()?)
}

fn recover_stale_sessions_in(dir: &Path) -> AppResult<Vec<String>> {
    let mut recovered = Vec::new();
    if !dir.exists() {
        return Ok(recovered);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let meta_file = entry.path().join("metadata.json");
        if !meta_file.exists() {
            continue;
        }
        let mut meta: SessionMeta = match read_json(&meta_file) {
            Ok(m) => m,
            Err(_) => continue, // unreadable metadata is left for EXC-CORRUPT (M5)
        };
        // A normal stale state, OR a `Draft` that already has a real recording
        // (a crash in the narrow window before the first `recording` write — M3).
        let wav = entry.path().join("audio.wav");
        let stale = matches!(
            meta.status,
            SessionStatus::Recording
                | SessionStatus::Paused
                | SessionStatus::Ending
                | SessionStatus::Analyzing
        ) || (meta.status == SessionStatus::Draft && wav_has_data(&wav));
        if !stale {
            continue;
        }
        // Repair the WAV header and derive the duration from the frames on disk.
        if wav.exists() {
            if let Ok(frames) = crate::audio::wav::repair_header(&wav) {
                meta.duration_ms = frames * 1000 / crate::audio::wav::SAMPLE_RATE as u64;
            }
        }
        meta.status = SessionStatus::Completed;
        write_json(&meta_file, &meta)?;
        recovered.push(meta.id.clone());
    }
    Ok(recovered)
}

/// Whether a WAV file holds actual samples (more than just the 44-byte header).
fn wav_has_data(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.len() > 44).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::wav::StereoWavWriter;

    #[test]
    fn recovers_a_stale_recording_session() {
        let base = std::env::temp_dir().join(format!("ca_recov_{}", std::process::id()));
        let sdir = base.join("sess-1");
        fs::create_dir_all(&sdir).unwrap();

        let meta = SessionMeta {
            id: "sess-1".into(),
            status: SessionStatus::Recording,
            name: Some("crashed".into()),
            labels: vec![],
            date: "2026-06-20T00:00:00Z".into(),
            duration_ms: 0,
            participants: vec![],
            context_notes: None,
            budget_cap: None,
            total_api_cost: 0.0,
        };
        write_json(&sdir.join("metadata.json"), &meta).unwrap();

        // 1 s of audio (16 000 frames @ 16 kHz).
        let mut w = StereoWavWriter::create(&sdir.join("audio.wav")).unwrap();
        for _ in 0..16_000 {
            w.write_frame(0.1, -0.1).unwrap();
        }
        w.finalize().unwrap();

        let recovered = recover_stale_sessions_in(&base).unwrap();
        assert_eq!(recovered, vec!["sess-1".to_string()]);

        let after: SessionMeta = read_json(&sdir.join("metadata.json")).unwrap();
        assert_eq!(after.status, SessionStatus::Completed);
        assert!((900..=1100).contains(&after.duration_ms), "duration {}", after.duration_ms);

        let _ = fs::remove_dir_all(&base);
    }

    fn session_meta(id: &str, status: SessionStatus) -> SessionMeta {
        SessionMeta {
            id: id.into(),
            status,
            name: None,
            labels: vec![],
            date: "2026-06-20T00:00:00Z".into(),
            duration_ms: 0,
            participants: vec![],
            context_notes: None,
            budget_cap: None,
            total_api_cost: 0.0,
        }
    }

    #[test]
    fn transcript_jsonl_round_trips_and_skips_bad_lines() {
        use crate::audio::StreamTag;
        let path = std::env::temp_dir().join(format!("ca_tr_{}.jsonl", std::process::id()));
        let _ = fs::remove_file(&path);
        let e1 = TranscriptEntry { id: "a".into(), t_ms: 0, stream: StreamTag::You, text: "hello".into(), confidence: 0.9 };
        let e2 = TranscriptEntry { id: "b".into(), t_ms: 1500, stream: StreamTag::Remote, text: "world".into(), confidence: 0.8 };
        append_transcript_entry_at(&path, &e1).unwrap();
        append_transcript_entry_at(&path, &e2).unwrap();
        // A torn/garbage line must be skipped, not fail the whole read.
        use std::io::Write as _;
        fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"{ not valid json\n").unwrap();

        let got = read_transcript_at(&path).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].text, "hello");
        assert_eq!(got[1].t_ms, 1500);
        assert_eq!(got[1].stream, StreamTag::Remote);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn recovers_a_draft_with_a_real_wav() {
        // M3: a crash before the first `recording` status write leaves a Draft
        // with a partial WAV — recovery must still rescue it.
        let base = std::env::temp_dir().join(format!("ca_recov_draft_{}", std::process::id()));
        let sdir = base.join("s");
        fs::create_dir_all(&sdir).unwrap();
        write_json(&sdir.join("metadata.json"), &session_meta("s", SessionStatus::Draft)).unwrap();
        let mut w = StereoWavWriter::create(&sdir.join("audio.wav")).unwrap();
        for _ in 0..16_000 {
            w.write_frame(0.0, 0.0).unwrap();
        }
        w.finalize().unwrap();

        assert_eq!(recover_stale_sessions_in(&base).unwrap(), vec!["s".to_string()]);
        let after: SessionMeta = read_json(&sdir.join("metadata.json")).unwrap();
        assert_eq!(after.status, SessionStatus::Completed);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn list_sessions_sorts_newest_first_and_skips_unreadable() {
        let base = std::env::temp_dir().join(format!("ca_list_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        for (id, date) in [
            ("a", "2026-01-05T00:00:00Z"),
            ("b", "2026-03-09T00:00:00Z"),
            ("c", "2026-02-01T00:00:00Z"),
        ] {
            let sd = base.join(id);
            fs::create_dir_all(&sd).unwrap();
            let mut m = session_meta(id, SessionStatus::Completed);
            m.date = date.into();
            write_json(&sd.join("metadata.json"), &m).unwrap();
        }
        // A corrupt row is omitted, not fatal (EXC-CORRUPT degrades by omission).
        let bad = base.join("bad");
        fs::create_dir_all(&bad).unwrap();
        fs::write(bad.join("metadata.json"), b"{ corrupt").unwrap();
        // A dir without metadata.json is ignored.
        fs::create_dir_all(base.join("nometa")).unwrap();

        let ids: Vec<String> = list_sessions_in(&base)
            .unwrap()
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, vec!["b", "c", "a"], "newest-first by ISO date");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn atomic_write_round_trips_and_leaves_no_temp() {
        let dir = std::env::temp_dir().join(format!("ca_atomic_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("metadata.json");
        write_json(&target, &session_meta("x", SessionStatus::Draft)).unwrap();
        let back: SessionMeta = read_json(&target).unwrap();
        assert_eq!(back.id, "x");
        // A successful atomic write leaves no `.<name>.<uuid>.tmp` behind.
        let leftover: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .map(|e| e.file_name())
            .collect();
        assert!(leftover.is_empty(), "atomic_write left temp files: {leftover:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn recovery_handles_no_wav_plain_draft_and_corrupt_metadata() {
        let base = std::env::temp_dir().join(format!("ca_recov_misc_{}", std::process::id()));
        // A stale Recording with no WAV is recovered (duration 0).
        let r = base.join("rec");
        fs::create_dir_all(&r).unwrap();
        write_json(&r.join("metadata.json"), &session_meta("rec", SessionStatus::Recording)).unwrap();
        // A plain Draft (no WAV) is left alone.
        let d = base.join("draft");
        fs::create_dir_all(&d).unwrap();
        write_json(&d.join("metadata.json"), &session_meta("draft", SessionStatus::Draft)).unwrap();
        // Unreadable metadata is skipped without panicking.
        let bad = base.join("bad");
        fs::create_dir_all(&bad).unwrap();
        fs::write(bad.join("metadata.json"), b"{ corrupt").unwrap();

        assert_eq!(recover_stale_sessions_in(&base).unwrap(), vec!["rec".to_string()]);
        let rec_after: SessionMeta = read_json(&r.join("metadata.json")).unwrap();
        assert_eq!(rec_after.status, SessionStatus::Completed);
        assert_eq!(rec_after.duration_ms, 0);
        let draft_after: SessionMeta = read_json(&d.join("metadata.json")).unwrap();
        assert_eq!(draft_after.status, SessionStatus::Draft);
        let _ = fs::remove_dir_all(&base);
    }
}
