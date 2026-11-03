import { formatResets } from "@/lib/ai-time"

type ResetsLineProps = {
  /** Epoch ms the limit lifts, or null when the CLI did not say. */
  at: number | null | undefined
  now: number
  testId?: string
  prefix?: string
}

/** "Resets at 14:30 (in 2h 5m)", live with `now`. */
export function ResetsLine({ at, now, testId, prefix }: ResetsLineProps) {
  if (!at)
    return (
      <span data-testid={testId}>{prefix}Retrying automatically soon</span>
    )
  const text = formatResets(at, now)
  return (
    <span data-testid={testId}>
      {prefix}
      {text.charAt(0).toUpperCase() + text.slice(1)}
    </span>
  )
}
