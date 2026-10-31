import { Slider } from "@/components/ui/slider"

type ConfidenceSliderProps = {
  value: number
  max: number
  onChange: (value: number) => void
  disabled?: boolean
}

/** How sure the learner is (1..max). The fill runs from warning to success as confidence rises. */
export function ConfidenceSlider({
  value,
  max,
  onChange,
  disabled,
}: ConfidenceSliderProps) {
  const pct = max > 1 ? Math.round(((value - 1) / (max - 1)) * 100) : 100
  return (
    <div className="flex flex-col gap-1.5" data-testid="confidence">
      <div className="flex items-center justify-between text-[12px]">
        <span id="confidence-label" className="text-muted-foreground">
          How sure are you?
        </span>
        <span
          data-testid="confidence-value"
          className="font-medium tabular-nums"
          aria-live="polite"
        >
          {value} / {max}
        </span>
      </div>
      <Slider
        min={1}
        max={max}
        step={1}
        value={[value]}
        onValueChange={([next]) => onChange(next ?? value)}
        disabled={disabled}
        aria-labelledby="confidence-label"
        data-testid="confidence-slider"
        style={{ "--conf": `${pct}%` } as React.CSSProperties}
        className="[&_[data-slot=slider-range]]:bg-[color-mix(in_oklch,var(--warning),var(--success)_var(--conf))]"
      />
      <div className="flex justify-between text-[11px] text-muted-foreground">
        <span>Guessing</span>
        <span>Certain</span>
      </div>
    </div>
  )
}
