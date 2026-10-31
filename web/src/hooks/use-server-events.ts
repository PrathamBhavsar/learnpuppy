import { useQueryClient } from "@tanstack/react-query"
import { useEffect } from "react"

import { SseManager } from "@/api/events"
import type { UiHooks } from "@/api/event-table"

let manager: SseManager | null = null

/** The tab's single SSE manager (created on first use with the app's query client). */
function getSseManager(
  queryClient: ReturnType<typeof useQueryClient>
): SseManager {
  manager ??= new SseManager({ queryClient })
  return manager
}

/** Replace the singleton; tests and mock mode use this to inject a fake `EventSource`. */
export function setSseManager(next: SseManager | null): void {
  manager?.disconnect()
  manager = next
}

/**
 * Keeps the tab's event stream on `workspaceId` (null = global events only) and routes the UI-level
 * events (`workspace.deleted`, `export.ready`) to `ui`.
 */
export function useServerEvents(
  workspaceId: string | null,
  ui?: UiHooks
): void {
  const queryClient = useQueryClient()
  useEffect(() => {
    const sse = getSseManager(queryClient)
    sse.setUiHooks(ui)
  }, [queryClient, ui])
  useEffect(() => {
    getSseManager(queryClient).connect(workspaceId)
  }, [queryClient, workspaceId])
  useEffect(
    () => () => {
      manager?.disconnect()
    },
    []
  )
}
