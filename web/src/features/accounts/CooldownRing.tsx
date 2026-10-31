import { useState } from "react"

import { cooldownFraction, formatCountdown } from "./model"

const RADIUS = 14
const CIRCUMFERENCE = 2 * Math.PI * RADIUS

type CooldownRingProps = {
  /** Epoch ms when the cooldown ends. */
  until: number
  now: number
  testId: string
}

/**
 * Ring that drains linearly over the real remaining time. The API only reports the end of the
 * cooldown, so the first remaining time seen for this end instant is taken as the full ring.
 */
export function CooldownRing({ until, now, testId }: CooldownRingProps) {
  const remaining = Math.max(0, until - now)
  const [total, setTotal] = useState({
    until,
    ms: Math.max(remaining, 1000),
  })
  let totalMs = total.ms
  if (total.until !== until) {
    totalMs = Math.max(remaining, 1000)
    setTotal({ until, ms: totalMs })
  }
  const fraction = cooldownFraction(remaining, totalMs)
  const label = remaining > 0 ? formatCountdown(remaining) : "ready soon"

  return (
    <div
      data-testid={testId}
      role="timer"
      aria-label={`Cooldown ${label}`}
      className="relative grid size-10 shrink-0 place-items-center"
    >
      <svg viewBox="0 0 36 36" className="size-full -rotate-90" aria-hidden>
        <circle
          cx="18"
          cy="18"
          r={RADIUS}
          fill="none"
          strokeWidth="3"
          className="stroke-muted"
        />
        <circle
          cx="18"
          cy="18"
          r={RADIUS}
          fill="none"
          strokeWidth="3"
          strokeLinecap="butt"
          strokeDasharray={CIRCUMFERENCE}
          strokeDashoffset={CIRCUMFERENCE * (1 - fraction)}
          className="lp-ring-arc stroke-warning"
        />
      </svg>
      <span
        data-testid={`${testId}-text`}
        className="absolute text-[9px] leading-none font-medium tabular-nums"
      >
        {remaining > 0 ? formatCountdown(remaining) : "0s"}
      </span>
    </div>
  )
}
