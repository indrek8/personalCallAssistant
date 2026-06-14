# Personal Call Assistant

## The Idea

A native macOS desktop application that acts as a personal AI meeting assistant. It sits between the microphone and speaker as a virtual audio proxy, listens to calls, transcribes in real-time, and provides live AI assistance during meetings. Works with any meeting platform (Teams, Google Meet, Zoom) by installing as a virtual microphone/speaker that you select in the meeting app.

Beyond recording, the app is a **meeting-driven work tracker**. Meetings generate actions - things you promised, things others promised, decisions made, follow-ups needed. Those actions live in the app, linked back to the exact moment in the conversation. The core loop: **prepare → record → extract → track → prepare**.

## Core Goals

- Capture meeting audio transparently (no bot joining the call)
- Real-time local speech-to-text transcription
- Live AI assistance (fact-checking, Q&A, suggestions) during calls
- Post-meeting extraction of actions, decisions, follow-ups
- Consolidated action tracking across all sessions
- Everything linked: action → session → transcript moment

## Target Platforms for Calls

- Microsoft Teams
- Google Meet
- Zoom
- Any app that lets you select a microphone/speaker device

---

## UI: Single Window, Sidebar + Main Area

No tabs, no multiple views. One window. Sidebar for navigation, main area shows whatever you selected. Like a mail client or Apple Notes.

```
+------------------+------------------------------------------+
| SIDEBAR          | MAIN AREA                                |
|                  | (changes based on sidebar selection)     |
| [+ New Session]  |                                          |
|                  |                                          |
| ACTIONS (12)     |                                          |
|                  |                                          |
| PROJECTS         |                                          |
|  > KGL (3 ses)   |                                          |
|    Board Q2      |                                          |
|    Tech Review   |                                          |
|    Finance Sync  |                                          |
|  > Kifiya (2)    |                                          |
|    Sprint Rev    |                                          |
|    API Planning  |                                          |
|  > Internal (1)  |                                          |
|    1:1 Sarah     |                                          |
|                  |                                          |
| [+ New Project]  |                                          |
+------------------+------------------------------------------+
```

### Sidebar Elements

- **[+ New Session]** button at the top - the primary action
- **Actions** - consolidated action list across all sessions (with badge count of pending)
- **Projects** - collapsible tree, each project contains its sessions
- **[+ New Project]** - create a new project at the bottom

Clicking anything in the sidebar changes the main area.

---

## Main Area: Five States

The main area has five states depending on what's happening:

### State 1: New Session Setup

*When you click [+ New Session]*

A simple form to set up the call. Nothing fancy - get in and start fast.

```
+------------------------------------------+
| NEW SESSION                              |
|                                          |
| Project:     [KGL Financial       v]     |
| Session name:[Board Call Q2        ]     |
| Template:    [Investor Call       v]     |
| Budget:      [$5.00               ]     |
|                                          |
| Participants (optional):                 |
| [Sarah, Ahmed                     ]     |
|                                          |
| Context for AI (optional):              |
| [Follow-up to Q1 board meeting.   ]     |
| [Discussing CBUAE Phase 2 delays  ]     |
| [and KYC module timeline.         ]     |
|                                          |
|         [>>> Start Session]              |
|                                          |
+------------------------------------------+
```

**Context for AI** is key - you paste in background info so the AI can fact-check and answer questions intelligently during the call. This is where "prepare for next call" output goes.

### State 2: Live Session (the star)

*After hitting Start Session. This is where you spend most of your time.*

The live view is split horizontally: transcript on top, AI panel on bottom. Controls float as a thin toolbar. The transcript gets the most space.

```
+------------------------------------------+
| *REC  KGL / Board Call Q2      00:23:15  |
| [||] [End] [!Bookmark] [$0.12/$5]       |
| Mic: AirPods Pro ▾  | Spk: MacBook ▾    |
+------------------------------------------+
| TRANSCRIPT                         [v AI]|
|                                          |
| 14:03:22  YOU                            |
| So the timeline for the CBUAE submission |
| is what we need to nail down today.      |
|                                          |
| 14:03:28  SARAH                          |
| The central bank pushed to August. But   |
| there's a hard dependency on the KYC     |
| module being certified first.            |
|                                          |
| 14:03:41  YOU                            |
| Right. What's the current status on KYC? |
|                                          |
| 14:03:45  AHMED                          |
| We submitted to the auditor last week.   |
| Expecting results by April 15th.         |
|                                          |
|==========================================|
| AI PANEL                [toggles: F Q S C]|
|                                          |
| FACT-CHECK (14:03:30)                    |
| "End of Q2" claim doesn't match. CBUAE  |
| Phase 2 deadline is Aug 2026 per Central |
| Bank circular CB-2025-041.              |
|                                          |
| COMMITMENT DETECTED (14:03:45)           |
| Ahmed: "KYC audit results by Apr 15"    |
|                        [+ Save Action]   |
|                                          |
| [Ask AI: __________________________ ][>] |
+------------------------------------------+
```

**Top toolbar** (minimal, stays out of the way):
- Recording indicator + session name + timer
- Pause / End buttons
- Bookmark button (Cmd+B) - marks this moment
- API cost meter
- **Device selectors** - dropdowns for real mic and real speaker, switchable mid-call

**Device selection** - because the meeting app is locked to our virtual devices, we own the real device routing:
- User selects which real mic and real speaker to use in our toolbar
- Switching mid-call is seamless — meeting app sees no interruption (still connected to virtual device)
- Dropdowns auto-update on hotplug events (AirPods connect/disconnect, headphones plugged in)
- If the active device disappears mid-call, auto-fallback to system default device
- Defaults can be set in app preferences (e.g., "always prefer AirPods when available")

**Transcript area** (top ~60% of space):
- Rolling transcript, auto-scrolls
- Speaker labels with distinct colors
- Timestamps
- Bookmarks appear inline as highlighted markers
- Scroll up to review, auto-scroll resumes when you return to bottom

**AI Panel** (bottom ~40%, resizable divider):
- **Toggle row** at the top: F(act-check) Q(&A) S(uggestions) C(ommitments) - click to toggle each on/off mid-call. Lit up = active. These are small toggle buttons, not a big sidebar.
- Latest AI outputs shown in reverse-chronological feed
- Each commitment detection has a **[+ Save Action]** button to immediately capture it
- **Ask AI** input at the very bottom - type a question, get an answer based on the transcript so far
- The AI panel can be collapsed with the [v AI] toggle if you just want full-screen transcript

**What "Ask AI" can do during a call:**
- "What exactly did Ahmed say about the timeline?"
- "Summarize what we've agreed so far"
- "What are the open questions we haven't addressed?"
- "Is this deadline realistic given what Sarah said earlier?"

### State 3: Post-Analysis

*After hitting End Session. Automatically transitions here.*

This is where the AI processes the full transcript and you review the results before saving. The session isn't "done" until you review and confirm.

```
+------------------------------------------+
| POST-ANALYSIS: KGL / Board Call Q2       |
| Mar 28, 2026 | 47 min | 3 participants  |
+------------------------------------------+
|                                          |
| SUMMARY                          [Regen] |
| Discussed CBUAE Phase 2 timeline -       |
| central bank extended to Aug 2026.       |
| KYC module certification is the          |
| critical dependency. Ahmed expects       |
| auditor results by Apr 15. Budget        |
| revision needed for Q3 planning.         |
|                                          |
|------------------------------------------|
|                                          |
| EXTRACTED ACTIONS                        |
|                                          |
| [x] Send CBUAE Phase 2 timeline to board|
|     Owner: [Me    v] Due: [Apr 5 ] [del]|
|     "I'll circulate the updated          |
|      timeline to the board by Friday"    |
|                                          |
| [x] Deliver KYC audit results           |
|     Owner: [Ahmed v] Due: [Apr 15] [del]|
|     "Expecting results by April 15th"   |
|                                          |
| [x] Prepare budget revision for Q3      |
|     Owner: [Me    v] Due: [Apr 10] [del]|
|     "Let's get the revised numbers       |
|      together before the next board"     |
|                                          |
| [ ] Share updated risk assessment        |
|     Owner: [Sarah v] Due: [     ] [del] |
|     (this was a suggestion, not a        |
|      commitment - uncheck to discard)    |
|                                          |
| [+ Add action manually]                 |
|                                          |
| DECISIONS                                |
| - Phase 2 deadline moved to Aug 2026    |
| - Will proceed with current KYC vendor  |
| - Q3 budget revision triggered          |
|                                          |
|     [Save & Close]   [Back to Transcript]|
|                                          |
+------------------------------------------+
```

**What you can do here:**
- **Summary** - AI-generated, editable, can regenerate with [Regen]
- **Extracted actions** - AI proposes, you review:
  - Check/uncheck to include or discard
  - Change owner (dropdown: Me, or any participant name)
  - Set/adjust deadline
  - Delete false positives
  - Add ones the AI missed with [+ Add action manually]
  - Each shows the transcript quote it was extracted from
- **Decisions** - key decisions captured (non-actionable but useful for reference)
- **[Back to Transcript]** - jump back to see the full transcript if you need context
- **[Save & Close]** - saves everything, session appears in sidebar under its project

### State 4: Session Review

*When you click a past session in the sidebar.*

Read-only view of a completed session. Transcript + summary + actions all in one scrollable page.

```
+------------------------------------------+
| KGL / Board Call Q2                      |
| Mar 28, 2026 | 47 min | $0.85 API cost  |
+------------------------------------------+
|                                          |
| SUMMARY                                  |
| Discussed CBUAE Phase 2 timeline...      |
|                                          |
|------------------------------------------|
|                                          |
| ACTIONS FROM THIS SESSION         3 of 4 |
|                                          |
| [DONE]  Send CBUAE timeline to board    |
|         Me | Due: Apr 5 | Done: Apr 4   |
|                                          |
| [PEND]  Review KYC audit results        |
|         Me | Due: Apr 15                 |
|                                          |
| [LATE]  Send revised cost estimates     |
|         Ahmed | Due: Apr 8 (2 days late) |
|                                          |
|------------------------------------------|
|                                          |
| TRANSCRIPT                               |
|                                          |
| 14:00:05  YOU                            |
| Thanks everyone for joining...           |
|                                          |
| [! BOOKMARK] "Q2 deadline clarification" |
|                                          |
| 14:03:28  SARAH                          |
| The central bank pushed to August...     |
| ...                                      |
|                                          |
|     [Prepare for Next Call]  [Re-analyze]|
|                                          |
+------------------------------------------+
```

**Key elements:**
- Summary at top for quick refresh
- Actions from this session with current status (they may have been updated since the session)
- Full transcript with bookmarks highlighted
- **[Prepare for Next Call]** - AI generates a briefing: what's still open, what was decided, suggested talking points. Output goes into clipboard or directly into a new session's context field.
- **[Re-analyze]** - re-run the extraction with a different prompt if you want

### Session Playback

Any completed session can be played back. The audio, transcript, bookmarks, and AI annotations are all timestamped - playback syncs them together like a video player scrubbing through the meeting.

```
+------------------------------------------+
| KGL / Board Call Q2            PLAYBACK  |
+------------------------------------------+
|                                          |
| [|<] [>] [>>]  00:14:22 / 00:47:03      |
| |==========*-----------------------------|
|            ^ current position            |
|  [1x v]  [! ! !   !  !!    !  ]         |
|           ^ bookmark/action markers      |
|                                          |
|------------------------------------------|
|                                          |
| > 14:03:22  YOU                          |
|   So the timeline for the CBUAE         |
|   submission is what we need to nail     |
|   down today.                            |
|                                          |
| > 14:03:28  SARAH              << active |
|   The central bank pushed to August.     |
|   But there's a hard dependency on the   |
|   KYC module being certified first.      |
|                                          |
|   [! BOOKMARK] "Q2 deadline"             |
|   [AI] Fact-check: deadline is Aug 2026  |
|        per CB-2025-041                   |
|                                          |
|   14:03:41  YOU                          |
|   Right. What's the current status?      |
|                                          |
+------------------------------------------+
```

**How it works:**
- **Audio playback** with standard controls: play/pause, skip forward/back, speed adjustment (0.5x, 1x, 1.5x, 2x)
- **Timeline scrubber** with visual markers for bookmarks (!) and actions (*) - click any marker to jump there
- **Transcript auto-scrolls** to follow the audio position - the currently spoken line is highlighted
- **Click any transcript line** to jump audio to that moment
- **Bookmarks and AI annotations** appear inline at their timestamp, so you see what was flagged as the audio plays
- **Actions appear at their source moment** - you can see exactly the context when something was promised

**Use cases:**
- "What exactly did Ahmed say about the deadline?" - scrub to the bookmark, listen to the actual words
- Review a call you were multitasking during - play at 2x, stop at bookmarks
- Settle a dispute about what was agreed - play the exact moment, share the transcript snippet
- Onboard someone: "Listen to the first 10 minutes of the board call for context"

### State 5: Actions (consolidated)

*When you click "Actions" in the sidebar.*

All actions from all sessions in one flat list. This is your todo list between meetings.

```
+------------------------------------------+
| ACTIONS                                  |
| [All v] [Pending v] [All owners v] [Q]  |
+------------------------------------------+
|                                          |
| MINE                              8 open |
|                                          |
| [pending v] Send CBUAE timeline to board |
|   KGL > Board Call Q2 | Due: Apr 5      |
|                                          |
| [pending v] Review KYC audit results     |
|   KGL > Board Call Q2 | Due: Apr 15     |
|                                          |
| [pending v] Prepare budget revision Q3   |
|   KGL > Finance Sync  | Due: Apr 10     |
|                                          |
| [pending v] Share API docs with dev team |
|   Kifiya > Sprint Rev  | Due: Apr 2     |
|                                          |
|------------------------------------------|
|                                          |
| WAITING ON OTHERS                 4 open |
|                                          |
| [pending v] Ahmed: Cost estimates        |
|   KGL > Board Call Q2 | Due: Apr 8  !!! |
|                                          |
| [pending v] Sarah: KYC audit results    |
|   KGL > Board Call Q2 | Due: Apr 15     |
|                                          |
| [pending v] Dev team: API sandbox setup  |
|   Kifiya > Sprint Rev  | Due: Apr 12    |
|                                          |
+------------------------------------------+
```

**How action status works:**

Each action has a small **status dropdown** right inline:

```
[pending  v]  ←  click to open
  --------
  pending
  in progress
  done
  won't do
  postponed
  --------
```

One click to change status. No modal, no detail screen needed for the common case.

**The "KGL > Board Call Q2" link** is clickable - takes you to that session in State 4.

**Filters** at the top:
- Project filter (All / specific project)
- Status filter (Pending / In Progress / Done / Won't Do / Postponed / All)
- Owner filter (Mine / Others / All / specific person)
- Search box

**Overdue items** get a visual indicator (red `!!!` or similar).

---

## The Natural Flow

The app supports this daily rhythm without switching views:

```
1. Open app
   → See sidebar: Actions (12 pending)
   → Click Actions → scan what's open, update a few statuses

2. Call coming up
   → Click past session in sidebar for the same project
   → Quick refresh: summary + open actions
   → Hit [Prepare for Next Call] → AI generates briefing

3. Start the call
   → Click [+ New Session] → fill in form → Start
   → Main area becomes live view
   → Transcript flowing, toggle AI features as needed
   → Bookmark key moments, save actions on the fly

4. Call ends
   → Hit End → transitions to post-analysis
   → Review AI-extracted actions, edit owners/deadlines
   → Save & Close

5. Between calls
   → Click Actions in sidebar → manage your list
   → Follow up on overdue items from others
```

It's always one click to get anywhere. The sidebar is your anchor.

---

## Data Model

### Project
```
project:
  id: uuid
  name: string                    # "KGL Financial"
  color: string                   # for visual distinction
  created_at: datetime
```

### Session
```
session:
  id: uuid
  project_id: uuid
  name: string                    # "Board Call Q2"
  status: enum                    # active, reviewing, completed
  date: datetime
  duration_seconds: int
  participants: [string]          # ["Sarah", "Ahmed"]
  context_notes: text             # pre-call context fed to AI
  summary: text                   # AI-generated post-call summary
  audio_file_path: string
  total_api_cost: float
  budget_cap: float
  created_at: datetime
```

### Transcript Entry
```
transcript_entry:
  id: uuid
  session_id: uuid
  timestamp: float                # seconds from session start
  speaker: string                 # "You", "Sarah", or "Speaker 2"
  text: string
  confidence: float               # whisper confidence score
```

### Bookmark
```
bookmark:
  id: uuid
  session_id: uuid
  timestamp: float                # seconds from session start
  note: string
  created_at: datetime
```

### Action
```
action:
  id: uuid
  session_id: uuid                # source session (clickable link)
  project_id: uuid
  title: string                   # "Send CBUAE Phase 2 timeline to board"
  owner: string                   # "me" or person's name
  owner_type: enum                # mine, theirs
  type: enum                      # action_item, follow_up, promise, decision
  status: enum                    # pending, in_progress, done, wont_do, postponed
  deadline: date                  # nullable
  transcript_snippet: text        # the quote from the conversation
  transcript_timestamp: float     # link to exact moment in transcript
  notes: text                     # user's additional notes
  created_at: datetime
  completed_at: datetime          # nullable
  created_by: enum                # ai_extracted, manual
```

### Session Template
```
template:
  id: uuid
  name: string                    # "Investor Call"
  toggles:
    fact_check: bool
    suggestions: bool
    commitments: bool
  budget_default: float
  extraction_prompt: text         # custom post-call prompt
```

### AI Query Log Entry
```
ai_query:
  id: uuid
  session_id: uuid
  timestamp: datetime
  type: enum                      # fact_check, suggestion, commitment_scan, qa, extraction
  prompt: text
  response: text
  model: string
  tokens_in: int
  tokens_out: int
  cost: float
  latency_ms: int
```

---

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+N` | New session |
| `Cmd+B` | Bookmark (during live session) |
| `Cmd+/` | Focus "Ask AI" input |
| `Cmd+.` | End session |
| `Space` | Pause/resume recording (when live) |
| `Cmd+1` | Go to Actions |

---

## Menu Bar

When the app is minimized during a live session:

```
[*] 00:23:15 | $0.12
    [|| Pause]
    [! Bookmark]
    [Open]
    [End Session]
```

---

## Tech Stack

- **App framework**: Tauri v2 (Rust backend + web frontend)
- **Audio capture**: Rust with `cpal` or Core Audio bindings
- **Local STT**: `whisper-rs` (Rust bindings for whisper.cpp)
- **Frontend**: React/TypeScript or Svelte
- **AI layer**: Anthropic Claude API (via `reqwest` from Rust backend)
- **Storage**: SQLite + filesystem for audio files
- **IPC**: Tauri command/event system

## Architecture

### Layer 1: Audio Proxy (Virtual Audio Device)

The app installs as a transparent audio proxy between the meeting app and real hardware.

**Setup:** User selects "Call Assistant Mic" and "Call Assistant Speaker" in their meeting app once. From then on, our app controls which real hardware is used.

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

**Components:**
- **HAL Audio Plugin** (`.driver` bundle at `/Library/Audio/Plug-Ins/HAL/`) — C/C++ Core Audio `AudioServerPlugIn`. Exposes a virtual mic (input) and virtual speaker (output). Loaded by `coreaudiod`. The plugin itself is a pair of ring buffers — no processing, just endpoints.
- **Audio Router** (Rust, in main app) — reads from real mic, writes to virtual mic buffer; reads from virtual speaker buffer, writes to real speakers. Tees both streams to Whisper. Manages device switching at runtime.
- **IPC** between HAL plugin and app via shared memory (mmap'd lock-free ring buffer). Sub-millisecond latency.

**Device lifecycle — virtual devices only exist while the app is running:**

The HAL plugin `.driver` is always installed, but it does **not** publish any devices on its own. Devices appear and disappear dynamically based on whether our app is connected.

```
App launches  ──> connects to HAL plugin via shared memory
              ──> plugin calls AudioObjectsPublishedAndDied() to ADD devices
              ──> "Call Assistant Mic" + "Call Assistant Speaker" appear system-wide

App quits     ──> shared memory connection drops
(or crashes)  ──> plugin detects disconnect (heartbeat timeout / mmap gone)
              ──> plugin calls AudioObjectsPublishedAndDied() to REMOVE devices
              ──> virtual devices vanish from all device pickers
```

This means:
- No phantom devices cluttering System Settings or meeting app dropdowns when our app isn't running
- If the app crashes mid-call, devices disappear — meeting app falls back to its default (same as unplugging a USB mic)
- Plugin is dormant when not in use — zero overhead, just a loaded but inactive bundle
- If the user had our device selected in Zoom and restarts our app, Zoom will re-discover the device automatically (Core Audio sends device-list-changed notifications)

**Device routing at runtime:**
```rust
struct AudioRouter {
    // Switchable by user mid-call via UI dropdowns
    real_input_device: AudioDeviceID,   // which hardware mic
    real_output_device: AudioDeviceID,  // which hardware speaker

    // Fixed — our HAL plugin, always the same
    virtual_input_device: AudioDeviceID,
    virtual_output_device: AudioDeviceID,
}
```

**Key concerns:**
- Clock sync between real and virtual devices (virtual slaves to real device's sample clock)
- Sample format matching (force 48kHz float32 across all devices)
- Hotplug handling — watch `kAudioObjectPropertySelectorWildcard` for device changes
- Fallback to system default if active device disappears

**Phased approach:** Start with BlackHole as a stand-in (Phase 1a), replace with custom HAL plugin for seamless UX (Phase 1b).

### Layer 2: Local STT
- Whisper.cpp (large-v3) on Apple Silicon
- Chunked processing (~5-10s segments)
- Timestamped, speaker-labeled output

### Layer 3: Live AI
- Transcript chunks sent to Claude API based on active toggles
- Fact-checking, Q&A, suggestions, commitment detection
- All calls logged with tokens/cost/latency

### Layer 4: Post-Analysis
- Full transcript → Claude API with structured extraction prompt
- Returns: actions with owners/deadlines, decisions, summary
- User reviews and confirms before saving

## Phased Build Plan

### Phase 1a: Core - Audio Prototype + Transcription + Basic Shell
- Tauri v2 project setup
- Audio capture using BlackHole as a stand-in virtual device (user manually configures aggregate devices)
- whisper-rs integration for local STT
- Sidebar + main area layout
- Basic new session → live transcript → end flow
- Device selector dropdowns (real mic/speaker) in toolbar
- SQLite setup
- **Validates**: audio proxy concept works, whisper runs well on Apple Silicon, device switching works

### Phase 1b: Custom HAL Audio Plugin
- Write Core Audio `AudioServerPlugIn` in C/C++ — virtual mic + virtual speaker
- Dynamic device lifecycle — plugin publishes/removes devices via `AudioObjectsPublishedAndDied()` based on app connection state (no phantom devices when app isn't running)
- Shared memory IPC (mmap'd ring buffer) between plugin and Rust app
- Heartbeat/disconnect detection so devices vanish on app crash
- Clock synchronization with real hardware devices
- Installer/uninstaller for the `.driver` bundle
- Code signing + notarization
- Replaces BlackHole — user just selects "Call Assistant" devices, no manual setup
- **Validates**: seamless device proxy, dynamic device lifecycle, no glitches, works with Teams/Meet/Zoom

### Phase 2: Session Management + Post-Analysis
- Project CRUD in sidebar
- Sessions stored and browsable
- Post-analysis screen: AI extraction of actions/decisions/summary
- Action review and editing before save
- Session review (State 4)

### Phase 3: Actions + Live AI
- Consolidated actions view (State 5)
- Action status management (pending/done/won't do/etc.)
- Live AI features: fact-check, Q&A, suggestions, commitment detection
- Toggle row in AI panel
- API cost tracking and budget cap

### Phase 4: Polish
- **Session playback** - audio player synced with transcript scrolling, bookmark/action markers on timeline
- Speaker diarization
- Session templates
- "Prepare for Next Call" AI briefing
- Menu bar presence
- Full-text search across sessions
- Keyboard shortcuts
- MCP server for Claude Desktop

## Key Risks

- **macOS audio capture**: SIP restrictions, permissions, notarization
- Reference: Core Audio Taps API, AudioTee library (Talat)
- **Whisper latency**: may need medium model for real-time, large for post-analysis
- **API cost control**: budget enforcement to avoid surprise bills

## Research References

### Bot-free tools (market reference)
- **Granola** - local recording, MCP server, Recipes (~$14-18/month)
- **Jamie** - bot-free, deletes audio after transcription
- **Krisp** - virtual mic/speaker for noise cancellation + recording

### Bot-based tools (market reference)
- **Fireflies.ai** - MCP server, AskFred query feature
- **Fathom** - generous free tier, visible bot joins call
- **MeetGeek** - flexible bot/no-bot modes
