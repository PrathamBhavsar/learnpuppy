import { CheckIcon, MaximizeIcon, MinusIcon, PlusIcon } from "lucide-react"
import { useEffect, useMemo, useRef, useState } from "react"

import type { ClassroomView } from "@/api/pending-types"
import { Button } from "@/components/ui/button"
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { cn } from "@/lib/utils"

import { NODE_H, NODE_W, layoutPlan } from "./plan-layout"
import { fitTransform, usePanZoom } from "./use-pan-zoom"
import "./styles.css"

type Plan = NonNullable<ClassroomView["plan"]>

const STATUS_LABEL = {
  current: "Current",
  visited: "Done",
  upcoming: "Upcoming",
} as const

const WIDE = 560

/** Observed content-box size of an element (0x0 until measured, e.g. under jsdom). */
function useSize<T extends Element>() {
  const ref = useRef<T>(null)
  const [size, setSize] = useState({ width: 0, height: 0 })
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const measure = () =>
      setSize({ width: el.clientWidth, height: el.clientHeight })
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(el)
    return () => observer.disconnect()
  }, [])
  return [ref, size] as const
}

/** The lesson plan as a layered DAG (SVG, dagre). Spatial data, so it stays left-to-right under RTL. */
export default function PlanDag({ plan }: { plan: Plan }) {
  const [box, size] = useSize<HTMLDivElement>()
  const rankdir = size.width >= WIDE ? "LR" : "TB"
  const layout = useMemo(() => layoutPlan(plan, rankdir), [plan, rankdir])
  const fit = useMemo(() => fitTransform(layout, size), [layout, size])
  const { ref, transform, handlers, zoomBy, reset, moved } = usePanZoom(fit)
  const [selected, setSelected] = useState<string | null>(null)
  const current = plan.currentId ?? null

  return (
    <div
      ref={box}
      dir="ltr"
      data-testid="plan-dag"
      className="relative size-full min-h-48 overflow-hidden"
    >
      <svg
        ref={ref}
        role="group"
        aria-label={`Lesson plan, ${plan.nodes.length} steps`}
        className="size-full cursor-grab touch-none active:cursor-grabbing"
        {...handlers}
      >
        <g
          transform={`translate(${transform.x} ${transform.y}) scale(${transform.k})`}
        >
          {layout.edges.map((edge) => {
            const target = layout.nodes.find((n) => n.id === edge.to)
            const intoCurrent = target?.id === current
            return (
              <path
                key={`${edge.from}-${edge.to}`}
                d={edge.d}
                pathLength={1}
                fill="none"
                data-testid={`plan-edge-${edge.from}-${edge.to}`}
                style={
                  {
                    "--lp-delay": `${edge.order * 40 + 40}ms`,
                  } as React.CSSProperties
                }
                className={cn(
                  "lp-draw stroke-border",
                  intoCurrent && "stroke-primary"
                )}
                strokeWidth={1.5}
              />
            )
          })}
          {layout.nodes.map((node) => (
            <Popover
              key={node.id}
              open={selected === node.id}
              onOpenChange={(open) => setSelected(open ? node.id : null)}
            >
              <PopoverAnchor asChild>
                <g
                  role="button"
                  tabIndex={0}
                  aria-label={`${node.title}, ${STATUS_LABEL[node.status]}`}
                  data-testid={`plan-node-${node.id}`}
                  data-status={node.status}
                  transform={`translate(${node.x} ${node.y})`}
                  onClick={() =>
                    setSelected((s) => (s === node.id ? null : node.id))
                  }
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault()
                      setSelected((s) => (s === node.id ? null : node.id))
                    }
                  }}
                  className="cursor-pointer outline-none [&:focus-visible>rect:first-of-type]:stroke-ring"
                >
                  <g
                    className="lp-node-in"
                    style={
                      {
                        "--lp-delay": `${node.order * 40}ms`,
                      } as React.CSSProperties
                    }
                  >
                    {node.status === "current" ? (
                      <rect
                        className="lp-pulse-ring fill-none stroke-primary"
                        x={-4}
                        y={-4}
                        width={NODE_W + 8}
                        height={NODE_H + 8}
                        strokeWidth={2}
                      />
                    ) : null}
                    <rect
                      width={NODE_W}
                      height={NODE_H}
                      strokeWidth={1.5}
                      className={cn(
                        "transition-colors duration-[360ms]",
                        node.status === "current" &&
                          "fill-primary/15 stroke-primary",
                        node.status === "visited" &&
                          "fill-success/10 stroke-success/60",
                        node.status === "upcoming" && "fill-card stroke-border"
                      )}
                    />
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <foreignObject width={NODE_W} height={NODE_H}>
                          <div className="flex size-full items-center gap-1.5 px-2.5 text-[12px] leading-tight">
                            {node.status === "visited" ? (
                              <svg
                                aria-hidden
                                viewBox="0 0 16 16"
                                className="size-3.5 shrink-0 text-success"
                                fill="none"
                                stroke="currentColor"
                                strokeWidth="2"
                              >
                                <path
                                  pathLength={1}
                                  className="lp-draw"
                                  d="M3 8.5l3.2 3L13 4.5"
                                />
                              </svg>
                            ) : null}
                            <span
                              className="line-clamp-2 min-w-0 text-start"
                              dir="auto"
                            >
                              {node.title}
                            </span>
                          </div>
                        </foreignObject>
                      </TooltipTrigger>
                      <TooltipContent>{node.title}</TooltipContent>
                    </Tooltip>
                  </g>
                </g>
              </PopoverAnchor>
              <PopoverContent
                side="bottom"
                className="w-64 gap-1.5"
                data-testid={`plan-popover-${node.id}`}
              >
                <p className="flex items-center gap-1.5 text-[13px] font-medium">
                  {node.status === "visited" ? (
                    <CheckIcon aria-hidden className="size-3.5 text-success" />
                  ) : null}
                  {node.title}
                </p>
                <p className="text-[11px] text-muted-foreground">
                  {STATUS_LABEL[node.status]}
                </p>
                <p dir="auto" className="text-[13px] leading-[1.5]">
                  {node.summary || "No summary yet."}
                </p>
              </PopoverContent>
            </Popover>
          ))}
        </g>
      </svg>
      <div className="absolute end-2 bottom-2 flex flex-col gap-1">
        <Button
          variant="outline"
          size="icon-xs"
          aria-label="Zoom in"
          data-testid="plan-zoom-in"
          onClick={() => zoomBy(1.25, size)}
        >
          <PlusIcon />
        </Button>
        <Button
          variant="outline"
          size="icon-xs"
          aria-label="Zoom out"
          data-testid="plan-zoom-out"
          onClick={() => zoomBy(0.8, size)}
        >
          <MinusIcon />
        </Button>
        <Button
          variant="outline"
          size="icon-xs"
          aria-label="Fit plan"
          data-testid="plan-fit"
          disabled={!moved}
          onClick={reset}
        >
          <MaximizeIcon />
        </Button>
      </div>
    </div>
  )
}
