# MVP Build — Manual Test Plan

> Hand-run verification of what's actually wired, milestone by milestone. Grounded
> in the code as merged (through M3 + the post-closeout hardening pass), **not** the end-state intent in
> [flows.md](flows.md) — where the build is intentionally behind the spec, this doc
> says so. Read alongside [milestones.md](milestones.md) (what's built) and
> [flows.md](flows.md) §9 (the EXC-* contract these cases map to).
>
> **Current coverage: M0–M3.** M4 (post-analysis), M5 (manage/polish)
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
cd src-tauri && cargo test      # 104 unit tests (VAD, resampler, recovery, model mgr, AI client/retry/SSE, post-analysis, labels/delete…)
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
- **Note:** pre-flight now includes the **API-key presence** check (M3, EXC-KEY). There's still **no disk-space check** (§4 of flows), and it doesn't guard against the mic and loopback resolving to the **same device** (a misconfiguration where the system default input *is* BlackHole) — a known limitation.

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
| — | EXC-MIC-PERM (block), EXC-WHISPER-LAG (UI) | M3+ |

---

## M3 — Live AI is now real (PRs #9–#12)

The §0 "stubbed" rows for the **AI panel**, **API key**, **cost meter**, and the
inert Ask-AI bar are now live. All cases below need a Claude API key set in Settings
(or `ANTHROPIC_API_KEY` in the env as a dev fallback). New per-session files:
`ai_live.json`, `chat.json`, `saved_actions.json`.

### M3-1 — API key (PR1)
- [ ] Settings → API & AI → paste a real key → **Test** → `Connected · claude-haiku-4-5 · saved to Keychain`; relaunch → "Key stored in Keychain". With no key, pre-flight **blocks Start** (EXC-KEY).

### M3-2 — Live findings (PR2)
- [ ] Start with **F** + **C** on. Speak a commitment ("I'll send the report by Friday") and a claim that contradicts your prep notes → a **Commitment** and a **Fact-check** finding appear in the panel within a batch cycle (≤30 s); the cost meter ticks up.
- [ ] Toggle **all four off** → **zero further API calls** (verify in the dev console). Toggle back on → analysis resumes (no retroactive re-run).
- [ ] Kill Wi-Fi mid-call → the transcript keeps flowing; after 3 failed batches an **EXC-API-LIVE** banner appears and live AI goes quiet. Set a low budget cap → **EXC-BUDGET** pauses live AI at the cap.
- [ ] **End mid-batch** (click End right after speaking, while a Haiku call may be in flight) → the session finalizes promptly (≤ ~20 s worst case, not minutes); WAV + transcript are saved and a teardown-cancelled batch isn't logged as a failure.

### M3-3 — Ask-AI (PR3)
- [ ] Type "summarize what we've agreed so far" → the answer **streams in** word-by-word in the card above the bar; cost increments; `chat.json` is written.
- **Hardening:** a refusal or a `max_tokens` cut is surfaced (a clear "declined" message / a truncation note), and a dropped or mid-stream-errored stream shows an error — never a blank or silently clipped answer.

### M3-4 — Save action (PR4)
- [ ] Click `[+ Save action]` on a commitment → it flips to "✓ Saved" and a line is appended to `saved_actions.json` in the session folder (survives End → it's there for M4 to merge).

**EXC mapping update:** EXC-KEY, EXC-BUDGET, EXC-API-LIVE are now implemented (M3); see the cases above.

---

## M4 — Post-Analysis is now real (branch `feat/m4-post-analysis`)

End no longer jumps to the dashboard — it routes to the **Post-Analysis** screen. The §0
"End Session" stub row is now live (the Dashboard *detail pane* stays mock until M5). New
per-session file: `analysis.json`.

### T13 — End → process → review
- [ ] Run a real capture (T4) with a Claude key set, speak a couple of commitments + a fact that conflicts with your prep notes, then **End**.
- **Expected:** the screen switches to **Post-Analysis** with a brief "Analyzing your session…" spinner, then a review with a real **summary**, **extracted actions** (owners/deadlines/quotes — Sonnet + your saved/live commitments merged, no obvious dupes), **decisions**, and a meta rail whose cost now includes the Sonnet call — within ~30 s.
- ✅ **Pass when:** the analysis reflects what was actually said; a `[+ Save action]` you clicked live appears in the list.

### T14 — Edit / uncheck / add
- [ ] Edit the summary text. Uncheck an action. Change an owner + due date. Delete one. Click **+ Add action** and type a title.
- **Expected:** all edits are local until save; unchecked rows dim; the "N of M" count tracks included rows.
- ✅ **Pass when:** the controls behave and nothing is lost while editing.

### T15 — Save & Close → persist
- [ ] **Save & Close** → returns to the Dashboard with the session `completed`. Quit + relaunch.
- **Expected:** only the **checked** actions + edited summary were saved; the session survives restart with its analysis intact and the cost including the Sonnet call.
- ✅ **Pass when:** `~/Library/Application Support/CallAssistant/sessions/{id}/analysis.json` holds your edited result and `metadata.json` is `completed`.

### T16 — Analysis failure (`EXC-API-POST`)
- [ ] Kill Wi-Fi, then End a session (or End with no/invalid key).
- **Expected:** an **error panel** with **Retry analysis** / **Save without analysis** / **Back to dashboard**. *Save without analysis* still yields a `completed` transcript-only session; *Retry* re-runs once the network is back.
- ✅ **Pass when:** a failed analysis never loses the transcript and always leaves a saveable session.

### T17 — Empty session (`EXC-EMPTY`)
- [ ] End a session with little/no speech (< ~25 words).
- **Expected:** **no Sonnet call** — a minimal review ("Nothing substantial was captured…", no actions/decisions), still saveable.
- ✅ **Pass when:** a near-silent session skips the model and saves cleanly.

**EXC mapping update:** EXC-API-POST + EXC-EMPTY are now implemented (M4); see T13/T16/T17.

---

## M5 — Manage, Settings & Polish is now real (branch `feat/m5-manage-polish`)

The §0 stubbed rows for the **Dashboard detail pane**, **labels**, **Re-analyze**, and
**Reveal in Finder** are now live. The detail pane shows the **real** session (no more CBUAE
mock). New global file: `labels.json`.

### T18 — Real detail pane + inline action status
- [ ] Select a completed session → the right pane shows its **real** summary, actions (with
  owner/deadline/quote), decisions, and a collapsible transcript — loaded via `get_session`.
  Change an action's **status** via the inline dropdown.
- **Expected:** the status persists (re-fetched, no spinner flash) and survives quit + relaunch
  (`analysis.json` updated, `completed_at` set when → Done).
- ✅ **Pass when:** the pane reflects the actual session and inline status edits stick.

### T19 — Labels (full manager + picker + filter)
- [ ] **Manage labels** (Dashboard list head or Settings → Storage) → create "Acme" with a color,
  rename it, recolor it, see its **usage count**, delete one. In **New Session**, pick existing
  labels and **create-on-type** a new one. On the dashboard, click a **filter chip**.
- **Expected:** labels persist to `labels.json`; rename/recolor reflects on existing sessions;
  a deleted label still renders on old sessions (snapshot); the filter narrows the list by label.
- ✅ **Pass when:** label CRUD round-trips and filtering works.

### T20 — Re-analyze a stored session
- [ ] Open a completed session → **Re-analyze** → confirm.
- **Expected:** routes to Post, re-runs Sonnet, lets you edit → **Save & Close** overwrites the
  analysis. A forced failure (kill Wi-Fi first) leaves the session **`completed` with the old
  analysis intact** (D21).
- ✅ **Pass when:** Re-analyze overwrites on success and never corrupts a completed session on failure.

### T21 — Delete / Discard (confirm dialogs)
- [ ] Dashboard → select a session → **Delete** (confirm). In Post (after End or Re-analyze) →
  **Discard** (confirm). End a live session → the **End** confirm is the styled dialog (not the OS prompt).
- **Expected:** Delete/Discard remove the session folder from disk and the list; confirms are the
  in-app `ConfirmDialog`.
- ✅ **Pass when:** destructive actions require confirmation and fully remove the session.

### T22 — Recover-into-review (`EXC-CRASH`, D23)
- [ ] Reach the Post **review** state (End a real session, wait for the draft), then **Force Quit**.
  Relaunch.
- **Expected:** a sticky **toast** "Recovered a session mid-review" with a **Resume review** action
  → reopens the draft in Post **without re-billing** (status stayed `reviewing`). (A crash during
  *recording* still recovers as `completed` — see T7.)
- ✅ **Pass when:** a half-reviewed session resumes its draft instead of silently completing.

### T23 — Unreadable session row + Reveal in Finder (`EXC-CORRUPT`)
- [ ] Hand-edit a session's `metadata.json` to invalid JSON. Relaunch.
- **Expected:** that session shows as a **⚠ Unreadable session** row (folder name as id); selecting
  it offers **Reveal in Finder** + **Delete**; the rest of the list still works.
- ✅ **Pass when:** a corrupt session is surfaced (not skipped) and never crashes the list.

### T24 — Reveal in Finder + error toasts
- [ ] Settings → Storage → **Reveal in Finder** opens the storage dir. Trigger any handled error
  (e.g. a bad API key during live AI).
- **Expected:** Finder opens the `CallAssistant` folder; handled `app-error`s appear as **toasts**
  (sticky when non-recoverable) instead of only a banner.
- ✅ **Pass when:** Reveal opens the folder and errors surface as dismissible toasts.

**EXC mapping update:** EXC-CORRUPT is now a visible "⚠ Unreadable" row (M5); recover-into-review
(D23) routes a crashed `reviewing` session back to Post.
