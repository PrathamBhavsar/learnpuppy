import { Button } from "@/components/ui/button"
import { useDirectionPref } from "@/hooks/use-theme"

/** Flips the layout between LTR and RTL (also reachable with `?dir=rtl`). */
export function DirectionToggle() {
  const { direction, setDirection } = useDirectionPref()
  const next = direction === "ltr" ? "rtl" : "ltr"
  return (
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
  )
}
