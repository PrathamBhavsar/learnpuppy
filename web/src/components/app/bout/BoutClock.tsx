import type { Clock } from "@/api/learning-types"
import { Progress } from "@/components/ui/progress"

/** F23: displays the server's bout clock. Every number here comes from `Clock`; nothing is
 * computed locally, so a refresh shows the same minutes the server would. */
export function BoutClock({ clock }: { clock: Clock }) {
  const active =
    clock.status.status === "active"
      ? clock.status.activeMinutes
      : clock.boutLimitMinutes
  const pct =
    clock.boutLimitMinutes > 0
      ? Math.round(Math.min(100, (active / clock.boutLimitMinutes) * 100))
      : 0

  return (
    <div data-testid="bout-clock" className="flex flex-col gap-1">
      <div className="flex items-baseline justify-between text-[12px] text-muted-foreground tabular-nums">
        <span>
          {active} of {clock.boutLimitMinutes} min this bout
        </span>
        <span>
          {clock.minutesToday} of {clock.dailyLimitMinutes} min today
        </span>
      </div>
      <Progress value={pct} aria-label="Bout time used" />
    </div>
  )
}
