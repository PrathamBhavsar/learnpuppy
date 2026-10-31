import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { pendingApi, unwrap } from "../client"
import { keys } from "../keys"
import type {
  IngestRun,
  IngestSources,
  IngestSourcesBody,
  IngestSourcesWire,
} from "../pending-types"

export function useIngestSources(ws: string) {
  return useQuery({
    queryKey: keys.ws(ws).ingestSources(),
    enabled: ws !== "",
    queryFn: async ({ signal }): Promise<IngestSources> => {
      const wire = unwrap(
        await pendingApi.GET("/workspaces/{ws}/ingest/sources", {
          params: { path: { ws } },
          signal,
        })
      )
      return toView(wire)
    },
  })
}

function toView(wire: IngestSourcesWire): IngestSources {
  return {
    ...wire.sources,
    enabled: wire.enabled,
    runningId: wire.runningId ?? null,
    candidates: wire.candidates,
    overlaps: wire.warnings,
  }
}

export function useSaveIngestSources(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (body: IngestSourcesBody): Promise<IngestSources> =>
      toView(
        unwrap(
          await pendingApi.PUT("/workspaces/{ws}/ingest/sources", {
            params: { path: { ws } },
            body,
          })
        )
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
    enabled: ws !== "",
    queryFn: async ({ signal }): Promise<IngestRun[]> =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/ingest/runs", {
          params: { path: { ws } },
          signal,
        })
      ).items,
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
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.ws(ws).ingestRuns() })
      void qc.invalidateQueries({ queryKey: keys.ws(ws).ingestSources() })
    },
  })
}

export function useCancelIngest(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (runId: string) => {
      await pendingApi.POST("/workspaces/{ws}/ingest/runs/{id}/cancel", {
        params: { path: { ws, id: runId } },
      })
    },
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: keys.ws(ws).ingestRuns() })
      void qc.invalidateQueries({ queryKey: keys.ws(ws).ingestSources() })
    },
  })
}
