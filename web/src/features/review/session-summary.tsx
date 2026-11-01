import { RotateCcwIcon } from "lucide-react"
import { motion } from "motion/react"
import { useEffect, useMemo, useState } from "react"

import type { Rating } from "@/api/pending-types"
import { InfoTip } from "@/components/app/info/info"
import { CountUp } from "@/components/app/count-up"
import { MasteryRing } from "@/components/app/mastery-ring"
import { Button } from "@/components/ui/button"
import { useReducedMotion } from "@/hooks/use-reduced-motion"

import { REVIEW_COPY } from "./copy"
import { reviewInfo } from "./info"
import { formatRelativeDue } from "./time"
import { type CardOutcome, type SessionState, gradedCount } from "./use-session"

const RATINGS: Rating[] = [1, 2, 3, 4]

export type TopicMastery = {
  id: string
  label: string
  value: number
  cards: number
}

/** Groups outcomes by source decision; mastery = share of cards rated Good or Easy. */
export function masteryByTopic(outcomes: CardOutcome[]): TopicMastery[] {
  const groups = new Map<string, CardOutcome[]>()
  for (const outcome of outcomes) {
    const key = outcome.sourceDecisionId ?? ""
    groups.set(key, [...(groups.get(key) ?? []), outcome])
  }
  return [...groups.entries()].map(([id, items]) => ({
    id,
    label: id ? `Decision ${id.replace(/^dec_/, "")}` : "Other cards",
    value: items.filter((o) => o.rating >= 3).length / items.length,
    cards: items.length,
  }))
}

export function formatElapsed(ms: number): string {
  const seconds = Math.max(0, Math.round(ms / 1000))
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`
}

/** A ring that fills from empty after a stagger delay (animation #38). */
function FillingRing({
  value,
  delayMs,
  label,
}: {
  value: number
  delayMs: number
  label: string
}) {
  const reduced = useReducedMotion()
  const [shown, setShown] = useState(reduced ? value : 0)
  useEffect(() => {
    if (reduced) return
    const timer = window.setTimeout(() => setShown(value), delayMs)
    return () => window.clearTimeout(timer)
  }, [value, delayMs, reduced])
  return <MasteryRing value={reduced ? value : shown} size={36} label={label} />
}

type Props = {
  state: SessionState
  onAgain?: () => void
  onExit: () => void
  exitLabel?: string
}

export function SessionSummary({ state, onAgain, onExit, exitLabel }: Props) {
  const reduced = useReducedMotion()
  const total = gradedCount(state)
  const correct = state.outcomes.filter((o) => o.correct).length
  const accuracy = total ? Math.round((correct / total) * 100) : 0
  const topics = useMemo(() => masteryByTopic(state.outcomes), [state.outcomes])
  const nextDue = state.outcomes.length
    ? Math.min(...state.outcomes.map((o) => o.nextDue))
    : null

  return (
    <motion.div
      data-testid="review-summary"
      initial={reduced ? false : { opacity: 0, scale: 0.96 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.6, ease: [0.22, 1, 0.36, 1] }}
      className="mx-auto flex w-full max-w-2xl flex-col gap-6"
    >
      <header className="flex flex-col gap-1">
        <h2 className="font-heading text-[24px] font-medium">
          {REVIEW_COPY.sessionComplete}
          <InfoTip info={reviewInfo.summary} className="ms-2 align-middle" />
        </h2>
        <p className="text-[13px] text-muted-foreground tabular-nums">
          {total} card{total === 1 ? "" : "s"} reviewed
        </p>
      </header>

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {RATINGS.map((rating, i) => (
          <motion.div
            key={rating}
            data-testid={`review-summary-rating-${rating}`}
            initial={reduced ? false : { opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.22, delay: 0.1 + Math.min(i, 7) * 0.024 }}
            className="flex flex-col gap-1 border bg-card p-3"
          >
            <span className="text-[11px] text-muted-foreground">
              {REVIEW_COPY.ratingLabels[rating - 1]}
            </span>
            <span className="font-heading text-[20px] font-medium tabular-nums">
              <CountUp value={state.rated[rating]} />
            </span>
          </motion.div>
        ))}
      </div>

      <dl className="grid grid-cols-3 gap-3">
        <div className="flex flex-col gap-1 border bg-card p-3">
          <dt className="text-[11px] text-muted-foreground">
            {REVIEW_COPY.summaryAccuracy}
          </dt>
          <dd
            data-testid="review-summary-accuracy"
            className="font-heading text-[20px] font-medium tabular-nums"
          >
            {accuracy}%
          </dd>
        </div>
        <div className="flex flex-col gap-1 border bg-card p-3">
          <dt className="text-[11px] text-muted-foreground">
            {REVIEW_COPY.summaryTime}
          </dt>
          <dd
            data-testid="review-summary-time"
            className="font-heading text-[20px] font-medium tabular-nums"
          >
            {formatElapsed(state.elapsedMs)}
          </dd>
        </div>
        <div className="flex flex-col gap-1 border bg-card p-3">
          <dt className="text-[11px] text-muted-foreground">
            {REVIEW_COPY.summaryNext}
          </dt>
          <dd
            data-testid="review-summary-next"
            className="font-heading text-[20px] font-medium"
          >
            {nextDue === null ? "—" : formatRelativeDue(nextDue)}
          </dd>
        </div>
      </dl>

      {topics.length > 0 ? (
        <section
          className="flex flex-col gap-2"
          aria-label={REVIEW_COPY.summaryMastery}
        >
          <h3 className="text-[12px] text-muted-foreground">
            {REVIEW_COPY.summaryMastery}
          </h3>
          <ul
            data-testid="review-summary-mastery"
            className="flex flex-wrap gap-3"
          >
            {topics.map((topic, i) => (
              <li
                key={topic.id}
                className="flex items-center gap-2.5 border bg-card px-3 py-2"
              >
                <FillingRing
                  value={topic.value}
                  delayMs={300 + Math.min(i, 7) * 60}
                  label={`${topic.label}: ${Math.round(topic.value * 100)}% good or easy`}
                />
                <span className="flex flex-col">
                  <span className="text-[12px]">{topic.label}</span>
                  <span className="text-[11px] text-muted-foreground tabular-nums">
                    {topic.cards} card{topic.cards === 1 ? "" : "s"}
                  </span>
                </span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <div className="flex items-center gap-2">
        {onAgain ? (
          <Button
            variant="outline"
            onClick={onAgain}
            data-testid="review-again"
          >
            <RotateCcwIcon
              data-icon="inline-start"
              className="rtl:-scale-x-100"
            />
            {REVIEW_COPY.again}
          </Button>
        ) : null}
        <Button
          variant="outline"
          onClick={onExit}
          data-testid="review-summary-exit"
        >
          {exitLabel ?? REVIEW_COPY.backToReview}
        </Button>
      </div>
    </motion.div>
  )
}
