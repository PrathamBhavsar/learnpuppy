import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { pendingApi, unwrap } from "../client"
import { keys } from "../keys"

export function useAccounts() {
  return useQuery({
    queryKey: keys.accounts(),
    queryFn: async ({ signal }) =>
      unwrap(await pendingApi.GET("/accounts", { signal })),
  })
}

export function useRescanAccounts() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async () => unwrap(await pendingApi.POST("/accounts/rescan")),
    onSuccess: (accounts) => qc.setQueryData(keys.accounts(), accounts),
  })
}

export function useAddClaudeProfile() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (path: string) =>
      unwrap(await pendingApi.POST("/accounts/claude", { body: { path } })),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.accounts() }),
  })
}

export function useSetAccountDisabled() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({ id, disabled }: { id: string; disabled: boolean }) =>
      unwrap(
        await pendingApi.PATCH("/accounts/{id}", {
          params: { path: { id } },
          body: { disabled },
        })
      ),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.accounts() }),
  })
}
