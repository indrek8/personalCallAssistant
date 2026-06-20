# M4 — Post-Analysis & Review · Execution Plan

> Implementation doc for [milestones.md → M4](milestones.md#m4--post-analysis--review):
> **End → Sonnet structured extraction → merge with live/saved commitments →
> review/edit → save → browse.** Grounded in the as-merged M3 code (not just the
> intent in [technical-design.md](technical-design.md)). Read alongside
> [flows.md](flows.md) §6 (D — End & Post-Analysis) + §9 (EXC-API-POST / EXC-EMPTY).

## Status

✅ **Complete.** Built as one continuous branch (`feat/m4-post-analysis`).
**96 unit tests** (was 78), `cargo clippy` clean, `svelte-check` clean. The on-device
run (a real call → End → review → save) is the remaining manual check
([manual-testing.md → M4](manual-testing.md#m4--post-analysis-is-now-real)).

---

## Decisions locked for M4

Continuing the [README decisions log](README.md#decisions-log) (D1–D16):

| # | Decision | Rationale |
|---|---|---|
| **D17** | **Post-analysis uses structured outputs** — `output_config.format` json_schema on Sonnet (mirrors D12 for Haiku). Defensive parse retained for refusal / `max_tokens`. | The API guarantees schema-valid JSON, so the review screen binds a typed `Analysis` directly. |
| **D-cost** | **Cost is accounted before the parse** — `analyze::run` bills `add_api_cost` + emits `cost-update` the moment the Sonnet call returns. | A refusal / unparseable body is still billed; accounting first means its cost is never lost (the M3 live-hardening lesson, applied to post-analysis). |
| **D18** | **`run_post_analysis` returns `()`; the review screen re-fetches via `get_session`.** The draft is persisted to `analysis.json` (status `reviewing`) the instant analysis completes. | Honors the IPC contract's `()` signature; a mid-review crash keeps the draft; the re-fetch picks up the freshly-persisted total cost (capture + Sonnet) with no staleness. |
| **D19** | **Action merge is Sonnet-primary; user-saved actions are always kept.** Every `[+ Save action]` is included (deduped only against itself by `id`). The softer *live* commitments are appended only when not a near-duplicate of a Sonnet/saved row. | A user's explicit save is never silently dropped; the user unchecks any remaining dupe in review. |
| **D20** | **Crash/quit during post-analysis recovers cleanly** — `ending`/`analyzing` → `completed` transcript-only; `reviewing` → `completed` keeping its draft `analysis.json`. **EXC-EMPTY ≈ 25 words** → skip Sonnet, minimal review. | No dangling state; re-analysis (the "Re-analyze" button) is M5. |

---

## The Claude wire contract (post-analysis)

`POST /v1/messages`, Sonnet, **one-shot** (non-streamed, retried via
`ClaudeClient::messages`). No prompt caching (single call). `max_tokens: 8192` — headroom
so a long call's JSON is never truncated mid-object.

```jsonc
{
  "model": "claude-sonnet-4-6",
  "max_tokens": 8192,
  "system": "<analyst instructions>\n\nPREP NOTES:\n<context_notes>",
  "messages": [{ "role": "user", "content":
    "LIVE-DETECTED COMMITMENTS (reconcile, do not duplicate):\n- …\n\nFULL TRANSCRIPT:\n[You] …\n[Remote] …" }],
  "output_config": { "format": { "type": "json_schema", "schema": ANALYSIS_SCHEMA } }
}
```

`ANALYSIS_SCHEMA` (`ai/prompts.rs::analysis_schema`) — same structured-output rules as
`findings_schema` (every object: `required` + `additionalProperties:false`):
`{ summary, actions[{title,owner,deadline,transcript_quote,type∈[commitment,follow_up,suggestion]}], decisions[], key_topics[] }`
(`deadline` is a possibly-empty string).

**Stop-reason handling** (`ai/analyze.rs`): `refusal` → `AppError::Api` (EXC-API-POST);
`max_tokens` → salvage what parsed + flag truncation in the summary; an unparseable body →
`AppError::Api`. Cost (`resp.usage.cost(MODEL_SONNET)`) is billed **before** any of these checks.

---

## What was built

**Backend**
- `session/model.rs` — typed `Analysis` + `Action` + enums (`ActionType` `commitment|follow_up|suggestion`, `ActionStatus` `pending|in_progress|done|wont_do|postponed`, `OwnerType` `mine|theirs`, `CreatedBy` `ai_extracted|manual`). `SessionFull.analysis` is now `Option<Analysis>`.
- `storage/mod.rs` — `analysis_path`, `write_analysis`, `read_analysis` (missing/corrupt → `None`), `read_saved_actions` / `read_ai_live` (JSONL), `set_session_ended` (status `Ending` + duration + cost), `add_api_cost`; `get_session` now populates `analysis`; recovery finalizes a crashed `reviewing` → `completed` keeping its draft.
- `ai/prompts.rs` — `analysis_system_prompt`, `analysis_user_message`, `analysis_schema`.
- `ai/analyze.rs` (new) — `run(app, session_id) -> AppResult<Analysis>`: EXC-EMPTY short-circuit, Sonnet structured call, cost-before-parse (D-cost), raw→`Action` enrichment (owner-type inference, type parse, empty-deadline→`None`), and the D19 merge. Pure, unit-tested helpers (`count_words`, `owner_type_of`, `action_type_of`, `normalize_key`, `is_duplicate`, `merge_actions`, `minimal_empty_analysis`).
- `session/manager.rs::end` — finalizes to `set_session_ended` (was `set_session_completed`).
- `commands.rs` — `run_post_analysis` (async, `AppHandle`, `analyzing → reviewing`, resets to `ending` on failure), `save_analysis` (validate + backfill ids + `completed`), `update_action_status` (patch status + `completed_at`, via the pure `patch_action_status`).

**Frontend**
- `types.ts` — `Analysis` / `AnalysisAction` + enums + `AnalysisProgressEvent`; `SessionFull.analysis: Analysis | null`.
- `ipc.ts` — `runPostAnalysis` / `saveAnalysis` / `updateActionStatus`.
- `stores.ts` — `postSessionId`, `analysisPhase`, and an `analysis-progress` listener.
- `Live.svelte::end()` — captures the session id → `postSessionId` → `navigate("post")` (was straight to dashboard).
- `Post.svelte` — rewritten from mock into the real three-state screen: **processing** spinner → **review** (editable summary + action rows with include/owner/deadline/delete/+add, decisions, meta rail, Save & Close, Regenerate, Back-to-Transcript overlay) → **error** (Retry / Save-without-analysis / Back).

## Status machine & crash safety

```
recording ─End→ ending(+duration,+cost) ─run_post_analysis→ analyzing ─Sonnet→ reviewing(+draft,+cost)
                                                                  │ fail              │ Save & Close
                                                                  └─ reset→ ending    └→ completed (final)
boot recovery:  ending | analyzing → completed (transcript-only)  ·  reviewing → completed (draft kept)
```

## IPC / storage / events

- **Commands implemented:** `run_post_analysis(session_id) -> ()` (async) · `save_analysis(session_id, analysis) -> ()` · `update_action_status(session_id, action_id, status) -> ()`.
- **Events:** `analysis-progress { phase: "analyzing" | "reviewing" }` (first emitter); `cost-update` reused for the Sonnet bill.
- **Storage:** `analysis.json` — `{summary, actions[], decisions[], key_topics[], generated_at}`; `action` = `{id, title, owner, owner_type, type, status, deadline?, transcript_quote, transcript_t_ms, notes?, created_by, completed_at?}`.

## Tests added

`ai/analyze.rs` (7): owner-type/type mapping, word-count gate, normalize/duplicate, raw→action, finding→action, D19 merge (saved always kept, live deduped), Sonnet-fixture parse. `ai/prompts.rs` (3): analysis schema strictness, system-prompt notes, user-message shape. `session/model.rs` (1): Analysis/Action wire-format. `storage/mod.rs` (3): analysis round-trip + missing/corrupt tolerance, JSONL reader, `reviewing`-recovery keeps the draft. `commands.rs` (2): `patch_action_status` sets/clears `completed_at`, unknown id → `NotFound`.

## Boundaries (deferred to M5)

- Dashboard detail pane stays mock; `update_action_status` ships but its inline-edit UI is M5.
- "Re-analyze" on a stored session; recover-into-review on boot.
- Back-to-Transcript is a read-only overlay, not the full playback timeline.
