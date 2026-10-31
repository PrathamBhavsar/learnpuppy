import { HttpResponse, http } from "msw"

import type {
  AnswerBody,
  AnswerResult,
  GradeBody,
  IntervalPreview,
  Rating,
} from "@/api/pending-types"

import { bus } from "../../mock/bus"
import { reviewStats, state } from "../db"
import { DAY } from "../fixtures/rng"
import { apiError, latency, notFound, once } from "../respond"

const API = "*/api/v1/workspaces/:ws/review"

const cardsOf = (ws: unknown) => state.db.cards.get(String(ws))

function previewFor(card: { state: string }): IntervalPreview {
  const fresh = card.state === "new" || card.state === "learning"
  return fresh
    ? { again: 0.0007, hard: 0.007, good: 1, easy: 4 }
    : { again: 0.007, hard: 3, good: 8, easy: 19 }
}

const INTERVAL_KEY: Record<Rating, keyof IntervalPreview> = {
  1: "again",
  2: "hard",
  3: "good",
  4: "easy",
}

const HIDDEN = ["correctKey", "explanation"] as const
const HIDDEN_FULL = [
  ...HIDDEN,
  "stability",
  "difficulty",
  "reps",
  "lapses",
] as const

/** Drops server-only fields (the answer, FSRS internals) from a stored card before it is served. */
function strip<T extends object, K extends keyof T>(
  value: T,
  keys: readonly K[]
): Omit<T, K> {
  const copy = { ...value }
  for (const key of keys) delete copy[key]
  return copy
}

export const reviewHandlers = [
  http.get(`${API}/queue`, async ({ params, request }) => {
    await latency()
    const cards = cardsOf(params["ws"])
    if (!cards) return notFound("Workspace")
    const limit = Number(new URL(request.url).searchParams.get("limit") ?? 50)
    const due = cards
      .filter((c) => c.due <= state.db.now)
      .sort((a, b) => a.due - b.due)
      .slice(0, limit)
      // The answer is withheld until /answer.
      .map((card) => strip(card, HIDDEN_FULL))
    return HttpResponse.json({ items: due, nextCursor: null })
  }),

  http.get(`${API}/stats`, async ({ params }) => {
    await latency()
    return cardsOf(params["ws"])
      ? HttpResponse.json(reviewStats(String(params["ws"])))
      : notFound("Workspace")
  }),

  http.get(`${API}/preview/:id`, ({ params }) => {
    const card = cardsOf(params["ws"])?.find((c) => c.id === params["id"])
    return card ? HttpResponse.json(previewFor(card)) : notFound("Card")
  }),

  http.get(`${API}/cards`, async ({ params, request }) => {
    await latency()
    const cards = cardsOf(params["ws"])
    if (!cards) return notFound("Workspace")
    const url = new URL(request.url)
    const stateFilter = url.searchParams.get("state")
    const q = (url.searchParams.get("q") ?? "").toLowerCase()
    return HttpResponse.json(
      cards
        .filter(
          (c) =>
            (!stateFilter || c.state === stateFilter) &&
            (!q || c.front.toLowerCase().includes(q))
        )
        .map((card) => strip(card, HIDDEN))
    )
  }),

  http.post(`${API}/:id/answer`, async ({ params, request }) => {
    await latency()
    const card = cardsOf(params["ws"])?.find((c) => c.id === params["id"])
    if (!card) return notFound("Card")
    const body = (await request.json()) as AnswerBody
    if (!card.options?.some((o) => o.key === body.choiceKey))
      return apiError("validation", "Unknown choice", {
        details: {
          fields: [{ path: "choiceKey", message: "Pick one of the options" }],
        },
      })
    const result = once<AnswerResult>(body.clientId, () => {
      const correct = body.choiceKey === card.correctKey
      const preview = previewFor(card)
      const rating: Rating = correct ? 3 : 1
      return {
        correct,
        correctKey: card.correctKey,
        explanation: card.explanation,
        rating,
        next: {
          due: state.db.now + preview[INTERVAL_KEY[rating]] * DAY,
          intervalDays: preview[INTERVAL_KEY[rating]],
        },
        preview,
      }
    })
    return HttpResponse.json(result)
  }),

  http.post(`${API}/:id/grade`, async ({ params, request }) => {
    await latency()
    const wsId = String(params["ws"])
    const card = cardsOf(wsId)?.find((c) => c.id === params["id"])
    if (!card) return notFound("Card")
    const body = (await request.json()) as GradeBody
    if (![1, 2, 3, 4].includes(body.rating))
      return apiError("validation", "Rating must be 1 to 4", {
        details: {
          fields: [{ path: "rating", message: "Use 1 (again) to 4 (easy)" }],
        },
      })
    const graded = once(body.clientId, () => {
      const days = previewFor(card)[INTERVAL_KEY[body.rating]]
      card.due = state.db.now + days * DAY
      card.state = "review"
      card.reps += 1
      return card
    })
    bus.emit("review.changed", {}, wsId)
    return HttpResponse.json(strip(graded, HIDDEN_FULL))
  }),

  http.post(`${API}/sync`, ({ params }) =>
    cardsOf(params["ws"]) ? HttpResponse.json({}) : notFound("Workspace")
  ),
]
