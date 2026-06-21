// App stores (technical-design.md §8). A single `mode` store drives which
// screen renders (matches the flows.md §1 state machine); the rest hold the
// loaded settings, the dashboard session list, the device list, and the
// currently selected session id.

import { writable, derived, get } from "svelte/store";
import type {
  AiChatDoneEvent,
  AiChatTokenEvent,
  AiFindingEvent,
  AnalysisProgressEvent,
  AppErrorEvent,
  AudioDevice,
  CaptureStateEvent,
  ChatTurn,
  CostUpdateEvent,
  Finding,
  LabelRef,
  ModelDownloadProgress,
  Mode,
  PostMode,
  SessionMeta,
  Settings,
  Toast,
  Toggles,
  TranscriptEntry,
  TranscriptEntryEvent,
  WhisperStatusEvent,
} from "./types";
import {
  getSettings as ipcGetSettings,
  listSessions as ipcListSessions,
  listLabels as ipcListLabels,
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

/** Live capture state: status, elapsed time, whisper lag, and running API cost. */
export const live = writable<{
  status: "idle" | "recording" | "paused";
  elapsedMs: number;
  lagging: boolean;
  cost: number;
}>({ status: "idle", elapsedMs: 0, lagging: false, cost: 0 });

/** Live-AI findings for the in-progress session (fed by `ai-finding`, newest first). */
export const findings = writable<Finding[]>([]);

/** Active live-AI feature toggles for the in-progress session. */
export const toggles = writable<Toggles>({ f: false, c: false, s: false, q: false });

/** Ask-AI exchanges for the in-progress session (fed by `ai-chat-*` events). */
export const chat = writable<ChatTurn[]>([]);

/** The id of the session currently capturing (filters transcript events). */
export const liveSessionId = writable<string | null>(null);

/** In-flight model download progress (null = none). */
export const modelDownload = writable<ModelDownloadProgress | null>(null);

// ---- Post-analysis stores (M4) ---------------------------------------------

/** The session being reviewed in the Post screen (captured on End, before nav). */
export const postSessionId = writable<string | null>(null);

/** Post-analysis lifecycle, drives the Post screen's spinner/error/review states. */
export const analysisPhase = writable<"idle" | "analyzing" | "reviewing" | "error">("idle");

/** How the Post screen was entered — set by the caller before navigate("post") (M5). */
export const postMode = writable<PostMode>("fresh");

// ---- Manage stores (M5) ----------------------------------------------------

/** The global label registry (labels.json), loaded on boot + after edits. */
export const labels = writable<LabelRef[]>([]);

/** Transient toast notifications. Sticky ones persist until dismissed. */
export const toasts = writable<Toast[]>([]);

let toastSeq = 0;

/** Push a toast; returns its id. Non-sticky toasts auto-dismiss after ~6s. */
export function pushToast(
  message: string,
  opts: { kind?: Toast["kind"]; code?: string; sticky?: boolean; action?: Toast["action"] } = {},
): string {
  const id = `t${++toastSeq}`;
  const toast: Toast = {
    id,
    kind: opts.kind ?? "info",
    message,
    code: opts.code,
    sticky: opts.sticky ?? false,
    action: opts.action,
  };
  toasts.update((list) => [...list, toast]);
  if (!toast.sticky) setTimeout(() => dismissToast(id), 6000);
  return id;
}

/** Remove a toast by id. */
export function dismissToast(id: string): void {
  toasts.update((list) => list.filter((t) => t.id !== id));
}

/** Resolve a session's embedded label ref against the registry (by id), falling
 * back to the embedded snapshot — so a rename/recolor reflects everywhere while a
 * deleted label still renders from the session's own copy (D24). */
export function resolveLabel(ref: LabelRef, registry: LabelRef[]): LabelRef {
  return registry.find((l) => l.id === ref.id) ?? ref;
}

/** Reload the global label registry (list_labels). */
export async function refreshLabels(): Promise<void> {
  if (!isTauri()) return;
  try {
    labels.set(await ipcListLabels());
  } catch (e) {
    pushToast(`Could not load labels: ${String(e)}`, { kind: "error" });
  }
}

/** Reset the live stores for a new capture session. */
export function startLive(sessionId: string, initialToggles: Toggles): void {
  liveSessionId.set(sessionId);
  transcript.set([]);
  findings.set([]);
  chat.set([]);
  toggles.set(initialToggles);
  live.set({ status: "recording", elapsedMs: 0, lagging: false, cost: 0 });
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
  await listen<AiFindingEvent>("ai-finding", (e) => {
    if (get(liveSessionId) === e.payload.session_id) {
      findings.update((f) => [e.payload.finding, ...f]);
    }
  });
  await listen<CostUpdateEvent>("cost-update", (e) => {
    if (get(liveSessionId) === e.payload.session_id) {
      // Session cost is monotonic; clamp so a cost-update that raced behind a
      // larger total (live batch vs Ask-AI emitting from two threads) can't tick
      // the meter backwards.
      live.update((l) => ({ ...l, cost: Math.max(l.cost, e.payload.total) }));
    }
  });
  // Ask-AI streams to the most-recent (streaming) chat turn — one Q&A at a time.
  await listen<AiChatTokenEvent>("ai-chat-token", (e) => {
    chat.update((turns) => {
      if (turns.length === 0 || !turns[turns.length - 1].streaming) return turns;
      const last = turns[turns.length - 1];
      return [...turns.slice(0, -1), { ...last, answer: last.answer + e.payload.token }];
    });
  });
  await listen<AiChatDoneEvent>("ai-chat-done", (e) => {
    chat.update((turns) => {
      if (turns.length === 0) return turns;
      const last = turns[turns.length - 1];
      return [...turns.slice(0, -1), { ...last, answer: e.payload.answer || last.answer, streaming: false }];
    });
  });
  await listen<AnalysisProgressEvent>("analysis-progress", (e) => {
    analysisPhase.set(e.payload.phase === "reviewing" ? "reviewing" : "analyzing");
  });
  await listen<ModelDownloadProgress>("model-download-progress", (e) => {
    modelDownload.set(e.payload.pct >= 100 ? null : e.payload);
  });
  await listen<AppErrorEvent>("app-error", (e) => {
    pushToast(e.payload.message, {
      kind: "error",
      code: e.payload.code,
      sticky: !e.payload.recoverable,
    });
  });
  await listen<{ session_id: string }>("session-recovered", async (e) => {
    // A crashed `reviewing` session is offered for resume (recover-into-review,
    // D23); anything else recovered transcript-only just gets an info toast.
    const id = e.payload.session_id;
    await refreshSessions();
    const sess = get(sessions).find((s) => s.id === id);
    if (sess?.status === "reviewing") {
      pushToast("Recovered a session mid-review.", {
        kind: "info",
        sticky: true,
        action: {
          label: "Resume review",
          run: () => {
            postMode.set("resume");
            postSessionId.set(id);
            navigate("post");
          },
        },
      });
    } else {
      pushToast("Recovered an interrupted session — saved to your list.", { kind: "info" });
    }
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
  void refreshLabels();
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
