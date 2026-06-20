# M3 — Live AI · Execution Plan

> Implementation-grade plan for the M3 milestone in [milestones.md](milestones.md#m3--live-ai):
> real-time Haiku findings + F/C/S/Q toggles + streamed Ask-AI (Sonnet), all
> cost-tracked. Grounded in the current Claude API contract (verified against the
> `claude-api` reference, 2026-06-20) and the **as-merged** M2 code, not just the
> intent in [technical-design.md](technical-design.md). Read alongside
> [flows.md](flows.md) §5 (C2/C3) + §9 (EXC-API-LIVE / EXC-BUDGET / EXC-KEY).

## Status

✅ **Complete & merged.** PR1 (#9) Claude client + Keychain keys · PR2 (#10) live Haiku
findings + toggles + cost + budget · PR3 (#11) streamed Sonnet Ask-AI · PR4 (#12) closeout
— save-action persistence, SSE-parse test, doc reconciliation. **78 unit tests**, clippy
clean. A post-closeout hardening pass (see *Post-closeout hardening* below) tightened
teardown, streaming-error/refusal handling, and cost accounting, and broadened M1–M3
coverage. The on-device live-AI run (a real call with your key) is the remaining manual check.

---

## Decisions locked for M3

Continuing the [README decisions log](README.md#decisions-log) (D1–D10):

| # | Decision | Rationale |
|---|---|---|
| **D11** | **API key in macOS Keychain** via the `keyring` crate; read precedence **Keychain → `ANTHROPIC_API_KEY` env** (dev/spike fallback). Onboarding + Settings "Test" buttons wired to a real 1-token ping; **EXC-KEY** added to pre-flight. | The key UI already exists but is inert; Keychain is the shipped-app design (technical-design §10). Env fallback keeps the spike/dev path working. *(Your call: "Keychain + Settings now".)* |
| **D12** | **Live findings use structured outputs** — `output_config.format` with a `json_schema`. Defensive parse retained as a fallback (a refusal or `max_tokens` cut can still yield non-conforming output). | The API guarantees schema-valid JSON on Haiku 4.5, eliminating most malformed-batch discards. Supersedes the "ask for JSON, parse defensively" note in technical-design §6. *(Your call: "Structured outputs".)* |
| **D13** | **Ask-AI streams** — parse the SSE response and emit `ai-chat-token` events; `ai-chat-done` carries the final answer + cost. | Word-by-word answers feel right mid-call; matches the `ai-chat-token`/`ai-chat-done` events already in the IPC contract. *(Your call: "Stream tokens live".)* |
| **D14** | **Models: `claude-haiku-4-5` (live) / `claude-sonnet-4-6` (chat).** Bare IDs, **no date suffix**. | Verified current against the API reference; unchanged since the M0/S4 spike. Cost/quality split per D6. |
| **D15** | **Threading: dedicated std threads + `reqwest::blocking`**, not tokio. | Matches the as-merged codebase — `audio/capture`, `stt`, and `stt/model_mgr` are all sync std-thread + `crossbeam` + `reqwest::blocking`. Introducing a tokio runtime just for M3 would fork the concurrency model. *(Deviation from technical-design §2's "tokio task" sketch — noted there for the closeout.)* |

---

## The Claude wire contract (what every PR builds against)

**Endpoint / headers** — `POST https://api.anthropic.com/v1/messages`, headers
`x-api-key: <key>`, `anthropic-version: 2023-06-01`, `content-type: application/json`.

**Live request (Haiku)** — frozen prefix cached, volatile transcript after it:

```jsonc
{
  "model": "claude-haiku-4-5",
  "max_tokens": 1024,
  "system": [{                                  // ← stable per session
    "type": "text",
    "text": "<role + all-4-feature instructions>\n\nSESSION CONTEXT:\n<context_notes>",
    "cache_control": { "type": "ephemeral" }    // 5-min TTL; batches ~30s apart reuse it
  }],
  "messages": [{ "role": "user", "content":
    "ACTIVE: F,C,Q\n\nTRANSCRIPT (last ~3 min):\n<rolling window>" }],
  "output_config": { "format": { "type": "json_schema", "schema": FINDINGS_SCHEMA } }
}
```

- **Active toggles go in the user message, not the system prompt** — so toggling F/C/S/Q doesn't invalidate the cached prefix. The system prompt always describes all four; the `ACTIVE:` line tells Haiku which to emit.
- **Caching pays off only when the cached prefix ≥ 4096 tokens** (Haiku's minimum). A small `context_notes` won't cache — and that's fine, the per-batch cost is already ~tiny. Verify with `usage.cache_read_input_tokens > 0`.
- Keep `FINDINGS_SCHEMA` byte-identical per session (it is) so it never invalidates.

**`FINDINGS_SCHEMA`** (structured-outputs rules: every object needs `required` +
`additionalProperties: false`; no numeric/length constraints — none needed here):

```jsonc
{ "type": "object", "additionalProperties": false,
  "required": ["fact_checks","commitments","suggestions","unanswered_questions"],
  "properties": {
    "fact_checks": { "type":"array", "items": { "type":"object","additionalProperties":false,
      "required":["claim","assessment","severity"], "properties": {
        "claim":{"type":"string"}, "assessment":{"type":"string"},
        "severity":{"type":"string","enum":["warning","info"]} } } },
    "commitments": { "type":"array", "items": { "type":"object","additionalProperties":false,
      "required":["who","what","by_when"], "properties": {
        "who":{"type":"string"}, "what":{"type":"string"}, "by_when":{"type":"string"} } } },
    "suggestions": { "type":"array", "items": {"type":"string"} },
    "unanswered_questions": { "type":"array", "items": {"type":"string"} } } }
```

**Chat request (Sonnet, streamed)** — `"model":"claude-sonnet-4-6"`, `"max_tokens":4096`,
`"stream": true`, `system` = role + `context_notes`, `messages` = full transcript so far + the question. SSE frames to handle (`data: {json}` per line):

| SSE event | Field to read | Action |
|---|---|---|
| `message_start` | `message.usage.input_tokens` (+ `cache_read_input_tokens`) | seed input-token count |
| `content_block_delta` | `delta.type=="text_delta"` → `delta.text` | append + emit `ai-chat-token{token}` |
| `message_delta` | `usage.output_tokens` **+ `delta.stop_reason`** | final output-token count; capture the stop reason |
| `error` (any frame) | `error.type` / `error.message` | **abort** — surface the error, don't return the partial answer as if complete |
| `message_stop` | — | compute cost, emit `ai-chat-done{answer,cost}` |

> **Stop-reason handling (hardening):** a streamed turn is an HTTP 200 even when it
> stops short. `chat::ask` maps `stop_reason` to user-facing text — a `refusal` shows a
> clear "declined" message (partial discarded), a `max_tokens` cut appends a truncation
> note, and a dropped/empty stream is a hard error — instead of a blank or silently
> clipped answer.

`reqwest::blocking::Response` is a `Read`, so the chat thread reads the SSE body
line-by-line and emits as it goes — no async runtime needed.

**Usage → cost** (per-MTok rates; extend the spike's formula with the cache terms):

```
cost = ( in_tok            * in_rate
       + out_tok           * out_rate
       + cache_write_tok   * in_rate * 1.25     // 5-min ephemeral write
       + cache_read_tok    * in_rate * 0.10 ) / 1_000_000
```

| Model | in_rate | out_rate |
|---|---|---|
| `claude-haiku-4-5` | 1.00 | 5.00 |
| `claude-sonnet-4-6` | 3.00 | 15.00 |

**Errors** → EXC mapping: `401`→**EXC-KEY**; `429`/`500`/`529`→retry with
`retry-after` honored + exponential backoff (cap N; **retries/backoff short-circuit on
session teardown** so End never blocks behind them); schema/parse failure on a live
batch → **discard the batch, log, continue** (transcript untouched; the call is still
billed, so its **cost is accounted before the parse** and recorded with a `discarded`
note); N consecutive live failures → **EXC-API-LIVE** auto-disable with a notice (a
discarded-but-reachable batch resets the counter — only real HTTP failures count).

---

## PR breakdown

### PR1 — Claude client + API-key management *(the foundation)*

- [ ] `config.rs`: `keyring`-backed `get_api_key()` (Keychain → `ANTHROPIC_API_KEY` env), `save_api_key(key)`, `has_api_key()`. Key never touches `settings.json`.
- [ ] `ai/mod.rs`: `ClaudeClient` over `reqwest::blocking` — `messages(req) -> Result<Resp>`; shared headers; `usage` + cost accounting (rate table + cache multipliers); retry/backoff on 429/5xx/529 (honor `retry-after`); error → `AppError` (EXC-KEY / EXC-API-*).
- [ ] IPC: `test_api_key({key}) -> {ok, model, error?}` (1-token Haiku ping); `save_api_key({key}) -> ()`; `get_api_key_status() -> {present}`.
- [ ] Pre-flight: prepend the **EXC-KEY** check (key present & last-test-valid) to `run_preflight`.
- [ ] Frontend: wire Onboarding step 1 "Test & continue" + Settings "Test" to `test_api_key`→`save_api_key`; show real Connected/✗ status (replace the hardcoded "Connected").

**Acceptance:** a real key validates via Test and persists in Keychain across restart; a bad key surfaces EXC-KEY inline; Start is blocked without a valid key; `ANTHROPIC_API_KEY` still works as a dev fallback.

### PR2 — Live analysis: Haiku findings + toggles + cost + budget *(the core)*

- [ ] `ai/prompts.rs`: live system-prompt builder (all-4-features) + `FINDINGS_SCHEMA`.
- [ ] `ai/live.rs`: `AiBatcher` on a dedicated std thread, fed transcript entries via a `crossbeam` tee off the SessionManager's entry path. Fires when **≥5 new entries OR ≥30 s**, **≥1 toggle on**, and **not in-flight**. Builds the cached-prefix request, calls Haiku with `output_config.format`, parses (defensive fallback), appends `ai_live.json`, emits `ai-finding` + `cost-update`.
- [ ] `set_toggles({f,c,s,q})`: updates batcher state for the *next* batch. **All off → zero calls** (batcher idles).
- [ ] **EXC-BUDGET**: running total ≥ `budget_cap` → emit `app-error`, pause live AI (transcript continues).
- [ ] **EXC-API-LIVE**: backoff; after N consecutive failures auto-disable + notice.
- [ ] SessionManager: spawn the batcher in `start_inner`, stop/join it in `end`/`pause` (idle on pause).
- [ ] Frontend: replace the inert panel with a real findings feed (type-colored: fact/commit/suggest/question), live F/C/S/Q toggle row, `[+ Save action]` on commitments (in-memory draft for now — see PR4), and a **cost meter in the live bar** (`live` store gains `cost`).

**Acceptance:** speaking a commitment or a fact that conflicts with `context_notes` surfaces the right finding within a batch cycle; **all toggles off → zero API calls** (verify in logs); cost meter increments; a live-AI failure never interrupts the transcript.

### PR3 — Ask-AI chat: Sonnet, streamed

- [ ] `ai/chat.rs`: `ask_ai({question})` → spawns a thread that streams Sonnet (full transcript + `context_notes` + question), parses SSE, emits `ai-chat-token` per delta then `ai-chat-done{answer,cost}`; appends `chat.json`; `cost-update`. Command returns `{answer, cost}` when the stream completes.
- [ ] Frontend: enable the Ask-AI input; render the streamed answer visually distinct from auto-findings (`chat` store).

**Acceptance:** "summarize what we've agreed" returns a sensible answer that streams in; cost meter increments; `chat.json` logged.

### PR4 — Closeout: tests, save-action persistence, docs

- [ ] Unit tests: cost computation incl. cache tokens; findings parse against fixtures (valid + malformed → discarded); batcher fire-condition logic; SSE delta parsing; retry/backoff with a mock HTTP layer; key precedence (Keychain vs env).
- [ ] **Save-action**: since End→dashboard with no M4 review yet, persist `[+ Save action]` commitments (appended to `saved_actions.json`) so they survive to be merged in M4 — don't let them vanish.
- [ ] Docs: tick M3 in [milestones.md](milestones.md) + [README.md](README.md); reconcile technical-design §6 (json_schema, streaming, caching, key precedence) and §2 (D15 threading deviation); add an M3 section to [manual-testing.md](manual-testing.md); update `MEMORY.md`.
- [ ] `cargo clippy` clean; all tests green.

**Acceptance:** the full M3 milestone acceptance in [milestones.md](milestones.md#m3--live-ai) passes on a real call.

---

## IPC additions

**Commands:** `test_api_key`, `save_api_key`, `get_api_key_status` (new, PR1) ·
`set_toggles` (PR2) · `ask_ai` (PR3) — all named per technical-design §7 where they exist.

**Events:** `ai-finding{session_id,finding}` · `cost-update{session_id,total,last}` ·
`ai-chat-token{token}` · `ai-chat-done{answer,cost}` · `app-error` reused for
EXC-KEY / EXC-API-LIVE / EXC-BUDGET.

## Storage additions

- `ai_live.json` — one record per batch: `[{t_ms,model,tokens_in,tokens_out,cache_read,cost,latency_ms,findings[],discarded?}]` (crash-safe append, same pattern as `transcript.jsonl`; a discarded batch still records its cost + a `discarded` reason).
- `chat.json` — one record per Ask-AI turn: `[{question,answer,tokens_in,tokens_out,cost}]`.
- `saved_actions.json` — `[+ Save action]` commitments, appended one per line (M4 merges these into post-analysis).
- API key → **Keychain** (service `com.callassistant.audio` / account `anthropic-api-key`); never in `settings.json` or logs.

## Frontend store changes

`live` gains `cost`; new `findings` (fed by `ai-finding`), `chat` (fed by
`ai-chat-*`), and `toggles` stores; `app-error` already routes to the banner.

---

## Post-closeout hardening

A correctness + test-coverage pass over the merged M3 layer (`ai/`), plus broader M1/M2
unit coverage. Nothing the milestone acceptance depends on changed — these harden the
edges. Suite: **42 → 78 tests**, clippy clean.

**Live-AI (`ai/mod.rs`, `ai/live.rs`):**
- **Teardown can't hang.** The Haiku client uses a short (20 s) timeout, and the
  retry/backoff loop (`messages_cancellable`) short-circuits on the batcher's stop flag,
  so `end()` no longer blocks behind a stack of in-flight retries (worst case was minutes
  during a 429 storm; now ≤ one in-flight call). A teardown-cancelled batch is **not**
  counted as a failure and emits no EXC-API-LIVE toast.
- **Cost is accounted before the parse.** A discarded batch (refusal / malformed body) is
  still billed, so its cost now hits the meter and the crash-safe `ai_live.json` record
  (with a `discarded` note) instead of being lost.
- **`set_toggles` race tightened** — toggles are re-read immediately before firing, so
  flipping everything off can't trigger one last stale batch (the "all-off ⇒ zero calls"
  guarantee holds to within microseconds).
- A `Discard` (reachable API, unparseable body) resets the consecutive-failure counter.

**Chat (`ai/mod.rs` `parse_sse`, `ai/chat.rs`):**
- A mid-stream `error` frame (e.g. `overloaded_error`) surfaces as an error instead of
  silently returning the partial answer as complete.
- `stop_reason` is inspected (refusal / `max_tokens` / dropped stream) — see the chat SSE
  table above.

**Session (`session/manager.rs`):** `end()` reads the AI-shared cost lock poison-safe, so
an AI-thread panic can never block the session being saved.

**Decision (D16): Ask-AI is not budget-gated.** `EXC-BUDGET` throttles **automatic** live
(Haiku) spend only; an explicit user Ask-AI question always runs, its cost still folded
into the session total. (Confirmed during this pass — an explicit user action shouldn't be
silently blocked mid-call.)

**Frontend:** the cost meter clamps with `Math.max` so a `cost-update` that races behind a
larger total (live batch vs Ask-AI on two threads) can't tick the meter backwards.

**Tests added (28):** retry/backoff state machine + status classification, batcher
fire-condition + `any_on`, SSE error/`stop_reason` handling, chat answer finalization;
plus M1/M2 — VAD hard-max continuation timestamps + `finish()`-at-EOS + min-length gate,
WAV `to_i16` clamping + unfinalized-tail repair, device-id round-trip, `list_sessions`
sort/skip, atomic-write cleanup, Settings/Toggles/StreamTag/SessionStatus serde contracts,
`whisper_threads` cap.

> **Known M1/M2 limitations surfaced (not yet fixed):** the model downloader retries
> permanent 4xx (e.g. a 404 model) up to its cap before failing; `start_inner` doesn't
> guard against `mic == remote` device. Both are minor and tracked for a follow-up.

## Sequencing notes / risks

- **PR1 first, always** — every later PR needs the client + key. It's also independently demoable (Test button works) on its own.
- **Caching is an optimization, not a gate** — ship PR2 working first; confirm `cache_read_input_tokens>0` and only then worry about prefix size. Below 4096 tokens it silently no-ops, which is correct.
- **`output_config.format` + `cache_control` coexist** — the schema sits outside the cached prefix tiers and is constant per session, so it won't invalidate the system cache.
- **Keep the WAV/transcript path untouched** — the batcher tees *off* the existing entry channel; build principle #3 (ground truth on disk) still holds, and every AI failure degrades to "you still have the transcript."
