import { useEffect, useRef } from "react"

/**
 * F25: reports each time the learner leaves the tab, for the caller to log server-side
 * (`focusLost`). Pure reporting only — counting a leave and any consequence of one are the
 * server's; this hook never decides anything about the bout.
 */
export function useFocusGuard(onLeave: () => void, enabled = true): void {
  const onLeaveRef = useRef(onLeave)
  useEffect(() => {
    onLeaveRef.current = onLeave
  })
  useEffect(() => {
    if (!enabled) return
    const report = () => {
      if (document.hidden) onLeaveRef.current()
    }
    document.addEventListener("visibilitychange", report)
    return () => document.removeEventListener("visibilitychange", report)
  }, [enabled])
}
