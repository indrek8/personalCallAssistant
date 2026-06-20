# Architecture

> Shared technical reference for both horizons. Each section notes what the **MVP** does now vs. the **[Version 1](vision.md)** target. Implementation-grade detail — flows, the Tauri IPC contract, schemas, milestones — lives in **[build/](build/)**; the path from MVP to v1 is in **[roadmap.md](roadmap.md)**.

---

## Tech Stack

| Concern | MVP (now) | Version 1 (target) |
|---|---|---|
| App framework | **Tauri v2** (Rust backend + web frontend) | same |
| Frontend | **Svelte + TypeScript** | same |
| Audio capture | **BlackHole fork** ("Call Assistant") — passive 2-stream (Multi-Output for remote + direct mic), no virtual mic | **Custom HAL plugin** (`.driver`), full proxy, zero manual setup |
| Audio capture | Rust with `cpal` | Rust with `cpal` / Core Audio |
| Local STT | `whisper-rs` (whisper.cpp), **`medium`** default (`small`/`base` fallback); **You/Remote** from 2 streams | `whisper-rs`, `large-v3` + per-speaker diarization |
| AI | Claude API — **Haiku** (live) + **Sonnet** (chat & post-analysis) | same, plus templates & budget caps |
| Storage | Flat files (JSON + WAV) | Files + **SQLite** for query/search |
| IPC | Tauri command/event system | + shared-memory ring buffer (plugin ↔ app) |

---

## System Architecture (MVP)

```
┌─────────────────────────────────────────────────────┐
│                   Tauri App                          │
│  ┌──────────────────────────────────────────────┐   │
│  │  Svelte Frontend                             │   │
│  │  Dashboard (mail-inbox split) │ Live takeover │   │
│  └──────────────────────┬───────────────────────┘   │
│                         │ Tauri Events/Commands      │
│  ┌──────────────────────┴───────────────────────┐   │
│  │  Rust Backend                                │   │
│  │  ┌──────────┐ ┌──────────┐ ┌───────────┐    │   │
│  │  │  Audio   │→│  Whisper  │→│  AI       │    │   │
│  │  │  Capture │ │  Pipeline │ │  Pipeline │    │   │
│  │  │  (cpal)  │ │          │ │  (Claude) │    │   │
│  │  └──────────┘ └──────────┘ └───────────┘    │   │
│  │  ┌──────────┐ ┌──────────────────────────┐  │   │
│  │  │  Storage │ │  Session Manager (state) │  │   │
│  │  │  (files) │ │                          │  │   │
│  │  └──────────┘ └──────────────────────────┘  │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘

Audio flow (MVP) — two passive streams, no virtual mic:
  Real mic ───────────────────────► cpal ──┐ "you"
  Meeting app ─► Multi-Output ─► BlackHole ─► cpal ──┤ "remote"
                    └─► your headphones (you hear it) │
                                                       ▼
                        16kHz mono ─► WAV + Whisper ─► Transcript ─► AI ─► Claude API
```

The app runs two UI modes: **dashboard** (split-pane, like Apple Mail) for browsing, and **session** (full-screen takeover) for live recording and post-processing. The Rust backend runs audio capture, the Whisper pipeline, and the AI pipeline on dedicated threads, streaming results to the frontend via Tauri events.

---

## Layer 1: Audio Proxy (Virtual Audio Device)

**The Version 1 target** sits transparently between the meeting app and real hardware: the user selects "Call Assistant" as their mic/speaker in the meeting app **once**, and from then on we own the routing and tee every stream to Whisper. The diagram below is that v1 picture — the **MVP reaches the same transcript more simply, by passive capture** (see the MVP subsection).

```
YOUR VOICE:
Real Mic ──> Our App ──> Virtual Mic ──> Meeting App
                │
                └──> Whisper Pipeline
REMOTE VOICES:
Meeting App ──> Virtual Speaker ──> Our App ──> Real Speaker
                                       │
                                       └──> Whisper Pipeline
```

### MVP: BlackHole fork — passive 2-stream capture

The MVP doesn't proxy the mic at all — it **listens passively** to both sides, so no virtual mic is needed:
- **Your voice** → captured directly from the real microphone (the meeting app keeps using it too).
- **Remote voices** → the meeting app's output runs through a **Multi-Output Device** (BlackHole + your headphones) so you still hear it *and* it lands in BlackHole, which we capture.
- Two known streams ⇒ **free "You" / "Remote" labels — no diarization.**

Fork BlackHole, rebrand as "Call Assistant" (device name + bundle id `com.callassistant.audio.driver`), 2-channel. **One-time setup:** install it, create the Multi-Output Device, and point the meeting app's speaker at it. Use **headphones** — open speakers let the mic re-capture the remote side (echo). Full mechanics: [build/technical-design.md §4](build/technical-design.md).

### Version 1: Custom HAL Audio Plugin

A Core Audio `AudioServerPlugIn` (`.driver` bundle at `/Library/Audio/Plug-Ins/HAL/`, loaded by `coreaudiod`) replaces BlackHole and eliminates manual setup. The plugin is a pair of lock-free ring buffers — no processing, just endpoints — connected to the Rust app via shared memory (sub-millisecond latency).

**Dynamic device lifecycle** — virtual devices exist only while the app runs:
```
App launches ─> connects to plugin via shared memory
             ─> plugin calls AudioObjectsPublishedAndDied() to ADD devices
             ─> "Call Assistant Mic/Speaker" appear system-wide
App quits/   ─> connection drops, heartbeat times out
crashes      ─> plugin REMOVES devices ─> they vanish from all pickers
```
No phantom devices when the app isn't running; a crash mid-call behaves like unplugging a USB mic (meeting app falls back to default).

**Device routing at runtime** — the real input/output devices are switchable mid-call via UI dropdowns; the virtual devices are fixed (our plugin).

**Key concerns:** clock sync (virtual slaves to the real device's sample clock), sample-format matching (force 48kHz float32 everywhere), hotplug handling (watch `kAudioObjectPropertySelectorWildcard`), and fallback to system default if the active device disappears.

---

## Layer 2: Local STT (Whisper)

- `whisper-rs` (whisper.cpp) on Apple Silicon
- **MVP:** **`medium`** default (`small`/`base` fallback) — all real-time on Apple Silicon (medium RTF ~0.055, validated in M0/S1)
- Chunked processing (~5–10s segments) with Voice Activity Detection to skip silence
- Outputs transcript entries `{ t, stream: "you"|"remote", text, confidence }`, emitted to the frontend via Tauri event **and** fed to the AI pipeline
- **MVP:** speaker attribution is **"You" vs "Remote"** for free (the two capture streams are known) — no diarization needed
- **v1:** `medium`/`large-v3` for post-analysis quality + diarization that splits the Remote stream into individual speakers

> ⚠ **Biggest technical unknown:** whether `whisper-rs` builds and runs fast enough on the target Mac. De-risk with a standalone spike before building the pipeline — see [mvp.md → Step 1](mvp.md#step-1-walking-skeleton).

---

## Layer 3: Live AI (during the call)

Transcript chunks are sent to Claude based on the active toggles. All calls are logged with tokens/cost/latency.

**A. Live Analysis (automatic)**
- **Trigger:** every ~30s or ~5 new sentences (whichever first)
- **Model:** Haiku (fast, cheap)
- **Input:** recent chunk + rolling ~3-min context + session context notes + active toggles
- **Toggles:** **F** fact-check (flag claims contradicting context), **C** commitments (promises/deadlines/action items), **S** suggestions (follow-up questions, missed points), **Q** unanswered questions
- **Output (structured JSON):**
  ```json
  {
    "fact_checks": [{"claim": "...", "assessment": "...", "severity": "warning|info"}],
    "commitments": [{"who": "...", "what": "...", "by_when": "..."}],
    "suggestions": ["..."],
    "unanswered_questions": ["..."]
  }
  ```
- When all toggles are off → no automatic calls (saves money)

**B. User Chat (on-demand)**
- **Trigger:** user types in "Ask AI"
- **Model:** Sonnet
- **Input:** full transcript so far + context notes + question → free-form answer

---

## Layer 4: Post-Session Analysis

- **Trigger:** End Session
- **Model:** Sonnet
- **Input:** full transcript + context notes + all live AI annotations
- **Output (structured JSON):**
  ```json
  {
    "summary": "...",
    "actions": [{"title": "...", "owner": "...", "deadline": "...", "transcript_quote": "...", "type": "commitment|follow_up|suggestion"}],
    "decisions": ["..."],
    "key_topics": ["..."]
  }
  ```
- Deduplicates against live-detected commitments; the user reviews/edits before saving.

---

## Storage

**MVP — flat files** under `~/Library/Application Support/CallAssistant/`:
```
├── settings.json                 # app settings
├── labels.json                   # [{ id, name, color }]
└── sessions/
    └── {session-id}/
        ├── metadata.json         # name, labels[], status, date, duration, participants, context_notes
        ├── audio.wav             # stereo 16-bit: L=you, R=remote
        ├── transcript.jsonl       # [{ t, stream: you|remote, text, confidence }]
        ├── ai_live.json          # live AI call logs (requests, responses, cost)
        ├── analysis.json         # post-session output (summary, actions, decisions)
        └── chat.json             # user Q&A log
```
Sessions are flat; labels are global and referenced by ID. **v1** adds SQLite for cross-session queries (global actions view, full-text search) while keeping audio/transcripts on disk.

---

## Data Model

Designed **MVP-forward**: even though the MVP uses flat labels + session-scoped actions, entities carry stable IDs so v1's projects + global-actions view is an additive migration, not a rewrite.

```
project (v1)            session                    action
  id                      id                         id
  name                    project_id (v1) / labels[] session_id
  color                   name                       project_id (v1)
  created_at              status (active|            title
                            reviewing|completed)     owner, owner_type (mine|theirs)
label (MVP)               date, duration_seconds     type (action_item|follow_up|
  id                      participants[]                   promise|decision)
  name                    context_notes              status (pending|in_progress|
  color                   summary                          done|wont_do|postponed)
                          audio_file_path            deadline (nullable)
transcript_entry          total_api_cost             transcript_snippet
  id                      budget_cap                 transcript_timestamp
  session_id              created_at                 notes
  timestamp                                          created_at, completed_at
  speaker               bookmark (v1)                created_by (ai_extracted|manual)
  text                    id, session_id
  confidence              timestamp, note          ai_query (log)
                                                     id, session_id, timestamp
template (v1)                                        type, prompt, response, model
  id, name, toggles{}                                tokens_in, tokens_out
  budget_default                                     cost, latency_ms
  extraction_prompt
```

---

## Key Risks

- **macOS audio capture** — SIP restrictions, permissions, notarization. References: Core Audio Taps API, AudioTee.
- **Whisper latency** — may need `medium` for real-time, `large` for post-analysis. *Spike early.*
- **API cost control** — budget enforcement to avoid surprise bills (MVP displays cost; v1 enforces a hard cap).
