import { useDirection } from "@/components/ui/direction"

type ResizeHandleProps = {
  width: number
  min: number
  max: number
  onChange: (width: number) => void
  /** The element whose inline-end edge the panel hugs. */
  containerRef: React.RefObject<HTMLElement | null>
}

const STEP = 16

/** Vertical splitter for a panel at the inline end: drag, or focus it and use the arrow keys. */
export function ResizeHandle({
  width,
  min,
  max,
  onChange,
  containerRef,
}: ResizeHandleProps) {
  const rtl = useDirection() === "rtl"
  const clamp = (value: number) =>
    Math.min(max, Math.max(min, Math.round(value)))

  const onPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId)
  }
  const onPointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) return
    const rect = containerRef.current?.getBoundingClientRect()
    if (!rect) return
    onChange(
      clamp(rtl ? event.clientX - rect.left : rect.right - event.clientX)
    )
  }
  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    // Growing the end panel means moving the splitter toward the start edge.
    const grow = rtl ? "ArrowRight" : "ArrowLeft"
    const shrink = rtl ? "ArrowLeft" : "ArrowRight"
    if (event.key === grow) onChange(clamp(width + STEP))
    else if (event.key === shrink) onChange(clamp(width - STEP))
    else if (event.key === "Home") onChange(min)
    else if (event.key === "End") onChange(max)
    else return
    event.preventDefault()
  }

  return (
    // A focusable separator is a WAI-ARIA widget (window splitter); jsx-a11y does not know that.
    // eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize plan panel"
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={width}
      // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
      tabIndex={0}
      data-testid="plan-resize"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onKeyDown={onKeyDown}
      className="group relative z-10 hidden w-1 shrink-0 cursor-col-resize touch-none bg-border outline-none hover:bg-primary/60 focus-visible:bg-primary md:block"
    >
      <span aria-hidden className="absolute -inset-x-1.5 inset-y-0" />
    </div>
  )
}
