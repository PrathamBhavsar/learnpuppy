import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { pendingApi, unwrap } from "../client"
import { keys } from "../keys"
import type { IngestSources } from "../pending-types"

export function useIngestSources(ws: string) {
  return useQuery({
    queryKey: keys.ws(ws).ingestSources(),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/ingest/sources", {
          params: { path: { ws } },
          signal,
        })
      ),
  })
}

export function useSaveIngestSources(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (body: Omit<IngestSources, "candidates" | "overlaps">) =>
      unwrap(
        await pendingApi.PUT("/workspaces/{ws}/ingest/sources", {
          params: { path: { ws } },
          body,
        })
      ),
    onSuccess: (sources) => {
      qc.setQueryData(keys.ws(ws).ingestSources(), sources)
      void qc.invalidateQueries({ queryKey: keys.workspacesAll() })
    },
  })
}

export function useIngestRuns(ws: string) {
  return useQuery({
    queryKey: keys.ws(ws).ingestRuns(),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/ingest/runs", {
          params: { path: { ws } },
          signal,
        })
      ),
  })
}

export function useRunIngest(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (sinceDays?: number) =>
      unwrap(
        await pendingApi.POST("/workspaces/{ws}/ingest/run", {
          params: { path: { ws } },
          body: sinceDays === undefined ? {} : { sinceDays },
        })
      ),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: keys.ws(ws).ingestRuns() }),
  })
}
