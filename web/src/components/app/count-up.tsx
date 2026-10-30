import { animate, motion, useMotionValue, useTransform } from "motion/react"
import { useEffect } from "react"

import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { cn } from "@/lib/utils"
import { dur, ease } from "@/lib/motion"

type CountUpProps = {
  value: number
  /** Decimal places shown while tweening (default 0). */
  decimals?: number
  format?: (value: number) => string
  className?: string
}

/** Tweens to `value` over 400 ms; tabular numerals; not announced while it runs. */
export function CountUp({
  value,
  decimals = 0,
  format,
  className,
}: CountUpProps) {
  const reduced = useReducedMotion()
  const motionValue = useMotionValue(value)
  const text = useTransform(motionValue, (v) => {
    const rounded = Number(v.toFixed(decimals))
    return format
      ? format(rounded)
      : rounded.toLocaleString(undefined, { minimumFractionDigits: decimals })
  })

  useEffect(() => {
    if (reduced) {
      motionValue.set(value)
      return
    }
    const controls = animate(motionValue, value, {
      duration: dur.slow + 0.04,
      ease: ease.out,
    })
    return () => controls.stop()
  }, [value, reduced, motionValue])

  return (
    <motion.span
      aria-live="off"
      data-value={value}
      className={cn("tabular-nums", className)}
    >
      {text}
    </motion.span>
  )
}
