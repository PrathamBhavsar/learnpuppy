import { AnimatePresence, motion } from "motion/react"
import { ChevronDownIcon } from "lucide-react"
import { useState } from "react"

import type { DigestDay, DigestSession } from "@/api/pending-types"
import { Badge } from "@/components/ui/badge"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { staggerDelay } from "@/lib/motion"
import { cn } from "@/lib/utils"

import { DIGEST_COPY } from "./copy"
import { formatDayLabel } from "./range"
import { SOURCE_LABELS, SourceIcon } from "./source-icon"

const timeFormat = new Intl.DateTimeFormat(undefined, {
  hour: "numeric",
  minute: "2-digit",
})

type DayGroupProps = {
  day: DigestDay
  now: number
  collapsed: boolean
  onToggle: () => void
  onOpenDecision: (id: string) => void
}

export function DayGroup({
  day,
  now,
  collapsed,
  onToggle,
  onOpenDecision,
}: DayGroupProps) {
  const reduced = useReducedMotion()
  const decisionCount = day.sessions.reduce((n, s) => n + s.decisions.length, 0)
  const label = formatDayLabel(day.day, now)

  return (
    <motion.section
      layout={!reduced}
      data-testid={`digest-day-${day.day}`}
      aria-label={label}
      className="flex flex-col"
    >
      <h2 className="sticky top-0 z-10 border-b bg-background/95 backdrop-blur">
        <button
          type="button"
          onClick={onToggle}
          aria-expanded={!collapsed}
          data-testid={`digest-day-toggle-${day.day}`}
          className="flex w-full items-center justify-between gap-3 px-1 py-2.5 text-start outline-none focus-visible:ring-1 focus-visible:ring-ring"
        >
          <span className="flex items-center gap-2 font-heading text-[14px] font-medium">
            <ChevronDownIcon
              aria-hidden
              className={cn(
                "size-4 text-muted-foreground transition-transform",
                collapsed && "-rotate-90 rtl:rotate-90"
              )}
            />
            {label}
          </span>
          <span
            dir="auto"
            data-testid={`digest-day-count-${day.day}`}
            className="text-[12px] font-normal text-muted-foreground tabular-nums"
          >
            {DIGEST_COPY.sessions(day.sessions.length)} ·{" "}
            {DIGEST_COPY.decisions(decisionCount)}
          </span>
        </button>
      </h2>
      <AnimatePresence initial={false}>
        {collapsed ? null : (
          <motion.ul
            key="sessions"
            initial={reduced ? false : { opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0, transition: { duration: 0.1 } }}
            transition={{ duration: 0.14 }}
            className="flex flex-col gap-2 py-3"
          >
            {day.sessions.map((session) => (
              <li key={session.sessionId}>
                <SessionCard
                  session={session}
                  onOpenDecision={onOpenDecision}
                />
              </li>
            ))}
          </motion.ul>
        )}
      </AnimatePresence>
    </motion.section>
  )
}

function SessionCard({
  session,
  onOpenDecision,
}: {
  session: DigestSession
  onOpenDecision: (id: string) => void
}) {
  const reduced = useReducedMotion()
  const [open, setOpen] = useState(false)
  const cardCount = session.decisions.reduce((n, d) => n + d.cardIds.length, 0)

  return (
    <motion.article
      layout={!reduced}
      data-testid={`digest-session-${session.sessionId}`}
      className="border bg-card transition-[border-color,transform] duration-[var(--dur-fast)] hover:-translate-y-px hover:border-ring/60"
    >
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        data-testid={`digest-session-toggle-${session.sessionId}`}
        className="flex w-full items-start gap-3 p-3 text-start outline-none focus-visible:ring-1 focus-visible:ring-ring"
      >
        <SourceIcon source={session.source} className="mt-0.5" />
        <span className="flex min-w-0 flex-1 flex-col gap-1">
          <span className="flex flex-wrap items-center gap-x-2 text-[12px] text-muted-foreground">
            <span className="font-medium text-foreground">
              {session.project}
            </span>
            <span>{SOURCE_LABELS[session.source] ?? session.source}</span>
            {session.startedAt ? (
              <span className="tabular-nums">
                {timeFormat.format(session.startedAt)}
              </span>
            ) : null}
          </span>
          <span dir="auto" className="text-[13px]">
            {session.summary}
          </span>
        </span>
        <span className="flex shrink-0 items-center gap-1.5">
          <Badge dir="auto" variant="secondary" className="tabular-nums">
            {DIGEST_COPY.decisions(session.decisions.length)}
          </Badge>
          <Badge dir="auto" variant="outline" className="tabular-nums">
            {DIGEST_COPY.cards(cardCount)}
          </Badge>
          <ChevronDownIcon
            aria-label={open ? DIGEST_COPY.collapse : DIGEST_COPY.expand}
            className={cn(
              "size-4 text-muted-foreground transition-transform",
              open && "rotate-180"
            )}
          />
        </span>
      </button>
      <AnimatePresence initial={false}>
        {open ? (
          <motion.ul
            key="decisions"
            initial={reduced ? false : { opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0, transition: { duration: 0.1 } }}
            transition={{ duration: 0.14 }}
            className="flex flex-col divide-y border-t"
          >
            {session.decisions.map((decision, i) => (
              <motion.li
                key={decision.id}
                initial={reduced ? false : { opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{
                  duration: 0.14,
                  delay: staggerDelay(i, session.decisions.length),
                }}
              >
                <button
                  type="button"
                  onClick={() => onOpenDecision(decision.id)}
                  data-testid={`digest-decision-${decision.id}`}
                  className="flex w-full items-center justify-between gap-3 px-3 py-2 text-start text-[13px] outline-none hover:bg-accent/50 focus-visible:bg-accent/50 focus-visible:ring-1 focus-visible:ring-ring"
                >
                  <span dir="auto" className="min-w-0 truncate">
                    {decision.title}
                  </span>
                  <span className="shrink-0 text-[11px] text-muted-foreground">
                    {DIGEST_COPY.basis[decision.basis].label}
                  </span>
                </button>
              </motion.li>
            ))}
          </motion.ul>
        ) : null}
      </AnimatePresence>
    </motion.article>
  )
}
