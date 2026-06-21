# M5 — Manage, Settings & Polish · Execution Plan

> Implementation doc for [milestones.md → M5](milestones.md#m5--manage-settings--polish):
> the surrounding app — **browse, manage actions, labels, settings, and the rough
> edges** — so the whole loop runs with no console babysitting. Grounded in the
> as-merged M4 code. Read alongside [flows.md](flows.md) §7 (Review/Manage/Re-analyze)
> + §9 (EXC-CORRUPT, recover-into-review).

## Status

✅ **Complete.** Built as one continuous branch (`feat/m5-manage-polish`).
**104 unit tests** (was 96), `cargo clippy` clean, `svelte-check` clean. This closes the
MVP — the full **onboard → new → live → end → review → save → browse → manage** loop is
real. The on-device run (a real call → manage → labels → re-analyze) is the remaining
manual check ([manual-testing.md → M5](manual-testing.md#m5--manage-settings--polish-is-now-real)).

---

## Decisions locked for M5

Continuing the [README decisions log](README.md#decisions-log) (D1–D20):

| # | Decision | Rationale |
|---|---|---|
| **D21** | **Re-analyze reuses `run_post_analysis`** (already state-agnostic); the command records the pre-analysis status and **restores it on failure** (was hardcoded `ending`). | A failed Re-analyze of a `completed` session stays `completed` with its existing `analysis.json` intact (we only overwrite on success). No new backend command. |
| **D22** | **The Post screen has an entry `postMode` (`fresh \| reanalyze \| resume`).** `fresh`/`reanalyze` call Sonnet; **`resume` loads the existing draft and renders review without re-billing**. | Makes Post idempotent on re-entry and powers recover-into-review. The caller sets `postMode` before `navigate("post")`. |
| **D23** | **Recover-into-review:** a crashed **`reviewing`** session **stays `reviewing`** (keeps its draft) and reopens in Post (`resume`) via an **actionable recovery toast**. Other stale phases finalize to `completed` transcript-only and are **re-analyzable on demand**. | No dangling state; the user resumes a half-reviewed session instead of losing the draft to a silent complete. |
| **D24** | **Global `labels.json` registry** = `Vec<LabelRef>`. Sessions keep **embedded `LabelRef` snapshots** (no migration); the dashboard **resolves** `id → name/color` from the registry with snapshot fallback. | Rename/recolor reflects everywhere; delete leaves self-describing orphans (registry-only delete). |
| **D25** | **EXC-CORRUPT is a visible row.** `SessionMeta` gains `#[serde(default)] unreadable: bool`; `list_sessions` returns an **`unreadable` placeholder** (`id = dir name`, `status: failed`) instead of skipping. | The dashboard shows "⚠ Unreadable" + Reveal in Finder; the list never crashes on a bad row. |
| **D26** | **`set_capture_device` is folded into `save_settings`** (which already persists the device); no redundant command. **`reveal_in_finder`** uses the already-loaded `tauri_plugin_opener`. | Avoids a duplicate code path; reconciles the §7 contract to reality. |

---

## What was built

**Backend**
- `session/model.rs` — `SessionMeta` derives `Default` + gains `unreadable: bool` (D25).
- `storage/mod.rs` — `labels_path` / `read_labels` (+`_at`, missing/corrupt → `[]`) / `write_labels`; `delete_session` (+`_in`, missing → `NotFound`); `list_sessions` surfaces an `unreadable` placeholder for a corrupt `metadata.json`; recovery **keeps a `reviewing` session reviewing** (D23).
- `commands.rs` — implemented `delete_session`; new `reveal_in_finder` (opener plugin); `list_labels` / `create_label` / `update_label` / `delete_label` over the registry, delegating to pure helpers `upsert_label` / `apply_label_update` / `remove_label` / `next_label_color` (unit-tested like M4's `patch_action_status`); `run_post_analysis` restores the prior status on failure (D21). Removed the dead `not_impl` helper (last stub gone).
- `lib.rs` — registered the 5 new commands.

**Frontend**
- `types.ts` — `Label` alias, `PostMode`, `Toast`, `SessionMeta.unreadable?`.
- `format.ts` (new) — `shortDate` / `longDate` / `fmtDuration` moved out of `mock.ts`.
- `ipc.ts` — `deleteSession`, `revealInFinder`, `listLabels`, `createLabel`, `updateLabel`, `deleteLabel`.
- `stores.ts` — `labels` (+`refreshLabels`), `toasts` (+`pushToast`/`dismissToast`), `postMode`, `resolveLabel`; `app-error` → toast; `session-recovered` → actionable "Resume review" toast for a recovered `reviewing` session (D23).
- **Components (new):** `Modal`, `ConfirmDialog`, `Toasts`, `StatusPill` (inline action-status dropdown), `LabelChip` (color-driven), `LabelManager` (create/rename/recolor/delete + usage counts).
- **Dashboard** — rewritten: real `getSession` detail pane (summary, decisions, transcript) with loading/error states, **inline `StatusPill`** → `update_action_status`, **Re-analyze** (confirm) / **Delete** (confirm) / **Resume review**, real label filter chips, **unreadable rows** + Reveal in Finder, "Analyze now" for an un-analyzed session.
- **NewSession** — real label picker (multiselect + create-on-type), mock defaults dropped.
- **Settings** — Reveal in Finder wired; Manage-labels entry.
- **Post** — `postMode` (resume loads the draft, no re-bill); **Discard** (confirm → delete).
- **Live** — `window.confirm` replaced by `ConfirmDialog` for End; sets `postMode="fresh"`.

## Status machine & crash safety (M5 delta)

```
completed ─Re-analyze→ analyzing ─Sonnet→ reviewing(draft overwritten) ─Save→ completed
                           │ fail
                           └─ restore→ completed (old analysis intact, D21)
boot recovery:  reviewing → stays reviewing (draft kept) ─Resume→ Post(resume)   (D23)
                ending | analyzing | recording | paused → completed (transcript-only)
```

## IPC / storage / events

- **Commands implemented:** `delete_session` · `reveal_in_finder(path?)` · `list_labels` · `create_label(name,color?)` · `update_label(id,name?,color?)` · `delete_label(id)`. (`set_capture_device` folded into `save_settings`, D26.)
- **Storage:** `labels.json` = `[{id,name,color?}]`. `SessionMeta` gains `unreadable` (synthesized only).
- **Events:** no new events; `app-error` now drives toasts, `session-recovered` the actionable resume.

## Tests added (8 net)

`storage/mod.rs`: labels round-trip + missing/corrupt → `[]`; `delete_session` removes the dir + missing → `NotFound`; (modified) `list_sessions` surfaces an `unreadable` placeholder; (modified) recovery keeps a `reviewing` session reviewing. `commands.rs`: `upsert_label` create/dedupe/empty + explicit-color; `apply_label_update` rename/recolor/unknown + color-only; `remove_label` delete/unknown; `next_label_color` cycling. (Frontend: `svelte-check` only — no FE unit harness.)

## Boundaries (deferred to v1+, per roadmap)

- **Session playback** (audio-synced scrub) — the detail transcript is read-only, no timeline (v1.2).
- **Projects / global cross-session actions view** — labels stay session-scoped (v1.1).
- **Full-text search** across transcripts (v1.1) — dashboard search stays name-only.
- **Editing session metadata** (name/participants/context) after creation.
- **Cascade label-delete** rewriting every session — registry-only delete; embedded snapshots persist.
