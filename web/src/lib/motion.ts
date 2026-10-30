// Motion tokens (docs/web/05). Durations are seconds for `motion`; the CSS twins live in styles/motion.css.
import type { Transition, Variants } from "motion/react"

export const dur = {
  instant: 0.08,
  fast: 0.14,
  base: 0.22,
  slow: 0.36,
  hero: 0.6,
} as const

export const ease = {
  out: [0.22, 1, 0.36, 1],
  in: [0.64, 0, 0.78, 0],
  inout: [0.65, 0, 0.35, 1],
} as const

export const spring = {
  snappy: { type: "spring", stiffness: 520, damping: 38 },
  gentle: { type: "spring", stiffness: 220, damping: 26 },
} as const satisfies Record<string, Transition>

/** Exit runs at 70% of the matching enter duration. */
const exitDuration = (enter: number): number => enter * 0.7

const STAGGER_STEP = 0.024
const STAGGER_MAX_ITEMS = 8
const STAGGER_MAX_LIST = 30

/** 24 ms per item, capped at 8 items (the rest appear together); lists over 30 never stagger. */
export function staggerDelay(index: number, count: number): number {
  if (count > STAGGER_MAX_LIST) return 0
  return Math.min(index, STAGGER_MAX_ITEMS - 1) * STAGGER_STEP
}

/** Route / panel enter: 8 px rise + fade. */
export const fadeRise: Variants = {
  hidden: { opacity: 0, y: 8 },
  visible: {
    opacity: 1,
    y: 0,
    transition: { duration: dur.base, ease: ease.out },
  },
  exit: {
    opacity: 0,
    y: -6,
    transition: { duration: exitDuration(dur.fast), ease: ease.in },
  },
}
