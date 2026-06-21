# Personal Call Assistant

> A native **macOS** desktop app that acts as an invisible AI meeting assistant. It captures **both sides** of any call (Teams · Meet · Zoom) through a virtual audio device, transcribes **locally** with Whisper, gives **live AI analysis** during the call, and extracts **actions, decisions, and a summary** afterward. **No bot ever joins the meeting.**

Beyond recording, it's a **meeting-driven work tracker**: every call produces actions — what you promised, what others owe you, what got decided — linked back to the moment in the transcript.

**Core loop:** prepare → record → extract → track → prepare

**Status:** ✅ **v0.1 (MVP) software-complete (M0–M5)** — 104 unit tests + clippy + svelte-check green. The one remaining gate is a real on-device call. **Next: [v0.2 — Organization & Tracking](docs/roadmap.md#v02--organization--tracking).**

> 🎨 Want to see the UI first? Open **`design/prototype.html`** in a browser — a high-fidelity prototype of all six screens.

---

## What it does

- **Two-sided capture, no bot.** Your mic is **You**; the meeting's audio (routed through a BlackHole virtual device) is **Remote**. The meeting app never sees a bot — you just hear the call normally through your headphones.
- **Local, real-time transcription.** Whisper (`whisper-rs` / whisper.cpp) runs on-device — `medium` by default — labeling each line **You** vs **Remote** for free from the two streams (no diarization).
- **Live AI during the call.** Toggle **F**act-check · **C**ommitments · **S**uggestions · **Q**uestions (Haiku), or ask free-form questions of the transcript so far (**Ask AI**, Sonnet, streamed). A running cost meter rides along; all toggles off → **zero** API calls.
- **Post-call analysis you review before saving.** End the call → Sonnet extracts a **summary**, **actions** (owner / deadline / source quote) and **decisions** → you edit, check/uncheck, and **Save**.
- **Browse & manage.** A mail-inbox dashboard with labels, a real detail pane (summary / actions / transcript from disk), inline action status, re-analyze, delete, and crash-safe recovery.
- **Private by default.** Audio + transcripts are plain files on your Mac; the API key lives in the macOS **Keychain**; AI features use **your own** Claude key and can be switched off entirely.

## How it works

A **Tauri v2** app — a **Rust** backend with a thin, event-driven **Svelte + TypeScript** frontend. All the real work (audio, STT, AI, storage) is in Rust; the UI just renders the events that stream back.

**Audio (the crux) — two passive streams, no virtual mic:**

```
Real mic ─────────────────────────────────► cpal ─┐  "You"
Meeting app ─► Multi-Output ─► BlackHole ──► cpal ─┤  "Remote"
                   └─► your headphones (you hear it)│
                                                    ▼
            16 kHz mono ─► WAV + Whisper ─► transcript ─► Claude
                                            (Haiku live · Sonnet chat & post-analysis)
```

You install a small BlackHole fork ("Call Assistant") and a one-time Multi-Output Device so the remote side reaches **both** your ears and our capture. Two known streams ⇒ **free You/Remote labels.**

**Ground truth on disk.** The stereo `audio.wav` (L = you, R = remote) and append-only `transcript.jsonl` are written incrementally, so every failure degrades to "you still have the recording and the transcript." Everything lives in flat JSON/WAV files under `~/Library/Application Support/CallAssistant/`.

Full detail: **[docs/architecture.md](docs/architecture.md)** · the implementation-grade plan: **[docs/build/](docs/build/)**.

## Getting started

> macOS + Apple Silicon. The app runs from source today (`npm run tauri dev`); a signed, packaged build is on the [roadmap](docs/roadmap.md#cross-cutting-distribution--hardening).

### Prerequisites

**Toolchain**
- **Rust** (stable) + **CMake** (`brew install cmake`) — `whisper-rs` compiles whisper.cpp natively (first build is slow, then cached).
- **Node.js** + npm — the SvelteKit frontend.

**Audio** (required for the Remote side)
1. `brew install blackhole-2ch`, then **reboot** (macOS won't expose BlackHole as an input until you restart).
2. Open **Audio MIDI Setup** → create a **Multi-Output Device** = your headphones **+** BlackHole 2ch.
3. Set that Multi-Output as the system (or meeting-app) output.
4. **Wear headphones** — open speakers let your mic re-capture the remote side (echo + double transcription).

**Whisper model** — downloaded in-app (onboarding or Settings): `medium` (~1.5 GB, default) or `small` (~466 MB, faster).

**Claude API key** — needed for live AI + post-analysis. Capture + transcription work without one. Stored in the macOS Keychain; `ANTHROPIC_API_KEY` in the env is a dev fallback.

### Run

```sh
npm install            # first time only
npm run tauri dev      # the real app (Rust backend + WebView)
```

First capture triggers the macOS microphone-permission prompt — allow it.

**Frontend-only preview** (mock data, no backend — layout checks only):

```sh
npm run dev
```

### Test

```sh
cd src-tauri && cargo test     # 104 unit tests — VAD, recovery, AI client/retry/SSE, post-analysis, labels…
cd src-tauri && cargo clippy   # lints (clean)
npm run check                  # svelte-check (types)
```

Hand-run verification — every flow, milestone by milestone, plus the full **E2E acceptance run** — is scripted in **[docs/build/manual-testing.md](docs/build/manual-testing.md)**.

### Where your data lives

```
~/Library/Application Support/CallAssistant/
├── settings.json
├── labels.json
└── sessions/{id}/   →   metadata.json · audio.wav · transcript.jsonl · analysis.json · …
```

Reset onboarding by deleting `settings.json`; wipe everything by deleting the whole folder.

## Project status

### Done — MVP (M0–M5, all merged)

| Milestone | What shipped | |
|---|---|:--:|
| **M0** · Spikes | Whisper speed · ×2 concurrent · dual-audio capture · Claude calls — all validated | ✅ |
| **M1** · Walking skeleton | Tauri + Svelte shell · real `cpal` device IPC · file storage | ✅ |
| **M2** · Capture → Transcript | Dual capture → VAD → Whisper → live **two-sided** transcript · crash recovery | ✅ |
| **M3** · Live AI | Haiku findings + F/C/S/Q toggles + cost meter · streamed Sonnet Ask-AI · Keychain keys | ✅ |
| **M4** · Post-analysis | End → Sonnet extraction → merge → review/edit → Save | ✅ |
| **M5** · Manage & polish | Real dashboard detail · labels · re-analyze · delete · recover-into-review · toasts | ✅ |

104 unit tests · `cargo clippy` clean · `svelte-check` clean.

### In progress — the one remaining gate

The MVP is **software-complete**; what software can't sign off is a **real on-device call** — BlackHole + a live key, start → live transcript → live AI → end → review → save → browse, plus the exception paths. That capstone is the **[E2E Acceptance Run](docs/build/manual-testing.md#e2e--mvp-acceptance-run-the-on-device-gate)**.

### Roadmap — toward v1.0 (Beta)

| Release | Theme | Headline work |
|---|---|---|
| **[v0.2](docs/roadmap.md#v02--organization--tracking)** | Organization & Tracking | Projects · sidebar nav · global cross-session **Actions** view · full-text search (SQLite derived index) |
| **[v0.3](docs/roadmap.md#v03--review--the-prep-loop)** | Review & the Prep Loop | Session **playback** · **Prepare for Next Call** briefing · templates · `large-v3` archival re-pass · export |
| **[v0.4](docs/roadmap.md#v04--seamless-capture--integrations)** | Seamless Capture & Integrations | Custom **HAL audio plugin** (zero setup) · menu bar · shortcuts · MCP · diarization |
| **[v1.0 — Beta](docs/roadmap.md#v10--beta)** | The vision, made public | Final polish + **signing · notarization · installer · auto-update** → first public release |

The cross-cutting **[Distribution & Hardening](docs/roadmap.md#cross-cutting-distribution--hardening)** track (code-signing, notarization, packaged installer, auto-update) is the **v1.0** go-public push. Per-version scope, decisions, and open calls are in **[docs/roadmap.md](docs/roadmap.md)**.

## Repository map

```
src/ · src-tauri/        the app — Tauri v2 · SvelteKit+TS frontend · Rust backend (npm run tauri dev)
  src/lib/screens/         Onboarding · Dashboard · NewSession · Live · Post · Settings
  src-tauri/src/           audio/ · stt/ · ai/ · storage/ · session/ · commands.rs · events.rs
spikes/                  M0 de-risking spikes (Whisper speed, dual-audio, Claude) — throwaway
docs/
├── vision.md            v1.0 (Beta) — the full aspiration (the destination)
├── mvp.md               MVP (v0.1) — first iteration toward v1.0 (scope + build steps)
├── roadmap.md           the bridge: v0.1 (MVP) → v0.2 → v0.3 → v0.4 → v1.0 (Beta)
├── architecture.md      stack · audio pipeline · storage · data model (MVP-now vs v1.0-target)
└── build/               implementation-grade plan — flows · technical-design · milestones · m3–m5 plans · manual-testing
design/
├── ui-spec.md           the 6-screen UI/UX spec
└── prototype.html       high-fidelity visual prototype of all six screens — open in a browser
```

## Tech stack & locked decisions (MVP)

| Decision | Choice |
|---|---|
| App framework | **Tauri v2** (Rust backend + web frontend) |
| Frontend | **Svelte + TypeScript** |
| Product shape | **Dashboard + Labels** — flat session list (Apple-Mail split-pane), actions scoped per session |
| Audio capture | **BlackHole fork** ("Call Assistant") — passive 2-stream (You + Remote), no virtual mic; HAL plugin is a v1.0 target |
| Local STT | **whisper-rs** (whisper.cpp), **`medium`** default (`small` / `base` fallback) |
| AI | **Claude API** — Haiku (live) · Sonnet (chat + post-analysis) |
| Storage | **Flat files** (JSON + WAV) under `~/Library/Application Support/CallAssistant/` |

Full rationale and the MVP-vs-v1.0 differences: **[docs/architecture.md](docs/architecture.md)**.

## Documentation

| Doc | What's in it |
|---|---|
| **[vision.md](docs/vision.md)** | v1.0 (Beta) — the full product (the destination) |
| **[mvp.md](docs/mvp.md)** | MVP scope + build steps (the first iteration) |
| **[roadmap.md](docs/roadmap.md)** | v0.1 → v0.2 → v0.3 → v0.4 → v1.0 — per-version scope & decisions |
| **[architecture.md](docs/architecture.md)** | Stack · audio pipeline · storage · data model |
| **[build/](docs/build/)** | Implementation-grade: flows · technical-design · milestones · manual-testing |
| **[design/ui-spec.md](design/ui-spec.md)** | The 6-screen UI/UX spec (+ `prototype.html`) |

## Why bot-free

Most meeting assistants send a visible bot into the call. Personal Call Assistant is a **transparent virtual-audio proxy** — nothing joins the meeting — **plus** an action tracker that links every commitment back to the exact moment it was made. Market context in **[vision.md](docs/vision.md#market-context-why-bot-free)**.
