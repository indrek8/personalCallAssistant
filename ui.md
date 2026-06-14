# UI/UX Design Brief: Personal Call Assistant

## What is this app?

A native macOS desktop application (built with Tauri) that acts as an invisible AI meeting assistant. It captures audio from any meeting app (Teams, Meet, Zoom) via a virtual audio device, transcribes in real-time using local Whisper, and provides live AI analysis during calls. After the call, it extracts actions, decisions, and a summary.

The core loop: **prepare → record → extract → track → prepare**.

## Design Direction

- Native macOS feel — clean, minimal, professional
- **No persistent sidebar** — this is a flow-based app with distinct full-screen views
- Dark mode primary (user is in calls, often screen-sharing — needs to be unobtrusive)
- Light mode support as well
- Monochrome with subtle accent color for active/recording states
- Information-dense but not cluttered — this is a power-user tool, not a consumer app
- Smooth transitions between views (dashboard → new session → live → post-analysis → dashboard)

## Navigation Model

The app has two distinct modes:

1. **Dashboard mode** — mail-inbox style split view for browsing sessions and managing actions
2. **Session mode** — full-screen immersive views for live recording and post-processing

```
┌──────────┐    ┌─────────────────────────┐    ┌─────────────┐    ┌────────────┐    ┌─────────────┐
│  Setup   │───>│      Dashboard          │───>│ New Session  │───>│   Live     │───>│   Post-     │
│(one-time)│    │  (mail-inbox layout)    │<──>│   (form)     │    │  Session   │    │ Processing  │
└──────────┘    │                         │    └─────────────┘    └────────────┘    └──────┬──────┘
                │  ┌─────────┬──────────┐ │                                                │
                │  │ Session │ Session  │ │<───────────────────────────────────────────────┘
                │  │  List   │ Detail   │ │                                          (Save & Close)
                │  └─────────┴──────────┘ │
                └─────────────────────────┘
                  Settings via gear icon
```

**Key principles:**
- **Dashboard** is the home base — always a split-pane layout like Apple Mail. Session list on the left, session detail on the right. You never lose context when browsing.
- **Live session and post-processing** take over the full window — you're in a call, zero distractions.
- Transitions: starting a session = full-screen takeover. Ending + saving = return to dashboard.

## Screens to Design

Please design **6 screens** total. For each screen, provide a detailed high-fidelity mockup.

---

### Screen 1: Setup / Onboarding (shown once)

First launch. The app needs configuration before it can do anything useful. This is a focused setup wizard — no chrome, no navigation, just get configured and go.

**Step-by-step flow (could be a single scrollable page or stepped wizard):**

1. **Welcome** — brief value prop, "Let's get you set up"
2. **API Key** — Claude API key input, with a "Test connection" button that validates it
3. **Audio Device** — select which audio device to capture from (dropdown listing available devices). Brief explanation that they need to set up "Call Assistant" as their meeting app's audio device.
4. **Whisper Model** — choose transcription model (base = fast, small = balanced, medium = accurate). Show estimated speed on their hardware.
5. **Done** — "You're all set. Create your first project to get started."

Should feel quick and painless — 2 minutes max. Each step validates before allowing next.

---

### Screen 2: Dashboard (the home base)

This is where the user lands every time they open the app. It uses a **mail-inbox style split layout** — like Apple Mail or Outlook. Session list on the left, session detail on the right.

**Layout — two panes with a top bar:**

```
+-----------------------------------------------------------------------------------+
| Call Assistant                                        [+ New Session]  [gear]     |
+-----------------------------------------------------------------------------------+
| SESSION LIST                     |  SESSION DETAIL                                |
| (scannable table)                |  (shown when a session is selected)            |
|                                  |                                                |
| Session Name     Labels        Date    Duration  Actions |                             |
|──────────────────────────────────────────|                                                |
| Board Call Q2    KGL           Mar 28  47min     3 (1⚠)  |  Board Call Q2              |
| Sprint Review    Kifiya        Mar 27  28min     1        |  Mar 28, 2026 | 47min |$0.85|
| 1:1 Sarah        Internal     Mar 26  22min     1        |                             |
| Finance Sync     KGL           Mar 25  32min     2        |  SUMMARY                    |
| API Planning     Kifiya        Mar 22  41min     done     |  Discussed CBUAE Phase 2    |
| Tech Review      KGL Dev      Mar 20  55min     done     |  timeline — central bank    |
|                                  |  extended to Aug 2026...     |
|                                  |                              |
|                                  |  ACTIONS              3 of 4|
|                                  |                              |
|                                  |  ✓ DONE  Send CBUAE timeline|
|                                  |    Me | Due: Apr 5           |
|                                  |                              |
|                                  |  ● PENDING  KYC audit review|
|                                  |    Me | Due: Apr 15          |
|                                  |                              |
|                                  |  ⚠ LATE  Cost estimates     |
|                                  |    Ahmed | Due: Apr 8        |
|                                  |                              |
|                                  |  TRANSCRIPT                  |
|                                  |  00:00:05 Speaker            |
|                                  |  Thanks everyone for joining.|
|                                  |  ...                         |
|                                  |                              |
|                                  |         [Re-analyze]         |
+-----------------------------------------------------------------------------------+
```

**Top bar (always visible):**
- App name/logo on the left
- **[+ New Session]** button — primary action, always accessible
- **[Settings gear]** icon — top right
- Optional: search / filter controls

**Left pane: Session list**

A sortable table of all sessions, like a mail inbox:
- **Columns**: Session Name, Labels, Date, Duration, Actions (count + overdue indicator)
- Sorted by date (most recent first) by default
- Clicking a row selects it and loads detail in the right pane
- Selected row is highlighted
- Rows with overdue actions get a subtle visual indicator (⚠ or red dot)
- Labels shown as small colored chips/pills in the row (a session can have multiple)
- **Filter/sort options** in the column headers or a filter bar:
  - Filter by label (multi-select dropdown)
  - Sort by date, duration
  - Search by session name

**Right pane: Session Detail**

When a session is selected in the left pane, the right pane shows its full detail (read-only, scrollable):

- **Header**: Session name, date, duration, API cost
- **Summary**: AI-generated summary text
- **Actions from this session** with current live status:
  - ✓ DONE (green) — completed actions with completion date
  - ● PENDING (amber) — open actions with due date
  - ⚠ LATE (red) — overdue actions with "X days late"
  - Status is editable inline (dropdown) — update action status right from here
- **Transcript** — full scrollable transcript with timestamps
- **[Re-analyze]** button at the bottom
- If no session is selected, show an empty state: "Select a session to view details"

**Empty state (no sessions yet):**
When there are no sessions, the full dashboard shows a welcoming empty state with a call-to-action to create the first project and session.

---

### Screen 3: New Session Setup

User clicked [+ New Session] from the dashboard. Full-screen focused form — this is a transitional view, the user is about to join a call so speed matters.

```
+----------------------------------------------------------+
| [< Back]                              NEW SESSION         |
+----------------------------------------------------------+
|                                                          |
|  Session name: [Board Call Q2               ]            |
|                                                          |
|  Labels:       [KGL] [Board] [+ add label   ]           |
|                                                          |
|  Participants: [Sarah, Ahmed                ] (optional) |
|                                                          |
|  Context for AI:                                         |
|  ┌──────────────────────────────────────────────────┐    |
|  │ Follow-up to Q1 board meeting.                   │    |
|  │ Discussing CBUAE Phase 2 delays and KYC module   │    |
|  │ timeline. Ahmed owes cost estimates from last     │    |
|  │ call.                                            │    |
|  └──────────────────────────────────────────────────┘    |
|                                                          |
|  AI Features:                                            |
|  [F Fact-check] [C Commitments] [S Suggestions] [Q Q&A] |
|   (toggles, on/off, can be changed during session too)   |
|                                                          |
|                                                          |
|              [>>> Start Session]                         |
|                                                          |
+----------------------------------------------------------+
```

- **[< Back]** returns to dashboard without starting
- **Labels** — multi-select from existing labels or type to create new ones. Shown as colored chips. Optional — a session can have zero labels.
- **Context for AI** is the key field — this is how the AI knows what to fact-check against. Should be the largest input, inviting multi-line text.
- **AI feature toggles** — four buttons, defaulting to the user's saved preferences
- **[Start Session]** — large, prominent. Clicking this transitions to the Live Session view. No going back without stopping the session.

---

### Screen 4: Live Session (the most important screen)

The session is actively recording. **Full-screen, zero distractions.** No navigation, no sidebar, no dashboard elements. Just the call experience.

Split horizontally: transcript on top (~60%), AI panel on bottom (~40%), with a thin toolbar at the very top.

```
+----------------------------------------------------------+
| ● REC  Board Call Q2                 00:23:15    [$0.12] |
|                              [⏸ Pause]  [⏹ End Session] |
+----------------------------------------------------------+
| TRANSCRIPT                                               |
|                                                          |
| 00:01:22  Speaker                                        |
| So the timeline for the CBUAE submission is what we      |
| need to nail down today.                                 |
|                                                          |
| 00:01:35  Speaker                                        |
| The central bank pushed their deadline to August. But    |
| there's a hard dependency on the KYC module being        |
| certified first.                                         |
|                                                          |
| 00:01:48  Speaker                                        |
| Right. What's the current status on KYC certification?   |
|                                                          |
| 00:01:53  Speaker                                        |
| We submitted to the auditor last week. Expecting         |
| results by April 15th.                                   |
|                                                          |
| 00:02:10  Speaker                                        |
| Good. And the cost estimates for Phase 2 — Ahmed, did    |
| you get those finalized?                                 |
|                                                          |
|==========================================================|
| AI PANEL                          [F] [C] [S] [Q]       |
|                                                          |
| ⚠ FACT-CHECK  00:01:35                                  |
| Claim: "end of Q2" for CBUAE deadline.                   |
| Context says CBUAE Phase 2 deadline is Aug 2026 per      |
| Central Bank circular CB-2025-041. This is end of Q3,    |
| not Q2.                                                  |
|                                                          |
| 📌 COMMITMENT  00:01:53                                  |
| "Expecting audit results by April 15th"                  |
| → Who: auditor/Ahmed  → Deadline: Apr 15                 |
|                                              [+ Save]    |
|                                                          |
| ❓ UNANSWERED  00:02:10                                  |
| Cost estimates for Phase 2 — question posed, waiting     |
| for answer.                                              |
|                                                          |
| [Ask AI: ________________________________________ ] [>]  |
+----------------------------------------------------------+
```

**Top Toolbar (thin, stays out of the way):**
- Pulsing red recording dot + session name + elapsed timer
- API cost counter — subtle, right side
- [Pause] — pauses recording/transcription, can resume
- [End Session] — stops everything, transitions to Post-Processing. Should require confirmation ("End this session?") to prevent accidental clicks.

**Transcript Area (top ~60%):**
- Rolling transcript, newest at the bottom, auto-scrolls
- Each entry: timestamp + "Speaker" label + text
- Muted timestamps, readable text
- Scroll up to review → auto-scroll pauses. Scroll back to bottom → resumes.
- Clean, minimal — this needs to be readable at a glance during a live call

**AI Panel (bottom ~40%, with a draggable resize handle):**
- **Toggle row** at the top right: [F] [C] [S] [Q] — small pill buttons. Colored = active, dim = off. Toggleable mid-call.
- **Feed of AI findings** — newest on top, each with:
  - Type icon + label (FACT-CHECK, COMMITMENT, SUGGESTION, UNANSWERED)
  - Timestamp linking to the transcript moment
  - The finding content
  - Commitments have a [+ Save] button to immediately capture as an action
- **"Ask AI" input** — fixed at the very bottom
  - Text input + send button
  - User asks a question → AI responds based on transcript + context
  - AI chat responses should be visually distinct from automatic findings (different background or style)
- The AI panel can be collapsed/minimized if user wants full-screen transcript

Design this screen with the realistic sample data shown above — a business call about project timelines with fact-checks and commitments detected.

---

### Screen 5: Post-Processing

Session just ended. AI processes the full transcript. User reviews before saving.

**Two sub-states:**

**5a: Processing (brief, shown while AI works)**
- Centered loading state: "Analyzing your session..."
- Progress indication (spinner or progress bar)
- Session name and duration shown
- Takes 10-30 seconds typically

**5b: Review Results (the main state)**

```
+----------------------------------------------------------+
| POST-ANALYSIS                                            |
| Board Call Q2  |  Mar 28, 2026  |  47 min  |  $0.85     |
+----------------------------------------------------------+
|                                                          |
| SUMMARY                                        [Regen]   |
| ┌──────────────────────────────────────────────────────┐ |
| │ Discussed CBUAE Phase 2 timeline — central bank      │ |
| │ extended deadline to Aug 2026. KYC module             │ |
| │ certification is the critical dependency. Ahmed       │ |
| │ expects auditor results by Apr 15. Budget revision    │ |
| │ needed for Q3 planning.                              │ |
| └──────────────────────────────────────────────────────┘ |
|                                                          |
| ACTIONS                                                  |
|                                                          |
| ☑ Send CBUAE Phase 2 timeline to board                  |
|   Owner: [Me ▾]     Due: [Apr 5  ]              [✕]    |
|   "I'll circulate the updated timeline to the board     |
|    by Friday"                                           |
|                                                          |
| ☑ Deliver KYC audit results                             |
|   Owner: [Ahmed ▾]  Due: [Apr 15 ]              [✕]    |
|   "Expecting results by April 15th"                     |
|                                                          |
| ☑ Prepare budget revision for Q3                        |
|   Owner: [Me ▾]     Due: [Apr 10 ]              [✕]    |
|   "Let's get the revised numbers together before the    |
|    next board"                                          |
|                                                          |
| ☐ Share updated risk assessment                         |
|   Owner: [Sarah ▾]  Due: [       ]              [✕]    |
|   (AI suggestion, not a clear commitment — uncheck to   |
|    discard)                                             |
|                                                          |
| [+ Add action manually]                                 |
|                                                          |
| DECISIONS                                                |
| • Phase 2 deadline moved to Aug 2026                    |
| • Will proceed with current KYC vendor                  |
| • Q3 budget revision triggered                          |
|                                                          |
|     [Back to Transcript]          [Save & Close]        |
|                                                          |
+----------------------------------------------------------+
```

- **Summary** — editable text block, [Regen] button to regenerate
- **Actions** — each with checkbox, owner dropdown, due date, delete button, transcript quote
- Unchecked items won't be saved (AI suggestions the user doesn't want)
- [+ Add action manually] for things the AI missed
- **Decisions** — non-actionable reference bullet points
- **[Back to Transcript]** — opens the raw transcript for reference
- **[Save & Close]** — saves everything, returns to Dashboard

---

### Screen 6: Settings

Accessed from the gear icon in the Dashboard top bar. Full-screen, simple.

```
+----------------------------------------------------------+
| [< Dashboard]                              SETTINGS       |
+----------------------------------------------------------+
|                                                          |
| API CONFIGURATION                                        |
|                                                          |
| Claude API Key                                           |
| [sk-ant-••••••••••••••••••••••••]  [👁] [Test]          |
| Status: ✓ Connected                                     |
|                                                          |
|──────────────────────────────────────────────────────────|
|                                                          |
| AUDIO                                                    |
|                                                          |
| Capture Device                                           |
| [Call Assistant (2ch)                          ▾]        |
|                                                          |
|──────────────────────────────────────────────────────────|
|                                                          |
| TRANSCRIPTION                                            |
|                                                          |
| Whisper Model                                            |
| ( ) Base     — fastest, lower accuracy                   |
| (●) Small    — balanced (recommended)                    |
| ( ) Medium   — slower, higher accuracy                   |
|                                                          |
|──────────────────────────────────────────────────────────|
|                                                          |
| AI DEFAULTS                                              |
|                                                          |
| Default features for new sessions:                       |
| [F Fact-check ✓] [C Commitments ✓]                     |
| [S Suggestions ] [Q Q&A ✓       ]                       |
|                                                          |
|──────────────────────────────────────────────────────────|
|                                                          |
| STORAGE                                                  |
|                                                          |
| Data location:                                           |
| ~/Library/Application Support/CallAssistant              |
|                                      [Open in Finder]    |
|                                                          |
+----------------------------------------------------------+
```

- Single scrollable page, no tabs
- Sections with clear dividers
- API key with masked input, show/hide toggle, test button with status indicator
- Audio device dropdown (lists available capture devices)
- Whisper model as radio buttons with descriptions
- AI toggle defaults
- Storage path display with Finder shortcut

---

## Visual Design Notes

- **Typography**: System font (SF Pro) for native macOS feel. Monospace for timestamps in transcript.
- **Window**: Standard macOS traffic lights (close/minimize/zoom) in top-left. Title bar area integrated with the top bar of each view.
- **Colors**:
  - Dark mode: deep gray/near-black backgrounds, white/light gray text
  - Recording state: red accent (pulsing dot, subtle red tint on toolbar)
  - AI findings: color-coded by type (amber for fact-checks, blue for commitments, green for suggestions, purple for unanswered questions)
  - Action statuses: green (done), amber (pending), red (overdue/late)
  - Toggle buttons: colored when active, dim/outline when off
- **Spacing**: Generous but not wasteful — information-dense for the power user
- **Icons**: SF Symbols style — minimal, line-based
- **Transitions**: The app should feel like navigating forward/back through a flow. Dashboard → New Session slides right. Live Session is an immersive takeover. Post-Processing slides in after the session ends. Save & Close returns to Dashboard.
- **The transcript** should feel like a clean chat log — scannable during a live call
- **The AI panel** should feel like a smart assistant — helpful annotations appearing in real-time, not overwhelming
- **The dashboard** should feel like a project management tool — quick scan of what's open, what needs attention

## Deliverables

For each of the 6 screens, provide:
1. A detailed high-fidelity mockup (dark mode)
2. Annotations explaining key interaction patterns where not obvious
