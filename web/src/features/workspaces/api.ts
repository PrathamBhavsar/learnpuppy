import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { api, pendingApi, unwrap } from "@/api/client"
import { keys } from "@/api/keys"

/** `POST /workspaces/{ws}/repair`: rewrites minimal metadata for a damaged workspace from its directory name. */
export function useRepairWorkspace() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (ws: string) =>
      unwrap(
        await pendingApi.POST("/workspaces/{ws}/repair", {
          params: { path: { ws } },
        })
      ),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.workspacesAll() }),
  })
}

/** Data directory from `/health`, used to show where a workspace lives on disk. */
export function useDataDir() {
  return useQuery({
    queryKey: ["health"] as const,
    staleTime: Infinity,
    queryFn: async ({ signal }) =>
      unwrap(await api.GET("/health", { signal })).dataDir,
  })
}
