export type RangePreset = "today" | "7d" | "30d" | "custom"

export const RANGE_PRESETS: readonly RangePreset[] = [
  "today",
  "7d",
  "30d",
  "custom",
]

const DAY = 86_400_000

/** `YYYY-MM-DD` in UTC, the digest's day key. */
export function isoDay(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10)
}

export function isRangePreset(value: unknown): value is RangePreset {
  return RANGE_PRESETS.includes(value as RangePreset)
}

const DAY_KEY = /^\d{4}-\d{2}-\d{2}$/
const validDay = (day: string) =>
  DAY_KEY.test(day) &&
  Number.isFinite(Date.parse(day)) &&
  isoDay(Date.parse(day)) === day

/** Resolves a preset (or custom bounds) to the `from`/`to` query the digest endpoint takes. */
export function resolveRange(
  preset: RangePreset,
  custom: { from?: string; to?: string },
  now: number
): { from?: string; to?: string } {
  switch (preset) {
    case "today":
      return { from: isoDay(now), to: isoDay(now) }
    case "7d":
      return { from: isoDay(now - 6 * DAY), to: isoDay(now) }
    case "30d":
      return { from: isoDay(now - 29 * DAY), to: isoDay(now) }
    case "custom": {
      const from =
        custom.from && validDay(custom.from) ? custom.from : undefined
      const to = custom.to && validDay(custom.to) ? custom.to : undefined
      return from && to && from > to ? { from: to, to: from } : { from, to }
    }
  }
}

const dayFormat = new Intl.DateTimeFormat(undefined, {
  weekday: "short",
  month: "short",
  day: "numeric",
  timeZone: "UTC",
})

/** "Today", "Yesterday", or "Tue, Oct 28" for a digest day key. */
export function formatDayLabel(day: string, now: number): string {
  if (!validDay(day)) return "Undated"
  if (day === isoDay(now)) return "Today"
  if (day === isoDay(now - DAY)) return "Yesterday"
  return dayFormat.format(new Date(`${day}T00:00:00Z`))
}
