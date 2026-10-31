// The hovered node's text as a DOM tag that follows the node (docs/web/00 D10: DOM overlay for labels).
// Position is written straight to the element on every camera event, never through React state.
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react"

import { cn } from "@/lib/utils"

import type { GraphEngine } from "../canvas/engine"
import { copy } from "../copy"
import { NODE_KINDS, SOURCES, type Scene } from "../model/scene"

import { SOURCE_BG } from "./source-style"

type Props = { engine: GraphEngine | null; scene: Scene | null }

export function NodeTag({ engine, scene }: Props) {
  const [idx, setIdx] = useState(-1)
  const ref = useRef<HTMLDivElement>(null)

  const place = useCallback(() => {
    const el = ref.current
    if (!el || !engine || idx < 0) return
    const pos = engine.nodeScreen(idx)
    if (!pos) return
    const [sx, sy] = pos
    const half = engine.nodeSide(idx) / 2
    const w = el.offsetWidth
    const h = el.offsetHeight
    const room = engine.size.w - (sx + half + 10)
    const x = room >= w + 4 ? sx + half + 10 : Math.max(4, sx - half - 10 - w)
    const y = Math.min(Math.max(sy - h / 2, 4), engine.size.h - h - 4)
    el.style.transform = `translate(${Math.round(x)}px, ${Math.round(y)}px)`
  }, [engine, idx])

  useEffect(() => {
    if (!engine) return
    const offHover = engine.on("hover", setIdx)
    const offCamera = engine.on("camera", place)
    return () => {
      offHover()
      offCamera()
    }
  }, [engine, place])

  useLayoutEffect(place, [place, scene])

  if (!scene || idx < 0 || idx >= scene.count) return null
  const source = SOURCES[scene.source[idx] as number] ?? "chat"
  const kind = NODE_KINDS[scene.kind[idx] as number] ?? "claim"
  return (
    <div
      ref={ref}
      data-testid="graph-node-tag"
      // Positioned from the canvas' top-left corner regardless of the page direction.
      dir="ltr"
      style={{ insetInlineStart: 0, top: 0 }}
      className="pointer-events-none absolute z-10 max-w-[300px] animate-in border border-border bg-popover px-2.5 py-2 text-popover-foreground shadow-sm fade-in-0 duration-150"
    >
      <p dir="auto" className="line-clamp-4 text-[12px] leading-[1.45]">
        {scene.texts[idx]}
      </p>
      <p className="mt-1.5 flex items-center gap-1.5 text-[11px] text-muted-foreground">
        <span aria-hidden className={cn("size-2", SOURCE_BG[source])} />
        {copy.legend.sourceNames[source]} · {kind} ·{" "}
        <span className="tabular-nums">{scene.degree[idx]}</span>
      </p>
    </div>
  )
}
