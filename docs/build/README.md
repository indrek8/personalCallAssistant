# MVP Build Plan

Implementation-grade plan for building the MVP — detailed enough to code against. This expands the high-level [../mvp.md](../mvp.md) and [../architecture.md](../architecture.md) into concrete flows, engineering, and a milestone sequence.

## The documents

| Doc | What's in it |
|---|---|
| **[flows.md](flows.md)** | Every state machine, user flow, and exception/recovery path — the behavioral contract |
| **[technical-design.md](technical-design.md)** | Rust modules, threading model, audio/STT/AI subsystems, the full Tauri IPC contract, Svelte frontend, storage & schemas |
| **[milestones.md](milestones.md)** | The build sequence M0–M5: tasks, acceptance criteria, de-risking spikes, testing, risks |

**Read order:** milestones (what & when) → flows (how it behaves) → technical-design (how it's wired).

## Build principles

1. **De-risk first.** Prove Whisper speed + dual-audio capture (M0 spikes) before building the app around them.
2. **Vertical slices.** Every milestone ends with something runnable, not a layer in isolation.
3. **Ground truth on disk.** The WAV + `transcript.json` are written incrementally and atomically — every failure degrades to "you still have the recording and transcript."
4. **Thin frontend.** Svelte renders events; all real work is in Rust.
5. **No silent failure.** Every exception in [flows.md](flows.md) §9 has a defined user-facing behavior and recovery.
6. **Build forward.** Stable IDs + normalized storage so v1's projects/global-actions ([../roadmap.md](../roadmap.md)) are an additive migration.

## Decisions log

Key technical decisions made in these docs (revisit consciously, not by accident):

| # | Decision | Rationale | Status |
|---|---|---|---|
| D1 | **2-stream You/Remote audio** — Multi-Output Device for remote + direct mic | Free speaker attribution without diarization | **Validated — M0 S1+S2 passed:** whisper `small` RTF ~0.04 single / ~0.03 concurrent ×2 (~25× realtime headroom). S3 hardware capture run still pending. |
| D2 | **No virtual mic in MVP** — passive listening only | Meeting app keeps using the real mic; virtual-mic proxy is a v1/HAL concern | Locked |
| D3 | Incremental, atomic writes (temp→fsync→rename) | Crash safety; recovery | Locked |
| D4 | VAD segmentation with a hard-max length | Avoids mid-word slicing and unbounded waits | Locked |
| D5 | Whisper **`medium`** default (fallback `small`/`base`), downloaded on demand | Best accuracy at negligible cost — medium is real-time too | **Validated (M0/S1):** medium RTF 0.055, small 0.040 — both ~20× realtime |
| D6 | Haiku (live) / Sonnet (chat + post-analysis) | Cost vs quality split | Locked |
| D7 | Event-driven frontend, single `mode` store as router | Matches the state machine; no URL routing needed | Locked |
| D8 | Flat-file storage with normalized IDs | Simple MVP, forward-compatible | Locked |

## Milestone overview

```
M0  Spikes              prove Whisper speed + dual-audio capture (throwaway)
M1  Walking skeleton    Tauri+Svelte shell, IPC device list, storage
M2  Capture→Transcript  dual capture, WAV, Whisper, live two-sided transcript
M3  Live AI             Haiku findings + toggles + Ask-AI (Sonnet) + cost
M4  Post-analysis       Sonnet extraction → review/edit → save
M5  Manage & polish     dashboard, labels, settings, onboarding, error handling
```

**Start here:** [milestones.md → M0](milestones.md#m0--de-risking-spikes-throwaway-code).
