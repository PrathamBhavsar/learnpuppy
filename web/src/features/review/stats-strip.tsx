import { motion } from "motion/react"

import { CountUp } from "@/components/app/count-up"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { cn } from "@/lib/utils"

import type { components } from "@/api/schema"
type ReviewStats = components["schemas"]["StatsDto"]

type StatsStripProps = {
  stats: ReviewStats
  className?: string
}

type StatDef = {
  key: string
  label: string
  value: number | null
  decimals?: number
  suffix?: string
  testId: string
}

function statDefs(stats: ReviewStats): StatDef[] {
  return [
    {
      key: "due",
      label: "Due now",
      value: stats.dueNow,
      testId: "review-stat-due",
    },
    {
      key: "new",
      label: "New today",
      value: stats.newToday,
      testId: "review-stat-new",
    },
    {
      key: "reviewed",
      label: "Reviewed today",
      value: stats.reviewedToday,
      testId: "review-stat-reviewed",
    },
    {
      key: "streak",
      label: "Streak",
      value: stats.streak,
      suffix: "d",
      testId: "review-stat-streak",
    },
    {
      key: "retention",
      label: "Retention 30d",
      value:
        stats.retention30d == null
          ? null
          : Math.round(stats.retention30d * 100),
      suffix: stats.retention30d == null ? undefined : "%",
      testId: "review-stat-retention",
    },
  ]
}

export function StatsStrip({ stats, className }: StatsStripProps) {
  const reduced = useReducedMotion()
  const defs = statDefs(stats)

  return (
    <dl
      data-testid="review-stats-strip"
      className={cn(
        "grid grid-cols-2 gap-px overflow-hidden border bg-border sm:grid-cols-3 lg:grid-cols-5",
        className
      )}
    >
      {defs.map((stat, index) => (
        <motion.div
          key={stat.key}
          data-testid={stat.testId}
          initial={reduced ? false : { opacity: 0, y: 6 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.22, delay: Math.min(index, 7) * 0.024 }}
          className="flex flex-col gap-0.5 bg-card p-3"
        >
          <dt className="text-[11px] text-muted-foreground">{stat.label}</dt>
          <dd
            dir="ltr"
            className="flex items-baseline gap-0.5 font-heading text-[20px] font-medium tabular-nums rtl:justify-end"
          >
            {stat.value === null ? "—" : <CountUp value={stat.value} />}
            {stat.suffix ? (
              <span className="text-[13px] text-muted-foreground">
                {stat.suffix}
              </span>
            ) : null}
          </dd>
        </motion.div>
      ))}
    </dl>
  )
}
