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

/** Returned by get_session — meta + (later) transcript + analysis. */
export interface SessionFull {
  meta: SessionMeta;
  transcript: unknown[];
  analysis: unknown | null;
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
