// Typed wrappers around the real Tauri commands registered in
// src-tauri/src/commands.rs. Command names here MUST match that file exactly —
// they are the integration contract (technical-design.md §7).

import { invoke } from "@tauri-apps/api/core";
import type {
  AudioDevice,
  CreatedSession,
  ModelStatus,
  PreflightResult,
  SessionDraft,
  SessionFull,
  SessionMeta,
  Settings,
  TestKeyResult,
  ApiKeyStatus,
  Toggles,
  Finding,
} from "./types";

/** Re-exported so stores can subscribe to backend events in one place. */
export { listen } from "@tauri-apps/api/event";

/** Are we running inside the Tauri shell (vs. a plain browser dev preview)? */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

// ---- Real M1 commands ------------------------------------------------------

/** list_audio_input_devices() -> AudioDevice[] (real, via cpal). */
export function listAudioInputDevices(): Promise<AudioDevice[]> {
  return invoke<AudioDevice[]>("list_audio_input_devices");
}

/** get_settings() -> Settings (loaded on boot). */
export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

/** save_settings(Settings) -> (). */
export function saveSettings(settings: Settings): Promise<void> {
  return invoke<void>("save_settings", { settings });
}

/** create_session(SessionDraft) -> { session_id }. Writes metadata.json. */
export function createSession(draft: SessionDraft): Promise<CreatedSession> {
  return invoke<CreatedSession>("create_session", { draft });
}

/** list_sessions() -> SessionMeta[] (dashboard list, from disk). */
export function listSessions(): Promise<SessionMeta[]> {
  return invoke<SessionMeta[]>("list_sessions");
}

/** get_session({ id }) -> SessionFull. */
export function getSession(id: string): Promise<SessionFull> {
  return invoke<SessionFull>("get_session", { id });
}

// ---- M2 live-capture commands ----------------------------------------------

/** run_preflight({ session_id }) -> PreflightResult (the §4 Start gate). */
export function runPreflight(sessionId: string): Promise<PreflightResult> {
  return invoke<PreflightResult>("run_preflight", { sessionId });
}

/** start_capture({ session_id }) -> (). Spawns capture → STT. */
export function startCapture(sessionId: string): Promise<void> {
  return invoke<void>("start_capture", { sessionId });
}

/** pause_capture() -> (). */
export function pauseCapture(): Promise<void> {
  return invoke<void>("pause_capture");
}

/** resume_capture() -> (). */
export function resumeCapture(): Promise<void> {
  return invoke<void>("resume_capture");
}

/** end_session() -> (). Finalizes the WAV + transcript. */
export function endSession(): Promise<void> {
  return invoke<void>("end_session");
}

/** list_models() -> ModelStatus[]. */
export function listModels(): Promise<ModelStatus[]> {
  return invoke<ModelStatus[]>("list_models");
}

/** download_model({ name }) -> (). Progress via `model-download-progress`. */
export function downloadModel(name: string): Promise<void> {
  return invoke<void>("download_model", { name });
}

// ---- M3 live-AI: API key management (PR1) ----------------------------------

/** test_api_key({ key }) -> { ok, model?, error? }. 1-token Haiku ping. */
export function testApiKey(key: string): Promise<TestKeyResult> {
  return invoke<TestKeyResult>("test_api_key", { key });
}

/** save_api_key({ key }) -> (). Persists to the macOS Keychain. */
export function saveApiKey(key: string): Promise<void> {
  return invoke<void>("save_api_key", { key });
}

/** get_api_key_status() -> { present }. Never returns the key itself. */
export function getApiKeyStatus(): Promise<ApiKeyStatus> {
  return invoke<ApiKeyStatus>("get_api_key_status");
}

/** set_toggles({ f, c, s, q }) -> (). Live-AI features for the next batch. */
export function setToggles(t: Toggles): Promise<void> {
  return invoke<void>("set_toggles", { f: t.f, c: t.c, s: t.s, q: t.q });
}

/** ask_ai({ question }) -> { answer, cost }. Streams ai-chat-token/done during. */
export function askAi(question: string): Promise<{ answer: string; cost: number }> {
  return invoke<{ answer: string; cost: number }>("ask_ai", { question });
}

/** save_action({ finding }) -> (). Persists a saved commitment to the session. */
export function saveAction(finding: Finding): Promise<void> {
  return invoke<void>("save_action", { finding });
}
