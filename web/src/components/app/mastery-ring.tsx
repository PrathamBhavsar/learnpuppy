import { cn } from "@/lib/utils"

const STROKES = [
  "stroke-mastery-0",
  "stroke-mastery-1",
  "stroke-mastery-2",
  "stroke-mastery-3",
  "stroke-mastery-4",
] as const

/** 0..1 mastery to the 0..4 color ramp. */
export function masteryLevel(value: number): 0 | 1 | 2 | 3 | 4 {
  const clamped = Math.min(1, Math.max(0, value))
  return Math.min(4, Math.floor(clamped * 5)) as 0 | 1 | 2 | 3 | 4
}

type MasteryRingProps = {
  /** 0..1 */
  value: number
  size?: number
  strokeWidth?: number
  label?: string
  className?: string
}

export function MasteryRing({
  value,
  size = 28,
  strokeWidth = 3,
  label,
  className,
}: MasteryRingProps) {
  const clamped = Math.min(1, Math.max(0, value))
  const radius = (size - strokeWidth) / 2
  const circumference = 2 * Math.PI * radius
  const percent = Math.round(clamped * 100)
  return (
    <svg
      role="img"
      aria-label={label ?? `Mastery ${percent}%`}
      width={size}
      height={size}
      viewBox={`0 0 ${size} ${size}`}
      className={cn(
        "shrink-0 -rotate-90 rtl:-scale-x-100 rtl:rotate-90",
        className
      )}
      data-level={masteryLevel(clamped)}
    >
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        strokeWidth={strokeWidth}
        className="stroke-muted"
      />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        strokeWidth={strokeWidth}
        strokeLinecap="butt"
        strokeDasharray={circumference}
        strokeDashoffset={circumference * (1 - clamped)}
        className={cn(
          STROKES[masteryLevel(clamped)],
          "transition-[stroke-dashoffset] duration-[var(--dur-slow)] ease-[var(--ease-out)]"
        )}
      />
    </svg>
  )
}
