import { useQueryClient } from "@tanstack/react-query"
import { useCallback } from "react"

import { keys } from "@/api/keys"
import type { SettingsDoc, UiSettings } from "@/api/pending-types"
import { useSaveSettings, useSettings } from "@/api/queries/settings"
import { toastError } from "@/lib/toast"

/**
 * Persists a change to the server's settings document. Reads the newest cached copy so two quick
 * changes do not overwrite each other with a stale document.
 */
export function useSettingsPatch() {
  const qc = useQueryClient()
  const save = useSaveSettings()
  const settings = useSettings()

  const patch = useCallback(
    (
      change: Partial<SettingsDoc> | { ui: Partial<UiSettings> },
      label: string
    ) => {
      const current = qc.getQueryData<SettingsDoc>(keys.settings())
      if (!current) return
      const next: SettingsDoc = {
        ...current,
        ...("ui" in change ? {} : change),
        ui: { ...current.ui, ...("ui" in change ? change.ui : {}) },
      }
      save.mutate(next, {
        onError: (error) => toastError(error, `Could not save ${label}`),
      })
    },
    [qc, save]
  )

  return { patch, settings, saving: save.isPending }
}
