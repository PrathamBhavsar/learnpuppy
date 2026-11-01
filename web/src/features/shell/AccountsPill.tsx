import { Link } from "@tanstack/react-router"
import { useEffect, useState } from "react"

import { useAccounts } from "@/api/queries/accounts"
import { aiState } from "@/lib/accounts-model"
import { formatLocalTime } from "@/lib/ai-time"
import { Button } from "@/components/ui/button"
import { SHELL_INFO } from "@/components/app/info/shell-content"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"

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
    data?.some(
      (a) => a.status === "cooldown" && (a.resetsAt ?? a.cooldownUntil)
    ) ?? false
  const now = useNow(cooling)

  if (isPending || !data)
    return (
      <Skeleton
        className="h-7 w-24"
        data-testid="accounts-pill-loading"
        data-tour="accounts"
      />
    )

  const ai = aiState(data)
  const { total, ready } = ai
  // `offline` / `none` (nothing connected) / `limited` (every AI at its limit) beat the ready count.
  const state =
    ai.kind === "ok"
      ? "all"
      : ai.kind === "partial"
        ? "partial"
        : ai.kind === "limited"
          ? "limited"
          : ai.kind
  const next = data
    .filter((a) => a.status === "cooldown" && (a.resetsAt ?? a.cooldownUntil))
    .map((a) => (a.resetsAt ?? a.cooldownUntil) as number)
    .sort((a, b) => a - b)[0]
  const countdown =
    next !== undefined && next > now ? formatCountdown(next - now) : null
  const label =
    ai.kind === "offline"
      ? "Offline"
      : ai.kind === "none"
        ? "Connect an AI"
        : ai.kind === "limited"
          ? ai.nextResetAt
            ? `All limited · back ${formatLocalTime(ai.nextResetAt, now)}`
            : "All limited"
          : `${ready}/${total} ready`

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          asChild
          variant="outline"
          size="sm"
          className="gap-1.5 tabular-nums"
        >
          <Link
            to="/accounts"
            data-testid="accounts-pill"
            data-tour="accounts"
            data-state={state}
          >
            <span
              aria-hidden
              className={
                state === "none" || state === "limited"
                  ? "size-1.5 bg-destructive"
                  : state === "partial" || state === "offline"
                    ? "size-1.5 bg-warning"
                    : "size-1.5 bg-success"
              }
            />
            <span className="sr-only">Accounts </span>
            {label}
            {countdown && ai.kind !== "limited" ? (
              <span
                className="text-muted-foreground"
                data-testid="accounts-countdown"
              >
                · {countdown}
              </span>
            ) : null}
          </Link>
        </Button>
      </TooltipTrigger>
      <TooltipContent>{SHELL_INFO.accounts.short}</TooltipContent>
    </Tooltip>
  )
}
