import { useEffect, useRef, useState } from "react"
import { motion } from "motion/react"

import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { ease } from "@/lib/motion"

/**
 * F24: a blank pause between steps. The server decides when one is due and how long it runs
 * (`Clock.gapPauseDue`, `gapPauseSeconds`); this only counts it down. There is no skip control —
 * it cannot be dismissed early by a click or a key.
 */
export function GapPause({
  seconds,
  onDone,
}: {
  seconds: number
  onDone: () => void
}) {
  const [remaining, setRemaining] = useState(seconds)
  // A new pause (a fresh `seconds`) resets the countdown; adjusted during render, per React's
  // guidance for state derived from a prop, rather than in an effect.
  const [trackedSeconds, setTrackedSeconds] = useState(seconds)
  if (seconds !== trackedSeconds) {
    setTrackedSeconds(seconds)
    setRemaining(seconds)
  }
  const reduced = useReducedMotion()
  const ref = useRef<HTMLDivElement>(null)
  const onDoneRef = useRef(onDone)

  useEffect(() => {
    onDoneRef.current = onDone
  })
  useEffect(() => {
    ref.current?.focus()
  }, [])

  useEffect(() => {
    if (remaining <= 0) {
      onDoneRef.current()
      return
    }
    const id = window.setTimeout(() => setRemaining((s) => s - 1), 1000)
    return () => window.clearTimeout(id)
  }, [remaining])

  return (
    <div
      ref={ref}
      role="status"
      aria-live="polite"
      data-testid="gap-pause"
      tabIndex={-1}
      className="fixed inset-0 z-50 flex flex-col items-center justify-center gap-3 bg-background outline-none"
    >
      {reduced ? null : (
        <motion.div
          aria-hidden
          data-testid="gap-pause-pulse"
          className="size-3 rounded-full bg-muted-foreground/40"
          animate={{ opacity: [0.3, 1, 0.3] }}
          transition={{ duration: 2, repeat: Infinity, ease: ease.inout }}
        />
      )}
      <p className="text-[13px] text-muted-foreground tabular-nums">
        Pause — {remaining}s
      </p>
    </div>
  )
}
