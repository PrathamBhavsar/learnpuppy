import { AnimatePresence, motion } from "motion/react"
import { ChevronDownIcon } from "lucide-react"
import { useState } from "react"

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { InfoTip } from "@/components/app/info/info"
import { cn } from "@/lib/utils"

import { reviewInfo } from "./info"

type ForecastProps = {
  /** 14 values, cards due per day. */
  data: number[]
  /** Epoch ms for day 0 (today). */
  now: number
  className?: string
}

export function Forecast({ data, now, className }: ForecastProps) {
  const reduced = useReducedMotion()
  const [open, setOpen] = useState(true)
  const max = Math.max(1, ...data)

  return (
    <section data-testid="review-forecast" className={cn("border", className)}>
      <div className="flex items-center pe-3">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          data-testid="review-forecast-toggle"
          aria-expanded={open}
          className="flex flex-1 items-center justify-between gap-2 px-3 py-2 text-[13px] font-medium outline-none focus-visible:ring-1 focus-visible:ring-ring"
        >
          <span dir="auto">14-day forecast</span>
          <ChevronDownIcon
            aria-hidden
            className={cn(
              "size-4 transition-transform",
              open ? "" : "-rotate-90 rtl:rotate-90"
            )}
          />
        </button>
        <InfoTip info={reviewInfo.forecast} />
      </div>
      <AnimatePresence initial={false}>
        {open ? (
          <motion.div
            key="bars"
            initial={reduced ? false : { opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.14 }}
          >
            <div
              dir="ltr"
              className="flex items-end gap-1 px-3 pt-1 pb-3"
              role="img"
              aria-label="14-day card forecast"
            >
              {data.map((count, day) => {
                const dueDate = new Date(now + day * 86_400_000)
                const heightPct = (count / max) * 100
                return (
                  <Tooltip key={day}>
                    <TooltipTrigger asChild>
                      <div className="flex flex-1 flex-col items-center gap-1">
                        <div className="flex h-20 w-full items-end justify-center">
                          <motion.div
                            data-testid={`review-forecast-bar-${day}`}
                            data-count={count}
                            initial={reduced ? false : { scaleY: 0 }}
                            animate={{ scaleY: 1 }}
                            transition={{
                              duration: 0.36,
                              delay: Math.min(day, 7) * 0.02,
                            }}
                            style={{
                              transformOrigin: "bottom",
                              height: `${Math.max(heightPct, count > 0 ? 6 : 2)}%`,
                            }}
                            className={cn(
                              "w-full",
                              count > 0 ? "bg-primary/70" : "bg-muted"
                            )}
                          />
                        </div>
                        <span className="text-[10px] text-muted-foreground tabular-nums">
                          {dueDate.toLocaleDateString(undefined, {
                            weekday: "narrow",
                          })}
                        </span>
                      </div>
                    </TooltipTrigger>
                    <TooltipContent>
                      <p className="text-[12px] tabular-nums">
                        {dueDate.toLocaleDateString(undefined, {
                          month: "short",
                          day: "numeric",
                        })}
                        : {count} due
                      </p>
                    </TooltipContent>
                  </Tooltip>
                )
              })}
            </div>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </section>
  )
}
