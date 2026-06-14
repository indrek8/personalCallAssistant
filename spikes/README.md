# M0 De-risking Spikes (throwaway)

Standalone cargo project for the **M0 spikes** in
[`../docs/build/milestones.md`](../docs/build/milestones.md). This is **not** part
of the Tauri app workspace — it is disposable code that answers the two technical
questions that can sink the project (Whisper speed, dual-audio capture) plus the
Claude API plumbing, before any architecture is committed.

> Everything here is throwaway. The real implementations live in `src-tauri/`
> (M1+). Findings from these spikes feed the decision gates in `milestones.md`
> §M0 and the audio/STT/AI sections of `technical-design.md`.

## Spikes

| Bin              | Spike | Proves | RUN doc |
|------------------|-------|--------|---------|
| `s1_whisper`     | S1    | `small`/`base` transcribe a 10 s 16 kHz WAV faster than realtime (RTF < 1.0) | [RUN-s1.md](RUN-s1.md) |
| `s3_dual_audio`  | S3    | mic + BlackHole capture simultaneously → stereo WAV (L=you, R=remote) | [RUN-s3.md](RUN-s3.md) |
| `s4_claude`      | S4    | one Haiku + one Sonnet Messages API call; parse text, tokens, cost | [RUN-s4.md](RUN-s4.md) |

(S2 — running two Whisper transcriptions concurrently — is a variation on S1 and
is not built here; run `s1_whisper` twice in parallel if you want to measure it.)

## Prerequisites

- **Rust** (stable) + **cmake** (`brew install cmake`) — `whisper-rs` builds
  whisper.cpp natively. Apple-Silicon GPU acceleration via the `metal` feature is
  on by default.
- **S1:** a ggml model (`./fetch-model.sh small`) and a 16 kHz mono WAV.
- **S3:** BlackHole installed (`brew install blackhole-2ch`), a Multi-Output
  Device configured, and a **reboot** after installing BlackHole.
- **S4:** an `ANTHROPIC_API_KEY` in the gitignored repo-root `.env`
  (`cp .env.example .env` from the repo root, then edit).

## Quick start

```sh
# from this directory: /Users/indrek/Development/personalCallAssistant/spikes

# S1 — Whisper speed
./fetch-model.sh small
cargo run --release --bin s1_whisper -- sample.wav        # see RUN-s1.md for the WAV

# S3 — dual capture (after installing BlackHole + reboot + Multi-Output device)
cargo run --release --bin s3_dual_audio

# S4 — Claude calls (after putting the key in ../.env)
cargo run --bin s4_claude
```

## Layout

```
spikes/
  Cargo.toml          # three [[bin]] targets, deps pinned per spike
  fetch-model.sh      # download a ggml whisper model into models/
  README.md           # this file
  RUN-s1.md           # S1 instructions (model + sample WAV)
  RUN-s3.md           # S3 instructions (BlackHole + Multi-Output + reboot)
  RUN-s4.md           # S4 instructions (.env key)
  src/bin/
    s1_whisper.rs
    s3_dual_audio.rs
    s4_claude.rs
  models/             # gitignored model blobs (created by fetch-model.sh)
  target/             # gitignored (root .gitignore already ignores spikes/target/)
```

## Notes

- Build artifacts (`spikes/target/`) are ignored by the repo-root `.gitignore`.
  `models/` (large blobs) and generated WAVs are throwaway — don't commit them.
- whisper.cpp builds are slow the first time (compiles a C++ library). Subsequent
  builds are cached.
