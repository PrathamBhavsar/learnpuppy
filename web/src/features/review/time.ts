// Small, self-contained formatters for the review screen (docs/web/04: dates/numbers via Intl).
// Kept local to the feature to respect feature-isolation boundaries.

import { useEffect, useState } from "react"

const MIN = 60_000
const HOUR = 3_600_000
const DAY = 86_400_000

const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" })

/** Turns an FSRS interval (in days, fractional for learning steps) into a short label. */
export function formatIntervalDays(days: number): string {
  if (days <= 0) return "<1m"
  const ms = days * DAY
  if (ms < MIN) return "<1m"
  if (ms < HOUR) return `${Math.round(ms / MIN)}m`
  if (ms < DAY) return `${Math.round(ms / HOUR)}h`
  const d = Math.round(ms / DAY)
  if (d < 7) return `${d}d`
  if (d < 30) return `${Math.round(d / 7)}w`
  if (d < 365) return `${Math.round(d / 30)}mo`
  return `${Math.round(d / 365)}y`
}

/** Relative time until a due timestamp (epoch ms), e.g. "in 3h" or "now". */
export function formatRelativeDue(due: number, now = Date.now()): string {
  const diff = due - now
  if (diff <= 0) return "now"
  if (diff < MIN) return "now"
  if (diff < HOUR) return rtf.format(Math.round(diff / MIN), "minute")
  if (diff < DAY) return rtf.format(Math.round(diff / HOUR), "hour")
  return rtf.format(Math.round(diff / DAY), "day")
}

/** The current time, refreshed every `everyMs` so "due in 3m" labels stay honest without impure renders. */
export function useNow(everyMs = 30_000): number {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), everyMs)
    return () => window.clearInterval(timer)
  }, [everyMs])
  return now
}
