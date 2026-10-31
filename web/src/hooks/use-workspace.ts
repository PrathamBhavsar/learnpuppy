import { useParams } from "@tanstack/react-router"

import { useWorkspaces } from "@/api/queries/workspaces"
import type { WorkspaceListItem } from "@/api/pending-types"
import { useUi } from "@/stores/ui"

export type ActiveWorkspace = {
  /** From the URL when inside `/w/:ws`, else the last visited, else the first live workspace. */
  id: string | null
  workspace: WorkspaceListItem | undefined
  workspaces: readonly WorkspaceListItem[]
  /** `:ws` is in the URL (as opposed to a global route using the remembered workspace). */
  inWorkspaceRoute: boolean
  isLoading: boolean
  isError: boolean
  /** Workspaces loaded but the URL's id is not among them. */
  notFound: boolean
}

export function useActiveWorkspace(): ActiveWorkspace {
  const { ws } = useParams({ strict: false })
  const last = useUi((s) => s.lastWorkspace)
  const query = useWorkspaces(false)
  const workspaces = query.data ?? []
  const byId = (id: string | null | undefined) =>
    id ? workspaces.find((w) => w.id === id) : undefined
  const workspace = byId(ws) ?? (ws ? undefined : (byId(last) ?? workspaces[0]))
  return {
    id: ws ?? workspace?.id ?? null,
    workspace,
    workspaces,
    inWorkspaceRoute: Boolean(ws),
    isLoading: query.isPending,
    isError: query.isError,
    notFound: Boolean(ws) && query.isSuccess && !byId(ws),
  }
}

/** Workspace display name for page titles; empty until the list has loaded. */
export function useWorkspaceName(ws: string): string | undefined {
  return useWorkspaces(false).data?.find((w) => w.id === ws)?.name
}
