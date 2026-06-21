# Roadmap: v0.1 (MVP) → v1.0 (Beta)

> How the **[MVP](mvp.md)** — today's **v0.1** — iterates toward **v1.0 (Beta)**, the first public release and the full **[product vision](vision.md)**. Each minor release closes part of the gap; nothing from the vision is dropped, just sequenced. The technical shape of every target lives in **[architecture.md](architecture.md)**.

```
v0.1 ────────► v0.2 ────────► v0.3 ────────► v0.4 ────────► v1.0
MVP            Organization   Review &       Seamless       Beta
(software-     & Tracking     Prep Loop      Capture        the vision,
 complete)                                                  signed & public
```

**Versioning:** every release before **v1.0** is **0.x — pre-public** (dev / private alpha; runs from source). **v1.0 = Beta = the full vision, signed and installable = the first public release.** Each version has a name; the minor bumps (v0.2 · v0.3 · v0.4) are the steps in between — each independently demoable, none shipped to the public yet.

## How to read this

Each release below uses the same shape, at **roadmap altitude** (the *what & why & when*, not an implementation plan — those are the `build/` docs):

- **Goal** — the one-line outcome that defines the release.
- **Scope — in / out** — what's included, and where the deferred pieces go.
- **Key decisions** — the consequential technical calls, with rationale (these become `Dxx` entries in the [build decisions log](build/README.md#decisions-log) when the work starts).
- **Depends on** — what must exist first.
- **Done when** — a measurable acceptance bar, like the MVP's.

Two markers flag where this revision **re-evaluates** the prior plan, so each is easy to accept or veto:

- **↻ Change** — a deviation from the previous roadmap (a re-sequence, split, addition, or sharpened decision), with the reason.
- **⚖ Open call** — a decision worth making consciously; the recommended default is stated.

---

## Cross-cutting: Distribution & Hardening

> What turns the app from "runs on my Mac via `npm run tauri dev`" into something a stranger can install. This is the **defining work of v1.0** — there's no public Beta without it — but it's listed once here because a piece of it reaches back into v0.4.

The 0.x line never ships publicly; it runs from source. Going public (**v1.0**) requires:

- **Developer-ID code signing** + **notarization** + stapling, with a hardened runtime and the right entitlements (microphone; and the HAL plugin's own signing).
- A **packaged installer** (`.dmg`/`.app`).
- **Auto-update** (the Tauri updater plugin) + a release/update feed.
- **Opt-in crash reporting / minimal telemetry** — privacy-first, to match the product's "local & private" promise (off by default, clearly disclosed).

**Sequencing:** the **HAL plugin in v0.4 needs at least dev/ad-hoc signing to load locally**, so signing work starts there; the full **Developer-ID notarization + installer + updater is the v1.0 "go public" push.** Treat this as one track that lands at v1.0, with an early toe-hold in v0.4.

---

## v0.1 — MVP

*Today. Software-complete.*

**Goal:** the end-to-end flow works — capture → live transcript → live AI → post-analysis → review → save → browse → manage. Full definition in **[mvp.md](mvp.md)**.

> **Status: software-complete (M0–M5 merged).** 104 unit tests + clippy + svelte-check green. The single remaining gate is the **on-device end-to-end run** on a real call (BlackHole + an API key), scripted in [build/manual-testing.md](build/manual-testing.md#e2e--mvp-acceptance-run-the-on-device-gate). Everything below assumes that gate has passed.

---

## v0.2 — Organization & Tracking

*The biggest single leap toward the vision — this is what turns the tool into a daily driver.*

**Goal:** turn the flat session log into a navigable, cross-session **work tracker** — group sessions into projects, see every open action in one place, and find anything by text.

**Scope — in:**

- **Projects** — a global `projects.json` registry; each session gains an optional `project_id`. Projects are the **primary grouping** (one per session, or none → an "Inbox"/No-project bucket); labels stay as **orthogonal, many-per-session tags**. (Projects = the folder; labels = the tags.)
- **Sidebar navigation** — introduce the persistent sidebar from [vision.md](vision.md) (`[+ New Session]` · **Actions** · **Projects** tree). The MVP's split-pane dashboard becomes the "all / per-project" view *inside* the sidebar shell; the flat `mode` router grows a navigation layer.
- **Global Actions view** — one consolidated list of every action across all sessions, with **inline status** (pending / in-progress / done / won't-do / postponed) and filters (project, owner = mine/theirs, status, due). The heart of the vision's sidebar model.
- **Full-text search** across session names, transcripts, summaries, and action titles → jump to the session.
- **SQLite as a derived index** powering the actions view + search at scale (see decisions).
- **Daily-driver polish:** **edit session metadata** after creation (name / labels / participants / context); **budget-cap enforcement** (a hard pause at the cap, not just the MVP's display); **minimal bookmark capture** during live (`Cmd+B` → a timestamped marker, shown inline in the review transcript).

**Scope — out / later:**

- Session playback / audio scrubbing → **v0.3**
- "Prepare for Next Call" briefing → **v0.3** (it depends on this actions view existing)
- The bookmark *timeline* (clickable jump points) → **v0.3** (capture lands here; the timeline that consumes it ships with playback)
- Per-speaker diarization → **v0.4**

**Key decisions:**

- **SQLite is a derived index/cache, not the source of truth.** Flat files (`metadata.json`, `transcript.jsonl`, `analysis.json`) stay ground truth; the DB is a **rebuildable projection** — a "reindex" reconstructs it entirely from disk. This honors build-principle #3 (ground truth on disk) and keeps every migration additive. The index holds a `sessions` table, an `actions` table, and an **FTS5** virtual table over transcripts/summaries. *Note:* actions are **positional inside `analysis.json`** today (no explicit `session_id` on the row), so the index **materializes** `session_id` + `project_id` onto each action at index time. Writes go to the file first (authoritative) and the index best-effort. (Alternative weighed: pure in-memory file scan — fine for the actions list at hundreds of sessions, too slow for full-text transcript search → index it once.)
- **Projects reuse the proven labels pattern.** `projects.json` = `Vec<ProjectRef{ id, name, color }>`; `SessionMeta` gains `#[serde(default)] project_id: Option<String>` — additive, **no migration** (existing sessions read as project-less). id→name/color resolves from the registry exactly like labels (D24).
- **Adopt the sidebar now.** Projects + global actions are the sidebar's whole reason to exist; shipping them inside the old top-bar dashboard would be throwaway UI. The cost is a frontend nav-shell rework around the `mode` store.

**Depends on:** MVP stable IDs (**✓ confirmed in code** — every entity carries an `id`) and normalized flat-file storage. No data migration for projects (additive field); the index is built on first launch of v0.2 and is disposable.

**Done when:**

- Create / rename / recolor / delete a project, assign sessions to it, and browse the project tree in the sidebar.
- One Actions view lists every action across all sessions; an inline status change persists to the owning `analysis.json` and reflects immediately; filters narrow the list; overdue items are flagged.
- Search returns hits across names + transcripts + summaries well under a second on a realistic library, and jumps to the session.
- Editing a session's metadata persists across restart.
- A budget cap actually **pauses** live AI at the cap (transcript continues).
- **Deleting the index and relaunching rebuilds it from files with identical results** (the rebuildable-projection guarantee).

**↻ Changes & ⚖ open calls:**

- ↻ Made SQLite's role **explicit** (derived index, files authoritative) — the prior plan only said "likely introduces SQLite alongside file storage."
- ↻ Pulled **budget-cap enforcement** forward (small, tracking-flavored) and **added session-metadata editing** (M5 explicitly deferred it; it's a real daily-driver gap). Bookmarks: kept *capture* here, moved the *timeline* to v0.3.
- ⚖ **v0.2 is the largest minor.** Consider splitting it across two minors — **v0.2:** projects + sidebar + global actions + the index; a follow-on minor: full-text search + bookmarks + polish (everything after shifts down by one). Recommended: one release if the index work is shared; split only if the first half is taking too long to demo.

---

## v0.3 — Review & the Prep Loop

> ↻ Renamed from "Richer review" to foreground the payoff: closing the
> **prepare → record → extract → track → prepare** loop.

**Goal:** make a finished session a rich, **replayable** artifact, and close the loop by feeding what's still open into the *next* call.

**Scope — in:**

- **Session playback** — `audio.wav` synced to transcript scroll; click a line to seek; bookmark + action markers on a timeline scrubber; 0.5–2× speed. Syncs on `TranscriptEntry.t_ms`; resolves the action→moment link (`Action.transcript_t_ms`, which the code stubs today and this release fills in).
- **"Prepare for Next Call"** — an AI briefing (open actions + recent decisions + suggested talking points) generated from the v0.2 actions data, sent to the clipboard **or** straight into a new session's context field.
- **Bookmark timeline** — the markers captured in v0.2 become clickable jump points in playback and the review transcript.
- **Templates** — saved toggle sets + a custom extraction prompt + a budget default, applied at New Session.
- **Higher-quality archival transcription** — an optional `large-v3` re-pass over the saved audio for the *archived* transcript (the live transcript stays `medium` for real-time).
- **Data export** — a session → Markdown (summary + actions + transcript) for sharing/backup.

**Scope — out / later:**

- Per-speaker diarization → **v0.4** (see open call)
- Custom HAL audio plugin / zero-setup capture → **v0.4**

**Key decisions:**

- **Playback reads ground-truth files, no re-encode** — stream `audio.wav` directly, sync on `t_ms`; no separate playback artifact.
- **The briefing is a Sonnet call** over the open-actions set + recent sessions for a project, reusing the analysis client; its output is text that seeds the next session's `context_notes`, not a stored entity.
- **Live (`medium`) and archival (`large-v3`) transcripts are separate passes;** the re-pass is opt-in per session (cost + time) and keeps the original JSONL alongside the upgraded one for safety.

**Depends on:** v0.2's global-actions data (for the briefing) and bookmark capture (for the timeline). The deferred `transcript_t_ms` linking from M4 lands here.

**Done when:**

- Play any completed session back with the transcript auto-scrolling in sync; click a line to seek; click a bookmark/action marker to jump to its moment.
- Generate a briefing for an upcoming call that pre-fills a new session's context with what's still open.
- Save a template and start a session from it with toggles + prompt pre-applied.
- Export a session to a readable Markdown file.

**↻ Changes & ⚖ open calls:**

- ↻ **Split diarization out.** The prior plan lumped diarization in here. Kept is the *easy* quality win — a `large-v3` archival re-pass; **per-speaker diarization moves to v0.4**: it's a hard STT item (splitting the Remote stream into individuals), orthogonal to the review theme, and the MVP deliberately sidesteps it via the 2-stream model.
- ↻ Added **data export** — a common review/share need absent from the prior plan.
- ⚖ Diarization placement is a real call: if per-speaker labels matter sooner, it can stay here as a stretch item. Defaulted to v0.4 on difficulty.

---

## v0.4 — Seamless Capture & Integrations

> ↻ Reframed from "Seamless audio & integrations"; the *public-release* packaging
> moved to the [v1.0 go-public push](#v10--beta) (see the cross-cutting track) since 0.x is still pre-public.

**Goal:** remove the last manual friction (BlackHole + Audio MIDI Setup) and round out the integrations — so v1.0 has nothing left but polish and packaging.

**Scope — in:**

- **Custom HAL audio plugin** (`.driver`, a Core Audio `AudioServerPlugIn`) replacing the BlackHole fork — dynamic device lifecycle (devices exist only while the app runs), shared-memory ring buffers, clock sync, hotplug. Eliminates **all** one-time audio setup. (See [architecture.md → Layer 1](architecture.md#layer-1-audio-proxy-virtual-audio-device).) Dev/ad-hoc-signed here; Developer-ID notarization is the v1.0 push.
- **Menu-bar presence** during live sessions (timer, cost, pause, bookmark, end).
- **Full keyboard-shortcut set** (the table in [vision.md](vision.md#keyboard-shortcuts)).
- **MCP server** — expose sessions/actions to Claude Desktop.
- **Per-speaker diarization** (moved from v0.3) — split the Remote stream into individual speakers.

**Key decisions:**

- **The HAL plugin is dumb endpoints, all logic in the app** — a pair of lock-free ring buffers over shared memory; the Rust app does all capture/processing; heartbeat-driven device add/remove. (architecture.md Layer 1.)
- **Signing splits across v0.4 and v1.0** — the plugin must be at least dev-signed to load locally in v0.4; the public Developer-ID + notarization is part of the v1.0 release.

**Depends on:** for *development*, an Apple Developer account to sign the plugin for local loading. The HAL plugin is independent of v0.2/v0.3 feature work and can proceed in parallel.

**Done when:**

- "Call Assistant" mic/speaker appear **only while the app runs** and vanish on quit — **no Audio MIDI Setup, no reboot, no Multi-Output device** — on the dev machine.
- Menu-bar controls drive a live session while the main window is hidden.
- (If included) the Remote stream is split into named speakers in the transcript.

**↻ Changes & ⚖ open calls:**

- ↻ Per-speaker **diarization moved in** from v0.3; the **public-release packaging moved out** to v1.0 (0.x is pre-public).
- ⚖ **The HAL plugin is the single largest engineering item in the whole post-MVP plan** (Core Audio + signing + device lifecycle). Consider giving it its own minor (v0.4 = HAL only; a follow-on minor = menu-bar / shortcuts / MCP / diarization), with everything after shifting down.

---

## v1.0 — Beta

*The vision, made public.*

**Goal:** the full **[product vision](vision.md)**, polished and **publicly shippable** — the first version a stranger can download, install, and run. This is the destination; from here it's refinement, not new pillars.

**Scope — in:**

- **Final polish** across everything v0.2–v0.4 delivered (the sidebar + projects, the global actions tracker, playback, the prepare-loop, seamless HAL capture).
- **The [Distribution & Hardening](#cross-cutting-distribution--hardening) track** — Developer-ID signing + **notarization** + a packaged `.dmg`, the **auto-updater**, and opt-in crash reporting. This is what makes v1.0 *public*.

**Depends on:** v0.2 + v0.3 + v0.4 feature-complete, and an Apple Developer account + a working notarization pipeline.

**Done when (Beta is reached):**

- [ ] Sidebar + projects navigation is the home *(v0.2)*
- [ ] A global cross-session actions tracker with inline status *(v0.2)*
- [ ] Full-text search across everything *(v0.2)*
- [ ] Session playback synced to the transcript, with bookmark/action markers *(v0.3)*
- [ ] The full **prepare → record → extract → track → prepare** loop, including the briefing *(v0.3)*
- [ ] Seamless HAL audio — zero manual setup *(v0.4)*
- [ ] A **signed, notarized, auto-updating** build a stranger can install *(v1.0)*

At that point the product matches **[vision.md](vision.md)**. Beyond it: diarization depth, more templates, menu-bar polish, the MCP surface, performance at scale — **refinement, not new pillars** (the 1.x line).

---

## Build forward, not into a corner

Keep the MVP's data model normalized with stable IDs so each step lands as an **additive migration, not a rewrite** (see [architecture.md → Data Model](architecture.md#data-model)). **Confirmed in the merged code:**

- every entity (`SessionMeta`, `Action`, `TranscriptEntry`, `LabelRef`) already carries a stable `id`;
- labels already use the **registry + embedded-snapshot** pattern that projects will reuse;
- `Action.transcript_t_ms` is already reserved for the v0.3 playback link.

So v0.2's projects are a single additive `project_id` field, and the SQLite index is a derived projection that **never becomes a second source of truth** — delete it and it rebuilds from the files.
