# Personal Call Assistant

A native macOS desktop app that acts as an invisible AI meeting assistant. It captures both sides of any meeting (Teams / Meet / Zoom) through a virtual audio device, transcribes locally with Whisper, gives live AI analysis during the call, and extracts actions, decisions, and a summary afterward — no bot ever joins the call.

Core loop: **prepare → record → extract → track → prepare**

## Two horizons

This project is planned as a journey between two points:

- **Version 1** — the destination: the full, ambitious product → **[docs/vision.md](docs/vision.md)**
- **MVP** — the first iteration toward v1: the smallest genuinely useful build that proves the hard parts → **[docs/mvp.md](docs/mvp.md)**
- **Roadmap** — how the MVP walks toward v1, increment by increment → **[docs/roadmap.md](docs/roadmap.md)**

We build the MVP first, then iterate it closer and closer to Version 1.

> **Status:** **M3 complete & merged** (PRs #9–#12) — the live-AI layer: Claude client + macOS-Keychain key management, real-time Haiku findings with F/C/S/Q toggles + cost meter + budget/failure handling, and a streamed Sonnet Ask-AI. 78 unit tests, clippy clean (a post-merge hardening pass tightened teardown, SSE error/refusal handling, and cost accounting, and broadened M1–M3 coverage); the on-device live-AI run (your key + a real call) is the remaining manual check. **M2 complete** — the capture → live two-sided transcript engine (dual-stream → 16 kHz stereo WAV, VAD → Whisper, IPC + Live UI, pre-flight, model mgmt, crash recovery, EXC-DEV-DROP). **M1/M0 complete** — app shell; all four spikes validated. Next: **M4 — Post-analysis** (Sonnet extraction → review → save). See **[docs/build/milestones.md](docs/build/milestones.md)**.

## Repository map

```
src/ · src-tauri/     the app — Tauri v2 + SvelteKit/TS + Rust (M1; `npm run tauri dev`)
spikes/               M0 de-risking spikes (Whisper speed, dual-audio, Claude) + models.md
docs/
├── vision.md         Version 1 — the full aspiration (the destination)
├── mvp.md            MVP — first iteration toward v1 (scope + build steps)
├── roadmap.md        the bridge: MVP → v1.1 → v1.2 → … → Version 1
├── architecture.md   tech stack · audio pipeline · data model (MVP-now vs v1-target)
└── build/            implementation-grade MVP build plan — flows, technical design, milestones
design/
├── ui-spec.md                  the 6-screen UI/UX spec
├── prototype.html              high-fidelity visual prototype — all 6 screens, open in a browser
└── sidebar-prototype.jsx       vNext reference (the sidebar + projects direction)
```

The app lives at the repo root — `package.json`, `src/` (SvelteKit + TS frontend), `src-tauri/` (Rust backend) — scaffolded and merged in **M1**. Run it with `npm run tauri dev`.

## Locked decisions (MVP)

| Decision | Choice |
|---|---|
| App framework | **Tauri v2** (Rust backend + web frontend) |
| Frontend | **Svelte + TypeScript** |
| Product shape | **Dashboard + Labels** — flat session list (Apple Mail style), actions scoped to each session |
| Audio capture | **BlackHole fork** ("Call Assistant") — passive 2-stream (You + Remote), no virtual mic; HAL plugin is a v1 target |
| Local STT | **whisper-rs** (whisper.cpp), **`medium`** default (`small` / `base` fallback) |
| AI | **Claude API** — Haiku for live analysis, Sonnet for chat + post-analysis |
| Storage | **Flat files** (JSON + WAV) under `~/Library/Application Support/CallAssistant/` |

Full rationale and the MVP-vs-v1 technical differences: **[docs/architecture.md](docs/architecture.md)**.
