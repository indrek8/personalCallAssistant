// Mock data for the parts of M1 the backend doesn't serve yet (session detail
// body, live transcript/AI feed, post-analysis). The dashboard *list* and the
// device dropdowns are real; everything here is illustrative content that
// matches design/prototype.html.

import type { SessionMeta } from "./types";

/** Map a label name to one of the prototype's colored chip classes. */
export function labelClass(name: string): string {
  const n = name.toLowerCase();
  if (n.includes("kgl dev") || n.includes("dev")) return "lbl-dev";
  if (n.includes("kgl")) return "lbl-kgl";
  if (n.includes("kif")) return "lbl-kif";
  return "lbl-int";
}

/** Short "Mar 28" style date from an ISO-8601 string (falls back to raw). */
export function shortDate(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

/** Long "Mar 28, 2026" style date. */
export function longDate(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

/** Duration in ms -> "47m" / "1h 05m". */
export function fmtDuration(ms: number): string {
  const totalMin = Math.round(ms / 60000);
  if (totalMin < 60) return `${totalMin}m`;
  const h = Math.floor(totalMin / 60);
  const m = totalMin % 60;
  return `${h}h ${String(m).padStart(2, "0")}m`;
}

/** Sample sessions so the dashboard reads well in a plain-browser preview. */
export const SAMPLE_SESSIONS: SessionMeta[] = [
  {
    id: "sample-1",
    status: "completed",
    name: "Board Call Q2",
    labels: [{ id: "kgl", name: "KGL" }],
    date: "2026-03-28T10:00:00Z",
    duration_ms: 47 * 60000,
    participants: ["Sarah", "Ahmed"],
    context_notes: null,
    budget_cap: 5,
    total_api_cost: 0.85,
  },
  {
    id: "sample-2",
    status: "completed",
    name: "Sprint Review",
    labels: [{ id: "kif", name: "Kifiya" }],
    date: "2026-03-27T14:00:00Z",
    duration_ms: 28 * 60000,
    participants: [],
    context_notes: null,
    budget_cap: 5,
    total_api_cost: 0.31,
  },
  {
    id: "sample-3",
    status: "completed",
    name: "1:1 with Sarah",
    labels: [{ id: "int", name: "Internal" }],
    date: "2026-03-26T09:00:00Z",
    duration_ms: 22 * 60000,
    participants: ["Sarah"],
    context_notes: null,
    budget_cap: 5,
    total_api_cost: 0.18,
  },
];

/** Static transcript lines for the Live screen (M1: mock). */
export const LIVE_TRANSCRIPT = [
  {
    ts: "00:21:22",
    who: "You",
    cls: "who-you",
    said: "So the timeline for the CBUAE submission is what we need to nail down today.",
    muted: false,
  },
  {
    ts: "00:21:35",
    who: "Sarah",
    cls: "who-sar",
    said: "The central bank pushed their deadline to August. But there's a hard dependency on the KYC module being certified first.",
    muted: false,
  },
  {
    ts: "00:21:48",
    who: "You",
    cls: "who-you",
    said: "Right. What's the current status on KYC certification?",
    muted: false,
  },
  {
    ts: "00:21:53",
    who: "Ahmed",
    cls: "who-ahm",
    said: "We submitted to the auditor last week. Expecting results by April 15th.",
    muted: false,
  },
  {
    ts: "00:22:10",
    who: "You",
    cls: "who-you",
    said: "Good. And the cost estimates for Phase 2 — Ahmed, did you get those finalized?",
    muted: true,
  },
];

/** Static AI findings for the Live screen (M1: mock). */
export const LIVE_FINDINGS = [
  {
    kind: "fact" as const,
    label: "Fact-check",
    ts: "00:21:35",
    html: 'Claim of an <b>"end of Q2"</b> deadline conflicts with context: CBUAE Phase 2 is due <b>Aug 2026</b> per Central Bank circular <b>CB-2025-041</b> — that\'s end of Q3.',
    save: false,
  },
  {
    kind: "commit" as const,
    label: "Commitment",
    ts: "00:21:53",
    html: "Ahmed → <b>KYC audit results by Apr 15</b>.",
    save: true,
  },
  {
    kind: "ask" as const,
    label: "Unanswered",
    ts: "00:22:10",
    html: "Cost estimates for Phase 2 — question posed to Ahmed, still waiting on an answer.",
    save: false,
  },
];
