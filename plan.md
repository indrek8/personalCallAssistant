# MVP Plan: Personal Call Assistant

## Context

Building a macOS desktop app that acts as an invisible meeting assistant. It sits between the microphone/speaker as a virtual audio proxy, transcribes locally with Whisper, shows a live transcript with AI analysis, and extracts actions post-meeting. Works with any meeting app (Teams, Meet, Zoom).

The MVP goal: **a working end-to-end flow** — start a session, capture audio, see live transcript, get AI insights during the call, end the session, get a full analysis with actions/summary.

## Tech Stack

- **App framework**: Tauri v2 (Rust backend + Svelte/TypeScript frontend)
- **Audio device**: Custom-branded BlackHole fork ("Call Assistant")
- **Audio capture**: Rust with `cpal`
- **Local STT**: `whisper-rs` (whisper.cpp Rust bindings)
- **AI**: Claude API (Haiku for live analysis, Sonnet for chat + post-analysis)
- **Storage**: File system (JSON + WAV files)
- **IPC**: Tauri command/event system

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Tauri App                          │
│                                                     │
│  ┌──────────────────────────────────────────────┐   │
│  │  Svelte Frontend                             │   │
│  │                                              │   │
│  │  Dashboard: mail-inbox split layout          │   │
│  │  ┌──────────────┬─────────────────────────┐  │   │
│  │  │ Session List │ Session Detail           │  │   │
│  │  └──────────────┴─────────────────────────┘  │   │
│  │                                              │   │
│  │  Live Session: full-screen takeover          │   │
│  │  ┌───────────────────────────────────────┐   │   │
│  │  │ Transcript + AI Panel                 │   │   │
│  │  └───────────────────────────────────────┘   │   │
│  └──────────────────────┬───────────────────────┘   │
│                         │ Tauri Events/Commands      │
│  ┌──────────────────────┴───────────────────────┐   │
│  │  Rust Backend                                │   │
│  │                                              │   │
│  │  ┌──────────┐  ┌──────────┐  ┌───────────┐  │   │
│  │  │  Audio   │  │  Whisper  │  │  AI       │  │   │
│  │  │  Capture │──│  Pipeline │──│  Pipeline │  │   │
│  │  │  (cpal)  │  │          │  │  (Claude) │  │   │
│  │  └──────────┘  └──────────┘  └───────────┘  │   │
│  │                                              │   │
│  │  ┌──────────┐  ┌──────────────────────────┐  │   │
│  │  │  Storage │  │  Session Manager         │  │   │
│  │  │  (files) │  │  (state machine)         │  │   │
│  │  └──────────┘  └──────────────────────────┘  │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘

Audio Flow:
  Real Mic ──> BlackHole ──> cpal capture ──> WAV file
                                    │
                                    └──> Whisper ──> Transcript ──> AI Pipeline
                                                                        │
  Meeting App <──> BlackHole (virtual device)                    Claude API
```

---

## UI Navigation Model

Two distinct modes:

1. **Dashboard mode** — mail-inbox style split view for browsing sessions
2. **Session mode** — full-screen immersive views for live recording and post-processing

```
┌──────────┐    ┌─────────────────────────┐    ┌─────────────┐    ┌────────────┐    ┌─────────────┐
│  Setup   │───>│      Dashboard          │───>│ New Session  │───>│   Live     │───>│   Post-     │
│(one-time)│    │  (mail-inbox layout)    │<──>│   (form)     │    │  Session   │    │ Processing  │
└──────────┘    │                         │    └─────────────┘    └────────────┘    └──────┬──────┘
                │  ┌─────────┬──────────┐ │                                                │
                │  │ Session │ Session  │ │<───────────────────────────────────────────────┘
                │  │  List   │ Detail   │ │                                          (Save & Close)
                │  └─────────┴──────────┘ │
                └─────────────────────────┘
                  Settings via gear icon
```

**Key principles:**
- Dashboard is the home base — split-pane like Apple Mail. Session list on left, detail on right.
- Live session and post-processing take over the full window — zero distractions during a call.
- Actions live only within their session (no global actions view).

---

## Component Details

### 1. BlackHole Fork ("Call Assistant")

Fork BlackHole, rebrand as "Call Assistant":
- Change device name to "Call Assistant"
- Change bundle ID to `com.callassistant.audio.driver`
- Build 2-channel (stereo) version
- Include build instructions / installer script

**User setup (one-time):**
1. Install the "Call Assistant" audio driver
2. Create an aggregate device in Audio MIDI Setup combining real mic + Call Assistant
3. In meeting app, select "Call Assistant" as mic/speaker

> Note: For post-MVP, the custom HAL plugin eliminates this manual setup.

### 2. Tauri v2 App Shell

- Tauri v2 with Svelte + TypeScript frontend
- Single window with two modes: dashboard (split-pane) and session (full-screen)
- Tauri event system for streaming data (transcript entries, AI responses) from Rust to Svelte

### 3. Audio Capture (Rust)

Runs on a dedicated thread:
- Capture audio from the "Call Assistant" device via `cpal`
- Write raw audio to WAV file continuously (for playback later)
- Feed audio chunks to Whisper pipeline via a channel
- Handle pause/resume (stop feeding Whisper, optionally stop WAV recording)

### 4. Whisper STT Pipeline (Rust)

Runs on a dedicated thread:
- Uses `whisper-rs` with `base` or `small` model (fast enough for real-time on Apple Silicon)
- Receives audio chunks (~5-10 second segments) from audio capture
- Voice Activity Detection (VAD) to skip silence
- Outputs transcript entries: `{ timestamp, text, confidence }`
- Emits each entry to frontend via Tauri event AND feeds to AI pipeline
- No speaker diarization in MVP — all text attributed to generic "Speaker"

### 5. AI Pipeline (Rust)

Three modes, all using Claude API via `reqwest`:

#### A. Live Analysis (automatic, during session)

- **Trigger**: Every ~30 seconds or ~5 new transcript sentences (whichever comes first)
- **Model**: Haiku (fast, cheap — ~200ms response time)
- **Input**: Recent transcript chunk + rolling context window (last ~3 min) + session context notes + active toggles
- **System prompt** tells the model which features are enabled:
  - **Fact-check** (F): Flag claims that contradict the context notes or seem inaccurate
  - **Commitments** (C): Detect promises, deadlines, action items
  - **Suggestions** (S): Suggest follow-up questions or points being missed
  - **Q&A** (Q): Flag questions that went unanswered
- **Output**: Structured JSON
  ```json
  {
    "fact_checks": [{"claim": "...", "assessment": "...", "severity": "warning|info"}],
    "commitments": [{"who": "...", "what": "...", "by_when": "..."}],
    "suggestions": ["..."],
    "unanswered_questions": ["..."]
  }
  ```
- **Cost tracking**: Log tokens_in, tokens_out, cost per call
- When all toggles are off → no automatic calls (saves money)

#### B. User Chat (on-demand, during session)

- **Trigger**: User types a question in the "Ask AI" input
- **Model**: Sonnet (higher quality for direct questions)
- **Input**: Full transcript so far + context notes + user's question
- **Output**: Free-form text response displayed in AI panel

#### C. Post-Session Analysis (after session ends)

- **Trigger**: User clicks "End Session"
- **Model**: Sonnet (quality matters for final output)
- **Input**: Full transcript + context notes + all live AI annotations
- **Output**: Structured JSON
  ```json
  {
    "summary": "...",
    "actions": [
      {"title": "...", "owner": "...", "deadline": "...", "transcript_quote": "...", "type": "commitment|follow_up|suggestion"}
    ],
    "decisions": ["..."],
    "key_topics": ["..."]
  }
  ```
- Deduplicates with live-detected commitments
- User reviews/edits before saving

### 6. File-Based Storage

```
~/Library/Application Support/CallAssistant/
├── settings.json                    # App settings
├── labels.json                      # Array of { id, name, color }
├── sessions/
│   └── {session-id}/
│       ├── metadata.json            # { name, labels[], status, date, duration, participants, context_notes }
│       ├── audio.wav                # Raw captured audio
│       ├── transcript.json          # Array of { timestamp, speaker, text, confidence }
│       ├── ai_live.json             # All live AI call logs (requests + responses + cost)
│       ├── analysis.json            # Post-session analysis output (summary, actions, decisions)
│       └── chat.json                # User chat Q&A log
```

Sessions are flat (no project hierarchy). Labels are stored globally and referenced by ID in each session's metadata. A session can have zero or many labels.

### 7. Frontend UI (Svelte + TypeScript)

#### Views

**Setup / Onboarding (one-time):**
- Step-by-step: API key → audio device → whisper model → done

**Dashboard (home base) — mail-inbox split layout:**
- Top bar: app name, [+ New Session] button, [Settings] gear
- Left pane: sortable session list table (columns: Session Name, Labels, Date, Duration, Actions count)
- Right pane: selected session detail (summary, actions with status, transcript)
- Filter by label, date, search
- Actions are scoped to individual sessions — no global actions view

**New Session (full-screen form):**
- Labels (multi-select/create), session name, participants, context notes textarea
- AI toggle defaults (F, C, S, Q)
- [Start Session] button

**Live Session (full-screen, immersive):**
- Top toolbar: recording indicator, session name, timer, pause/stop, API cost
- Transcript area (~60%): rolling, auto-scrolling, timestamps + speaker + text
- AI panel (~40%, resizable): toggle row [F][C][S][Q], findings feed, "Ask AI" input
- AI panel can be collapsed for full-screen transcript

**Post-Processing (full-screen):**
- Processing state with progress indicator
- Review: editable summary, actions with checkboxes/owners/deadlines, decisions
- [Save & Close] returns to dashboard

**Settings (full-screen):**
- API key, audio device, whisper model, AI toggle defaults, storage path

---

## Implementation Order

### Step 1: Project Scaffolding
- Tauri v2 + Svelte + TypeScript project setup
- Rust workspace with crate structure
- Dashboard layout shell (split-pane with empty session list + detail)
- File storage module (create/read project and session directories)

### Step 2: Audio Capture
- BlackHole fork: rename, build, document setup
- cpal integration: capture from BlackHole device
- WAV file writing on a background thread
- Start/stop/pause controls wired to frontend

### Step 3: Whisper Pipeline
- whisper-rs integration with base/small model
- Chunked processing on background thread
- Transcript entries streamed to frontend via Tauri events
- Live transcript display in Svelte (auto-scrolling)

### Step 4: AI Pipeline — Live Analysis
- Claude API client in Rust (reqwest)
- Live analysis loop: batch transcript chunks → Haiku → structured findings
- Toggle system (F/Q/S/C) controlling which features are active
- AI panel in frontend displaying findings
- API cost tracking

### Step 5: AI Pipeline — Chat + Post-Analysis
- "Ask AI" input → Sonnet with full transcript
- Post-session analysis flow: end session → Sonnet processes full transcript → structured output
- Post-analysis review UI (edit actions, owners, deadlines)
- Save & Close flow

### Step 6: Session Management
- Label CRUD (create, rename, delete, assign color)
- Session list in dashboard left pane (sortable table, filterable by label)
- Session detail in dashboard right pane (summary, actions, transcript)
- New session form
- Onboarding/setup flow

### Step 7: Polish & Settings
- Settings screen (API key, whisper model, audio device, default toggles)
- Error handling (API failures, audio device disconnects, Whisper errors)
- Cost display in toolbar
- Pause/resume behavior

---

## What's NOT in MVP

- Custom HAL audio plugin (using BlackHole fork instead)
- Speaker diarization (all speakers labeled generic "Speaker")
- Session playback (audio player synced with transcript)
- Session templates
- Global actions view across sessions (actions scoped to individual sessions only)
- "Prepare for Next Call" AI briefing
- Menu bar presence
- Bookmarks
- Keyboard shortcuts (beyond basic OS ones)
- Full-text search
- Budget cap enforcement (just display cost)

---

## Verification / Testing Plan

1. **Audio capture**: Start a session, play audio through BlackHole, verify WAV file is written correctly
2. **Whisper**: Verify transcript entries appear in UI within ~10 seconds of speech
3. **Live AI**: Enable toggles, speak sentences with commitments/facts — verify AI panel shows findings
4. **Chat**: Ask "summarize so far" during a session — verify response appears
5. **Post-analysis**: End a session — verify summary and actions are generated, editable, and saveable
6. **Persistence**: Close and reopen app — verify past sessions are browsable with all data intact
7. **Real meeting test**: Use in an actual Teams/Meet/Zoom call to validate the full flow
