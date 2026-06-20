// TypeScript mirrors of the Rust serde types (src-tauri/src/session/model.rs,
// src-tauri/src/storage/schema.rs, src-tauri/src/audio/mod.rs).
// Keep these in sync with the Rust side — they are the IPC contract.

/** `AudioDevice` — list_audio_input_devices() returns AudioDevice[]. */
export interface AudioDevice {
  id: string;
  name: string;
  is_default: boolean;
}

/** Session lifecycle status (flows.md §2). serde `rename_all = "lowercase"`. */
export type SessionStatus =
  | "draft"
  | "recording"
  | "paused"
  | "ending"
  | "analyzing"
  | "reviewing"
  | "completed"
  | "failed"
  | "recovering";

/** A label/tag reference attached to a session. */
export interface LabelRef {
  id: string;
  name: string;
  color?: string | null;
}

/** Input to create_session — the cheap "New Session" form. */
export interface SessionDraft {
  name?: string | null;
  labels?: LabelRef[];
  participants?: string[];
  context_notes?: string | null;
  budget_cap?: number | null;
}

/** Persisted session metadata (metadata.json) + dashboard list shape. */
export interface SessionMeta {
  id: string;
  status: SessionStatus;
  name?: string | null;
  labels: LabelRef[];
  /** ISO-8601 creation timestamp. */
  date: string;
  duration_ms: number;
  participants: string[];
  context_notes?: string | null;
  budget_cap?: number | null;
  total_api_cost: number;
}

/** Returned by create_session. */
export interface CreatedSession {
  session_id: string;
}

/** Returned by get_session — meta + transcript + (later) analysis. */
export interface SessionFull {
  meta: SessionMeta;
  transcript: TranscriptEntry[];
  analysis: unknown | null;
}

/** Which side of the call an utterance/entry belongs to. */
export type StreamTag = "you" | "remote";

/** One finalized transcript line (transcript.jsonl / `transcript-entry` event). */
export interface TranscriptEntry {
  id: string;
  /** Start time from capture start, in milliseconds. */
  t_ms: number;
  stream: StreamTag;
  text: string;
  /** Mean Whisper token probability, 0–1. */
  confidence: number;
}

/** Payload of the `transcript-entry` event. */
export interface TranscriptEntryEvent {
  session_id: string;
  entry: TranscriptEntry;
}

/** Payload of the `capture-state` event. */
export interface CaptureStateEvent {
  state: "recording" | "paused" | "ending" | "ended";
  elapsed_ms: number;
}

/** Payload of the `whisper-status` event (EXC-WHISPER-LAG). */
export interface WhisperStatusEvent {
  session_id?: string;
  lagging: boolean;
  queue_depth: number;
}

/** One pre-flight check (from run_preflight). */
export interface PreflightCheck {
  id: string;
  label: string;
  status: "ok" | "warn" | "fail";
  message: string;
  fixable?: string | null;
}

/** Result of run_preflight — `ok` is false if any check failed. */
export interface PreflightResult {
  ok: boolean;
  checks: PreflightCheck[];
}

/** Whisper model catalog entry + download/validity status. */
export interface ModelStatus {
  name: string;
  label: string;
  approx_mb: number;
  speed_note: string;
  /** Shown in the onboarding/Settings picker (base is hidden). */
  offered: boolean;
  downloaded: boolean;
  size_bytes: number;
  path: string;
}

/** Payload of `model-download-progress`. */
export interface ModelDownloadProgress {
  name: string;
  pct: number;
}

/** Payload of `app-error`. */
export interface AppErrorEvent {
  code: string;
  message: string;
  recoverable: boolean;
}

/** Returned by test_api_key — a 1-token validation ping result. */
export interface TestKeyResult {
  ok: boolean;
  model?: string | null;
  error?: string | null;
}

/** Returned by get_api_key_status — whether a key is configured (key not exposed). */
export interface ApiKeyStatus {
  present: boolean;
}

/** The four live-AI feature toggles. */
export interface Toggles {
  f: boolean;
  c: boolean;
  s: boolean;
  q: boolean;
}

/** `settings.json` (the API key is never stored here). */
export interface Settings {
  capture_device_id?: string | null;
  whisper_model: string;
  default_toggles: Toggles;
  budget_default: number;
  storage_path?: string | null;
  first_run: boolean;
}

/** Top-level app mode (technical-design.md §8, flows.md §1). */
export type Mode =
  | "booting"
  | "onboarding"
  | "dashboard"
  | "new"
  | "live"
  | "post"
  | "settings";
