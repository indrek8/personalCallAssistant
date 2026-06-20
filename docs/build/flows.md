# MVP Build — Flows & States

> The complete behavioral specification: every screen state, every user flow (happy path), and every exception with its recovery. Read alongside **[technical-design.md](technical-design.md)** (how it's wired) and **[milestones.md](milestones.md)** (build order). Scope and decisions: **[../mvp.md](../mvp.md)**, **[README.md](README.md)**.

---

## 1. App State Machine (top level)

The window is always in exactly one of these modes. Onboarding and the two session-takeovers are *modal* — you can't freely navigate out.

```
            ┌──────────────┐
 launch ───►│  BOOTING     │  load settings, check first-run
            └──────┬───────┘
        first run? │
        ┌──────────┴───────────┐
        ▼ yes                  ▼ no
  ┌────────────┐         ┌────────────────┐
  │ ONBOARDING │ ──done─►│   DASHBOARD     │◄──────────────┐
  └────────────┘         └───┬──────┬──────┘               │
                   [+ New]   │      │  [gear]              │
                             ▼      ▼                      │
                    ┌─────────────┐ ┌──────────┐           │
                    │ NEW_SESSION │ │ SETTINGS │──back─────┤
                    └──────┬──────┘ └──────────┘           │
                    Start  │  (pre-flight ok)              │
                           ▼                               │
                    ┌──────────────┐                       │
                    │ LIVE         │  (modal takeover)     │
                    └──────┬───────┘                       │
                       End │                               │
                           ▼                               │
                    ┌──────────────┐   Save & Close        │
                    │ POST_PROCESS │ ──────────────────────┘
                    └──────────────┘
```

| From | Trigger | To | Guard / notes |
|---|---|---|---|
| BOOTING | settings loaded, `first_run=false` | DASHBOARD | — |
| BOOTING | `first_run=true` | ONBOARDING | no API key / never completed setup |
| ONBOARDING | finish | DASHBOARD | writes `settings.json`, `first_run=false` |
| DASHBOARD | `[+ New Session]` | NEW_SESSION | — |
| DASHBOARD | `[gear]` | SETTINGS | — |
| NEW_SESSION | `[Start]` | LIVE | **pre-flight must pass** (§4) |
| NEW_SESSION | `[Back]` / Esc | DASHBOARD | discards the draft |
| LIVE | `[End]` (confirm) | POST_PROCESS | — |
| LIVE | crash / force-quit | (recovered next launch) | see EXC-CRASH |
| POST_PROCESS | `[Save & Close]` | DASHBOARD | session → `completed` |
| POST_PROCESS | `[Discard]` (confirm) | DASHBOARD | session deleted |
| SETTINGS | `[Back]` | DASHBOARD | — |

**Rule:** only one LIVE session may exist at a time. `[+ New Session]` is disabled while a session is live.

---

## 2. Session Lifecycle (the session's own status)

Stored in each session's `metadata.json` as `status`. Drives recovery and the dashboard badges.

```
            create()                 Start
   (none) ──────────► draft ──────────────► recording ⇄ paused
                                               │  (Pause/Resume)
                                          End  │
                                               ▼
                                            ending ──► analyzing ──► reviewing
                                                          │  (Sonnet)   │ Save
                                                          │             ▼
                                              analysis fail│         completed
                                                          ▼
                                                       reviewing (manual)
   any state ── unclean exit ──► (on next launch) recovering ──► reviewing | completed
```

| Status | Meaning | Persisted artifacts so far |
|---|---|---|
| `draft` | form filled, not started | `metadata.json` only |
| `recording` | actively capturing | + `audio.wav`, `transcript.jsonl`, `ai_live.json` (all appended live) |
| `paused` | capture suspended | same; capture threads idle |
| `ending` | capture stopped, finalizing files | flushing buffers |
| `analyzing` | post-analysis API call in flight | — |
| `reviewing` | analysis done (or skipped), user editing | + `analysis.json` (draft) |
| `completed` | saved | `analysis.json` final |
| `failed` | unrecoverable error during capture | partial files; surfaced for inspection |

---

## 3. Flow A — First Launch & Onboarding

A 4-step wizard. Each step validates before **Continue** unlocks. Everything is skippable except it warns that the app won't capture without setup.

1. **Welcome** — value prop, `[Get started]`.
2. **Connect Claude** — paste API key → `[Test]` makes a 1-token ping to the Messages API.
   - ✓ valid → store in Keychain, show "Connected".
   - ✗ invalid/network → inline error (§9 EXC-KEY), stay on step.
3. **Audio setup** — explains the one-time Multi-Output Device setup (§10), offers `[Open Audio MIDI Setup]`, and a capture-device dropdown (populated from `list_audio_input_devices`). Detects whether a "Call Assistant"/BlackHole device exists; if not → guided install link (EXC-NODEV).
4. **Transcription model** — choose `base`/`small`/`medium`. If the chosen model file isn't present → triggers a download with progress (EXC-MODEL). Shows estimated speed note.
5. **Done** → write settings, `first_run=false`, go to DASHBOARD (empty state).

**Exceptions:** EXC-KEY, EXC-NODEV, EXC-MODEL, EXC-MIC-PERM (mic permission is requested lazily on first capture, not here).

---

## 4. Flow B — New Session & Pre-Flight

The form (name, labels, participants, context, F/C/S/Q toggles) is cheap. The important part is the **pre-flight gate** that runs when `[Start]` is pressed — we never enter LIVE in a broken state.

**Pre-flight checks (in order, fail-fast with a fix-it prompt):**

| # | Check | If it fails |
|---|---|---|
| 1 | API key present & marked valid | Block, link to Settings (EXC-KEY) |
| 2 | Capture (BlackHole) device exists | Block, guided setup (EXC-NODEV) |
| 3 | Mic permission granted | Trigger macOS prompt; if denied → block with instructions (EXC-MIC-PERM) |
| 4 | Whisper model file present | Offer download now (EXC-MODEL) |
| 5 | Disk space > threshold (e.g. 500 MB) | Warn, allow continue (EXC-DISK) |
| 6 | Budget cap > $0 (if set) | informational |

All pass → create session dir, `status=recording`, start capture, transition to LIVE. The "context for AI" + active toggles are captured into the session and into the live-AI system prompt.

> **Soft warning, not a block:** if no Multi-Output Device is detected as the system/meeting output, warn "we may only hear your side" — but allow it (the user may be testing, or only wants their own notes).

---

## 5. Flow C — Live Session (the core loop)

LIVE is a continuous pipeline. From the user's view it's "talk, watch transcript + insights appear." Underneath, several sub-flows run concurrently (threading in [technical-design.md](technical-design.md) §3).

### C1 — Capture & transcript (always on while `recording`)
1. Two `cpal` streams capture in parallel: **mic → "You"**, **BlackHole → "Remote"**.
2. Each stream is downmixed/resampled to 16 kHz mono, teed to (a) `audio.wav` writer, (b) a VAD segmenter.
3. On an utterance boundary (silence gap or max-length), the segment → Whisper → a `transcript_entry { t, stream:"you"|"remote", text, confidence }`.
4. Entry is **appended to `transcript.jsonl`** and **emitted** (`transcript-entry` event) → appears in the UI, color-coded You vs Remote, auto-scrolling.

### C2 — Live AI findings (only while any toggle is on)
1. A batcher accumulates new entries; fires when **≥5 new entries OR ≥30 s** since last fire.
2. Sends `{recent window + rolling context + session context notes + active toggles}` → **Haiku** → structured JSON findings.
3. Each finding (fact-check / commitment / suggestion / unanswered) is appended to `ai_live.json`, cost added, and emitted (`ai-finding`, `cost-update`) → renders in the AI panel feed.
4. **Commitment** findings render a `[+ Save action]` button → on click, creates a draft action (C5).
5. If all toggles are off → batcher idle, **zero API calls**.

### C3 — Ask AI (on demand)
1. User types a question → `ask_ai` command with `{full transcript so far + context + question}` → **Sonnet**.
2. Response streamed/returned, rendered visually distinct from auto-findings, appended to `chat.json`, cost updated.

### C4 — Pause / Resume
- **Pause:** capture threads stop feeding WAV + Whisper; `status=paused`; timer pauses; AI batcher idles. The meeting itself is unaffected (we're passive).
- **Resume:** threads resume; a small gap marker is inserted in the transcript.

### C5 — Toggles & save-action mid-call
- Toggling F/C/S/Q updates the live-AI system prompt for the *next* batch (no retroactive re-analysis).
- `[+ Save action]` on a commitment → adds to an in-memory action draft list (finalized in post-analysis), with the source quote + timestamp.

### C6 — End
1. `[End]` → confirm dialog ("End this session?").
2. On confirm: `status=ending`, stop capture, flush WAV + last Whisper segment, persist final `transcript.jsonl`, compute duration → transition to POST_PROCESS.

**LIVE exceptions:** EXC-DEV-DROP (mic/output disconnect), EXC-WHISPER-LAG (backpressure), EXC-API-LIVE (Haiku/Sonnet failure), EXC-BUDGET (cap hit), EXC-SILENCE (no speech), EXC-SLEEP (system sleep), EXC-CRASH.

---

## 6. Flow D — End & Post-Analysis

> ✅ **Implemented in M4.** End finalizes to `ending`; the Post screen calls `run_post_analysis`
> (`analyzing → reviewing`), and Save & Close completes. EXC-CLOSE-DURING is simplified for M4:
> a crashed `analyzing` recovers to `completed` transcript-only, and a quit mid-`reviewing` keeps its
> draft `analysis.json` (D20) — recover-into-review is M5. See [m4-plan.md](m4-plan.md).

1. **Analyzing** (`status=analyzing`): full `transcript.jsonl` + context + live annotations → **Sonnet** → `{summary, actions[], decisions[], key_topics[]}`. Spinner with session name/duration. Typically 10–30 s.
2. Live-detected commitments + any `[+ Save action]` items are **merged & de-duplicated** with Sonnet's extracted actions.
3. **Reviewing** (`status=reviewing`): editable summary (`[Regenerate]`), action rows (check/uncheck, owner, due date, source quote, delete, `[+ Add manually]`), decisions list.
4. `[Save & Close]` → write final `analysis.json`, `status=completed`, return to DASHBOARD (new session selected).
5. `[Back to Transcript]` → read-only transcript overlay for reference.

**Exceptions:** EXC-API-POST (analysis fails → offer Retry / Save-without-analysis / keep transcript only), EXC-EMPTY (transcript empty or < N words → skip analysis, go straight to a minimal reviewing state), EXC-CLOSE-DURING (user quits during analyzing → on relaunch, resume at `reviewing` with whatever exists, or re-offer analysis).

---

## 7. Flow E — Review, Manage & Re-analyze

From DASHBOARD (mail-inbox split):
- **Select session** → right pane: summary, actions (with live status), transcript. Loaded from disk.
- **Update action status** inline (pending → in progress → done → won't do → postponed) → patched into `analysis.json`.
- **Filter** by label; **search** by name (MVP: in-memory over loaded metadata).
- **Re-analyze** → re-runs post-analysis on the stored transcript with the current prompt (overwrites `analysis.json` after confirm).

**Exceptions:** EXC-CORRUPT (a session's JSON won't parse → show the row as "⚠ unreadable", offer Reveal in Finder, don't crash the list), EXC-NOAUDIO (audio.wav missing → review still works, playback disabled — note: playback itself is post-MVP).

---

## 8. Flow F — Settings & Model Management

Sections: API & AI (key, test, default toggles), Audio (capture device), Transcription (Whisper model + download state), Storage (path, reveal).
- **Change API key** → re-test before saving.
- **Change model** → if not downloaded, download with progress; switching mid-app is fine (applies to next session).
- **Change capture device** → re-enumerated live (hotplug aware).

**Exceptions:** EXC-KEY, EXC-MODEL, EXC-NODEV.

---

## 9. Exception & Recovery Catalogue

The contract for everything that can go wrong. Every one has a defined detection point, user-facing behavior, and resulting state — nothing silently fails.

| ID | Condition | Detection | User-facing behavior | Recovery / resulting state |
|---|---|---|---|---|
| EXC-KEY | Missing/invalid API key | `[Test]`, pre-flight, or 401 | Inline error; link to Settings | Blocked until valid |
| EXC-MIC-PERM | Mic permission denied | Pre-flight / capture start | Modal: how to enable in System Settings; `[Open]` deep-link | Blocked; re-check on return |
| EXC-NODEV | No BlackHole/Call Assistant device | Device enumeration | Guided install + setup steps (§10) | Blocked until present |
| EXC-NOMULTI | No Multi-Output configured | Heuristic at start | **Soft warn** "may only hear your side" | Allowed to continue |
| EXC-MODEL | Whisper model file absent | Pre-flight / settings | Download w/ progress; cancelable | Blocked until present or smaller model chosen |
| EXC-DEV-DROP | Active mic/output disappears mid-call | cpal device-change callback | Toast "Mic disconnected — switched to {default}"; auto-fallback to system default; keep recording | Stays `recording`; logged in transcript as a gap |
| EXC-WHISPER-LAG | Whisper can't keep up (queue grows) | Segment queue depth > threshold | Subtle "transcription catching up…" indicator; never drop audio (WAV is ground truth) | Queue drains; entries arrive late but complete |
| EXC-API-LIVE | Haiku/Sonnet live call fails (429/5xx/timeout) | HTTP result | Small non-blocking "AI paused — retrying" chip; exp. backoff | Transcript unaffected; findings resume on success; after N fails, auto-disable that feature with a notice |
| EXC-BUDGET | Session cost hits `budget_cap` | After each AI call | Banner "Budget reached — live AI paused"; transcript continues | Live AI off; user can raise cap to resume |
| EXC-SILENCE | Long stretch with no speech | VAD | Nothing transcribed (correct); "Listening…" persists | No-op |
| EXC-SLEEP | System/display sleeps mid-call | App lifecycle / device stop | On wake: detect gap, attempt to resume capture; if device lost → EXC-DEV-DROP | Resume or fallback |
| EXC-API-POST | Post-analysis call fails | HTTP result | Dialog: `[Retry]` / `[Save without analysis]` / `[Back to transcript]` | Session still saveable as `reviewing`→`completed` w/ empty analysis |
| EXC-EMPTY | Transcript empty / too short at End | Word-count check | Skip Sonnet; minimal review ("Nothing substantial captured") | Saveable or discardable |
| EXC-DISK | Disk write fails / full | Write error | Toast + pause recording to protect data | `failed` if unrecoverable; partial files kept |
| EXC-CRASH | App quit/crash while `recording`/`analyzing` | Stale `status` on next launch | On launch: "Recovered an unsaved session" → open it in POST_PROCESS using whatever was persisted | WAV+transcript salvaged (incremental writes); offer (re)analysis |
| EXC-CORRUPT | Session JSON won't parse | On dashboard load | Row shows "⚠ unreadable"; `[Reveal in Finder]` | List keeps working; no crash |

**Design principle:** the **WAV file and `transcript.jsonl` are ground truth and are written incrementally**, so every failure mode degrades to "you still have the recording and the transcript."

---

## 10. Permissions & One-Time System Setup

These are macOS realities the UX must hand-hold (until the v1 HAL plugin removes them).

**A. Microphone permission** — requested lazily on first capture via `AVCaptureDevice`/cpal. If denied, EXC-MIC-PERM deep-links to System Settings → Privacy → Microphone.

**B. The Multi-Output Device (to hear *and* capture the remote side):**
1. Open **Audio MIDI Setup** → create a **Multi-Output Device** = `[BlackHole 2ch] + [your headphones]`.
2. Set that Multi-Output as the **meeting app's speaker** (or the macOS system output).
3. Result: remote audio reaches your ears **and** BlackHole; we capture BlackHole as the "Remote" stream; your real mic is the "You" stream.
4. **Use headphones** — on open speakers, your mic re-captures the remote audio → echo/double transcription.

> The app can't fully automate this on the MVP (no driver), but onboarding shows the exact steps with an `[Open Audio MIDI Setup]` button, and pre-flight detects the missing pieces (EXC-NODEV, EXC-NOMULTI).

**C. Whisper model download** — models aren't bundled (size). First use downloads the chosen model to the app-support dir with progress + checksum (EXC-MODEL).
