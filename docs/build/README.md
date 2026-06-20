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
3. **Ground truth on disk.** The WAV + `transcript.jsonl` are written incrementally and atomically — every failure degrades to "you still have the recording and transcript."
4. **Thin frontend.** Svelte renders events; all real work is in Rust.
5. **No silent failure.** Every exception in [flows.md](flows.md) §9 has a defined user-facing behavior and recovery.
6. **Build forward.** Stable IDs + normalized storage so v1's projects/global-actions ([../roadmap.md](../roadmap.md)) are an additive migration.

## Decisions log

Key technical decisions made in these docs (revisit consciously, not by accident):

| # | Decision | Rationale | Status |
|---|---|---|---|
| D1 | **2-stream You/Remote audio** — Multi-Output Device for remote + direct mic | Free speaker attribution without diarization | **Validated — M0 S1+S2+S3 passed:** whisper `small` RTF ~0.04 single / ~0.03 concurrent ×2 (~25× realtime headroom); S3 dual-capture on hardware → clean L/R attribution (idle channel held -120 dBFS = zero cross-bleed). |
| D2 | **No virtual mic in MVP** — passive listening only | Meeting app keeps using the real mic; virtual-mic proxy is a v1/HAL concern | Locked |
| D3 | Incremental, atomic writes (temp→fsync→rename) | Crash safety; recovery | Locked |
| D4 | VAD segmentation with a hard-max length | Avoids mid-word slicing and unbounded waits | Locked |
| D5 | Whisper **`medium`** recommended default, **`small`** the floor; the user picks in onboarding (`base` hidden from the picker); downloaded on demand | Best accuracy at negligible cost — medium is real-time too; `base` is too weak for meeting terms | **Validated (M0/S1):** medium RTF 0.055, small 0.040 — both ~20× realtime |
| D6 | Haiku (live) / Sonnet (chat + post-analysis) | Cost vs quality split | **Locked — M0/S4 validated:** `claude-haiku-4-5` + `claude-sonnet-4-6` resolve, responses parse, token/cost accounting confirmed. |
| D7 | Event-driven frontend, single `mode` store as router | Matches the state machine; no URL routing needed | Locked |
| D8 | Flat-file storage with normalized IDs | Simple MVP, forward-compatible | Locked |
| D9 | **Transcript as `transcript.jsonl`** (append-only, one entry per line) | True crash-safe incremental writes (no array-rewrite window) — the §9 "JSONL internally" option; read back into the array the UI expects | **M2** |
| D10 | **EXC-DEV-DROP = detect → rebuild on the default device → notify** (retry-capped per side) | A mid-call device unplug must not freeze capture; seamless hot-swap stays a v1/HAL concern | **M2** |
| D11 | **API key in macOS Keychain** (`keyring`), read precedence Keychain → `ANTHROPIC_API_KEY` env | Shipped-app design; never in `settings.json`; env keeps the dev/spike path working | **M3** |
| D12 | **Live findings via structured outputs** (`output_config.format` json_schema) | The API guarantees schema-valid JSON on Haiku; defensive parse stays a fallback | **M3** |
| D13 | **Ask-AI streamed** (SSE → `ai-chat-token`) | Word-by-word answers feel right mid-call | **M3** |
| D14 | Models `claude-haiku-4-5` (live) / `claude-sonnet-4-6` (chat) — bare ids | Verified current against the API reference | **M3** |
| D15 | **AI runs on std threads + `reqwest::blocking`**, not tokio | Matches the M2 concurrency model (capture / STT / model_mgr all sync) | **M3** |
| D16 | **Ask-AI is not budget-gated** — `EXC-BUDGET` throttles automatic live (Haiku) spend only; an explicit user Ask-AI question always runs (its cost is still folded into the session total) | An explicit user action shouldn't be silently blocked mid-call | **M3 (hardening)** |

## Milestone overview

```
M0  Spikes              prove Whisper speed + dual-audio capture (throwaway)
M1  Walking skeleton    Tauri+Svelte shell, IPC device list, storage
M2  Capture→Transcript  dual capture, WAV, Whisper, live two-sided transcript
M3  Live AI             Haiku findings + toggles + Ask-AI (Sonnet) + cost
M4  Post-analysis       Sonnet extraction → review/edit → save
M5  Manage & polish     dashboard, labels, settings, onboarding, error handling
```

**Progress:** **M3 ✅ complete & merged** (PRs #9–#12) — Claude client + Keychain keys, live Haiku findings + F/C/S/Q toggles + cost + budget/failure handling, streamed Sonnet Ask-AI, save-action persistence; **78 unit tests**, clippy clean (a post-closeout hardening + coverage pass tightened teardown, streaming-error/refusal handling, and cost accounting — see [m3-plan.md §Post-closeout hardening](m3-plan.md#post-closeout-hardening)). M2 ✅ (PRs #4–#8) — capture → live two-sided transcript. M1 ✅ (PR #1); **M0 ✅** (s1–s4). **Next → [M4: Post-Analysis & Review](milestones.md#m4--post-analysis--review).**
