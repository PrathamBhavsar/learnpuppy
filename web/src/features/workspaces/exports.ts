import { toast } from "sonner"
import { create } from "zustand"

import { API_PREFIX } from "@/api/client"
import { useExportWorkspace } from "@/api/queries/workspaces"
import { toastError, toastSuccess } from "@/lib/toast"

type ExportReady = { jobId: string; token: string }

type ExportsState = {
  /** jobId → workspace name, for exports started from this tab and not yet ready. */
  pending: Record<string, string>
  add: (jobId: string, name: string) => void
  take: (jobId: string) => string | undefined
}

const useExports = create<ExportsState>((set, get) => ({
  pending: {},
  add: (jobId, name) =>
    set((s) => ({ pending: { ...s.pending, [jobId]: name } })),
  take: (jobId) => {
    const name = get().pending[jobId]
    if (name === undefined) return undefined
    set((s) => {
      const pending = { ...s.pending }
      delete pending[jobId]
      return { pending }
    })
    return name
  },
}))

/** Starts the browser download for a one-shot export token (a plain navigation, no fetch). */
function download(token: string): void {
  const link = document.createElement("a")
  link.href = `${API_PREFIX}/downloads/${encodeURIComponent(token)}`
  link.download = ""
  document.body.append(link)
  link.click()
  link.remove()
}

/**
 * `export.ready` handler wired in the root route: finishes the progress toast of an export this tab
 * started with a Download action (user gesture, so the browser allows the file), or just announces
 * an export started elsewhere.
 */
export function notifyExportReady({ jobId, token }: ExportReady): void {
  const name = useExports.getState().take(jobId)
  if (name === undefined) {
    toastSuccess("Export ready")
    return
  }
  toast.success(`${name} export is ready`, {
    id: jobId,
    duration: 30_000,
    action: { label: "Download", onClick: () => download(token) },
  })
}

/** Kicks off an export and shows progress until the SSE `export.ready` arrives. */
export function useStartExport() {
  const mutation = useExportWorkspace()
  const add = useExports((s) => s.add)
  return {
    isPending: mutation.isPending,
    start: (workspace: { id: string; name: string }) =>
      mutation.mutate(workspace.id, {
        onSuccess: ({ jobId }) => {
          add(jobId, workspace.name)
          toast.loading(`Exporting ${workspace.name}…`, { id: jobId })
        },
        onError: (error) => toastError(error, "Export failed"),
      }),
  }
}
