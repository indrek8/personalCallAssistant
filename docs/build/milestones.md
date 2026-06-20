# MVP Build — Milestones

> The actual build sequence. Each milestone is an **independently demoable increment** with concrete tasks and acceptance criteria. De-risking spikes come **first** (M0). References: [flows.md](flows.md), [technical-design.md](technical-design.md).

**Principle:** prove the two things that can sink the project (Whisper speed, dual-audio capture) before building the app around them. Then grow the app as vertical slices — each milestone ends with something you can actually run.

```
M0 Spikes ─► M1 Skeleton ─► M2 Capture→Transcript ─► M3 Live AI ─► M4 Post-analysis ─► M5 Manage & polish
 (throwaway)   (shell+IPC)     (the engine)            (Haiku/Sonnet)  (Sonnet review)    (browse, settings, errors)
```

---

## M0 — De-risking Spikes (throwaway code)

**Goal:** answer the two open technical questions before committing to architecture. Code here is disposable.

> **Status:** ✅ **M0 complete — all four spikes validated.** Whisper `small` RTF 0.040 / `medium` 0.055; concurrent ×2 RTF 0.032 → **2-stream model locked**; s3 dual-capture on hardware → clean L/R attribution (idle channel held -120 dBFS = zero cross-bleed); s4 Claude Haiku + Sonnet calls returned, parsed, and cost-accounted (token/cost fields confirmed).

- [x] **S1 · Whisper speed.** Standalone Rust bin: load `small` (and `base`) via `whisper-rs`, transcribe a pre-recorded 10 s 16 kHz WAV, print text + wall-time. Run on the target Mac.
- [x] **S2 · Whisper ×2 feasibility.** Run two transcriptions concurrently (simulating you+remote). Measure CPU + latency. Decides **2-stream vs single-stream** attribution.
- [x] **S3 · Dual-audio capture.** Standalone Rust bin: open the **mic** and a **BlackHole** input via `cpal` simultaneously; write a 10 s stereo WAV (L=mic, R=blackhole). Manually set up the Multi-Output Device and confirm remote audio (e.g. a YouTube tab) lands on R while your voice lands on L. **Result (M0/S3): validated on hardware** — Shokz mic + BlackHole, 16 s capture; voice isolated to L while the idle R channel held -120 dBFS, then YouTube isolated to R while idle L held -120 dBFS → free 2-way attribution, zero cross-bleed.
- [x] **S4 · Claude calls.** Minimal `reqwest` call to Haiku + Sonnet with the live + post JSON schemas; confirm parsing and capture token/cost fields. **Result (M0/S4): validated** — `claude-haiku-4-5` + `claude-sonnet-4-6` both returned "spike ok", response JSON parsed, token/cost accounting correct (Haiku $0.000038, Sonnet $0.000114 per ping). Confirms the M3 `ai/mod.rs` HTTP shape.

**Acceptance / decision gate:**
- Real-time factor for `small` is comfortably < 1.0 (transcribes faster than realtime) → use `small`; else fall back to `base`. **Result (M0/S1): `small` RTF 0.040, `medium` RTF 0.055 — both ~20× realtime, so the MVP defaults to `medium` for accuracy.**
- ×2 concurrent is sustainable → **lock 2-stream You/Remote**; else fall back to mixed mono + generic "Speaker" (update [technical-design.md](technical-design.md) §4–5).
- Dual capture works → the audio model in §4 is real. **Result (M0/S3): confirmed** — clean L/R separation, zero cross-bleed; the §4 audio model holds.

> If S1/S3 fail badly, that's a *cheap* pivot point — far better to learn here than in M2.

---

## M1 — Walking Skeleton

**Goal:** a running app proving the whole stack is wired — frontend ↔ Rust ↔ filesystem ↔ system audio — with no real features yet. (= [../mvp.md](../mvp.md) Step 1.)

> **Status:** ✅ **Complete & merged** (PR #1) — all acceptance criteria met (build green; session create/persist verified; real `cpal` dropdown).

- [x] Scaffold Tauri v2 + Svelte + TS (`npm create tauri-app@latest`).
- [x] Module skeleton per [technical-design.md](technical-design.md) §3 (empty `audio/ stt/ ai/ storage/ session/`).
- [x] `mode` store + screen shells: Dashboard (split), New Session, Live (stub), Post (stub), Settings, Onboarding — ported from `design/prototype.html`.
- [x] Command `list_audio_input_devices` (real `cpal`) → populates a dropdown. **Proves Svelte ↔ Rust ↔ Core Audio.**
- [x] Storage module: `create_session` writes `sessions/{uuid}/metadata.json`; `list_sessions` reads them; dashboard left pane renders from disk.
- [x] Boot: load `settings.json`, route to onboarding vs dashboard.

**Acceptance:**
- `npm run tauri dev` opens the window on the dashboard shell.
- Creating a session writes a real folder that survives an app restart and reappears in the list.
- The device dropdown is populated by the Rust command, not hardcoded.
- macOS mic-permission prompt handled gracefully on first device access.

---

## M2 — Capture → Live Transcript (the engine)

**Goal:** start a session, capture both sides, see a live transcript, end and save. The heart of the product.

- [ ] `audio/capture.rs`: dual `cpal` streams (mic + BlackHole), resample → 16 kHz mono (`rubato`).
- [ ] `audio/wav.rs`: incremental stereo WAV writer (L=you, R=remote) via `hound`.
- [ ] `audio/vad.rs`: silence-gap segmentation with hard-max length → tagged utterances.
- [ ] `stt`: `WhisperWorker` thread; model load + `transcribe`; emit `TranscriptEntry`.
- [ ] `storage`: incremental append to `transcript.json` (crash-safe).
- [ ] IPC: `start_capture`, `pause_capture`, `resume_capture`, `end_session`; events `transcript-entry`, `capture-state`, `whisper-status`.
- [ ] Live screen: real rolling transcript (You/Remote colors), timer, pause/resume, end.
- [ ] `run_preflight` (§4 checks) gates Start.
- [ ] Model manager: download/verify a model (`model_mgr.rs`) + settings UI.
- [ ] Recovery scan on boot (EXC-CRASH) → route stale sessions to Post.

**Acceptance:**
- Talking on a real Zoom/Meet call (with Multi-Output set up) produces a **two-sided live transcript** within ~10 s of speech.
- `audio.wav` + `transcript.json` are written incrementally; killing the app mid-call leaves both intact and recoverable.
- Pause/resume works; device disconnect falls back to default without losing the session (EXC-DEV-DROP).

---

## M3 — Live AI

**Goal:** real-time fact-checks, commitments, suggestions, unanswered-Qs, plus Ask-AI — all cost-tracked.

- [ ] `ai/mod.rs`: Claude `reqwest` client, cost accounting, retries/backoff.
- [ ] `ai/live.rs`: `AiBatcher` (≥5 entries OR ≥30 s) → Haiku → strict-JSON findings; defensive parse.
- [ ] Toggle system (F/C/S/Q) → live system prompt; `set_toggles`.
- [ ] `ai/chat.rs`: `ask_ai` → Sonnet (optionally streamed).
- [ ] Events `ai-finding`, `cost-update`, `ai-chat-*`; append `ai_live.json` / `chat.json`.
- [ ] AI panel: findings feed, `[+ Save action]` on commitments, Ask-AI bar, cost meter.
- [ ] Budget cap → `EXC-BUDGET` pauses live AI; transcript continues.
- [ ] Failure handling: `EXC-API-LIVE` chip + auto-disable after N failures.

**Acceptance:**
- Speaking a commitment/factual-conflict surfaces the right finding in the panel within a batch cycle.
- All toggles off → **zero** API calls (verify in logs).
- "Summarize what we've agreed" via Ask-AI returns a sensible answer; cost meter increments.
- Live AI failures never interrupt the transcript.

---

## M4 — Post-Analysis & Review

**Goal:** End → Sonnet extraction → review/edit → save.

- [ ] `ai/analyze.rs`: full transcript + context + live annotations → Sonnet → `{summary,actions,decisions,key_topics}`.
- [ ] Merge/dedupe Sonnet actions with live commitments + saved actions.
- [ ] IPC: `run_post_analysis` (progress events), `save_analysis`, `update_action_status`.
- [ ] Post screen (two-pane): editable summary `[Regenerate]`, action rows (check/owner/due/quote/delete/add), decisions, meta rail, Save & Close / Back to Transcript.
- [ ] Exceptions: `EXC-API-POST` (Retry / Save-without-analysis), `EXC-EMPTY` (skip analysis).
- [ ] `status` transitions analyzing → reviewing → completed.

**Acceptance:**
- Ending a real session yields an editable summary + extracted actions with owners/dates/quotes within ~30 s.
- Unchecking an action excludes it from the saved set; manual add works.
- Save returns to dashboard with the session present and `completed`; data survives restart.
- Analysis failure still lets you save the session with the transcript intact.

---

## M5 — Manage, Settings & Polish

**Goal:** the surrounding app — browse, manage actions, configure, and handle the rough edges.

- [ ] Dashboard detail pane: summary, actions (inline status edit), transcript — all from disk.
- [ ] Label CRUD (`labels.json`), filter by label, name search.
- [ ] `Re-analyze` on a stored session.
- [ ] Onboarding wizard wired to real key-test + device + model steps.
- [ ] Settings fully functional (key/Keychain, device, model, default toggles, storage reveal).
- [ ] Exception surfacing: global `app-error` → toast/banner; `EXC-CORRUPT` handling in the list.
- [ ] Empty states, loading states, confirm dialogs (End, Discard, Re-analyze).

**Acceptance:**
- Full loop with **no console babysitting**: onboard → new → live → end → review → save → browse → update action status.
- Corrupt/missing files degrade gracefully (no crashes).
- A first-time user can get from launch to a working capture using only in-app guidance.

---

## Testing Strategy

| Layer | Approach |
|---|---|
| Audio/STT | Spikes (M0) + golden WAV → expected-ish transcript; manual real-call checks |
| AI | Unit-test JSON parsing with recorded fixtures (incl. malformed); mock HTTP for retry/backoff |
| Storage | Round-trip + crash-injection (kill mid-write → file still valid) + recovery-scan test |
| IPC | Each command has a smoke test; event payloads type-checked against shared TS types |
| Flows | Manual run-through of every flow in [flows.md](flows.md), including each EXC-* |
| End-to-end | The §M5 acceptance loop on a real Teams/Meet/Zoom call |

## Risk Register

| Risk | Likelihood | Mitigation |
|---|---|---|
| Whisper too slow for real-time | Med | **M0/S1–S2 first**; fall back `small`→`base`, or mixed-mono single pass |
| Dual-capture / Multi-Output flaky | Med | **M0/S3**; clear setup UX; soft-warn (EXC-NOMULTI); v1 HAL plugin removes it |
| macOS mic/notarization friction | Med | Handle permission flow early (M1–M2); dev-sign; notarize before distribution |
| Live AI cost surprises | Low | Budget cap + cost meter + "all toggles off = no calls" |
| whisper-rs build issues on Apple Silicon | Med | Pin versions; validate in M0; document toolchain |
| Scope creep from v1 features | Med | Anything not in [../mvp.md](../mvp.md) → [../roadmap.md](../roadmap.md), no exceptions |

## Definition of Done (MVP)

The [verification plan in ../mvp.md](../mvp.md#verification--testing-plan) passes end-to-end on a real call: capture → live transcript → live AI → end → analysis → review → save → browse, with crash-safety and graceful errors throughout.
