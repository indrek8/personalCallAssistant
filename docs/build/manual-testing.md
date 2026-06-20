# MVP Build — Manual Test Plan

> Hand-run verification of what's actually wired, milestone by milestone. Grounded
> in the code as merged (M2 closeout, `dd846c7`), **not** the end-state intent in
> [flows.md](flows.md) — where the build is intentionally behind the spec, this doc
> says so. Read alongside [milestones.md](milestones.md) (what's built) and
> [flows.md](flows.md) §9 (the EXC-* contract these cases map to).
>
> **Current coverage: M0–M2.** M3 (Live AI), M4 (post-analysis), M5 (manage/polish)
> sections get appended as those milestones land.

---

## 0. What is real vs. stubbed today (read this first)

So you don't chase ghosts or file built-as-designed gaps as bugs.

**Real & worth testing (M1–M2):**
- Onboarding wizard flow + first-run routing
- Whisper model download (resumable) — onboarding **and** Settings
- Pre-flight gate (mic / loopback device / model present)
- **Dual-stream capture → live two-sided transcript** (You/Remote), the core
- Pause / resume (timer excludes paused time; clean utterance boundary)
- End → session saved `completed`, survives restart
- Crash recovery on boot (`EXC-CRASH`)
- Device-drop fallback (`EXC-DEV-DROP`)
- Ground-truth files on disk (stereo WAV + `transcript.jsonl`)
- Settings persistence (capture device, model, default toggles)
- Dashboard session **list** + name search

**Stubbed / not wired yet — expected, do not log as bugs:**

| Area | Today | Lands in |
|---|---|---|
| AI panel in Live ("Live Intelligence") | Inert placeholder; "Ask AI" disabled | M3 |
| API key | Not validated, stored, or used. "Test" advances/does nothing; Settings shows a fake "Connected" | M3 |
| Cost meter in Live | Absent | M3 |
| **End Session** | Goes **straight to Dashboard** as `completed` (transcript only). The Post-Analysis screen is unreachable mock scaffolding | M4 |
| **Dashboard detail pane** (summary / actions / transcript body) | **Hardcoded "CBUAE" mock**, identical for every session. Header (name/date/duration/cost) is real | M5 |
| "Re-analyze" / "Open playback" buttons | Non-functional (Open playback wrongly routes to Live) | M4/M5 |
| Labels | New Session always attaches **Acme + Board**; "+ add" is inert; dashboard filter chips are hardcoded All/Acme/Globex | M5 |
| "Reveal in Finder" (Settings → Storage) | Inert | M5 |

> The single biggest gotcha: **after a real capture, the Dashboard detail will show
> the CBUAE mock, not your transcript.** To verify your transcript, watch the Live
> view during capture, or open `transcript.jsonl` on disk (§8).

---

## 1. Prerequisites (one-time)

**Build toolchain**
- Rust (stable) + `cmake` (`brew install cmake`) — `whisper-rs` compiles whisper.cpp
  natively. **First build is slow** (compiles a C++ lib + downloads crates); later
  builds are cached.

**Audio (required for the remote/"Remote" side)**
1. `brew install blackhole-2ch` — then **reboot** (the macOS audio HAL won't expose
   BlackHole as an input until you restart).
2. Open **Audio MIDI Setup** → **+** → **Create Multi-Output Device** → check **both**
   your headphones **and** "BlackHole 2ch".
3. Set that Multi-Output as the **system output** (or the meeting app's speaker).
4. **Wear headphones.** On open speakers your mic re-captures the remote audio → echo
   + double transcription.

**Model**
- A Whisper model must be downloaded before capture. Default is **`medium`** (~1.5 GB);
  **`small`** (~466 MB) is the faster floor. Download happens in onboarding step 3 or
  Settings → Transcription (no need to pre-fetch).

**No API key needed for M0–M2** — nothing here calls Claude.

---

## 2. How to run

```sh
npm install                 # first time only
npm run tauri dev           # the real app (Rust backend + WebView)
```

- First launch triggers the **macOS microphone-permission** prompt on first capture —
  allow it (Terminal/your IDE is the grantee in dev).
- **Browser-only preview:** `npm run dev` opens the frontend in a browser with **mock
  data and no backend** (`isTauri()` is false). Good for visual/layout checks only —
  every IPC-backed case below requires `npm run tauri dev`.

**Reset tricks**
- **Re-run onboarding:** quit, delete `~/Library/Application Support/CallAssistant/settings.json`
  (default `first_run` is `true`, so it regenerates into onboarding). 
- **Clean slate:** delete the whole `~/Library/Application Support/CallAssistant/` dir
  (wipes settings, sessions, downloaded models).

**Optional automated pre-checks** (fast confidence before manual work):
```sh
cd src-tauri && cargo test      # 20 unit tests (VAD, resampler, recovery, model mgr…)
cd src-tauri && cargo clippy    # should be clean
npm run check                   # svelte-check (types)
```

---

## 3. Test cases

Each case: **Steps → Expected → ✅ Pass when**. Check the box when it passes.

### T1 — First-run onboarding & routing
- [ ] **Steps:** Clean slate (§2 reset). Launch. Walk all 4 steps: Connect Claude
  (paste anything or "Skip for now") → Audio device (pick your mic) → Transcription
  model (pick + download `small` to keep it quick) → "Go to dashboard".
- **Expected:** Each step advances; the model step shows a **live download %** then
  flips to "✓ ready"; finishing lands on the Dashboard (empty state "No sessions yet").
  Relaunching goes **straight to Dashboard** (no onboarding).
- ✅ **Pass when:** setup completes once and never re-prompts; `settings.json` now has
  `first_run: false` and your chosen `whisper_model`.
- **Note:** the API-key field is cosmetic (M3) — "Test & continue" and "Skip for now"
  behave identically and the key is not saved.

### T2 — Model management
- [ ] **Steps:** Settings (gear) → Transcription. Download the model you didn't already
  get. Optionally kill Wi-Fi mid-download for ~10 s, then restore.
- **Expected:** Progress %, then "✓ downloaded". Interrupted downloads **resume** (don't
  restart from 0) and survive transient network drops; the file only appears as ready
  once it passes ggml-magic validation. `base` is intentionally **not** offered (only
  Small + Medium).
- ✅ **Pass when:** model shows downloaded and a later capture uses it without re-fetch.

### T3 — Pre-flight gate (`EXC-NODEV` / `EXC-MODEL`)
- [ ] **Steps:** Dashboard → New Session → Start, under three conditions:
  1. Everything present (mic + BlackHole + model).
  2. Model **not** downloaded (pick a model in Settings you haven't fetched, or clear it).
  3. BlackHole **absent** (quit, `brew uninstall blackhole-2ch` or just test on a Mac
     without it) — or simpler, confirm the check exists by reading the failing row.
- **Expected:** (1) proceeds to Live. (2) blocks with a red **"Transcription model"** row
  and an inline **Download** button (downloads in place, then Start works). (3) blocks
  with a red **"Loopback device — set up the Multi-Output device"** row.
- ✅ **Pass when:** Start is gated exactly on mic + loopback + model; a fixable failure
  offers the fix and clears on retry **without** creating duplicate sessions.
- **Note:** there is **no** API-key or disk-space check in pre-flight yet (M3/§4 of flows).

### T4 — Capture → live two-sided transcript (the core)
- [ ] **Steps:** Start a session. Speak a few sentences into your mic (the **You** side).
  Separately, play speech audio — a YouTube clip / podcast / a real Zoom or Meet call —
  routed through the Multi-Output device (the **Remote** side). Pause ~1 s between
  sentences.
- **Expected:** Lines stream into the transcript, newest at the bottom, **auto-scrolling**.
  Your speech is labelled **You** (gold); the routed audio is **Remote** (teal/blue).
  A line appears shortly after you stop a sentence (silence gap ≈ 0.6 s + fast Whisper
  inference); a long unbroken monologue is force-cut into pieces (~12 s max). The timer
  counts up; "Listening…" pulses.
- ✅ **Pass when:** both sides transcribe with correct You/Remote attribution and **no
  cross-bleed** (your voice never tagged Remote or vice-versa), within ~10 s of speech.
- **Tip:** if Remote stays empty, the Multi-Output isn't the active output or doesn't
  include BlackHole. If You stays empty, wrong mic / mic permission denied.

### T5 — Pause / resume
- [ ] **Steps:** Mid-capture, click **Pause**. Wait ~15 s (stay quiet, or talk — it
  should be ignored). Click **Resume** and speak again.
- **Expected:** Header flips to **PAUSED** (dot stops pulsing), the **timer freezes**,
  and audio during the pause is **not** captured or transcribed. On resume it returns to
  **REC** and the timer advances again; the sentence in progress at pause time is closed
  off cleanly (not fused with post-resume speech).
- ✅ **Pass when:** paused wall-time is excluded from the final duration and no
  pause-gap audio leaks into the transcript.

### T6 — End → save → persist
- [ ] **Steps:** Click **End** → confirm the "End this session?" dialog. Then fully quit
  and relaunch the app.
- **Expected:** End returns to the **Dashboard** (not a post-analysis screen — that's
  M4). The session appears in the left list with its **name, date, and recorded
  duration**. After relaunch it's still there.
- ✅ **Pass when:** the session persists across restart with a sensible duration.
- **Reminder:** clicking it shows the **mock** CBUAE detail (see §0) — that's expected;
  verify the real transcript via §8 instead.

### T7 — Crash recovery (`EXC-CRASH`)
- [ ] **Steps:** Start a session, speak for ~20 s so there's real audio + transcript,
  then **Force Quit** the app (⌥⌘Esc → Call Assistant, or `kill -9` the process) — do
  **not** click End. Relaunch.
- **Expected:** On boot a banner appears: *"Recovered a session that was interrupted —
  it's been saved to your list."* The crashed session is in the list as **completed**
  with a duration derived from the audio on disk. `audio.wav` and `transcript.jsonl`
  are intact (incremental writes flush ~1×/s, so at most ~1 s of tail is lost).
- ✅ **Pass when:** no session is left stuck "recording"; the WAV plays and the
  transcript file is readable after the crash.

### T8 — Device drop fallback (`EXC-DEV-DROP`)
- [ ] **Steps:** Start a session using a **removable mic** (USB mic, or AirPods/BT
  headset as the input). Mid-capture, unplug / disconnect it.
- **Expected:** A red banner: *"You input disconnected — switched to {device}"*, the
  audio-device list refreshes, and **capture keeps going** on the default input — the
  session is never lost. (Rebuild is retry-capped at 5/side; if no input is available
  at all you get a "…no fallback available" banner and that side goes silent.)
- ✅ **Pass when:** a mid-call unplug degrades gracefully without freezing or ending the
  session.
- **Known quirk:** fallback always targets the default **input** device, so dropping the
  *BlackHole* (Remote) side rebuilds it onto your mic rather than a loopback — fine for
  not-crashing, but Remote audio won't resume until BlackHole is back and a new session
  starts. Seamless hot-swap is a v1/HAL concern.

### T9 — Ground-truth files on disk
- [ ] **Steps:** After any capture, open
  `~/Library/Application Support/CallAssistant/sessions/{uuid}/` (newest folder).
- **Expected:**
  - `audio.wav` — **stereo 16-bit, L = You, R = Remote**. Play it: `afplay audio.wav`,
    or open in a player and pan hard-left (only your voice) / hard-right (only remote).
  - `transcript.jsonl` — one JSON object per line:
    `{"id","t_ms","stream":"you"|"remote","text","confidence"}`.
  - `metadata.json` — `status: "completed"`, your name/labels/duration.
- ✅ **Pass when:** L/R attribution in the WAV matches the You/Remote labels you saw live,
  and every transcript line is valid JSON.

### T10 — Settings persistence
- [ ] **Steps:** Settings → change capture device, switch the active Whisper model,
  toggle the F/C/S/Q default chips. Quit and relaunch; reopen Settings and start a New
  Session.
- **Expected:** Device, model, and toggle defaults all **persist** across restart; New
  Session pre-selects the saved device and pre-lights the saved toggles. (The toggles
  don't *do* anything yet — they drive M3 live AI — but they must round-trip.)
- ✅ **Pass when:** all three survive a restart.
- **Note:** the "Test" (API key) and "Reveal in Finder" buttons are inert (M3/M5).

### T11 — Dashboard list & search
- [ ] **Steps:** With ≥2 sessions, type part of a session name in the top search box.
- **Expected:** The **list** is real (names, dates, durations, the Acme/Board label
  chips, newest first) and filters live by name as you type.
- ✅ **Pass when:** search narrows the real list.
- **Expected-mock (not a bug):** the detail pane body, the All/Acme/Globex segment
  chips, and Re-analyze are placeholder/non-functional until M5.

### T12 — Quiet & edge behavior
- [ ] **Silence (`EXC-SILENCE`):** Start a session and stay silent. **Expected:** nothing
  is transcribed (correct), "Waiting for speech…" / "Listening…" persists, ending still
  yields a valid (empty-transcript) session.
- [ ] **Corrupt row:** hand-edit a session's `metadata.json` to invalid JSON. **Expected:**
  that session is silently **skipped** from the list (no crash). Full "⚠ unreadable" row
  UI is M5.

---

## 4. A fast smoke pass (~5 min, after a code change)

If you just want "did I break the engine": **T4** (capture → two-sided transcript) →
**T5** (pause/resume) → **T6** (end + persist). That exercises the whole hot path
(cpal → resample → VAD → Whisper → JSONL → events → UI → storage).

No-hardware subset (no BlackHole/mic set up): `cargo test` + **T1**, **T2**, **T10**,
**T11** — onboarding, model download, settings, list/search all work without audio.

---

## 5. Where the data lives

```
~/Library/Application Support/CallAssistant/
├── settings.json                     # capture_device_id, whisper_model, default_toggles, first_run…
├── models/  ggml-{small|medium}.bin  # downloaded on demand (gitignored-size blobs)
└── sessions/{uuid}/
    ├── metadata.json                 # status, name, labels[], date, duration_ms…
    ├── audio.wav                     # stereo 16-bit: L=you, R=remote
    └── transcript.jsonl              # one {id,t_ms,stream,text,confidence} per line
```

(`ai_live.json`, `chat.json`, `analysis.json` appear with M3/M4.)

---

## 6. Mapping to the EXC-* contract

| Case | flows.md §9 code | Status |
|---|---|---|
| T3 | EXC-NODEV, EXC-MODEL | ✅ implemented |
| T3 (loopback heuristic) | EXC-NOMULTI (soft warn) | partial — hard fail on missing device; soft "may only hear your side" warn is lighter than spec'd |
| T7 | EXC-CRASH | ✅ implemented (routes to `completed`, not POST_PROCESS yet — M4) |
| T8 | EXC-DEV-DROP | ✅ implemented (default-input fallback) |
| T12 | EXC-SILENCE, EXC-CORRUPT | ✅ silence; EXC-CORRUPT degrades-by-omission (full UI M5) |
| — | EXC-KEY, EXC-MIC-PERM (block), EXC-WHISPER-LAG (UI), EXC-BUDGET, EXC-API-* | M3+ |
```
