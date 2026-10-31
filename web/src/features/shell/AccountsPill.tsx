import { Link } from "@tanstack/react-router"
import { useEffect, useState } from "react"

import { useAccounts } from "@/api/queries/accounts"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"

function formatCountdown(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000))
  const m = Math.floor(total / 60)
  const s = total % 60
  return m > 0 ? `${m}m ${String(s).padStart(2, "0")}s` : `${s}s`
}

function useNow(active: boolean): number {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!active) return
    const timer = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(timer)
  }, [active])
  return now
}

/** `2/3 ready`; amber with a countdown while accounts cool down, red when none can answer. */
export function AccountsPill() {
  const { data, isPending } = useAccounts()
  const cooling =
    data?.some((a) => a.status === "cooldown" && a.cooldownUntil) ?? false
  const now = useNow(cooling)

  if (isPending || !data)
    return <Skeleton className="h-7 w-24" data-testid="accounts-pill-loading" />

  const total = data.length
  const ready = data.filter((a) => a.status === "ready").length
  const state = ready === 0 ? "none" : ready < total ? "partial" : "all"
  const next = data
    .filter((a) => a.status === "cooldown" && a.cooldownUntil)
    .map((a) => a.cooldownUntil as number)
    .sort((a, b) => a - b)[0]
  const countdown =
    next !== undefined && next > now ? formatCountdown(next - now) : null

  return (
    <Button
      asChild
      variant="outline"
      size="sm"
      className="gap-1.5 tabular-nums"
    >
      <Link to="/accounts" data-testid="accounts-pill" data-state={state}>
        <span
          aria-hidden
          className={
            state === "none"
              ? "size-1.5 bg-destructive"
              : state === "partial"
                ? "size-1.5 bg-warning"
                : "size-1.5 bg-success"
          }
        />
        <span className="sr-only">Accounts </span>
        {ready}/{total} ready
        {countdown ? (
          <span
            className="text-muted-foreground"
            data-testid="accounts-countdown"
          >
            · {countdown}
          </span>
        ) : null}
      </Link>
    </Button>
  )
}
