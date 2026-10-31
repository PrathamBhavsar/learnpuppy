import {
  type QueryClient,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query"

import { pendingApi, unwrap } from "../client"
import { keys } from "../keys"
import type {
  CreateWorkspaceBody,
  PatchWorkspaceBody,
  WorkspaceDetail,
  WorkspaceListItem,
} from "../pending-types"

export function useWorkspaces(archived = false) {
  return useQuery({
    queryKey: keys.workspaces(archived),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces", {
          params: { query: { archived } },
          signal,
        })
      ),
  })
}

export function useWorkspace(ws: string) {
  return useQuery({
    queryKey: keys.ws(ws).detail(),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}", {
          params: { path: { ws } },
          signal,
        })
      ),
  })
}

function invalidateLists(qc: QueryClient) {
  return Promise.all([
    qc.invalidateQueries({ queryKey: keys.workspacesAll() }),
    qc.invalidateQueries({ queryKey: keys.bootstrap() }),
  ])
}

export function useCreateWorkspace() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreateWorkspaceBody) =>
      unwrap(await pendingApi.POST("/workspaces", { body })),
    onSuccess: () => invalidateLists(qc),
  })
}

/** Optimistic rename/edit: the list and detail show the new name at once, rolled back on error. */
export function useUpdateWorkspace(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (body: PatchWorkspaceBody) =>
      unwrap(
        await pendingApi.PATCH("/workspaces/{ws}", {
          params: { path: { ws } },
          body,
        })
      ),
    onMutate: async (body) => {
      await qc.cancelQueries({ queryKey: keys.workspacesAll() })
      const lists = qc.getQueriesData<WorkspaceListItem[]>({
        queryKey: keys.workspacesAll(),
      })
      const detail = qc.getQueryData<WorkspaceDetail>(keys.ws(ws).detail())
      const { name, icon, hue, description } = body
      const patch = Object.fromEntries(
        Object.entries({ name, icon, hue, description }).filter(
          ([, v]) => v !== undefined
        )
      )
      for (const [key, list] of lists) {
        if (list)
          qc.setQueryData<WorkspaceListItem[]>(
            key,
            list.map((w) => (w.id === ws ? { ...w, ...patch } : w))
          )
      }
      if (detail)
        qc.setQueryData<WorkspaceDetail>(keys.ws(ws).detail(), {
          ...detail,
          ...patch,
        })
      return { lists, detail }
    },
    onError: (_error, _body, context) => {
      for (const [key, list] of context?.lists ?? []) qc.setQueryData(key, list)
      if (context?.detail) qc.setQueryData(keys.ws(ws).detail(), context.detail)
    },
    onSettled: () => {
      void invalidateLists(qc)
      void qc.invalidateQueries({ queryKey: keys.ws(ws).detail() })
    },
  })
}

export function useSetWorkspaceArchived() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({ ws, archived }: { ws: string; archived: boolean }) =>
      unwrap(
        await pendingApi.POST(
          archived ? "/workspaces/{ws}/archive" : "/workspaces/{ws}/restore",
          { params: { path: { ws } } }
        )
      ),
    onSuccess: () => invalidateLists(qc),
  })
}

export function useExportWorkspace() {
  return useMutation({
    mutationFn: async (ws: string) =>
      unwrap(
        await pendingApi.POST("/workspaces/{ws}/export", {
          params: { path: { ws } },
        })
      ),
  })
}

export function useTrash() {
  return useQuery({
    queryKey: keys.trash(),
    queryFn: async ({ signal }) =>
      unwrap(await pendingApi.GET("/trash", { signal })),
  })
}

export function useRestoreFromTrash() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (id: string) =>
      unwrap(
        await pendingApi.POST("/trash/{id}/restore", {
          params: { path: { id } },
        })
      ),
    onSuccess: () =>
      Promise.all([
        invalidateLists(qc),
        qc.invalidateQueries({ queryKey: keys.trash() }),
      ]),
  })
}

export function usePurgeFromTrash() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (id: string) => {
      await pendingApi.DELETE("/trash/{id}", { params: { path: { id } } })
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.trash() }),
  })
}

export function useGlobalSearch(q: string, limit = 30) {
  const term = q.trim()
  return useQuery({
    queryKey: keys.search(term),
    enabled: term.length > 0,
    staleTime: 10_000,
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/search", {
          params: { query: { q: term, limit } },
          signal,
        })
      ),
  })
}

export function useDeleteWorkspace() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({
      ws,
      confirmName,
    }: {
      ws: string
      confirmName: string
    }) => {
      await pendingApi.DELETE("/workspaces/{ws}", {
        params: { path: { ws } },
        body: { confirmName },
      })
    },
    onSuccess: (_d, { ws }) => {
      qc.removeQueries({ queryKey: keys.ws(ws).all })
      return Promise.all([
        invalidateLists(qc),
        qc.invalidateQueries({ queryKey: keys.trash() }),
      ])
    },
  })
}

export function useImportWorkspace() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (file: File) => {
      const form = new FormData()
      form.set("file", file)
      return unwrap(
        await pendingApi.POST("/workspaces/import", {
          body: form as unknown as { file: string },
          bodySerializer: (body) => body as unknown as FormData,
        })
      )
    },
    onSuccess: () => invalidateLists(qc),
  })
}
