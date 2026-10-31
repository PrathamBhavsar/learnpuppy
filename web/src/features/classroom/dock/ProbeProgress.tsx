import { cn } from "@/lib/utils"

type ProbeProgressProps = {
  /** Questions answered so far. */
  done: number
  max: number
  className?: string
}

/** Segmented bar, one segment per probe question; each fill scales in from the inline start. */
export function ProbeProgress({ done, max, className }: ProbeProgressProps) {
  return (
    <div
      role="progressbar"
      aria-label="Probe progress"
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={Math.min(done, max)}
      data-testid="probe-progress"
      className={cn("flex gap-1", className)}
    >
      {Array.from({ length: max }, (_, i) => (
        <span key={i} className="relative h-1 flex-1 bg-muted">
          {i < done ? (
            <span
              data-testid="probe-segment-filled"
              className="lp-seg-fill absolute inset-0 bg-primary"
            />
          ) : null}
        </span>
      ))}
    </div>
  )
}
