/** Human durations and clock times for "your AI is limited until…" copy. */

/** "4h 12m", "12m", "45s": the two most significant units. */
function formatSpan(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000))
  const d = Math.floor(total / 86_400)
  const h = Math.floor((total % 86_400) / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  if (d > 0) return `${d}d ${h}h`
  if (h > 0) return `${h}h ${m}m`
  if (m > 0) return total < 600 ? `${m}m ${String(s).padStart(2, "0")}s` : `${m}m`
  return `${s}s`
}

const CLOCK = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
})
const DAY_CLOCK = new Intl.DateTimeFormat(undefined, {
  weekday: "short",
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
})

/** Local wall-clock time of `epochMs`: "14:30", or "Thu 14:30" when it is not today. */
export function formatLocalTime(epochMs: number, now = Date.now()): string {
  const same = new Date(epochMs).toDateString() === new Date(now).toDateString()
  return (same ? CLOCK : DAY_CLOCK).format(new Date(epochMs))
}

/** "resets at 14:30 (in 2h 5m)". */
export function formatResets(epochMs: number, now = Date.now()): string {
  const left = epochMs - now
  if (left <= 0) return "limit should have lifted; try again"
  return `resets at ${formatLocalTime(epochMs, now)} (in ${formatSpan(left)})`
}
