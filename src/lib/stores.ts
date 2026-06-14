// App stores (technical-design.md §8). A single `mode` store drives which
// screen renders (matches the flows.md §1 state machine); the rest hold the
// loaded settings, the dashboard session list, the device list, and the
// currently selected session id.

import { writable, derived, get } from "svelte/store";
import type { AudioDevice, Mode, SessionMeta, Settings } from "./types";
import {
  getSettings as ipcGetSettings,
  listSessions as ipcListSessions,
  listAudioInputDevices as ipcListDevices,
  isTauri,
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
