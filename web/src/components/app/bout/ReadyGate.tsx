import { useEffect, useState } from "react"
import { motion } from "motion/react"

import type { ReadyAnswers } from "@/api/learning-types"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { ease } from "@/lib/motion"

/** F03: the focus-point timer must hold this long before "Begin" unlocks. */
export const FOCUS_SECONDS = 30

/**
 * F03: phone away, tabs closed, then a focus-point timer. The breathing routine is optional and
 * never gates "Begin" — toggling it on or off never changes when the button unlocks.
 */
export function ReadyGate({
  busy,
  onReady,
}: {
  busy: boolean
  onReady: (ready: ReadyAnswers) => void
}) {
  const [phoneAway, setPhoneAway] = useState(false)
  const [tabsClosed, setTabsClosed] = useState(false)
  const [focusSeconds, setFocusSeconds] = useState(0)
  const [breathing, setBreathing] = useState(false)
  const reduced = useReducedMotion()
  const checklistDone = phoneAway && tabsClosed
  const ready = checklistDone && focusSeconds >= FOCUS_SECONDS

  useEffect(() => {
    if (!checklistDone) return
    const id = window.setInterval(() => {
      setFocusSeconds((s) => Math.min(s + 1, FOCUS_SECONDS))
    }, 1000)
    return () => window.clearInterval(id)
  }, [checklistDone])

  return (
    <section data-testid="step-ready" className="flex flex-col gap-4">
      <h2 className="font-heading text-[16px] font-medium">Get ready</h2>
      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <Checkbox
            id="ready-phone"
            checked={phoneAway}
            onCheckedChange={(v) => setPhoneAway(v === true)}
          />
          <Label htmlFor="ready-phone">Phone is away</Label>
        </div>
        <div className="flex items-center gap-2">
          <Checkbox
            id="ready-tabs"
            checked={tabsClosed}
            onCheckedChange={(v) => setTabsClosed(v === true)}
          />
          <Label htmlFor="ready-tabs">Other tabs are closed</Label>
        </div>
      </div>
      {checklistDone ? (
        <p
          data-testid="focus-timer"
          className="text-[13px] text-muted-foreground"
        >
          {ready
            ? "Focus point reached."
            : `Hold your focus for ${FOCUS_SECONDS - focusSeconds}s more.`}
        </p>
      ) : null}
      <div className="flex flex-col gap-2 border-t pt-3">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="self-start"
          onClick={() => setBreathing((v) => !v)}
        >
          {breathing ? "Hide breathing routine" : "Optional breathing routine"}
        </Button>
        {breathing ? (
          <div className="flex flex-col items-center gap-3 py-4">
            {reduced ? (
              <p className="text-[13px]">
                Breathe in slowly, then breathe out.
              </p>
            ) : (
              <motion.div
                aria-hidden
                data-testid="breathing-circle"
                className="size-16 rounded-full bg-primary/30"
                animate={{ scale: [1, 1.4, 1] }}
                transition={{ duration: 8, repeat: Infinity, ease: ease.inout }}
              />
            )}
            <p
              data-testid="breathing-warning"
              className="max-w-sm text-center text-[12px] text-muted-foreground"
            >
              Never do this in water or while driving. Stop if you feel dizzy.
              This step is always optional.
            </p>
          </div>
        ) : null}
      </div>
      <Button
        className="self-start"
        disabled={!ready || busy}
        onClick={() => onReady({ phoneAway, tabsClosed, focusSeconds })}
      >
        Begin
      </Button>
    </section>
  )
}
