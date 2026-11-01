import { PawPrintIcon } from "lucide-react"
import { motion } from "motion/react"
import { Dialog as DialogPrimitive } from "radix-ui"
import {
  type KeyboardEvent,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react"

import { Button } from "@/components/ui/button"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { dur, ease, spring } from "@/lib/motion"
import { useUi } from "@/stores/ui"

import { TOUR_STEPS, type TourStep } from "./tour-steps"
import { shouldAutoStart, useTour } from "./tour-store"

const PAD = 6
const GAP = 14
const CARD_W = 352

type Box = { top: number; left: number; width: number; height: number }

const find = (target: string): HTMLElement | null =>
  document.querySelector<HTMLElement>(`[data-tour="${target}"]`)

const firstTarget = (step: TourStep): HTMLElement | null => {
  for (const t of step.targets ?? []) {
    const el = find(t)
    if (el) return el
  }
  return null
}

/** Steps that apply to what is on screen right now (home vs inside a workspace). */
function resolveSteps(): TourStep[] {
  const has = (t: string) => find(t) !== null
  return TOUR_STEPS.filter((s) => {
    if (s.show && !s.show(has)) return false
    if (s.targets && !firstTarget(s)) return false
    return true
  })
}

function measure(el: HTMLElement | null): Box | null {
  if (!el) return null
  const r = el.getBoundingClientRect()
  if (r.width === 0 || r.height === 0) return null
  return {
    top: r.top - PAD,
    left: r.left - PAD,
    width: r.width + PAD * 2,
    height: r.height + PAD * 2,
  }
}

/** Where the card goes: beside the target when it fits, else below, above, or centred. */
function place(
  box: Box | null,
  card: { w: number; h: number },
  vp: { w: number; h: number },
  rtl: boolean
): { top: number; left: number } {
  const clamp = (v: number, size: number, max: number) =>
    Math.max(12, Math.min(v, max - size - 12))
  if (!box)
    return {
      top: Math.max(12, (vp.h - card.h) / 2),
      left: Math.max(12, (vp.w - card.w) / 2),
    }
  const midY = clamp(box.top + box.height / 2 - card.h / 2, card.h, vp.h)
  const midX = clamp(box.left + box.width / 2 - card.w / 2, card.w, vp.w)
  const right = { top: midY, left: box.left + box.width + GAP }
  const left = { top: midY, left: box.left - card.w - GAP }
  const below = { top: box.top + box.height + GAP, left: midX }
  const above = { top: box.top - card.h - GAP, left: midX }
  const fits = (p: { top: number; left: number }) =>
    p.top >= 12 &&
    p.left >= 12 &&
    p.top + card.h <= vp.h - 12 &&
    p.left + card.w <= vp.w - 12
  const order = rtl ? [left, below, above, right] : [right, below, above, left]
  return order.find(fits) ?? below
}

function Splash({
  onStart,
  onSkip,
}: {
  onStart: () => void
  onSkip: () => void
}) {
  const reduced = useReducedMotion()
  const rise = (delay: number) =>
    reduced
      ? {}
      : {
          initial: { opacity: 0, y: 12 },
          animate: { opacity: 1, y: 0 },
          transition: { duration: dur.hero, ease: ease.out, delay },
        }
  return (
    <div
      className="flex flex-col items-center gap-4 text-center"
      data-testid="tour-splash"
    >
      <motion.span
        {...(reduced
          ? {}
          : {
              initial: { scale: 0.4, opacity: 0, rotate: -20 },
              animate: { scale: 1, opacity: 1, rotate: 0 },
              transition: spring.gentle,
            })}
        aria-hidden
        className="grid size-16 place-items-center bg-primary text-primary-foreground"
      >
        <PawPrintIcon className="size-9" />
      </motion.span>
      <motion.div {...rise(0.25)}>
        <DialogPrimitive.Title className="font-heading text-[32px] leading-tight font-medium">
          LearnPuppy
        </DialogPrimitive.Title>
      </motion.div>
      <motion.div {...rise(0.55)}>
        <DialogPrimitive.Description className="max-w-sm text-[14px] text-muted-foreground">
          Your own tutor. It builds a map of what you know and brings the right
          questions back at the right time.
        </DialogPrimitive.Description>
      </motion.div>
      <motion.div {...rise(0.9)} className="flex items-center gap-2 pt-2">
        <Button onClick={onStart} data-testid="tour-start">
          Show me around
        </Button>
        <Button variant="ghost" onClick={onSkip} data-testid="tour-skip">
          Skip
        </Button>
      </motion.div>
    </div>
  )
}

/**
 * First-run welcome: a short splash, then a spotlight tour over real elements (`data-tour` attributes).
 * Mounted once at the root. Keyboard: Right/Left (or Enter) move, Esc skips; focus is trapped and restored.
 */
export function Tour() {
  const phase = useTour((s) => s.phase)
  const startFirstRun = useTour((s) => s.startFirstRun)
  const startTour = useTour((s) => s.startTour)
  const finish = useTour((s) => s.finish)

  useEffect(() => {
    if (!shouldAutoStart(window.location.search)) return
    const timer = window.setTimeout(startFirstRun, 500)
    return () => window.clearTimeout(timer)
  }, [startFirstRun])

  if (phase === "idle") return null
  return (
    <DialogPrimitive.Root open onOpenChange={(open) => !open && finish()}>
      <DialogPrimitive.Portal>
        {phase === "splash" ? (
          <>
            <DialogPrimitive.Overlay className="fixed inset-0 z-[60] bg-background/90 backdrop-blur-sm" />
            <DialogPrimitive.Content
              className="fixed inset-0 z-[61] grid place-items-center p-6 outline-none"
              onPointerDownOutside={(e) => e.preventDefault()}
              onKeyDown={(e) => e.stopPropagation()}
              onEscapeKeyDown={finish}
              onOpenAutoFocus={(e) => {
                e.preventDefault()
                document
                  .querySelector<HTMLElement>("[data-testid=tour-start]")
                  ?.focus()
              }}
            >
              <Splash onStart={startTour} onSkip={finish} />
            </DialogPrimitive.Content>
          </>
        ) : (
          <TourSteps onDone={finish} />
        )}
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  )
}

function TourSteps({ onDone }: { onDone: () => void }) {
  const reduced = useReducedMotion()
  const rtl = useUi((s) => s.direction) === "rtl"
  const steps = useMemo(() => resolveSteps(), [])
  const [index, setIndex] = useState(0)
  const step = steps[index] ?? steps[0]
  const [box, setBox] = useState<Box | null>(null)
  const [vp, setVp] = useState({ w: window.innerWidth, h: window.innerHeight })
  const [cardH, setCardH] = useState(240)
  const cardRef = useRef<HTMLDivElement>(null)
  const nextRef = useRef<HTMLButtonElement>(null)
  const last = index === steps.length - 1

  const sync = useCallback(() => {
    setBox(step ? measure(firstTarget(step)) : null)
    setVp({ w: window.innerWidth, h: window.innerHeight })
    if (cardRef.current) setCardH(cardRef.current.offsetHeight)
  }, [step])

  useLayoutEffect(() => {
    if (step) firstTarget(step)?.scrollIntoView({ block: "nearest" })
    // Measuring the highlighted element after layout is the point of this effect.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    sync()
  }, [step, sync])

  useEffect(() => {
    window.addEventListener("resize", sync)
    window.addEventListener("scroll", sync, true)
    return () => {
      window.removeEventListener("resize", sync)
      window.removeEventListener("scroll", sync, true)
    }
  }, [sync])

  if (!step) return null

  const go = (delta: number) => {
    const next = index + delta
    if (next < 0) return
    if (next >= steps.length) return onDone()
    setIndex(next)
  }
  const onKeyDown = (e: KeyboardEvent) => {
    e.stopPropagation()
    const forward = rtl ? "ArrowLeft" : "ArrowRight"
    const back = rtl ? "ArrowRight" : "ArrowLeft"
    if (e.key === forward) {
      e.preventDefault()
      go(1)
    } else if (e.key === back) {
      e.preventDefault()
      go(-1)
    }
  }

  const cardW = Math.min(CARD_W, vp.w - 24)
  const pos = place(box, { w: cardW, h: cardH }, vp, rtl)
  const move = reduced
    ? { duration: 0 }
    : { type: "spring" as const, stiffness: 300, damping: 32 }

  return (
    <>
      <DialogPrimitive.Overlay
        className={
          box
            ? "fixed inset-0 z-[60]"
            : "fixed inset-0 z-[60] bg-background/80 backdrop-blur-sm"
        }
        data-testid="tour-overlay"
      >
        {box ? (
          <motion.div
            aria-hidden
            data-testid="tour-spotlight"
            initial={false}
            animate={box}
            transition={move}
            className="pointer-events-none fixed shadow-[0_0_0_100vmax_color-mix(in_oklab,var(--background)_78%,transparent)] ring-2 ring-primary"
          />
        ) : null}
      </DialogPrimitive.Overlay>
      <DialogPrimitive.Content
        asChild
        onPointerDownOutside={(e) => e.preventDefault()}
        onEscapeKeyDown={onDone}
        onOpenAutoFocus={(e) => {
          e.preventDefault()
          nextRef.current?.focus()
        }}
      >
        <motion.div
          ref={cardRef}
          data-testid="tour-card"
          onKeyDown={onKeyDown}
          initial={false}
          animate={{ top: pos.top, left: pos.left }}
          transition={move}
          style={{ width: cardW }}
          className="fixed z-[61] flex flex-col gap-3 bg-popover p-4 text-[13px] text-popover-foreground ring-1 ring-foreground/15 outline-none"
        >
          <div className="flex items-baseline justify-between gap-2">
            <DialogPrimitive.Title
              className="font-heading text-[15px] font-medium"
              data-testid="tour-title"
            >
              {step.title}
            </DialogPrimitive.Title>
            <span className="text-[11px] text-muted-foreground tabular-nums">
              {index + 1} / {steps.length}
            </span>
          </div>
          <DialogPrimitive.Description asChild>
            <div className="flex flex-col gap-2 text-muted-foreground">
              {step.body.map((p) => (
                <p key={p}>{p}</p>
              ))}
            </div>
          </DialogPrimitive.Description>
          <ol aria-hidden className="flex gap-1" data-testid="tour-dots">
            {steps.map((s, i) => (
              <li
                key={s.id}
                data-active={i === index || undefined}
                className={
                  i === index
                    ? "h-1.5 w-4 bg-primary"
                    : i < index
                      ? "h-1.5 w-1.5 bg-primary/50"
                      : "h-1.5 w-1.5 bg-border"
                }
              />
            ))}
          </ol>
          <div className="flex items-center justify-between gap-2">
            <Button
              variant="ghost"
              size="sm"
              onClick={onDone}
              data-testid="tour-skip"
            >
              {last ? "Close" : "Skip tour"}
            </Button>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                disabled={index === 0}
                onClick={() => go(-1)}
                data-testid="tour-back"
              >
                Back
              </Button>
              <Button
                ref={nextRef}
                size="sm"
                onClick={() => go(1)}
                data-testid="tour-next"
              >
                {last ? "Finish" : "Next"}
              </Button>
            </div>
          </div>
          <span role="status" aria-live="polite" className="sr-only">
            Step {index + 1} of {steps.length}: {step.title}
          </span>
        </motion.div>
      </DialogPrimitive.Content>
    </>
  )
}
