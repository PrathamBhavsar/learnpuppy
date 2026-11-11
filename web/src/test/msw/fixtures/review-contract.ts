// Wire-shape builders for the review endpoints. Typed against the generated schema so drift is a compile error.
import type { components } from "@/api/schema"

import type { CardFixture } from "./review"
import { DAY } from "./rng"

type Dto = components["schemas"]

const LABELS = ["Again", "Hard", "Good", "Easy"] as const

function intervalLabel(ms: number): string {
  const days = ms / DAY
  if (days >= 1) return `${Math.round(days)}d`
  const hours = ms / 3_600_000
  if (hours >= 1) return `${Math.round(hours)}h`
  return `${Math.max(1, Math.round(ms / 60_000))}m`
}

/** What each rating would schedule, `[again, hard, good, easy]`. */
export function previewList(
  card: CardFixture,
  now: number
): Dto["PreviewDto"][] {
  const fresh = card.state === "new" || card.state === "learning"
  const days = fresh ? [0.0007, 0.007, 1, 4] : [0.007, 3, 8, 19]
  return days.map((d, i) => {
    const intervalMs = Math.round(d * DAY)
    return {
      rating: i + 1,
      label: LABELS[i]!,
      intervalMs,
      intervalLabel: intervalLabel(intervalMs),
      due: now + intervalMs,
    }
  })
}

export function cardBrief(card: CardFixture, now: number): Dto["CardBriefDto"] {
  return {
    id: card.id,
    front: card.front,
    state: card.state,
    due: card.due,
    reps: card.reps,
    lapses: card.lapses,
    isDue: card.due <= now,
    mastery: Math.min(1, card.stability / 30),
    sourceDecisionId: card.sourceDecisionId,
    lastReview: null,
  }
}

export function queueCard(card: CardFixture): Dto["QueueCardDto"] {
  return {
    id: card.id,
    front: card.front,
    options: card.answerMode === "recall" ? [] : card.options,
    answerMode: card.answerMode ?? "mcq",
    kind: card.kind,
    sourceDecisionId: card.sourceDecisionId,
    due: card.due,
    state: card.state,
  }
}

export function cardPage(
  cards: CardFixture[],
  nextCursor: string | null = null,
  now = Date.now()
): Dto["CardPageDto"] {
  return {
    total: cards.length,
    nextCursor,
    items: cards.map((card) => cardBrief(card, now)),
  }
}

export function cardAnswer(card: CardFixture, now: number): Dto["AnswerDto"] {
  const preview = previewList(card, now)
  return {
    correct: true,
    correctKey: card.correctKey,
    answer: null,
    explanation: card.explanation,
    rating: 3,
    next: { due: preview[2]!.due, intervalDays: preview[2]!.intervalMs / DAY },
    preview,
  }
}
