import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import type { Client } from "openapi-fetch"
import { pendingApi, newClientId, unwrap } from "@/api/client"
import { keys } from "@/api/keys"
import type {
  AnswerResult as LegacyAnswer,
  CardListItem as LegacyCard,
  IntervalPreview,
  Rating,
} from "@/api/pending-types"
import type { components, paths } from "./schema"

// Reuse F10's transport/middleware with B20 types until the shared schema is regenerated.
const reviewApi = pendingApi as unknown as Client<paths>
const api = reviewApi

type Dto = components["schemas"]
export type AnswerResult = LegacyAnswer & { previewItems?: Dto["PreviewDto"][] }
export type BrowserCard = Pick<
  Dto["CardBriefDto"],
  "id" | "front" | "due" | "state" | "reps" | "lapses"
>

export function useReviewQueue(ws: string) {
  return useQuery({
    queryKey: keys.ws(ws).reviewQueue(200),
    queryFn: async ({ signal }) => {
      const queue = unwrap(
        await api.GET("/workspaces/{ws}/review/queue", {
          params: { path: { ws }, query: { limit: 200 } },
          signal,
        })
      )
      return {
        ...queue,
        items: queue.items.map((card) => ({
          ...card,
          sourceDecisionId: card.sourceDecisionId ?? undefined,
        })),
      }
    },
  })
}

export function useReviewStats(ws: string) {
  return useQuery({
    queryKey: keys.ws(ws).reviewStats(),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/workspaces/{ws}/review/stats", {
          params: { path: { ws } },
          signal,
        })
      ),
  })
}

// F10 mock responses predate B20's generated DTOs. Keep that translation at the boundary.
function normalizeAnswer(data: Dto["AnswerDto"] | LegacyAnswer): AnswerResult {
  if (!Array.isArray(data.preview)) return data as LegacyAnswer
  const names = ["again", "hard", "good", "easy"] as const
  const preview: IntervalPreview = { again: 0, hard: 0, good: 0, easy: 0 }
  for (const item of data.preview) {
    const name = names[item.rating - 1]
    if (name) preview[name] = item.intervalMs / 86_400_000
  }
  return {
    ...data,
    rating: data.rating as Rating,
    preview,
    previewItems: data.preview,
  }
}

export function useCards(
  ws: string,
  filter: { state?: Dto["CardState"]; q?: string } = {}
) {
  return useQuery({
    queryKey: keys.ws(ws).reviewCards(filter),
    queryFn: async ({ signal }): Promise<BrowserCard[]> => {
      const cards: BrowserCard[] = []
      let cursor: string | undefined
      do {
        const page: Dto["CardPageDto"] | LegacyCard[] = unwrap(
          await api.GET("/workspaces/{ws}/review/cards", {
            params: { path: { ws }, query: { ...filter, limit: 200, cursor } },
            signal,
          })
        )
        if (Array.isArray(page)) return page
        cards.push(...page.items)
        const next = page.nextCursor ?? undefined
        if (next && next === cursor)
          throw new Error("Card pagination did not advance")
        cursor = next
      } while (cursor)
      return cards
    },
  })
}

export function useCardDetail(ws: string, id: string | undefined) {
  return useQuery({
    queryKey: [...keys.ws(ws).review(), "card", id ?? ""],
    enabled: Boolean(id),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/workspaces/{ws}/review/cards/{card}", {
          params: { path: { ws, card: id ?? "" } },
          signal,
        })
      ),
  })
}

export function useAnswerCard(ws: string) {
  return useMutation({
    mutationFn: async ({
      cardId,
      choiceKey,
    }: {
      cardId: string
      choiceKey: string
    }) =>
      normalizeAnswer(
        unwrap(
          await api.POST("/workspaces/{ws}/review/{card}/answer", {
            params: { path: { ws, card: cardId } },
            body: { choiceKey, clientId: newClientId() },
          })
        )
      ),
  })
}

export function useGradeCard(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationKey: ["ws", ws, "review-grade"],
    mutationFn: async ({
      cardId,
      rating,
    }: {
      cardId: string
      rating: Rating
    }) =>
      unwrap(
        await api.POST("/workspaces/{ws}/review/{card}/grade", {
          params: { path: { ws, card: cardId } },
          body: { rating, clientId: newClientId() },
        })
      ),
    onSettled: () =>
      Promise.all([
        qc.invalidateQueries({ queryKey: keys.ws(ws).review() }),
        qc.invalidateQueries({ queryKey: keys.workspacesAll() }),
      ]),
  })
}
