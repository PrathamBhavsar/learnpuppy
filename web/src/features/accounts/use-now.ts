import { useEffect, useState } from "react"

/** Wall clock that ticks every second while `active`; drives the cooldown countdowns. */
export function useNow(active: boolean): number {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!active) return
    const first = window.setTimeout(() => setNow(Date.now()), 0)
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => {
      window.clearTimeout(first)
      window.clearInterval(timer)
    }
  }, [active])
  return now
}
