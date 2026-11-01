import { useEffect, useState } from "react"

import type { GraphEngine, MarqueeRect } from "../canvas/engine"

/** The rubber-band rectangle drawn while Shift-dragging (docs/web/05 #20). */
export function MarqueeBox({ engine }: { engine: GraphEngine | null }) {
  const [rect, setRect] = useState<MarqueeRect | null>(null)
  useEffect(() => engine?.on("marqueeRect", setRect), [engine])
  if (!rect) return null
  return (
    <div
      aria-hidden
      data-testid="graph-marquee"
      dir="ltr"
      style={{
        insetInlineStart: 0,
        top: 0,
        width: rect.w,
        height: rect.h,
        transform: `translate(${rect.x}px, ${rect.y}px)`,
      }}
      className="pointer-events-none absolute z-10 animate-in border border-primary bg-primary/10 duration-150 fade-in-0"
    />
  )
}
