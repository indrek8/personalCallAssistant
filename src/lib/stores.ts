// App stores (technical-design.md §8). A single `mode` store drives which
// screen renders (matches the flows.md §1 state machine); the rest hold the
// loaded settings, the dashboard session list, the device list, and the
// currently selected session id.

import { writable, derived, get } from "svelte/store";
import type {
  AppErrorEvent,
  AudioDevice,
  CaptureStateEvent,
  ModelDownloadProgress,
  Mode,
  SessionMeta,
  Settings,
  TranscriptEntry,
  TranscriptEntryEvent,
  WhisperStatusEvent,
} from "./types";
import {
  getSettings as ipcGetSettings,
  listSessions as ipcListSessions,
  listAudioInputDevices as ipcListDevices,
  isTauri,
  listen,
} from "./ipc";

/** The single router store — drives which screen renders. */
export const mode = writable<Mode>("booting");

/** Loaded once on boot, written back on Settings save. */
export const settings = writable<Settings | null>(null);

/** Dashboard session list (from list_sessions, on disk). */
export const sessions = writable<SessionMeta[]>([]);

/** Audio input devices (from list_audio_input_devices). */
export const devices = writable<AudioDevice[]>([]);

/** Selected session id for the dashboard detail pane. */
export const selectedSessionId = writable<string | null>(null);

/** Derived selected session, or null. */
export const selectedSession = derived(
  [sessions, selectedSessionId],
  ([$sessions, $id]) => $sessions.find((s) => s.id === $id) ?? null,
);

/** Non-fatal error banner text (null = hidden). */
export const banner = writable<string | null>(null);

export function navigate(next: Mode): void {
  mode.set(next);
}

// ---- Live-session stores (M2) ----------------------------------------------

/** Live transcript for the in-progress session (fed by `transcript-entry`). */
export const transcript = writable<TranscriptEntry[]>([]);

/** Live capture state: status, elapsed time, and whisper lag. */
export const live = writable<{
  status: "idle" | "recording" | "paused";
  elapsedMs: number;
  lagging: boolean;
}>({ status: "idle", elapsedMs: 0, lagging: false });

/** The id of the session currently capturing (filters transcript events). */
export const liveSessionId = writable<string | null>(null);

/** In-flight model download progress (null = none). */
export const modelDownload = writable<ModelDownloadProgress | null>(null);

/** Reset the live stores for a new capture session. */
export function startLive(sessionId: string): void {
  liveSessionId.set(sessionId);
  transcript.set([]);
  live.set({ status: "recording", elapsedMs: 0, lagging: false });
}

let listenersReady = false;

/** Subscribe (once) to the backend event stream and fan events into stores. */
export async function setupEventListeners(): Promise<void> {
  if (!isTauri() || listenersReady) return;
  listenersReady = true;

  await listen<TranscriptEntryEvent>("transcript-entry", (e) => {
    if (get(liveSessionId) === e.payload.session_id) {
      transcript.update((t) => [...t, e.payload.entry]);
    }
  });
  await listen<CaptureStateEvent>("capture-state", (e) => {
    const s = e.payload.state;
    live.update((l) => ({
      ...l,
      elapsedMs: e.payload.elapsed_ms,
      status: s === "paused" ? "paused" : s === "recording" ? "recording" : "idle",
    }));
  });
  await listen<WhisperStatusEvent>("whisper-status", (e) => {
    live.update((l) => ({ ...l, lagging: e.payload.lagging }));
  });
  await listen<ModelDownloadProgress>("model-download-progress", (e) => {
    modelDownload.set(e.payload.pct >= 100 ? null : e.payload);
  });
  await listen<AppErrorEvent>("app-error", (e) => {
    banner.set(`${e.payload.code}: ${e.payload.message}`);
  });
  await listen<{ session_id: string }>("session-recovered", () => {
    banner.set("Recovered a session that was interrupted — it's been saved to your list.");
    void refreshSessions();
  });
}

/**
 * Boot: load settings, route onboarding vs dashboard, and pull the session
 * list + devices in the background. Safe to call outside Tauri (browser
 * preview) — it falls back to a default route so the UI still renders.
 */
export async function boot(): Promise<void> {
  if (!isTauri()) {
    // Plain-browser preview: no backend. Show the dashboard with mock state.
    settings.set(null);
    mode.set("dashboard");
    return;
  }

  try {
    const s = await ipcGetSettings();
    settings.set(s);
    mode.set(s.first_run ? "onboarding" : "dashboard");
  } catch (e) {
    banner.set(`Could not load settings: ${String(e)}`);
    mode.set("dashboard");
  }

  // Subscribe to the backend event stream (transcript, capture state, errors).
  void setupEventListeners();

  // Fire-and-forget refreshes; failures surface in the banner but don't block.
  void refreshSessions();
  void refreshDevices();
}

/** Reload the dashboard list from disk (list_sessions). */
export async function refreshSessions(): Promise<void> {
  if (!isTauri()) return;
  try {
    const list = await ipcListSessions();
    sessions.set(list);
    // Keep selection valid; default-select the first row if nothing chosen.
    const sel = get(selectedSessionId);
    if (!sel || !list.some((s) => s.id === sel)) {
      selectedSessionId.set(list.length ? list[0].id : null);
    }
  } catch (e) {
    banner.set(`Could not load sessions: ${String(e)}`);
  }
}

/** Reload the audio input device list (list_audio_input_devices). */
export async function refreshDevices(): Promise<void> {
  if (!isTauri()) return;
  try {
    devices.set(await ipcListDevices());
  } catch (e) {
    banner.set(`Could not list audio devices: ${String(e)}`);
  }
}
