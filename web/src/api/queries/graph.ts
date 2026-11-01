import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query"

import { pendingApi, unwrap } from "../client"
import { keys } from "../keys"
import type { ChainTableRow } from "../pending-types"

export function useGraph(ws: string) {
  return useQuery({
    queryKey: keys.ws(ws).graphSnapshot(),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/graph", {
          params: { path: { ws } },
          signal,
        })
      ),
    // While the server computes positions the snapshot says so; poll until it is ready.
    refetchInterval: (query) =>
      query.state.data?.layoutPending ? 1500 : false,
  })
}

export function useGraphNode(ws: string, nodeId: string | null | undefined) {
  return useQuery({
    queryKey: keys.ws(ws).graphNode(nodeId ?? ""),
    enabled: Boolean(nodeId),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/graph/nodes/{id}", {
          params: { path: { ws, id: nodeId ?? "" } },
          signal,
        })
      ),
  })
}

export function useGraphSearch(ws: string, q: string) {
  const term = q.trim()
  return useQuery({
    queryKey: keys.ws(ws).graphSearch(term),
    enabled: term.length > 0,
    placeholderData: keepPreviousData,
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/graph/search", {
          params: { path: { ws }, query: { q: term } },
          signal,
        })
      ).items,
  })
}

export function useChainTable(ws: string, selection: readonly string[]) {
  return useQuery({
    queryKey: keys.ws(ws).graphChains(selection),
    queryFn: async ({ signal }): Promise<ChainTableRow[]> => {
      const table = unwrap(
        await pendingApi.GET("/workspaces/{ws}/graph/chains", {
          params: {
            path: { ws },
            query: selection.length ? { selection: selection.join(",") } : {},
          },
          signal,
        })
      )
      return table.groups.flatMap((group) =>
        group.chains.map(({ chain }) => ({
          groupId: group.sessionId,
          groupLabel: group.label,
          chainId: chain.id,
          from: chain.from,
          to: chain.to,
          kind: chain.kind,
        }))
      )
    },
  })
}

export function useRelayout(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async () =>
      unwrap(
        await pendingApi.POST("/workspaces/{ws}/graph/relayout", {
          params: { path: { ws } },
        })
      ),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.ws(ws).graph() }),
  })
}

export function useDeletePreview(ws: string) {
  return useMutation({
    mutationFn: async (nodeIds: string[]) =>
      unwrap(
        await pendingApi.POST("/workspaces/{ws}/graph/delete-preview", {
          params: { path: { ws } },
          body: { nodeIds },
        })
      ),
  })
}

export function usePracticeFromSelection(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (nodeIds: string[]) =>
      unwrap(
        await pendingApi.POST("/workspaces/{ws}/graph/practice", {
          params: { path: { ws } },
          body: { nodeIds },
        })
      ),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.ws(ws).chats() }),
  })
}

/** Applies a delete the user confirmed in the preview dialog; the token binds it to that preview. */
export function useDeleteNodes(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (body: { nodeIds: string[]; previewToken: string }) =>
      unwrap(
        await pendingApi.DELETE("/workspaces/{ws}/graph/nodes", {
          params: { path: { ws } },
          body,
        })
      ),
    onSuccess: () =>
      Promise.all([
        qc.invalidateQueries({ queryKey: keys.ws(ws).graph() }),
        qc.invalidateQueries({ queryKey: keys.workspacesAll() }),
      ]),
  })
}
