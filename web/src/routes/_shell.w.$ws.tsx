import { Outlet, createFileRoute, useRouterState } from "@tanstack/react-router"
import { useEffect } from "react"

import { useActiveWorkspace } from "@/hooks/use-workspace"
import { rememberSection } from "@/features/workspaces/model"
import { WorkspaceNotFound } from "@/features/workspaces/WorkspaceNotFound"

/**
 * Guards every `/w/:ws/*` screen: an unknown, archived or deleted id gets the "Workspace not found"
 * page with a picker, and the section last visited is remembered per workspace for the picker.
 */
function WorkspaceLayout() {
  const { ws } = Route.useParams()
  const { notFound } = useActiveWorkspace()
  const section = useRouterState({
    select: (s) => s.location.pathname.split("/")[3] ?? "",
  })
  useEffect(() => {
    if (!notFound) rememberSection(ws, section)
  }, [ws, section, notFound])
  return notFound ? <WorkspaceNotFound /> : <Outlet />
}

export const Route = createFileRoute("/_shell/w/$ws")({
  component: WorkspaceLayout,
})
