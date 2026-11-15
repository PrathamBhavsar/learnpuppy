import { useEffect, useRef, useState, type ReactNode } from "react"

import { toApiError } from "@/api/errors"
import type { CheckinResult } from "@/api/learning-types"
import { ErrorState } from "@/components/app/error-state"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { toastError } from "@/lib/toast"

import {
  useCheckinNeeded,
  useClock,
  useLearningActions,
} from "@/api/queries/learning"
import { BoutClock } from "./BoutClock"
import { GapPause } from "./GapPause"
import { LockedScreen } from "./LockedScreen"
import { ReadyGate } from "./ReadyGate"
import { SleepCheckin } from "./SleepCheckin"
import { useFocusGuard } from "@/hooks/use-focus-guard"

/** How often an open bout reports that the learner is still here. The server counts the minutes. */
const HEARTBEAT_MS = 60_000

type Props = {
  ws: string
  /** The course the bout is for. Bouts are workspace-wide; this is only recorded on the log. */
  course: string
  /** The learner's own reason for the course, shown at the ready gate (F02). */
  why?: string
  /** Rendered once a bout is open. Call `onClosed` when Close finishes: that ends the bout. */
  children: (api: { onClosed: () => void }) => ReactNode
}

/**
 * F03, F04, F23, F24, F25: everything that stands between the learner and a step. The sleep
 * check-in (first bout of the day), the ready gate, the locked screen, the bout clock, blank gap
 * pauses and the focus guard are all shown here, and every decision (is a bout open, how many
 * minutes, is a pause due, is it locked) comes from the server's clock. The server also refuses
 * `step` without an open bout, so this screen cannot be skipped by calling the API directly.
 */
export function BoutShell({ ws, course, why, children }: Props) {
  const clockQ = useClock(ws)
  const needed = useCheckinNeeded(ws)
  const act = useLearningActions(ws)
  const [checkin, setCheckin] = useState<CheckinResult | null>(null)
  const [checkinSeen, setCheckinSeen] = useState(false)
  const [closed, setClosed] = useState(false)
  const [pausedAt, setPausedAt] = useState<number | null>(null)

  const clock = clockQ.data
  const bout = clock?.bout ?? null
  const live = bout !== null && !closed
  const heartbeat = useRef(act.heartbeat.mutate)
  useEffect(() => {
    heartbeat.current = act.heartbeat.mutate
  })

  useEffect(() => {
    if (!live || bout === null) return
    const beat = () => heartbeat.current(bout)
    beat()
    const id = window.setInterval(beat, HEARTBEAT_MS)
    return () => window.clearInterval(id)
  }, [live, bout])

  useFocusGuard(() => {
    if (bout !== null) act.focusLost.mutate(bout)
  }, live)

  if (!clock) {
    return clockQ.isError ? (
      <ErrorState
        message={toApiError(clockQ.error).message}
        onRetry={() => clockQ.refetch()}
      />
    ) : (
      <Spinner />
    )
  }

  if (bout === null && !closed) {
    const locked = clock.status.status !== "active"
    if ((needed.data === true || checkin !== null) && !checkinSeen && !locked) {
      return (
        <div data-testid="bout-checkin" className="flex flex-col gap-3">
          <SleepCheckin
            busy={act.checkin.isPending}
            suggestReviewOnly={checkin ? checkin.reviewOnlySuggested : null}
            onSubmit={(hours, note) =>
              act.checkin.mutate(
                { sleepHours: hours, note: note === "" ? undefined : note },
                {
                  onSuccess: setCheckin,
                  onError: (e) => toastError(e, "Could not save your check-in"),
                }
              )
            }
          />
          {checkin ? (
            <Button className="self-start" onClick={() => setCheckinSeen(true)}>
              Continue
            </Button>
          ) : null}
        </div>
      )
    }
    if (needed.isPending && !locked) return <Spinner />
    if (locked) {
      return (
        <LockedScreen
          clock={clock}
          overridePending={act.override.isPending}
          onOverride={(reason) =>
            act.override.mutate(
              { reason },
              { onError: (e) => toastError(e, "Could not override") }
            )
          }
        />
      )
    }
    return (
      <ReadyGate
        why={why}
        busy={act.startBout.isPending}
        onReady={(ready) =>
          act.startBout.mutate(
            { course, ready },
            { onError: (e) => toastError(e, "Could not start the bout") }
          )
        }
      />
    )
  }

  const activeMinutes =
    clock.status.status === "active" ? clock.status.activeMinutes : null
  const pauseDue =
    live &&
    clock.gapPauseDue &&
    clock.gapPauseSeconds > 0 &&
    activeMinutes !== null &&
    pausedAt !== activeMinutes

  return (
    <div data-testid="bout-shell" className="flex flex-col gap-4">
      {closed ? (
        <p
          data-testid="bout-ended"
          className="text-[12px] text-muted-foreground"
        >
          Bout ended. The next one opens after the gap.
        </p>
      ) : (
        <BoutClock clock={clock} />
      )}
      {pauseDue && bout !== null ? (
        <GapPause
          seconds={clock.gapPauseSeconds}
          onDone={() => {
            setPausedAt(activeMinutes)
            act.heartbeat.mutate(bout)
          }}
        />
      ) : null}
      {children({
        onClosed: () => {
          if (bout !== null) act.endBout.mutate(bout)
          setClosed(true)
        },
      })}
    </div>
  )
}
