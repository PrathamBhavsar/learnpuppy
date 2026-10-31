import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { pendingApi, unwrap } from "../client"
import { keys } from "../keys"
import type { SettingsDoc } from "../pending-types"

export function useSettings() {
  return useQuery({
    queryKey: keys.settings(),
    queryFn: async ({ signal }) =>
      unwrap(await pendingApi.GET("/settings", { signal })),
  })
}

export function useSettingsCatalog() {
  return useQuery({
    queryKey: keys.settingsCatalog(),
    queryFn: async ({ signal }) =>
      unwrap(await pendingApi.GET("/settings/catalog", { signal })),
    staleTime: 5 * 60_000,
  })
}

export function useSaveSettings() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (body: SettingsDoc) =>
      unwrap(await pendingApi.PUT("/settings", { body })),
    onSuccess: (doc) => {
      qc.setQueryData(keys.settings(), doc)
      void qc.invalidateQueries({ queryKey: keys.accounts() })
    },
  })
}

export function useDiagnostics() {
  return useMutation({
    mutationFn: async () => unwrap(await pendingApi.POST("/diagnostics")),
  })
}
