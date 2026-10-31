import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { pendingApi, unwrap } from "../client"
import { keys } from "../keys"

export type DigestRange = { from?: string; to?: string; source?: string }

export function useDigest(ws: string, range: DigestRange = {}) {
  return useQuery({
    queryKey: keys.ws(ws).digest(range),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/digest", {
          params: { path: { ws }, query: range },
          signal,
        })
      ),
  })
}

export function useDecision(ws: string, id: string | null | undefined) {
  return useQuery({
    queryKey: keys.ws(ws).decision(id ?? ""),
    enabled: Boolean(id),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/digest/decisions/{id}", {
          params: { path: { ws, id: id ?? "" } },
          signal,
        })
      ),
  })
}

export function useRegenerateCards(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (decisionId: string) =>
      unwrap(
        await pendingApi.POST("/workspaces/{ws}/digest/decisions/{id}/cards", {
          params: { path: { ws, id: decisionId } },
        })
      ),
    onSuccess: () =>
      Promise.all([
        qc.invalidateQueries({ queryKey: keys.ws(ws).digestAll() }),
        qc.invalidateQueries({ queryKey: keys.ws(ws).review() }),
      ]),
  })
}
