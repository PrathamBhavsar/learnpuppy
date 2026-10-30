import { MotionConfig } from "motion/react"
import { type ReactNode, useEffect } from "react"

import { DirectionProvider } from "@/components/ui/direction"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import {
  applyDirection,
  applyReduceMotion,
  applyResolvedTheme,
} from "@/lib/theme"
import { useUi } from "@/stores/ui"

/** Theme, direction and motion preferences for the whole tree. */
export function AppProviders({ children }: { children: ReactNode }) {
  const direction = useUi((s) => s.direction)
  const reduceMotion = useUi((s) => s.reduceMotion)
  const theme = useUi((s) => s.theme)
  const resolvedTheme = useUi((s) => s.resolvedTheme)
  const syncSystemTheme = useUi((s) => s.syncSystemTheme)

  // The inline script already painted these; this keeps the DOM in step with the store.
  useEffect(() => {
    applyResolvedTheme(resolvedTheme)
    applyDirection(direction)
    applyReduceMotion(reduceMotion)
  }, [resolvedTheme, direction, reduceMotion])

  useEffect(() => {
    if (theme !== "system") return
    const query = window.matchMedia("(prefers-color-scheme: dark)")
    query.addEventListener("change", syncSystemTheme)
    return () => query.removeEventListener("change", syncSystemTheme)
  }, [theme, syncSystemTheme])

  return (
    <DirectionProvider dir={direction}>
      <MotionConfig reducedMotion={reduceMotion ? "always" : "user"}>
        <TooltipProvider delayDuration={300}>
          {children}
          <Toaster
            position={direction === "rtl" ? "bottom-left" : "bottom-right"}
          />
        </TooltipProvider>
      </MotionConfig>
    </DirectionProvider>
  )
}
