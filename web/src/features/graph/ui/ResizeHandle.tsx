/* eslint-disable jsx-a11y/no-noninteractive-element-interactions, jsx-a11y/no-noninteractive-tabindex -- a focusable separator is a splitter widget (WAI-ARIA window splitter pattern) */
import { useRef } from "react"

import { TABLE_MAX, TABLE_MIN } from "@/stores/graphView"

import { copy } from "../copy"

type Props = { height: number; onResize: (height: number) => void }

const STEP = 24

/** Horizontal splitter above the chain table: drag or use the arrow keys. */
export function ResizeHandle({ height, onResize }: Props) {
  const drag = useRef<{ y: number; h: number } | null>(null)
  const clamp = (h: number) => Math.min(TABLE_MAX, Math.max(TABLE_MIN, Math.round(h)))
  return (
    <div
      role="separator"
      aria-orientation="horizontal"
      aria-label={copy.table_.resize}
      aria-valuemin={TABLE_MIN}
      aria-valuemax={TABLE_MAX}
      aria-valuenow={height}
      tabIndex={0}
      data-testid="graph-table-resize"
      className="relative z-10 h-1.5 shrink-0 cursor-row-resize touch-none border-t border-border bg-muted/40 outline-none after:absolute after:inset-x-0 after:-top-1.5 after:h-3 hover:bg-primary/30 focus-visible:bg-primary/40"
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId)
        drag.current = { y: e.clientY, h: height }
      }}
      onPointerMove={(e) => {
        if (drag.current) onResize(clamp(drag.current.h + (drag.current.y - e.clientY)))
      }}
      onPointerUp={() => (drag.current = null)}
      onKeyDown={(e) => {
        if (e.key === "ArrowUp") onResize(clamp(height + STEP))
        else if (e.key === "ArrowDown") onResize(clamp(height - STEP))
        else return
        e.preventDefault()
      }}
    />
  )
}
