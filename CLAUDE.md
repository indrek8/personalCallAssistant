# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A native **macOS** desktop app (Tauri v2 · SvelteKit/TS · Rust) that passively captures both sides of a meeting via a BlackHole loopback (no bot joins the call), transcribes locally with Whisper, runs a live Claude sidecar, and extracts actions/decisions/summary afterward.

**The `docs/` tree is authoritative and unusually complete — read it before any non-trivial change:**

- `docs/architecture.md` — stack, audio pipeline, storage, data model (MVP-now vs the v1.0 target)
- `docs/build/technical-design.md` — Rust modules, threading model, the full IPC contract (§7), schemas
- `docs/build/flows.md` — every state machine + the `EXC-*` exception/recovery contract (§9)
- `docs/build/milestones.md` + `docs/build/README.md` — build status and the **decisions log (D1–D26)**
- `docs/build/manual-testing.md` — hand-run test plan + the on-device E2E acceptance run

## Commands

Run from the repo root. The first Rust build is slow — `whisper-rs` compiles whisper.cpp natively (`brew install cmake`).

```sh
npm install                        # first time
npm run tauri dev                  # run the real app (Rust backend + WebView)
npm run dev                        # frontend-only preview (mock data, no backend — layout checks only)
npm run tauri build                # package the app
npm run check                      # svelte-check (TS types) — the frontend's "lint"

cd src-tauri && cargo test         # Rust unit tests (inline #[cfg(test)] modules)
cd src-tauri && cargo test <name>  # run a single test / matching subset by name
cd src-tauri && cargo clippy       # Rust lints (kept clean — treat warnings as failures)
```

Capture needs one-time audio setup (BlackHole 2ch + a Multi-Output Device + headphones) and a Whisper model (downloaded in-app); live/post AI needs a Claude API key. See `docs/build/manual-testing.md` §1.

## Architecture (the big picture)

**Thin frontend, everything in Rust.** Svelte/TS issues commands and renders events; a single `mode` store (`booting|onboarding|dashboard|new|live|post|settings`) is the router — no URL routing. All audio/STT/AI/storage work lives in `src-tauri/src/`.

**Concurrency: dedicated std threads + `crossbeam` channels + `reqwest::blocking` — NOT tokio (decision D15).** The guiding rules: **audio capture must never block**, and the **`audio.wav` + `transcript.jsonl` are ground truth**, written incrementally so every failure degrades to "you still have the recording and the transcript." Do not introduce a tokio runtime.

**The live pipeline** (per session):

```
two cpal streams  (mic = "you",  BlackHole = "remote")
  → resample to 16 kHz mono → tee to WAV writer + VAD segmenter
  → WhisperWorker (dedicated thread) → TranscriptEntry
  → append transcript.jsonl  +  emit `transcript-entry`
  → AiBatcher (Haiku, fires on ≥5 new entries OR ≥30 s) → findings
     ask_ai (Sonnet, SSE-streamed) on demand
End → post-analysis (Sonnet, structured output) → review/edit → save
```

**IPC is the integration contract.** `commands.rs` (frontend→Rust `invoke`) + `events.rs` (Rust→frontend `emit`), enumerated in `technical-design.md` §7. **`src/lib/types.ts` is a hand-maintained mirror of the Rust `serde` types** — change a payload on one side and you must update the other.

**Session lifecycle** (`metadata.json` `status`): `draft → recording ⇄ paused → ending → analyzing → reviewing → completed` (+ `failed`, `recovering`). A boot recovery scan finalizes interrupted sessions; a crashed `reviewing` session keeps its draft (D23). Full machine: `flows.md` §2.

**Storage:** flat files under `~/Library/Application Support/CallAssistant/` (`settings.json`, `labels.json`, `sessions/{uuid}/…`). Writes are atomic (temp→fsync→rename); `transcript.jsonl` / `ai_live.json` are append-only. Labels use a global registry + embedded per-session snapshots (D24).

**Rust modules** (`src-tauri/src/`): `audio/` (capture · vad · wav), `stt/` (whisper worker · model_mgr), `ai/` (claude client · live batcher · chat · analyze · prompts), `storage/`, `session/` (manager · model), plus `commands.rs`, `events.rs`, `config.rs` (Keychain), `error.rs` (`AppError` → `EXC-*` codes).

## Conventions that bite

- **No silent failure.** Every handled error maps to an `EXC-*` code with a defined UI behavior and recovery (`flows.md` §9, `error.rs`). The WAV + transcript are never sacrificed to an error path.
- **The API key lives in the macOS Keychain only** (`config.rs`) — never in `settings.json` or logs. Read precedence: Keychain → `ANTHROPIC_API_KEY` env (dev fallback).
- **AI calls use structured outputs** (`output_config.format` json_schema) for both live findings and post-analysis. **Active F/C/S/Q toggles ride in the *user* turn**, not the cached system prefix, so toggling never invalidates the prompt cache. **Cost is accounted before the response is parsed** (a refusal / `max_tokens` cut is still billed). Models: `claude-haiku-4-5` (live) / `claude-sonnet-4-6` (chat + post).
- **Decisions are logged.** Before reversing a load-bearing choice, check the **decisions log (D1–D26)** in `docs/build/README.md`; add a new `Dxx` rather than silently flipping one.
- **Versioning:** releases are **v-prefixed, named milestones** — `v0.1` (MVP) → `v0.2` → `v0.3` → `v0.4` → **`v1.0` (Beta)**. Everything before `v1.0` is `0.x` / pre-public. Always write `v0.2` (never bare `0.2`); never write "Version 1" — it is `v1.0 — Beta`. Scheme + per-version scope: `docs/roadmap.md`.
