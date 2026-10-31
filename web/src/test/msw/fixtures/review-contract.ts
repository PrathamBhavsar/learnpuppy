import type { components } from "@/features/review/schema"
import type { CardFixture } from "./review"

type Dto = components["schemas"]

export function cardPage(
  cards: CardFixture[],
  nextCursor: string | null = null
): Dto["CardPageDto"] {
  return {
    total: cards.length,
    nextCursor,
    items: cards.map((card) => ({
      id: card.id,
      front: card.front,
      state: card.state,
      due: card.due,
      reps: card.reps,
      lapses: card.lapses,
      isDue: true,
      mastery: 0.6,
      sourceDecisionId: card.sourceDecisionId ?? null,
      lastReview: null,
    })),
  }
}

export function cardAnswer(card: CardFixture, now: number): Dto["AnswerDto"] {
  const preview = [1, 2, 3, 4].map((rating) => ({
    rating,
    label: ["Again", "Hard", "Good", "Easy"][rating - 1]!,
    intervalLabel: `${rating * 2}d`,
    intervalMs: rating * 2 * 86_400_000,
    due: now + rating * 2 * 86_400_000,
  }))
  return {
    correct: true,
    correctKey: card.correctKey,
    explanation: card.explanation,
    rating: 3,
    next: { due: preview[2]!.due, intervalDays: 6 },
    preview,
  }
}
