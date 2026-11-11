import { HttpResponse, http } from "msw"

import type { components } from "@/api/schema"

import { bus } from "../../mock/bus"
import { reviewStats, state } from "../db"
import {
  cardBrief,
  cardPage,
  previewList,
  queueCard,
} from "../fixtures/review-contract"
import { DAY } from "../fixtures/rng"
import { apiError, latency, notFound, once } from "../respond"

type Dto = components["schemas"]

const API = "*/api/v1/workspaces/:ws/review"

const cardsOf = (ws: unknown) => state.db.cards.get(String(ws))

export const reviewHandlers = [
  http.get(`${API}/queue`, async ({ params, request }) => {
    await latency()
    const cards = cardsOf(params["ws"])
    if (!cards) return notFound("Workspace")
    const limit = Number(new URL(request.url).searchParams.get("limit") ?? 50)
    const due = cards
      .filter((c) => c.due <= state.db.now)
      .sort((a, b) => a.due - b.due)
    // The answer is withheld until /answer.
    const body: Dto["QueueDto"] = {
      items: due.slice(0, limit).map(queueCard),
      dueNow: due.length,
      now: state.db.now,
    }
    return HttpResponse.json(body)
  }),

  http.get(`${API}/stats`, async ({ params }) => {
    await latency()
    return cardsOf(params["ws"])
      ? HttpResponse.json(reviewStats(String(params["ws"])))
      : notFound("Workspace")
  }),

  http.get(`${API}/preview/:card`, ({ params }) => {
    const card = cardsOf(params["ws"])?.find((c) => c.id === params["card"])
    if (!card) return notFound("Card")
    const body: Dto["PreviewListDto"] = {
      items: previewList(card, state.db.now),
    }
    return HttpResponse.json(body)
  }),

  http.get(`${API}/cards`, async ({ params, request }) => {
    await latency()
    const cards = cardsOf(params["ws"])
    if (!cards) return notFound("Workspace")
    const url = new URL(request.url)
    const stateFilter = url.searchParams.get("state")
    const q = (url.searchParams.get("q") ?? "").toLowerCase()
    const limit = Number(url.searchParams.get("limit") ?? 50)
    const offset = Number(url.searchParams.get("cursor") ?? 0)
    const matching = cards.filter(
      (c) =>
        (!stateFilter || c.state === stateFilter) &&
        (!q || c.front.toLowerCase().includes(q))
    )
    const end = offset + limit
    const page = cardPage(
      matching.slice(offset, end),
      end < matching.length ? String(end) : null,
      state.db.now
    )
    return HttpResponse.json({ ...page, total: matching.length })
  }),

  http.get(`${API}/cards/:card`, async ({ params }) => {
    await latency()
    const card = cardsOf(params["ws"])?.find((c) => c.id === params["card"])
    if (!card) return notFound("Card")
    const history = Array.from({ length: Math.min(card.reps, 6) }, (_, i) => ({
      ts: state.db.now - (card.reps - i) * DAY,
      rating: ((i + card.lapses) % 4) + 1,
      deviceId: "dev_mock",
    }))
    const body: Dto["CardDetailDto"] = {
      ...cardBrief(card, state.db.now),
      options: card.options,
      answerMode: card.answerMode ?? "mcq",
      correctKey: card.correctKey,
      rationale: card.explanation,
      stability: card.stability,
      difficulty: card.difficulty,
      history,
      preview: previewList(card, state.db.now),
    }
    return HttpResponse.json(body)
  }),

  http.post(`${API}/:card/answer`, async ({ params, request }) => {
    await latency()
    const card = cardsOf(params["ws"])?.find((c) => c.id === params["card"])
    if (!card) return notFound("Card")
    const body = (await request.json()) as Dto["AnswerRequest"]
    const recall = card.answerMode === "recall"
    if (recall && !body.text)
      return apiError("validation", "Type an answer", {
        details: {
          fields: [{ path: "text", message: "A recall card takes text" }],
        },
      })
    if (!recall && !card.options.some((o) => o.key === body.choiceKey))
      return apiError("validation", "Unknown choice", {
        details: {
          fields: [{ path: "choiceKey", message: "Pick one of the options" }],
        },
      })
    const result = once<Dto["AnswerDto"]>(body.clientId, () => {
      const correct = recall
        ? (body.text ?? "").trim().toLowerCase() ===
          (card.answer ?? "").trim().toLowerCase()
        : body.choiceKey === card.correctKey
      const preview = previewList(card, state.db.now)
      const rating = correct ? 3 : 1
      const chosen = preview[rating - 1]!
      return {
        correct,
        correctKey: recall ? "" : card.correctKey,
        answer: recall ? (card.answer ?? "") : null,
        explanation: card.explanation,
        rating,
        next: { due: chosen.due, intervalDays: chosen.intervalMs / DAY },
        preview,
      }
    })
    return HttpResponse.json(result)
  }),

  http.post(`${API}/:card/grade`, async ({ params, request }) => {
    await latency()
    const wsId = String(params["ws"])
    const card = cardsOf(wsId)?.find((c) => c.id === params["card"])
    if (!card) return notFound("Card")
    const body = (await request.json()) as Dto["GradeRequest"]
    if (![1, 2, 3, 4].includes(body.rating))
      return apiError("validation", "Rating must be 1 to 4", {
        details: {
          fields: [{ path: "rating", message: "Use 1 (again) to 4 (easy)" }],
        },
      })
    const graded = once<Dto["GradeDto"]>(body.clientId, () => {
      const chosen = previewList(card, state.db.now)[body.rating - 1]!
      card.due = chosen.due
      card.state = "review"
      card.reps += 1
      return {
        card: cardBrief(card, state.db.now),
        preview: previewList(card, state.db.now),
      }
    })
    bus.emit("review.changed", {}, wsId)
    return HttpResponse.json(graded)
  }),

  http.post(`${API}/sync`, ({ params }) => {
    const body: Dto["SyncDto"] = { cards: 0, reviews: 0 }
    return cardsOf(params["ws"])
      ? HttpResponse.json(body)
      : notFound("Workspace")
  }),
]
