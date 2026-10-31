import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import type { Client } from "openapi-fetch"
import { pendingApi, unwrap } from "@/api/client"
import { keys } from "@/api/keys"
import type {
  DecisionDetail,
  DigestDay,
  NodeSource,
  RegenerateResult,
} from "@/api/pending-types"
import type { components, paths } from "./schema"

const api = pendingApi as unknown as Client<paths>

type Dto = components["schemas"]
type Range = { from?: string; to?: string; source?: string }

function normalizeDigest(data: Dto["DigestDto"] | DigestDay[]): DigestDay[] {
  if (Array.isArray(data)) return data
  return data.days.map((day) => ({
    ...day,
    sessions: day.sessions.map((session) => ({
      ...session,
      source: session.source as NodeSource,
      startedAt: session.startedAt ?? 0,
    })),
  }))
}

export function useDigest(ws: string, range: Range) {
  return useQuery({
    queryKey: keys.ws(ws).digest(range),
    queryFn: async ({ signal }) =>
      normalizeDigest(
        unwrap(
          await api.GET("/workspaces/{ws}/digest", {
            // UI dates are inclusive; the API's `to` is the start of the following UTC day.
            params: {
              path: { ws },
              query: {
                ...range,
                to: range.to
                  ? new Date(Date.parse(`${range.to}T00:00:00Z`) + 86_400_000)
                      .toISOString()
                      .slice(0, 10)
                  : undefined,
              },
            },
            signal,
          })
        )
      ).filter((day) => !range.to || day.day <= range.to),
  })
}

function normalizeDecision(data: Dto["DecisionDetailDto"] | DecisionDetail) {
  if (!("decision" in data))
    return { ...data, cards: [] as Dto["CardBriefDto"][], commitsError: null }
  return {
    ...data.decision,
    commits: data.commits.map((commit) => ({ ...commit, at: undefined })),
    commitsError: data.commitsError,
    cards: data.cards,
  }
}

export function useDecision(ws: string, id: string | null) {
  return useQuery({
    queryKey: keys.ws(ws).decision(id ?? ""),
    enabled: Boolean(id),
    queryFn: async ({ signal }) =>
      normalizeDecision(
        unwrap(
          await api.GET("/workspaces/{ws}/digest/decisions/{id}", {
            params: { path: { ws, id: id ?? "" } },
            signal,
          })
        )
      ),
  })
}

export function useRegenerateCards(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (id: string): Promise<RegenerateResult> =>
      unwrap(
        await api.POST("/workspaces/{ws}/digest/decisions/{id}/cards", {
          params: { path: { ws, id } },
        })
      ),
    onSuccess: () =>
      Promise.all([
        qc.invalidateQueries({ queryKey: keys.ws(ws).digestAll() }),
        qc.invalidateQueries({ queryKey: keys.ws(ws).review() }),
      ]),
  })
}
