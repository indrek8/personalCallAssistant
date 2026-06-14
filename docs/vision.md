# Version 1 — Product Vision

> **This is the destination, not the build plan.** Version 1 is the full, ambitious product described below. We get here *iteratively*: the first iteration is the **[MVP](mvp.md)**, and **[roadmap.md](roadmap.md)** is the step-by-step path that walks the MVP toward this vision. The technical design — stack, audio architecture, data model — lives in **[architecture.md](architecture.md)**.

---

## The Idea

A native macOS desktop application that acts as a personal AI meeting assistant. It sits between the microphone and speaker as a virtual audio proxy, listens to calls, transcribes in real-time, and provides live AI assistance during meetings. Works with any meeting platform (Teams, Google Meet, Zoom) by installing as a virtual microphone/speaker that you select in the meeting app.

Beyond recording, the app is a **meeting-driven work tracker**. Meetings generate actions — things you promised, things others promised, decisions made, follow-ups needed. Those actions live in the app, linked back to the exact moment in the conversation. The core loop: **prepare → record → extract → track → prepare**.

## Core Goals

- Capture meeting audio transparently (no bot joining the call)
- Real-time local speech-to-text transcription
- Live AI assistance (fact-checking, Q&A, suggestions) during calls
- Post-meeting extraction of actions, decisions, follow-ups
- Consolidated action tracking across all sessions
- Everything linked: action → session → transcript moment

## Target Platforms for Calls

- Microsoft Teams
- Google Meet
- Zoom
- Any app that lets you select a microphone/speaker device

---

## UI: Single Window, Sidebar + Main Area

No tabs, no multiple views. One window. Sidebar for navigation, main area shows whatever you selected. Like a mail client or Apple Notes.

```
+------------------+------------------------------------------+
| SIDEBAR          | MAIN AREA                                |
|                  | (changes based on sidebar selection)     |
| [+ New Session]  |                                          |
|                  |                                          |
| ACTIONS (12)     |                                          |
|                  |                                          |
| PROJECTS         |                                          |
|  > KGL (3 ses)   |                                          |
|    Board Q2      |                                          |
|    Tech Review   |                                          |
|    Finance Sync  |                                          |
|  > Kifiya (2)    |                                          |
|    Sprint Rev    |                                          |
|    API Planning  |                                          |
|  > Internal (1)  |                                          |
|    1:1 Sarah     |                                          |
|                  |                                          |
| [+ New Project]  |                                          |
+------------------+------------------------------------------+
```

**Sidebar elements:**

- **[+ New Session]** button at the top — the primary action
- **Actions** — consolidated action list across all sessions (with badge count of pending)
- **Projects** — collapsible tree, each project contains its sessions
- **[+ New Project]** — create a new project at the bottom

Clicking anything in the sidebar changes the main area.

> **MVP note:** the MVP starts flatter — a mail-inbox dashboard with **labels** instead of a projects tree, and actions scoped to each session rather than a global list. The sidebar + projects + global-actions model here is the v1 target. See [mvp.md](mvp.md) and [roadmap.md](roadmap.md).

---

## Main Area: Five States

The main area has five states depending on what's happening.

### State 1: New Session Setup

*When you click [+ New Session].* A simple form to set up the call. Nothing fancy — get in and start fast.

```
+------------------------------------------+
| NEW SESSION                              |
|                                          |
| Project:     [KGL Financial       v]     |
| Session name:[Board Call Q2        ]     |
| Template:    [Investor Call       v]     |
| Budget:      [$5.00               ]     |
|                                          |
| Participants (optional):                 |
| [Sarah, Ahmed                     ]     |
|                                          |
| Context for AI (optional):              |
| [Follow-up to Q1 board meeting.   ]     |
| [Discussing CBUAE Phase 2 delays  ]     |
| [and KYC module timeline.         ]     |
|                                          |
|         [>>> Start Session]              |
+------------------------------------------+
```

**Context for AI** is key — you paste in background info so the AI can fact-check and answer questions intelligently during the call. This is where "prepare for next call" output goes.

### State 2: Live Session (the star)

*After hitting Start Session. This is where you spend most of your time.* The live view is split horizontally: transcript on top, AI panel on bottom. Controls float as a thin toolbar. The transcript gets the most space.

```
+------------------------------------------+
| *REC  KGL / Board Call Q2      00:23:15  |
| [||] [End] [!Bookmark] [$0.12/$5]       |
| Mic: AirPods Pro ▾  | Spk: MacBook ▾    |
+------------------------------------------+
| TRANSCRIPT                         [v AI]|
|                                          |
| 14:03:22  YOU                            |
| So the timeline for the CBUAE submission |
| is what we need to nail down today.      |
|                                          |
| 14:03:28  SARAH                          |
| The central bank pushed to August. But   |
| there's a hard dependency on the KYC     |
| module being certified first.            |
|                                          |
| 14:03:45  AHMED                          |
| We submitted to the auditor last week.   |
| Expecting results by April 15th.         |
|                                          |
|==========================================|
| AI PANEL                [toggles: F Q S C]|
|                                          |
| FACT-CHECK (14:03:30)                    |
| "End of Q2" claim doesn't match. CBUAE  |
| Phase 2 deadline is Aug 2026 per Central |
| Bank circular CB-2025-041.              |
|                                          |
| COMMITMENT DETECTED (14:03:45)           |
| Ahmed: "KYC audit results by Apr 15"    |
|                        [+ Save Action]   |
|                                          |
| [Ask AI: __________________________ ][>] |
+------------------------------------------+
```

**Top toolbar** (minimal, stays out of the way): recording indicator + session name + timer; Pause / End; Bookmark (Cmd+B); API cost meter; and **device selectors** — dropdowns for the real mic and real speaker, switchable mid-call.

**Device selection** — because the meeting app is locked to our virtual devices, we own the real device routing:
- User selects which real mic and real speaker to use in our toolbar
- Switching mid-call is seamless — meeting app sees no interruption (still connected to the virtual device)
- Dropdowns auto-update on hotplug events (AirPods connect/disconnect, headphones plugged in)
- If the active device disappears mid-call, auto-fallback to system default
- Defaults can be set in preferences (e.g., "always prefer AirPods when available")

**Transcript area** (top ~60%): rolling, auto-scrolls; speaker labels with distinct colors; timestamps; bookmarks appear inline as highlighted markers; scroll up to review, auto-scroll resumes at the bottom.

**AI Panel** (bottom ~40%, resizable divider):
- **Toggle row**: F(act-check) Q(&A) S(uggestions) C(ommitments) — click to toggle each on/off mid-call. Lit up = active.
- Latest AI outputs in a reverse-chronological feed
- Each commitment detection has a **[+ Save Action]** button to capture it immediately
- **Ask AI** input at the bottom — type a question, get an answer based on the transcript so far
- The panel can collapse with [v AI] for a full-screen transcript

**What "Ask AI" can do during a call:**
- "What exactly did Ahmed say about the timeline?"
- "Summarize what we've agreed so far"
- "What are the open questions we haven't addressed?"
- "Is this deadline realistic given what Sarah said earlier?"

### State 3: Post-Analysis

*After hitting End Session.* The AI processes the full transcript and you review the results before saving. The session isn't "done" until you review and confirm.

```
+------------------------------------------+
| POST-ANALYSIS: KGL / Board Call Q2       |
| Mar 28, 2026 | 47 min | 3 participants  |
+------------------------------------------+
| SUMMARY                          [Regen] |
| Discussed CBUAE Phase 2 timeline -       |
| central bank extended to Aug 2026.       |
| KYC module certification is the          |
| critical dependency. Ahmed expects       |
| auditor results by Apr 15.              |
|------------------------------------------|
| EXTRACTED ACTIONS                        |
|                                          |
| [x] Send CBUAE Phase 2 timeline to board|
|     Owner: [Me    v] Due: [Apr 5 ] [del]|
|     "I'll circulate the updated          |
|      timeline to the board by Friday"    |
|                                          |
| [x] Deliver KYC audit results           |
|     Owner: [Ahmed v] Due: [Apr 15] [del]|
|     "Expecting results by April 15th"   |
|                                          |
| [ ] Share updated risk assessment        |
|     Owner: [Sarah v] Due: [     ] [del] |
|     (suggestion, not a commitment -      |
|      uncheck to discard)                 |
|                                          |
| [+ Add action manually]                 |
|                                          |
| DECISIONS                                |
| - Phase 2 deadline moved to Aug 2026    |
| - Will proceed with current KYC vendor  |
|                                          |
|     [Save & Close]   [Back to Transcript]|
+------------------------------------------+
```

You can: edit the AI summary (or [Regen]); review extracted actions (check/uncheck to include or discard, change owner, set deadline, delete false positives, add missed ones, each showing its source quote); see captured decisions; jump [Back to Transcript]; and [Save & Close] to persist everything.

### State 4: Session Review

*When you click a past session in the sidebar.* A read-only view of a completed session — transcript + summary + actions in one scrollable page.

```
+------------------------------------------+
| KGL / Board Call Q2                      |
| Mar 28, 2026 | 47 min | $0.85 API cost  |
+------------------------------------------+
| SUMMARY                                  |
| Discussed CBUAE Phase 2 timeline...      |
|------------------------------------------|
| ACTIONS FROM THIS SESSION         3 of 4 |
| [DONE]  Send CBUAE timeline to board    |
|         Me | Due: Apr 5 | Done: Apr 4   |
| [PEND]  Review KYC audit results        |
|         Me | Due: Apr 15                 |
| [LATE]  Send revised cost estimates     |
|         Ahmed | Due: Apr 8 (2 days late) |
|------------------------------------------|
| TRANSCRIPT                               |
| 14:00:05  YOU                            |
| Thanks everyone for joining...           |
| [! BOOKMARK] "Q2 deadline clarification" |
|                                          |
|     [Prepare for Next Call]  [Re-analyze]|
+------------------------------------------+
```

- Summary at top for a quick refresh
- Actions with current status (they may have been updated since the session)
- Full transcript with bookmarks highlighted
- **[Prepare for Next Call]** — AI generates a briefing: what's still open, what was decided, suggested talking points. Output goes to the clipboard or into a new session's context field.
- **[Re-analyze]** — re-run extraction with a different prompt

### State 5: Actions (consolidated)

*When you click "Actions" in the sidebar.* All actions from all sessions in one flat list — your todo list between meetings.

```
+------------------------------------------+
| ACTIONS                                  |
| [All v] [Pending v] [All owners v] [Q]  |
+------------------------------------------+
| MINE                              8 open |
| [pending v] Send CBUAE timeline to board |
|   KGL > Board Call Q2 | Due: Apr 5      |
| [pending v] Prepare budget revision Q3   |
|   KGL > Finance Sync  | Due: Apr 10     |
|------------------------------------------|
| WAITING ON OTHERS                 4 open |
| [pending v] Ahmed: Cost estimates        |
|   KGL > Board Call Q2 | Due: Apr 8  !!! |
| [pending v] Dev team: API sandbox setup  |
|   Kifiya > Sprint Rev  | Due: Apr 12    |
+------------------------------------------+
```

Each action has an inline **status dropdown** (pending / in progress / done / won't do / postponed) — one click to change status, no modal. The "KGL > Board Call Q2" link jumps to that session. Filters at the top: project, status, owner (mine/others/specific), and search. Overdue items get a visual indicator.

### Session Playback

Any completed session can be played back. Audio, transcript, bookmarks, and AI annotations are all timestamped — playback syncs them together like a video player scrubbing through the meeting.

- **Audio playback** with play/pause, skip, and speed (0.5x–2x)
- **Timeline scrubber** with markers for bookmarks and actions — click any marker to jump
- **Transcript auto-scrolls** to follow audio; the current line is highlighted
- **Click any transcript line** to jump audio to that moment
- **Bookmarks and AI annotations** appear inline at their timestamp
- **Actions appear at their source moment** — see the exact context of a promise

Use cases: hear exactly what someone said at a bookmark; review a call you multitasked through at 2x; settle a dispute by replaying the moment; onboard someone with "listen to the first 10 minutes for context."

---

## The Natural Daily Flow

The app supports this rhythm without switching views:

1. **Open app** → see Actions (12 pending) → scan what's open, update a few statuses
2. **Call coming up** → open the last session for that project → refresh on summary + open actions → [Prepare for Next Call] generates a briefing
3. **Start the call** → [+ New Session] → fill form → Start → transcript flows, toggle AI features, bookmark moments, save actions on the fly
4. **Call ends** → End → post-analysis → review extracted actions, edit owners/deadlines → Save & Close
5. **Between calls** → Actions → manage your list, follow up on overdue items from others

It's always one click to get anywhere. The sidebar is your anchor.

---

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+N` | New session |
| `Cmd+B` | Bookmark (during live session) |
| `Cmd+/` | Focus "Ask AI" input |
| `Cmd+.` | End session |
| `Space` | Pause/resume recording (when live) |
| `Cmd+1` | Go to Actions |

## Menu Bar Presence

When the app is minimized during a live session:

```
[*] 00:23:15 | $0.12
    [|| Pause]
    [! Bookmark]
    [Open]
    [End Session]
```

---

## Market Context (why bot-free)

**Bot-free tools** (our category):
- **Granola** — local recording, MCP server, Recipes (~$14–18/month)
- **Jamie** — bot-free, deletes audio after transcription
- **Krisp** — virtual mic/speaker for noise cancellation + recording

**Bot-based tools** (the visible-bot approach we avoid):
- **Fireflies.ai** — MCP server, AskFred query feature
- **Fathom** — generous free tier, visible bot joins the call
- **MeetGeek** — flexible bot/no-bot modes

Our differentiator: a transparent virtual-audio proxy (no bot in the call) **plus** a meeting-driven action tracker that links every commitment back to the exact moment it was made.
