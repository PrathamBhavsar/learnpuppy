/* eslint-disable jsx-a11y/click-events-have-key-events, jsx-a11y/interactive-supports-focus -- options are driven by the listbox (aria-activedescendant); click is the pointer path */
// Accessible twin of the canvas: a virtualized listbox of the visible nodes, most connected first.
// It mirrors selection and search, is reachable by keyboard and screen reader, and becomes visible
// while it has focus so sighted keyboard users can see where they are (docs/web/06 §a11y).
import { useVirtualizer } from "@tanstack/react-virtual"
import { useEffect, useMemo, useRef, useState } from "react"

import { cn } from "@/lib/utils"

import { copy } from "../copy"
import type { Scene } from "../model/scene"

const ITEM_HEIGHT = 28

type Props = {
  scene: Scene
  visible: Uint8Array | null
  /** Search matches; when set the list shows only them. */
  match: Uint8Array | null
  selected: ReadonlySet<number>
  active: number
  onPick: (idx: number) => void
  onOpen: (idx: number) => void
  /** Text for the polite live region. */
  announcement: string
}

export function GraphList({
  scene,
  visible,
  match,
  selected,
  active,
  onPick,
  onOpen,
  announcement,
}: Props) {
  const items = useMemo(() => {
    const keep = (i: number) =>
      (!visible || visible[i] === 1) && (!match || match[i] === 1)
    return scene.order.filter(keep)
  }, [scene, visible, match])
  const scroller = useRef<HTMLDivElement>(null)
  const [cursor, setCursor] = useState(0)

  // eslint-disable-next-line react-hooks/incompatible-library -- the compiler is not enabled; the virtualizer is used as documented
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ITEM_HEIGHT,
    overscan: 6,
    initialRect: { width: 320, height: 192 },
  })

  // Follow the canvas: when the active node changes elsewhere, bring its row into view.
  useEffect(() => {
    if (active < 0) return
    const at = items.indexOf(active)
    if (at >= 0) {
      setCursor(at)
      virtualizer.scrollToIndex(at)
    }
  }, [active, items, virtualizer])

  const current = items[Math.min(cursor, items.length - 1)]
  const move = (to: number) => {
    if (items.length === 0) return
    const next = Math.min(items.length - 1, Math.max(0, to))
    setCursor(next)
    virtualizer.scrollToIndex(next)
    const idx = items[next]
    if (idx !== undefined) onPick(idx)
  }

  return (
    <div
      data-lp-graph-list
      className={cn(
        "absolute start-3 bottom-3 z-40 h-48 w-80 overflow-hidden border border-border bg-popover text-popover-foreground shadow-sm",
        // Visually hidden until focus lands inside, but always exposed to assistive tech.
        "[clip-path:inset(50%)] focus-within:[clip-path:none]"
      )}
    >
      <div
        ref={scroller}
        role="listbox"
        tabIndex={0}
        aria-label={copy.list.label}
        aria-activedescendant={
          current !== undefined ? `graph-list-${scene.ids[current]}` : undefined
        }
        data-testid="graph-list"
        className="size-full overflow-y-auto outline-none focus-visible:ring-1 focus-visible:ring-ring/50 focus-visible:ring-inset"
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") move(cursor + 1)
          else if (e.key === "ArrowUp") move(cursor - 1)
          else if (e.key === "Home") move(0)
          else if (e.key === "End") move(items.length - 1)
          else if (e.key === "Enter" || e.key === " ") {
            if (current !== undefined) onOpen(current)
          } else return
          e.preventDefault()
        }}
      >
        <div
          style={{ height: virtualizer.getTotalSize(), position: "relative" }}
        >
          {virtualizer.getVirtualItems().map((item) => {
            const idx = items[item.index]
            if (idx === undefined) return null
            const id = scene.ids[idx] as string
            return (
              <div
                key={item.key}
                id={`graph-list-${id}`}
                role="option"
                aria-selected={selected.has(idx)}
                aria-posinset={item.index + 1}
                aria-setsize={items.length}
                data-testid="graph-list-item"
                style={{
                  position: "absolute",
                  insetInlineStart: 0,
                  insetInlineEnd: 0,
                  top: 0,
                  height: item.size,
                  transform: `translateY(${item.start}px)`,
                }}
                className={cn(
                  "flex cursor-pointer items-center gap-2 px-2 text-[12px]",
                  selected.has(idx) && "bg-primary/10",
                  item.index === cursor &&
                    "outline outline-1 -outline-offset-1 outline-ring"
                )}
                onClick={() => {
                  setCursor(item.index)
                  onPick(idx)
                }}
              >
                <span dir="auto" className="min-w-0 flex-1 truncate">
                  {scene.texts[idx]}
                </span>
                <span className="text-muted-foreground tabular-nums">
                  {scene.degree[idx]}
                </span>
              </div>
            )
          })}
        </div>
      </div>
      <div
        role="status"
        aria-live="polite"
        aria-atomic
        className="sr-only"
        data-testid="graph-live"
      >
        {announcement}
      </div>
    </div>
  )
}
