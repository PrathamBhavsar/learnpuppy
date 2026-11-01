import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { diagnosticsApi, pendingApi, unwrap } from "../client"
import { keys } from "../keys"
import type { ProfilePingBody, SettingsDoc } from "../pending-types"

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
    mutationFn: async () => unwrap(await diagnosticsApi.POST("/diagnostics")),
  })
}

/** 1-token ping through a role profile; the server answers `ok: false` rather than erroring on a bad model. */
export function usePingProfile() {
  return useMutation({
    mutationFn: async (body: ProfilePingBody) =>
      unwrap(await pendingApi.POST("/settings/test", { body })),
  })
}

/** "Use free models": every role on a free opencode or kiro model, with fallback across them. */
export function useApplyFreeDefaults() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async () =>
      unwrap(await pendingApi.POST("/settings/free-defaults")),
    onSuccess: (doc) => {
      qc.setQueryData(keys.settings(), doc)
      void qc.invalidateQueries({ queryKey: keys.settingsCatalog() })
    },
  })
}
