import { Button } from "@/components/ui/button"
import { SHELL_INFO } from "@/components/app/info/shell-content"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useDirectionPref } from "@/hooks/use-theme"

/** Flips the layout between LTR and RTL (also reachable with `?dir=rtl`). */
export function DirectionToggle() {
  const { direction, setDirection } = useDirectionPref()
  const next = direction === "ltr" ? "rtl" : "ltr"
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="sm"
          data-testid="direction-toggle"
          aria-label={`Layout direction: ${direction.toUpperCase()}. Switch to ${next.toUpperCase()}`}
          onClick={() => setDirection(next)}
          className="w-10 tabular-nums"
        >
          {direction.toUpperCase()}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{SHELL_INFO.direction.short}</TooltipContent>
    </Tooltip>
  )
}
