# MVP Build — Technical Design

> How it's wired: process/thread model, Rust module layout, audio + STT + AI subsystems, the full Tauri IPC contract, the Svelte frontend, and storage/schemas. Behavior is in **[flows.md](flows.md)**; build order in **[milestones.md](milestones.md)**.

---

## 1. System Overview

```
┌──────────────────────── Tauri v2 App ────────────────────────┐
│  Svelte + TS (WebView)            Rust (native)               │
│  ┌─────────────────┐  invoke()   ┌──────────────────────────┐ │
│  │ screens + stores │ ─────────► │ commands.rs (API surface)│ │
│  │ (event-driven)   │ ◄───────── │ events.rs (emit to UI)   │ │
│  └─────────────────┘   emit()    └──────────┬───────────────┘ │
│                                             ▼                  │
│                                   ┌──────────────────────────┐ │
│                                   │ SessionManager (state)   │ │
│                                   └──┬───────┬───────┬───────┘ │
│                          audio ◄─────┘  stt ◄┘   ai ◄┘ storage │
└───────────────────────────────────────────────────────────────┘
```

The frontend is **thin and event-driven**: it issues commands and renders whatever events stream back. All real work (audio, STT, AI, files) is in Rust.

---

## 2. Process & Thread Model

A single process, multiple long-lived threads + an async runtime. The guiding rule: **audio capture must never block**, and the **WAV is ground truth**.

```
 main / UI (WebView)
   │ invoke
   ▼
 SessionManager (owns state, on a control task)
   │ spawns per-session:
   ├─ AudioThread:mic ───┐                         crossbeam channels
   ├─ AudioThread::black ─┤─► [SampleChunk] ─► VAD/Segmenter ─► [Utterance]
   │                      └─► WAV writer (tee)                      │
   │                                                                ▼
   ├─ WhisperWorker  ◄──────────────────────────────── [Utterance queue]
   │     └─► [TranscriptEntry] ──► append store ──► emit transcript-entry
   │                                   │
   ├─ AiBatcher (tokio task) ◄─────────┘ (subscribes to entries)
   │     └─► Haiku ─► [Finding] ──► append ──► emit ai-finding / cost-update
   └─ (on End) PostAnalysis (tokio task) ─► Sonnet ─► emit analysis-progress
```

| Thread / task | Runtime | Responsibility | Backpressure rule |
|---|---|---|---|
| `audio::mic`, `audio::blackhole` | OS threads (cpal callback) | Pull frames, resample→16k mono, tee to WAV + segmenter | Never blocks; drops to a bounded ring only the *segmenter* side may lag |
| `WhisperWorker` | dedicated OS thread | Transcribe utterances sequentially | Bounded queue; if full → entries arrive late, audio still saved (EXC-WHISPER-LAG) |
| `AiBatcher` | tokio task | Debounce entries → Haiku | Skips a cycle if a call is in flight |
| `PostAnalysis`, `ask_ai` | tokio tasks | One-shot Sonnet calls | Timeout + retry |
| `SessionManager` | tokio (control) | State machine, spawn/join, persistence orchestration | Serializes commands |

Communication: `crossbeam-channel` for the hot audio→stt path; `tokio::sync::mpsc`/`watch` for control + UI events; `AppHandle::emit` for Rust→frontend.

---

## 3. Rust Module Structure (`src-tauri/src/`)

```
main.rs              # Tauri builder, register commands, init tracing, recovery scan
commands.rs          # #[tauri::command] fns — the IPC surface (§6)
events.rs            # typed event names + emit helpers
session/
  mod.rs             # SessionManager: state enum, transitions, orchestration
  model.rs           # Session, SessionMeta, status enum
audio/
  mod.rs             # AudioEngine: device enum, stream setup, multi-output detection
  capture.rs         # cpal streams (mic + blackhole), resample, tee
  wav.rs             # incremental WAV writer (hound)
  vad.rs             # voice-activity segmentation → utterances
stt/
  mod.rs             # WhisperWorker: model load, transcribe, attribution
  model_mgr.rs       # model download/verify/list
ai/
  mod.rs             # Claude client (reqwest), cost accounting
  live.rs            # AiBatcher + live system prompt + finding parse
  chat.rs            # ask_ai (Sonnet)
  analyze.rs         # post-analysis (Sonnet) + schema
  prompts.rs         # prompt templates
storage/
  mod.rs             # paths, atomic writes, incremental append, recovery
  schema.rs          # serde structs for every JSON file (§7)
config.rs            # settings.json + Keychain (keyring)
error.rs             # AppError (thiserror) → maps to EXC-* codes
```

---

## 4. Audio Subsystem (the crux)

**Two independent capture streams**, each tagged by speaker side. No virtual mic needed for the MVP (we passively listen).

```
 real mic   ─cpal─► [f32 @ devrate] ─resample─► 16k mono ─┬─► WAV (ch L)   ─► tee ─► VAD ─► "you" utterances
 BlackHole  ─cpal─► [f32 @ 48k]     ─resample─► 16k mono ─┴─► WAV (ch R)   ─► tee ─► VAD ─► "remote" utterances
```

- **Devices:** mic = user-selected real input; remote = the "Call Assistant"/BlackHole input. Enumerated via cpal; hotplug via cpal's device-change events → emit `device-changed`, auto-fallback on drop (EXC-DEV-DROP).
- **Format:** force **16 kHz mono f32** into Whisper (resample with `rubato`). Persist `audio.wav` as **stereo 16-bit** (L=you, R=remote) so playback/debug keeps attribution. (44.1/48k native → 16k for STT.)
- **Tee:** each callback writes raw frames to the WAV writer (ground truth, never skipped) and pushes to the segmenter ring.
- **Multi-Output detection:** at start, inspect the default output; if it isn't a Multi-Output containing BlackHole → emit a soft `EXC-NOMULTI` warning (we'll likely only hear "you").
- **Attribution = free 2-way labels** because the two physical streams are known. No diarization. (Trade-off: 2 Whisper passes — validated in the M0 spike; fallback = mix to mono + generic "Speaker".)

---

## 5. STT Subsystem

- **`whisper-rs`** (whisper.cpp), **`medium`** default (`small`/`base` selectable), Metal/Core ML on Apple Silicon.
- **VAD segmentation:** energy-based (or `webrtc-vad`) cuts utterances on a silence gap (~600 ms) with a **hard max length** (~12 s) so we never wait forever or slice mid-word.
- **WhisperWorker:** single dedicated thread; pulls utterances from a bounded queue (both streams interleaved, each tagged). For each: run whisper → `TranscriptEntry`.
- **Output:** `{ id, t_ms, stream: "you"|"remote", text, confidence }` → append to `transcript.jsonl` (incremental) + emit `transcript-entry`.
- **Lag handling:** if the queue depth exceeds a threshold, emit `whisper-status{lagging:true}`; audio is still fully captured in WAV (EXC-WHISPER-LAG). Optionally drop to a faster model under sustained lag (config).

---

## 6. AI Subsystem

One Claude client (`reqwest`, JSON). Model IDs come from settings; **Haiku** for live, **Sonnet** for chat + post. Every call records `tokens_in/out`, `cost`, `latency_ms` into `ai_live.json`/`chat.json` and emits `cost-update`.

**Live (Haiku) — `ai/live.rs`:**
- Trigger: `≥5 new entries OR ≥30 s`, and not already in flight, and ≥1 toggle on.
- Input: recent window + rolling ~3 min context + session `context_notes` + active toggles.
- The frozen system prompt describes **all four** features and sits behind a `cache_control` breakpoint (cached prefix); the **active toggles ride in the user turn**, so flipping F/C/S/Q never invalidates the cache (D12). Output is constrained by **structured outputs** (`output_config.format` json_schema), so the response is schema-valid:
  ```json
  { "fact_checks":[{"claim":"","assessment":"","severity":"warning|info"}],
    "commitments":[{"who":"","what":"","by_when":""}],
    "suggestions":[""], "unanswered_questions":[""] }
  ```
- Parse stays defensive (a refusal / `max_tokens` cut → discard that batch, log, continue — *not* an EXC-API-LIVE failure). Backoff on 429/5xx; after **3** consecutive HTTP failures, auto-disable live AI with a notice (EXC-API-LIVE).

**Chat (Sonnet) — `ai/chat.rs`:** `ask_ai(question)` → full transcript + context + question → free-form answer, **SSE-streamed** (`ClaudeClient::stream_text` → `ai-chat-token` per delta, `ai-chat-done` at the end).

> **Threading (D15):** the AI client is `reqwest::blocking` driven from **dedicated std threads**, not tokio — matching the rest of the backend (capture / STT / model_mgr). The batcher is its own thread (teed off the transcript-entry channel); `ask_ai` / `test_api_key` run via `spawn_blocking`. Streaming needs no async runtime — the blocking `Response` is a `Read`. (The §2 sketch below predates this and still shows the original "tokio task" shape.)

**Post-analysis (Sonnet) — `ai/analyze.rs`:** full transcript + context + live annotations → strict JSON:
```json
{ "summary":"", "actions":[{"title":"","owner":"","deadline":"","transcript_quote":"","type":"commitment|follow_up|suggestion"}],
  "decisions":[""], "key_topics":[""] }
```
Merge/dedupe with live commitments + `[+ Save action]` items before presenting.

**Cost/budget:** running total per session; when `total ≥ budget_cap` → emit `EXC-BUDGET`, pause live AI (transcript continues).

---

## 7. The IPC Contract

The complete frontend↔Rust surface. Frontend calls **commands**; Rust pushes **events**. This is the integration contract both sides build against.

### Commands (frontend → Rust, `invoke`)

| Command | Args | Returns | Notes |
|---|---|---|---|
| `get_settings` | — | `Settings` | on boot |
| `save_settings` | `Settings` | `()` | |
| `test_api_key` | `{key}` | `{ok, model, error?}` | 1-token ping |
| `list_audio_input_devices` | — | `AudioDevice[]` | id, name, is_default |
| `set_capture_device` | `{device_id}` | `()` | |
| `list_models` / `get_model_status` | `{name?}` | `ModelStatus[]` | downloaded? size |
| `download_model` | `{name}` | `()` | progress via events |
| `list_sessions` | — | `SessionMeta[]` | dashboard list |
| `get_session` | `{id}` | `SessionFull` | meta+transcript+analysis |
| `create_session` | `SessionDraft` | `{session_id}` | writes `metadata.json` |
| `run_preflight` | `{session_id}` | `PreflightResult` | the §4 checks |
| `start_capture` | `{session_id}` | `()` | spawns pipeline |
| `pause_capture` / `resume_capture` | — | `()` | |
| `set_toggles` | `{f,c,s,q}` | `()` | affects next batch |
| `ask_ai` | `{question}` | `{answer, cost}` | or streamed |
| `end_session` | — | `()` | finalize → analyzing |
| `run_post_analysis` | `{session_id}` | `()` | progress via events |
| `save_analysis` | `{session_id, analysis}` | `()` | → completed |
| `update_action_status` | `{session_id, action_id, status}` | `()` | patches analysis.json |
| `delete_session` | `{id}` | `()` | |
| `reveal_in_finder` | `{path}` | `()` | |

### Events (Rust → frontend, `emit`)

| Event | Payload | When |
|---|---|---|
| `transcript-entry` | `{session_id, entry}` | each finalized utterance |
| `ai-finding` | `{session_id, finding}` | each live finding |
| `ai-chat-token` / `ai-chat-done` | `{token}` / `{answer}` | ask_ai streaming |
| `cost-update` | `{session_id, total, last}` | after any AI call |
| `capture-state` | `{state, elapsed_ms}` | recording/paused ticks |
| `whisper-status` | `{lagging, queue_depth}` | when lag changes |
| `device-changed` | `{inputs:[AudioDevice]}` | hotplug |
| `analysis-progress` | `{phase}` | analyzing → reviewing |
| `model-download-progress` | `{name, pct}` | downloads |
| `app-error` | `{code:"EXC-…", message, recoverable}` | any handled exception |
| `session-recovered` | `{session_id}` | crash recovery on boot |

---

## 8. Frontend Architecture (Svelte + TS)

- **Router:** a single `mode` store (`onboarding|dashboard|new|live|post|settings`) drives which screen renders — matches the §1 state machine; no URL routing needed.
- **Stores:**
  - `settings` — loaded once, written on change.
  - `sessions` — list for the dashboard; `selectedSession` derived.
  - `live` — `{status, elapsedMs, cost, toggles}`; updated by `capture-state`/`cost-update`.
  - `transcript` — appended by `transcript-entry` (keyed by session).
  - `findings` — appended by `ai-finding`; `chat` for ask-AI.
  - `errors` — toast/banner queue fed by `app-error`.
- **Event wiring:** a single `listen()` setup on mount fans Tauri events into the right stores. Components are pure renderers of store state.
- **Components:** `Dashboard{SessionList,SessionDetail}`, `NewSession`, `Live{Toolbar,Transcript,AiPanel,AskBar}`, `Post{Summary,ActionRow,Decisions}`, `Settings`, shared `{Chip,StatusPill,Toggle,Toast}`. (Visual language: `design/prototype.html`.)

---

## 9. Storage & Data Schemas

Flat files under `~/Library/Application Support/CallAssistant/`. **Writes are incremental & atomic** (write temp → fsync → rename) so a crash never corrupts a file mid-write.

```
settings.json
labels.json                         # [{id,name,color}]
models/  ggml-{base|small|medium}.bin
sessions/{uuid}/
  metadata.json                     # status, name, labels[], date, duration, participants, context_notes, budget_cap, total_api_cost
  audio.wav                         # stereo 16-bit: L=you, R=remote
  transcript.jsonl                   # appended: [{id,t_ms,stream,text,confidence}]
  ai_live.json                      # appended: [{id,t_ms,type,payload,model,tokens_in,tokens_out,cost,latency_ms}]
  chat.json                         # [{t,question,answer,cost}]
  analysis.json                     # {summary,actions[],decisions[],key_topics[],generated_at}
```

- **`action`** (in `analysis.json`): `{id, title, owner, owner_type, type, status, deadline?, transcript_quote, transcript_t_ms, notes?, created_by, completed_at?}`.
- **Incremental append strategy:** `transcript.jsonl`/`ai_live.json` are written as JSON arrays via append-friendly rewrite (or JSONL internally, serialized to JSON on read) so the latest state survives a crash.
- **Recovery scan (boot):** find sessions with `status ∈ {recording,paused,ending,analyzing}` → mark `recovering`, rebuild from `transcript.jsonl`/`audio.wav`, route to POST_PROCESS, emit `session-recovered` (EXC-CRASH).
- **Forward-compatibility:** every entity has a stable `id`; storage stays normalized enough that v1's projects + global-actions view is an additive migration (see [../roadmap.md](../roadmap.md)).

---

## 10. Configuration & Secrets

- **API key (shipped app)** → macOS **Keychain** via the `keyring` crate. Never written to `settings.json` or logs.
- **API key (dev / spikes)** → a gitignored root **`.env`** loaded via `dotenvy` (`.env.example` is the tracked template). The shipped app uses Keychain, not `.env`.
- **`settings.json`** → `{ capture_device_id, whisper_model, default_toggles, budget_default, storage_path, first_run }`.
- **Model files** → downloaded on demand to `models/`, checksum-verified.

## 11. Key Dependencies

**Rust:** `tauri` 2, `cpal`, `rubato` (resample), `webrtc-vad` (or energy VAD), `whisper-rs`, `hound` (WAV), `reqwest`+`serde`/`serde_json`, `tokio`, `crossbeam-channel`, `keyring`, `uuid`, `chrono`, `thiserror`/`anyhow`, `tracing`. **Frontend:** `svelte`, `typescript`, `vite`, `@tauri-apps/api`.
