// Small presentation helpers shared across screens (dates + durations).
// These are real formatters (moved out of the browser-preview `mock.ts` in M5).

/** Short "Mar 28" style date from an ISO-8601 string (falls back to raw). */
export function shortDate(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso || "—";
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

/** Long "Mar 28, 2026" style date. */
export function longDate(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso || "—";
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
