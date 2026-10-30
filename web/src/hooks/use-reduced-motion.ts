import { useReducedMotion as useSystemReducedMotion } from "motion/react"

import { useUi } from "@/stores/ui"

/** True when the OS asks for reduced motion or the Settings override is on. */
export function useReducedMotion(): boolean {
  const system = useSystemReducedMotion()
  const override = useUi((s) => s.reduceMotion)
  return override || system === true
}
