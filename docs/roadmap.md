# Roadmap: MVP → Version 1

> How the **[MVP](mvp.md)** iterates toward **[Version 1](vision.md)**. Each release closes part of the gap. Nothing from the vision is dropped — just sequenced.

```
MVP ──────> v1.1 ──────> v1.2 ──────> vNext ──────> Version 1
(it works)  (organize)   (review)     (seamless)    (the vision)
```

## MVP — done when…

The end-to-end flow works: capture → live transcript → live AI → post-analysis → review → save → browse. Full definition in **[mvp.md](mvp.md)**.

## v1.1 — Organization & tracking

*The biggest single leap toward the vision — this is what turns the tool into a daily driver.*

- **Projects** — group sessions under projects (augments the MVP's flat labels)
- **Global Actions view** — consolidated cross-session to-do list with inline status management (pending / in progress / done / won't do / postponed), the heart of the [vision.md](vision.md) sidebar model
- **Full-text search** across all sessions
- **Bookmarks** during live sessions (`Cmd+B`)
- *Likely introduces SQLite alongside file storage — see [architecture.md](architecture.md).*

## v1.2 — Richer review

- **Session playback** — audio synced with transcript scroll, bookmark/action markers on a timeline
- **"Prepare for Next Call"** — AI briefing of open items + suggested talking points, fed into the next session's context
- **Speaker diarization** — split the "Remote" stream into individual speakers (the MVP already labels You vs Remote)
- **Session templates** — saved toggle sets + custom extraction prompts
- **Budget-cap enforcement** — hard stop, not just a cost display

## vNext — Seamless audio & integrations

- **Custom HAL audio plugin** (`.driver`) replacing the BlackHole fork — dynamic device lifecycle, no manual aggregate-device setup, code-signed + notarized (see [architecture.md](architecture.md) Layer 1)
- **Menu bar presence** during live sessions
- **Full keyboard-shortcut set** (the table in [vision.md](vision.md))
- **MCP server** for Claude Desktop

## Reaching Version 1

When the sidebar + projects navigation, the global actions tracker, seamless HAL audio, session playback, and the full "prepare → record → extract → track → prepare" briefing loop are all in place, the product matches **[vision.md](vision.md)**. From there it's refinement, not new pillars.

> **Build forward, not into a corner.** Keep the MVP's data model normalized with stable IDs (see [architecture.md → Data Model](architecture.md#data-model)) so v1.1's projects + global-actions view land as an additive migration rather than a rewrite.
