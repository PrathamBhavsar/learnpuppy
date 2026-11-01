import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { newClientId, pendingApi, unwrap } from "../client"
import { keys } from "../keys"
import type {
  AnswerBody,
  CardState,
  Rating,
  ReviewQueue,
  ReviewStats,
} from "../pending-types"

export function useReviewQueue(ws: string, limit?: number) {
  return useQuery({
    queryKey: keys.ws(ws).reviewQueue(limit),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/review/queue", {
          params: {
            path: { ws },
            query: limit === undefined ? {} : { limit },
          },
          signal,
        })
      ),
  })
}

export function useReviewStats(ws: string) {
  return useQuery({
    queryKey: keys.ws(ws).reviewStats(),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/review/stats", {
          params: { path: { ws } },
          signal,
        })
      ),
  })
}

export function useIntervalPreview(ws: string, cardId: string | undefined) {
  return useQuery({
    queryKey: keys.ws(ws).reviewPreview(cardId ?? ""),
    enabled: Boolean(cardId),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/review/preview/{card}", {
          params: { path: { ws, card: cardId ?? "" } },
          signal,
        })
      ),
  })
}

export function useCards(
  ws: string,
  filter: { state?: CardState; q?: string } = {}
) {
  return useQuery({
    queryKey: keys.ws(ws).reviewCards(filter),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/review/cards", {
          params: {
            path: { ws },
            query: {
              ...(filter.state ? { state: filter.state } : {}),
              ...(filter.q ? { q: filter.q } : {}),
            },
          },
          signal,
        })
      ),
  })
}

export function useCardDetail(ws: string, cardId: string | undefined) {
  return useQuery({
    queryKey: [...keys.ws(ws).review(), "card", cardId ?? ""] as const,
    enabled: Boolean(cardId),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/review/cards/{card}", {
          params: { path: { ws, card: cardId ?? "" } },
          signal,
        })
      ),
  })
}

/** Reveals correctness; the server withholds the answer until this call. */
export function useAnswerCard(ws: string) {
  return useMutation({
    mutationFn: async ({
      cardId,
      ...body
    }: Omit<AnswerBody, "clientId"> & { cardId: string; clientId?: string }) =>
      unwrap(
        await pendingApi.POST("/workspaces/{ws}/review/{card}/answer", {
          params: { path: { ws, card: cardId } },
          body: { ...body, clientId: body.clientId ?? newClientId() },
        })
      ),
  })
}

type GradeVars = { cardId: string; rating: Rating; clientId?: string }

/**
 * Optimistic grade (docs/web/04): the card leaves every cached queue at once so the session
 * advances immediately; on error the queues and stats are restored and the caller toasts.
 */
export function useGradeCard(ws: string) {
  const qc = useQueryClient()
  const queueKey = [...keys.ws(ws).review(), "queue"] as const
  return useMutation({
    mutationFn: async ({ cardId, rating, clientId }: GradeVars) =>
      unwrap(
        await pendingApi.POST("/workspaces/{ws}/review/{card}/grade", {
          params: { path: { ws, card: cardId } },
          body: { rating, clientId: clientId ?? newClientId() },
        })
      ),
    onMutate: async ({ cardId }) => {
      await qc.cancelQueries({ queryKey: queueKey })
      await qc.cancelQueries({ queryKey: keys.ws(ws).reviewStats() })
      const queues = qc.getQueriesData<ReviewQueue>({ queryKey: queueKey })
      const stats = qc.getQueryData<ReviewStats>(keys.ws(ws).reviewStats())
      for (const [key, queue] of queues) {
        if (queue)
          qc.setQueryData<ReviewQueue>(key, {
            ...queue,
            items: queue.items.filter((c) => c.id !== cardId),
          })
      }
      if (stats)
        qc.setQueryData<ReviewStats>(keys.ws(ws).reviewStats(), {
          ...stats,
          dueNow: Math.max(0, stats.dueNow - 1),
          reviewedToday: stats.reviewedToday + 1,
        })
      return { queues, stats }
    },
    onError: (_error, _vars, context) => {
      for (const [key, queue] of context?.queues ?? [])
        qc.setQueryData(key, queue)
      if (context?.stats)
        qc.setQueryData(keys.ws(ws).reviewStats(), context.stats)
    },
    onSettled: () =>
      Promise.all([
        qc.invalidateQueries({ queryKey: keys.ws(ws).reviewStats() }),
        qc.invalidateQueries({ queryKey: keys.workspacesAll() }),
      ]),
  })
}

export function useSyncReview(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async () => {
      await pendingApi.POST("/workspaces/{ws}/review/sync", {
        params: { path: { ws } },
      })
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.ws(ws).review() }),
  })
}
