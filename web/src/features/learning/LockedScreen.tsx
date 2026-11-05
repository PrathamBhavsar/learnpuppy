import { useState } from "react"

import type { Clock } from "@/api/learning-types"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"

type LockedStatus = Exclude<Clock["status"]["status"], "active">

const COPY: Record<LockedStatus, string> = {
  bout_limit_reached:
    "This bout hit the 90-minute limit. Close it out, then take the gap.",
  gap: "Taking a gap before the next bout.",
  daily_limit_reached: "Today's study time is used up.",
}

/**
 * F23: the between-bout gap, the per-bout limit and the daily limit, all read straight from the
 * server's clock — a refresh re-fetches the same clock, so none of the three can be escaped by
 * reloading. The only way through is the logged, typed-reason override.
 */
export function LockedScreen({
  clock,
  onOverride,
  overridePending,
}: {
  clock: Clock
  onOverride: (reason: string) => void
  overridePending: boolean
}) {
  const [reason, setReason] = useState("")
  const { status } = clock
  if (status.status === "active") return null

  return (
    <section data-testid="screen-locked" className="flex flex-col gap-3">
      <h2 className="font-heading text-[16px] font-medium">Locked</h2>
      <p className="text-[13px]">{COPY[status.status]}</p>
      {status.status === "gap" ? (
        <p
          data-testid="locked-time-left"
          className="text-[13px] tabular-nums"
        >
          Back in {status.remainingMinutes} min.
        </p>
      ) : null}
      <div className="flex flex-col gap-2 border-t pt-3">
        {clock.overrideUsedToday ? (
          <p
            data-testid="override-used"
            className="text-[12px] text-muted-foreground"
          >
            Today&apos;s override is already used.
          </p>
        ) : (
          <>
            <Textarea
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              aria-label="Reason for overriding the limit"
              placeholder="Why does this need to happen now?"
              rows={2}
            />
            <Button
              variant="outline"
              size="sm"
              className="self-start"
              disabled={reason.trim() === "" || overridePending}
              onClick={() => onOverride(reason.trim())}
            >
              Override, just this once
            </Button>
          </>
        )}
      </div>
    </section>
  )
}
