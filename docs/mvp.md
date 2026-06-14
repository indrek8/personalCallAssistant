# MVP — First Iteration Toward v1

> The MVP is the **smallest build that's genuinely useful and proves the hard parts**. It's the first step toward **[Version 1](vision.md)**; the sequence that closes the gap is in **[roadmap.md](roadmap.md)**. Technical design is in **[architecture.md](architecture.md)**; the screen-by-screen UI spec is in **[design/ui-spec.md](../design/ui-spec.md)**.

## Goal

A working **end-to-end flow**: start a session → capture audio → see a live transcript → get AI insights during the call → end the session → get a full analysis with actions + summary. Works with any meeting app (Teams / Meet / Zoom).

## Locked Decisions

The planning docs originally disagreed on two forks; both are settled for the MVP:

- **Product shape → Dashboard + Labels.** Flat session list (Apple Mail split-pane), sessions tagged with labels, actions scoped to each session. *Not* the sidebar + projects + global-actions model from [vision.md](vision.md) — that's a v1 target, sequenced in [roadmap.md](roadmap.md).
- **Frontend → Svelte + TypeScript.** (`design/sidebar-prototype.jsx` is a React prototype of the off-path sidebar model — reference only. The canonical prototype is `design/prototype.html`.)

Full stack + rationale: [architecture.md](architecture.md).

## Scope

**In:**
- BlackHole fork ("Call Assistant") — passive 2-stream capture (Multi-Output for remote + direct mic), manual one-time setup
- Local Whisper transcription with **"You" / "Remote"** labels (free from the two streams; no diarization)
- Live AI panel with F / C / S / Q toggles (Haiku)
- "Ask AI" chat during the call (Sonnet)
- Post-session analysis: summary + extracted actions + decisions, reviewed before save (Sonnet)
- Dashboard (mail-inbox split), labels, session detail, settings, onboarding
- Flat-file storage; API cost display

**Out — deferred to [roadmap.md](roadmap.md):** custom HAL plugin, projects, global cross-session actions view, session playback, bookmarks, diarization, templates, "Prepare for Next Call", menu bar, full-text search, budget-cap enforcement, keyboard shortcuts beyond OS defaults.

## UI — Six Screens

Two modes: **dashboard** (browsing) and **session** (full-screen takeover). Full spec with mockups: **[design/ui-spec.md](../design/ui-spec.md)**; interactive preview: open `design/prototype.html` in a browser.

1. **Setup / Onboarding** (once) — API key → audio device → Whisper model → done
2. **Dashboard** — mail-inbox split: session list (left) + session detail (right); top bar with [+ New Session] + settings
3. **New Session** — name, labels, participants, context-for-AI textarea, F/C/S/Q toggles, [Start Session]
4. **Live Session** — transcript (~60%) + AI panel (~40%: toggles + findings feed + Ask AI); thin toolbar (rec dot, timer, cost, pause, end)
5. **Post-Processing** — processing spinner → review: editable summary, actions (checkbox / owner / deadline / source quote), decisions → [Save & Close]
6. **Settings** — API key (with test), audio device, Whisper model, AI defaults, storage path

---

## Implementation Order

> **Implementation-grade build plan:** the steps below are the summary. The detailed, workable plan — every flow, exception, the Tauri IPC contract, data schemas, and milestone task checklists — lives in **[build/](build/)** ([flows](build/flows.md) · [technical-design](build/technical-design.md) · [milestones](build/milestones.md)).

### Step 1: Walking Skeleton

**The first build milestone** (the M0 de-risking spikes in [build/milestones.md](build/milestones.md) come first). Step 1 is a **running app that proves the whole stack is wired together** — frontend ↔ Rust ↔ filesystem ↔ system audio — before building any real features on top. No Whisper, no Claude, no audio capture yet. Just a tracer bullet through every layer.

**Build:**
1. **Scaffold** a Tauri v2 project with the Svelte + TypeScript template (`npm create tauri-app@latest`, choose Svelte + TS).
2. **Dashboard shell** (Svelte): the mail-inbox split layout from `design/ui-spec.md` Screen 2 — top bar with `[+ New Session]` + gear, left session-list pane, right detail pane. Render from mock data for now. *(Full session-detail rendering, filtering, and label CRUD come in Step 6.)*
3. **New Session form** (Svelte): `ui-spec.md` Screen 3 — name, labels, participants, context textarea, F/C/S/Q toggles, `[Start Session]`. Wire navigation: dashboard → form → (stub) live view.
4. **One real IPC slice:** a Rust `#[tauri::command] list_audio_input_devices()` using `cpal` that returns the system's input devices. Call it from the form (or Settings) and populate a real dropdown. *This single end-to-end call proves Svelte ↔ Rust ↔ Core Audio works.*
5. **File-storage module** (Rust): create/read the storage tree under `~/Library/Application Support/CallAssistant/`. Implement `create_session()` → writes `sessions/{uuid}/metadata.json`, and `list_sessions()` → reads them back. Point the dashboard's left pane at real `list_sessions()` output instead of mock data.

**Done when (acceptance criteria):**
- [ ] `npm run tauri dev` launches a window showing the dashboard shell.
- [ ] Clicking `[+ New Session]`, filling the form, and hitting Start creates a folder under `~/Library/Application Support/CallAssistant/sessions/{uuid}/` with a valid `metadata.json`.
- [ ] Created sessions appear in the left pane, loaded from disk, surviving an app restart.
- [ ] The audio-device dropdown is populated by the real `cpal` Rust command (not hardcoded).
- [ ] The app handles the macOS microphone-permission prompt gracefully on first device access.

**Result:** a clickable, persistent app shell with a proven audio-device path and real storage — the foundation every later step builds on.

> ⚠ **De-risk in parallel — the Whisper spike.** The single biggest technical unknown is whether `whisper-rs` builds and runs fast enough on your Mac. *Before* committing to Steps 2–3, write a throwaway ~30-line Rust binary that loads a `base`/`small` model and transcribes a pre-recorded 10-second WAV, and time it. If real-time transcription isn't feasible at `small`, you want to know now — it shapes the whole audio pipeline. This spike is independent of Step 1 and can be done first or alongside it.

### Step 2: Audio Capture
- BlackHole fork: rename, build, document the one-time setup
- `cpal` integration: dual-stream capture (real mic = "you", BlackHole = "remote")
- WAV file writing on a background thread (for playback later)
- Start / stop / pause controls wired to the frontend

### Step 3: Whisper Pipeline
- `whisper-rs` integration, **`medium`** default (`small`/`base` selectable)
- Chunked processing (~5–10s) on a background thread, with VAD to skip silence
- Transcript entries streamed to the frontend via Tauri events
- Live transcript display in Svelte (auto-scrolling)

### Step 4: AI Pipeline — Live Analysis
- Claude API client in Rust (`reqwest`)
- Live-analysis loop: batch transcript chunks → Haiku → structured findings
- Toggle system (F / C / S / Q) controlling which features are active
- AI panel in the frontend displaying findings
- API cost tracking (tokens in/out, cost per call)

### Step 5: AI Pipeline — Chat + Post-Analysis
- "Ask AI" input → Sonnet with full transcript + context
- Post-session flow: End Session → Sonnet processes full transcript → structured output (summary, actions, decisions)
- Post-analysis review UI (edit actions, owners, deadlines; check/uncheck; add manually)
- Save & Close flow

### Step 6: Session Management
- Label CRUD (create, rename, delete, assign color)
- Session list in the dashboard left pane (sortable, filterable by label)
- Session detail in the right pane (summary, actions, transcript)
- New session form; onboarding / setup flow

### Step 7: Polish & Settings
- Settings screen (API key, Whisper model, audio device, default toggles)
- Error handling (API failures, audio device disconnects, Whisper errors)
- Cost display in the toolbar
- Pause / resume behavior

---

## Verification / Testing Plan

1. **Audio capture** — start a session, play audio through BlackHole, verify the WAV is written correctly
2. **Whisper** — transcript entries appear in the UI within ~10s of speech
3. **Live AI** — enable toggles, speak sentences with commitments/facts → AI panel shows findings
4. **Chat** — ask "summarize so far" mid-session → response appears
5. **Post-analysis** — end a session → summary + actions are generated, editable, and saveable
6. **Persistence** — close and reopen the app → past sessions browsable with all data intact
7. **Real meeting test** — use in an actual Teams/Meet/Zoom call to validate the full flow
